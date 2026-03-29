//! Functional composition pipeline for serial decode → resolve → validate.
//!
//! Each pipeline stage is a free function that takes a `DecodedItem` and returns
//! a `DecodedItem` with more fields populated. Stages compose left-to-right and
//! each reads only fields populated by earlier stages.

use super::validate::ValidationResult;
use super::{Element, ItemSerial, Rarity, ResolvedPart, ResolvedString, SerialError};
use crate::manifest;
use crate::parts::{category_name, level_from_code, weapon_info_from_first_varint};

/// Fully-resolved item with all extracted properties.
#[derive(Debug, Clone)]
pub struct DecodedItem {
    pub serial: ItemSerial,
    pub category: Option<i64>,
    pub category_name: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub level: Option<u64>,
    pub elements: Vec<Element>,
    pub rarity: Option<Rarity>,
    pub parts: Vec<ResolvedPart>,
    pub strings: Vec<ResolvedString>,
    pub name: Option<String>,
    pub validation: Option<ValidationResult>,
    pub confidence: Option<f64>,
}

/// Decode a serial string into a minimal DecodedItem.
pub fn decode(serial: &str) -> Result<DecodedItem, SerialError> {
    let item = ItemSerial::decode(serial)?;
    Ok(DecodedItem {
        serial: item,
        category: None,
        category_name: None,
        manufacturer: None,
        weapon_type: None,
        level: None,
        elements: Vec::new(),
        rarity: None,
        parts: Vec::new(),
        strings: Vec::new(),
        name: None,
        validation: None,
        confidence: None,
    })
}

/// Resolve category from token structure.
pub fn resolve_category(mut item: DecodedItem) -> DecodedItem {
    item.category = item.serial.parts_category();
    if let Some(cat) = item.category {
        item.category_name = category_name(cat).map(|s| s.to_string());
    }
    item
}

/// Resolve manufacturer and weapon type from category/tokens.
pub fn resolve_identity(mut item: DecodedItem) -> DecodedItem {
    if let Some(mfg_id) = item.serial.manufacturer {
        if let Some((mfg, wtype)) = weapon_info_from_first_varint(mfg_id) {
            item.manufacturer = Some(mfg.to_string());
            item.weapon_type = Some(wtype.to_string());
        }
    } else if let Some(cat) = item.category {
        item.weapon_type = category_name(cat).map(|s| s.to_string());
    }

    item.level = item
        .serial
        .level
        .and_then(level_from_code)
        .map(|(capped, _)| capped as u64);

    item.rarity = item.serial.rarity;
    item
}

/// Resolve all Part tokens to names/slots via manifest.
pub fn resolve_parts(mut item: DecodedItem) -> DecodedItem {
    item.parts = item.serial.resolved_parts();
    item.strings = item.serial.string_tokens();

    let total = item.parts.len();
    let resolved = item.parts.iter().filter(|p| p.name.is_some()).count();
    item.confidence = if total > 0 {
        Some(resolved as f64 / total as f64)
    } else {
        Some(0.0)
    };

    item
}

/// Detect elements from resolved part names.
pub fn resolve_elements(mut item: DecodedItem) -> DecodedItem {
    item.elements = item.serial.elements.clone();
    item
}

/// Resolve item name via 3-pass legendary + generic fallback.
pub fn resolve_name(mut item: DecodedItem) -> DecodedItem {
    let parts_with_names = item.serial.parts_with_names();
    let is_legendary = is_legendary_from_parts(&item.parts);

    item.name = resolve_item_name(&parts_with_names, item.category, is_legendary);
    item
}

/// Determine if an item is legendary by checking its rarity comp parts.
///
/// An item is legendary only if it has comp_05_legendary_* without a lower
/// base rarity comp (comp_01 through comp_04). Purple items can carry a
/// legendary comp for special behavior without being legendary themselves.
fn is_legendary_from_parts(parts: &[ResolvedPart]) -> bool {
    let mut has_legendary = false;
    let mut has_base_rarity = false;
    for part in parts {
        if part.slot != "rarity" {
            continue;
        }
        let name = match part.name {
            Some(n) => n,
            None => continue,
        };
        let segment = name.split('.').next_back().unwrap_or(name);
        if segment.starts_with("comp_05_legendary") {
            has_legendary = true;
        } else if segment.starts_with("comp_0") {
            has_base_rarity = true;
        }
    }
    has_legendary && !has_base_rarity
}

/// Run validation checks.
pub fn validate(mut item: DecodedItem) -> DecodedItem {
    item.validation = Some(item.serial.validate());
    item
}

/// Run the full pipeline: decode → category → identity → parts → elements → name → validate.
pub fn full_resolve(serial: &str) -> Result<DecodedItem, SerialError> {
    let item = decode(serial)?;
    Ok(validate(resolve_name(resolve_elements(resolve_parts(
        resolve_identity(resolve_category(item)),
    )))))
}

// --- Name Resolution (moved from bl4-cli) ---

/// Resolve an item name for any item.
///
/// For unique/legendary items, uses comp/barrel-based resolution.
/// For generic items, uses part names with category context.
pub fn resolve_item_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
    is_legendary: bool,
) -> Option<String> {
    if let Some(name) = resolve_legendary_name(parts, category, is_legendary) {
        return Some(name);
    }

    for (_index, name, _values) in parts {
        if let Some(n) = name {
            let segment = n.split('.').next_back().unwrap_or(n);
            if !is_legendary && segment.starts_with("comp_05_legendary_") {
                continue;
            }
            if let Some(display) = manifest::item_name_from_part(segment, category) {
                return Some(display.to_string());
            }
        }
    }

    None
}

/// Three-pass legendary name resolution:
/// 1. comp_05_legendary_* suffix
/// 2. Legendary barrel suffix
/// 3. Per-category NCS metadata (if legendary with generic barrel)
pub fn resolve_legendary_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
    is_legendary: bool,
) -> Option<String> {
    if is_legendary {
        if let Some(name) = name_from_legendary_comp(parts) {
            return Some(name);
        }
    }

    let (barrel_result, generic_barrel_base) = name_from_barrel(parts);
    if let Some(name) = barrel_result {
        return Some(name);
    }

    if is_legendary {
        return name_from_category_metadata(category, generic_barrel_base);
    }

    None
}

fn name_from_legendary_comp(parts: &[(u64, Option<&'static str>, Vec<u64>)]) -> Option<String> {
    for (_index, name, _values) in parts {
        let n = (*name)?;
        let segment = n.split('.').next_back().unwrap_or(n);
        if let Some(suffix) = segment.strip_prefix("comp_05_legendary_") {
            if !suffix.is_empty() {
                return match_legendary_suffix(suffix);
            }
        }
    }
    None
}

fn name_from_barrel(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
) -> (Option<String>, Option<&'static str>) {
    let mut generic_barrel_base: Option<&str> = None;
    let mut barrel_candidates: Vec<&str> = Vec::new();

    for (_index, name, _values) in parts {
        if let Some(n) = name {
            let segment = n.split('.').next_back().unwrap_or(n);
            if let Some(suffix) = legendary_barrel_suffix(segment) {
                barrel_candidates.push(suffix);
            }
            if generic_barrel_base.is_none() {
                generic_barrel_base = generic_barrel_base_name(segment);
            }
        }
    }

    if barrel_candidates.is_empty() {
        return (None, generic_barrel_base);
    }

    if generic_barrel_base.is_some() {
        return (None, generic_barrel_base);
    }

    if let Some(known) = best_known_legendary(&barrel_candidates) {
        return (Some(known), generic_barrel_base);
    }

    let name = match_legendary_suffix(barrel_candidates.last().unwrap());
    (name, generic_barrel_base)
}

fn name_from_category_metadata(
    category: Option<i64>,
    generic_barrel_base: Option<&str>,
) -> Option<String> {
    let cat = category?;
    let barrel_base = generic_barrel_base?;
    let alias = manifest::legendary_barrel_alias(cat, barrel_base)?;
    let segment = alias.split('.').next_back().unwrap_or(alias);
    let prefix = format!("part_{}_", barrel_base);
    let suffix = segment.strip_prefix(&prefix)?;
    match_legendary_suffix(suffix)
}

fn best_known_legendary(candidates: &[&str]) -> Option<String> {
    let legendaries = crate::reference::KNOWN_LEGENDARIES;
    for suffix in candidates {
        let suffix_lower = suffix.to_lowercase();
        for leg in legendaries {
            let leg_segment = leg.internal.split('.').next_back().unwrap_or(leg.internal);
            if let Some(leg_suffix) = leg_segment.strip_prefix("comp_05_legendary_") {
                if leg_suffix.to_lowercase() == suffix_lower {
                    return Some(leg.name.to_string());
                }
            }
        }
    }
    None
}

fn legendary_barrel_suffix(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("part_barrel_")?;

    if rest.starts_with("licensed_") {
        return None;
    }

    for prefix in ["01_", "02_"] {
        if let Some(suffix) = rest.strip_prefix(prefix) {
            if suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_lowercase()) {
                return None;
            }
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            return Some(suffix);
        }
    }

    if rest == "01" || rest == "02" {
        return None;
    }

    Some(rest)
}

fn generic_barrel_base_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("part_")?;
    if rest == "barrel_01" || rest == "barrel_02" {
        Some(rest)
    } else {
        None
    }
}

fn match_legendary_suffix(suffix: &str) -> Option<String> {
    let np_key = format!("np_{}", suffix.to_lowercase());
    if let Some(name) = manifest::item_display_name(&np_key) {
        return Some(name.to_string());
    }

    let suffix_lower = suffix.to_lowercase();
    for leg in crate::reference::KNOWN_LEGENDARIES {
        let leg_segment = leg.internal.split('.').next_back().unwrap_or(leg.internal);
        if let Some(leg_suffix) = leg_segment.strip_prefix("comp_05_legendary_") {
            if leg_suffix.to_lowercase() == suffix_lower {
                return Some(leg.name.to_string());
            }
        }
    }

    if suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let title = suffix
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_resolve_shalashaska() {
        let item = full_resolve("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+A00}").unwrap();
        assert_eq!(item.name.as_deref(), Some("Shalashaska"));
        assert_eq!(item.manufacturer.as_deref(), Some("Jakobs"));
        assert_eq!(item.weapon_type.as_deref(), Some("Pistol"));
        assert_eq!(item.level, Some(50));
        assert!(item.elements.is_empty());
        let scope = item.parts.iter().find(|p| p.index == 26).unwrap();
        assert_eq!(scope.slot, "scope");
        assert!(!scope.is_element);
    }

    #[test]
    fn full_resolve_shield() {
        let item = full_resolve("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        assert!(item.elements.is_empty());
        assert!(item.category.is_some());
    }

    #[test]
    fn full_resolve_validates() {
        let item = full_resolve("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+A00}").unwrap();
        assert!(item.validation.is_some());
    }

    #[test]
    fn full_resolve_epic_with_legendary_comp() {
        // Epic Torgue Shotgun with comp_04_epic + comp_05_legendary_linebacker.
        // Should resolve as "Rider" (generic name), not "Linebacker" (legendary name).
        let item =
            full_resolve("@UgfIh4FpCJ&`GZQM3YDlv4IO&aKh!={NYtn1phBTWp<bcNApi").unwrap();
        assert_eq!(item.name.as_deref(), Some("Rider"));
        assert_eq!(item.manufacturer.as_deref(), Some("Torgue"));
        assert_eq!(item.weapon_type.as_deref(), Some("Shotgun"));
        assert_eq!(item.level, Some(11));
    }

    #[test]
    fn resolve_name_from_comp() {
        let parts = vec![(83, Some("JAK_PS.comp_05_legendary_shalashaska"), vec![])];
        let name = resolve_item_name(&parts, Some(3), true);
        assert_eq!(name.as_deref(), Some("Shalashaska"));
    }

    #[test]
    fn resolve_name_no_match() {
        let parts = vec![(1, Some("JAK_PS.part_body"), vec![])];
        let name = resolve_legendary_name(&parts, Some(3), false);
        assert!(name.is_none());
    }
}
