//! Item name extraction from NCS inv_name_part files
//!
//! NCS `inv_name_part` files contain interleaved string pairs:
//! - Even index: naming key (e.g., "np_anarchy", "np_weap_DAD_AR_B01")
//! - Odd index: GUID reference (e.g., "Uni_Inv_TED_SG_Anarchy, UUID, Anarchy")
//!
//! The display name is the third comma-separated field in the GUID string.

use std::collections::HashMap;
use std::path::Path;

/// A single item name mapping from NCS data
#[derive(Debug, Clone)]
pub struct ItemNameEntry {
    /// Naming key (e.g., "np_anarchy")
    pub np_key: String,
    /// Category from NCS (e.g., "WeaponNamingStrategies", "Uni_Inv_TED_SG_Anarchy")
    pub category: String,
    /// UUID identifier
    pub uuid: String,
    /// Display name shown in-game (e.g., "Anarchy")
    pub display_name: String,
}

/// Extract item name entries from a single inv_name_part binary
pub fn extract_from_binary(data: &[u8]) -> Vec<ItemNameEntry> {
    let content = match crate::NcsContent::parse(data) {
        Some(c) => c,
        None => return Vec::new(),
    };

    if content.type_name() != "inv_name_part" {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let strings = &content.strings;

    // Strings are interleaved pairs: np_key at even, GUID at odd
    let mut i = 0;
    while i + 1 < strings.len() {
        let np_key = &strings[i];
        let guid_str = &strings[i + 1];

        // np_key must start with "np_" or "NP_"
        if !np_key.starts_with("np_") && !np_key.starts_with("NP_") {
            i += 1;
            continue;
        }

        // GUID format: "Category, UUID, DisplayName"
        if let Some(entry) = parse_guid_entry(np_key, guid_str) {
            entries.push(entry);
            i += 2;
        } else {
            i += 1;
        }
    }

    entries
}

/// Parse a GUID string into an ItemNameEntry
fn parse_guid_entry(np_key: &str, guid_str: &str) -> Option<ItemNameEntry> {
    let parts: Vec<&str> = guid_str.splitn(3, ", ").collect();
    if parts.len() != 3 {
        return None;
    }

    let uuid = parts[1];
    // UUID should be 32 hex characters
    if uuid.len() != 32 || !uuid.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let display_name = parts[2].trim();
    if display_name.is_empty() {
        return None;
    }

    Some(ItemNameEntry {
        np_key: np_key.to_lowercase(),
        category: parts[0].to_string(),
        uuid: uuid.to_string(),
        display_name: display_name.to_string(),
    })
}

/// Extract item names from all inv_name_part files in a directory.
///
/// Uses the file with the most entries (highest patch version) as the
/// authoritative source, since later patches contain all prior entries
/// plus new ones.
pub fn extract_from_directory(ncs_dir: &Path) -> Vec<ItemNameEntry> {
    let mut best: Vec<ItemNameEntry> = Vec::new();

    for entry in walkdir::WalkDir::new(ncs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let fname = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if !fname.starts_with("inv_name_part") {
            continue;
        }

        if let Ok(data) = std::fs::read(path) {
            let entries = extract_from_binary(&data);
            if entries.len() > best.len() {
                best = entries;
            }
        }
    }

    best
}

/// Deduplicated map of np_key → display_name.
///
/// When multiple entries share the same np_key (different UUIDs/variants),
/// picks the "best" display name — preferring shorter, simpler names that
/// look like base item names rather than variant names.
pub fn build_name_map(entries: &[ItemNameEntry]) -> HashMap<String, String> {
    let mut by_key: HashMap<String, Vec<&ItemNameEntry>> = HashMap::new();
    for entry in entries {
        by_key.entry(entry.np_key.clone()).or_default().push(entry);
    }

    let mut map = HashMap::new();
    for (key, variants) in by_key {
        if variants.len() == 1 {
            map.insert(key, variants[0].display_name.clone());
        } else {
            // Pick the best variant — prefer the entry whose category matches
            // the most common pattern for this key type
            let best = pick_best_variant(&variants);
            map.insert(key, best.display_name.clone());
        }
    }

    map
}

/// Pick the best variant from multiple entries sharing the same np_key.
///
/// Heuristics:
/// 1. Prefer entries with UUID `641B14834BAE08173BD6AAACEDAB0310` (the "default" UUID)
/// 2. Prefer shorter display names (base names, not "Upgraded X" variants)
/// 3. Prefer names without variant prefixes
fn pick_best_variant<'a>(variants: &[&'a ItemNameEntry]) -> &'a ItemNameEntry {
    // The "default" UUID that appears on base variants
    const DEFAULT_UUID: &str = "641B14834BAE08173BD6AAACEDAB0310";

    let variant_prefixes = ["upgraded ", "big encore ", "badass ", "vile "];

    let mut best = variants[0];
    let mut best_score: i32 = i32::MIN;

    for &v in variants {
        let mut score: i32 = 0;
        let name_lower = v.display_name.to_lowercase();

        // Prefer default UUID
        if v.uuid == DEFAULT_UUID {
            score += 100;
        }

        // Penalize variant prefixes
        if variant_prefixes.iter().any(|p| name_lower.starts_with(p)) {
            score -= 50;
        }

        // Prefer shorter names
        score -= v.display_name.len() as i32;

        if score > best_score {
            best_score = score;
            best = v;
        }
    }

    best
}

/// Write item names to a TSV file.
///
/// Format: `np_key\tdisplay_name`
pub fn write_tsv(entries: &[ItemNameEntry], path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let map = build_name_map(entries);

    let mut pairs: Vec<_> = map.into_iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut f = std::fs::File::create(path)?;
    for (key, name) in &pairs {
        writeln!(f, "{}\t{}", key, name)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_guid_entry() {
        let entry = parse_guid_entry(
            "np_anarchy",
            "Uni_Inv_TED_SG_Anarchy, 641B14834BAE08173BD6AAACEDAB0310, Anarchy",
        )
        .unwrap();
        assert_eq!(entry.np_key, "np_anarchy");
        assert_eq!(entry.category, "Uni_Inv_TED_SG_Anarchy");
        assert_eq!(entry.display_name, "Anarchy");
    }

    #[test]
    fn test_parse_guid_entry_invalid() {
        assert!(parse_guid_entry("np_test", "not a guid").is_none());
        assert!(parse_guid_entry("np_test", "Cat, SHORT, Name").is_none());
    }

    #[test]
    fn test_pick_best_variant_prefers_default_uuid() {
        let default = ItemNameEntry {
            np_key: "np_test".into(),
            category: "Uni_Test".into(),
            uuid: "641B14834BAE08173BD6AAACEDAB0310".into(),
            display_name: "Anarchy".into(),
        };
        let variant = ItemNameEntry {
            np_key: "np_test".into(),
            category: "Uni_Test".into(),
            uuid: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA1".into(),
            display_name: "Forsaken Chaos".into(),
        };
        let best = pick_best_variant(&[&variant, &default]);
        assert_eq!(best.display_name, "Anarchy");
    }
}
