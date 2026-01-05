//! NCS command type definitions

use serde::Serialize;
use std::collections::HashMap;

/// Result of scanning a directory
#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub total_files: usize,
    pub parsed_files: usize,
    pub types: HashMap<String, Vec<String>>,
    pub formats: HashMap<String, usize>,
}

/// Information about a single NCS file
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub type_name: String,
    pub format_code: String,
    pub entry_names: Vec<String>,
    pub guids: Vec<String>,
    pub numeric_values: Vec<(String, f64)>,
}

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub type_name: String,
    pub matches: Vec<String>,
}

/// Part index entry extracted from inv.bin
#[derive(Debug, Serialize)]
pub struct PartIndex {
    pub part_name: String,
    pub serial_index: u32,
    pub manufacturer: String,
    pub weapon_type: String,
}

/// Complete item-to-parts mapping extracted from inv.bin
#[derive(Debug, Serialize)]
pub struct ItemParts {
    /// Item identifier (e.g., "DAD_PS", "Armor_Shield", "Grenade_Standard")
    pub item_id: String,
    /// All valid parts for this item
    pub parts: Vec<String>,
    /// Legendary compositions (comp_05_legendary_*)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legendary_compositions: Vec<LegendaryComposition>,
}

/// Legendary composition with mandatory parts
#[derive(Debug, Serialize)]
pub struct LegendaryComposition {
    /// Composition name (e.g., "comp_05_legendary_Zipgun")
    pub name: String,
    /// Unique naming part (e.g., "uni_zipper")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_name: Option<String>,
    /// Mandatory unique parts
    pub mandatory_parts: Vec<String>,
}

/// NexusSerialized entry - maps internal codes to display names
/// Pattern in NCS: "NexusSerialized, {GUID}, {Display Name}"
#[derive(Debug, Serialize, Clone)]
pub struct NexusSerializedEntry {
    /// The GUID from the NexusSerialized entry
    pub guid: String,
    /// Display name (e.g., "Ripper Shotgun", "Daedalus Pistol")
    pub display_name: String,
    /// Parsed manufacturer code (e.g., "BOR", "DAD") if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer_code: Option<String>,
    /// Parsed weapon type (e.g., "Shotgun", "Pistol") if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_type: Option<String>,
}

/// Manufacturer mapping extracted from NexusSerialized
#[derive(Debug, Serialize)]
pub struct ManufacturerMapping {
    /// Internal code (e.g., "BOR", "DAD")
    pub code: String,
    /// Display name (e.g., "Ripper", "Daedalus")
    pub name: String,
}
