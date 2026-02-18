//! CLI argument definitions for bl4
//!
//! This module contains all clap-derived structs and enums for CLI parsing.

mod core;
mod drops;
mod idb;
mod memory;
mod ncs;
#[cfg(feature = "research")]
mod research;
mod save;
mod serial;

pub use core::{Cli, Commands};
pub use drops::DropsCommand;
pub use idb::{ItemsDbCommand, OutputFormat};
pub use memory::{MemoryAction, PreloadAction};
pub use ncs::NcsCommand;
#[cfg(feature = "research")]
pub use research::{ExtractCommand, UsmapCommand};
pub use save::{MapAction, SaveAction, SaveArgs};
pub use serial::SerialCommand;
