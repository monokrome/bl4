//! NCS header parsing
//!
//! Handles parsing of NCS file headers including type name, format code,
//! entry section, string table location, and binary section location.

use crate::types::Header;

/// Parse NCS content header
pub fn parse_header(data: &[u8]) -> Option<Header> {
    use crate::header::{parse_basic_header_with_config, ParseConfig};

    if data.len() < 20 {
        return None;
    }

    // Use shared header parsing with strict config (starts at byte 8)
    let config = ParseConfig::strict();
    let basic = parse_basic_header_with_config(data, &config)?;

    let type_offset = basic.type_offset;
    let type_name = basic.type_name;
    let format_offset = basic.format_offset;
    let format_code = basic.format_code.clone();

    // Entry section starts after format code (variable length, e.g., "abjx" or "abcefhijl")
    let entry_section_offset = format_offset + format_code.len();

    // Parse entry section to get field count and string count
    // Structure: [entry_marker=0x01] [string_count] [0xc0 | field_count]
    let (field_count, string_count) = parse_entry_section(data, entry_section_offset);

    // String table starts after entry section bytes
    let string_table_offset = find_string_table_start(data, entry_section_offset)?;

    // Find control section (marks end of string table, start of category names)
    // Pattern: 01 00 XX YY followed by "none", "base", etc.
    let control_section_offset = find_control_section(data, string_table_offset);

    // Category names start 4 bytes after control section
    let category_names_offset = control_section_offset.map(|off| off + 4);

    // Binary section starts after the 7a marker
    let binary_offset = find_binary_section_with_count(data, string_table_offset, string_count)?;

    Some(Header {
        type_offset,
        type_name,
        format_offset,
        format_code,
        guid: None,
        field_count,
        string_count,
        entry_section_offset,
        string_table_offset,
        control_section_offset,
        category_names_offset,
        binary_offset,
    })
}

/// Parse entry section to extract field count and string count
///
/// Returns (field_count, string_count)
///
/// There are several known encoding schemes:
/// 1. Simple format: [0x01] [string_count] [0xc0 | field_count]
///    - 0x01 is entry marker
///    - Next byte is string count
///    - 0xc0-0xcf marker encodes field count in low nibble
///    - Used by: achievement, rarity
///
/// 2. Extended format: [string_count] [field_count] [0x01]
///    - First byte is string/entry count
///    - Second byte is field count directly
///    - Third byte is start marker (0x01)
///    - Used by: itempoollist, itempool
///
/// 3. Direct format: [0x01] [field_count] [0xNN]
///    - 0x01 is entry marker
///    - Second byte is field count directly (1-10)
///    - Third byte varies (offset or string count)
///    - Used by: hit_region (abij format)
pub fn parse_entry_section(data: &[u8], offset: usize) -> (u8, Option<u32>) {
    if offset + 3 > data.len() {
        return (1, None);
    }

    let b0 = data[offset];
    let b1 = data[offset + 1];
    let b2 = data[offset + 2];

    // Check for simple format: [0x01] [string_count] [0xc0 | field_count]
    if b0 == 0x01 && b2 >= 0xc0 && b2 <= 0xcf {
        let field_count = b2 & 0x0f;
        let string_count = b1 as u32;
        if field_count >= 1 && field_count <= 10 {
            return (field_count, Some(string_count));
        }
    }

    // Check for extended format: [string_count] [field_count] [0x01]
    // Pattern: first byte > 0x10 (typical string count), second byte 2-10, third byte = 0x01
    if b0 >= 0x10 && b1 >= 2 && b1 <= 10 && b2 == 0x01 {
        let string_count = b0 as u32;
        let field_count = b1;
        return (field_count, Some(string_count));
    }

    // Check for direct format: [0x01] [field_count] [0xNN]
    // Pattern: 0x01 marker, then small field count (1-10), then other value
    // Used by hit_region with pattern like 01 03 30
    // NOTE: The third byte is NOT a string count in this format - it's likely
    // a flags byte or string table offset. Don't extract string_count here;
    // let the string table parser determine count based on section boundaries.
    if b0 == 0x01 && b1 >= 1 && b1 <= 10 && b2 < 0xc0 {
        let field_count = b1;
        // Don't try to extract string_count from b2 - it's not the count
        return (field_count, None);
    }

    // Fallback: look for 0xc0-0xcf marker in nearby bytes
    for i in offset..offset + 8.min(data.len() - offset) {
        let byte = data[i];
        if byte >= 0xc0 && byte <= 0xcf {
            let count = byte & 0x0f;
            if count >= 1 && count <= 10 {
                return (count, None);
            }
        }
    }

    // Default to 1 field if no pattern matched
    (1, None)
}

/// Find where string table begins
pub fn find_string_table_start(data: &[u8], after: usize) -> Option<usize> {
    // Look for first null-terminated printable string after the format header
    // Some formats (like abjm) have packed data before strings, so search further
    let search_limit = (data.len() / 2).min(512); // Search up to half the file or 512 bytes

    for i in after..after + search_limit.min(data.len() - after) {
        if data[i].is_ascii_alphabetic() || data[i] == b'/' || data[i] == b'_' {
            // Verify it's followed by more printable chars and then null
            let mut j = i;
            while j < data.len() && (data[j].is_ascii_graphic() || data[j] == b' ') {
                j += 1;
            }
            // String must be at least 2 chars and end with null
            if j > i + 1 && j < data.len() && data[j] == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Find the control section offset (marks end of string table)
///
/// Control section pattern: `01 00 XX YY` where:
/// - 01 = marker
/// - 00 = separator
/// - XX = entry/index count
/// - YY = type/mode byte (often 0xe9, 0x62, 0x06, etc.)
pub fn find_control_section(data: &[u8], after: usize) -> Option<usize> {
    // First, try to find "none" and work backwards
    if let Some(none_pos) = find_none_string(data, after) {
        // Pattern A: 01 00 XX YY none (4 bytes before "none")
        if none_pos >= 4 {
            let check_pos = none_pos - 4;
            if data[check_pos] == 0x01 && data[check_pos + 1] == 0x00 {
                return Some(check_pos);
            }
        }

        // Pattern B: XX 00 YY none (3 bytes before "none", where XX != 0x01)
        // This is used by rarity and similar files
        if none_pos >= 3 {
            let check_pos = none_pos - 3;
            let count_byte = data[check_pos];
            if data[check_pos + 1] == 0x00 && count_byte > 0 && count_byte < 0x30 {
                return Some(check_pos);
            }
        }
    }

    // Fallback: scan for original pattern
    for pos in after..data.len().saturating_sub(3) {
        // Look for pattern: 01 00 XX YY where XX and YY are valid control bytes
        if data[pos] == 0x01 && data[pos + 1] == 0x00 {
            let count_byte = data[pos + 2];
            let mode_byte = data[pos + 3];

            // Count should be reasonable (1-255)
            // Mode byte is often 0xe0-0xef range or specific values like 0x62, 0x06
            if count_byte > 0 && (mode_byte >= 0x06 || mode_byte == 0x00) {
                // Verify this is followed by category names like "none"
                if pos + 4 < data.len() {
                    let after_control = pos + 4;
                    // Check if we see "none" or another valid string after
                    if data[after_control..].starts_with(b"none")
                        || data[after_control..].starts_with(b"base")
                        || (data[after_control].is_ascii_alphabetic()
                            && data.get(after_control + 1).map_or(false, |b| b.is_ascii_lowercase()))
                    {
                        return Some(pos);
                    }
                }
            }
        }
    }
    None
}

/// Find the position of "none" category name in data using SIMD-accelerated search
fn find_none_string(data: &[u8], after: usize) -> Option<usize> {
    use memchr::memmem;

    if after >= data.len() {
        return None;
    }

    let finder = memmem::Finder::new(b"none\x00");
    finder.find(&data[after..]).map(|pos| after + pos)
}

/// Find where binary section begins (after string table)
///
/// The binary section starts immediately after all null-terminated strings.
/// We need to count strings using the string_count from the header.
pub fn find_binary_section_with_count(data: &[u8], string_start: usize, expected_string_count: Option<u32>) -> Option<usize> {
    if string_start >= data.len() {
        return Some(data.len());
    }

    // For now, scan through all null-terminated strings
    // Binary section starts right after the last string's null terminator
    let mut pos = string_start;
    let mut strings_counted = 0u32;

    // Count exactly the expected number of strings
    while pos < data.len() {
        let start = pos;

        // Find null terminator
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }

        // Count this string (even if empty)
        strings_counted += 1;

        // Skip the null terminator
        if pos < data.len() {
            pos += 1;
        } else {
            break;
        }

        // Stop when we've counted enough strings
        if let Some(expected) = expected_string_count {
            if strings_counted >= expected {
                eprintln!("Binary section at 0x{:x} after {} strings", pos, strings_counted);
                return Some(pos);
            }
        }
    }

    eprintln!("End of strings at 0x{:x} after {} strings (no limit)", pos, strings_counted);
    Some(pos)
}

