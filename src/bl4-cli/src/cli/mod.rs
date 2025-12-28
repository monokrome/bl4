//! CLI argument definitions for bl4
//!
//! This module contains all clap-derived structs and enums for CLI parsing.

mod core;
mod idb;
mod memory;
mod ncs;
#[cfg(feature = "research")]
mod research;
mod save;
mod serial;

pub use core::{Cli, Commands};
pub use idb::{ItemsDbCommand, OutputFormat};
pub use memory::{MemoryAction, PreloadAction};
pub use ncs::NcsCommand;
#[cfg(feature = "research")]
pub use research::{ExtractCommand, UsmapCommand};
pub use save::SaveCommand;
pub use serial::SerialCommand;
