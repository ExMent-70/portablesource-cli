// portablesource
// Copyright (C) 2025  PortableSource / NeuroDonu
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Script generator module for creating platform-specific startup scripts.

// portablesource/src/installer/script_generator.rs

//! Script generator module for creating platform-specific startup scripts.

use crate::installer::{PipManager, MainFileFinder};
use crate::installer::templates;
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
        let bat_file = repo_path.join(format!("start_{}.bat", repo_name));
        let program_args = repo_info.program_args.clone().unwrap_or_default();
        
        // 1. Determine execution strategy (Main file vs Module vs Interactive)
        let launch_cmd = self.determine_launch_command_windows(repo_path, repo_info, &repo_name, &program_args)?;

        // 2. Generate CUDA environment variables block
        let cuda_section = self.generate_cuda_env_windows();

        // 3. Select Template
        let use_virtual_drive = self.needs_virtual_drive(&self.install_path);
        let template = if use_virtual_drive {
            templates::WINDOWS_BATCH_VDRIVE
        } else {
            templates::WINDOWS_BATCH_SIMPLE
        };

        // 4. Fill Template
        // Note: For simple template, we use absolute path. For VDrive, BASE_PATH is ignored/overwritten inside the script logic.
        let base_path_str = self.install_path.to_string_lossy().replace('\\', "\\\\");
        
        let content = template
            .replace("{{REPO_NAME}}", &repo_name)
            .replace("{{BASE_PATH}}", &base_path_str) // Only used in SIMPLE template
            .replace("{{CUDA_SECTION}}", &cuda_section)
            .replace("{{LAUNCH_CMD}}", &launch_cmd);

        // 5. Write File
        let mut f = fs::File::create(&bat_file)?;
        f.write_all(content.as_bytes())?;

        Ok(true)
    }

    /// Helper to determine what command to run (Windows)
    fn determine_launch_command_windows(&self, repo_path: &Path, repo_info: &RepositoryInfo, repo_name: &str, args: &str) -> Result<String> {
        // Strategy 1: Explicit Main File
        let mut main_file = repo_info.main_file.clone();
        if main_file.is_none() { 
            main_file = self.main_file_finder.find_main_file(repo_name, repo_path, repo_info.url.as_deref()); 
        }

        if let Some(main) = main_file {
            return Ok(format!("\"%python_exe%\" \"{}\" {}", main, args));
        }

        // Strategy 2: Pyproject Scripts
        let pyproject_path = repo_path.join("pyproject.toml");
        if pyproject_path.exists() {
            info!("Main file not found, checking pyproject.toml for scripts");
            let (_, script_module) = self.pip_manager.check_scripts_in_pyproject(repo_path)?;
            if let Some(module) = script_module {
                info!("Using pyproject.toml script: {}", module);
                return Ok(format!("\"%python_exe%\" -m {} {}", module, args));
            }
        }

        // Strategy 3: Interactive Fallback
        warn!("No main file or pyproject.toml scripts found, generating interactive Python shell");
        Ok("\"%python_exe%\"".to_string())
    }

    /// Helper to generate CUDA environment variables for Windows
    fn generate_cuda_env_windows(&self) -> String {
        if !self.config_manager.has_cuda() {
            return "REM No CUDA paths configured".to_string();
        }
        
        r#"set cuda_bin=%env_path%\CUDA\bin
set cuda_lib=%env_path%\CUDA\lib
set cuda_lib_64=%env_path%\CUDA\lib\x64
set cuda_nvml_bin=%env_path%\CUDA\nvml\bin
set cuda_nvml_lib=%env_path%\CUDA\nvml\lib
set cuda_nvvm_bin=%env_path%\CUDA\nvvm\bin
set cuda_nvvm_lib=%env_path%\CUDA\nvvm\lib

set PATH=%cuda_bin%;%PATH%
set PATH=%cuda_lib%;%PATH%
set PATH=%cuda_lib_64%;%PATH%
set PATH=%cuda_nvml_bin%;%PATH%
set PATH=%cuda_nvml_lib%;%PATH%
set PATH=%cuda_nvvm_bin%;%PATH%
set PATH=%cuda_nvvm_lib%;%PATH%"#.to_string()
    }

    /// Generate Unix shell script
    #[cfg(unix)]
    fn generate_startup_script_unix(&self, repo_path: &Path, repo_info: &RepositoryInfo) -> Result<bool> {
        use std::os::unix::fs::PermissionsExt;
        
        let repo_name = repo_path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let sh_file = repo_path.join(format!("start_{}.sh", repo_name));
        let program_args = repo_info.program_args.clone().unwrap_or_default();

        // 1. Determine Launch Command
        let launch_cmd = self.determine_launch_command_unix(repo_path, repo_info, &repo_name, &program_args)?;

        // 2. Generate CUDA Exports
        let cuda_exports = self.generate_cuda_env_unix();

        // 3. Fill Template
        let content = templates::UNIX_SHELL_SCRIPT
            .replace("{{INSTALL_PATH}}", &self.install_path.to_string_lossy())
            .replace("{{REPO_PATH}}", &repo_path.to_string_lossy())
            .replace("{{REPO_NAME}}", &repo_name)
            .replace("{{CUDA_EXPORTS}}", &cuda_exports)
            .replace("{{LAUNCH_CMD}}", &launch_cmd);

        // 4. Write File & Set Permissions
        let mut f = fs::File::create(&sh_file)?;
        f.write_all(content.as_bytes())?;
        let mut perms = fs::metadata(&sh_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&sh_file, perms)?;

        Ok(true)
    }

    #[cfg(unix)]
    fn determine_launch_command_unix(&self, repo_path: &Path, repo_info: &RepositoryInfo, repo_name: &str, args: &str) -> Result<String> {
        let mut main_file = repo_info.main_file.clone();
        if main_file.is_none() { 
            main_file = self.main_file_finder.find_main_file(repo_name, repo_path, repo_info.url.as_deref()); 
        }

        if let Some(main) = main_file {
            return Ok(format!(
                "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\" \"{}\" {}\nelse\n  exec python3 \"{}\" {}\nfi",
                main, args, main, args
            ));
        }

        let pyproject_path = repo_path.join("pyproject.toml");
        if pyproject_path.exists() {
            let (_, script_module) = self.pip_manager.check_scripts_in_pyproject(repo_path)?;
            if let Some(module) = script_module {
                info!("Using pyproject.toml script module: {}", module);
                return Ok(format!(
                    "if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\" -m {} {}\nelse\n  exec python3 -m {} {}\nfi",
                    module, args, module, args
                ));
            }
        }

        warn!("No main file or pyproject.toml scripts found, generating basic python launcher");
        Ok("if [[ -x \"$PYEXE\" ]]; then\n  exec \"$PYEXE\"\nelse\n  exec python3\nfi".to_string())
    }

    #[cfg(unix)]
    fn generate_cuda_env_unix(&self) -> String {
        if !self.config_manager.has_cuda() { return "".to_string(); }
        
        let mut exports = String::new();
        // Helpers to avoid Option boilerplate
        let get_path = |opt: Option<PathBuf>| opt.unwrap_or_default().to_string_lossy().to_string();
        
        let base = get_path(self.config_manager.get_cuda_base_path());
        let bin = get_path(self.config_manager.get_cuda_bin());
        let lib = get_path(self.config_manager.get_cuda_lib());
        let lib64 = get_path(self.config_manager.get_cuda_lib_64());

        exports.push_str(&format!("export CUDA_PATH=\"{}\"\n", base));
        exports.push_str(&format!("export CUDA_HOME=\"{}\"\n", base));
        exports.push_str(&format!("export CUDA_ROOT=\"{}\"\n", base));
        exports.push_str(&format!("export PATH=\"{}:$PATH\"\n", bin));
        exports.push_str(&format!("export LD_LIBRARY_PATH=\"{}:{}:${{LD_LIBRARY_PATH:-}}\"\n", lib, lib64));
        exports
    }

    /// Generate Unix shell script (no-op for non-Unix platforms)
    #[cfg(not(unix))]
    fn generate_startup_script_unix(&self, _repo_path: &Path, _repo_info: &RepositoryInfo) -> Result<bool> {
        Ok(true) // No-op on non-Unix platforms
    }

    /// Check if virtual drive is needed based on path characteristics
    fn needs_virtual_drive(&self, base_path: &Path) -> bool {
        let path_str = base_path.to_string_lossy();
        // Check path length > 150 characters or spaces or non-ascii
        path_str.len() > 150 || path_str.contains(' ') || !path_str.is_ascii()
    }
}