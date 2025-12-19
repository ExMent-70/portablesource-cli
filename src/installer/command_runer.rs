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

// src/installer/command_runner.rs

use crate::{Result, PortableSourceError};
use crate::envs_manager::PortableEnvironmentManager;
use log::{info, debug};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// Enum для типизации команд. Он может остаться здесь.
#[derive(Clone, Copy, Debug)]
pub enum CommandType {
    Git,
    Pip,
    Uv,
    Python,
    Other,
}

/// CommandRunner - это централизованный исполнитель всех внешних команд.
/// Он держит ссылку на EnvironmentManager, чтобы правильно настраивать окружение.
pub struct CommandRunner<'a> {
    env_manager: &'a PortableEnvironmentManager,
}


impl<'a> CommandRunner<'a> {
    pub fn new(env_manager: &'a PortableEnvironmentManager) -> Self {
        Self { env_manager }
    }

    /// Публичный метод для запуска команды с логированием
    pub fn run(&self, args: &[String], label: Option<&str>, cwd: Option<&Path>) -> Result<()> {
        if args.is_empty() { return Ok(()); }
        
        // false = скрывать окно (стандартное поведение)
        let mut cmd = self.create_command(args, cwd, false);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        
        let command_type = self.determine_command_type(args);
        
        self.run_with_progress(cmd, label, command_type)
    }

    /// Публичный метод для "тихого" запуска
    pub fn run_silent(&self, args: &[String], label: Option<&str>, cwd: Option<&Path>) -> Result<()> {
        if args.is_empty() { return Ok(()); }
        if let Some(l) = label { info!("{}...", l); }

        // false = скрывать окно
        let mut cmd = self.create_command(args, cwd, false);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        
        let status = cmd.status().map_err(|e| PortableSourceError::command(e.to_string()))?;
        if !status.success() {
            return Err(PortableSourceError::command(format!("Silent command failed with status: {}", status)));
        }
        Ok(())
    }

    /// Запуск команды с прямым выводом в консоль (для отображения прогресс-баров)
    pub fn run_verbose(&self, args: &[String], label: Option<&str>, cwd: Option<&Path>) -> Result<()> {
        if args.is_empty() { return Ok(()); }
        
        if let Some(l) = label { info!("{}...", l); }

        // true = НЕ СКРЫВАТЬ ОКНО (Важное изменение!)
        let mut cmd = self.create_command(args, cwd, true);
        
        // Направляем вывод прямо в консоль пользователя
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        
        // Принудительно включаем прогресс-бары для pip/uv, даже если они думают, что это не TTY
        cmd.env("PYTHONUNBUFFERED", "1");
        cmd.env("PIP_PROGRESS_BAR", "on"); // Заставляет pip показывать полоску
        cmd.env("FORCE_COLOR", "1");       // Заставляет инструменты использовать цвета
        
        let status = cmd.status().map_err(|e| PortableSourceError::command(e.to_string()))?;
        
        if !status.success() {
            return Err(PortableSourceError::command(format!("Command failed with status: {}", status)));
        }
        Ok(())
    }

    // --- Приватные хелперы ---

    /// Создает объект `Command` с настроенным окружением.
    /// visible: если true, окно консоли будет видно (для run_verbose).
    fn create_command(&self, args: &[String], cwd: Option<&Path>, visible: bool) -> Command {
        // ИЗМЕНЕНИЕ ЗДЕСЬ: Добавляем эту строку, чтобы компилятор не ругался на Linux
        #[cfg(not(windows))]
        let _ = visible; 
		
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let envs = self.env_manager.setup_environment_for_subprocess();
        cmd.envs(envs);
        
        // Hide console window on Windows ONLY if visible is false
        #[cfg(windows)]
        {
            if !visible {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
        }
        
        cmd
    }
    
    /// Определяет тип команды по аргументам.
    fn determine_command_type(&self, args: &[String]) -> CommandType {
        // (Оставьте старый код этого метода)
        if args.len() >= 2 {
            let exe_name = Path::new(&args[0]).file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&args[0])
                .to_lowercase();
            
            if exe_name == "python" || exe_name == "python3" {
                if args.len() >= 3 && args[1] == "-m" {
                    match args[2].as_str() {
                        "pip" => CommandType::Pip,
                        "uv" => CommandType::Uv,
                        _ => CommandType::Python,
                    }
                } else {
                    CommandType::Python
                }
            } else if exe_name == "pip" || exe_name == "pip3" {
                CommandType::Pip
            } else if exe_name == "uv" {
                CommandType::Uv
            } else if exe_name == "git" {
                CommandType::Git
            } else {
                CommandType::Other
            }
        } else {
            CommandType::Other
        }
    }

    fn run_with_progress(&self, mut cmd: Command, label: Option<&str>, command_type: CommandType) -> Result<()> {
        // (Оставьте старый код этого метода)
        if let Some(l) = label { info!("{}...", l); }
        let mut child = cmd.spawn().map_err(|e| PortableSourceError::command(e.to_string()))?;
        
        let mut stderr_lines = Vec::new();
        
        let error_prefix = match command_type {
            CommandType::Git => "Git command failed",
            CommandType::Pip => "Pip command failed",
            CommandType::Uv => "UV command failed",
            CommandType::Python => "Python command failed",
            CommandType::Other => "Command failed",
        };
        
        if let Some(out) = child.stdout.take() {
            let reader = BufReader::new(out);
            for line in reader.lines().flatten() { debug!("[stdout] {}", line); }
        }
        
        if let Some(err) = child.stderr.take() {
            let reader = BufReader::new(err);
            for line in reader.lines().flatten() {
                debug!("[stderr] {}", line);
                stderr_lines.push(line);
            }
        }
        
        let status = child.wait().map_err(|e| PortableSourceError::command(e.to_string()))?;
        if !status.success() {
            let error_msg = if !stderr_lines.is_empty() {
                format!("Command failed with status: {}\nOutput:\n{}", status, stderr_lines.join("\n"))
            } else {
                format!("Command failed with status: {}", status)
            };
            debug!("{}: {}", error_prefix, error_msg);
            return Err(PortableSourceError::command(error_msg));
        }
        Ok(())
    }
}