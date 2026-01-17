//! Correct NCS binary parser based on reverse engineering findings
//!
//! ## Binary Section Structure
//!
//! The binary section has three parts:
//!
//! 1. **Bit-packed first entry** (~161 bytes for inv0):
//!    - 12-bit string indices (LSB-first)
//!    - Can reference any of the 3,341 strings
//!
//! 2. **Byte-packed entries** (~9,780 bytes for inv0):
//!    - Format: `00 <name_idx> <data_idx> <data_idx> ...`
//!    - 1,197 entries total
//!    - 8-bit string indices (first 256 strings only)
//!    - Entry names: "ammo_assaultrifle", "firmware_aspect_heating_up", etc.
//!
//! 3. **Tail section** (~74,660 bytes for inv0):
//!    - 51 sections of byte-packed entries
//!    - Sections separated by `00 00` terminators
//!    - 7,873 entries total
//!    - Format: same as part 2
//!    - Entry names: "pickup_attr_effects", "firmware_godkiller_weight", etc.

use crate::bit_reader::BitReader;
use crate::types::StringTable;
use serde::{Deserialize, Serialize};

/// A parsed entry from the binary section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryEntryV2 {
    /// Entry name (first index, resolved to string)
    pub name: String,
    /// String indices that make up this entry's data
    pub indices: Vec<u32>,
    /// Resolved strings (for convenience)
    pub strings: Vec<String>,
}

/// Binary section parser using correct algorithm
pub struct BinaryParserV2<'a> {
    data: &'a [u8],
    strings: &'a StringTable,
}

impl<'a> BinaryParserV2<'a> {
    pub fn new(data: &'a [u8], strings: &'a StringTable) -> Self {
        Self { data, strings }
    }

    /// Parse all three sections of the binary data
    pub fn parse(&self, binary_offset: usize) -> BinaryParsed {
        if binary_offset >= self.data.len() {
            return BinaryParsed::default();
        }

        let binary_data = &self.data[binary_offset..];

        // Skip parsing if binary section is too small
        if binary_data.len() < 50 {
            return BinaryParsed::default();
        }

        // Find where byte-packed section starts (after bit-packed first entry)
        // Look for 0x28 separator around offset 161
        let byte_packed_start = self.find_byte_packed_start(binary_data);

        let mut result = BinaryParsed::default();

        // Parse bit-packed first entry
        if byte_packed_start > 0 && byte_packed_start <= binary_data.len() {
            result.first_entry = self.parse_bit_packed_first_entry(
                &binary_data[..byte_packed_start],
            );
        }

        // Parse byte-packed entries and tail
        if byte_packed_start < binary_data.len() {
            let (entries, tail_offset) = self.parse_byte_packed_entries(
                &binary_data[byte_packed_start..],
            );
            result.byte_packed_entries = entries;

            // Parse tail section
            if tail_offset > 0 && byte_packed_start + tail_offset < binary_data.len() {
                result.tail_sections = self.parse_tail_sections(
                    &binary_data[byte_packed_start + tail_offset..],
                );
            }
        }

        result
    }

    /// Find where byte-packed section starts (after bit-packed first entry)
    fn find_byte_packed_start(&self, data: &[u8]) -> usize {
        // Look for 0x28 separator in first 512 bytes
        for i in 16..512.min(data.len()) {
            if data[i] == 0x28 {
                return i;
            }
        }
        161 // Fallback to known size for inv0
    }

    /// Parse bit-packed first entry using 12-bit indices
    fn parse_bit_packed_first_entry(&self, data: &[u8]) -> Option<BinaryEntryV2> {
        let mut reader = BitReader::new(data);
        let mut indices = Vec::new();

        // Read 12-bit indices until we run out of data or hit separator
        while reader.has_bits(12) {
            if let Some(idx) = reader.read_bits(12) {
                if idx < self.strings.len() as u32 {
                    indices.push(idx);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if indices.is_empty() {
            return None;
        }

        // First index is the name
        let name = self.get_string(indices[0]);
        let strings: Vec<String> = indices.iter().map(|&i| self.get_string(i)).collect();

        Some(BinaryEntryV2 {
            name,
            indices,
            strings,
        })
    }

    /// Parse byte-packed entries section
    /// Format: `00 <name_idx> <data_indices> ...` where each index is 1 byte
    /// Returns (entries, offset to tail section)
    fn parse_byte_packed_entries(&self, data: &[u8]) -> (Vec<BinaryEntryV2>, usize) {
        let mut entries = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            // Look for entry start marker (0x00)
            if data[pos] != 0x00 {
                pos += 1;
                continue;
            }

            // Check for section terminator (0x00 0x00)
            if pos + 1 < data.len() && data[pos + 1] == 0x00 {
                // Found tail section start
                return (entries, pos);
            }

            pos += 1; // Skip the 0x00 marker

            if pos >= data.len() {
                break;
            }

            // Read entry name index
            let name_idx = data[pos] as u32;
            pos += 1;

            // Read data indices until next 0x00 or end
            let mut indices = vec![name_idx];
            while pos < data.len() && data[pos] != 0x00 {
                indices.push(data[pos] as u32);
                pos += 1;
            }

            // Validate all indices
            if indices.iter().all(|&i| (i as usize) < self.strings.len()) {
                let name = self.get_string(name_idx);
                let strings: Vec<String> = indices.iter().map(|&i| self.get_string(i)).collect();

                entries.push(BinaryEntryV2 {
                    name,
                    indices,
                    strings,
                });
            }
        }

        (entries, 0)
    }

    /// Parse tail sections (51 sections of byte-packed entries)
    fn parse_tail_sections(&self, data: &[u8]) -> Vec<Vec<BinaryEntryV2>> {
        let mut sections = Vec::new();
        let mut current_section = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            // Look for entry start marker (0x00)
            if data[pos] != 0x00 {
                pos += 1;
                continue;
            }

            // Check for section terminator (0x00 0x00)
            if pos + 1 < data.len() && data[pos + 1] == 0x00 {
                // End current section
                if !current_section.is_empty() {
                    sections.push(current_section);
                    current_section = Vec::new();
                }
                pos += 2; // Skip both 0x00 bytes
                continue;
            }

            pos += 1; // Skip the 0x00 marker

            if pos >= data.len() {
                break;
            }

            // Read entry name index
            let name_idx = data[pos] as u32;
            pos += 1;

            // Read data indices until next 0x00 or end
            let mut indices = vec![name_idx];
            while pos < data.len() && data[pos] != 0x00 {
                indices.push(data[pos] as u32);
                pos += 1;
            }

            // Validate all indices
            if indices.iter().all(|&i| (i as usize) < self.strings.len()) {
                let name = self.get_string(name_idx);
                let strings: Vec<String> = indices.iter().map(|&i| self.get_string(i)).collect();

                current_section.push(BinaryEntryV2 {
                    name,
                    indices,
                    strings,
                });
            }
        }

        // Add last section if not empty
        if !current_section.is_empty() {
            sections.push(current_section);
        }

        sections
    }

    fn get_string(&self, idx: u32) -> String {
        self.strings
            .get(idx as usize)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("INVALID[{}]", idx))
    }
}

/// Parsed binary section data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BinaryParsed {
    /// Bit-packed first entry (12-bit indices)
    pub first_entry: Option<BinaryEntryV2>,
    /// Byte-packed entries section (8-bit indices)
    pub byte_packed_entries: Vec<BinaryEntryV2>,
    /// Tail sections (51 sections of byte-packed entries)
    pub tail_sections: Vec<Vec<BinaryEntryV2>>,
}

impl BinaryParsed {
    /// Get all entries across all sections
    pub fn all_entries(&self) -> Vec<&BinaryEntryV2> {
        let mut all = Vec::new();

        if let Some(ref first) = self.first_entry {
            all.push(first);
        }

        all.extend(self.byte_packed_entries.iter());

        for section in &self.tail_sections {
            all.extend(section.iter());
        }

        all
    }

    /// Count total entries
    pub fn total_entries(&self) -> usize {
        let first_count = if self.first_entry.is_some() { 1 } else { 0 };
        let tail_count: usize = self.tail_sections.iter().map(|s| s.len()).sum();
        first_count + self.byte_packed_entries.len() + tail_count
    }
}

/// Extract serial indices from parsed binary data
///
/// Serial indices are embedded as numeric strings in entries with part names.
/// Pattern: part entries contain numeric strings (1-541 range) that are the serial indices.
pub fn extract_serial_indices(parsed: &BinaryParsed) -> Vec<SerialIndexEntry> {
    let mut entries = Vec::new();

    for entry in parsed.all_entries() {
        // Check if this is a part entry
        if is_part_name(&entry.name) {
            // Derive category from part name prefix
            let category = category_from_prefix(&entry.name);

            // Look for numeric strings (excluding decimals) that could be serial indices
            for s in &entry.strings {
                // Skip the part name itself
                if s == &entry.name {
                    continue;
                }

                // Check if this is a small integer (serial indices are 1-1000)
                if let Ok(index) = s.parse::<u32>() {
                    // Filter out decimal numbers (they contain periods)
                    if !s.contains('.') && !s.contains('-') && index >= 1 && index <= 1000 {
                        entries.push(SerialIndexEntry {
                            part_name: entry.name.clone(),
                            index,
                            scope: "Unknown".to_string(),
                            category,
                        });
                    }
                }
            }
        }
    }

    entries
}

/// Check if a string is a part name
fn is_part_name(s: &str) -> bool {
    // Part prefixes
    if s.starts_with("part_")
        || s.starts_with("comp_")
        || s.starts_with("uni_")
        || s.starts_with("SHD_Aug_")
    {
        return true;
    }

    // Weapon parts: MANU_TYPE_PartName
    const MANUFACTURERS: &[&str] = &["BOR", "DAD", "JAK", "MAL", "ORD", "TED", "TOR", "VLA"];
    const WEAPON_TYPES: &[&str] = &["AR", "HW", "PS", "SG", "SM", "SR"];

    let parts: Vec<&str> = s.splitn(3, '_').collect();
    if parts.len() >= 3 {
        if MANUFACTURERS.contains(&parts[0]) && WEAPON_TYPES.contains(&parts[1]) {
            return true;
        }
    }

    false
}

/// Derive category from part name prefix
///
/// Part names like "BOR_SG_Barrel_01_A" have prefixes like "BOR_SG" that map to categories.
/// This function extracts the prefix and returns the category ID.
///
/// For parts without manufacturer prefixes (like "comp_*", "part_*"), returns None.
pub fn category_from_prefix(part_name: &str) -> Option<i64> {
    // Extract MANU_TYPE prefix (e.g., "BOR_SG" from "BOR_SG_Barrel_01_A")
    let parts: Vec<&str> = part_name.splitn(3, '_').collect();
    if parts.len() < 2 {
        return None;
    }

    let prefix = format!("{}_{}", parts[0], parts[1]);

    // Map prefix to category using known mappings
    // These are derived from src/bl4/src/parts.rs SERIAL_TO_PARTS_CAT
    match prefix.as_str() {
        // Shotguns
        "DAD_SG" => Some(8),
        "TOR_SG" => Some(11),
        "MAL_SG" => Some(19),
        "JAK_SG" => Some(9),
        "TED_SG" => Some(10),
        "BOR_SG" => Some(12),

        // Pistols
        "JAK_PS" => Some(3),
        "DAD_PS" => Some(2),
        "TOR_PS" => Some(5),
        "TED_PS" => Some(4),

        // Assault Rifles
        "TED_AR" => Some(15),
        "DAD_AR" => Some(13),
        "ORD_AR" => Some(18),
        "VLA_AR" => Some(17),
        "TOR_AR" => Some(16),
        "JAK_AR" => Some(14),

        // Snipers
        "VLA_SR" => Some(25),
        "JAK_SR" => Some(26),
        "ORD_SR" => Some(28),
        "MAL_SR" => Some(29),
        "BOR_SR" => Some(25), // Shares category with VLA_SR

        // SMGs
        "DAD_SM" => Some(20),
        "BOR_SM" => Some(21),
        "MAL_SM" => Some(23),
        "VLA_SM" => Some(22),

        _ => None,
    }
}

/// Serial index entry extracted from binary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialIndexEntry {
    /// Part name
    pub part_name: String,
    /// Serial index number
    pub index: u32,
    /// Scope: "Root" or "Sub"
    pub scope: String,
    /// Category ID (derived from part name prefix, None if cannot be determined)
    pub category: Option<i64>,
}

/// Export serial indices to TSV format
pub fn serial_indices_to_tsv(entries: &[SerialIndexEntry], item_type: &str) -> String {
    let mut lines = vec!["item_type\tpart\tserial_index\tscope\tcategory".to_string()];

    for entry in entries {
        let category_str = entry.category.map_or("unknown".to_string(), |c| c.to_string());
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            item_type, entry.part_name, entry.index, entry.scope, category_str
        ));
    }

    lines.join("\n")
}
