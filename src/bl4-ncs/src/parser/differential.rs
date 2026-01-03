//! Differential name decoding for NCS entry names
//!
//! NCS uses differential encoding where subsequent entry names encode
//! only changed portions relative to the previous name.

use crate::types::Value;

/// Expand known abbreviations in entry names
/// e.g., "ID_A_" -> "ID_Achievement_"
pub fn expand_abbreviations(s: &str) -> String {
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
pub fn decode_differential_name(encoded: &str, base: &str) -> String {
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
pub fn find_first_numeric_segment(s: &str) -> Option<(usize, usize)> {
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
pub fn find_last_numeric_segment(s: &str) -> Option<(usize, usize)> {
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
pub fn split_packed_value(s: &str) -> Option<(&str, &str)> {
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
pub fn try_parse_packed_value(s: &str, _field_index: usize, _field_count: u8) -> Option<Value> {
    if let Some((value_part, _name_part)) = split_packed_value(s) {
        if let Ok(n) = value_part.parse::<i64>() {
            return Some(Value::Integer(n));
        }
    }
    None
}
