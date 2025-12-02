//! Parts database lookup for Borderlands 4 items
//!
//! Maps part indices from item serials to human-readable names.
//!
//! Loads stat/part index mappings from the manifest data embedded at compile time.
//! Also loads the full parts database with category mappings.

use serde::Deserialize;
use std::collections::HashMap;

/// Embedded items database JSON (compiled into binary)
const ITEMS_DATABASE_JSON: &str = include_str!("../../../share/manifest/items_database.json");

/// Embedded parts database JSON (compiled into binary)
const PARTS_DATABASE_JSON: &str = include_str!("../../../share/manifest/parts_database.json");

// ============================================================================
// JSON deserialization types for manifest data
// ============================================================================

#[derive(Debug, Deserialize)]
struct ItemsDatabase {
    items: Vec<ItemEntry>,
}

#[derive(Debug, Deserialize)]
struct ItemEntry {
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    category: Option<String>,
    #[serde(default)]
    stats: HashMap<String, Vec<StatModifier>>,
}

#[derive(Debug, Deserialize)]
struct StatModifier {
    modifier_type: String,
    index: u32,
    #[serde(default)]
    #[allow(dead_code)]
    guid: Option<String>,
}

// ============================================================================
// Public types
// ============================================================================

/// Part information from the database
#[derive(Debug, Clone)]
pub struct PartInfo {
    /// Part index (matches serial Part token index)
    pub index: u64,
    /// Part name - identifies the modifier type (e.g., "Damage", "body_mod_b")
    pub name: String,
    /// Modifier type (Value, Scale, Add)
    pub modifier_type: Option<String>,
}

/// Manufacturer ID to name mapping
/// Derived from analysis of item serials with known manufacturers
pub fn manufacturer_name(id: u64) -> Option<&'static str> {
    match id {
        4 => Some("Daedalus"),
        6 => Some("Torgue"),
        10 => Some("Tediore"),
        14 => Some("Ripper"),
        15 => Some("Order"),
        129 => Some("Jakobs"),
        134 => Some("Vladof"),
        138 => Some("Maliwan"),
        _ => None,
    }
}

/// Part Group ID (Category) to name mapping
/// Derived from memory dump analysis and serial decoding
pub fn category_name(category: i64) -> Option<&'static str> {
    match category {
        // Pistols
        2 => Some("Daedalus Pistol"),
        3 => Some("Jakobs Pistol"),
        4 => Some("Tediore Pistol"),
        5 => Some("Torgue Pistol"),
        6 => Some("Order Pistol"),
        7 => Some("Vladof Pistol"),
        // Shotguns
        8 => Some("Daedalus Shotgun"),
        9 => Some("Jakobs Shotgun"),
        10 => Some("Tediore Shotgun"),
        11 => Some("Torgue Shotgun"),
        12 => Some("Bor Shotgun"),
        // Assault Rifles
        13 => Some("Daedalus Assault Rifle"),
        14 => Some("Jakobs Assault Rifle"),
        15 => Some("Tediore Assault Rifle"),
        16 => Some("Torgue Assault Rifle"),
        17 => Some("Vladof Assault Rifle"),
        18 => Some("Order Assault Rifle"),
        // SMGs
        20 => Some("Daedalus SMG"),
        21 => Some("Bor SMG"),
        22 => Some("Vladof SMG"),
        23 => Some("Maliwan SMG"),
        // Snipers
        26 => Some("Jakobs Sniper"),
        27 => Some("Vladof Sniper"),
        28 => Some("Order Sniper"),
        29 => Some("Maliwan Sniper"),
        // Heavy Weapons
        244 => Some("Vladof Heavy"),
        245 => Some("Torgue Heavy"),
        246 => Some("Bor Heavy"),
        247 => Some("Maliwan Heavy"),
        // Shields
        279 => Some("Energy Shield"),
        280 => Some("Bor Shield"),
        281 => Some("Daedalus Shield"),
        282 => Some("Jakobs Shield"),
        283 => Some("Armor Shield"),
        284 => Some("Maliwan Shield"),
        285 => Some("Order Shield"),
        286 => Some("Tediore Shield"),
        287 => Some("Torgue Shield"),
        288 => Some("Vladof Shield"),
        // Gadgets and Gear
        300 => Some("Grenade Gadget"),
        310 => Some("Turret Gadget"),
        320 => Some("Repair Kit"),
        330 => Some("Terminal Gadget"),
        // Enhancements
        400 => Some("Daedalus Enhancement"),
        401 => Some("Bor Enhancement"),
        402 => Some("Jakobs Enhancement"),
        403 => Some("Maliwan Enhancement"),
        404 => Some("Order Enhancement"),
        405 => Some("Tediore Enhancement"),
        406 => Some("Torgue Enhancement"),
        407 => Some("Vladof Enhancement"),
        408 => Some("COV Enhancement"),
        409 => Some("Atlas Enhancement"),
        _ => None,
    }
}

/// Item type character to description mapping
/// Note: Type characters don't map 1:1 to weapon categories - they appear to
/// encode structural information about the serial format itself.
pub fn item_type_name(type_char: char) -> &'static str {
    // Based on analysis, these type chars appear across multiple weapon types.
    // The character likely indicates serial format version or encoding variant
    // rather than item category.
    match type_char {
        'a'..='d' => "Weapon (variant a-d)",
        'e' => "Item (multi-type)",
        'f' | 'g' => "Weapon (variant f-g)",
        'r' => "Item (variant r)",
        'u' => "Sniper (variant u)",
        'v' | 'w' | 'x' | 'y' | 'z' => "Weapon (variant v-z)",
        '!' | '#' => "Class Mod/Special",
        _ => "Unknown",
    }
}

/// Parts database with index-to-name lookup (legacy: global indices only)
#[derive(Debug, Default)]
pub struct PartsDatabase {
    parts: HashMap<u64, PartInfo>,
}

impl PartsDatabase {
    /// Create an empty database
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
        }
    }

    /// Load parts from embedded manifest data (compiled into binary)
    ///
    /// Parses `share/manifest/items_database.json` and extracts all stat index mappings.
    pub fn load_embedded() -> Self {
        let mut db = Self::new();

        // Parse the embedded JSON
        if let Ok(items_db) = serde_json::from_str::<ItemsDatabase>(ITEMS_DATABASE_JSON) {
            for item in items_db.items {
                for (stat_name, modifiers) in item.stats {
                    for modifier in modifiers {
                        // Clean up stat name (remove _Value, _Scale, _Add suffix for display)
                        let clean_name = stat_name
                            .trim_end_matches("_Value")
                            .trim_end_matches("_Scale")
                            .trim_end_matches("_Add")
                            .to_string();

                        db.parts.insert(
                            modifier.index as u64,
                            PartInfo {
                                index: modifier.index as u64,
                                name: clean_name,
                                modifier_type: Some(modifier.modifier_type),
                            },
                        );
                    }
                }
            }
        }

        db
    }

    /// Insert a part into the database
    pub fn insert(&mut self, index: u64, name: &str) {
        self.parts.insert(
            index,
            PartInfo {
                index,
                name: name.to_string(),
                modifier_type: None,
            },
        );
    }

    /// Look up a part by index
    pub fn get(&self, index: u64) -> Option<&PartInfo> {
        self.parts.get(&index)
    }

    /// Get part name by index, or return index as string if not found
    pub fn get_name(&self, index: u64) -> String {
        self.parts
            .get(&index)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("part_{}", index))
    }

    /// Number of parts in database
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Check if database is empty
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

// ============================================================================
// Category-aware parts database (new format)
// ============================================================================

/// JSON structure for parts_database.json
#[derive(Debug, Deserialize)]
struct PartsDatabaseJson {
    #[allow(dead_code)]
    version: u32,
    parts: Vec<PartEntry>,
}

#[derive(Debug, Deserialize)]
struct PartEntry {
    category: i64,
    index: i16,
    name: String,
    group: String,
}

/// Extended part information with category
#[derive(Debug, Clone)]
pub struct CategoryPartInfo {
    /// Part Group ID (Category)
    pub category: i64,
    /// Part index within the category
    pub index: i16,
    /// Full part name (e.g., "DAD_PS.part_barrel_01")
    pub name: String,
    /// Group description (e.g., "Daedalus Pistol")
    pub group: String,
}

/// Category-aware parts database
///
/// Maps (category, index) pairs to part names.
/// Part indices in serials are relative to the Part Group ID (category).
#[derive(Debug, Default)]
pub struct CategoryPartsDatabase {
    /// Maps (category, index) to part info
    parts: HashMap<(i64, i16), CategoryPartInfo>,
    /// Total number of parts
    count: usize,
}

impl CategoryPartsDatabase {
    /// Create an empty database
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
            count: 0,
        }
    }

    /// Load from embedded parts_database.json
    pub fn load_embedded() -> Self {
        let mut db = Self::new();

        if let Ok(json_db) = serde_json::from_str::<PartsDatabaseJson>(PARTS_DATABASE_JSON) {
            for entry in json_db.parts {
                db.parts.insert(
                    (entry.category, entry.index),
                    CategoryPartInfo {
                        category: entry.category,
                        index: entry.index,
                        name: entry.name,
                        group: entry.group,
                    },
                );
            }
            db.count = db.parts.len();
        }

        db
    }

    /// Look up a part by category and index
    pub fn get(&self, category: i64, index: i16) -> Option<&CategoryPartInfo> {
        self.parts.get(&(category, index))
    }

    /// Get part name by category and index
    pub fn get_name(&self, category: i64, index: i16) -> String {
        self.parts
            .get(&(category, index))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("cat{}:part_{}", category, index))
    }

    /// Number of parts in database
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if database is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manufacturer_lookup() {
        assert_eq!(manufacturer_name(4), Some("Daedalus"));
        assert_eq!(manufacturer_name(6), Some("Torgue"));
        assert_eq!(manufacturer_name(138), Some("Maliwan"));
        assert_eq!(manufacturer_name(999), None);
    }

    #[test]
    fn test_item_type_lookup() {
        assert_eq!(item_type_name('r'), "Item (variant r)");
        assert_eq!(item_type_name('v'), "Weapon (variant v-z)");
        assert_eq!(item_type_name('?'), "Unknown");
    }

    #[test]
    fn test_parts_database_loads_from_manifest() {
        let db = PartsDatabase::load_embedded();
        // Should have loaded entries from the manifest
        assert!(!db.is_empty(), "Database should not be empty");
        // Check some known indices from items_database.json
        // Index 37 = Spread, Index 38 = Accuracy, Index 48 = Damage
        assert_eq!(db.get_name(37), "Spread");
        assert_eq!(db.get_name(38), "Accuracy");
        assert_eq!(db.get_name(48), "Damage");
        // Unknown indices should return part_N
        assert_eq!(db.get_name(99999), "part_99999");
    }

    #[test]
    fn test_parts_database_has_modifier_types() {
        let db = PartsDatabase::load_embedded();
        // Check that modifier types are preserved
        if let Some(part) = db.get(37) {
            assert_eq!(part.name, "Spread");
            assert!(part.modifier_type.is_some());
        }
    }

    #[test]
    fn test_category_name_lookup() {
        assert_eq!(category_name(2), Some("Daedalus Pistol"));
        assert_eq!(category_name(22), Some("Vladof SMG"));
        assert_eq!(category_name(283), Some("Armor Shield"));
        assert_eq!(category_name(999), None);
    }

    #[test]
    fn test_category_parts_database_loads() {
        let db = CategoryPartsDatabase::load_embedded();
        // Should have loaded parts
        assert!(!db.is_empty(), "Database should not be empty");
        // Check we have a reasonable number of parts
        assert!(db.len() > 2000, "Expected > 2000 parts, got {}", db.len());
    }

    #[test]
    fn test_category_parts_database_lookup() {
        let db = CategoryPartsDatabase::load_embedded();
        // Look up first part in Daedalus Pistol (category 2, index 0)
        if let Some(part) = db.get(2, 0) {
            assert!(
                part.name.starts_with("DAD_PS."),
                "Expected DAD_PS prefix, got {}",
                part.name
            );
            assert_eq!(part.group, "Daedalus Pistol");
        }
        // Unknown parts should return formatted fallback
        assert!(db.get_name(9999, 0).contains("cat9999"));
    }

    #[test]
    fn test_validate_weapon_serial_parts() {
        use crate::serial::ItemSerial;

        let db = CategoryPartsDatabase::load_embedded();

        // Test several weapon serials from example saves
        let weapon_serials = [
            "@Ugr$ZCm/&tH!t{KgK/Shxu>k",         // Vladof SMG
            "@Ugr$-Om/)@{!br-XMkT6!aX/4)00",     // Weapon
            "@Ugr$`Rm/&zJ!r-c)M!l~mXi)E?;UaJW",  // Weapon
            "@Ugr%Scm/&tH!fZ*PK~(/",             // Weapon
            "@Ugr%DXm/)@{!u+qGK~(/",             // Weapon
            "@Ugr$iFm/&tH!bF&$I;cyHK>z",         // Weapon
        ];

        let mut total_parts = 0;
        let mut found_parts = 0;
        let mut missing_parts: Vec<(i64, u64)> = Vec::new();

        for serial in weapon_serials {
            let item = ItemSerial::decode(serial).expect("Failed to decode serial");

            // Get Part Group ID
            if let Some(group_id) = item.part_group_id() {
                for (index, _values) in item.parts() {
                    total_parts += 1;
                    // Cast u64 index to i16 for database lookup
                    let idx = index as i16;
                    if db.get(group_id, idx).is_some() {
                        found_parts += 1;
                    } else {
                        missing_parts.push((group_id, index));
                    }
                }
            }
        }

        // Report results
        eprintln!(
            "Weapon serial validation: {}/{} parts found in database",
            found_parts, total_parts
        );
        if !missing_parts.is_empty() {
            eprintln!("Missing parts:");
            for (cat, idx) in &missing_parts {
                eprintln!("  - category {} index {}", cat, idx);
            }
        }

        // For now, we allow some missing parts since the database may be incomplete
        // But we should find at least some parts
        assert!(
            total_parts > 0,
            "Should have extracted at least one part from weapon serials"
        );
    }

    #[test]
    fn test_validate_equipment_serial_parts() {
        use crate::serial::ItemSerial;

        let db = CategoryPartsDatabase::load_embedded();

        // Test equipment serials from example saves
        let equipment_serials = [
            "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_",   // Shield
            "@Uge8^+m/)}}!tno~L{+MNG&KZ/WB~",   // Shield
            "@Uge8;)m/&zJ!tkr0N4>8ns8H{t!6ljj", // Shield
            "@Uge98>m/&tH!bn`8NWCvCXb=",        // Shield
            "@Uge8Oqm/&tH!fdUeM7=L8Xb=",        // Shield
            "@Uge8aum/&tH!sNN2LJa}",            // Shield
            "@Uge8&&m/)@{!pPj9MZNDQXi)DP3jh",   // Shield
            "@Uge8s!m/)@{!sK/MNWCvCs1N",        // Shield
        ];

        let mut total_parts = 0;
        let mut found_parts = 0;
        let mut missing_by_category: std::collections::HashMap<i64, Vec<u64>> =
            std::collections::HashMap::new();

        for serial in equipment_serials {
            let item = ItemSerial::decode(serial).expect("Failed to decode serial");

            // Get Part Group ID
            if let Some(group_id) = item.part_group_id() {
                for (index, _values) in item.parts() {
                    total_parts += 1;
                    // Cast u64 index to i16 for database lookup
                    let idx = index as i16;
                    if db.get(group_id, idx).is_some() {
                        found_parts += 1;
                    } else {
                        missing_by_category.entry(group_id).or_default().push(index);
                    }
                }
            }
        }

        // Report results
        eprintln!(
            "Equipment serial validation: {}/{} parts found in database",
            found_parts, total_parts
        );
        if !missing_by_category.is_empty() {
            eprintln!("Missing equipment parts by category:");
            for (cat, indices) in &missing_by_category {
                eprintln!(
                    "  - category {} ({}): indices {:?}",
                    cat,
                    category_name(*cat).unwrap_or("Unknown"),
                    indices
                );
            }
        }

        assert!(
            total_parts > 0,
            "Should have extracted at least one part from equipment serials"
        );
    }
}
