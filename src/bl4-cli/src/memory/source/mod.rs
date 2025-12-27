//! Memory Source Abstraction
//!
//! Core abstractions for reading memory from various sources:
//! - Live process attachment via `Bl4Process`
//! - Memory dump files via `DumpFile` (MDMP and gcore formats)
//! - Mock sources for testing

#![allow(dead_code)]
#![allow(clippy::manual_range_contains)]

mod dump;
mod mock;
mod process;
mod region;
mod traits;

pub use dump::DumpFile;
pub use mock::MockMemorySource;
pub use process::{find_bl4_process, get_tgid, parse_maps, Bl4Process};
pub use region::MemoryRegion;
pub use traits::MemorySource;

#[cfg(test)]
pub mod tests {
    pub use super::mock::MockMemorySource;
}
