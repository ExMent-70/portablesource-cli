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

//! PortableSource - Portable AI/ML Environment Manager
//! 
//! This is a Rust implementation of the PortableSource CLI tool,
//! originally written in Python.

pub mod cli;
pub mod config;
pub mod gpu;
pub mod utils;
pub mod envs_manager;
pub mod installer;
pub mod repository_installer;
pub mod error;

pub use error::{Result, PortableSourceError};