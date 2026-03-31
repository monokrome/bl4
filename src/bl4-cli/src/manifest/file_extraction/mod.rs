//! File-based extraction from unpacked game directories
//!
//! Walks extracted game directories to find and catalog manufacturers,
//! weapon types, balance data, naming strategies, gear types, rarity data,
//! and elemental data from .uasset files.

use std::path::Path;

mod balance;
mod gear;
mod manufacturers;
mod types;

/// Convert a path to a string with forward slashes (cross-platform).
fn forward_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) use balance::extract_naming_data;
pub(crate) use gear::{extract_elemental_data, extract_gear_types, extract_rarity_data};
pub(crate) use manufacturers::{extract_manufacturers, extract_weapon_types};
