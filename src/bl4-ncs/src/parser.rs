//! NCS content parser for structured JSON output
//!
//! Parses decompressed NCS content into structured data that can be
//! serialized to JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// Parsed NCS document with structured content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Type name from header (e.g., "achievement", "itempool")
    pub type_name: String,
    /// Format code (e.g., "abjx", "abij")
    pub format_code: String,
    /// Records extracted from the content
    pub records: Vec<Record>,
}

/// A single record with key-value entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Entry name/identifier
    pub name: String,
    /// Field values
    pub fields: HashMap<String, Value>,
    /// Dependent entries (for format codes with 'x')
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dep_entries: Vec<Record>,
}

/// Value types in NCS records
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Reference(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Null,
}

/// Header information from NCS content
#[derive(Debug, Clone)]
pub struct Header {
    /// Offset where type name starts
    pub type_offset: usize,
    /// Type name
    pub type_name: String,
    /// Format code offset
    pub format_offset: usize,
    /// Format code (4 chars like "abjx")
    pub format_code: String,
    /// GUID bytes (if present)
    pub guid: Option<[u8; 16]>,
    /// Offset where string table begins
    pub string_table_offset: usize,
    /// Offset where binary data begins
    pub binary_offset: usize,
}

/// String table extracted from NCS content
#[derive(Debug, Clone)]
pub struct StringTable {
    /// All strings in order
    pub strings: Vec<String>,
    /// Map from string content to index
    pub index_map: HashMap<String, usize>,
}

impl StringTable {
    /// Get string by index
    pub fn get(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(|s| s.as_str())
    }

    /// Get index of a string
    pub fn find(&self, s: &str) -> Option<usize> {
        self.index_map.get(s).copied()
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// Bitstream reader for parsing packed binary data
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read n bits as u32
    pub fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 || n > 32 {
            return None;
        }

        let mut result: u32 = 0;
        let mut bits_read = 0u8;

        while bits_read < n {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let remaining_in_byte = 8 - self.bit_pos;
            let bits_to_read = remaining_in_byte.min(n - bits_read);

            let mask = ((1u32 << bits_to_read) - 1) as u8;
            let byte_val = self.data[self.byte_pos];
            let extracted = (byte_val >> self.bit_pos) & mask;

            result |= (extracted as u32) << bits_read;
            bits_read += bits_to_read;
            self.bit_pos += bits_to_read;

            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Some(result)
    }

    /// Read a single bit
    pub fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|v| v != 0)
    }

    /// Read variable-length integer (Elias gamma coding)
    pub fn read_varint(&mut self) -> Option<u32> {
        // Count leading zeros
        let mut zeros = 0u8;
        while !self.read_bit()? {
            zeros += 1;
            if zeros > 30 {
                return None;
            }
        }

        if zeros == 0 {
            return Some(1);
        }

        // Read the value bits
        let value = self.read_bits(zeros)?;
        Some((1 << zeros) | value)
    }

    /// Check if we've reached end of data
    pub fn is_empty(&self) -> bool {
        self.byte_pos >= self.data.len()
    }

    /// Get current position in bits
    pub fn position(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }
}

/// Parse NCS content header
pub fn parse_header(data: &[u8]) -> Option<Header> {
    if data.len() < 20 {
        return None;
    }

    // Find type name start - look for first alphabetic byte after zeros
    let mut type_offset = None;
    for i in 0..32.min(data.len()) {
        if i > 0 && data[i - 1] == 0 && data[i].is_ascii_alphabetic() {
            type_offset = Some(i);
            break;
        }
    }

    let type_offset = type_offset?;

    // Find null terminator for type name
    let type_end = data[type_offset..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| type_offset + p)?;

    let type_name = std::str::from_utf8(&data[type_offset..type_end])
        .ok()?
        .to_string();

    if type_name.len() < 2 {
        return None;
    }

    // Find format code (4 bytes starting with "ab")
    // Usually 4-6 bytes after type name null terminator
    let mut format_offset = None;
    for i in type_end + 1..type_end + 10.min(data.len()) {
        if i + 4 <= data.len() && &data[i..i + 2] == b"ab" {
            // Verify all 4 bytes are lowercase letters
            if data[i..i + 4].iter().all(|&b| b.is_ascii_lowercase()) {
                format_offset = Some(i);
                break;
            }
        }
    }

    let format_offset = format_offset?;
    let format_code = std::str::from_utf8(&data[format_offset..format_offset + 4])
        .ok()?
        .to_string();

    // String table starts after format code + some header bytes
    // Usually 4 more bytes (possibly GUID start), then strings begin
    let after_format = format_offset + 4;
    let string_table_offset = find_string_table_start(data, after_format)?;

    // Binary section starts after string table ends
    let binary_offset = find_binary_section(data, string_table_offset)?;

    Some(Header {
        type_offset,
        type_name,
        format_offset,
        format_code,
        guid: None,
        string_table_offset,
        binary_offset,
    })
}

/// Find where string table begins
fn find_string_table_start(data: &[u8], after: usize) -> Option<usize> {
    // Look for first printable string after the format header
    for i in after..after + 20.min(data.len() - after) {
        if data[i].is_ascii_alphabetic() || data[i] == b'/' || data[i] == b'_' {
            // Verify it's followed by more printable chars and then null
            let mut j = i;
            while j < data.len() && (data[j].is_ascii_graphic() || data[j] == b' ') {
                j += 1;
            }
            if j > i + 1 && j < data.len() && data[j] == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Find where binary section begins (after string table)
fn find_binary_section(data: &[u8], string_start: usize) -> Option<usize> {
    // Scan through strings until we find a pattern that's clearly not strings
    let mut pos = string_start;
    let mut consecutive_non_printable = 0;

    while pos < data.len() {
        if data[pos] == 0 {
            pos += 1;
            continue;
        }

        if !data[pos].is_ascii_graphic() && data[pos] != b' ' {
            consecutive_non_printable += 1;
            if consecutive_non_printable > 3 {
                // Found binary section
                return Some(pos - consecutive_non_printable);
            }
        } else {
            consecutive_non_printable = 0;
        }
        pos += 1;
    }

    Some(data.len())
}

/// Parse string table from NCS content
pub fn parse_string_table(data: &[u8], header: &Header) -> StringTable {
    let mut strings = Vec::new();
    let mut index_map = HashMap::new();
    let mut current = Vec::new();
    let mut in_string = false;

    let end = header.binary_offset.min(data.len());

    for &byte in &data[header.string_table_offset..end] {
        if byte == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    if is_valid_string(s) {
                        index_map.insert(s.to_string(), strings.len());
                        strings.push(s.to_string());
                    }
                }
                current.clear();
            }
            in_string = false;
        } else if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
            in_string = true;
        } else if in_string && current.len() >= 2 {
            // Non-printable ends string
            if let Ok(s) = std::str::from_utf8(&current) {
                if is_valid_string(s) {
                    index_map.insert(s.to_string(), strings.len());
                    strings.push(s.to_string());
                }
            }
            current.clear();
            in_string = false;
        } else {
            current.clear();
            in_string = false;
        }
    }

    // Handle trailing string
    if !current.is_empty() {
        if let Ok(s) = std::str::from_utf8(&current) {
            if is_valid_string(s) {
                index_map.insert(s.to_string(), strings.len());
                strings.push(s.to_string());
            }
        }
    }

    StringTable { strings, index_map }
}

fn is_valid_string(s: &str) -> bool {
    if s.len() < 1 {
        return false;
    }
    // Must have at least some letters
    let letter_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    letter_count >= 1 || s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// Parse document based on format code
pub fn parse_document(data: &[u8]) -> Option<Document> {
    let header = parse_header(data)?;
    let string_table = parse_string_table(data, &header);

    let records = match header.format_code.as_str() {
        "abjx" => parse_abjx(data, &header, &string_table),
        "abij" => parse_abij(data, &header, &string_table),
        "abhj" => parse_abhj(data, &header, &string_table),
        "abpe" => parse_abpe(data, &header, &string_table),
        "abqr" => parse_abqr(data, &header, &string_table),
        _ => parse_generic(data, &header, &string_table),
    };

    Some(Document {
        type_name: header.type_name,
        format_code: header.format_code,
        records,
    })
}

/// Parse abjx format (most common)
/// Structure: entries with JSON-like fields, extended with dep_entries
fn parse_abjx(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header, strings, true)
}

/// Parse abij format
/// Structure: indexed entries with JSON-like fields
fn parse_abij(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header, strings, false)
}

/// Parse abhj format
/// Structure: hash-indexed entries with JSON-like fields
fn parse_abhj(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header, strings, false)
}

/// Parse abpe format
/// Structure: property-based entries (used by audio_event)
fn parse_abpe(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_entries_format(data, header, strings, false)
}

/// Parse abqr format
/// Structure: quiet/reference format (used by DialogQuietTime)
fn parse_abqr(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    // abqr has offset tables at the start - different structure
    // For now, extract what we can from strings
    parse_strings_as_records(strings)
}

/// Generic fallback parser
fn parse_generic(data: &[u8], header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_strings_as_records(strings)
}

/// Parse entries-based format (abjx, abij, abhj, abpe)
fn parse_entries_format(
    _data: &[u8],
    header: &Header,
    strings: &StringTable,
    _has_dep_entries: bool,
) -> Vec<Record> {
    // Parse entries by detecting record boundaries
    // For achievement-like types, entries are bounded by numeric IDs
    // For other types, entries start with identifiers (ID_, CamelCase, etc.)

    let mut records = Vec::new();
    let schema = get_schema(&header.type_name);

    match schema.boundary {
        RecordBoundary::NumericId => parse_by_numeric_id(strings, &schema, &mut records),
        RecordBoundary::Identifier => parse_by_identifier(strings, &schema, &mut records),
    }

    records
}

/// Schema definition for NCS types
struct TypeSchema {
    field_names: Vec<&'static str>,
    boundary: RecordBoundary,
    id_field: &'static str,
}

enum RecordBoundary {
    NumericId,
    Identifier,
}

fn get_schema(type_name: &str) -> TypeSchema {
    match type_name {
        "achievement" => TypeSchema {
            field_names: vec!["name", "achievementid"],
            boundary: RecordBoundary::Identifier,
            id_field: "achievementid",
        },
        "itempool" | "itempoollist" => TypeSchema {
            field_names: vec!["name", "weight"],
            boundary: RecordBoundary::Identifier,
            id_field: "name",
        },
        "rarity" => TypeSchema {
            field_names: vec!["name", "weight", "color"],
            boundary: RecordBoundary::Identifier,
            id_field: "name",
        },
        "manufacturer" => TypeSchema {
            field_names: vec!["name", "alias", "id"],
            boundary: RecordBoundary::Identifier,
            id_field: "name",
        },
        _ => TypeSchema {
            field_names: vec![],
            boundary: RecordBoundary::Identifier,
            id_field: "name",
        },
    }
}

fn parse_by_numeric_id(strings: &StringTable, schema: &TypeSchema, records: &mut Vec<Record>) {
    let mut current_record: Option<Record> = None;
    let mut field_index = 0;

    for s in &strings.strings {
        if is_metadata(s) {
            continue;
        }

        // Pure numeric strings start new records for this schema type
        if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
            // Save previous record
            if let Some(rec) = current_record.take() {
                records.push(rec);
            }

            // Start new record with ID
            let mut rec = Record {
                name: format!("entry_{}", s),
                fields: HashMap::new(),
                dep_entries: Vec::new(),
            };
            rec.fields
                .insert(schema.id_field.to_string(), Value::Integer(s.parse().unwrap_or(0)));
            current_record = Some(rec);
            field_index = 1; // ID already added
        } else if let Some(ref mut rec) = current_record {
            if let Some(value) = parse_string_value(s) {
                let field_name = if field_index < schema.field_names.len() {
                    schema.field_names[field_index].to_string()
                } else {
                    format!("value_{}", field_index - schema.field_names.len())
                };
                rec.fields.insert(field_name, value);
                field_index += 1;
            }
        }
    }

    if let Some(rec) = current_record {
        records.push(rec);
    }
}

fn parse_by_identifier(strings: &StringTable, schema: &TypeSchema, records: &mut Vec<Record>) {
    let mut current_record: Option<Record> = None;
    let mut field_index = 0;

    for s in &strings.strings {
        if is_metadata(s) {
            continue;
        }

        if is_entry_identifier(s) {
            // Save previous record
            if let Some(rec) = current_record.take() {
                if !rec.name.is_empty() {
                    records.push(rec);
                }
            }

            // Start new record
            current_record = Some(Record {
                name: s.clone(),
                fields: HashMap::new(),
                dep_entries: Vec::new(),
            });
            field_index = 0;
        } else if let Some(ref mut rec) = current_record {
            if let Some(value) = parse_string_value(s) {
                let field_name = if field_index < schema.field_names.len() {
                    schema.field_names[field_index].to_string()
                } else {
                    format!("value_{}", field_index - schema.field_names.len())
                };
                rec.fields.insert(field_name, value);
                field_index += 1;
            }
        }
    }

    if let Some(rec) = current_record {
        if !rec.name.is_empty() || !rec.fields.is_empty() {
            records.push(rec);
        }
    }
}

/// Get schema field names for known types
fn get_schema_fields(type_name: &str) -> Vec<&'static str> {
    match type_name {
        "achievement" => vec!["achievementid"],
        "itempool" => vec!["weight", "pool"],
        "rarity" => vec!["weight", "color"],
        "manufacturer" => vec!["name", "alias"],
        "weapon_type" => vec!["name", "category"],
        "attribute" => vec!["name", "value", "modifier"],
        _ => vec![],
    }
}

/// Parse strings into simple records
fn parse_strings_as_records(strings: &StringTable) -> Vec<Record> {
    let mut records = Vec::new();

    // Group strings into logical entries
    let mut entries: Vec<Vec<String>> = Vec::new();
    let mut current_entry: Vec<String> = Vec::new();

    for s in &strings.strings {
        if is_entry_identifier(s) {
            if !current_entry.is_empty() {
                entries.push(current_entry);
                current_entry = Vec::new();
            }
        }
        current_entry.push(s.clone());
    }

    if !current_entry.is_empty() {
        entries.push(current_entry);
    }

    // Convert to records
    for entry_strings in entries {
        if entry_strings.is_empty() {
            continue;
        }

        let name = entry_strings[0].clone();
        let mut fields = HashMap::new();

        for (i, s) in entry_strings.iter().skip(1).enumerate() {
            if let Some(value) = parse_string_value(s) {
                let field_name = if is_field_name(s) {
                    s.clone()
                } else {
                    format!("value_{}", i)
                };
                fields.insert(field_name, value);
            }
        }

        records.push(Record {
            name,
            fields,
            dep_entries: Vec::new(),
        });
    }

    records
}

fn is_field_name(s: &str) -> bool {
    // Field names are typically lowercase with underscores
    s.len() >= 2
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        && s.chars().next().map_or(false, |c| c.is_ascii_alphabetic())
}

fn is_entry_identifier(s: &str) -> bool {
    // Entry identifiers are typically:
    // - CamelCase or UPPER_CASE
    // - Start with uppercase or contain uppercase
    // - May contain ID_, /Script/, etc.
    if s.len() < 2 {
        return false;
    }

    // Definite entry markers
    if s.starts_with("ID_")
        || s.starts_with("/Script/")
        || s.starts_with("/Game/")
        || s.contains("_def")
    {
        return true;
    }

    // Check for CamelCase or mixed case
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());

    has_upper && has_lower && s.chars().next().map_or(false, |c| c.is_ascii_uppercase())
}

fn is_metadata(s: &str) -> bool {
    matches!(s, "none" | "basegame" | "base") || s.starts_with("cor")
}

fn parse_string_value(s: &str) -> Option<Value> {
    // Try to parse as number
    if let Ok(n) = s.parse::<i64>() {
        return Some(Value::Integer(n));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Some(Value::Number(f));
    }

    // Boolean
    if s == "true" {
        return Some(Value::Boolean(true));
    }
    if s == "false" {
        return Some(Value::Boolean(false));
    }

    // Reference (starts with /)
    if s.starts_with('/') {
        return Some(Value::Reference(s.to_string()));
    }

    // Just a string
    Some(Value::String(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10110101, 0b11001010];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(4), Some(0b1011));
    }

    #[test]
    fn test_bit_reader_cross_byte() {
        let data = [0xFF, 0xFF];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(12), Some(0xFFF));
    }

    #[test]
    fn test_parse_string_value() {
        assert!(matches!(parse_string_value("123"), Some(Value::Integer(123))));
        assert!(matches!(parse_string_value("1.5"), Some(Value::Number(_))));
        assert!(matches!(parse_string_value("true"), Some(Value::Boolean(true))));
        assert!(matches!(parse_string_value("/Script/Test"), Some(Value::Reference(_))));
        assert!(matches!(parse_string_value("hello"), Some(Value::String(_))));
    }

    #[test]
    fn test_is_entry_identifier() {
        assert!(is_entry_identifier("ID_Test_123"));
        assert!(is_entry_identifier("CamelCase"));
        assert!(is_entry_identifier("/Script/OakGame"));
        assert!(!is_entry_identifier("lowercase"));
        assert!(!is_entry_identifier("12345"));
    }

    #[test]
    fn test_is_field_name() {
        assert!(is_field_name("field_name"));
        assert!(is_field_name("value"));
        assert!(!is_field_name("CamelCase"));
        assert!(!is_field_name("UPPER"));
    }
}
