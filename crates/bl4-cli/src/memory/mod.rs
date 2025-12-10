//! Memory analysis module for Borderlands 4
//!
//! This module provides functionality to:
//! - Find and attach to the BL4 process (including under Proton/Wine)
//! - Read/write process memory
//! - Read from memory dump files (gcore output)
//! - Locate UE5 structures (GUObjectArray, GNames, etc.)
//! - Generate usmap files from live process or dumps
//! - Read and modify game state (inventory, stats, etc.)
//!
//! ## Module Structure
//!
//! - `constants` - SDK offsets and UE5 structure layouts
//! - `pattern` - SIMD-accelerated pattern scanning
//! - `source` - Memory source abstraction (live process, dump files)
//! - `fname` - FName pool reading
//! - `guobjects` - GUObjectArray iteration
//! - `reflection` - UClass discovery, properties, usmap generation
//! - `parts` - Part definition extraction

pub mod constants;
pub mod pattern;

// Re-export constants at module level for backwards compatibility
pub use constants::*;

// The rest of the original memory.rs functionality is included here
// TODO: Split into separate submodules:
// - source.rs (MemorySource trait, DumpFile, Bl4Process, MemoryRegion)
// - fname.rs (FNamePool, FNameReader, read_fname)
// - guobjects.rs (GUObjectArray, UObjectIterator)
// - reflection.rs (UClass discovery, properties, structs, enums, usmap)
// - parts.rs (PartDefinition, extract_part_definitions)

mod legacy;
pub use legacy::*;
