//! File-based extraction from unpacked game directories
//!
//! Walks extracted game directories to find and catalog manufacturers,
//! weapon types, balance data, naming strategies, gear types, rarity data,
//! and elemental data from .uasset files.

mod balance;
mod gear;
mod manufacturers;
mod types;

pub(crate) use balance::extract_naming_data;
pub(crate) use gear::{extract_elemental_data, extract_gear_types, extract_rarity_data};
pub(crate) use manufacturers::{extract_manufacturers, extract_weapon_types};
