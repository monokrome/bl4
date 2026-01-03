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
#[cfg(test)]
pub use mock::MockMemorySource;
pub use process::Bl4Process;

// Re-export for API completeness
#[allow(unused_imports)]
pub use process::find_bl4_process;
pub use region::MemoryRegion;
pub use traits::MemorySource;

#[cfg(test)]
pub mod tests {
    pub use super::mock::MockMemorySource;
}
