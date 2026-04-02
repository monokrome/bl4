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

/// Resolve rarity from comp parts, falling back to header-derived rarity.
///
/// The header rarity (`serial.rarity`) is unreliable for many items. The
/// authoritative source is the rarity comp slot: `comp_05_legendary_*`
/// means legendary, `comp_04_epic` means epic, etc. An item with both
/// `comp_04_epic` and `comp_05_legendary_*` is epic — the legendary comp
/// grants special behavior but doesn't change the item's rarity tier.
pub fn resolve_rarity(mut item: DecodedItem) -> DecodedItem {
    let rarity_from_parts = rarity_from_comp_parts(&item.parts);
    if rarity_from_parts.is_some() {
        item.rarity = rarity_from_parts;
    }
    item
}

fn rarity_from_comp_parts(parts: &[ResolvedPart]) -> Option<Rarity> {
    let mut has_legendary = false;
    let mut base_tier: Option<u8> = None;

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
        } else if let Some(rest) = segment.strip_prefix("comp_0") {
            // comp_01 = Common, comp_02 = Uncommon, comp_03 = Rare, comp_04 = Epic
            let tier = rest.as_bytes().first().copied().unwrap_or(0);
            if tier.is_ascii_digit() {
                let t = tier - b'0';
                base_tier = Some(base_tier.map_or(t, |b: u8| b.max(t)));
            }
        }
    }

    // A base rarity comp (01-04) always wins. comp_05_legendary is only
    // the rarity when it appears alone (no base comp coexists).
    match base_tier {
        Some(1) => Some(Rarity::Common),
        Some(2) => Some(Rarity::Uncommon),
        Some(3) => Some(Rarity::Rare),
        Some(4) => Some(Rarity::Epic),
        None if has_legendary => Some(Rarity::Legendary),
        _ => None,
    }
}

/// Resolve item name. Legendary items go through the 3-pass legendary
/// resolver; everything else falls through to generic name lookup.
pub fn resolve_name(mut item: DecodedItem) -> DecodedItem {
    let parts_with_names = item.serial.parts_with_names();
    let is_legendary = item.rarity == Some(Rarity::Legendary);

    item.name = if is_legendary {
        resolve_legendary_name(&parts_with_names, item.category)
            .or_else(|| resolve_generic_name(&parts_with_names, item.category))
    } else {
        resolve_generic_name(&parts_with_names, item.category)
    };

    item
}

/// Run validation checks.
pub fn validate(mut item: DecodedItem) -> DecodedItem {
    item.validation = Some(item.serial.validate());
    item
}

/// Run the full pipeline: decode → category → identity → parts → elements → rarity → name → validate.
pub fn full_resolve(serial: &str) -> Result<DecodedItem, SerialError> {
    let item = decode(serial)?;
    Ok(validate(resolve_name(resolve_rarity(resolve_elements(
        resolve_parts(resolve_identity(resolve_category(item))),
    )))))
}

// --- Name Resolution (moved from bl4-cli) ---

/// Resolve a generic (non-legendary) item name from manifest part lookups.
pub fn resolve_generic_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
) -> Option<String> {
    for (_index, name, _values) in parts {
        if let Some(n) = name {
            let segment = n.split('.').next_back().unwrap_or(n);
            if segment.starts_with("comp_05_legendary_") {
                continue;
            }
            if let Some(display) = manifest::item_name_from_part(segment, category) {
                return Some(display.to_string());
            }
        }
    }
    None
}

/// Resolve an item name using any strategy (legendary + generic fallback).
///
/// Kept for backward compatibility with callers that pass `is_legendary`.
pub fn resolve_item_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
    is_legendary: bool,
) -> Option<String> {
    if is_legendary {
        if let Some(name) = resolve_legendary_name(parts, category) {
            return Some(name);
        }
    }
    resolve_generic_name(parts, category)
}

/// Three-pass legendary name resolution:
/// 1. comp_05_legendary_* suffix
/// 2. Legendary barrel suffix
/// 3. Per-category NCS metadata (if legendary with generic barrel)
///
/// Only called for items already determined to be legendary.
pub fn resolve_legendary_name(
    parts: &[(u64, Option<&'static str>, Vec<u64>)],
    category: Option<i64>,
) -> Option<String> {
    if let Some(name) = name_from_legendary_comp(parts) {
        return Some(name);
    }

    let (barrel_result, generic_barrel_base) = name_from_barrel(parts);
    if let Some(name) = barrel_result {
        return Some(name);
    }

    name_from_category_metadata(category, generic_barrel_base)
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
        let item = full_resolve("@UgfIh4FpCJ&`GZQM3YDlv4IO&aKh!={NYtn1phBTWp<bcNApi").unwrap();
        assert_eq!(item.name.as_deref(), Some("Rider"));
        assert_eq!(item.manufacturer.as_deref(), Some("Torgue"));
        assert_eq!(item.weapon_type.as_deref(), Some("Shotgun"));
        assert_eq!(item.level, Some(11));
    }

    #[test]
    fn resolve_name_from_comp() {
        let parts = vec![(83, Some("JAK_PS.comp_05_legendary_shalashaska"), vec![])];
        let name = resolve_legendary_name(&parts, Some(3));
        assert_eq!(name.as_deref(), Some("Shalashaska"));
    }

    #[test]
    fn resolve_name_no_match() {
        let parts = vec![(1, Some("JAK_PS.part_body"), vec![])];
        let name = resolve_legendary_name(&parts, Some(3));
        assert!(name.is_none());
    }

    #[test]
    fn resolve_rarity_from_parts() {
        let item = full_resolve("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+A00}").unwrap();
        assert_eq!(item.rarity, Some(Rarity::Legendary));
    }

    #[test]
    fn resolve_rarity_epic_with_legendary_comp() {
        // Epic item with comp_04_epic + comp_05_legendary_* should resolve as Epic
        let item = full_resolve("@UgfIh4FpCJ&`GZQM3YDlv4IO&aKh!={NYtn1phBTWp<bcNApi").unwrap();
        assert_eq!(item.rarity, Some(Rarity::Epic));
    }

    #[test]
    fn generic_name_skips_legendary_comp() {
        let parts = vec![
            (1, Some("TOR_SG.comp_04_epic"), vec![]),
            (2, Some("TOR_SG.comp_05_legendary_linebacker"), vec![]),
            (3, Some("TOR_SG.part_barrel_01"), vec![]),
        ];
        let name = resolve_generic_name(&parts, Some(10));
        // Should NOT resolve to "Linebacker"
        assert!(name.as_deref() != Some("Linebacker"));
    }
}
