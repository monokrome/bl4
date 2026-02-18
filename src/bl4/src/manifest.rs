//! Manifest data for Borderlands 4 items
//!
//! Provides lookup functions for part names, category names, manufacturers, etc.
//! Data is embedded at compile time from share/manifest/ files.
//!
//! Parts and category names are stored as TSV (tab-separated values).
//! Manufacturers and weapon types remain JSON (hand-curated reference data).

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;

// Embed manifest files at compile time
const CATEGORY_NAMES_TSV: &str = include_str!("../../../share/manifest/category_names.tsv");
const PARTS_DATABASE_TSV: &str = include_str!(concat!(env!("OUT_DIR"), "/parts_database.tsv"));
const MANUFACTURERS_JSON: &str = include_str!("../../../share/manifest/manufacturers.json");
const WEAPON_TYPES_JSON: &str = include_str!("../../../share/manifest/weapon_types.json");
const DROP_POOLS_TSV: &str = include_str!("../../../share/manifest/drop_pools.tsv");

// ============================================================================
// Data Structures (JSON-based reference data only)
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Manufacturer {
    code: String,
    name: String,
    #[serde(default)]
    path: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WeaponType {
    name: String,
    #[serde(default)]
    manufacturers: Vec<Manufacturer>,
}

// ============================================================================
// Parsed Data (Lazy Initialized)
// ============================================================================

/// Category ID -> Category Name (parsed from TSV)
static CATEGORY_NAMES: Lazy<HashMap<i64, String>> = Lazy::new(|| {
    parse_tsv_pairs(CATEGORY_NAMES_TSV)
});

/// (Category, Index) -> Part Name (parsed from TSV)
static PARTS_BY_ID: Lazy<HashMap<(i64, i64), String>> = Lazy::new(|| {
    parse_tsv_parts(PARTS_DATABASE_TSV)
});

fn parse_tsv_pairs(tsv: &str) -> HashMap<i64, String> {
    tsv.lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.splitn(2, '\t');
            let id = cols.next()?.parse::<i64>().ok()?;
            let name = cols.next()?.to_string();
            Some((id, name))
        })
        .collect()
}

fn parse_tsv_parts(tsv: &str) -> HashMap<(i64, i64), String> {
    tsv.lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.splitn(3, '\t');
            let category = cols.next()?.parse::<i64>().ok()?;
            let index = cols.next()?.parse::<i64>().ok()?;
            let name = cols.next()?.to_string();
            Some(((category, index), name))
        })
        .collect()
}

/// Drop pool data for legendary items per (manufacturer, gear_type) pair
#[derive(Debug, Clone)]
pub struct DropPool {
    pub manufacturer_code: String,
    pub gear_type_code: String,
    pub legendary_count: u32,
    pub boss_source_count: u32,
    pub world_pool_name: String,
}

/// (ManufacturerCode, GearTypeCode) -> DropPool
static DROP_POOLS: Lazy<HashMap<(String, String), DropPool>> = Lazy::new(|| {
    DROP_POOLS_TSV
        .lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.splitn(5, '\t').collect();
            if cols.len() < 5 {
                return None;
            }
            let pool = DropPool {
                manufacturer_code: cols[0].to_string(),
                gear_type_code: cols[1].to_string(),
                legendary_count: cols[2].parse().ok()?,
                boss_source_count: cols[3].parse().ok()?,
                world_pool_name: cols[4].to_string(),
            };
            Some(((cols[0].to_string(), cols[1].to_string()), pool))
        })
        .collect()
});

/// Manufacturer Code -> Full Name
static MANUFACTURERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mfrs: HashMap<String, Manufacturer> =
        serde_json::from_str(MANUFACTURERS_JSON).expect("Failed to parse manufacturers.json");

    mfrs.into_iter().map(|(code, m)| (code, m.name)).collect()
});

/// Weapon Type Name -> Manufacturer Codes
static WEAPON_TYPES: Lazy<HashMap<String, Vec<String>>> = Lazy::new(|| {
    let types: HashMap<String, WeaponType> =
        serde_json::from_str(WEAPON_TYPES_JSON).expect("Failed to parse weapon_types.json");

    types
        .into_iter()
        .map(|(name, wt)| {
            let codes: Vec<String> = wt.manufacturers.into_iter().map(|m| m.code).collect();
            (name, codes)
        })
        .collect()
});

// ============================================================================
// Public API
// ============================================================================

/// Get the name of a category by ID
pub fn category_name(category_id: i64) -> Option<&'static str> {
    CATEGORY_NAMES.get(&category_id).map(|s| s.as_str())
}

/// Get a part name by category and index
pub fn part_name(category: i64, index: i64) -> Option<&'static str> {
    PARTS_BY_ID.get(&(category, index)).map(|s| s.as_str())
}

/// Get a manufacturer's full name from its code
pub fn manufacturer_name(code: &str) -> Option<&'static str> {
    MANUFACTURERS.get(code).map(|s| s.as_str())
}

/// Get drop pool data for a (manufacturer, gear_type) pair
pub fn drop_pool(manufacturer_code: &str, gear_type_code: &str) -> Option<&'static DropPool> {
    DROP_POOLS.get(&(manufacturer_code.to_string(), gear_type_code.to_string()))
}

/// Get the total number of legendaries in a world drop pool (e.g., all "Pistols")
pub fn world_pool_legendary_count(world_pool_name: &str) -> u32 {
    DROP_POOLS
        .values()
        .filter(|p| p.world_pool_name == world_pool_name)
        .map(|p| p.legendary_count)
        .sum()
}

/// Get all manufacturer codes for a weapon type
pub fn weapon_type_manufacturers(weapon_type: &str) -> Option<&'static [String]> {
    WEAPON_TYPES.get(weapon_type).map(|v| v.as_slice())
}

/// Get all category IDs and names
pub fn all_categories() -> impl Iterator<Item = (i64, &'static str)> {
    CATEGORY_NAMES.iter().map(|(&id, name)| (id, name.as_str()))
}

/// Get all manufacturer codes and names
pub fn all_manufacturers() -> impl Iterator<Item = (&'static str, &'static str)> {
    MANUFACTURERS
        .iter()
        .map(|(code, name)| (code.as_str(), name.as_str()))
}

/// Extract slot name from a manifest part name.
///
/// Takes the segment after the last `.`, strips the `part_` prefix,
/// and strips trailing `_NN` digit suffixes.
///
/// Examples:
/// - `"DAD_PS.part_barrel_01"` → `"barrel"`
/// - `"part_scope_02"` → `"scope"`
/// - `"part_body"` → `"body"`
/// - `"comp_03_rare"` → `"comp_03_rare"` (no `part_` prefix)
pub fn slot_from_part_name(name: &str) -> &str {
    let segment = name.split('.').next_back().unwrap_or(name);

    let stripped = match segment.strip_prefix("part_") {
        Some(rest) => rest,
        None => return segment,
    };

    // Strip trailing _NN (1-2 digits)
    if let Some(pos) = stripped.rfind('_') {
        let suffix = &stripped[pos + 1..];
        if !suffix.is_empty() && suffix.len() <= 2 && suffix.chars().all(|c| c.is_ascii_digit()) {
            return &stripped[..pos];
        }
    }

    stripped
}

/// Check if manifest data is loaded (forces initialization)
pub fn is_loaded() -> bool {
    // Access lazy statics to force initialization
    let _ = CATEGORY_NAMES.len();
    let _ = PARTS_BY_ID.len();
    let _ = MANUFACTURERS.len();
    true
}

/// Get statistics about loaded manifest data
pub fn stats() -> ManifestStats {
    ManifestStats {
        categories: CATEGORY_NAMES.len(),
        parts: PARTS_BY_ID.len(),
        manufacturers: MANUFACTURERS.len(),
        weapon_types: WEAPON_TYPES.len(),
    }
}

#[derive(Debug, Clone)]
pub struct ManifestStats {
    pub categories: usize,
    pub parts: usize,
    pub manufacturers: usize,
    pub weapon_types: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_name() {
        // Should find known categories
        assert!(category_name(2).is_some()); // Daedalus Pistol
        assert!(category_name(9).is_some()); // Jakobs Shotgun
    }

    #[test]
    fn test_part_name() {
        // This depends on having actual parts in the database
        // Just verify it doesn't panic
        let _ = part_name(2, 1);
    }

    #[test]
    fn test_manufacturer_name() {
        assert_eq!(manufacturer_name("JAK"), Some("Jakobs"));
        assert_eq!(manufacturer_name("TOR"), Some("Torgue"));
        assert_eq!(manufacturer_name("BOR"), Some("Ripper")); // NCS NexusSerialized: BOR = Ripper
        assert_eq!(manufacturer_name("XXX"), None);
    }

    #[test]
    fn test_stats() {
        let s = stats();
        assert!(s.categories > 0);
        // Parts database may be empty until populated via NCS extraction
        // assert!(s.parts > 0);
        assert!(s.manufacturers > 0);
    }

    #[test]
    fn test_weapon_type_manufacturers() {
        // Pistols should have manufacturers
        let pistol_mfrs = weapon_type_manufacturers("Pistols");
        assert!(pistol_mfrs.is_some());
        assert!(!pistol_mfrs.unwrap().is_empty());

        // SMG should have manufacturers
        let smg_mfrs = weapon_type_manufacturers("SMG");
        assert!(smg_mfrs.is_some());

        // Shotguns should have manufacturers
        let shotgun_mfrs = weapon_type_manufacturers("Shotguns");
        assert!(shotgun_mfrs.is_some());

        // Unknown type returns None
        assert!(weapon_type_manufacturers("LaserBlaster3000").is_none());
    }

    #[test]
    fn test_all_categories() {
        let categories: Vec<_> = all_categories().collect();
        assert!(!categories.is_empty());

        // All IDs should be positive
        for (id, name) in &categories {
            assert!(*id >= 0);
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_all_manufacturers() {
        let manufacturers: Vec<_> = all_manufacturers().collect();
        assert!(!manufacturers.is_empty());

        // Should include known manufacturers
        let codes: Vec<&str> = manufacturers.iter().map(|(c, _)| *c).collect();
        assert!(codes.contains(&"JAK")); // Jakobs
        assert!(codes.contains(&"TOR")); // Torgue

        // All entries should have non-empty values
        for (code, name) in &manufacturers {
            assert!(!code.is_empty());
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_is_loaded() {
        // is_loaded forces initialization and always returns true
        assert!(is_loaded());
        // Call again to ensure it's idempotent
        assert!(is_loaded());
    }

    #[test]
    fn test_drop_pool() {
        let pool = drop_pool("JAK", "PS");
        assert!(pool.is_some());
        let pool = pool.unwrap();
        assert_eq!(pool.manufacturer_code, "JAK");
        assert_eq!(pool.gear_type_code, "PS");
        assert!(pool.legendary_count > 0);
        assert_eq!(pool.world_pool_name, "Pistols");
    }

    #[test]
    fn test_drop_pool_unknown() {
        assert!(drop_pool("ZZZ", "XX").is_none());
    }

    #[test]
    fn test_world_pool_legendary_count() {
        let pistol_count = world_pool_legendary_count("Pistols");
        assert!(pistol_count > 0);
        assert!(world_pool_legendary_count("Nonexistent") == 0);
    }

    #[test]
    fn test_slot_from_part_name() {
        assert_eq!(slot_from_part_name("DAD_PS.part_barrel_01"), "barrel");
        assert_eq!(slot_from_part_name("part_scope_02"), "scope");
        assert_eq!(slot_from_part_name("part_body"), "body");
        assert_eq!(slot_from_part_name("comp_03_rare"), "comp_03_rare");
        assert_eq!(slot_from_part_name("JAK_SG.part_foregrip_03"), "foregrip");
        assert_eq!(slot_from_part_name("part_mag_1"), "mag");
        assert_eq!(slot_from_part_name("part_barrel"), "barrel");
    }
}
