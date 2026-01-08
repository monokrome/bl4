//! Tag-value binary parser for NCS files
//!
//! Some NCS formats (like inv) use tag-value encoding instead of fixed schemas.
//! The format code (e.g., "abcefhijl") defines a tag dictionary where each character
//! maps to a tag byte that can appear in the data stream.
//!
//! Structure:
//! ```text
//! [tag_byte] [value based on tag]
//! [tag_byte] [value based on tag]
//! ...
//! ```
//!
//! Tag mappings:
//! - 'a' (0x61): String index
//! - 'b' (0x62): u32
//! - 'c' (0x63): u32/f32
//! - 'e' (0x65): List (count + string indices)
//! - 'f' (0x66): List or special property
//! - 'h' (0x68): Complex structure
//! - 'i' (0x69): Complex structure
//! - 'j' (0x6a): Dependent entries
//! - 'l' (0x6c): Nested structure

use crate::bit_reader::{BitReader, bit_width};
use crate::types::StringTable;
use std::collections::HashMap;

/// Tag byte representing a field type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TagByte {
    StringIndex = 0x61, // 'a'
    U32 = 0x62,         // 'b'
    U32F32 = 0x63,      // 'c'
    ListE = 0x65,       // 'e'
    ListF = 0x66,       // 'f'
    ComplexH = 0x68,    // 'h'
    ComplexI = 0x69,    // 'i'
    DepEntries = 0x6a,  // 'j'
    Nested = 0x6c,      // 'l'
}

impl TagByte {
    /// Try to create TagByte from raw byte
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x61 => Some(TagByte::StringIndex),
            0x62 => Some(TagByte::U32),
            0x63 => Some(TagByte::U32F32),
            0x65 => Some(TagByte::ListE),
            0x66 => Some(TagByte::ListF),
            0x68 => Some(TagByte::ComplexH),
            0x69 => Some(TagByte::ComplexI),
            0x6a => Some(TagByte::DepEntries),
            0x6c => Some(TagByte::Nested),
            _ => None,
        }
    }

    /// Get the character representation
    pub fn to_char(self) -> char {
        self as u8 as char
    }
}

/// Value parsed from tag-value stream
#[derive(Debug, Clone)]
pub enum TagValue {
    /// String from string table
    String(String),
    /// Unsigned 32-bit integer
    Integer(u32),
    /// Integer and float interpretation of same bits
    IntFloat { u32_val: u32, f32_val: f32 },
    /// List of strings
    List(Vec<String>),
    /// Nested structure
    Nested(Box<TagValueRecord>),
    /// Null/empty value
    Null,
}

/// A record parsed from tag-value stream
#[derive(Debug, Clone)]
pub struct TagValueRecord {
    /// Record name (first field)
    pub name: String,
    /// Tag-value pairs
    pub properties: HashMap<String, TagValue>,
}

/// Tag-value parser for NCS binary data
pub struct TagValueParser<'a> {
    data: &'a [u8],
    strings: &'a StringTable,
    string_bits: u8,
}

impl<'a> TagValueParser<'a> {
    /// Create a new tag-value parser
    pub fn new(data: &'a [u8], strings: &'a StringTable) -> Self {
        let string_bits = bit_width(strings.len() as u32);
        Self {
            data,
            strings,
            string_bits,
        }
    }

    /// Parse all records starting from binary offset
    pub fn parse_records(&self, binary_offset: usize) -> Vec<TagValueRecord> {
        let binary_data = &self.data[binary_offset..];
        let mut reader = BitReader::new(binary_data);
        let mut records = Vec::new();

        // Parse records until we can't read anymore
        while reader.has_bits(self.string_bits as usize) {
            match self.parse_record(&mut reader) {
                Some(record) => records.push(record),
                None => break,
            }
        }

        records
    }

    /// Parse a single record from tag-value stream
    fn parse_record(&self, reader: &mut BitReader) -> Option<TagValueRecord> {
        eprintln!("[TAG-VALUE] parse_record: Starting at bit position {}", reader.position());

        // First read record name (always present)
        let name_index = reader.read_bits(self.string_bits)?;
        let name = self.strings.get(name_index as usize)?.to_string();

        eprintln!("[TAG-VALUE] Record name: {:?} (index {}) at bit position {}",
            name, name_index, reader.position());

        // Align to byte boundary before reading tags
        reader.align_byte();
        eprintln!("[TAG-VALUE] Aligned to byte boundary, now at bit position {}", reader.position());

        // Skip empty/terminator names
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            eprintln!("[TAG-VALUE] Skipping empty/none record");
            return None;
        }

        let mut properties = HashMap::new();

        // Now read tag-value pairs until we hit a terminator or run out of data
        let mut property_count = 0;
        loop {
            // Check if we have at least 8 bits for a tag byte
            if !reader.has_bits(8) {
                eprintln!("[TAG-VALUE] No more bits for tag byte");
                break;
            }

            let pos_before_tag = reader.position();
            let tag_byte = reader.read_bits(8)? as u8;

            eprintln!("[TAG-VALUE] Read tag byte: 0x{:02x} ('{}') at position {}",
                tag_byte, tag_byte as char, pos_before_tag);

            // Try to parse as tag
            match TagByte::from_byte(tag_byte) {
                Some(tag) => {
                    let property_name = format!("prop_{}_{}", tag.to_char(), property_count);

                    match self.parse_tag_value(tag, reader) {
                        Some(value) => {
                            eprintln!("[TAG-VALUE] Parsed {}: {:?}", property_name, value);
                            properties.insert(property_name, value);
                            property_count += 1;
                        }
                        None => {
                            eprintln!("[TAG-VALUE] Failed to parse value for tag {:?}", tag);
                            break;
                        }
                    }
                }
                None => {
                    // Not a recognized tag - might be end of record or padding
                    eprintln!("[TAG-VALUE] Unknown tag byte 0x{:02x}, ending record", tag_byte);
                    break;
                }
            }

            // Safety limit
            if property_count > 100 {
                eprintln!("[TAG-VALUE] Hit safety limit of 100 properties");
                break;
            }
        }

        eprintln!("[TAG-VALUE] Finished record with {} properties", properties.len());

        Some(TagValueRecord {
            name,
            properties,
        })
    }

    /// Parse value based on tag type
    fn parse_tag_value(&self, tag: TagByte, reader: &mut BitReader) -> Option<TagValue> {
        match tag {
            TagByte::StringIndex => {
                let idx = reader.read_bits(self.string_bits)?;
                let s = self.strings.get(idx as usize)?.to_string();
                Some(TagValue::String(s))
            }
            TagByte::U32 => {
                let v = reader.read_bits(32)?;
                Some(TagValue::Integer(v))
            }
            TagByte::U32F32 => {
                let v = reader.read_bits(32)?;
                let f = f32::from_bits(v);
                Some(TagValue::IntFloat { u32_val: v, f32_val: f })
            }
            TagByte::ListE | TagByte::ListF => {
                self.parse_tag_list(reader)
            }
            TagByte::ComplexH | TagByte::ComplexI | TagByte::Nested => {
                // Try to parse nested record
                if let Some(nested) = self.parse_record(reader) {
                    Some(TagValue::Nested(Box::new(nested)))
                } else {
                    Some(TagValue::Null)
                }
            }
            TagByte::DepEntries => {
                // For now, treat as list
                self.parse_tag_list(reader)
            }
        }
    }

    /// Parse a list value (may have count prefix)
    fn parse_tag_list(&self, reader: &mut BitReader) -> Option<TagValue> {
        eprintln!("[TAG-VALUE] parse_tag_list: starting at bit position {}", reader.position());

        // Try reading a count first (varint or small integer)
        // For now, try reading until we hit "none" or empty string
        let mut list = Vec::new();

        for _ in 0..1000 {  // Safety limit
            if !reader.has_bits(self.string_bits as usize) {
                break;
            }

            let idx = reader.read_bits(self.string_bits)?;
            let s = self.strings.get(idx as usize)?;

            eprintln!("[TAG-VALUE] List item: index {} = {:?}", idx, s);

            // Check for terminator
            if s.is_empty() || s.eq_ignore_ascii_case("none") {
                eprintln!("[TAG-VALUE] Found list terminator");
                break;
            }

            list.push(s.to_string());
        }

        eprintln!("[TAG-VALUE] Parsed list with {} items", list.len());
        Some(TagValue::List(list))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_byte_from_byte() {
        assert_eq!(TagByte::from_byte(0x61), Some(TagByte::StringIndex));
        assert_eq!(TagByte::from_byte(0x62), Some(TagByte::U32));
        assert_eq!(TagByte::from_byte(0x66), Some(TagByte::ListF));
        assert_eq!(TagByte::from_byte(0xFF), None);
    }

    #[test]
    fn test_tag_byte_to_char() {
        assert_eq!(TagByte::StringIndex.to_char(), 'a');
        assert_eq!(TagByte::U32.to_char(), 'b');
        assert_eq!(TagByte::ListF.to_char(), 'f');
    }
}
