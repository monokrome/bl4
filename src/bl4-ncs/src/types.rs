//! Core types for NCS document parsing
//!
//! Defines the main data structures used to represent parsed NCS content.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Field count per entry (extracted from entry section marker)
    pub field_count: u8,
    /// Number of strings in string table (if known from header)
    pub string_count: Option<u32>,
    /// Offset where entry section begins (after format code)
    pub entry_section_offset: usize,
    /// Offset where string table begins
    pub string_table_offset: usize,
    /// Offset where control section begins (marks end of string table)
    pub control_section_offset: Option<usize>,
    /// Offset where category names begin (after control section)
    pub category_names_offset: Option<usize>,
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
    /// Create a new empty string table
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            index_map: HashMap::new(),
        }
    }

    /// Create a string table from strings and index map
    pub fn with_data(strings: Vec<String>, index_map: HashMap<String, usize>) -> Self {
        Self { strings, index_map }
    }

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

impl Default for StringTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Tag types from the NCS binary format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Pair,      // 0x61 'a' - string reference
    U32,       // 0x62 'b' - 32-bit unsigned
    U32F32,    // 0x63 'c' - 32-bit as both u32 and f32
    List,      // 0x64-0x66 'd', 'e', 'f' - list terminated by "none"
    Variant,   // 0x70 'p' - variant with 2-bit subtype
    End,       // 0x7a 'z' - end of tags section
    Unknown(u8),
}

impl TagType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x61 => TagType::Pair,
            0x62 => TagType::U32,
            0x63 => TagType::U32F32,
            0x64 | 0x65 | 0x66 => TagType::List,
            0x70 => TagType::Variant,
            0x7a => TagType::End,
            _ => TagType::Unknown(b),
        }
    }
}

/// Parsed tag value
#[derive(Debug, Clone)]
pub enum TagValue {
    Pair(String),
    U32(u32),
    U32F32 { u: u32, f: f32 },
    List(Vec<String>),
    Variant { subtype: u8, value: Box<TagValue> },
    Empty,
}

/// Represents an unpacked value from a packed NCS string
#[derive(Debug, Clone, PartialEq)]
pub enum UnpackedValue {
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Boolean value
    Boolean(bool),
}

/// Result of unpacking a packed string
#[derive(Debug, Clone)]
pub struct UnpackedString {
    /// Original packed string
    pub original: String,
    /// Unpacked values in order
    pub values: Vec<UnpackedValue>,
    /// Whether the string was actually packed (multiple values)
    pub was_packed: bool,
}

// ============================================================================
// Binary Section Types
// ============================================================================

/// Result of parsing the binary section
#[derive(Debug, Clone)]
pub struct BinaryParseResult {
    /// Table ID (index into string table, typically points to type name)
    pub table_id: u32,
    /// Bit-packed string indices from the first section
    pub bit_indices: Vec<u32>,
    /// Entry metadata groups (separated by 0x28 or 0x20)
    pub entry_groups: Vec<EntryGroup>,
    /// Tail data after the structured section
    pub tail_data: Vec<u8>,
}

/// Metadata group for a single entry
#[derive(Debug, Clone)]
pub struct EntryGroup {
    /// Raw byte values in this group
    pub values: Vec<u8>,
    /// Interpreted field offsets/widths
    pub field_info: Vec<FieldInfo>,
}

/// Field metadata parsed from entry group
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// Bit offset into the packed section
    pub bit_offset: u32,
    /// Bit width of this field
    pub bit_width: u8,
    /// String index (if resolved)
    pub string_index: Option<u32>,
}

/// A record from the binary section (legacy structure for compatibility)
#[derive(Debug, Clone)]
pub struct BinaryRecord {
    /// Tags in this record
    pub tags: Vec<(TagType, TagValue)>,
    /// Entry values (2-bit type + data)
    pub entries: Vec<BinaryEntry>,
}

/// An entry from a binary record
#[derive(Debug, Clone)]
pub struct BinaryEntry {
    /// Entry type (0=end, 1=simple, 2=variant, 3=ref)
    pub entry_type: u8,
    /// Entry name (string index)
    pub name_index: u32,
    /// Entry value
    pub value: TagValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_table() {
        let strings = vec!["foo".to_string(), "bar".to_string()];
        let mut index_map = HashMap::new();
        index_map.insert("foo".to_string(), 0);
        index_map.insert("bar".to_string(), 1);

        let table = StringTable::with_data(strings, index_map);

        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert_eq!(table.get(0), Some("foo"));
        assert_eq!(table.get(1), Some("bar"));
        assert_eq!(table.get(2), None);
        assert_eq!(table.find("foo"), Some(0));
        assert_eq!(table.find("baz"), None);
    }

    #[test]
    fn test_tag_type_from_byte() {
        assert_eq!(TagType::from_byte(0x61), TagType::Pair);
        assert_eq!(TagType::from_byte(0x62), TagType::U32);
        assert_eq!(TagType::from_byte(0x63), TagType::U32F32);
        assert_eq!(TagType::from_byte(0x64), TagType::List);
        assert_eq!(TagType::from_byte(0x70), TagType::Variant);
        assert_eq!(TagType::from_byte(0x7a), TagType::End);
        assert_eq!(TagType::from_byte(0x00), TagType::Unknown(0x00));
    }
}
