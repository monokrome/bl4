//! PAK file extraction functions for game data
//!
//! Extracts authoritative game data from pak_manifest.json including:
//! - Manufacturers and their codes
//! - Weapon types and their manufacturers
//! - Gear types (shields, gadgets, etc.)
//! - Element types
//! - Rarity tiers
//! - Stat types and modifiers

mod attributes;
mod manufacturers;
mod weapon_gear;

pub(crate) use attributes::{
    extract_elements_from_pak, extract_rarities_from_pak, extract_stats_from_pak,
};
pub(crate) use manufacturers::extract_manufacturer_names_from_pak;
pub(crate) use weapon_gear::{extract_gear_types_from_pak, extract_weapon_types_from_pak};
