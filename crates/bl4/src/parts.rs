//! Parts database lookup for Borderlands 4 items
//!
//! Maps part indices from item serials to human-readable names.
//!
//! ## Naming System
//!
//! The parts database is primarily a **weapon naming table**. Each part index
//! maps to a naming entry that determines weapon prefixes:
//!
//! - **Part index** (e.g., `{4}` = ReloadSpeed) identifies the primary modifier
//! - **Stats keys** (e.g., `modd`, `accuracy`) are secondary modifiers
//! - **The name value** is the prefix shown on the weapon (e.g., "Cursed", "Rotten")
//!
//! Example: A weapon with part `{8}` (body_mod_b) and secondary `modd` gets
//! the prefix "Cursed" → "Cursed Linebacker"

use std::collections::HashMap;

/// Part information from the database
#[derive(Debug, Clone)]
pub struct PartInfo {
    /// Part index (matches serial Part token index)
    pub index: u64,
    /// Part name - identifies the modifier type (e.g., "Damage", "body_mod_b")
    pub name: String,
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

/// Parts database with index-to-name lookup
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

    /// Load parts from embedded data
    /// This uses the built-in parts data extracted from game files
    pub fn load_embedded() -> Self {
        let mut db = Self::new();

        // Core stat modifiers (indices 2-5)
        db.insert(2, "Damage");
        db.insert(3, "CritDamage");
        db.insert(4, "ReloadSpeed");
        db.insert(5, "MagSize");

        // Body mods (indices 7-13)
        db.insert(7, "body_mod_a");
        db.insert(8, "body_mod_b");
        db.insert(9, "body_mod_c");
        db.insert(10, "body_mod_d");
        db.insert(11, "body_mod_a+b");
        db.insert(12, "body_mod_a+c");
        db.insert(13, "body_mod_b+c");

        // Barrel mods (indices 15-22)
        db.insert(15, "barrel_mod_a");
        db.insert(16, "barrel_mod_b");
        db.insert(17, "barrel_mod_c");
        db.insert(18, "barrel_mod_d");
        db.insert(19, "barrel_mod_a+b");
        db.insert(20, "barrel_mod_a+c");
        db.insert(21, "barrel_mod_b+c");
        db.insert(22, "barrel_mod_a+d");

        // Common manufacturer-specific parts
        db.insert(100, "TOR_Barrel_01");
        db.insert(101, "TOR_Barrel_02");
        db.insert(137, "part_137");
        db.insert(198, "body_mod_c");
        db.insert(199, "body_mod_d");
        db.insert(207, "part_207");
        db.insert(500, "ORD_Mag_02");

        db
    }

    /// Insert a part into the database
    pub fn insert(&mut self, index: u64, name: &str) {
        self.parts.insert(
            index,
            PartInfo {
                index,
                name: name.to_string(),
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
    fn test_parts_database() {
        let db = PartsDatabase::load_embedded();
        assert!(!db.is_empty());
        assert_eq!(db.get_name(2), "Damage");
        assert_eq!(db.get_name(99999), "part_99999");
    }
}
