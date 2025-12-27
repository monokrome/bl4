//! File-based extraction from unpacked game directories
//!
//! Walks extracted game directories to find and catalog manufacturers,
//! weapon types, balance data, naming strategies, gear types, rarity data,
//! and elemental data from .uasset files.

mod balance;
mod gear;
mod manufacturers;
mod types;

pub use balance::{extract_balance_data, extract_naming_data};
pub use gear::{extract_elemental_data, extract_gear_types, extract_rarity_data};
pub use manufacturers::{extract_manufacturers, extract_weapon_types};
pub use types::{BalanceCategory, GearType, Manufacturer, ManufacturerRef, WeaponType};
