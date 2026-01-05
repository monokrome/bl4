//! Shared NCS header parsing
//!
//! Provides unified header parsing used by both content.rs and parser.rs.
//! Uses memchr for efficient SIMD-accelerated pattern searching.

use memchr::{memchr, memmem};

/// Basic header info extracted from NCS data
#[derive(Debug, Clone)]
pub struct BasicHeader {
    /// Offset where type name starts
    pub type_offset: usize,
    /// Offset where type name ends (null terminator)
    #[allow(dead_code)]
    pub type_end: usize,
    /// Type name (e.g., "itempoollist", "achievement")
    pub type_name: String,
    /// Offset where format code starts
    pub format_offset: usize,
    /// Format code (e.g., "abjx", "abij")
    pub format_code: String,
    /// Bytes before type name (header prefix)
    pub prefix_bytes: Vec<u8>,
}

/// Configuration for header parsing
#[derive(Debug, Clone)]
pub struct ParseConfig {
    /// Minimum offset to start searching for type name
    pub type_search_start: usize,
    /// Maximum offset to search for type name
    pub type_search_end: usize,
    /// Maximum distance after type name to search for format code
    pub format_search_range: usize,
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self {
            type_search_start: 1,
            type_search_end: 32,
            format_search_range: 600,
        }
    }
}

impl ParseConfig {
    /// Standard config for parser.rs (stricter, starts at byte 8)
    pub fn strict() -> Self {
        Self {
            type_search_start: 8,
            type_search_end: 32,
            format_search_range: 600,
        }
    }
}

/// Parse basic header information from NCS data
///
/// Uses SIMD-accelerated memchr for efficient pattern matching.
#[allow(dead_code)]
pub fn parse_basic_header(data: &[u8]) -> Option<BasicHeader> {
    parse_basic_header_with_config(data, &ParseConfig::default())
}

/// Parse header with custom configuration
pub fn parse_basic_header_with_config(data: &[u8], config: &ParseConfig) -> Option<BasicHeader> {
    if data.len() < 20 {
        return None;
    }

    // Find type name start - look for alphabetic byte after null
    let type_offset = find_type_start(data, config)?;

    // Find null terminator for type name using memchr
    let type_end = type_offset + memchr(0, &data[type_offset..])?;

    // Type name must be at least 2 chars
    if type_end <= type_offset + 1 {
        return None;
    }

    let type_name = std::str::from_utf8(&data[type_offset..type_end])
        .ok()?
        .to_string();

    // Validate type name
    if !is_valid_type_name(&type_name) {
        return None;
    }

    // Find format code using memmem for "ab" pattern
    let format_offset = find_format_code(data, type_end, config)?;

    // Read format code - can be 4-10+ chars like "abcefhijl" or "abhX"
    // Find the null terminator, but limit to reasonable length
    let search_end = (format_offset + 20).min(data.len());
    let format_end = format_offset + memchr(0, &data[format_offset..search_end]).unwrap_or(4);

    // Validate format code: must be all alphabetic (allow uppercase like "abhX")
    let format_bytes = &data[format_offset..format_end];
    let valid_end = format_bytes
        .iter()
        .position(|&b| !b.is_ascii_alphabetic())
        .map(|p| format_offset + p)
        .unwrap_or(format_end);

    let format_code = std::str::from_utf8(&data[format_offset..valid_end])
        .ok()?
        .to_string();

    Some(BasicHeader {
        type_offset,
        type_end,
        type_name,
        format_offset,
        format_code,
        prefix_bytes: data[..type_offset].to_vec(),
    })
}

/// Find where type name starts in the data
fn find_type_start(data: &[u8], config: &ParseConfig) -> Option<usize> {
    let search_start = config.type_search_start;
    let search_end = config.type_search_end.min(data.len());

    // Look for null byte followed by alphabetic character
    for i in search_start..search_end {
        if i > 0 && data[i - 1] == 0 && data[i].is_ascii_alphabetic() {
            return Some(i);
        }
    }
    None
}

/// Find format code position using memchr
fn find_format_code(data: &[u8], after: usize, config: &ParseConfig) -> Option<usize> {
    let search_start = after + 3;
    let search_end = (after + config.format_search_range).min(data.len());

    if search_start >= search_end {
        return None;
    }

    // Use memmem finder for "ab" pattern - SIMD accelerated
    let finder = memmem::Finder::new(b"ab");
    let search_slice = &data[search_start..search_end];

    let mut offset = 0;
    while let Some(rel_pos) = finder.find(&search_slice[offset..]) {
        let abs_pos = search_start + offset + rel_pos;

        // Verify it's a valid format code
        if abs_pos + 4 <= data.len() {
            let code_bytes = &data[abs_pos..abs_pos + 4];
            // All 4 bytes must be alphabetic (allows uppercase like "abhX")
            if code_bytes.iter().all(|&b| b.is_ascii_alphabetic()) {
                // Must be preceded by null or control byte
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

/// Validate type name
fn is_valid_type_name(name: &str) -> bool {
    name.len() >= 2 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Find all candidate type start positions (for content.rs compatibility)
pub fn find_type_starts(data: &[u8]) -> Vec<usize> {
    let mut candidates = Vec::with_capacity(4);

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

/// Find null terminator from offset using memchr
#[inline]
pub fn find_null(data: &[u8], start: usize) -> Option<usize> {
    memchr(0, &data[start..]).map(|p| start + p)
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
        data
    }

    #[test]
    fn test_parse_basic_header() {
        let data = make_test_ncs("achievement", "abjx");
        let header = parse_basic_header(&data).unwrap();

        assert_eq!(header.type_name, "achievement");
        assert_eq!(header.format_code, "abjx");
    }

    #[test]
    fn test_parse_uppercase_format() {
        let data = make_test_ncs("credits", "abhX");
        let header = parse_basic_header(&data).unwrap();

        assert_eq!(header.type_name, "credits");
        assert_eq!(header.format_code, "abhX");
    }

    #[test]
    fn test_find_null() {
        let data = b"hello\0world";
        assert_eq!(find_null(data, 0), Some(5));
        assert_eq!(find_null(data, 6), None);
    }

    #[test]
    fn test_type_starts() {
        let mut data = vec![0u8; 10];
        data[0] = 0;
        data[1] = b'a'; // Type starts at 1

        let starts = find_type_starts(&data);
        assert!(starts.contains(&1));
    }
}
