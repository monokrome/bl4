//! Firmware manipulation for equipment items
//!
//! Firmware is an equipment modifier that can be applied to class mods,
//! enhancements, gadgets, and other non-weapon items. Each item can have
//! at most one firmware.
//!
//! Firmware encoding differs by item type:
//! - Class mods: Part { index: 234, values: [fw_idx], encoding: Single }
//! - Equipment:  VarInt appended to trailing variable section
//!
//! Both types use String("ft") at header position 8 to flag firmware presence.

use crate::manifest;
use crate::serial::{PartEncoding, Token};

/// Firmware category for class mods
const CLASS_MOD_FIRMWARE_CATEGORY: u64 = 234;

/// Firmware category for equipment (enhancements, gadgets, etc.)
const EQUIPMENT_FIRMWARE_CATEGORY: i64 = 247;

/// Detected firmware on an item
#[derive(Debug, Clone)]
pub struct DetectedFirmware {
    pub name: String,
    pub index: i64,
    pub category: i64,
}

/// Whether the token stream has the firmware flag
fn has_firmware_flag(tokens: &[Token]) -> bool {
    tokens.iter().any(|t| matches!(t, Token::String(s) if s == "ft"))
}

/// Detect firmware on an item from its token stream.
pub fn detect(tokens: &[Token], item_category: i64) -> Option<DetectedFirmware> {
    if !has_firmware_flag(tokens) {
        return None;
    }

    if crate::skills::is_class_mod(item_category) {
        detect_class_mod(tokens)
    } else {
        detect_equipment(tokens)
    }
}

/// Detect firmware on a class mod (Part with index 234)
fn detect_class_mod(tokens: &[Token]) -> Option<DetectedFirmware> {
    for token in tokens {
        if let Token::Part { index, values, .. } = token {
            if *index == CLASS_MOD_FIRMWARE_CATEGORY && !values.is_empty() {
                let fw_idx = values[0] as i64;
                let name = manifest::part_name(CLASS_MOD_FIRMWARE_CATEGORY as i64, fw_idx)
                    .unwrap_or("unknown")
                    .to_string();
                return Some(DetectedFirmware {
                    name,
                    index: fw_idx,
                    category: CLASS_MOD_FIRMWARE_CATEGORY as i64,
                });
            }
        }
    }
    None
}

/// Detect firmware on equipment (last VarInt before final Separator)
fn detect_equipment(tokens: &[Token]) -> Option<DetectedFirmware> {
    // Find the last VarInt before the final Separator
    // The firmware VarInt is the last one added to the trailing section
    let last_sep = tokens.iter().rposition(|t| matches!(t, Token::Separator))?;

    // Walk backwards from the last separator to find the last VarInt
    for i in (0..last_sep).rev() {
        match &tokens[i] {
            Token::Var { val, .. } => {
                let fw_idx = *val as i64;
                let name = manifest::part_name(EQUIPMENT_FIRMWARE_CATEGORY, fw_idx)
                    .unwrap_or("unknown")
                    .to_string();
                return Some(DetectedFirmware {
                    name,
                    index: fw_idx,
                    category: EQUIPMENT_FIRMWARE_CATEGORY,
                });
            }
            Token::Separator | Token::SoftSeparator => break,
            _ => continue,
        }
    }
    None
}

/// Resolve a firmware name to its index in the appropriate category.
pub fn resolve_firmware(name: &str, item_category: i64) -> Result<(i64, i64), String> {
    let fw_category = if crate::skills::is_class_mod(item_category) {
        CLASS_MOD_FIRMWARE_CATEGORY as i64
    } else {
        EQUIPMENT_FIRMWARE_CATEGORY
    };

    // Try exact part name match (with or without prefix)
    let with_prefix = if name.starts_with("part_firmware_") {
        name.to_string()
    } else {
        format!("part_firmware_{}", name)
    };

    if let Some(idx) = manifest::part_index(fw_category, &with_prefix) {
        return Ok((fw_category, idx));
    }

    // Try the raw name as-is
    if let Some(idx) = manifest::part_index(fw_category, name) {
        return Ok((fw_category, idx));
    }

    // Try display name from tooltips (case-insensitive)
    let lower = name.to_lowercase().replace(' ', "_");
    let with_prefix_lower = format!("part_firmware_{}", lower);
    if let Some(idx) = manifest::part_index(fw_category, &with_prefix_lower) {
        return Ok((fw_category, idx));
    }

    Err(format!("firmware '{}' not found in category {}", name, fw_category))
}

/// List all available firmware for an item category.
pub fn available_firmware(item_category: i64) -> Vec<(i64, String)> {
    let fw_category = if crate::skills::is_class_mod(item_category) {
        CLASS_MOD_FIRMWARE_CATEGORY as i64
    } else {
        EQUIPMENT_FIRMWARE_CATEGORY
    };

    let mut result = Vec::new();
    // Iterate through known firmware indices
    for idx in 1..=300 {
        if let Some(name) = manifest::part_name(fw_category, idx) {
            if name.contains("firmware") {
                result.push((idx, name.to_string()));
            }
        }
    }
    result
}

/// Apply firmware to a token stream. Handles both class mod and equipment formats.
///
/// If firmware already exists, replaces it. If not, adds it (including the
/// "ft" header flag).
pub fn apply(tokens: &[Token], fw_index: i64, item_category: i64) -> Vec<Token> {
    if crate::skills::is_class_mod(item_category) {
        apply_class_mod(tokens, fw_index)
    } else {
        apply_equipment(tokens, fw_index)
    }
}

/// Apply firmware to a class mod token stream.
fn apply_class_mod(tokens: &[Token], fw_index: i64) -> Vec<Token> {
    let mut result = tokens.to_vec();

    let already_has = has_firmware_flag(&result);

    // Find existing firmware Part (index: 234) and replace its values
    if already_has {
        for token in &mut result {
            if let Token::Part { index, values, .. } = token {
                if *index == CLASS_MOD_FIRMWARE_CATEGORY {
                    *values = vec![fw_index as u64];
                    return result;
                }
            }
        }
    }

    // No existing firmware — add flag and Part
    if !already_has {
        insert_firmware_flag(&mut result);
    }

    let fw_token = Token::Part {
        index: CLASS_MOD_FIRMWARE_CATEGORY,
        values: vec![fw_index as u64],
        encoding: PartEncoding::Single,
    };

    let insert_pos = result.iter().rposition(|t| matches!(t, Token::Separator))
        .unwrap_or(result.len());
    result.insert(insert_pos, fw_token);

    result
}

/// Apply firmware to an equipment token stream.
fn apply_equipment(tokens: &[Token], fw_index: i64) -> Vec<Token> {
    let mut result = tokens.to_vec();

    let had_firmware = has_firmware_flag(&result);

    if had_firmware {
        // Replace the last VarInt before the final Separator
        let last_sep = result.iter().rposition(|t| matches!(t, Token::Separator))
            .unwrap_or(result.len());
        for i in (0..last_sep).rev() {
            if let Token::Var { val, .. } = &mut result[i] {
                *val = fw_index as u64;
                return result;
            }
        }
    }

    // No existing firmware — add flag and VarInt
    insert_firmware_flag(&mut result);
    let last_sep = result.iter().rposition(|t| matches!(t, Token::Separator))
        .unwrap_or(result.len());
    result.insert(last_sep, Token::VarInt(fw_index as u64));

    result
}

/// Insert the "ft" firmware flag into the header, replacing the VarInt at position 8.
fn insert_firmware_flag(tokens: &mut Vec<Token>) {
    // Find position 8 (after header: category, 3x SoftSep+Var pairs, Separator)
    // Token 8 is the first token after the first Separator
    let first_sep = tokens.iter().position(|t| matches!(t, Token::Separator));
    let insert_pos = match first_sep {
        Some(pos) => pos + 1,
        None => return,
    };

    if insert_pos >= tokens.len() {
        return;
    }

    // Replace the existing token at position 8 with String("ft")
    // and insert the SoftSep + Var(1) + Separator sequence after it
    tokens[insert_pos] = Token::String("ft".to_string());
    tokens.insert(insert_pos + 1, Token::SoftSeparator);
    tokens.insert(insert_pos + 2, Token::VarInt(1));
    tokens.insert(insert_pos + 3, Token::Separator);
}

/// Remove firmware from a token stream.
pub fn remove(tokens: &[Token], item_category: i64) -> Vec<Token> {
    if !has_firmware_flag(tokens) {
        return tokens.to_vec();
    }

    let mut result = tokens.to_vec();

    if crate::skills::is_class_mod(item_category) {
        // Remove the firmware Part token
        result.retain(|t| {
            !matches!(t, Token::Part { index, .. } if *index == CLASS_MOD_FIRMWARE_CATEGORY)
        });
    } else {
        // Remove the last VarInt before the final Separator
        let last_sep = result.iter().rposition(|t| matches!(t, Token::Separator))
            .unwrap_or(result.len());
        for i in (0..last_sep).rev() {
            if matches!(&result[i], Token::Var { .. }) {
                result.remove(i);
                break;
            }
        }
    }

    // Remove the "ft" flag and its SoftSep+Var(1)+Separator sequence
    remove_firmware_flag(&mut result);

    result
}

/// Remove the "ft" flag and restore the original header token.
fn remove_firmware_flag(tokens: &mut Vec<Token>) {
    let ft_pos = match tokens.iter().position(|t| matches!(t, Token::String(s) if s == "ft")) {
        Some(p) => p,
        None => return,
    };

    // Remove String("ft"), SoftSep, Var(1), Separator (4 tokens)
    // and replace with the original VarInt
    // For class mods it was Var(9), for equipment it was Var(2)
    // We can detect by checking if there's a Part { index: 234 } anywhere
    let is_class_mod = tokens.iter().any(|t| {
        matches!(t, Token::Part { index, .. } if *index == CLASS_MOD_FIRMWARE_CATEGORY)
    });

    let restore_val = if is_class_mod { 9 } else { 2 };

    // Remove the 3 inserted tokens after "ft" (SoftSep, Var(1), Separator)
    if ft_pos + 3 < tokens.len() {
        tokens.drain(ft_pos + 1..=ft_pos + 3);
    }

    // Replace "ft" with the original VarInt
    tokens[ft_pos] = Token::VarInt(restore_val);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_firmware_flag() {
        let with = vec![Token::String("ft".to_string())];
        let without = vec![Token::VarInt(9)];
        assert!(has_firmware_flag(&with));
        assert!(!has_firmware_flag(&without));
    }
}
