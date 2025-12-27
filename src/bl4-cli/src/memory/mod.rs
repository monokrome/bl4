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

// Re-export binary types
pub use binary::{CodeBounds, PeSection, find_code_bounds, scan_pattern};

// Re-export discovery functions
pub use discovery::{
    discover_class_uclass, discover_gnames, discover_guobject_array, find_ue5_offsets, read_fname,
};

// Re-export source types
pub use source::{Bl4Process, DumpFile, MemorySource, find_bl4_process};

// Re-export FName types
pub use fname::{FNamePool, FNameReader};

// Re-export UE5 types
pub use ue5::{GNamesPool, GUObjectArray, UObjectIterator, Ue5Offsets, GUOBJECTARRAY_VA};

// Re-export parts extraction
pub use parts::extract_parts_raw;

// Re-export reflection types
pub use reflection::{
    EPropertyType, EnumInfo, PropertyInfo, StructInfo, UClassMetaclassInfo, UObjectInfo,
    UObjectOffsets, discover_uclass_metaclass_exhaustive, find_all_uclasses,
};

// Re-export walker functions
pub use walker::{analyze_dump, extract_property, read_property_type, walk_guobject_array};

// Re-export usmap functions
pub use usmap::{
    extract_enum_values, extract_reflection_data, extract_struct_properties, write_usmap,
};

// Legacy module contains remaining memory functionality
// TODO: Further split into discovery.rs, reflection.rs
mod legacy;
pub use legacy::*;
