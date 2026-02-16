//! Inventory part definitions parser (inv.bin)
//!
//! Parses the NCS inv.bin file which is the authoritative source for valid item parts.
//!
//! # Structure
//!
//! The inv.bin file contains:
//! - Item type definitions (e.g., `DAD_PS`, `Armor_Shield`)
//! - Valid parts for each item type (e.g., `DAD_PS_Barrel_01`)
//! - Legendary compositions with mandatory parts
//!
//! # Example
//!
//! ```ignore
//! use bl4_ncs::inventory::{parse_inventory, ItemParts};
//!
//! let data = std::fs::read("inv.bin")?;
//! let inventory = parse_inventory(&data)?;
//!
//! for item in &inventory.items {
//!     println!("{}: {} parts", item.item_id, item.parts.len());
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Known weapon manufacturers
const MANUFACTURERS: &[&str] = &["BOR", "DAD", "JAK", "MAL", "ORD", "TED", "TOR", "VLA"];

/// Known weapon types
const WEAPON_TYPES: &[&str] = &["AR", "HW", "PS", "SG", "SM", "SR"];

/// Serial index entry for a part
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialIndex {
    /// Part name (e.g., "part_barrel_01")
    pub part: String,
    /// Serial index number
    pub index: u32,
    /// Scope: "Root" for item types, "Sub" for parts
    pub scope: String,
    /// Slot type (e.g., "barrel", "grip")
    pub slot: Option<String>,
}

/// Extracted part indices from inv.bin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartIndices {
    /// Item type this set of parts belongs to (e.g., "DAD_PS", "Armor_Shield")
    pub item_type: String,
    /// Parts with their serial indices
    pub parts: Vec<SerialIndex>,
}

/// Parsed inventory containing all item types and their parts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// All item types with their valid parts
    pub items: Vec<ItemParts>,
    /// Total part count across all items
    pub total_parts: usize,
}

/// Item type with its valid parts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemParts {
    /// Item identifier (e.g., "DAD_PS", "Armor_Shield")
    pub item_id: String,
    /// Category of item (Weapon, Shield, etc.)
    pub category: ItemCategory,
    /// All valid parts for this item
    pub parts: Vec<String>,
    /// Legendary compositions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legendary_compositions: Vec<LegendaryComposition>,
}

/// Item category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemCategory {
    /// Weapon (MANU_TYPE pattern)
    Weapon {
        manufacturer: String,
        weapon_type: String,
    },
    /// Shield (Armor_Shield)
    Shield,
    /// Other/Unknown category
    Other,
}

/// Legendary composition with mandatory parts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendaryComposition {
    /// Composition name (e.g., "comp_05_legendary_Zipgun")
    pub name: String,
    /// Unique naming part (e.g., "uni_zipper")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_name: Option<String>,
    /// Mandatory unique parts
    pub mandatory_parts: Vec<String>,
}

/// Parse inventory data from raw bytes
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub fn parse_inventory(data: &[u8]) -> Option<Inventory> {
    let strings = extract_null_strings(data);
    if strings.is_empty() {
        return None;
    }

    let mut items: BTreeMap<String, ItemParts> = BTreeMap::new();

    // First pass: identify all item types and collect their parts
    for s in &strings {
        // Check for weapon type pattern: MANU_WEAPONTYPE (e.g., DAD_PS)
        if let Some((item_id, manufacturer, weapon_type)) = parse_weapon_type(s) {
            items.entry(item_id.clone()).or_insert_with(|| ItemParts {
                item_id,
                category: ItemCategory::Weapon {
                    manufacturer,
                    weapon_type,
                },
                parts: Vec::new(),
                legendary_compositions: Vec::new(),
            });
        }

        // Check for shield type
        if s == "Armor_Shield" {
            items.entry(s.to_string()).or_insert_with(|| ItemParts {
                item_id: s.to_string(),
                category: ItemCategory::Shield,
                parts: Vec::new(),
                legendary_compositions: Vec::new(),
            });
        }

        // Check for weapon part pattern: MANU_WEAPONTYPE_PartName
        if let Some((item_id, _part_name)) = parse_weapon_part(s) {
            if let Some(item) = items.get_mut(&item_id) {
                if !item.parts.contains(s) {
                    item.parts.push(s.clone());
                }
            }
        }

        // Shield parts: part_ra_* (reactive armor) and part_core_* (cores)
        if s.starts_with("part_ra_") || s.starts_with("part_core_") {
            if let Some(item) = items.get_mut("Armor_Shield") {
                if !item.parts.contains(s) {
                    item.parts.push(s.clone());
                }
            }
        }
    }

    // Second pass: identify legendary compositions
    let mut current_comp: Option<String> = None;
    let mut current_uni: Option<String> = None;

    for s in &strings {
        if s.starts_with("comp_05_legendary_") {
            current_comp = Some(s.clone());
            current_uni = None;
        } else if s.starts_with("uni_") && current_comp.is_some() {
            current_uni = Some(s.clone());
        } else if s.starts_with("part_") && current_comp.is_some() {
            let comp_name = current_comp.clone().unwrap();

            // Find which item this composition belongs to
            for item in items.values_mut() {
                if item.parts.iter().any(|p| {
                    p.contains(&s.replace("part_", "")) || s.contains(&item.item_id.replace('_', ""))
                }) {
                    if let Some(existing) = item
                        .legendary_compositions
                        .iter_mut()
                        .find(|c| c.name == comp_name)
                    {
                        if !existing.mandatory_parts.contains(s) {
                            existing.mandatory_parts.push(s.clone());
                        }
                    } else {
                        item.legendary_compositions.push(LegendaryComposition {
                            name: comp_name.clone(),
                            unique_name: current_uni.clone(),
                            mandatory_parts: vec![s.clone()],
                        });
                    }
                    break;
                }
            }
        }
    }

    // Sort parts within each item
    for item in items.values_mut() {
        item.parts.sort();
    }

    let items_vec: Vec<_> = items.into_values().collect();
    let total_parts = items_vec.iter().map(|i| i.parts.len()).sum();

    Some(Inventory {
        items: items_vec,
        total_parts,
    })
}

/// Check if a part is valid for an item
pub fn is_valid_part(inventory: &Inventory, item_id: &str, part_name: &str) -> bool {
    inventory
        .items
        .iter()
        .find(|i| i.item_id == item_id)
        .map(|i| i.parts.contains(&part_name.to_string()))
        .unwrap_or(false)
}

/// Get all parts for an item
pub fn get_parts<'a>(inventory: &'a Inventory, item_id: &str) -> Option<&'a [String]> {
    inventory
        .items
        .iter()
        .find(|i| i.item_id == item_id)
        .map(|i| i.parts.as_slice())
}

/// Get parts by slot type (barrel, grip, etc.)
pub fn get_parts_by_slot<'a>(inventory: &'a Inventory, item_id: &str, slot: &str) -> Vec<&'a String> {
    inventory
        .items
        .iter()
        .find(|i| i.item_id == item_id)
        .map(|i| {
            i.parts
                .iter()
                .filter(|p| p.to_lowercase().contains(&slot.to_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}


/// Check if string is a weapon part (MANU_TYPE_PartName pattern)
fn is_weapon_part(s: &str) -> bool {
    let parts: Vec<&str> = s.splitn(3, '_').collect();
    if parts.len() < 3 {
        return false;
    }
    MANUFACTURERS.contains(&parts[0]) && WEAPON_TYPES.contains(&parts[1])
}

/// Check if a string matches a part name pattern
#[allow(clippy::too_many_lines)]
fn is_part_pattern(s: &str) -> bool {
    // Too short
    if s.len() < 3 {
        return false;
    }

    // Too long (paths are not parts)
    if s.len() > 80 {
        return false;
    }

    // Must contain underscore for most part patterns
    if !s.contains('_') {
        // Exception: single-word rarity levels
        return s == "common" || s == "uncommon" || s == "rare" || s == "epic" || s == "legendary";
    }

    // Standard part prefixes
    if s.starts_with("part_")
        || s.starts_with("comp_")
        || s.starts_with("SHD_Aug_")
        || s.starts_with("uistat_")
        || s.starts_with("attr_")
        || s.starts_with("np_")
        || s.starts_with("ra_")
        || s.starts_with("uni_")
    {
        return true;
    }

    // Weapon parts: MANU_TYPE_PartName
    if is_weapon_part(s) {
        return true;
    }

    // Shield parts: part_ra_*, part_core_*
    if s.starts_with("part_ra_") || s.starts_with("part_core_") {
        return true;
    }

    // Composition patterns
    if s.starts_with("comp_") || s.starts_with("Weapon.base_comp_") || s.starts_with("Shield.comp_") {
        return true;
    }

    // Enhancement/Grenade patterns
    if s.contains("_Enhancement") || s.contains("_Grenade") {
        let parts: Vec<&str> = s.split('_').collect();
        // Make sure it's a short identifier, not a long path
        if parts.len() <= 5 {
            return true;
        }
    }

    // Hover drive parts
    if s.contains("hover_drive_rank_") || s.ends_with("_HoverDrive") {
        return true;
    }

    // Generic slot-based patterns (lowercase check)
    let lower = s.to_lowercase();
    if lower.starts_with("body_")
        || lower.starts_with("barrel_")
        || lower.starts_with("grip_")
        || lower.starts_with("magazine_")
        || lower.starts_with("scope_")
        || lower.starts_with("foregrip_")
        || lower.starts_with("underbarrel_")
        || lower.starts_with("payload_")
        || lower.starts_with("firmware_")
    {
        return true;
    }

    // Rarity levels
    if s == "common" || s == "uncommon" || s == "rare" || s == "epic" || s == "legendary" {
        return true;
    }

    false
}

/// Check if a string is a part name (legacy, use is_part_pattern)
#[allow(dead_code)]
fn is_part_name(s: &str) -> bool {
    is_part_pattern(s)
}


/// Raw string entry from NCS data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStringEntry {
    /// Index in string table
    pub string_index: usize,
    /// The string value
    pub value: String,
}

/// Extract all strings from inv.bin in order (raw, unfiltered)
///
/// Returns the complete string table for downstream processing.
pub fn extract_raw_strings(data: &[u8]) -> Vec<RawStringEntry> {
    extract_null_strings(data)
        .into_iter()
        .enumerate()
        .map(|(i, s)| RawStringEntry {
            string_index: i,
            value: s,
        })
        .collect()
}

/// Export raw strings to TSV format
pub fn raw_strings_to_tsv(strings: &[RawStringEntry]) -> String {
    let mut lines = vec!["index\tvalue".to_string()];
    for entry in strings {
        // Escape tabs and newlines in value
        let escaped = entry.value.replace('\t', "\\t").replace('\n', "\\n");
        lines.push(format!("{}\t{}", entry.string_index, escaped));
    }
    lines.join("\n")
}

/// Extract all (preceding_string, numeric_value) pairs from string table
///
/// For each numeric string, captures the preceding non-numeric string.
/// No filtering - outputs everything for downstream processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringNumericPair {
    /// Index of the preceding string
    pub string_index: usize,
    /// The preceding string value
    pub string_value: String,
    /// The numeric value that follows
    pub numeric_value: u32,
    /// Index of the numeric string in string table
    pub numeric_index: usize,
}

/// Extract all string-numeric pairs from the string table (raw, unfiltered)
pub fn extract_string_numeric_pairs(data: &[u8]) -> Vec<StringNumericPair> {
    let strings = extract_null_strings(data);
    let mut pairs = Vec::new();

    for i in 1..strings.len() {
        if let Ok(num) = strings[i].parse::<u32>() {
            if strings[i - 1].parse::<u32>().is_err() {
                pairs.push(StringNumericPair {
                    string_index: i - 1,
                    string_value: strings[i - 1].clone(),
                    numeric_value: num,
                    numeric_index: i,
                });
            }
        }
    }

    pairs
}

/// Export string-numeric pairs to TSV format
pub fn string_numeric_pairs_to_tsv(pairs: &[StringNumericPair]) -> String {
    let mut lines = vec!["string_index\tstring_value\tnumeric_value\tnumeric_index".to_string()];
    for pair in pairs {
        let escaped = pair.string_value.replace('\t', "\\t").replace('\n', "\\n");
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            pair.string_index, escaped, pair.numeric_value, pair.numeric_index
        ));
    }
    lines.join("\n")
}

/// Extract null-terminated strings from binary data
fn extract_null_strings(data: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();

    for &b in data {
        if b == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    if !s.is_empty() {
                        strings.push(s.to_string());
                    }
                }
                current.clear();
            }
        } else if (32..=126).contains(&b) {
            current.push(b);
        } else {
            current.clear();
        }
    }

    strings
}

/// Parse a weapon type identifier (e.g., "DAD_PS")
fn parse_weapon_type(s: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.len() != 2 {
        return None;
    }

    let manufacturer = parts[0];
    let weapon_type = parts[1];

    if !MANUFACTURERS.contains(&manufacturer) {
        return None;
    }

    if !WEAPON_TYPES.contains(&weapon_type) {
        return None;
    }

    Some((
        s.to_string(),
        manufacturer.to_string(),
        weapon_type.to_string(),
    ))
}

/// Parse a weapon part (e.g., "DAD_PS_Barrel_01")
fn parse_weapon_part(s: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = s.splitn(3, '_').collect();
    if parts.len() < 3 {
        return None;
    }

    let manufacturer = parts[0];
    let weapon_type = parts[1];

    if !MANUFACTURERS.contains(&manufacturer) {
        return None;
    }

    if !WEAPON_TYPES.contains(&weapon_type) {
        return None;
    }

    let rest = parts[2];
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }

    let item_id = format!("{}_{}", manufacturer, weapon_type);
    Some((item_id, rest.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_weapon_type() {
        assert_eq!(
            parse_weapon_type("DAD_PS"),
            Some((
                "DAD_PS".to_string(),
                "DAD".to_string(),
                "PS".to_string()
            ))
        );
        assert_eq!(
            parse_weapon_type("BOR_HW"),
            Some((
                "BOR_HW".to_string(),
                "BOR".to_string(),
                "HW".to_string()
            ))
        );
        assert_eq!(parse_weapon_type("DAD_PS_Barrel"), None);
        assert_eq!(parse_weapon_type("UNKNOWN_PS"), None);
        assert_eq!(parse_weapon_type("DAD_XX"), None);
    }

    #[test]
    fn test_parse_weapon_part() {
        assert_eq!(
            parse_weapon_part("DAD_PS_Barrel_01"),
            Some(("DAD_PS".to_string(), "Barrel_01".to_string()))
        );
        assert_eq!(
            parse_weapon_part("BOR_SG_Grip_05_A"),
            Some(("BOR_SG".to_string(), "Grip_05_A".to_string()))
        );
        assert_eq!(parse_weapon_part("DAD_PS"), None);
        assert_eq!(parse_weapon_part("UNKNOWN_PS_Barrel"), None);
    }

    #[test]
    fn test_extract_null_strings() {
        let data = b"hello\0world\0test\0";
        let strings = extract_null_strings(data);
        assert_eq!(strings, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_item_category_serialize() {
        let weapon = ItemCategory::Weapon {
            manufacturer: "DAD".to_string(),
            weapon_type: "PS".to_string(),
        };
        let json = serde_json::to_string(&weapon).unwrap();
        assert!(json.contains("weapon"));
        assert!(json.contains("DAD"));

        let shield = ItemCategory::Shield;
        let json = serde_json::to_string(&shield).unwrap();
        assert_eq!(json, "\"shield\"");
    }

    #[test]
    fn test_extract_raw_strings() {
        let data = b"alpha\0beta\0gamma\0";
        let raw = extract_raw_strings(data);
        assert_eq!(raw.len(), 3);
        assert_eq!(raw[0].string_index, 0);
        assert_eq!(raw[0].value, "alpha");
        assert_eq!(raw[1].string_index, 1);
        assert_eq!(raw[1].value, "beta");
        assert_eq!(raw[2].string_index, 2);
        assert_eq!(raw[2].value, "gamma");
    }

    #[test]
    fn test_extract_raw_strings_skips_non_ascii() {
        // Non-ASCII bytes clear the accumulator but don't prevent subsequent strings
        let data = b"good\0\x80\x81bad\0ok\0";
        let raw = extract_raw_strings(data);
        assert_eq!(raw.len(), 3);
        assert_eq!(raw[0].value, "good");
        assert_eq!(raw[1].value, "bad");
        assert_eq!(raw[2].value, "ok");

        // Non-ASCII mid-string truncates only the current token
        let data2 = b"hel\x80lo\0world\0";
        let raw2 = extract_raw_strings(data2);
        assert_eq!(raw2.len(), 2);
        assert_eq!(raw2[0].value, "lo");
        assert_eq!(raw2[1].value, "world");
    }

    #[test]
    fn test_extract_string_numeric_pairs_basic() {
        let data = b"part_barrel\042\0other\0100\0";
        let pairs = extract_string_numeric_pairs(data);
        assert_eq!(pairs.len(), 2);

        assert_eq!(pairs[0].string_value, "part_barrel");
        assert_eq!(pairs[0].numeric_value, 42);
        assert_eq!(pairs[0].string_index, 0);
        assert_eq!(pairs[0].numeric_index, 1);

        assert_eq!(pairs[1].string_value, "other");
        assert_eq!(pairs[1].numeric_value, 100);
    }

    #[test]
    fn test_extract_string_numeric_pairs_consecutive_numbers() {
        // Two consecutive numbers: only the first one preceded by a non-number gets paired
        let data = b"name\010\020\0";
        let pairs = extract_string_numeric_pairs(data);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].string_value, "name");
        assert_eq!(pairs[0].numeric_value, 10);
    }

    #[test]
    fn test_extract_string_numeric_pairs_no_numbers() {
        let data = b"alpha\0beta\0gamma\0";
        let pairs = extract_string_numeric_pairs(data);
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_raw_strings_to_tsv() {
        let entries = vec![
            RawStringEntry {
                string_index: 0,
                value: "hello".to_string(),
            },
            RawStringEntry {
                string_index: 1,
                value: "world\ttab".to_string(),
            },
        ];
        let tsv = raw_strings_to_tsv(&entries);
        assert!(tsv.starts_with("index\tvalue"));
        assert!(tsv.contains("0\thello"));
        assert!(tsv.contains("1\tworld\\ttab"));
    }

    #[test]
    fn test_is_part_pattern() {
        assert!(is_part_pattern("part_barrel_01"));
        assert!(is_part_pattern("comp_05_legendary_Zipgun"));
        assert!(is_part_pattern("DAD_PS_Barrel_01"));
        assert!(is_part_pattern("uni_zipper"));
        assert!(is_part_pattern("legendary"));
        assert!(!is_part_pattern("ab"));
        assert!(!is_part_pattern("no_match_here_from_unknown_prefix"));
    }

    #[test]
    #[ignore]
    fn test_raw_string_numeric_pairs() {
        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let pairs = extract_string_numeric_pairs(&data);
        println!("Total string-numeric pairs: {}", pairs.len());

        // Show first 100
        println!("\nFirst 100 pairs:");
        for pair in pairs.iter().take(100) {
            println!(
                "  [{}] {} -> {} at [{}]",
                pair.string_index, pair.string_value, pair.numeric_value, pair.numeric_index
            );
        }

        // Count how many have potential part patterns
        let part_patterns = pairs.iter().filter(|p| {
            let s = &p.string_value;
            s.starts_with("part_")
                || s.starts_with("comp_")
                || s.contains("_PS_")
                || s.contains("_SG_")
                || s.contains("_AR_")
                || s.contains("_SM_")
                || s.contains("_SR_")
                || s.contains("_HW_")
        }).count();
        println!("\nPairs with part-like patterns: {}", part_patterns);
    }

}
