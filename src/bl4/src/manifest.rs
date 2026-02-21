//! Manifest data for Borderlands 4 items
//!
//! Provides lookup functions for part names, category names, manufacturers, etc.
//! Data is embedded at compile time from share/manifest/ files.
//!
//! Parts and category names are stored as TSV (tab-separated values).
//! Manufacturers and weapon types remain JSON (hand-curated reference data).

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

// Embed manifest files at compile time
const CATEGORY_NAMES_TSV: &str = include_str!("../../../share/manifest/category_names.tsv");
const PARTS_DATABASE_TSV: &str = include_str!(concat!(env!("OUT_DIR"), "/parts_database.tsv"));
const MANUFACTURERS_JSON: &str = include_str!("../../../share/manifest/manufacturers.json");
const WEAPON_TYPES_JSON: &str = include_str!("../../../share/manifest/weapon_types.json");
const DROP_POOLS_TSV: &str = include_str!("../../../share/manifest/drop_pools.tsv");
const PART_POOLS_TSV: &str = include_str!("../../../share/manifest/part_pools.tsv");
const BOSS_REPLAY_COSTS_TSV: &str =
    include_str!("../../../share/manifest/data_tables/table_bossreplay_costs.tsv");

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

/// (Category, Index) -> (Part Name, Slot) parsed from TSV
static PARTS_BY_ID: Lazy<HashMap<(i64, i64), (String, String)>> = Lazy::new(|| {
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

fn parse_tsv_parts(tsv: &str) -> HashMap<(i64, i64), (String, String)> {
    tsv.lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.splitn(4, '\t');
            let category = cols.next()?.parse::<i64>().ok()?;
            let index = cols.next()?.parse::<i64>().ok()?;
            let name = cols.next()?.to_string();
            let slot = cols.next().unwrap_or("unknown").to_string();
            Some(((category, index), (name, slot)))
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

/// Internal boss name -> display name (parsed from boss replay costs TSV)
static BOSS_NAMES: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut names = HashMap::new();
    for line in BOSS_REPLAY_COSTS_TSV.lines().skip(1) {
        let cols: Vec<&str> = line.splitn(5, '\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let row_name = cols[0];
        let comment = cols[1];
        // Parse comment: "Table_BossReplay_Costs, <UUID>, <DisplayName>"
        if let Some(display_name) = parse_boss_comment(comment) {
            names.insert(row_name.to_string(), display_name.to_string());
        }
    }
    names
});

fn parse_boss_comment(comment: &str) -> Option<&str> {
    if comment.is_empty() {
        return None;
    }
    let mut parts = comment.splitn(3, ", ");
    let _table = parts.next()?;
    let uuid = parts.next()?;
    if uuid.len() != 32 || !uuid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    parts.next()
}

/// Strip manufacturer prefix from a part name.
///
/// `"DAD_PS.part_barrel_01"` → `"part_barrel_01"`, `"part_body"` → `"part_body"`
fn normalize_part_name(name: &str) -> &str {
    name.split('.').next_back().unwrap_or(name)
}

/// Category ID -> Set of normalized part names known in that category's pool
static PART_POOL_MEMBERS: Lazy<HashMap<i64, HashSet<String>>> = Lazy::new(|| {
    let mut pools: HashMap<i64, HashSet<String>> = HashMap::new();
    for line in PART_POOLS_TSV.lines().skip(1) {
        let mut cols = line.splitn(2, '\t');
        let Some(cat) = cols.next().and_then(|s| s.parse::<i64>().ok()) else {
            continue;
        };
        let Some(name) = cols.next() else { continue };
        pools
            .entry(cat)
            .or_default()
            .insert(normalize_part_name(name).to_string());
    }
    pools
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
    PARTS_BY_ID
        .get(&(category, index))
        .map(|(name, _)| name.as_str())
}

/// Get the slot (vertical) name for a part by category and index
pub fn part_slot(category: i64, index: i64) -> Option<&'static str> {
    PARTS_BY_ID
        .get(&(category, index))
        .map(|(_, slot)| slot.as_str())
}

/// Get a manufacturer's full name from its code
pub fn manufacturer_name(code: &str) -> Option<&'static str> {
    MANUFACTURERS.get(code).map(|s| s.as_str())
}

/// Get drop pool data for a (manufacturer, gear_type) pair
pub fn drop_pool(manufacturer_code: &str, gear_type_code: &str) -> Option<&'static DropPool> {
    DROP_POOLS.get(&(manufacturer_code.to_string(), gear_type_code.to_string()))
}

/// Check if a part name exists in the known pool for a category.
///
/// Names are normalized (manufacturer prefix stripped) before comparison.
/// Returns `None` if the category has no pool data, `Some(bool)` otherwise.
pub fn is_part_in_pool(category: i64, name: &str) -> Option<bool> {
    let pool = PART_POOL_MEMBERS.get(&category)?;
    Some(pool.contains(normalize_part_name(name)))
}

/// Get the total number of legendaries in a world drop pool (e.g., all "Pistols")
pub fn world_pool_legendary_count(world_pool_name: &str) -> u32 {
    DROP_POOLS
        .values()
        .filter(|p| p.world_pool_name == world_pool_name)
        .map(|p| p.legendary_count)
        .sum()
}

/// Get the display name for a boss by its internal name
pub fn boss_display_name(internal_name: &str) -> Option<&'static str> {
    BOSS_NAMES.get(internal_name).map(|s| s.as_str())
}

/// Get all boss name mappings (internal_name -> display_name)
pub fn all_boss_names() -> &'static HashMap<String, String> {
    &BOSS_NAMES
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

/// Known slot prefixes for `part_*` names, ordered longest-first for matching.
const SLOT_PREFIXES: &[&str] = &[
    "secondary_elem",
    "secondary_ammo",
    "body_element",
    "body_armor",
    "body_bolt",
    "body_energy",
    "body_mag",
    "barrel",
    "body",
    "firmware",
    "foregrip",
    "grip",
    "mag",
    "multi",
    "passive",
    "scope",
    "secondary",
    "shield",
    "stat2",
    "stat3",
    "stat",
    "underbarrel",
    "unique",
];

/// Extract slot name from a manifest part name.
///
/// Matches against known slot prefixes after stripping manufacturer prefix
/// and `part_`. For `comp_*` / `base_comp_*` parts, returns `"rarity"`.
/// For bare element names (fire, cryo, etc.), returns `"element"`.
///
/// Examples:
/// - `"DAD_PS.part_barrel_02_finnty"` → `"barrel"`
/// - `"part_stat2_wt_ps_equipspeed"` → `"stat2"`
/// - `"part_body_b"` → `"body"`
/// - `"comp_05_legendary_stopgap"` → `"rarity"`
/// - `"radiation"` → `"element"`
pub fn slot_from_part_name(name: &str) -> &'static str {
    let segment = name.split('.').next_back().unwrap_or(name);

    if segment.starts_with("comp_") || segment.starts_with("base_comp_") {
        return "rarity";
    }

    match segment {
        "fire" | "cryo" | "shock" | "corrosive" | "radiation" | "sonic" => return "element",
        _ => {}
    }

    if segment.starts_with("exosoldier_") {
        return "class_mod";
    }

    let stripped = match segment.strip_prefix("part_") {
        Some(rest) => rest,
        None => return "unknown",
    };

    for prefix in SLOT_PREFIXES {
        if stripped.starts_with(prefix) {
            let rest = &stripped[prefix.len()..];
            if rest.is_empty() || rest.starts_with('_') {
                return prefix;
            }
        }
    }

    "unknown"
}

/// Category ID -> Maximum known part index in that category
static MAX_PART_INDEX: Lazy<HashMap<i64, i64>> = Lazy::new(|| {
    let mut max_by_cat: HashMap<i64, i64> = HashMap::new();
    for &(cat, idx) in PARTS_BY_ID.keys() {
        let entry = max_by_cat.entry(cat).or_insert(0);
        if idx > *entry {
            *entry = idx;
        }
    }
    max_by_cat
});

/// Get the maximum known part index for a category.
/// Returns None if the category has no parts in the manifest.
pub fn max_part_index(category: i64) -> Option<i64> {
    MAX_PART_INDEX.get(&category).copied()
}

/// Get the number of known parts for a category.
pub fn category_part_count(category: i64) -> usize {
    PARTS_BY_ID.keys().filter(|(cat, _)| *cat == category).count()
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
    fn test_max_part_index() {
        // Categories with parts should return Some
        // Category 2 = Daedalus Pistol, should have parts
        let max = max_part_index(2);
        assert!(max.is_some());
        assert!(max.unwrap() > 0);

        // Non-existent category returns None
        assert!(max_part_index(99999).is_none());
    }

    #[test]
    fn test_category_part_count() {
        // Category with parts should have non-zero count
        let count = category_part_count(2);
        assert!(count > 0);

        // Non-existent category returns 0
        assert_eq!(category_part_count(99999), 0);
    }

    #[test]
    fn test_part_slot() {
        // Category 2 (Daedalus Pistol) should have slot info for its parts
        if let Some(slot) = part_slot(2, 1) {
            assert!(!slot.is_empty());
        }
    }

    #[test]
    fn test_slot_from_part_name() {
        // Basic slots
        assert_eq!(slot_from_part_name("DAD_PS.part_barrel_01"), "barrel");
        assert_eq!(slot_from_part_name("part_barrel_02_finnty"), "barrel");
        assert_eq!(slot_from_part_name("part_barrel_licensed_ted_shooting"), "barrel");
        assert_eq!(slot_from_part_name("part_scope_02"), "scope");
        assert_eq!(slot_from_part_name("part_body"), "body");
        assert_eq!(slot_from_part_name("part_body_b"), "body");
        assert_eq!(slot_from_part_name("part_body_mag_sg"), "body_mag");
        assert_eq!(slot_from_part_name("JAK_SG.part_foregrip_03"), "foregrip");
        assert_eq!(slot_from_part_name("part_mag_1"), "mag");
        assert_eq!(slot_from_part_name("part_barrel"), "barrel");
        assert_eq!(slot_from_part_name("part_grip_04_hyp"), "grip");

        // Stat mods
        assert_eq!(slot_from_part_name("part_stat2_wt_ps_equipspeed"), "stat2");
        assert_eq!(slot_from_part_name("part_stat3_statuseffect_chance"), "stat3");

        // Rarity / comp
        assert_eq!(slot_from_part_name("comp_05_legendary_stopgap"), "rarity");
        assert_eq!(slot_from_part_name("base_comp_02_uncommon"), "rarity");
        assert_eq!(slot_from_part_name("comp_03_rare"), "rarity");

        // Elements
        assert_eq!(slot_from_part_name("radiation"), "element");
        assert_eq!(slot_from_part_name("cryo"), "element");

        // Other
        assert_eq!(slot_from_part_name("part_firmware_baker"), "firmware");
        assert_eq!(slot_from_part_name("part_passive_blue_3_1_tier_1"), "passive");
        assert_eq!(slot_from_part_name("part_secondary_ammo_sg"), "secondary_ammo");
        assert_eq!(slot_from_part_name("part_secondary_elem_cryo_fire"), "secondary_elem");
        assert_eq!(slot_from_part_name("part_shield_ammo"), "shield");
        assert_eq!(slot_from_part_name("part_underbarrel_04_atlas_ball"), "underbarrel");
    }

    #[test]
    fn test_normalize_part_name() {
        assert_eq!(normalize_part_name("DAD_PS.part_barrel_01"), "part_barrel_01");
        assert_eq!(normalize_part_name("part_body"), "part_body");
        assert_eq!(normalize_part_name("comp_01_common"), "comp_01_common");
        assert_eq!(normalize_part_name("BOR_REPAIR_KIT.part_borg"), "part_borg");
    }

    #[test]
    fn test_is_part_in_pool_known_category() {
        // Category 2 (Daedalus Pistol) should have pool data
        let result = is_part_in_pool(2, "part_barrel_01");
        assert!(result.is_some(), "Category 2 should have pool data");
    }

    #[test]
    fn test_is_part_in_pool_unknown_category() {
        assert!(is_part_in_pool(99999, "part_body").is_none());
    }

    #[test]
    fn test_is_part_in_pool_normalizes_prefix() {
        // Should find prefixed names by stripping the prefix
        let result = is_part_in_pool(2, "DAD_PS.part_barrel_01");
        assert!(result.is_some());
        // The normalized form "part_barrel_01" should be in the pool
        if let Some(found) = result {
            assert!(found, "DAD_PS.part_barrel_01 should be in category 2 pool");
        }
    }

    #[test]
    fn test_part_pool_stats() {
        // Verify pool data loaded with reasonable counts
        let total_categories = PART_POOL_MEMBERS.len();
        assert!(total_categories > 50, "Expected 50+ categories, got {}", total_categories);
    }
}
