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

/// Extract serial indices from inv.bin binary section
pub fn extract_serial_indices(data: &[u8]) -> BTreeMap<String, PartIndices> {
    use crate::parser::{parse_header, find_binary_section_with_count};
    use crate::string_table::parse_string_table;

    let header = match parse_header(data) {
        Some(h) => h,
        None => return BTreeMap::new(),
    };

    let _strings = parse_string_table(data, &header);

    // Find binary offset
    let binary_offset = match find_binary_section_with_count(data, header.string_table_offset, Some(18393)) {
        Some(offset) => offset,
        None => return BTreeMap::new(),
    };

    let binary_data = &data[binary_offset..];
    let mut result: BTreeMap<String, PartIndices> = BTreeMap::new();

    // Tag-based extraction using best performing tag+offset combinations
    // Based on analysis:
    // - tag 'f' (0x66) at offset 27: 3,471 indices (63% of target)
    // - tag 'a' (0x61) at offset 5: 2,921 indices
    // Combined with position-based deduplication

    let mut position_value_pairs: std::collections::HashSet<(usize, u32)> = std::collections::HashSet::new();

    // Primary: tag 'f' at offset 27
    for i in 0..binary_data.len() {
        if binary_data[i] == 0x66 && i + 27 < binary_data.len() {
            let pos = i + 27;

            // Try u8 for values < 256
            let val_u8 = binary_data[pos] as u32;
            if val_u8 >= 1 && val_u8 < 256 {
                position_value_pairs.insert((pos, val_u8));
            }

            // Try u16 LE for values >= 256
            if pos + 1 < binary_data.len() {
                let val_u16 = u16::from_le_bytes([binary_data[pos], binary_data[pos + 1]]) as u32;
                if val_u16 >= 256 && val_u16 <= 541 {
                    position_value_pairs.insert((pos, val_u16));
                }
            }
        }
    }

    // Secondary: tag 'a' at offset 5
    for i in 0..binary_data.len() {
        if binary_data[i] == 0x61 && i + 5 < binary_data.len() {
            let pos = i + 5;

            let val_u8 = binary_data[pos] as u32;
            if val_u8 >= 1 && val_u8 < 256 {
                position_value_pairs.insert((pos, val_u8));
            }

            if pos + 1 < binary_data.len() {
                let val_u16 = u16::from_le_bytes([binary_data[pos], binary_data[pos + 1]]) as u32;
                if val_u16 >= 256 && val_u16 <= 541 {
                    position_value_pairs.insert((pos, val_u16));
                }
            }
        }
    }

    // Convert to result format
    let mut all_indices: Vec<u32> = position_value_pairs.iter().map(|(_, v)| *v).collect();
    all_indices.sort();

    for &index in &all_indices {
        let entry = result.entry("parts".to_string()).or_insert_with(|| PartIndices {
            item_type: "parts".to_string(),
            parts: Vec::new(),
        });

        entry.parts.push(SerialIndex {
            part: format!("serial_{}", index),
            index,
            scope: "Sub".to_string(),
            slot: None,
        });
    }

    result
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

/// Export serial indices to TSV format
///
/// Format: item_type\tslot\tpart\tserial_index
pub fn serial_indices_to_tsv(indices: &BTreeMap<String, PartIndices>) -> String {
    let mut lines = vec!["item_type\tslot\tpart\tserial_index".to_string()];

    for (item_type, part_indices) in indices {
        for si in &part_indices.parts {
            let slot = si.slot.as_deref().unwrap_or("unknown");
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                item_type, slot, si.part, si.index
            ));
        }
    }

    lines.join("\n")
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
    use crate::parser::parse_header;
    use crate::string_table::parse_string_table;

    let header = match parse_header(data) {
        Some(h) => h,
        None => {
            // Fallback to simple null-string extraction
            return extract_null_strings(data)
                .into_iter()
                .enumerate()
                .map(|(i, s)| RawStringEntry {
                    string_index: i,
                    value: s,
                })
                .collect();
        }
    };

    let strings = parse_string_table(data, &header);
    (0..strings.len())
        .filter_map(|i| strings.get(i).map(|s| RawStringEntry {
            string_index: i,
            value: s.to_string(),
        }))
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
    use crate::parser::parse_header;
    use crate::string_table::parse_string_table;

    let header = match parse_header(data) {
        Some(h) => h,
        None => return Vec::new(),
    };

    let strings = parse_string_table(data, &header);
    let mut pairs = Vec::new();

    for i in 1..strings.len() {
        if let Some(s) = strings.get(i) {
            if let Ok(num) = s.parse::<u32>() {
                // Found a numeric - get preceding string
                if let Some(prev) = strings.get(i - 1) {
                    // Skip if preceding is also numeric
                    if prev.parse::<u32>().is_err() {
                        pairs.push(StringNumericPair {
                            string_index: i - 1,
                            string_value: prev.to_string(),
                            numeric_value: num,
                            numeric_index: i,
                        });
                    }
                }
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
    #[ignore] // Integration test - requires real data file
    fn test_extract_serial_indices_from_inv4() {
        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let indices = extract_serial_indices(&data);

        // Count total parts across all item types
        let total_parts: usize = indices.values().map(|p| p.parts.len()).sum();
        let total_item_types = indices.len();

        println!("Found {} item types with {} total serial indices", total_item_types, total_parts);

        // Print breakdown by item type
        for (item_type, part_indices) in &indices {
            let root_count = part_indices.parts.iter().filter(|p| p.scope == "Root").count();
            let sub_count = part_indices.parts.iter().filter(|p| p.scope == "Sub").count();
            println!("  {}: {} Root, {} Sub", item_type, root_count, sub_count);
        }

        // If we found under 1000 parts, we are grossly lacking
        assert!(total_parts >= 1000, "Expected at least 1000 parts, got {}", total_parts);
    }

    #[test]
    #[ignore] // Integration test - requires real data file
    fn test_binary_parser_on_inv4() {
        use crate::binary_parser::BinaryParser;
        use crate::parser::{parse_header, find_binary_section_with_count};
        use crate::string_table::parse_string_table;

        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        // Parse NCS header using the document parser (same as CLI)
        let header = parse_header(&data).expect("Failed to parse header");
        let string_table = parse_string_table(&data, &header);

        println!("Type: {}", header.type_name);
        println!("Format code: {}", header.format_code);
        println!("String count: {}", string_table.len());

        // Find correct binary offset by counting exactly 18,393 strings
        let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
            .expect("Failed to find binary section");

        println!("Binary section offset: 0x{:x}", binary_offset);
        println!("File size: {} bytes", data.len());
        println!("Binary section size: {} bytes", data.len() - binary_offset);

        let parser = BinaryParser::new(
            &data,
            &string_table,
            &header.format_code,
        );

        let records = parser.parse_records(binary_offset);
        println!("Parsed {} records from binary section", records.len());

        // Debug: try reading first few values manually
        use crate::bit_reader::{bit_width, BitReader};
        let string_bits = bit_width(string_table.len() as u32);
        println!("String bits: {}", string_bits);

        let binary_data = &data[binary_offset..];
        let mut reader = BitReader::new(binary_data);

        // Try reading first few string indices
        println!("First 10 potential string indices:");
        for i in 0..10 {
            if let Some(idx) = reader.read_bits(string_bits) {
                let s = string_table.get(idx as usize).unwrap_or("(oob)");
                println!("  [{}] idx={:5} -> {:?}", i, idx, s);
            }
        }

        // Look for serialindex in records
        let mut serial_count = 0;
        for record in &records {
            if record.fields.contains_key("serialindex") {
                serial_count += 1;
            }
            for dep in &record.dep_entries {
                if dep.fields.contains_key("serialindex") {
                    serial_count += 1;
                }
            }
        }
        println!("Found {} records with serialindex", serial_count);

        // Don't fail on 0 records - we're still debugging
        println!("Total records: {}", records.len());
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

    #[test]
    #[ignore]
    fn test_find_real_binary_offset() {
        use crate::parser::parse_header;

        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let header = parse_header(&data).expect("Failed to parse header");

        println!("File size: 0x{:x} ({} bytes)", data.len(), data.len());
        println!("String table offset: 0x{:x}", header.string_table_offset);
        println!("String count from header: {}", header.string_count.unwrap_or(0));
        println!("Binary offset from header: 0x{:x}", header.binary_offset);

        // Manually count to the 18,393rd string
        let mut pos = header.string_table_offset;
        let mut string_count = 0;
        let target = header.string_count.unwrap_or(18393);

        println!("\nCounting {} strings...", target);

        while pos < data.len() && string_count < target {
            let start = pos;

            // Find null terminator
            while pos < data.len() && data[pos] != 0 {
                pos += 1;
            }

            // Count this string
            string_count += 1;

            // Show last few strings
            if string_count >= target - 3 {
                let s = std::str::from_utf8(&data[start..pos]).unwrap_or("<invalid>");
                println!("String {}: {:?}", string_count, s);
            }

            // Skip null terminator
            if pos < data.len() {
                pos += 1;
            }
        }

        println!("\nAfter {} strings, position is: 0x{:x}", string_count, pos);
        println!("Bytes remaining: {} ({} bytes)", data.len() - pos, data.len() - pos);

        // Show what's after the last string
        println!("\nFirst 200 bytes after string table:");
        for i in pos..(pos + 200).min(data.len()) {
            if (i - pos) % 16 == 0 {
                print!("\n{:08x}: ", i);
            }
            print!("{:02x} ", data[i]);
        }
        println!("\n");

        // Try to parse as null-terminated strings
        println!("Trying to parse remaining bytes as strings:");
        let mut str_pos = pos;
        let mut found_strings = 0;
        while str_pos < data.len() && found_strings < 20 {
            let start = str_pos;
            while str_pos < data.len() && data[str_pos] != 0 {
                str_pos += 1;
            }

            if str_pos > start {
                if let Ok(s) = std::str::from_utf8(&data[start..str_pos]) {
                    println!("  [{}] {:?}", found_strings, s);
                    found_strings += 1;
                }
            }

            if str_pos < data.len() {
                str_pos += 1;
            }
        }
    }

    #[test]
    #[ignore]
    fn test_parse_tags_sequentially() {
        use crate::parser::{parse_header, find_binary_section_with_count};
        use crate::string_table::parse_string_table;

        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let header = parse_header(&data).expect("Failed to parse header");
        let _strings = parse_string_table(&data, &header);
        let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
            .expect("Failed to find binary section");

        let binary_data = &data[binary_offset..];

        println!("Binary section: offset=0x{:x}, size={} bytes", binary_offset, binary_data.len());

        // Tags in format code "abcefhijl"
        let tag_bytes = [0x61u8, 0x62, 0x63, 0x65, 0x66, 0x68, 0x69, 0x6a, 0x6c]; // a,b,c,e,f,h,i,j,l

        println!("\nFirst 50 tag occurrences:");
        let mut tag_count = 0;
        for i in 0..binary_data.len().min(5000) {
            if tag_bytes.contains(&binary_data[i]) {
                let tag_char = binary_data[i] as char;

                // Show next 30 bytes
                print!("0x{:04x}: '{}' | ", i, tag_char);
                for j in 0..30.min(binary_data.len() - i - 1) {
                    print!("{:02x} ", binary_data[i + 1 + j]);
                }
                println!();

                tag_count += 1;
                if tag_count >= 50 {
                    break;
                }
            }
        }

        println!("\n\nAnalyzing structure around known serial index 237 at 0x13cd:");
        let pos_237: usize = 0x13cd;
        println!("Context (100 bytes before to 20 after):");
        for i in (pos_237.saturating_sub(100))..pos_237.saturating_add(20).min(binary_data.len()) {
            if i % 16 == 0 {
                print!("\n{:04x}: ", i);
            }
            let byte = binary_data[i];
            if tag_bytes.contains(&byte) {
                print!("[{}] ", byte as char);
            } else if i == pos_237 {
                print!("<{:02x}> ", byte);
            } else {
                print!("{:02x} ", byte);
            }
        }
        println!();
    }

    #[test]
    #[ignore]
    fn test_extract_via_j_tags() {
        use crate::parser::{parse_header, find_binary_section_with_count};
        use crate::string_table::parse_string_table;

        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let header = parse_header(&data).expect("Failed to parse header");
        let _strings = parse_string_table(&data, &header);
        let binary_offset = find_binary_section_with_count(&data, header.string_table_offset, Some(18393))
            .expect("Failed to find binary section");

        let binary_data = &data[binary_offset..];

        println!("Scanning for 'j' tags and potential serial indices...");

        let mut found_indices = Vec::new();

        // Scan for 'j' (0x6a) tags
        for i in 0..binary_data.len() {
            if binary_data[i] == 0x6a {
                // Check next 30 bytes for values in serial index range (1-541)
                for offset in 1..30 {
                    if i + offset >= binary_data.len() {
                        break;
                    }

                    let val_u8 = binary_data[i + offset] as u32;
                    if val_u8 >= 1 && val_u8 <= 541 {
                        found_indices.push((i, offset, val_u8, "u8"));
                    }

                    // Also check u16 LE
                    if i + offset + 1 < binary_data.len() {
                        let val_u16 = u16::from_le_bytes([
                            binary_data[i + offset],
                            binary_data[i + offset + 1]
                        ]) as u32;
                        if val_u16 >= 1 && val_u16 <= 541 {
                            found_indices.push((i, offset, val_u16, "u16le"));
                        }
                    }
                }
            }
        }

        println!("Found {} potential serial indices after 'j' tags", found_indices.len());

        // Group by offset distance
        let mut by_offset: std::collections::HashMap<usize, Vec<u32>> = std::collections::HashMap::new();
        for (_, offset, val, _) in &found_indices {
            by_offset.entry(*offset).or_insert_with(Vec::new).push(*val);
        }

        println!("\nSerial indices grouped by offset from 'j' tag:");
        let mut offsets: Vec<_> = by_offset.keys().collect();
        offsets.sort();
        for offset in offsets.iter().take(10) {
            let values = by_offset.get(offset).unwrap();
            println!("  Offset {}: {} values (e.g., {:?})", offset, values.len(), &values[..values.len().min(10)]);
        }

        // Count unique values found at offset 21 (known distance for serial index 237)
        if let Some(vals_at_21) = by_offset.get(&21) {
            let mut unique: Vec<u32> = vals_at_21.clone();
            unique.sort();
            unique.dedup();
            println!("\nUnique serial indices at offset 21 from 'j': {}", unique.len());
            println!("First 30: {:?}", &unique[..unique.len().min(30)]);
        }

        // Now scan ALL tag types and find the best offset for each
        println!("\n\nScanning ALL tag types (a,b,c,e,f,h,i,j,l)...");
        let tag_chars = ['a', 'b', 'c', 'e', 'f', 'h', 'i', 'j', 'l'];

        for tag_char in tag_chars {
            let tag_byte = tag_char as u8;
            let mut tag_indices = Vec::new();

            for i in 0..binary_data.len() {
                if binary_data[i] == tag_byte {
                    // Check offsets 1-30
                    for offset in 1..30 {
                        if i + offset >= binary_data.len() {
                            break;
                        }

                        let val_u8 = binary_data[i + offset] as u32;
                        if val_u8 >= 1 && val_u8 <= 541 {
                            tag_indices.push((offset, val_u8));
                        }

                        if i + offset + 1 < binary_data.len() {
                            let val_u16 = u16::from_le_bytes([
                                binary_data[i + offset],
                                binary_data[i + offset + 1]
                            ]) as u32;
                            if val_u16 >= 256 && val_u16 <= 541 {
                                tag_indices.push((offset, val_u16));
                            }
                        }
                    }
                }
            }

            // Group by offset
            let mut by_off: std::collections::HashMap<usize, Vec<u32>> = std::collections::HashMap::new();
            for (off, val) in &tag_indices {
                by_off.entry(*off).or_insert_with(Vec::new).push(*val);
            }

            // Find offset with most unique values
            let mut best_offset = 0;
            let mut best_count = 0;
            for (off, vals) in &by_off {
                let mut unique = vals.clone();
                unique.sort();
                unique.dedup();
                if unique.len() > best_count {
                    best_count = unique.len();
                    best_offset = *off;
                }
            }

            if best_count > 0 {
                let vals = by_off.get(&best_offset).unwrap();
                let mut unique = vals.clone();
                unique.sort();
                unique.dedup();
                println!("  Tag '{}': best at offset {} with {} unique values (total {} hits)",
                    tag_char, best_offset, unique.len(), vals.len());
            }
        }

        // Combine all tag types at their best offsets
        println!("\n\nCombining all tags at optimal offsets...");

        // Use (position, value) pairs to avoid counting the same byte position multiple times
        let mut position_value_pairs: std::collections::HashSet<(usize, u32)> = std::collections::HashSet::new();

        // Define best offsets for each tag
        let tag_offsets = [
            ('a', 5),
            ('b', 5),
            ('c', 20),
            ('e', 19),
            ('f', 27),
            ('h', 28),
            ('i', 3),
            ('j', 14),  // Using best offset (14) instead of 21
            ('l', 26),
        ];

        for (tag_char, offset) in tag_offsets {
            let tag_byte = tag_char as u8;
            for i in 0..binary_data.len() {
                if binary_data[i] == tag_byte && i + offset < binary_data.len() {
                    let pos = i + offset;

                    // Try u8 for values < 256
                    let val_u8 = binary_data[pos] as u32;
                    if val_u8 >= 1 && val_u8 < 256 {
                        position_value_pairs.insert((pos, val_u8));
                    }

                    // Try u16 LE for values >= 256
                    if pos + 1 < binary_data.len() {
                        let val_u16 = u16::from_le_bytes([
                            binary_data[pos],
                            binary_data[pos + 1]
                        ]) as u32;
                        if val_u16 >= 256 && val_u16 <= 541 {
                            position_value_pairs.insert((pos, val_u16));
                        }
                    }
                }
            }
        }

        let all_occurrences: Vec<u32> = position_value_pairs.iter().map(|(_, v)| *v).collect();
        let unique_count = {
            let mut unique = all_occurrences.clone();
            unique.sort();
            unique.dedup();
            unique.len()
        };

        println!("Total serial index occurrences (deduped by position): {}", all_occurrences.len());
        println!("Unique serial index values: {}", unique_count);
        println!("Target total occurrences: 5,513");

        // Find which single tag+offset combination is closest to 5,513
        println!("\n\nChecking individual tag+offset counts:");
        for (tag_char, offset) in tag_offsets {
            let tag_byte = tag_char as u8;
            let mut count = 0;
            let mut uniq = std::collections::HashSet::new();

            for i in 0..binary_data.len() {
                if binary_data[i] == tag_byte && i + offset < binary_data.len() {
                    let val_u8 = binary_data[i + offset] as u32;
                    if val_u8 >= 1 && val_u8 < 256 {
                        count += 1;
                        uniq.insert(val_u8);
                    }

                    if i + offset + 1 < binary_data.len() {
                        let val_u16 = u16::from_le_bytes([
                            binary_data[i + offset],
                            binary_data[i + offset + 1]
                        ]) as u32;
                        if val_u16 >= 256 && val_u16 <= 541 {
                            count += 1;
                            uniq.insert(val_u16);
                        }
                    }
                }
            }

            let diff = (count as i32 - 5513).abs();
            println!("  Tag '{}' at offset {}: {} occurrences ({} unique) - diff from target: {}",
                tag_char, offset, count, uniq.len(), diff);
        }
    }

    #[test]
    #[ignore]
    fn test_string_indices() {
        use crate::parser::parse_header;
        use crate::string_table::parse_string_table;

        let inv_path = "/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin";
        let data = std::fs::read(inv_path).expect("Failed to read inv4.bin");

        let header = parse_header(&data).expect("Failed to parse header");
        let strings = parse_string_table(&data, &header);

        println!("String table size: {}", strings.len());
        println!("First 20 strings:");
        for i in 0..20.min(strings.len()) {
            println!("  [{}] = {}", i, strings.get(i).unwrap_or("(none)"));
        }

        // Find key strings
        let key_strings = [
            "Active", "Root", "Sub", "Subs", "inv_type", "Armor_Shield", "serialindex",
        ];
        println!("\nKey string indices:");
        for key in key_strings {
            if let Some(idx) = strings.find(key) {
                println!("  {} = {}", key, idx);
            } else {
                println!("  {} = NOT FOUND", key);
            }
        }

        // Search for strings around indices 49-60
        println!("\nStrings around key indices (45-90):");
        for i in 45..90.min(strings.len()) {
            println!("  [{}] = {}", i, strings.get(i).unwrap_or("(none)"));
        }

        // Count numeric strings (potential serialindex values)
        let mut numeric_count = 0;
        let mut numeric_samples = Vec::new();
        for i in 0..strings.len() {
            if let Some(s) = strings.get(i) {
                if s.parse::<u32>().is_ok() {
                    numeric_count += 1;
                    if numeric_samples.len() < 20 {
                        numeric_samples.push((i, s.to_string()));
                    }
                }
            }
        }
        println!("\nNumeric strings: {} total", numeric_count);
        println!("First 20 numeric strings:");
        for (idx, val) in &numeric_samples {
            println!("  [{}] = {}", idx, val);
        }

        // Test extraction
        use crate::inventory::extract_serial_indices;
        let indices = extract_serial_indices(&data);
        let total_parts: usize = indices.values().map(|p| p.parts.len()).sum();
        let root_count: usize = indices
            .values()
            .flat_map(|p| p.parts.iter())
            .filter(|p| p.scope == "Root")
            .count();
        let sub_count: usize = indices
            .values()
            .flat_map(|p| p.parts.iter())
            .filter(|p| p.scope == "Sub")
            .count();
        println!("\n=== Extraction Results ===");
        println!("Total item types: {}", indices.len());
        println!("Total parts: {}", total_parts);
        println!("Root: {}, Sub: {}", root_count, sub_count);

        // Show first few entries
        println!("\nFirst 10 entries:");
        for (item_type, part_indices) in indices.iter().take(3) {
            println!("  {}: {} parts", item_type, part_indices.parts.len());
            for p in part_indices.parts.iter().take(5) {
                println!("    {} = {} ({})", p.part, p.index, p.scope);
            }
        }
    }
}
