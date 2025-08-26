//! Script generator module for creating platform-specific startup scripts.

use crate::installer::{PipManager, MainFileFinder};
use crate::config::ConfigManager;
use crate::Result;
use log::{info, warn};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct RepositoryInfo {
    pub url: Option<String>,
    pub main_file: Option<String>,
    pub program_args: Option<String>,
}

pub struct ScriptGenerator<'a> {
    pip_manager: &'a PipManager<'a>,
    config_manager: &'a ConfigManager,
    main_file_finder: &'a MainFileFinder,
    install_path: PathBuf,
}

impl<'a> ScriptGenerator<'a> {
    pub fn new(
        pip_manager: &'a PipManager,
        config_manager: &'a ConfigManager,
        main_file_finder: &'a MainFileFinder,
        install_path: PathBuf,
    ) -> Self {
        Self {
            pip_manager,
            config_manager,
            main_file_finder,
            install_path,
        }
    }

    /// Generate startup script for the repository (platform-specific)
    pub fn generate_startup_script(&self, repo_path: &Path, repo_info: &RepositoryInfo) -> Result<bool> {
        if cfg!(windows) {
            self.generate_startup_script_windows(repo_path, repo_info)
        } else {
            self.generate_startup_script_unix(repo_path, repo_info)
        }
    }

    /// Generate Windows batch script
    fn generate_startup_script_windows(&self, repo_path: &Path, repo_info: &RepositoryInfo) -> Result<bool> {
        let repo_name = repo_path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        
        let mut main_file = repo_info.main_file.clone();
        if main_file.is_none() { 
            main_file = self.main_file_finder.find_main_file(&repo_name, repo_path, repo_info.url.as_deref()); 
        }
        
        // Check for pyproject.toml scripts if main_file is not found
        let pyproject_path = repo_path.join("pyproject.toml");
        let (has_pyproject_scripts, script_module) = if main_file.is_none() && pyproject_path.exists() {
            info!("Main file not found, checking pyproject.toml for scripts");
            self.check_scripts_in_pyproject(repo_path)?
        } else {
            (false, None)
        };

        let bat_file = repo_path.join(format!("start_{}.bat", repo_name));
        let program_args = repo_info.program_args.clone().unwrap_or_default();

        // CUDA PATH section if configured
        let cuda_section = if self.config_manager.has_cuda() {
            format!(
                "set cuda_bin=%env_path%\\CUDA\\bin\nset cuda_lib=%env_path%\\CUDA\\lib\nset cuda_lib_64=%env_path%\\CUDA\\lib\\x64\nset cuda_nvml_bin=%env_path%\\CUDA\\nvml\\bin\nset cuda_nvml_lib=%env_path%\\CUDA\\nvml\\lib\nset cuda_nvvm_bin=%env_path%\\CUDA\\nvvm\\bin\nset cuda_nvvm_lib=%env_path%\\CUDA\\nvvm\\lib\n\nset PATH=%cuda_bin%;%PATH%\nset PATH=%cuda_lib%;%PATH%\nset PATH=%cuda_lib_64%;%PATH%\nset PATH=%cuda_nvml_bin%;%PATH%\nset PATH=%cuda_nvml_lib%;%PATH%\nset PATH=%cuda_nvvm_bin%;%PATH%\nset PATH=%cuda_nvvm_lib%;%PATH%\n"
            )
        } else { 
            "REM No CUDA paths configured".into() 
        };
        
        // Generate base script content without execution command
        let use_virtual_drive = self.needs_virtual_drive(&self.install_path);
        
        let base_content = if use_virtual_drive {
            // Use virtual drive for complex paths
            format!("@echo off\n") + &format!(
                "echo Launch {}...\n\nREM Setup cleanup trap for any exit\nset CLEANUP_DRIVE=\nset CLEANUP_NEEDED=0\n\nREM Get absolute path to install directory\nfor %%i in (\"%~dp0..\\..\\\") do set \"ROOT_PATH=%%~fi\"\n\nREM Clean up any leftover drives from previous runs\necho Checking for leftover virtual drives...\nfor %%d in (X Y Z W V U T S R Q P O N M L K J I H G F E D) do (\n    if exist %%d:\\ (\n        REM Check if it's a network/virtual drive that might be ours\n        for /f \"skip=1\" %%x in ('wmic logicaldisk where \"DeviceID='%%d:'\" get DriveType 2^>nul') do (\n            if \"%%x\"==\"4\" (\n                REM Network drive - check if it contains our files\n                if exist \"%%d:\\portablesource-rs.exe\" (\n                    echo Found leftover drive %%d: with our installation\n                ) else (\n                    REM Check if it points to our installation path\n                    for /f \"tokens=*\" %%p in ('subst %%d: 2^>nul') do (\n                        echo %%p | findstr /i \"%ROOT_PATH%\" >nul\n                        if not errorlevel 1 (\n                            echo Unmounting leftover drive %%d: pointing to our path\n                            subst %%d: /D >nul 2>&1\n                        )\n                    )\n                )\n            )\n        )\n    )\n)\n\nREM Smart drive letter selection with persistence\nset VDRIVE=\nset VDRIVE_FILE=\"%ROOT_PATH%\\vdrive.txt\"\n\nREM First try to read saved drive letter\nif exist %VDRIVE_FILE% (\n    set /p VDRIVE=<%VDRIVE_FILE%\n    echo Found saved drive letter: %VDRIVE%\n    \n\tif defined VDRIVE (\n\t\tREM Check if this drive exists in system\n\t\tif not exist %VDRIVE%:\\ (\n\t\t\techo Drive %VDRIVE%: does not exist, will mount it\n\t\t\tgoto :mount_drive\n\t\t) else (\n\t\t\techo Drive %VDRIVE%: exists, checking contents...\n\t\t\t\n\t\t\tREM Drive exists - check if it contains our portablesource-rs.exe\n\t\t\tif exist \"%VDRIVE%:\\portablesource-rs.exe\" (\n\t\t\t\techo Drive %VDRIVE%: contains our installation, not mounting\n\t\t\t\tgoto :use_existing_drive\n\t\t\t) else (\n\t\t\t\tREM Check if drive is empty\n\t\t\t\tdir %VDRIVE%:\\ /b >nul 2>&1\n\t\t\t\tif errorlevel 1 (\n\t\t\t\t\techo Drive %VDRIVE%: is empty, unmounting and reusing\n\t\t\t\t\tsubst %VDRIVE%: /D >nul 2>&1\n\t\t\t\t\tgoto :mount_drive\n\t\t\t\t) else (\n\t\t\t\t\techo Drive %VDRIVE%: is occupied by other software, finding new drive\n\t\t\t\t\tset VDRIVE=\n\t\t\t\t)\n\t\t\t)\n\t\t)\n\t)\n)\n\nREM Find new available drive letter\nif not defined VDRIVE (\n    echo Searching for new available drive letter...\n    for %%d in (X Y Z W V U T S R Q P O N M L K J I H G F E D) do (\n        if not exist %%d:\\ (\n            echo Drive %%d: does not exist - AVAILABLE\n            set VDRIVE=%%d\n            echo Selected new available drive: %%d\n            echo %%d>%VDRIVE_FILE%\n            goto :mount_drive\n        )\n    )\n    echo All drives checked, none available\n)\n\nREM No available drive found\nif not defined VDRIVE (\n    echo WARNING: No available drive letters found, using absolute path\n    set base_path=%ROOT_PATH%\n    echo Using absolute path: %base_path%\n    goto :setup_paths\n)\n\n:mount_drive\nset DRIVE_MOUNTED=1\nset CLEANUP_DRIVE=%VDRIVE%\nset CLEANUP_NEEDED=1\necho Mounting install path as %VDRIVE%: drive...\nsubst %VDRIVE%: \"%ROOT_PATH%\"\nif errorlevel 1 (\n    echo ERROR: Failed to mount virtual drive. Path: %ROOT_PATH%\n    pause\n    exit /b 1\n)\nREM Change drive and directory, then reliably continue execution\ncd /d %VDRIVE%:\\\ngoto :setup_paths\n\n:use_existing_drive\nset DRIVE_MOUNTED=0\nset CLEANUP_DRIVE=\nset CLEANUP_NEEDED=0\necho Using existing mounted drive %VDRIVE%:\nREM Change drive and directory, then reliably continue execution\ncd /d %VDRIVE%:\\\ngoto :setup_paths\n\n:setup_paths\nREM Setup Ctrl+C handler for cleanup\nif %CLEANUP_NEEDED%==1 (\n    set \"CLEANUP_CMD=if exist %CLEANUP_DRIVE%:\\ subst %CLEANUP_DRIVE%: /D >nul 2^>^&1\"\n)\n\nif defined VDRIVE (\n    set base_path=%VDRIVE%:\n) else (\n    REM base_path already set to %ROOT_PATH% in fallback mode\n)\nset env_path=%base_path%\\ps_env\nset envs_path=%base_path%\\envs\nset repos_path=%base_path%\\repos\nset ffmpeg_path=%env_path%\\ffmpeg\nset git_path=%env_path%\\git\\bin\nset python_path=%envs_path%\\{}\nset python_exe=%python_path%\\python.exe\nset repo_path=%repos_path%\\{}\n\nset tmp_path=%base_path%\\tmp\nset USERPROFILE=%tmp_path%\nset TEMP=%tmp_path%\\Temp\nset TMP=%tmp_path%\\Temp\nset APPDATA=%tmp_path%\\AppData\\Roaming\nset LOCALAPPDATA=%tmp_path%\\AppData\\Local\nset HF_HOME=%repo_path%\\huggingface_home\nset XDG_CACHE_HOME=%tmp_path%\nset HF_DATASETS_CACHE=%HF_HOME%\\datasets\n\nset PYTHONIOENCODING=utf-8\nset PYTHONUNBUFFERED=1\nset PYTHONDONTWRITEBYTECODE=1\n\nREM === CUDA PATHS ===\n{}\nset PATH=%python_path%;%PATH%\nset PATH=%python_path%\\Scripts;%PATH%\nset PATH=%git_path%;%PATH%\nset PATH=%ffmpeg_path%;%PATH%\n\ncd /d \"%repo_path%\"\n",
                repo_name,
                repo_name,
                repo_name,
                cuda_section,
            )
        } else {
            // Use direct paths for simple paths
            let install_path_str = self.install_path.to_string_lossy().replace('\\', "\\\\");
            format!("@echo off\n") + &format!(
                "echo Launch {}...\n\nset base_path={}\nset env_path=%base_path%\\ps_env\nset envs_path=%base_path%\\envs\nset repos_path=%base_path%\\repos\nset ffmpeg_path=%env_path%\\ffmpeg\nset git_path=%env_path%\\git\\bin\nset python_path=%envs_path%\\{}\nset python_exe=%python_path%\\python.exe\nset repo_path=%repos_path%\\{}\n\nset tmp_path=%base_path%\\tmp\nset USERPROFILE=%tmp_path%\nset TEMP=%tmp_path%\\Temp\nset TMP=%tmp_path%\\Temp\nset APPDATA=%tmp_path%\\AppData\\Roaming\nset LOCALAPPDATA=%tmp_path%\\AppData\\Local\nset HF_HOME=%repo_path%\\huggingface_home\nset XDG_CACHE_HOME=%tmp_path%\nset HF_DATASETS_CACHE=%HF_HOME%\\datasets\n\nset PYTHONIOENCODING=utf-8\nset PYTHONUNBUFFERED=1\nset PYTHONDONTWRITEBYTECODE=1\n\nREM === CUDA PATHS ===\n{}\nset PATH=%python_path%;%PATH%\nset PATH=%python_path%\\Scripts;%PATH%\nset PATH=%git_path%;%PATH%\nset PATH=%ffmpeg_path%;%PATH%\n\ncd /d \"%repo_path%\"\n",
                repo_name,
                install_path_str,
                repo_name,
                repo_name,
                cuda_section,
            )
        };
        
        // Determine execution command based on available options
        let content = if let Some(main_file_path) = main_file {
            // Case 1: main_file found - use it
            if use_virtual_drive {
                base_content + &format!(
                    "\"%python_exe%\" {} {}\nset EXIT_CODE=%ERRORLEVEL%\n\necho Cleaning up...\nif %CLEANUP_NEEDED%==1 (\n    if defined CLEANUP_DRIVE (\n        echo Unmounting drive %CLEANUP_DRIVE%:\n        subst %CLEANUP_DRIVE%: /D\n    )\n) else (\n    if defined VDRIVE (\n        echo Drive %VDRIVE%: was not mounted by us, leaving it\n    ) else (\n        echo Using absolute path mode, no drive to unmount\n    )\n)\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n",
                    main_file_path,
                    program_args,
                )
            } else {
                base_content + &format!(
                    "\"%python_exe%\" {} {}\nset EXIT_CODE=%ERRORLEVEL%\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n",
                    main_file_path,
                    program_args,
                )
            }
        } else if has_pyproject_scripts {
            // Case 2: no main_file but pyproject.toml has scripts
            if let Some(module_path) = script_module {
                info!("No main file found, using pyproject.toml script: {}", module_path);
                if use_virtual_drive {
                    base_content + &format!(
                        "\"%python_exe%\" -m {} {}\nset EXIT_CODE=%ERRORLEVEL%\n\necho Cleaning up...\nif %CLEANUP_NEEDED%==1 (\n    if defined CLEANUP_DRIVE (\n        echo Unmounting drive %CLEANUP_DRIVE%:\n        subst %CLEANUP_DRIVE%: /D\n    )\n) else (\n    if defined VDRIVE (\n        echo Drive %VDRIVE%: was not mounted by us, leaving it\n    ) else (\n        echo Using absolute path mode, no drive to unmount\n    )\n)\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n",
                        module_path,
                        program_args,
                    )
                } else {
                    base_content + &format!(
                        "\"%python_exe%\" -m {} {}\nset EXIT_CODE=%ERRORLEVEL%\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n",
                        module_path,
                        program_args,
                    )
                }
            } else {
                // Fallback case - should not happen but handle gracefully
                warn!("No main file or valid pyproject script found, generating interactive shell");
                if use_virtual_drive {
                    base_content + &format!(
                        "\"%python_exe%\"\nset EXIT_CODE=%ERRORLEVEL%\n\necho Cleaning up...\nif %CLEANUP_NEEDED%==1 (\n    if defined CLEANUP_DRIVE (\n        echo Unmounting drive %CLEANUP_DRIVE%:\n        subst %CLEANUP_DRIVE%: /D\n    )\n) else (\n    if defined VDRIVE (\n        echo Drive %VDRIVE%: was not mounted by us, leaving it\n    ) else (\n        echo Using absolute path mode, no drive to unmount\n    )\n)\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n"
                    )
                } else {
                    base_content + &format!(
                        "\"%python_exe%\"\nset EXIT_CODE=%ERRORLEVEL%\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n"
                    )
                }
            }
        } else {
            // Case 3: no main_file and no pyproject.toml - just python shell
            warn!("No main file or pyproject.toml scripts found, generating interactive Python shell");
            if use_virtual_drive {
                base_content + &format!(
                    "\"%python_exe%\"\nset EXIT_CODE=%ERRORLEVEL%\n\necho Cleaning up...\nif %CLEANUP_NEEDED%==1 (\n    if defined CLEANUP_DRIVE (\n        echo Unmounting drive %CLEANUP_DRIVE%:\n        subst %CLEANUP_DRIVE%: /D\n    )\n) else (\n    if defined VDRIVE (\n        echo Drive %VDRIVE%: was not mounted by us, leaving it\n    ) else (\n        echo Using absolute path mode, no drive to unmount\n    )\n)\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n"
                )
            } else {
                base_content + &format!(
                    "\"%python_exe%\"\nset EXIT_CODE=%ERRORLEVEL%\n\nif %EXIT_CODE% neq 0 (\n    echo.\n    echo Program finished with error (code: %EXIT_CODE%)\n) else (\n    echo.\n    echo Program finished successfully\n)\n\npause\n"
                )
            }
        };
        
        let mut f = fs::File::create(&bat_file)?;
        f.write_all(content.as_bytes())?;

        Ok(true)
    }

    /// Generate Unix shell script
    #[cfg(unix)]
    fn generate_startup_script_unix(&self, repo_path: &Path, repo_info: &RepositoryInfo) -> Result<bool> {
        use std::os::unix::fs::PermissionsExt;
        
        let repo_name = repo_path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let mut main_file = repo_info.main_file.clone();
        if main_file.is_none() { 
            main_file = self.main_file_finder.find_main_file(&repo_name, repo_path, repo_info.url.as_deref()); 
        }
        
        // Check for pyproject.toml scripts if main_file is not found
        let pyproject_path = repo_path.join("pyproject.toml");
        let (has_pyproject_scripts, script_module) = if main_file.is_none() && pyproject_path.exists() {
            info!("Main file not found, checking pyproject.toml for scripts");
            self.check_scripts_in_pyproject(repo_path)?
        } else {
            (false, None)
        };

        let install_path = &self.install_path;
        let sh_file = repo_path.join(format!("start_{}.sh", repo_name));
        let program_args = repo_info.program_args.clone().unwrap_or_default();
        
        // CUDA PATH exports if configured (optional)
        let mut cuda_exports = String::new();
        if self.config_manager.has_cuda() {
            let base_path = self.config_manager.get_cuda_base_path().unwrap_or_default();
            let bin_path = self.config_manager.get_cuda_bin().unwrap_or_default();
            let lib_path = self.config_manager.get_cuda_lib().unwrap_or_default();
            let lib64_path = self.config_manager.get_cuda_lib_64().unwrap_or_default();
            let base = base_path.to_string_lossy();
            let bin = bin_path.to_string_lossy();
            let lib = lib_path.to_string_lossy();
            let lib64 = lib64_path.to_string_lossy();
            cuda_exports.push_str(&format!("export CUDA_PATH=\"{}\"\n", base));
            cuda_exports.push_str(&format!("export CUDA_HOME=\"{}\"\n", base));
            cuda_exports.push_str(&format!("export CUDA_ROOT=\"{}\"\n", base));
            cuda_exports.push_str(&format!("export PATH=\"{}:$PATH\"\n", bin));
            // Use default expansion for unset variable due to 'set -u'
            cuda_exports.push_str(&format!("export LD_LIBRARY_PATH=\"{}:{}:${{LD_LIBRARY_PATH:-}}\"\n", lib, lib64));
        }

        // Generate base script content without execution command
        let base_content = format!("#!/usr/bin/env bash\nset -Eeuo pipefail\n\nINSTALL=\"{}\"\nENV_PATH=\"$INSTALL/ps_env\"\nBASE_PREFIX=\"$ENV_PATH/mamba_env\"\nREPO_PATH=\"{}\"\nVENV=\"$INSTALL/envs/{}\"\nPYEXE=\"$VENV/bin/python\"\n\n# Detect mode: allow override via PORTABLESOURCE_MODE\nMODE=\"${{PORTABLESOURCE_MODE:-}}\"\nif [[ -z \"$MODE\" ]]; then\n  if command -v git >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1 && command -v ffmpeg >/dev/null 2>&1; then\n    MODE=cloud\n  else\n    MODE=desk\n  fi\nfi\n\n# prepend micromamba base bin to PATH (no activation) in DESK mode\nif [[ \"$MODE\" == \"desk\" ]]; then\n  export PATH=\"$BASE_PREFIX/bin:$PATH\"\nfi\n\n# activate project venv if present (be tolerant to unset vars)\nif [[ -f \"$VENV/bin/activate\" ]]; then\n  set +u\n  source \"$VENV/bin/activate\" || true\n  set -u\nfi\n\n{}\ncd \"$REPO_PATH\"\n",
            install_path.to_string_lossy(),
            repo_path.to_string_lossy(),
            repo_name,
            cuda_exports,
        );
        
        // Determine execution command based on available options
        let content = if let Some(main_file) = main_file {
            // Use main_file if available
            base_content + &format!(
                "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\" \"{}\" {}\nelse\n  exec python3 \"{}\" {}\nfi\n",
                main_file,
                program_args,
                main_file,
                program_args,
            )
        } else if has_pyproject_scripts {
            if let Some(module_path) = script_module {
                info!("Using pyproject.toml script module: {}", module_path);
                base_content + &format!(
                    "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\" -m {} {}\nelse\n  exec python3 -m {} {}\nfi\n",
                    module_path,
                    program_args,
                    module_path,
                    program_args,
                )
            } else {
                warn!("pyproject.toml found but no suitable scripts detected");
                base_content + "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\"\nelse\n  exec python3\nfi\n"
            }
        } else {
            // No main_file and no pyproject.toml - just run python
            warn!("No main file or pyproject.toml scripts found, generating basic python launcher");
            base_content + "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\"\nelse\n  exec python3\nfi\n"
        };

        let mut f = fs::File::create(&sh_file)?;
        f.write_all(content.as_bytes())?;
        let mut perms = fs::metadata(&sh_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&sh_file, perms)?;

        Ok(true)
    }

    /// Generate Unix shell script (no-op for non-Unix platforms)
    #[cfg(not(unix))]
    fn generate_startup_script_unix(&self, _repo_path: &Path, _repo_info: &RepositoryInfo) -> Result<bool> {
        Ok(true) // No-op on non-Unix platforms
    }

    /// Check for pyproject.toml scripts
    fn check_scripts_in_pyproject(&self, repo_path: &Path) -> Result<(bool, Option<String>)> {
        self.pip_manager.check_scripts_in_pyproject(repo_path)
    }
    
    /// Check if virtual drive is needed based on path characteristics
    fn needs_virtual_drive(&self, base_path: &Path) -> bool {
        let path_str = base_path.to_string_lossy();
        
        // Check path length > 150 characters
        if path_str.len() > 150 {
            return true;
        }
        
        // Check for spaces in path
        if path_str.contains(' ') {
            return true;
        }
        
        // Check for non-ASCII characters (includes Russian text)
        if !path_str.is_ascii() {
            return true;
        }
        
        false
    }
}