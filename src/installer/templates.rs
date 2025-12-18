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

// portablesource/src/installer/templates.rs

pub const WINDOWS_BATCH_SIMPLE: &str = r#"@echo off
echo Launch {{REPO_NAME}}...

set base_path={{BASE_PATH}}
set env_path=%base_path%\ps_env
set envs_path=%base_path%\envs
set repos_path=%base_path%\repos

set ffmpeg_path=%env_path%\ffmpeg
set git_path=%env_path%\git\bin
set python_path=%envs_path%\{{REPO_NAME}}
set python_exe=%python_path%\python.exe
set repo_path=%repos_path%\{{REPO_NAME}}

set tmp_path=%base_path%\tmp
set USERPROFILE=%tmp_path%
set TEMP=%tmp_path%\Temp
set TMP=%tmp_path%\Temp
set APPDATA=%tmp_path%\AppData\Roaming
set LOCALAPPDATA=%tmp_path%\AppData\Local
set HF_HOME=%repo_path%\huggingface_home
set XDG_CACHE_HOME=%tmp_path%
set HF_DATASETS_CACHE=%HF_HOME%\datasets

set PYTHONIOENCODING=utf-8
set PYTHONUNBUFFERED=1
set PYTHONDONTWRITEBYTECODE=1

REM === CUDA PATHS ===
{{CUDA_SECTION}}

set PATH=%python_path%;%PATH%
set PATH=%python_path%\Scripts;%PATH%
set PATH=%git_path%;%PATH%
set PATH=%ffmpeg_path%;%PATH%

cd /d "%repo_path%"
{{LAUNCH_CMD}}
set EXIT_CODE=%ERRORLEVEL%

if %EXIT_CODE% neq 0 (
    echo.
    echo Program finished with error (code: %EXIT_CODE%)
) else (
    echo.
    echo Program finished successfully
)

pause
"#;

pub const WINDOWS_BATCH_VDRIVE: &str = r#"@echo off
echo Launch {{REPO_NAME}}...

REM Setup cleanup trap for any exit
set CLEANUP_DRIVE=
set CLEANUP_NEEDED=0

REM Get absolute path to install directory
for %%i in ("%~dp0..\..\") do set "ROOT_PATH=%%~fi"

REM Clean up any leftover drives from previous runs
echo Checking for leftover virtual drives...
for %%d in (X Y Z W V U T S R Q P O N M L K J I H G F E D) do (
    if exist %%d:\ (
        REM Check if it's a network/virtual drive that might be ours
        for /f "skip=1" %%x in ('wmic logicaldisk where "DeviceID='%%d:'" get DriveType 2^>nul') do (
            if "%%x"=="4" (
                REM Network drive - check if it contains our files
                if exist "%%d:\portablesource-rs.exe" (
                    echo Found leftover drive %%d: with our installation
                ) else (
                    REM Check if it points to our installation path
                    for /f "tokens=*" %%p in ('subst %%d: 2^>nul') do (
                        echo %%p | findstr /i "%ROOT_PATH%" >nul
                        if not errorlevel 1 (
                            echo Unmounting leftover drive %%d: pointing to our path
                            subst %%d: /D >nul 2>&1
                        )
                    )
                )
            )
        )
    )
)

REM Smart drive letter selection with persistence
set VDRIVE=
set VDRIVE_FILE="%ROOT_PATH%\vdrive.txt"

REM First try to read saved drive letter
if exist %VDRIVE_FILE% (
    set /p VDRIVE=<%VDRIVE_FILE%
    echo Found saved drive letter: %VDRIVE%
    
    if defined VDRIVE (
        REM Check if this drive exists in system
        if not exist %VDRIVE%:\ (
            echo Drive %VDRIVE%: does not exist, will mount it
            goto :mount_drive
        ) else (
            echo Drive %VDRIVE%: exists, checking contents...
            
            REM Drive exists - check if it contains our portablesource-rs.exe
            if exist "%VDRIVE%:\portablesource-rs.exe" (
                echo Drive %VDRIVE%: contains our installation, not mounting
                goto :use_existing_drive
            ) else (
                REM Check if drive is empty
                dir %VDRIVE%:\ /b >nul 2>&1
                if errorlevel 1 (
                    echo Drive %VDRIVE%: is empty, unmounting and reusing
                    subst %VDRIVE%: /D >nul 2>&1
                    goto :mount_drive
                ) else (
                    echo Drive %VDRIVE%: is occupied by other software, finding new drive
                    set VDRIVE=
                )
            )
        )
    )
)

REM Find new available drive letter
if not defined VDRIVE (
    echo Searching for new available drive letter...
    for %%d in (X Y Z W V U T S R Q P O N M L K J I H G F E D) do (
        if not exist %%d:\ (
            echo Drive %%d: does not exist - AVAILABLE
            set VDRIVE=%%d
            echo Selected new available drive: %%d
            echo %%d>%VDRIVE_FILE%
            goto :mount_drive
        )
    )
    echo All drives checked, none available
)

REM No available drive found
if not defined VDRIVE (
    echo WARNING: No available drive letters found, using absolute path
    set base_path=%ROOT_PATH%
    echo Using absolute path: %base_path%
    goto :setup_paths
)

:mount_drive
set DRIVE_MOUNTED=1
set CLEANUP_DRIVE=%VDRIVE%
set CLEANUP_NEEDED=1
echo Mounting install path as %VDRIVE%: drive...
subst %VDRIVE%: "%ROOT_PATH%"
if errorlevel 1 (
    echo ERROR: Failed to mount virtual drive. Path: %ROOT_PATH%
    pause
    exit /b 1
)
REM Change drive and directory, then reliably continue execution
cd /d %VDRIVE%:\
goto :setup_paths

:use_existing_drive
set DRIVE_MOUNTED=0
set CLEANUP_DRIVE=
set CLEANUP_NEEDED=0
echo Using existing mounted drive %VDRIVE%:
REM Change drive and directory, then reliably continue execution
cd /d %VDRIVE%:\
goto :setup_paths

:setup_paths
REM Setup Ctrl+C handler for cleanup
if %CLEANUP_NEEDED%==1 (
    set "CLEANUP_CMD=if exist %CLEANUP_DRIVE%:\ subst %CLEANUP_DRIVE%: /D >nul 2^>^&1"
)

if defined VDRIVE (
    set base_path=%VDRIVE%:
) else (
    REM base_path already set to %ROOT_PATH% in fallback mode
)
set env_path=%base_path%\ps_env
set envs_path=%base_path%\envs
set repos_path=%base_path%\repos

set ffmpeg_path=%env_path%\ffmpeg
set git_path=%env_path%\git\bin
set python_path=%envs_path%\{{REPO_NAME}}
set python_exe=%python_path%\python.exe
set repo_path=%repos_path%\{{REPO_NAME}}

set tmp_path=%base_path%\tmp
set USERPROFILE=%tmp_path%
set TEMP=%tmp_path%\Temp
set TMP=%tmp_path%\Temp
set APPDATA=%tmp_path%\AppData\Roaming
set LOCALAPPDATA=%tmp_path%\AppData\Local
set HF_HOME=%repo_path%\huggingface_home
set XDG_CACHE_HOME=%tmp_path%
set HF_DATASETS_CACHE=%HF_HOME%\datasets

set PYTHONIOENCODING=utf-8
set PYTHONUNBUFFERED=1
set PYTHONDONTWRITEBYTECODE=1

REM === CUDA PATHS ===
{{CUDA_SECTION}}

set PATH=%python_path%;%PATH%
set PATH=%python_path%\Scripts;%PATH%
set PATH=%git_path%;%PATH%
set PATH=%ffmpeg_path%;%PATH%

cd /d "%repo_path%"
{{LAUNCH_CMD}}
set EXIT_CODE=%ERRORLEVEL%

echo Cleaning up...
if %CLEANUP_NEEDED%==1 (
    if defined CLEANUP_DRIVE (
        echo Unmounting drive %CLEANUP_DRIVE%:
        subst %CLEANUP_DRIVE%: /D
    )
) else (
    if defined VDRIVE (
        echo Drive %VDRIVE%: was not mounted by us, leaving it
    ) else (
        echo Using absolute path mode, no drive to unmount
    )
)

if %EXIT_CODE% neq 0 (
    echo.
    echo Program finished with error (code: %EXIT_CODE%)
) else (
    echo.
    echo Program finished successfully
)

pause
"#;

pub const UNIX_SHELL_SCRIPT: &str = r#"#!/usr/bin/env bash
set -Eeuo pipefail

INSTALL="{{INSTALL_PATH}}"
ENV_PATH="$INSTALL/ps_env"
BASE_PREFIX="$ENV_PATH/mamba_env"
REPO_PATH="{{REPO_PATH}}"
VENV="$INSTALL/envs/{{REPO_NAME}}"
PYEXE="$VENV/bin/python"

# Detect mode: allow override via PORTABLESOURCE_MODE
MODE="${PORTABLESOURCE_MODE:-}"
if [[ -z "$MODE" ]]; then
  if command -v git >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1 && command -v ffmpeg >/dev/null 2>&1; then
    MODE=cloud
  else
    MODE=desk
  fi
fi

# prepend micromamba base bin to PATH (no activation) in DESK mode
if [[ "$MODE" == "desk" ]]; then
  export PATH="$BASE_PREFIX/bin:$PATH"
fi

# activate project venv if present (be tolerant to unset vars)
if [[ -f "$VENV/bin/activate" ]]; then
  set +u
  source "$VENV/bin/activate" || true
  set -u
fi

{{CUDA_EXPORTS}}

cd "$REPO_PATH"
{{LAUNCH_CMD}}
"#;