//! NCS content parser for structured JSON output
//!
//! Parses decompressed NCS content into structured data that can be
//! serialized to JSON.

use std::collections::HashMap;

use crate::bit_reader::{bit_width, BitReader};
use crate::string_table::parse_string_table;
use crate::types::{
    BinaryParseResult, Document, EntryGroup, FieldInfo, Header, Record, StringTable,
    UnpackedString, UnpackedValue, Value,
};

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
    let format_code = basic.format_code;

    // Entry section starts after format code (4 bytes)
    let entry_section_offset = format_offset + 4;

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
    let binary_offset = find_binary_section(data, string_table_offset)?;

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
fn parse_entry_section(data: &[u8], offset: usize) -> (u8, Option<u32>) {
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
fn find_string_table_start(data: &[u8], after: usize) -> Option<usize> {
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
fn find_control_section(data: &[u8], after: usize) -> Option<usize> {
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

/// Find where binary section begins (after string table) using SIMD-accelerated search
fn find_binary_section(data: &[u8], string_start: usize) -> Option<usize> {
    use memchr::memmem;

    if string_start >= data.len() {
        return Some(data.len());
    }

    // Look for the 0x7a section divider pattern: 7a 00 00 00 00 00
    // This marks the end of the tags section and start of binary data
    let divider = &[0x7a, 0x00, 0x00, 0x00, 0x00, 0x00];
    let finder = memmem::Finder::new(divider);

    if let Some(pos) = finder.find(&data[string_start..]) {
        // Binary section starts right after the 6-byte divider
        return Some(string_start + pos + 6);
    }

    // Fallback: scan through strings until we find non-printable pattern
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
                return Some(pos - consecutive_non_printable);
            }
        } else {
            consecutive_non_printable = 0;
        }
        pos += 1;
    }

    Some(data.len())
}

/// Unpack a potentially packed NCS string into its component values.
///
/// NCS uses aggressive value packing where multiple values are concatenated:
/// - "1airship" -> [Integer(1), String("airship")]
/// - "0.175128Session" -> [Float(0.175128), String("Session")]
/// - "5true" -> [Integer(5), Boolean(true)]
/// - "simple" -> [String("simple")] (not packed)
pub fn unpack_string(s: &str) -> UnpackedString {
    let original = s.to_string();

    // Empty string
    if s.is_empty() {
        return UnpackedString {
            original,
            values: vec![],
            was_packed: false,
        };
    }

    // Pure numeric string (integer)
    if s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = s.parse::<i64>() {
            return UnpackedString {
                original,
                values: vec![UnpackedValue::Integer(n)],
                was_packed: false,
            };
        }
    }

    // Pure float string
    if s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && s.contains('.') {
        if let Ok(f) = s.parse::<f64>() {
            return UnpackedString {
                original,
                values: vec![UnpackedValue::Float(f)],
                was_packed: false,
            };
        }
    }

    // Check for packed patterns
    let mut values = Vec::new();
    let mut remaining = s;

    // Pattern 1: Float prefix (e.g., "0.175128Session")
    if let Some(float_end) = find_float_end(remaining) {
        if float_end < remaining.len() {
            let float_str = &remaining[..float_end];
            if let Ok(f) = float_str.parse::<f64>() {
                values.push(UnpackedValue::Float(f));
                remaining = &remaining[float_end..];
            }
        }
    }

    // Pattern 2: Integer prefix (e.g., "1airship", "5true")
    if values.is_empty() {
        if let Some(int_end) = find_integer_end(remaining) {
            if int_end < remaining.len() {
                let int_str = &remaining[..int_end];
                if let Ok(n) = int_str.parse::<i64>() {
                    values.push(UnpackedValue::Integer(n));
                    remaining = &remaining[int_end..];
                }
            }
        }
    }

    // Check for boolean suffix
    if remaining.eq_ignore_ascii_case("true") {
        values.push(UnpackedValue::Boolean(true));
        remaining = "";
    } else if remaining.eq_ignore_ascii_case("false") {
        values.push(UnpackedValue::Boolean(false));
        remaining = "";
    }

    // Remaining string (if any)
    if !remaining.is_empty() {
        values.push(UnpackedValue::String(remaining.to_string()));
    }

    // If we only got one value and it's a string equal to original, not packed
    let was_packed = values.len() > 1
        || (values.len() == 1
            && !matches!(&values[0], UnpackedValue::String(s) if s == &original));

    // If nothing was unpacked, treat as plain string
    if values.is_empty() {
        values.push(UnpackedValue::String(original.clone()));
    }

    UnpackedString {
        original,
        values,
        was_packed,
    }
}

/// Find the end position of a float at the start of a string
fn find_float_end(s: &str) -> Option<usize> {
    let mut chars = s.chars().peekable();
    let mut pos = 0;
    let mut has_dot = false;
    let mut has_digit = false;

    // Optional leading minus
    if chars.peek() == Some(&'-') {
        chars.next();
        pos += 1;
    }

    // Digits before decimal
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            has_digit = true;
            chars.next();
            pos += 1;
        } else {
            break;
        }
    }

    // Decimal point
    if chars.peek() == Some(&'.') {
        has_dot = true;
        chars.next();
        pos += 1;

        // Digits after decimal
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                has_digit = true;
                chars.next();
                pos += 1;
            } else {
                break;
            }
        }
    }

    if has_dot && has_digit && pos > 0 {
        Some(pos)
    } else {
        None
    }
}

/// Find the end position of an integer at the start of a string
fn find_integer_end(s: &str) -> Option<usize> {
    let mut pos = 0;

    for c in s.chars() {
        if c.is_ascii_digit() {
            pos += 1;
        } else {
            break;
        }
    }

    if pos > 0 {
        Some(pos)
    } else {
        None
    }
}

/// Batch unpack multiple strings, returning only those that were packed
pub fn find_packed_strings(strings: &[String]) -> Vec<UnpackedString> {
    strings
        .iter()
        .map(|s| unpack_string(s))
        .filter(|u| u.was_packed)
        .collect()
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
fn parse_abqr(_data: &[u8], _header: &Header, strings: &StringTable) -> Vec<Record> {
    // abqr has offset tables at the start - different structure
    // For now, extract what we can from strings
    parse_strings_as_records(strings)
}

/// Generic fallback parser
fn parse_generic(_data: &[u8], _header: &Header, strings: &StringTable) -> Vec<Record> {
    parse_strings_as_records(strings)
}

/// Parse entries-based format (abjx, abij, abhj, abpe)
fn parse_entries_format(
    _data: &[u8],
    header: &Header,
    strings: &StringTable,
    _has_dep_entries: bool,
) -> Vec<Record> {
    // Use field count from header to properly group strings
    // Each entry consists of: [name] [field_1] [field_2] ... [field_n]
    let schema = get_schema(&header.type_name);

    // For types with complex string packing, use entry-name-based grouping
    if header.type_name == "itempoollist" || header.type_name == "itempool" {
        return parse_by_entry_names(strings, &schema);
    }

    parse_by_field_count(strings, header.field_count, &schema)
}

/// Parse strings by detecting entry name patterns
///
/// For files with complex string packing, we can't rely on field count.
/// Instead, detect which strings are entry names (start with IPL_, Preset_, etc.)
/// and group values between them.
fn parse_by_entry_names(strings: &StringTable, schema: &TypeSchema) -> Vec<Record> {
    let mut records = Vec::new();
    let mut current_entry: Option<(String, Vec<String>)> = None;

    for s in &strings.strings {
        if is_itempool_entry_name(s) {
            // Save previous entry
            if let Some((name, values)) = current_entry.take() {
                records.push(create_record_from_values(name, values, schema));
            }
            // Start new entry
            current_entry = Some((s.clone(), Vec::new()));
        } else if let Some((_, ref mut values)) = current_entry {
            // Add as field value (skip metadata)
            if !is_metadata(s) {
                values.push(s.clone());
            }
        }
    }

    // Save final entry
    if let Some((name, values)) = current_entry {
        records.push(create_record_from_values(name, values, schema));
    }

    records
}

/// Check if a string looks like an itempool entry name
fn is_itempool_entry_name(s: &str) -> bool {
    // Entry names have specific patterns:
    // - IPL_Something (item pool list)
    // - Preset_Something
    // - Table_Something
    // - Script/Game paths

    // IPL must be followed by underscore for entry names
    // "IPLLootable..." is a reference, not an entry name
    if s.starts_with("IPL_") {
        // Must have content after IPL_
        return s.len() > 4;
    }

    // Other prefixes
    if s.starts_with("Preset_") || s.starts_with("Table_") {
        return true;
    }

    // Script/Game paths are typically references, not entry names
    // Only treat as entry if it looks like a pool definition
    if s.starts_with("/Script/") || s.starts_with("/Game/") {
        return s.contains("Pool") || s.contains("Loot");
    }

    false
}

/// Create a record from a name and list of values
fn create_record_from_values(name: String, values: Vec<String>, schema: &TypeSchema) -> Record {
    let mut fields = HashMap::new();

    for (i, value) in values.iter().enumerate() {
        let field_name = if i < schema.field_names.len() {
            schema.field_names[i].to_string()
        } else {
            format!("field_{}", i)
        };

        let parsed_value = parse_string_value(value).unwrap_or(Value::String(value.clone()));
        fields.insert(field_name, parsed_value);
    }

    Record {
        name,
        fields,
        dep_entries: Vec::new(),
    }
}

/// Parse strings into entries using the field count from header
///
/// The field_count represents total columns per entry INCLUDING the name.
/// So field_count=2 means: [name, value1], not [name, value1, value2]
///
/// Handles packed values where a field string contains both a value and the next entry's name.
fn parse_by_field_count(
    strings: &StringTable,
    field_count: u8,
    schema: &TypeSchema,
) -> Vec<Record> {
    let mut records = Vec::new();
    // field_count is total columns including name
    let strings_per_entry = field_count.max(1) as usize;

    // Filter out metadata strings first
    let valid_strings: Vec<&str> = strings
        .strings
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !is_metadata(s))
        .collect();

    // Track base name for differential decoding
    let mut base_name: Option<String> = None;
    // Track if the next entry's name was embedded in a packed value
    let mut pending_name_diff: Option<String> = None;

    // Group strings into entries
    let mut i = 0;
    while i < valid_strings.len() {
        // Get the entry name - either from pending packed value or from current string
        let raw_name = if let Some(ref pending) = pending_name_diff {
            pending.as_str()
        } else {
            valid_strings[i]
        };

        // Apply differential decoding to get full entry name
        let name = if let Some(ref base) = base_name {
            decode_differential_name(raw_name, base)
        } else {
            // First entry - expand abbreviations like ID_A_ -> ID_Achievement_
            expand_abbreviations(raw_name)
        };

        // Update base name for next entry
        base_name = Some(name.clone());

        // Clear pending name after use
        let used_pending = pending_name_diff.is_some();
        pending_name_diff = None;

        // Calculate where field values start
        let field_start = if used_pending { i } else { i + 1 };

        // Extract field values (field_count - 1 values after the name)
        let mut fields = HashMap::new();
        let value_count = (field_count as usize).saturating_sub(1);

        for j in 0..value_count {
            let value_idx = field_start + j;
            if value_idx >= valid_strings.len() {
                break;
            }
            let value_str = valid_strings[value_idx];

            // Check for packed values (contains next entry name)
            let value = if let Some((value_part, name_part)) = split_packed_value(value_str) {
                // Store the embedded name for the next entry
                pending_name_diff = Some(name_part.to_string());
                // Parse just the value portion
                if let Ok(n) = value_part.parse::<i64>() {
                    Value::Integer(n)
                } else {
                    Value::String(value_part.to_string())
                }
            } else if let Some(val) = parse_string_value(value_str) {
                val
            } else {
                Value::String(value_str.to_string())
            };

            let field_name = if j < schema.field_names.len() {
                schema.field_names[j].to_string()
            } else {
                format!("field_{}", j)
            };

            fields.insert(field_name, value);
        }

        // Skip if this looks like garbage data
        if is_garbage_entry(&name) {
            break;
        }

        records.push(Record {
            name,
            fields,
            dep_entries: Vec::new(),
        });

        // Advance index
        if used_pending {
            // We used a pending name, so only consumed the value strings
            i = field_start + value_count;
        } else {
            // Normal case: consumed name + value strings
            i += strings_per_entry;
        }
    }

    records
}

/// Schema definition for NCS types
struct TypeSchema {
    field_names: Vec<&'static str>,
}

fn get_schema(type_name: &str) -> TypeSchema {
    // Field names are for the value fields AFTER the entry name
    // Schema: [entry_name] [field_0] [field_1] ... [field_n-1]
    match type_name {
        "achievement" => TypeSchema {
            field_names: vec!["achievementid"],
        },
        "itempool" | "itempoollist" => TypeSchema {
            field_names: vec!["weight", "pool"],
        },
        "rarity" => TypeSchema {
            field_names: vec!["weight", "color"],
        },
        "manufacturer" => TypeSchema {
            field_names: vec!["alias", "id"],
        },
        "aim_assist_parameters" => TypeSchema {
            field_names: vec!["value", "min", "max"],
        },
        "preferredparts" => TypeSchema {
            field_names: vec!["weight", "category"],
        },
        "loot_config" => TypeSchema {
            field_names: vec!["weight", "pool", "conditions"],
        },
        _ => TypeSchema {
            field_names: vec![],
        },
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

/// Check if an entry name looks like garbage data (binary interpreted as text)
fn is_garbage_entry(name: &str) -> bool {
    // Too short to be a valid entry name
    if name.len() < 3 {
        return true;
    }

    // Contains non-ASCII characters
    if name.chars().any(|c| !c.is_ascii()) {
        return true;
    }

    // Starts with non-identifier character (not letter, underscore, or /)
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' && first != '/' {
        return true;
    }

    // Contains problematic special characters that wouldn't be in valid names
    if name.contains('&') || name.contains(',') || name.contains('!') ||
       name.contains('@') || name.contains('#') || name.contains('%') ||
       name.contains('(') || name.contains(')') || name.contains('"') {
        return true;
    }

    false
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

/// Expand known abbreviations in entry names
/// e.g., "ID_A_" -> "ID_Achievement_"
fn expand_abbreviations(s: &str) -> String {
    let mut result = s.to_string();

    // Common abbreviations in Borderlands NCS files
    let expansions = [
        ("ID_A_", "ID_Achievement_"),
        ("ID_M_", "ID_Manufacturer_"),
        ("ID_W_", "ID_Weapon_"),
        ("ID_I_", "ID_Item_"),
        ("ID_P_", "ID_Part_"),
        ("ID_R_", "ID_Rarity_"),
    ];

    for (abbrev, full) in &expansions {
        if result.starts_with(abbrev) {
            result = format!("{}{}", full, &result[abbrev.len()..]);
            break;
        }
    }

    result
}

/// Decode differential name encoding
///
/// Subsequent entry names encode only changed portions relative to the previous name.
/// The encoding works by:
/// 1. Leading digit(s) replace the last N digits of the number segment
/// 2. Remaining text replaces the last segment (suffix after final underscore)
///
/// Examples (from achievement):
/// - Base: "ID_Achievement_10_worldevents_colosseum"
/// - "1airship" -> Number becomes "11", suffix becomes "airship"
///   -> "ID_Achievement_11_worldevents_airship"
/// - "2meteor" -> Number becomes "12", suffix becomes "meteor"
///   -> "ID_Achievement_12_worldevents_meteor"
/// - "24_missions_side" -> Number becomes "24", suffix becomes "missions_side"
///   -> "ID_Achievement_24_missions_side"
fn decode_differential_name(encoded: &str, base: &str) -> String {
    if encoded.is_empty() {
        return base.to_string();
    }

    // Check if this looks like a full name (starts with common prefixes)
    if encoded.starts_with("ID_")
        || encoded.starts_with("/Script/")
        || encoded.starts_with("/Game/")
        || encoded.contains("_def")
    {
        return expand_abbreviations(encoded);
    }

    // Count leading digits
    let digit_count = encoded.chars().take_while(|c| c.is_ascii_digit()).count();

    if digit_count == 0 {
        // No leading digits - this might be a full identifier
        // Check if it looks like an ID pattern
        if encoded.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
            return expand_abbreviations(encoded);
        }
        // Otherwise treat as suffix replacement only
        if let Some(last_underscore) = base.rfind('_') {
            return format!("{}{}", &base[..=last_underscore], encoded);
        }
        return format!("{}_{}", base, encoded);
    }

    // Extract the digit prefix and new suffix
    let new_digits = &encoded[..digit_count];
    let new_suffix = encoded[digit_count..].trim_start_matches('_');

    // Find the numeric segment in the base
    if let Some((num_start, num_end)) = find_first_numeric_segment(base) {
        let base_num = &base[num_start..num_end];

        // Replace last N digits of base_num with new_digits
        let keep_len = base_num.len().saturating_sub(digit_count);
        let new_num = format!("{}{}", &base_num[..keep_len], new_digits);

        // Find where the suffix segment starts (after the number)
        let after_num = &base[num_end..];

        // Determine how much of the suffix to replace based on the new suffix
        // If new_suffix contains underscore, it's a complete new path - replace everything
        // If new_suffix is just letters, replace only the final segment
        if new_suffix.contains('_') {
            // New suffix has structure (e.g., "missions_side") - replace entire suffix
            format!("{}{}_{}", &base[..num_start], new_num, new_suffix)
        } else if let Some(second_underscore) = after_num.strip_prefix('_').and_then(|s| s.find('_')) {
            // Keep the middle segment (e.g., "worldevents"), replace final segment only
            let middle = &after_num[1..second_underscore + 1]; // "worldevents"
            format!("{}{}_{}_{}", &base[..num_start], new_num, middle, new_suffix)
        } else if after_num.starts_with('_') && !new_suffix.is_empty() {
            // Only one segment after number, replace it entirely
            format!("{}{}_{}", &base[..num_start], new_num, new_suffix)
        } else if !new_suffix.is_empty() {
            format!("{}{}_{}", &base[..num_start], new_num, new_suffix)
        } else {
            format!("{}{}", &base[..num_start], new_num)
        }
    } else {
        // No numeric segment found, append as-is
        format!("{}{}", base, encoded)
    }
}

/// Find the first numeric segment in a string
/// Returns (start_index, end_index) of the numeric segment
fn find_first_numeric_segment(s: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut end = None;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
            end = Some(i + c.len_utf8());
        } else if start.is_some() {
            // End of first numeric segment
            break;
        }
    }

    match (start, end) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    }
}

/// Find the last numeric segment in a string
/// Returns (start_index, end_index) of the numeric segment
#[allow(dead_code)]
fn find_last_numeric_segment(s: &str) -> Option<(usize, usize)> {
    let chars: Vec<char> = s.chars().collect();
    let mut end = None;
    let mut start = None;

    for i in (0..chars.len()).rev() {
        if chars[i].is_ascii_digit() {
            if end.is_none() {
                end = Some(i + 1);
            }
            start = Some(i);
        } else if end.is_some() {
            // We found the end of the last numeric segment
            break;
        }
    }

    match (start, end) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    }
}

/// Check if a string is a packed value (contains both a field value and next entry name)
/// Returns (value_part, name_part) if packed, None otherwise
/// e.g., "1224_missions_side" = ("12", "24_missions_side")
fn split_packed_value(s: &str) -> Option<(&str, &str)> {
    // Packed values have the pattern: <value_digits><differential_name>
    // where differential_name starts with digits followed by underscore or letters
    //
    // Heuristic: Look for a split point where:
    // - First part is 1-3 digits (typical ID length)
    // - Second part starts with 1-2 digits and has underscore or letters
    // - Prefer 2-digit value_part (most common ID length in game data)
    let digit_count = s.chars().take_while(|c| c.is_ascii_digit()).count();

    if digit_count < 2 {
        return None; // Need at least 2 digits for it to be packed
    }

    // Collect all valid splits and choose the best one
    let mut valid_splits: Vec<(usize, &str, &str)> = Vec::new();

    for split_pos in 1..digit_count.min(4) {
        let value_part = &s[..split_pos];
        let name_part = &s[split_pos..];

        // Check if name_part looks like a differential name
        // (starts with 1-2 digits and has underscore or letters after)
        let name_digit_count = name_part.chars().take_while(|c| c.is_ascii_digit()).count();

        // Name part should have 1-2 leading digits (typical diff encoding)
        if name_digit_count >= 1 && name_digit_count <= 2 && name_digit_count < name_part.len() {
            let after_digits = &name_part[name_digit_count..];
            if after_digits.starts_with('_') || after_digits.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
                valid_splits.push((split_pos, value_part, name_part));
            }
        }
    }

    if valid_splits.is_empty() {
        return None;
    }

    // Prefer 2-digit value_part if available (most common ID pattern)
    // Otherwise prefer shorter name_part digit prefix
    valid_splits
        .into_iter()
        .min_by_key(|(split_pos, _, name_part)| {
            let name_digits = name_part.chars().take_while(|c| c.is_ascii_digit()).count();
            // Score: prefer split_pos=2, then by fewer name digits
            let pos_score = if *split_pos == 2 { 0 } else { 10 };
            pos_score + name_digits
        })
        .map(|(_, v, n)| (v, n))
}

/// Try to parse a packed value string, returning just the value portion
#[allow(dead_code)]
fn try_parse_packed_value(s: &str, _field_index: usize, _field_count: u8) -> Option<Value> {
    if let Some((value_part, _name_part)) = split_packed_value(s) {
        if let Ok(n) = value_part.parse::<i64>() {
            return Some(Value::Integer(n));
        }
    }
    None
}

// ============================================================================
// Binary Section Parsing
// ============================================================================

/// Parse the binary section of an NCS file
///
/// The binary section has two main parts:
/// 1. Bit-packed string indices (first ~32 bytes, variable)
/// 2. Structured metadata section (byte values separated by 0x28 or 0x20)
///
/// The structured metadata creates entry groups that correspond to entries
/// in the string table.
pub fn parse_binary_section(
    data: &[u8],
    binary_offset: usize,
    strings: &StringTable,
) -> Option<BinaryParseResult> {
    if binary_offset >= data.len() {
        return None;
    }

    let binary_data = &data[binary_offset..];

    // Calculate bit width for string table lookup
    let string_bits = bit_width(strings.len() as u32);

    // Find the structured metadata section by looking for byte values
    // in the 0x08-0x28 range followed by 0x28/0x20 separators
    let structured_start = find_structured_section_start(binary_data);

    // Part 1: Read bit-packed indices from the first section
    let bit_section = &binary_data[..structured_start.min(binary_data.len())];
    let bit_indices = read_bit_packed_indices(bit_section, string_bits);

    // Get table_id (first bit-packed value)
    let table_id = bit_indices.first().copied().unwrap_or(0);

    // Part 2: Parse structured metadata section
    let (entry_groups, tail_start) = if structured_start < binary_data.len() {
        parse_structured_section(&binary_data[structured_start..])
    } else {
        (Vec::new(), 0)
    };

    // Part 3: Extract tail data
    let tail_offset = structured_start + tail_start;
    let tail_data = if tail_offset < binary_data.len() {
        binary_data[tail_offset..].to_vec()
    } else {
        Vec::new()
    };

    Some(BinaryParseResult {
        table_id,
        bit_indices,
        entry_groups,
        tail_data,
    })
}

/// Find where the structured metadata section starts
///
/// The bit-packed section typically ends when we see a pattern of
/// byte values in the 0x08-0x30 range (field metadata), followed
/// by a separator (0x28 or 0x20) and terminator (0x00 0x00).
///
/// Two format variations:
/// 1. Separator format: values 0x08-0x40 with 0x28/0x20 separators
/// 2. Compact format: 0x80 0x80 header followed by fixed-width records
fn find_structured_section_start(data: &[u8]) -> usize {
    // First, check for 0x28 separator - if present, use separator format
    let has_separator = data.iter().any(|&b| b == 0x28);

    if has_separator {
        // Use separator format detection (skip compact format check)
    } else {
        // Check for compact format (0x80 0x80 header)
        // This pattern appears when there are no 0x28 separators
        for i in 16..data.len().saturating_sub(4) {
            if data[i] == 0x80 && data[i + 1] == 0x80 {
                // Verify there's a 00 00 terminator ahead
                let has_terminator = data[i + 2..]
                    .windows(2)
                    .take(48)
                    .any(|w| w == [0x00, 0x00]);

                if has_terminator {
                    return i;
                }
            }
        }
    }

    // Strategy: Find the first 0x28 separator and walk back to find
    // the start of the structured section.
    //
    // The structured section has:
    // - Values mostly in 0x08-0x30 range
    // - 0x28 or 0x20 separators between groups
    // - 0x00 0x00 terminator

    // First, find the position of the first 0x28 separator
    let first_sep = data.iter().position(|&b| b == 0x28);

    if let Some(sep_pos) = first_sep {
        // Walk backwards from separator to find start of first group
        // The structured section values are typically >= 0x08 and <= 0x40
        let mut start = sep_pos;

        while start > 0 {
            let prev = data[start - 1];
            // Valid structured values are in range 0x08-0x40
            // Skip high bytes like 0x80, 0xff which are bit-packed data
            if prev >= 0x08 && prev <= 0x40 {
                start -= 1;
            } else {
                break;
            }
        }

        // Verify this is a reasonable start
        if start < sep_pos && start >= 16 {
            return start;
        }
    }

    // Alternative: look for a clean transition pattern
    // Bit-packed data often has high bytes (0x80+), structured has low bytes
    for i in 24..data.len().saturating_sub(16) {
        // Check for transition: high byte(s) followed by low byte run
        let has_high_before = i > 0 && (data[i - 1] >= 0x80 || data[i - 1] < 0x08);
        let has_low_run = data[i..].iter().take(8).all(|&b| b <= 0x40);
        let has_separator = data[i..].iter().take(16).any(|&b| b == 0x28 || b == 0x20);
        let has_terminator = data[i..]
            .windows(2)
            .take(48)
            .any(|w| w == [0x00, 0x00]);

        if has_high_before && has_low_run && has_separator && has_terminator {
            return i;
        }
    }

    // If no structured section found, assume it's all bit-packed
    data.len()
}

/// Read bit-packed string indices from the first section
fn read_bit_packed_indices(data: &[u8], bit_width: u8) -> Vec<u32> {
    let mut indices = Vec::new();
    let mut reader = BitReader::new(data);

    // Read indices until we run out of data
    while reader.has_bits(bit_width as usize) {
        if let Some(idx) = reader.read_bits(bit_width) {
            indices.push(idx);
        } else {
            break;
        }
    }

    indices
}

/// Parse the structured metadata section into entry groups
///
/// Returns (entry_groups, tail_start_offset)
///
/// Two formats are supported:
/// 1. Separator format: Groups separated by 0x28 or 0x20, ending with 00 00
/// 2. Compact format: 0x80 0x80 header followed by fixed-width records
fn parse_structured_section(data: &[u8]) -> (Vec<EntryGroup>, usize) {
    // Check for compact format (0x80 0x80 header)
    if data.len() >= 4 && data[0] == 0x80 && data[1] == 0x80 {
        return parse_compact_structured_section(data);
    }

    // Standard separator format
    let mut groups = Vec::new();
    let mut current_values = Vec::new();
    let mut tail_start = data.len();

    for (i, &byte) in data.iter().enumerate() {
        // Check for terminator (00 00)
        if byte == 0x00 {
            if i + 1 < data.len() && data[i + 1] == 0x00 {
                // Save current group if any
                if !current_values.is_empty() {
                    groups.push(create_entry_group(current_values.clone()));
                }
                tail_start = i + 2; // Skip past 00 00
                break;
            }
            continue;
        }

        // Check for separator (0x28 or 0x20)
        if byte == 0x28 || byte == 0x20 {
            if !current_values.is_empty() {
                groups.push(create_entry_group(current_values.clone()));
                current_values.clear();
            }
            continue;
        }

        // Add byte to current group
        current_values.push(byte);
    }

    // Handle any remaining values
    if !current_values.is_empty() {
        groups.push(create_entry_group(current_values));
    }

    (groups, tail_start)
}

/// Parse compact structured section (0x80 0x80 header format)
///
/// This format is used by files like rarity that don't have separators.
/// Structure: 0x80 0x80 [byte pairs for each entry] 00 00
fn parse_compact_structured_section(data: &[u8]) -> (Vec<EntryGroup>, usize) {
    let mut groups = Vec::new();

    // Skip the 0x80 0x80 header
    let start = 2;
    let mut tail_start = data.len();

    // Find terminator (00 00)
    let end = data[start..]
        .windows(2)
        .position(|w| w == [0x00, 0x00])
        .map(|pos| start + pos)
        .unwrap_or(data.len());

    if end > start {
        let payload = &data[start..end];

        // Try to determine record width
        // Common patterns: 2 bytes per entry
        let record_width = 2;

        for chunk in payload.chunks(record_width) {
            groups.push(create_entry_group(chunk.to_vec()));
        }

        tail_start = end + 2; // Skip past 00 00
    }

    (groups, tail_start)
}

/// Create an entry group from raw byte values
fn create_entry_group(values: Vec<u8>) -> EntryGroup {
    // Interpret values as field metadata
    // Each value might represent bit offset, bit width, or position
    let mut field_info = Vec::new();
    let mut bit_offset = 0u32;

    for &val in &values {
        // Hypothesis: values represent cumulative bit offsets or widths
        field_info.push(FieldInfo {
            bit_offset,
            bit_width: val,
            string_index: None,
        });
        bit_offset += val as u32;
    }

    EntryGroup { values, field_info }
}

/// Debug function to dump binary section info
pub fn debug_binary_section(data: &[u8], binary_offset: usize) -> String {
    let mut output = String::new();
    use std::fmt::Write;

    if binary_offset >= data.len() {
        return "Binary offset out of bounds".to_string();
    }

    let binary_data = &data[binary_offset..];
    let _ = writeln!(output, "Binary section at 0x{:x}, {} bytes", binary_offset, binary_data.len());

    // Show first 64 bytes as hex
    let preview_len = binary_data.len().min(64);
    let _ = writeln!(output, "First {} bytes:", preview_len);
    for (i, chunk) in binary_data[..preview_len].chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let _ = writeln!(output, "  {:04x}: {}", i * 16, hex.join(" "));
    }

    // Try to read as bits
    let mut reader = BitReader::new(binary_data);
    let _ = writeln!(output, "\nBit reading test:");

    // Try reading first few values
    if let Some(v) = reader.read_bits(8) {
        let _ = writeln!(output, "  First 8 bits: {} (0x{:02x})", v, v);
    }
    if let Some(v) = reader.read_bits(8) {
        let _ = writeln!(output, "  Next 8 bits: {} (0x{:02x})", v, v);
    }
    if let Some(v) = reader.read_bits(16) {
        let _ = writeln!(output, "  Next 16 bits: {} (0x{:04x})", v, v);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_unpack_string_simple() {
        // Pure integer
        let result = unpack_string("123");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::Integer(123)]);

        // Pure float
        let result = unpack_string("1.5");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::Float(1.5)]);

        // Pure string
        let result = unpack_string("hello");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::String("hello".into())]);
    }

    #[test]
    fn test_unpack_string_packed_int_string() {
        // Integer + string (e.g., "1airship")
        let result = unpack_string("1airship");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Integer(1));
        assert_eq!(result.values[1], UnpackedValue::String("airship".into()));

        // Multiple digits + string
        let result = unpack_string("12ships");
        assert!(result.was_packed);
        assert_eq!(result.values[0], UnpackedValue::Integer(12));
        assert_eq!(result.values[1], UnpackedValue::String("ships".into()));
    }

    #[test]
    fn test_unpack_string_packed_float_string() {
        // Float + string (e.g., "0.175128Session")
        let result = unpack_string("0.175128Session");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Float(0.175128));
        assert_eq!(result.values[1], UnpackedValue::String("Session".into()));
    }

    #[test]
    fn test_unpack_string_packed_int_bool() {
        // Integer + boolean (e.g., "5true")
        let result = unpack_string("5true");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Integer(5));
        assert_eq!(result.values[1], UnpackedValue::Boolean(true));

        let result = unpack_string("0false");
        assert!(result.was_packed);
        assert_eq!(result.values[0], UnpackedValue::Integer(0));
        assert_eq!(result.values[1], UnpackedValue::Boolean(false));
    }

    #[test]
    fn test_find_packed_strings() {
        let strings = vec![
            "hello".to_string(),
            "123".to_string(),
            "1airship".to_string(),
            "0.5test".to_string(),
            "world".to_string(),
        ];
        let packed = find_packed_strings(&strings);
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0].original, "1airship");
        assert_eq!(packed[1].original, "0.5test");
    }
}
