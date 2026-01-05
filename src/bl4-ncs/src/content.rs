//! NCS content parsing for decompressed data
//!
//! Parses the internal structure of decompressed NCS files to extract
//! type information, string tables, and data entries.
//!
//! ## Format Codes
//!
//! Each NCS file has a 4-letter format code (e.g., "abjx", "abij") that indicates
//! the structure and features present:
//!
//! - `ab` - Base prefix (always present)
//! - `i` - Indexed entries with table references
//! - `j` - JSON-like nested structure
//! - `l` - List of items
//! - `m` - Map/dictionary structure
//! - `p` - Property definitions
//! - `x` - Extended attributes
//! - `h` - Hash table
//! - `e` - Enum values

use std::collections::HashMap;

use crate::header::{find_type_starts, parse_basic_header_with_config, ParseConfig};

/// NCS content header
#[derive(Debug, Clone)]
pub struct Header {
    /// Type name (e.g., "itempoollist", "trait_pool")
    pub type_name: String,
    /// Format code (e.g., "abjx", "abij")
    pub format_code: String,
    /// Raw header bytes for analysis
    pub raw_header: Vec<u8>,
}

/// Parsed NCS content
#[derive(Debug, Clone)]
pub struct Content {
    /// Header information
    pub header: Header,
    /// String table entries
    pub strings: Vec<String>,
    /// Key-value pairs extracted from content
    pub metadata: HashMap<String, String>,
}

impl Content {
    /// Parse NCS content from decompressed data
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }

        // Try standard header parsing first (most common case)
        if let Some(content) = Self::try_parse_standard(data) {
            return Some(content);
        }

        // Fall back to trying each potential type start position
        for type_start in find_type_starts(data) {
            if let Some(content) = Self::try_parse_at(data, type_start) {
                return Some(content);
            }
        }
        None
    }

    /// Try to parse using standard header parsing
    fn try_parse_standard(data: &[u8]) -> Option<Self> {
        let config = ParseConfig::default();
        let basic = parse_basic_header_with_config(data, &config)?;

        // String table starts after format code + null terminator
        let strings_start = basic.format_offset + basic.format_code.len() + 1;
        let strings = extract_strings(data, strings_start);
        let metadata = extract_metadata(&strings);

        Some(Self {
            header: Header {
                type_name: basic.type_name,
                format_code: basic.format_code,
                raw_header: basic.prefix_bytes,
            },
            strings,
            metadata,
        })
    }

    /// Try to parse content starting from a specific offset (legacy fallback)
    fn try_parse_at(data: &[u8], type_start: usize) -> Option<Self> {
        use crate::header::find_null;

        let type_end = find_null(data, type_start)?;

        // Type name must be at least 2 chars
        if type_end <= type_start + 1 {
            return None;
        }

        let type_name = std::str::from_utf8(&data[type_start..type_end])
            .ok()?
            .to_string();

        // Validate type name
        if type_name.len() < 2
            || !type_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }

        // Find format code using shared logic
        let _config = ParseConfig {
            type_search_start: type_start,
            type_search_end: type_start + 1,
            format_search_range: 600,
        };

        // Use memmem for format code search
        let format_start = find_format_code_after(data, type_end)?;

        // Read format code - can be 4-10+ chars like "abcefhijl" or "abhX"
        let search_end = (format_start + 20).min(data.len());
        let format_end = format_start + memchr::memchr(0, &data[format_start..search_end]).unwrap_or(4);

        // Validate format code: must be all alphabetic
        let format_bytes = &data[format_start..format_end];
        let valid_end = format_bytes
            .iter()
            .position(|&b| !b.is_ascii_alphabetic())
            .map(|p| format_start + p)
            .unwrap_or(format_end);

        let format_code = std::str::from_utf8(&data[format_start..valid_end])
            .ok()?
            .to_string();

        let strings = extract_strings(data, valid_end + 1);
        let metadata = extract_metadata(&strings);

        Some(Self {
            header: Header {
                type_name,
                format_code,
                raw_header: data[..type_start].to_vec(),
            },
            strings,
            metadata,
        })
    }

    /// Get the NCS type name
    pub fn type_name(&self) -> &str {
        &self.header.type_name
    }

    /// Get the format code
    pub fn format_code(&self) -> &str {
        &self.header.format_code
    }

    /// Check if this is a specific type
    pub fn is_type(&self, name: &str) -> bool {
        self.header.type_name == name
    }

    /// Check if format has indexed entries
    pub fn has_indexed_entries(&self) -> bool {
        self.header.format_code.contains('i')
    }

    /// Check if format has list structure
    pub fn has_list(&self) -> bool {
        self.header.format_code.contains('l')
    }

    /// Check if format has properties
    pub fn has_properties(&self) -> bool {
        self.header.format_code.contains('p')
    }

    /// Get strings that look like GUIDs
    pub fn guids(&self) -> impl Iterator<Item = &str> {
        self.strings.iter().filter_map(|s| {
            // GUID format: 32 hex chars, often with underscores or hyphens
            if s.len() >= 32 && s.chars().filter(|c| c.is_ascii_hexdigit()).count() >= 28 {
                Some(s.as_str())
            } else {
                None
            }
        })
    }

    /// Get strings that look like asset paths
    pub fn asset_paths(&self) -> impl Iterator<Item = &str> {
        self.strings.iter().filter_map(|s| {
            if s.starts_with("/Script/")
                || s.starts_with("/Game/")
                || s.contains('.') && s.contains('_')
            {
                Some(s.as_str())
            } else {
                None
            }
        })
    }

    /// Get strings that look like numeric values
    pub fn numeric_values(&self) -> impl Iterator<Item = (&str, f64)> {
        self.strings
            .iter()
            .filter_map(|s| s.parse::<f64>().ok().map(|v| (s.as_str(), v)))
    }

    /// Get entry names (strings that look like identifiers)
    pub fn entry_names(&self) -> impl Iterator<Item = &str> {
        self.strings.iter().filter_map(|s| {
            // Entry names are typically CamelCase or snake_case, start with letter/underscore
            if s.len() >= 3
                && (s
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_alphabetic() || c == '_')
                    .unwrap_or(false))
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !s.chars().all(|c| c.is_ascii_lowercase())
            {
                Some(s.as_str())
            } else {
                None
            }
        })
    }
}

/// Find format code after a given offset using memmem
fn find_format_code_after(data: &[u8], after: usize) -> Option<usize> {
    use memchr::memmem;

    let search_start = after + 3;
    let search_end = (after + 600).min(data.len());

    if search_start >= search_end {
        return None;
    }

    let finder = memmem::Finder::new(b"ab");
    let search_slice = &data[search_start..search_end];

    let mut offset = 0;
    while let Some(rel_pos) = finder.find(&search_slice[offset..]) {
        let abs_pos = search_start + offset + rel_pos;

        if abs_pos + 4 <= data.len() {
            let code_bytes = &data[abs_pos..abs_pos + 4];
            if code_bytes.iter().all(|&b| b.is_ascii_alphabetic()) {
                if abs_pos > 0 && data[abs_pos - 1] <= 3 {
                    return Some(abs_pos);
                }
            }
        }

        offset += rel_pos + 1;
        if offset >= search_slice.len() {
            break;
        }
    }

    None
}

/// Extract all readable strings from data
fn extract_strings(data: &[u8], start: usize) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    let mut in_string = false;

    for &byte in &data[start..] {
        if byte == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    if is_valid_string(s) {
                        strings.push(s.to_string());
                    }
                }
                current.clear();
            }
            in_string = false;
        } else if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
            in_string = true;
        } else if in_string && current.len() >= 3 {
            if let Ok(s) = std::str::from_utf8(&current) {
                if is_valid_string(s) {
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
                strings.push(s.to_string());
            }
        }
    }

    strings
}

/// Check if a string is valid (not just noise)
fn is_valid_string(s: &str) -> bool {
    if s.len() < 2 {
        return false;
    }
    let letter_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    letter_count >= s.len() / 3
}

/// Extract metadata from string patterns
fn extract_metadata(strings: &[String]) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    for s in strings {
        if s == "none" || s == "basegame" || s == "base" {
            metadata.insert("namespace".to_string(), s.clone());
        } else if s.starts_with("cor") && s.len() > 3 {
            metadata.insert("correlation".to_string(), s.clone());
        } else if s.contains("_def") {
            metadata.insert("definition".to_string(), s.clone());
        }
    }

    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_ncs(type_name: &str, format_code: &str) -> Vec<u8> {
        let mut data = vec![0u8; 5]; // Header zeros
        data.extend_from_slice(&[0x01, 0x8f]); // Size bytes
        data.extend_from_slice(&[0x0e, 0x00]); // Format bytes
        data.extend_from_slice(type_name.as_bytes());
        data.push(0); // Null terminator
        data.extend_from_slice(&[0x03, 0x05, 0x00]); // Format info
        data.extend_from_slice(format_code.as_bytes());
        data.extend_from_slice(&[0x1d, 0x06, 0x01]); // Entry info
        data.extend_from_slice(b"test_entry\0");
        data.extend_from_slice(b"12.000000\0");
        data.extend_from_slice(b"none\0");
        data.extend_from_slice(b"basegame\0");
        data
    }

    #[test]
    fn test_parse_basic() {
        let data = make_test_ncs("itempoollist", "abjx");
        let content = Content::parse(&data).unwrap();

        assert_eq!(content.type_name(), "itempoollist");
        assert_eq!(content.format_code(), "abjx");
    }

    #[test]
    fn test_parse_strings() {
        let data = make_test_ncs("trait_pool", "abjx");
        let content = Content::parse(&data).unwrap();

        assert!(content.strings.iter().any(|s| s == "test_entry"));
        assert!(content.strings.iter().any(|s| s == "none"));
        assert!(content.strings.iter().any(|s| s == "basegame"));
    }

    #[test]
    fn test_is_type() {
        let data = make_test_ncs("vending_machine", "abhj");
        let content = Content::parse(&data).unwrap();

        assert!(content.is_type("vending_machine"));
        assert!(!content.is_type("itempoollist"));
    }

    #[test]
    fn test_parse_too_short() {
        let data = vec![0u8; 10];
        assert!(Content::parse(&data).is_none());
    }

    #[test]
    fn test_metadata_extraction() {
        let data = make_test_ncs("test_type", "abjx");
        let content = Content::parse(&data).unwrap();

        assert!(content.metadata.contains_key("namespace"));
    }

    #[test]
    fn test_header_debug() {
        let data = make_test_ncs("test", "abjx");
        let content = Content::parse(&data).unwrap();
        let debug = format!("{:?}", content.header);
        assert!(debug.contains("Header"));
    }

    #[test]
    fn test_content_clone() {
        let data = make_test_ncs("test", "abjx");
        let content = Content::parse(&data).unwrap();
        let cloned = content.clone();
        assert_eq!(content.type_name(), cloned.type_name());
    }

    #[test]
    fn test_variable_null_padding() {
        let mut data = vec![0u8; 8];
        data.push(0);
        data.extend_from_slice(b"test_type");
        data.push(0);
        data.push(0);
        data.extend_from_slice(&[0x03, 0x03, 0x00]);
        data.extend_from_slice(b"abjx");
        data.extend_from_slice(b"\x01\x06\x01");
        data.extend_from_slice(b"entry\0");

        let content = Content::parse(&data).expect("Should parse with extra null");
        assert_eq!(content.type_name(), "test_type");
        assert_eq!(content.format_code(), "abjx");
    }
}
