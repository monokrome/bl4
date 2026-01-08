//! Native binary section parser for NCS files
//!
//! # Format Code Field Types
//!
//! Each character in the format code (e.g., "abcefhijl") defines a field type:
//! - 'a' (0x61): String pair - reads string index (16 bits for large tables)
//! - 'b' (0x62): u32 - reads 32 bits as unsigned integer
//! - 'c' (0x63): u32+f32 - reads 32 bits interpreted as both u32 and f32
//! - 'd' (0x64): List - reads string indices until "none" terminator
//! - 'e' (0x65): List - same as 'd'
//! - 'f' (0x66): List - same as 'd'
//! - 'h' (0x68): Complex nested structure
//! - 'i' (0x69): Complex nested structure
//! - 'j' (0x6a): dep_entries - nested records
//! - 'l' (0x6c): Nested structure
//! - 'p' (0x70): Variant with 2-bit subtype prefix

use crate::bit_reader::{bit_width, BitReader};
use crate::types::StringTable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Field type derived from format code character
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// String pair (reads string index)
    Pair,
    /// Unsigned 32-bit integer
    U32,
    /// 32 bits as both u32 and f32
    U32F32,
    /// List of strings (until "none" terminator)
    List,
    /// Complex nested structure
    Complex,
    /// Dependent entries (nested records)
    DepEntries,
    /// Nested structure
    Nested,
    /// Variant with subtype
    Variant,
    /// Unknown field type
    Unknown(u8),
}

impl FieldType {
    /// Convert format code character to field type
    pub fn from_char(c: char) -> Self {
        match c {
            'a' => FieldType::Pair,
            'b' => FieldType::U32,
            'c' => FieldType::U32F32,
            'd' | 'e' | 'f' => FieldType::List,
            'h' | 'i' => FieldType::Complex,
            'j' => FieldType::DepEntries,
            'l' => FieldType::Nested,
            'p' => FieldType::Variant,
            _ => FieldType::Unknown(c as u8),
        }
    }
}

/// Parsed field value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    /// String value (from pair field)
    String(String),
    /// Integer value
    Integer(u32),
    /// Float value
    Float(f32),
    /// Integer and float (from u32f32 field)
    IntFloat { u32_val: u32, f32_val: f32 },
    /// List of strings
    List(Vec<String>),
    /// Nested record
    Nested(Box<ParsedRecord>),
    /// Array of nested records
    Records(Vec<ParsedRecord>),
    /// Null/empty value
    Null,
}

/// A parsed record from the binary section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRecord {
    /// Record name (first string field)
    pub name: String,
    /// Field values keyed by field name
    pub fields: HashMap<String, FieldValue>,
    /// Dependent entries (for 'j' field type)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dep_entries: Vec<ParsedRecord>,
}

/// Schema for a record based on format code
#[derive(Debug, Clone)]
pub struct RecordSchema {
    /// Field types in order
    pub field_types: Vec<FieldType>,
    /// Field names (if known from type)
    pub field_names: Vec<String>,
}

impl RecordSchema {
    /// Create schema from format code string
    pub fn from_format_code(format_code: &str) -> Self {
        let field_types: Vec<FieldType> = format_code.chars().map(FieldType::from_char).collect();

        // Default field names based on position
        let field_names: Vec<String> = (0..field_types.len())
            .map(|i| format!("field_{}", i))
            .collect();

        Self {
            field_types,
            field_names,
        }
    }

    /// Create schema with custom field names
    pub fn with_names(format_code: &str, names: &[&str]) -> Self {
        let field_types: Vec<FieldType> = format_code.chars().map(FieldType::from_char).collect();

        let field_names: Vec<String> = names
            .iter()
            .enumerate()
            .map(|(i, &name)| {
                if name.is_empty() {
                    format!("field_{}", i)
                } else {
                    name.to_string()
                }
            })
            .chain((names.len()..field_types.len()).map(|i| format!("field_{}", i)))
            .collect();

        Self {
            field_types,
            field_names,
        }
    }
}

/// Parser for NCS binary section
pub struct BinaryParser<'a> {
    /// Raw data
    data: &'a [u8],
    /// String table for resolving indices
    strings: &'a StringTable,
    /// Bit width for string indices
    string_bits: u8,
    /// Format code schema
    schema: RecordSchema,
}

impl<'a> BinaryParser<'a> {
    /// Create a new binary parser
    pub fn new(data: &'a [u8], strings: &'a StringTable, format_code: &str) -> Self {
        let string_bits = bit_width(strings.len() as u32);
        let schema = RecordSchema::from_format_code(format_code);

        Self {
            data,
            strings,
            string_bits,
            schema,
        }
    }

    /// Parse all records from binary section
    pub fn parse_records(&self, binary_offset: usize) -> Vec<ParsedRecord> {
        let mut records = Vec::new();

        if binary_offset >= self.data.len() {
            return records;
        }

        let binary_data = &self.data[binary_offset..];
        let mut reader = BitReader::new(binary_data);

        // Parse records until we can't read anymore
        while reader.has_bits(self.string_bits as usize) {
            match self.parse_record(&mut reader) {
                Some(record) => records.push(record),
                None => break,
            }
        }

        records
    }

    /// Parse a single record
    fn parse_record(&self, reader: &mut BitReader) -> Option<ParsedRecord> {
        eprintln!("[DEBUG] parse_record: Starting at bit position {}", reader.position());

        // First field should be the record name (string pair)
        let name_index = reader.read_bits(self.string_bits)?;

        eprintln!("[DEBUG] Read name_index: {} (after {} bits read, now at position {})",
            name_index, self.string_bits, reader.position());

        let name = self.strings.get(name_index as usize)?.to_string();

        eprintln!("[DEBUG] Record name: {:?}", name);

        // Skip empty/terminator names
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            eprintln!("[DEBUG] Skipping empty/none record");
            return None;
        }

        let mut fields = HashMap::new();
        let mut dep_entries = Vec::new();

        // Parse remaining fields according to schema
        for (i, &field_type) in self.schema.field_types.iter().enumerate().skip(1) {
            let field_name = self
                .schema
                .field_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("field_{}", i));

            let pos_before = reader.position();
            eprintln!("[DEBUG] Parsing field {} ({}): {:?} at bit position {}",
                i, field_name, field_type, pos_before);

            let value = match field_type {
                FieldType::Pair => {
                    let idx = match reader.read_bits(self.string_bits) {
                        Some(v) => v,
                        None => {
                            eprintln!("[DEBUG] Failed to read {} bits for Pair field", self.string_bits);
                            return None;
                        }
                    };
                    let s = self
                        .strings
                        .get(idx as usize)
                        .unwrap_or("unknown")
                        .to_string();
                    FieldValue::String(s)
                }
                FieldType::U32 => {
                    let v = reader.read_bits(32)?;
                    FieldValue::Integer(v)
                }
                FieldType::U32F32 => {
                    let v = reader.read_bits(32)?;
                    let f = f32::from_bits(v);
                    FieldValue::IntFloat {
                        u32_val: v,
                        f32_val: f,
                    }
                }
                FieldType::List => {
                    let list = self.parse_list(reader)?;
                    FieldValue::List(list)
                }
                FieldType::Complex | FieldType::Nested => {
                    // For complex/nested, try to parse a nested record
                    if let Some(nested) = self.parse_record(reader) {
                        FieldValue::Nested(Box::new(nested))
                    } else {
                        FieldValue::Null
                    }
                }
                FieldType::DepEntries => {
                    // Parse dependent entries - a list of nested records
                    let entries = self.parse_dep_entries(reader)?;
                    dep_entries.extend(entries);
                    continue; // Don't add to fields
                }
                FieldType::Variant => {
                    // Read 2-bit subtype, then value based on subtype
                    let subtype = reader.read_bits(2)?;
                    match subtype {
                        0 => FieldValue::Null,
                        1 => {
                            let idx = reader.read_bits(self.string_bits)?;
                            let s = self
                                .strings
                                .get(idx as usize)
                                .unwrap_or("unknown")
                                .to_string();
                            FieldValue::String(s)
                        }
                        2 => {
                            if let Some(nested) = self.parse_record(reader) {
                                FieldValue::Nested(Box::new(nested))
                            } else {
                                FieldValue::Null
                            }
                        }
                        3 => {
                            // Reference - read two string indices
                            let idx1 = reader.read_bits(self.string_bits)?;
                            let idx2 = reader.read_bits(self.string_bits)?;
                            let s1 = self
                                .strings
                                .get(idx1 as usize)
                                .unwrap_or("unknown")
                                .to_string();
                            let s2 = self
                                .strings
                                .get(idx2 as usize)
                                .unwrap_or("unknown")
                                .to_string();
                            FieldValue::String(format!("{}:{}", s1, s2))
                        }
                        _ => FieldValue::Null,
                    }
                }
                FieldType::Unknown(_) => FieldValue::Null,
            };

            let pos_after = reader.position();
            let bits_consumed = pos_after - pos_before;
            eprintln!("[DEBUG] Field {} consumed {} bits, now at position {}",
                i, bits_consumed, pos_after);

            fields.insert(field_name, value);
        }

        Some(ParsedRecord {
            name,
            fields,
            dep_entries,
        })
    }

    /// Parse a list of strings until "none" terminator
    fn parse_list(&self, reader: &mut BitReader) -> Option<Vec<String>> {
        let mut list = Vec::new();

        eprintln!("[DEBUG] parse_list: starting at bit position {}", reader.position());
        eprintln!("[DEBUG] parse_list: string_bits={}, max_valid_index={}",
            self.string_bits, self.strings.len() - 1);

        // Try reading a count first (some formats use count instead of terminator)
        // Try 8-bit count first
        let potential_count = reader.read_bits(8)?;
        eprintln!("[DEBUG] parse_list: potential count (8-bit) = {}", potential_count);

        if potential_count == 0 {
            eprintln!("[DEBUG] parse_list: count is 0, returning empty list");
            return Some(list);
        }

        // If count seems reasonable (< 1000), use it
        if potential_count < 1000 {
            eprintln!("[DEBUG] parse_list: using count-based parsing (count={})", potential_count);
            for i in 0..potential_count {
                let idx = reader.read_bits(self.string_bits)?;
                if idx as usize >= self.strings.len() {
                    eprintln!("[DEBUG] parse_list: item {} has invalid index {}", i, idx);
                    return None;
                }
                let s = self.strings.get(idx as usize)?.to_string();
                eprintln!("[DEBUG] parse_list: item {}: {:?}", i, s);
                list.push(s);
            }
            return Some(list);
        }

        // Otherwise, fall back to terminator-based parsing
        eprintln!("[DEBUG] parse_list: count {} too large, trying terminator-based parsing", potential_count);

        // Put the "count" back as a string index
        if potential_count as usize >= self.strings.len() {
            eprintln!("[DEBUG] parse_list: first value out of range, aborting");
            return None;
        }

        let first_str = self.strings.get(potential_count as usize)?.to_string();
        if first_str.is_empty() || first_str.eq_ignore_ascii_case("none") {
            return Some(list);
        }
        list.push(first_str);

        loop {
            if !reader.has_bits(self.string_bits as usize) {
                eprintln!("[DEBUG] parse_list: no more bits available");
                break;
            }

            let pos_before = reader.position();
            let idx = match reader.read_bits(self.string_bits) {
                Some(v) => v,
                None => {
                    eprintln!("[DEBUG] parse_list: failed to read string index at position {}", pos_before);
                    return None;
                }
            };

            eprintln!("[DEBUG] parse_list: read index {} at bit position {} -> {}",
                idx, pos_before, reader.position());

            // Check if index is in valid range
            if idx as usize >= self.strings.len() {
                eprintln!("[DEBUG] parse_list: index {} out of range (max {})",
                    idx, self.strings.len() - 1);
                eprintln!("[DEBUG] parse_list: This suggests wrong bit alignment or wrong offset");
                eprintln!("[DEBUG] parse_list: Bit pattern that produced {}: 0x{:04x} / 0b{:015b}",
                    idx, idx, idx);
                // Continue to read a few more to see the pattern
                let mut debug_indices = vec![idx];
                for _ in 0..5 {
                    if let Some(next_idx) = reader.read_bits(self.string_bits) {
                        debug_indices.push(next_idx);
                    } else {
                        break;
                    }
                }
                eprintln!("[DEBUG] parse_list: Next few indices: {:?}", debug_indices);
                return None;
            }

            let s = self.strings.get(idx as usize).unwrap();

            // "none" is the list terminator
            if s.is_empty() || s.eq_ignore_ascii_case("none") {
                eprintln!("[DEBUG] parse_list: found terminator '{}' at index {}", s, idx);
                break;
            }

            eprintln!("[DEBUG] parse_list: added string {:?}", s);
            list.push(s.to_string());

            // Safety limit
            if list.len() > 1000 {
                eprintln!("[DEBUG] parse_list: hit safety limit of 1000 items");
                break;
            }
        }

        eprintln!("[DEBUG] parse_list: finished with {} items", list.len());
        Some(list)
    }

    /// Parse dependent entries (nested records)
    fn parse_dep_entries(&self, reader: &mut BitReader) -> Option<Vec<ParsedRecord>> {
        let mut entries = Vec::new();

        // Read entry count or parse until terminator
        // Based on Ghidra, this reads 2 bits for entry type
        loop {
            if !reader.has_bits(2) {
                break;
            }

            let entry_type = reader.read_bits(2)?;

            if entry_type == 0 {
                // End of dep_entries
                break;
            }

            // Read entry name
            if !reader.has_bits(self.string_bits as usize) {
                break;
            }

            let name_idx = reader.read_bits(self.string_bits)?;
            let name = self
                .strings
                .get(name_idx as usize)
                .unwrap_or("unknown")
                .to_string();

            if name.is_empty() || name.eq_ignore_ascii_case("none") {
                break;
            }

            let entry = match entry_type {
                1 => {
                    // Simple entry - just name
                    ParsedRecord {
                        name,
                        fields: HashMap::new(),
                        dep_entries: Vec::new(),
                    }
                }
                2 => {
                    // Nested entry with fields
                    let mut fields = HashMap::new();
                    // Parse nested record's fields
                    if let Some(nested) = self.parse_record(reader) {
                        fields.insert("value".to_string(), FieldValue::Nested(Box::new(nested)));
                    }
                    ParsedRecord {
                        name,
                        fields,
                        dep_entries: Vec::new(),
                    }
                }
                3 => {
                    // Reference entry
                    let ref_idx = reader.read_bits(self.string_bits)?;
                    let ref_name = self
                        .strings
                        .get(ref_idx as usize)
                        .unwrap_or("unknown")
                        .to_string();
                    let mut fields = HashMap::new();
                    fields.insert("ref".to_string(), FieldValue::String(ref_name));
                    ParsedRecord {
                        name,
                        fields,
                        dep_entries: Vec::new(),
                    }
                }
                _ => continue,
            };

            entries.push(entry);

            // Safety limit
            if entries.len() > 10000 {
                break;
            }
        }

        Some(entries)
    }
}

/// Extract serialindex values from parsed records
///
/// Searches for records with "serialindex" fields and extracts:
/// - Part name (record name or parent dep_entry name)
/// - Index value (from serialindex.index field)
/// - Scope ("Root" or "Sub" from serialindex._scope)
/// - Category (from serialindex._category, usually "inv_type")
pub fn extract_serial_indices(records: &[ParsedRecord]) -> Vec<SerialIndexEntry> {
    let mut entries = Vec::new();

    for record in records {
        extract_from_record(record, None, &mut entries);
    }

    entries
}

fn extract_from_record(
    record: &ParsedRecord,
    parent_name: Option<&str>,
    entries: &mut Vec<SerialIndexEntry>,
) {
    // Check if this record has a serialindex field
    if let Some(FieldValue::Nested(serialindex)) = record.fields.get("serialindex") {
        // Extract index value
        let index = match serialindex.fields.get("index") {
            Some(FieldValue::Integer(v)) => *v,
            _ => 0,
        };

        // Extract scope
        let scope = match serialindex.fields.get("_scope") {
            Some(FieldValue::String(s)) => s.clone(),
            _ => "Unknown".to_string(),
        };

        // Extract category
        let category = match serialindex.fields.get("_category") {
            Some(FieldValue::String(s)) => s.clone(),
            _ => "Unknown".to_string(),
        };

        // Use parent name if this is a dep_entry, otherwise use record name
        let part_name = parent_name.unwrap_or(&record.name).to_string();

        entries.push(SerialIndexEntry {
            part_name,
            index,
            scope,
            category,
        });
    }

    // Recursively check dep_entries
    for dep in &record.dep_entries {
        extract_from_record(dep, Some(&record.name), entries);
    }

    // Check nested fields
    for value in record.fields.values() {
        if let FieldValue::Nested(nested) = value {
            extract_from_record(nested, Some(&record.name), entries);
        }
        if let FieldValue::Records(recs) = value {
            for rec in recs {
                extract_from_record(rec, Some(&record.name), entries);
            }
        }
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
    /// Category: usually "inv_type"
    pub category: String,
}

/// Export serial indices to TSV format
pub fn serial_indices_to_tsv(entries: &[SerialIndexEntry], item_type: &str) -> String {
    let mut lines = vec!["item_type\tpart\tserial_index\tscope\tcategory".to_string()];

    for entry in entries {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            item_type, entry.part_name, entry.index, entry.scope, entry.category
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_from_char() {
        assert_eq!(FieldType::from_char('a'), FieldType::Pair);
        assert_eq!(FieldType::from_char('b'), FieldType::U32);
        assert_eq!(FieldType::from_char('c'), FieldType::U32F32);
        assert_eq!(FieldType::from_char('d'), FieldType::List);
        assert_eq!(FieldType::from_char('j'), FieldType::DepEntries);
    }

    #[test]
    fn test_record_schema() {
        let schema = RecordSchema::from_format_code("abcefhijl");
        assert_eq!(schema.field_types.len(), 9);
        assert_eq!(schema.field_types[0], FieldType::Pair);
        assert_eq!(schema.field_types[1], FieldType::U32);
        assert_eq!(schema.field_types[8], FieldType::Nested);
    }
}
