//! Entry parsing and record creation for NCS documents
//!
//! Handles parsing of NCS entries from string tables into structured records.

use std::collections::HashMap;

use crate::types::{Record, StringTable, Value};

use super::differential::{decode_differential_name, expand_abbreviations, split_packed_value};

/// Parse entries-based format (abjx, abij, abhj, abpe)
pub fn parse_entries_format(
    _data: &[u8],
    field_count: u8,
    type_name: &str,
    strings: &StringTable,
    _has_dep_entries: bool,
) -> Vec<Record> {
    // Use field count from header to properly group strings
    // Each entry consists of: [name] [field_1] [field_2] ... [field_n]
    let schema = get_schema(type_name);

    // For types with complex string packing, use entry-name-based grouping
    if type_name == "itempoollist" || type_name == "itempool" {
        return parse_by_entry_names(strings, &schema);
    }

    parse_by_field_count(strings, field_count, &schema)
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
pub struct TypeSchema {
    pub field_names: Vec<&'static str>,
}

pub fn get_schema(type_name: &str) -> TypeSchema {
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
pub fn parse_strings_as_records(strings: &StringTable) -> Vec<Record> {
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

pub fn is_entry_identifier(s: &str) -> bool {
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

pub fn is_metadata(s: &str) -> bool {
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

pub fn parse_string_value(s: &str) -> Option<Value> {
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
