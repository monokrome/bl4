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

#[allow(dead_code)]
mod file_extraction;
#[allow(dead_code)]
mod items_database;
#[allow(dead_code)]
mod pak_extraction;
#[allow(dead_code)]
mod pak_manifest;
#[allow(dead_code)]
mod property_parsing;
#[allow(dead_code)]
mod reference_data;
mod uasset_extraction;

pub(crate) use items_database::extract_manifest;

pub(crate) use pak_extraction::{
    extract_elements_from_pak, extract_gear_types_from_pak, extract_manufacturer_names_from_pak,
    extract_rarities_from_pak, extract_stats_from_pak, extract_weapon_types_from_pak,
};

pub(crate) use pak_manifest::PakManifest;

pub(crate) use uasset_extraction::extract_uasset_manifest;
