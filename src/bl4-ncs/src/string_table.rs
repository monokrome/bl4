//! String table parsing for NCS content
//!
//! Provides functions for extracting and processing string tables from NCS binary data.

use std::collections::HashMap;

use crate::types::{Header, StringTable};

/// Parse string table from NCS content
pub fn parse_string_table(data: &[u8], header: &Header) -> StringTable {
    // Calculate bounds for string table
    let end = header
        .control_section_offset
        .unwrap_or(header.binary_offset)
        .min(data.len())
        .max(header.string_table_offset);

    let max_strings = header.string_count.map(|c| c as usize);

    // Extract raw strings
    let raw_strings = extract_raw_strings(&data[header.string_table_offset..end], max_strings);

    // Build final string table with packed string splitting
    build_string_table(raw_strings)
}

/// Extract null-terminated strings from raw bytes
fn extract_raw_strings(data: &[u8], max_count: Option<usize>) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    let mut in_string = false;

    for &byte in data {
        if let Some(max) = max_count {
            if strings.len() >= max {
                break;
            }
        }

        match byte {
            0 => {
                if !current.is_empty() {
                    if let Some(s) = try_extract_string(&current) {
                        strings.push(s);
                    }
                    current.clear();
                }
                in_string = false;
            }
            b if b.is_ascii_graphic() || b == b' ' => {
                current.push(b);
                in_string = true;
            }
            _ if in_string && current.len() >= 2 => {
                if let Some(s) = try_extract_string(&current) {
                    strings.push(s);
                }
                current.clear();
                in_string = false;
            }
            _ => {
                current.clear();
                in_string = false;
            }
        }
    }

    // Handle trailing string
    if !current.is_empty() {
        let at_limit = max_count.map(|m| strings.len() >= m).unwrap_or(false);
        if !at_limit {
            if let Some(s) = try_extract_string(&current) {
                strings.push(s);
            }
        }
    }

    strings
}

/// Try to extract a valid string from bytes
#[inline]
fn try_extract_string(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes)
        .ok()
        .filter(|s| is_valid_string(s))
        .map(|s| s.to_string())
}

/// Build StringTable from raw strings, splitting packed strings as needed
fn build_string_table(raw_strings: Vec<String>) -> StringTable {
    let mut strings = Vec::with_capacity(raw_strings.len());
    let mut index_map = HashMap::with_capacity(raw_strings.len());

    for raw in raw_strings {
        if should_split_string(&raw) {
            for s in split_packed_string(&raw) {
                index_map.insert(s.clone(), strings.len());
                strings.push(s);
            }
        } else {
            index_map.insert(raw.clone(), strings.len());
            strings.push(raw);
        }
    }

    StringTable::with_data(strings, index_map)
}

/// Check if a string contains markers indicating it's packed
#[inline]
fn should_split_string(s: &str) -> bool {
    s.len() > 20 && (s.contains("IPL") || s.contains("/Script/") || s.contains("Table_"))
}

/// Check if a string looks valid (not garbage binary data)
fn is_valid_string(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Minimum length of 2 chars to avoid garbage single-byte strings
    if s.len() < 2 {
        return false;
    }

    // Reject strings with garbage characters (!, @, #, %, ^, &, etc.)
    if s.chars()
        .any(|c| matches!(c, '!' | '@' | '#' | '%' | '^' | '&' | '*' | '(' | ')' | '"' | '`'))
    {
        return false;
    }

    // Pure numeric strings (like "10", "24", "123") are valid
    if s.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // Must have at least some letters for non-numeric strings
    let letter_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if letter_count == 0 && !s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
        return false;
    }

    // Check for garbage indicators
    // Trailing or leading spaces are suspicious
    if s.starts_with(' ') || s.ends_with(' ') {
        return false;
    }

    // Multiple consecutive spaces indicate garbage
    if s.contains("  ") {
        return false;
    }

    // Short strings (2-3 chars) that aren't pure numbers need stricter validation
    if s.len() <= 3 && !s.chars().all(|c| c.is_ascii_digit()) {
        // Must be alphanumeric or underscore only
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
        // Short strings should be all lowercase, all uppercase, or known patterns
        // Mixed case like "zR" or "D3" is suspicious
        let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
        if has_lower && has_upper {
            return false; // Reject mixed case short strings
        }
        // Must start with a letter for short strings (avoids "D3", "2P", etc.)
        if !s.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
            return false;
        }
        // Allow known short keywords and common patterns
        let lower = s.to_lowercase();
        if !matches!(
            lower.as_str(),
            "pad" | "id" | "min" | "max" | "key" | "map" | "set" | "get" | "new" | "old"
                | "add" | "sub" | "div" | "mul" | "mod" | "int" | "str" | "vec" | "ptr"
                | "ref" | "val" | "nil" | "null" | "end" | "all" | "any" | "one" | "two"
        ) {
            // For unknown short strings, require all lowercase (common identifiers)
            if has_upper {
                return false;
            }
        }
    }

    // Backticks, control chars indicate garbage (binary interpreted as ASCII)
    if s.contains('`') || s.chars().any(|c| c.is_ascii_control()) {
        return false;
    }

    // High ratio of underscores to letters is suspicious (like "corrai_cbicaldb...")
    if letter_count > 0 {
        let underscore_count = s.chars().filter(|&c| c == '_').count();
        // Normal identifiers have roughly 1 underscore per 5-10 chars
        // Garbage might have more underscores or random patterns
        if underscore_count > letter_count / 2 && underscore_count > 3 {
            return false;
        }
    }

    true
}

/// Split a packed string that may contain multiple concatenated names
///
/// NCS files sometimes pack multiple entry names into a single null-terminated
/// string. This function attempts to split them at known boundaries.
fn split_packed_string(s: &str) -> Vec<String> {
    let mut results = Vec::new();

    // Known split patterns - these indicate a new entry name is starting
    let split_markers = [
        "IPL_",     // Item Pool List entries
        "IPL",      // IPL without underscore (can appear mid-string)
        "Table_",   // Table references
        "Preset_",  // Preset entries
        "/Script/", // Unreal script paths
        "/Game/",   // Unreal game paths
    ];

    let mut remaining = s;

    while !remaining.is_empty() {
        // Find the earliest split marker
        let mut best_split: Option<(usize, &str)> = None;

        for marker in &split_markers {
            // Look for marker starting at position 1 or later (not at the start)
            if let Some(pos) = remaining[1..].find(marker) {
                let actual_pos = pos + 1;
                if best_split.is_none() || actual_pos < best_split.unwrap().0 {
                    best_split = Some((actual_pos, marker));
                }
            }
        }

        if let Some((split_pos, _marker)) = best_split {
            // Extract the part before the marker
            let part = &remaining[..split_pos];
            if !part.is_empty() && is_valid_string(part) {
                // Try to normalize the entry name
                let normalized = normalize_entry_name(part);
                if !normalized.is_empty() {
                    results.push(normalized);
                }
            }

            // Continue with the part starting at the marker
            remaining = &remaining[split_pos..];
        } else {
            // No more split markers - add remaining as final entry
            if !remaining.is_empty() && is_valid_string(remaining) {
                let normalized = normalize_entry_name(remaining);
                if !normalized.is_empty() {
                    results.push(normalized);
                }
            }
            break;
        }
    }

    // If no splits were made, return original string
    if results.is_empty() && !s.is_empty() {
        results.push(s.to_string());
    }

    results
}

/// Normalize an entry name by adding proper prefixes
fn normalize_entry_name(s: &str) -> String {
    // If it starts with underscore and contains "IPL", it's probably missing the prefix
    if s.starts_with('_') {
        // Check if there's content after the underscore
        let without_underscore = &s[1..];
        if !without_underscore.is_empty() {
            // This is likely an entry name fragment like "_GrassBoss"
            // Return as-is for now; the caller may prepend "IPL" if appropriate
            return format!("IPL{}", s);
        }
    }

    s.to_string()
}

/// Extract category names that appear after the control section
///
/// These are DLC/content pack identifiers like "none", "base", "basegame"
/// that appear between the control section and the field abbreviations.
pub fn extract_inline_strings(data: &[u8], header: &Header, _primary_count: usize) -> Vec<String> {
    let mut category_names = Vec::new();

    // Start from category names offset (after control section)
    let start = match header.category_names_offset {
        Some(off) => off,
        None => return category_names,
    };

    // Find first XX XX 00 00 marker (entry data start) or field abbreviation pattern
    let end = {
        let mut end_pos = header.binary_offset;
        // Look for entry data markers (XX XX 00 00 where XX > 0)
        for i in start..header.binary_offset.saturating_sub(3) {
            if i + 3 < data.len()
                && data[i] != 0
                && data[i + 1] != 0
                && data[i + 2] == 0
                && data[i + 3] == 0
            {
                end_pos = i;
                break;
            }
            // Also stop at field abbreviation patterns (contains '.' or '!' or '_')
            // These encode field names compactly, not category strings
            if data[i] == b'.' || data[i] == b'!' {
                // Found field abbreviation marker, back up to start of that string
                let mut j = i;
                while j > start && data[j - 1] != 0 {
                    j -= 1;
                }
                end_pos = j;
                break;
            }
        }
        end_pos
    };

    // Scan for category strings between control section and end marker
    let mut current = Vec::new();
    for pos in start..end {
        let byte = data[pos];
        if byte == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    // Category names are simple lowercase identifiers
                    if s.len() >= 2
                        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                        && !s.contains('.')
                        && !s.contains('!')
                        && !s.contains('_')
                    {
                        category_names.push(s.to_string());
                    }
                }
                current.clear();
            }
        } else if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
        } else {
            current.clear();
        }
    }

    // Handle last string
    if !current.is_empty() {
        if let Ok(s) = std::str::from_utf8(&current) {
            if s.len() >= 2
                && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                && !s.contains('.')
                && !s.contains('!')
                && !s.contains('_')
            {
                category_names.push(s.to_string());
            }
        }
    }

    category_names
}

/// Extract field abbreviation that appears after category names
///
/// Field abbreviations like "corid_aid.a!" encode field names compactly.
/// They appear between category names and binary data markers.
/// Returns the abbreviation string (without the trailing '!') if found.
pub fn extract_field_abbreviation(data: &[u8], header: &Header) -> Option<String> {
    let start = header.category_names_offset.unwrap_or(header.binary_offset);
    let end = header.binary_offset;

    // Look for a string containing '.' or '!' (field abbreviation marker)
    let mut current = Vec::new();
    let mut found_abbrev = None;

    for pos in start..end {
        let byte = data[pos];
        if byte == 0 {
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    // Field abbreviations contain '.' or end with '!'
                    if s.contains('.') || s.ends_with('!') {
                        // Strip trailing '!' if present
                        let clean = s.trim_end_matches('!');
                        if !clean.is_empty() {
                            found_abbrev = Some(clean.to_string());
                        }
                    }
                }
                current.clear();
            }
        } else if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
        } else {
            // Check if current is a field abbreviation before clearing
            // (field abbreviation may be terminated by control byte, not null)
            if !current.is_empty() {
                if let Ok(s) = std::str::from_utf8(&current) {
                    if s.contains('.') || s.ends_with('!') {
                        let clean = s.trim_end_matches('!');
                        if !clean.is_empty() {
                            found_abbrev = Some(clean.to_string());
                        }
                    }
                }
            }
            current.clear();
        }
    }

    // Check last string if not null-terminated
    if !current.is_empty() {
        if let Ok(s) = std::str::from_utf8(&current) {
            if s.contains('.') || s.ends_with('!') {
                let clean = s.trim_end_matches('!');
                if !clean.is_empty() {
                    found_abbrev = Some(clean.to_string());
                }
            }
        }
    }

    found_abbrev
}

/// Create a combined string table with primary strings, category names, and field abbreviation
pub fn create_combined_string_table(primary: &StringTable, inline: &[String]) -> StringTable {
    let mut strings = primary.strings.clone();
    let mut index_map = primary.index_map.clone();

    for s in inline {
        if !index_map.contains_key(s) {
            index_map.insert(s.clone(), strings.len());
            strings.push(s.clone());
        }
    }

    StringTable::with_data(strings, index_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_string() {
        assert!(is_valid_string("hello"));
        assert!(is_valid_string("test_name"));
        assert!(is_valid_string("123"));
        assert!(is_valid_string("ID_Achievement_01"));

        assert!(!is_valid_string(""));
        assert!(!is_valid_string("a")); // too short
        assert!(!is_valid_string("hello!")); // contains !
        assert!(!is_valid_string(" hello")); // starts with space
    }

    #[test]
    fn test_split_packed_string() {
        let result = split_packed_string("simple");
        assert_eq!(result, vec!["simple"]);

        // IPL splitting
        let result = split_packed_string("testIPL_Something");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_normalize_entry_name() {
        assert_eq!(normalize_entry_name("_Boss"), "IPL_Boss");
        assert_eq!(normalize_entry_name("Normal"), "Normal");
    }
}
