//! Manifest data for Borderlands 4 items
//!
//! Provides lookup functions for part names, category names, manufacturers, etc.
//! Data is embedded at compile time from share/manifest/ JSON files.
//!
//! ## Category Names
//!
//! Maps category IDs to human-readable names:
//!
//! ```json
#![doc = include_str!("../../../share/manifest/category_names.json")]
//! ```
//!
//! ## Manufacturers
//!
//! Maps manufacturer codes to full names:
//!
//! ```json
#![doc = include_str!("../../../share/manifest/manufacturers.json")]
//! ```
//!
//! ## Weapon Types
//!
//! Maps weapon types to their valid manufacturers:
//!
//! ```json
#![doc = include_str!("../../../share/manifest/weapon_types.json")]
//! ```

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;

// Embed manifest JSON files at compile time
const CATEGORY_NAMES_JSON: &str = include_str!("../../../share/manifest/category_names.json");
const PARTS_DATABASE_JSON: &str = include_str!("../../../share/manifest/parts_database.json");
const MANUFACTURERS_JSON: &str = include_str!("../../../share/manifest/manufacturers.json");
const WEAPON_TYPES_JSON: &str = include_str!("../../../share/manifest/weapon_types.json");

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct CategoryNamesFile {
    categories: HashMap<String, String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PartsDatabase {
    version: u32,
    #[serde(default)]
    source: Option<String>,
    parts: Vec<PartEntry>,
    #[serde(default)]
    categories: HashMap<String, CategoryInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PartEntry {
    category: i64,
    index: i64,
    name: String,
    #[serde(default)]
    group: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CategoryInfo {
    count: usize,
    name: String,
}

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

/// Category ID -> Category Name
static CATEGORY_NAMES: Lazy<HashMap<i64, String>> = Lazy::new(|| {
    let file: CategoryNamesFile =
        serde_json::from_str(CATEGORY_NAMES_JSON).expect("Failed to parse category_names.json");

    file.categories
        .into_iter()
        .filter_map(|(k, v)| k.parse::<i64>().ok().map(|id| (id, v)))
        .collect()
});

/// (Category, Index) -> Part Name
static PARTS_BY_ID: Lazy<HashMap<(i64, i64), String>> = Lazy::new(|| {
    let db: PartsDatabase =
        serde_json::from_str(PARTS_DATABASE_JSON).expect("Failed to parse parts_database.json");

    db.parts
        .into_iter()
        .map(|p| ((p.category, p.index), p.name))
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
}
