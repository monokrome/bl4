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
//!
//! Currently only replacing existing firmware is supported. Adding firmware
//! to items that don't have it requires header mutation (inserting the "ft"
//! flag and restructuring the header) which isn't reliably understood yet.

use crate::manifest;
use crate::serial::Token;

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
    if crate::skills::is_class_mod(item_category) {
        // Class mods require "ft" flag + Part { index: 234 }
        if !has_firmware_flag(tokens) {
            return None;
        }
        detect_class_mod(tokens)
    } else {
        // Equipment: check last trailing VarInt against firmware parts list
        detect_equipment(tokens, item_category)
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

/// Detect firmware on equipment (last VarInt before final Separator).
///
/// Validates the VarInt value against the item type's firmware parts pool
/// to distinguish firmware indices from stat/seed values.
fn detect_equipment(tokens: &[Token], item_category: i64) -> Option<DetectedFirmware> {
    let fw_category = equipment_firmware_category(item_category);

    // Scan all trailing VarInts (between last SoftSeparator and end)
    // checking each against the firmware parts list
    for token in tokens.iter().rev() {
        match token {
            Token::Var { val, .. } => {
                let fw_idx = *val as i64;
                if let Some(name) = manifest::part_name(fw_category, fw_idx) {
                    if name.contains("firmware") {
                        return Some(DetectedFirmware {
                            name: name.to_string(),
                            index: fw_idx,
                            category: fw_category,
                        });
                    }
                }
            }
            // Stop at SoftSeparator — firmware is in the trailing section after the last SoftSep
            Token::SoftSeparator => break,
            _ => continue,
        }
    }
    None
}

/// Get the firmware parts category for an equipment item type.
///
/// Manufacturer-specific categories (e.g., 268=Jakobs Enhancement) share
/// firmware pools with their base type (e.g., 247=Enhancement).
fn equipment_firmware_category(item_category: i64) -> i64 {
    let cat_name = manifest::category_name(item_category).unwrap_or("");
    let lower = cat_name.to_lowercase();
    if lower.contains("repair kit") { 243 }
    else if lower.contains("heavy weapon gadget") || lower.contains("turret gadget") { 244 }
    else if lower.contains("grenade gadget") || lower.contains("terminal gadget") || lower.contains("weapon gadget") { 245 }
    else if lower.contains("shield") { 246 }
    else if lower.contains("enhancement") { 247 }
    else { EQUIPMENT_FIRMWARE_CATEGORY }
}

/// Resolve a firmware name to its index in the appropriate category.
pub fn resolve_firmware(name: &str, item_category: i64) -> Result<(i64, i64), String> {
    let fw_category = if crate::skills::is_class_mod(item_category) {
        CLASS_MOD_FIRMWARE_CATEGORY as i64
    } else {
        equipment_firmware_category(item_category)
    };

    let with_prefix = if name.starts_with("part_firmware_") {
        name.to_string()
    } else {
        format!("part_firmware_{}", name)
    };

    if let Some(idx) = manifest::part_index(fw_category, &with_prefix) {
        return Ok((fw_category, idx));
    }

    if let Some(idx) = manifest::part_index(fw_category, name) {
        return Ok((fw_category, idx));
    }

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
        equipment_firmware_category(item_category)
    };

    let mut result = Vec::new();
    for idx in 1..=300 {
        if let Some(name) = manifest::part_name(fw_category, idx) {
            if name.contains("firmware") {
                result.push((idx, name.to_string()));
            }
        }
    }
    result
}

/// Replace firmware on a token stream that already has firmware.
///
/// Returns None if the item doesn't have firmware. Adding firmware to
/// items without it is not yet supported (requires header mutation).
pub fn apply(tokens: &[Token], fw_index: i64, item_category: i64) -> Option<Vec<Token>> {
    if crate::skills::is_class_mod(item_category) {
        if !has_firmware_flag(tokens) {
            return None;
        }
        Some(replace_class_mod_firmware(tokens, fw_index))
    } else {
        // Equipment: detect existing firmware by checking trailing VarInts
        if detect_equipment(tokens, item_category).is_none() {
            return None;
        }
        Some(replace_equipment_firmware(tokens, fw_index, item_category))
    }
}

/// Replace firmware Part values on a class mod.
fn replace_class_mod_firmware(tokens: &[Token], fw_index: i64) -> Vec<Token> {
    let mut result = tokens.to_vec();
    for token in &mut result {
        if let Token::Part { index, values, .. } = token {
            if *index == CLASS_MOD_FIRMWARE_CATEGORY {
                *values = vec![fw_index as u64];
                return result;
            }
        }
    }
    result
}

/// Replace firmware VarInt on equipment.
///
/// Finds the VarInt whose value maps to a firmware part in the item's
/// firmware category, and replaces its value.
fn replace_equipment_firmware(tokens: &[Token], fw_index: i64, item_category: i64) -> Vec<Token> {
    let fw_category = equipment_firmware_category(item_category);
    let mut result = tokens.to_vec();

    for i in (0..result.len()).rev() {
        match &result[i] {
            Token::Var { val, .. } => {
                let idx = *val as i64;
                if let Some(name) = manifest::part_name(fw_category, idx) {
                    if name.contains("firmware") {
                        result[i] = Token::VarInt(fw_index as u64);
                        return result;
                    }
                }
            }
            Token::SoftSeparator => break,
            _ => continue,
        }
    }
    result
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
