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

//! Command-line interface for PortableSource

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "portablesource")]
#[command(about = "PortableSource - Portable AI/ML Environment Manager")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// Enable debug logging
    #[arg(long)]
    pub debug: bool,
    
    /// Installation path
    #[arg(long)]
    pub install_path: Option<PathBuf>,
    
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Setup environment (Portable)
    SetupEnv,
    
    /// Register installation path in registry (Unix only)
    #[cfg(unix)]
    SetupReg,
    
    /// Unregister installation path from registry (Unix only)
    #[cfg(unix)]
    Unregister,
    
    /// Uninstall PortableSource completely (Linux only)
    #[cfg(unix)]
    Uninstall,
    
    /// Change installation path (Unix only)
    #[cfg(unix)]
    ChangePath,
    
    /// Install repository (alias: ir)
    #[command(alias = "ir")]
    InstallRepo {
        /// Repository URL or name
        repo: String,
    },
    
    /// Update repository (alias: ur)
    #[command(alias = "ur")]
    UpdateRepo {
        /// Repository name (optional; if omitted, a TUI selector will be shown)
        repo: Option<String>,
    },
    
    /// Delete repository (alias: dr)
    #[command(alias = "dr")]
    DeleteRepo {
        /// Repository name
        repo: String,
    },
    
    /// List installed repositories (alias: lr)
    #[command(alias = "lr")]
    ListRepos,

    /// Run repository start script (alias: rr)
    #[command(alias = "rr")]
    RunRepo {
        /// Repository name to run
        repo: String,
        /// Additional arguments to pass to the repository script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    
    /// Show system information
    SystemInfo,
    
    /// Check environment status and tools
    CheckEnv,
    
    #[cfg(windows)]
    /// Install MSVC Build Tools
    InstallMsvc,
    
    #[cfg(windows)]
    /// Check MSVC Build Tools installation
    CheckMsvc,
    
    /// Show True if gpu nvidia. Else False
    CheckGpu,
    
    /// Show version
    Version,
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }
    
    /// Check if any command was provided
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }
    
    /// Get the command or return a default help command
    pub fn get_command(&self) -> &Commands {
        self.command.as_ref().unwrap_or(&Commands::SystemInfo)
    }
}