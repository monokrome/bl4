//! Manifest extraction from game files
//!
//! Extracts game data from unpacked .uasset files into organized JSON manifest files.
//!
//! # Modules
//!
//! - `pak_extraction` - Extract data from pak_manifest.json (authoritative)
//! - `pak_manifest` - Generate pak manifest from uextract output
//! - `property_parsing` - Parse property strings from uasset files
//! - `file_extraction` - Walk extracted directories to find game data
//! - `items_database` - Generate consolidated items database
//! - `reference_data` - Wrapper functions for bl4::reference data

#![allow(dead_code)]

mod file_extraction;
mod items_database;
mod pak_extraction;
mod pak_manifest;
mod property_parsing;
mod reference_data;

// Re-export main types and functions
pub use file_extraction::{
    extract_balance_data, extract_elemental_data, extract_gear_types, extract_manufacturers,
    extract_naming_data, extract_rarity_data, extract_weapon_types, BalanceCategory, GearType,
    Manufacturer, ManufacturerRef, WeaponType,
};

pub use items_database::{
    extract_item_pools, extract_item_stats, extract_manifest, generate_items_database, ItemPool,
    ItemStats, ItemsDatabase, ManifestIndex, StatModifier, StatsSummary,
};

pub use pak_extraction::{
    extract_elements_from_pak, extract_gear_types_from_pak, extract_manufacturer_names_from_pak,
    extract_rarities_from_pak, extract_stats_from_pak, extract_weapon_types_from_pak,
    manufacturer_names, ExtractedElement, ExtractedGearType, ExtractedManufacturer,
    ExtractedRarity, ExtractedStat, ExtractedWeaponType,
};

pub use pak_manifest::{
    generate_pak_manifest, ExtractedItem, PakManifest, StatValue, UextractAsset, UextractExport,
    UextractProperty,
};

pub use property_parsing::{
    extract_strings, parse_property_strings, parse_stat_properties, stat_descriptions, AssetInfo,
    PropertyEntry, StatEntry, StatProperty,
};

pub use reference_data::{
    element_types, gear_type_info, generate_reference_manifest, known_legendaries,
    manufacturer_info, rarity_tiers, weapon_type_info, ConsolidatedManifest, ElementType,
    GearTypeInfo, LegendaryItem, ManufacturerInfo, RarityTier, WeaponTypeInfo,
};
