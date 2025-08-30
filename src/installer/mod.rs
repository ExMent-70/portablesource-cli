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

pub mod command_runer;
pub mod git_manager;
pub mod pip_manager;
pub mod dependency_installer;
pub mod script_generator;
pub mod server_client;
pub mod main_file_finder;

pub use command_runer::CommandRunner;
pub use git_manager::{GitManager, RepositoryInfo};
pub use pip_manager::PipManager;
pub use dependency_installer::DependencyInstaller;
pub use script_generator::{ScriptGenerator, RepositoryInfo as ScriptRepositoryInfo};
pub use server_client::{ServerClient, RepositoryInfo as ServerRepositoryInfo};
pub use main_file_finder::MainFileFinder;