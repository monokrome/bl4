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

        // Try each potential type start position
        for type_start in find_type_starts(data) {
            if let Some(content) = Self::try_parse_at(data, type_start) {
                return Some(content);
            }
        }
        None
    }

    /// Try to parse content starting from a specific offset
    fn try_parse_at(data: &[u8], type_start: usize) -> Option<Self> {
        let type_end = find_null(data, type_start)?;

        // Type name must be at least 2 chars
        if type_end <= type_start + 1 {
            return None;
        }

        let type_name = std::str::from_utf8(&data[type_start..type_end])
            .ok()?
            .to_string();

        // Validate type name (should be alphanumeric with underscores, min 2 chars)
        if type_name.len() < 2
            || !type_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }

        // Find format code (4 chars starting with "ab")
        // May be up to 600 bytes after type name (inv files have huge headers)
        let mut format_start = None;
        let search_end = (type_end + 600).min(data.len());
        for pos in type_end + 3..search_end {
            if pos + 4 <= data.len() {
                if data[pos] == b'a' && data[pos + 1] == b'b' {
                    // Verify it's a valid format code (4 letters, can include uppercase)
                    if data[pos..pos + 4].iter().all(|&b| b.is_ascii_alphabetic()) {
                        // Additional check: should be preceded by a null or control byte
                        if pos > 0 && data[pos - 1] <= 3 {
                            format_start = Some(pos);
                            break;
                        }
                    }
                }
            }
        }
        let format_start = format_start?;

        let format_code = std::str::from_utf8(&data[format_start..format_start + 4])
            .ok()?
            .to_string();

        // Extract strings from the content
        let strings = extract_strings(data, format_start + 4);

        // Build metadata from known patterns
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
            // Not all lowercase (those are keywords)
            {
                Some(s.as_str())
            } else {
                None
            }
        })
    }
}

/// Find candidate positions where the type name might start
fn find_type_starts(data: &[u8]) -> Vec<usize> {
    let mut candidates = Vec::new();

    // Check for 1-zero header format (type name at byte 1)
    if data.len() > 1 && data[0] == 0 {
        let first_char = data[1];
        if first_char.is_ascii_alphabetic() || first_char == b'_' {
            candidates.push(1);
        }
    }

    // Check for header format with null followed by type name (up to byte 32)
    for i in 5..data.len().min(32) {
        if data[i] == 0 && i + 1 < data.len() {
            let next = data[i + 1];
            if next.is_ascii_alphabetic() || next == b'_' {
                candidates.push(i + 1);
            }
        }
    }

    candidates
}

/// Find null terminator from offset
fn find_null(data: &[u8], start: usize) -> Option<usize> {
    data[start..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| start + p)
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
            // Non-printable byte ends string
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
    // Must have at least some letters
    let letter_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    letter_count >= s.len() / 3
}

/// Extract metadata from string patterns
fn extract_metadata(strings: &[String]) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    for s in strings {
        // Look for known patterns
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
                                                     // Add some test strings
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
        // Test that format code is found even with extra null padding
        // Format: header + type_name + NULL + NULL + format_bytes + "abjx"
        let mut data = vec![0u8; 8]; // 8-byte header
        data.push(0); // Null before type name
        data.extend_from_slice(b"test_type"); // Type name
        data.push(0); // Null after type name
        data.push(0); // Extra null (variable padding)
        data.extend_from_slice(&[0x03, 0x03, 0x00]); // Format bytes
        data.extend_from_slice(b"abjx"); // Format code
        data.extend_from_slice(b"\x01\x06\x01"); // Entry info
        data.extend_from_slice(b"entry\0"); // Test entry

        let content = Content::parse(&data).expect("Should parse with extra null");
        assert_eq!(content.type_name(), "test_type");
        assert_eq!(content.format_code(), "abjx");
    }
}
