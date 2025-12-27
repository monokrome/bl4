//! Command handlers for bl4 CLI
//!
//! Each subcommand has its own module with handler functions.

pub mod configure;
pub mod items_db;
pub mod launch;
pub mod memory;
pub mod parts;
pub mod save;
pub mod serial;

#[cfg(feature = "research")]
pub mod extract;
#[cfg(feature = "research")]
pub mod manifest;
#[cfg(feature = "research")]
pub mod usmap;
