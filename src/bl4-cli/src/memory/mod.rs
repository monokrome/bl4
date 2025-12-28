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

pub mod binary;
pub mod constants;
pub mod discovery;
pub mod fname;
pub mod parts;
pub mod pattern;
pub mod reflection;
pub mod source;
pub mod ue5;
pub mod usmap;
pub mod walker;

// Re-export constants at module level
pub use constants::*;

// Re-export source types
pub use source::{Bl4Process, DumpFile, MemorySource};

// Re-export discovery functions
pub use discovery::{discover_class_uclass, discover_gnames, discover_guobject_array};

// Re-export FName types
pub use fname::{FNamePool, FNameReader};

// Re-export parts extraction
pub use parts::extract_parts_raw;

// Re-export walker functions
pub use walker::{analyze_dump, walk_guobject_array};

// Re-export usmap functions
pub use usmap::{extract_reflection_data, write_usmap};

// Re-export binary functions
pub use binary::{find_code_bounds, scan_pattern};

// Re-export reflection types
pub use reflection::find_all_uclasses;

// Legacy module contains remaining memory functionality
// TODO: Further split into discovery.rs, reflection.rs
mod legacy;
pub use legacy::*;
