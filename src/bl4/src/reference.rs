//! Reference data for Borderlands 4 items
//!
//! Hardcoded reference data for game concepts like rarities, elements,
//! weapon types, manufacturers, and gear types. This data is used for
//! display and categorization purposes.

use std::collections::HashMap;

// ============================================================================
// Rarity
// ============================================================================

/// Rarity tier information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RarityTier {
    pub tier: u8,
    pub code: &'static str,
    pub name: &'static str,
    pub color: &'static str,
}

/// All rarity tiers in order
pub const RARITY_TIERS: &[RarityTier] = &[
    RarityTier {
        tier: 1,
        code: "comp_01",
        name: "Common",
        color: "#FFFFFF",
    },
    RarityTier {
        tier: 2,
        code: "comp_02",
        name: "Uncommon",
        color: "#00FF00",
    },
    RarityTier {
        tier: 3,
        code: "comp_03",
        name: "Rare",
        color: "#0080FF",
    },
    RarityTier {
        tier: 4,
        code: "comp_04",
        name: "Epic",
        color: "#A020F0",
    },
    RarityTier {
        tier: 5,
        code: "comp_05",
        name: "Legendary",
        color: "#FFA500",
    },
];

/// Get rarity tier by tier number
pub fn rarity_by_tier(tier: u8) -> Option<&'static RarityTier> {
    RARITY_TIERS.iter().find(|r| r.tier == tier)
}

/// Get rarity tier by code
pub fn rarity_by_code(code: &str) -> Option<&'static RarityTier> {
    RARITY_TIERS.iter().find(|r| r.code == code)
}

// ============================================================================
// Elements
// ============================================================================

/// Element type information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementType {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub color: &'static str,
}

/// All element types
pub const ELEMENT_TYPES: &[ElementType] = &[
    ElementType {
        code: "kinetic",
        name: "Impact",
        description: "Non-elemental kinetic damage",
        color: "#808080",
    },
    ElementType {
        code: "fire",
        name: "Fire",
        description: "Incendiary damage, effective vs flesh",
        color: "#FF4500",
    },
    ElementType {
        code: "shock",
        name: "Electric",
        description: "Shock damage, effective vs shields",
        color: "#00BFFF",
    },
    ElementType {
        code: "corrosive",
        name: "Corrosive",
        description: "Acid damage, effective vs armor",
        color: "#32CD32",
    },
    ElementType {
        code: "cryo",
        name: "Cryo",
        description: "Freezing damage, slows and can freeze enemies",
        color: "#ADD8E6",
    },
    ElementType {
        code: "radiation",
        name: "Radiation",
        description: "Radiation damage, spreads to nearby enemies",
        color: "#FFFF00",
    },
];

/// Get element by code
pub fn element_by_code(code: &str) -> Option<&'static ElementType> {
    ELEMENT_TYPES.iter().find(|e| e.code == code)
}

// ============================================================================
// Weapon Types
// ============================================================================

/// Weapon type information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponType {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// All weapon types
pub const WEAPON_TYPES: &[WeaponType] = &[
    WeaponType {
        code: "AR",
        name: "Assault Rifle",
        description: "Full-auto/burst fire rifles",
    },
    WeaponType {
        code: "HW",
        name: "Heavy Weapon",
        description: "Launchers and miniguns",
    },
    WeaponType {
        code: "PS",
        name: "Pistol",
        description: "Semi-auto and full-auto handguns",
    },
    WeaponType {
        code: "SG",
        name: "Shotgun",
        description: "High-damage spread weapons",
    },
    WeaponType {
        code: "SM",
        name: "SMG",
        description: "Submachine guns",
    },
    WeaponType {
        code: "SR",
        name: "Sniper Rifle",
        description: "Long-range precision weapons",
    },
];

/// Get weapon type by code
pub fn weapon_type_by_code(code: &str) -> Option<&'static WeaponType> {
    WEAPON_TYPES.iter().find(|w| w.code == code)
}

// ============================================================================
// Manufacturers
// ============================================================================

/// Manufacturer information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manufacturer {
    pub code: &'static str,
    pub name: &'static str,
    pub weapon_types: &'static [&'static str],
    pub style: &'static str,
}

/// All manufacturers
pub const MANUFACTURERS: &[Manufacturer] = &[
    Manufacturer {
        code: "BOR",
        name: "Borg",
        weapon_types: &["SM", "SG", "HW", "SR"],
        style: "Cult/organic aesthetics",
    },
    Manufacturer {
        code: "DAD",
        name: "Daedalus",
        weapon_types: &["AR", "SM", "PS", "SG"],
        style: "High-tech precision",
    },
    Manufacturer {
        code: "JAK",
        name: "Jakobs",
        weapon_types: &["AR", "PS", "SG", "SR"],
        style: "Old West, semi-auto, high damage per shot",
    },
    Manufacturer {
        code: "MAL",
        name: "Maliwan",
        weapon_types: &["SM", "SG", "SR", "HW"],
        style: "Elemental weapons, energy-based",
    },
    Manufacturer {
        code: "ORD",
        name: "Order",
        weapon_types: &["AR", "PS", "SR"],
        style: "Military precision",
    },
    Manufacturer {
        code: "RIP",
        name: "Ripper",
        weapon_types: &["SG", "SR"],
        style: "Aggressive, high-damage",
    },
    Manufacturer {
        code: "TED",
        name: "Tediore",
        weapon_types: &["AR", "PS", "SG", "SM"],
        style: "Disposable, thrown on reload",
    },
    Manufacturer {
        code: "TOR",
        name: "Torgue",
        weapon_types: &["AR", "PS", "SG", "HW"],
        style: "Explosive/gyrojet rounds",
    },
    Manufacturer {
        code: "VLA",
        name: "Vladof",
        weapon_types: &["AR", "SM", "SR", "HW"],
        style: "High fire rate, large magazines",
    },
    Manufacturer {
        code: "GRV",
        name: "Gravitar",
        weapon_types: &[],
        style: "Class mods manufacturer",
    },
];

/// Get manufacturer by code
pub fn manufacturer_by_code(code: &str) -> Option<&'static Manufacturer> {
    MANUFACTURERS.iter().find(|m| m.code == code)
}

/// Get manufacturer name by code (convenience function)
pub fn manufacturer_name_by_code(code: &str) -> Option<&'static str> {
    manufacturer_by_code(code).map(|m| m.name)
}

// ============================================================================
// Gear Types
// ============================================================================

/// Gear type information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearType {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// All gear types
pub const GEAR_TYPES: &[GearType] = &[
    GearType {
        code: "shield",
        name: "Shield",
        description: "Defensive equipment",
    },
    GearType {
        code: "classmod",
        name: "Class Mod",
        description: "Character class modifications",
    },
    GearType {
        code: "enhancement",
        name: "Enhancement",
        description: "Permanent character upgrades",
    },
    GearType {
        code: "gadget",
        name: "Gadget",
        description: "Deployable equipment",
    },
    GearType {
        code: "repair_kit",
        name: "Repair Kit",
        description: "Healing items",
    },
    GearType {
        code: "grenade",
        name: "Grenade",
        description: "Throwable explosive devices",
    },
];

/// Get gear type by code
pub fn gear_type_by_code(code: &str) -> Option<&'static GearType> {
    GEAR_TYPES.iter().find(|g| g.code == code)
}

// ============================================================================
// Stats
// ============================================================================

/// Get stat description by name
pub fn stat_description(stat: &str) -> Option<&'static str> {
    match stat {
        "Damage" => Some("Base damage"),
        "CritDamage" => Some("Critical hit damage"),
        "FireRate" => Some("Firing rate"),
        "ReloadTime" => Some("Reload time"),
        "MagSize" => Some("Magazine size"),
        "Accuracy" => Some("Base accuracy"),
        "AccImpulse" => Some("Accuracy impulse (recoil recovery)"),
        "AccRegen" => Some("Accuracy regeneration"),
        "AccDelay" => Some("Accuracy delay"),
        "Spread" => Some("Projectile spread"),
        "Recoil" => Some("Weapon recoil"),
        "Sway" => Some("Weapon sway"),
        "ProjectilesPerShot" => Some("Pellets per shot"),
        "AmmoCost" => Some("Ammo consumption"),
        "StatusChance" => Some("Status effect chance"),
        "StatusDamage" => Some("Status effect damage"),
        "EquipTime" => Some("Weapon equip time"),
        "PutDownTime" => Some("Weapon holster time"),
        "ZoomDuration" => Some("ADS zoom time"),
        "ElementalPower" => Some("Elemental damage bonus"),
        "DamageRadius" => Some("Splash damage radius"),
        _ => None,
    }
}

/// Get all stat descriptions as a HashMap
pub fn all_stat_descriptions() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("Damage", "Base damage");
    m.insert("CritDamage", "Critical hit damage");
    m.insert("FireRate", "Firing rate");
    m.insert("ReloadTime", "Reload time");
    m.insert("MagSize", "Magazine size");
    m.insert("Accuracy", "Base accuracy");
    m.insert("AccImpulse", "Accuracy impulse (recoil recovery)");
    m.insert("AccRegen", "Accuracy regeneration");
    m.insert("AccDelay", "Accuracy delay");
    m.insert("Spread", "Projectile spread");
    m.insert("Recoil", "Weapon recoil");
    m.insert("Sway", "Weapon sway");
    m.insert("ProjectilesPerShot", "Pellets per shot");
    m.insert("AmmoCost", "Ammo consumption");
    m.insert("StatusChance", "Status effect chance");
    m.insert("StatusDamage", "Status effect damage");
    m.insert("EquipTime", "Weapon equip time");
    m.insert("PutDownTime", "Weapon holster time");
    m.insert("ZoomDuration", "ADS zoom time");
    m.insert("ElementalPower", "Elemental damage bonus");
    m.insert("DamageRadius", "Splash damage radius");
    m
}

// ============================================================================
// Known Legendaries
// ============================================================================

/// Known legendary item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendaryItem {
    pub internal: &'static str,
    pub name: &'static str,
    pub weapon_type: &'static str,
    pub manufacturer: &'static str,
}

/// Known legendary items
pub const KNOWN_LEGENDARIES: &[LegendaryItem] = &[
    // Daedalus
    LegendaryItem {
        internal: "DAD_AR.comp_05_legendary_OM",
        name: "OM",
        weapon_type: "AR",
        manufacturer: "DAD",
    },
    LegendaryItem {
        internal: "DAD_AR_Lumberjack",
        name: "Lumberjack",
        weapon_type: "AR",
        manufacturer: "DAD",
    },
    LegendaryItem {
        internal: "DAD_SG.comp_05_legendary_HeartGUn",
        name: "Heart Gun",
        weapon_type: "SG",
        manufacturer: "DAD",
    },
    LegendaryItem {
        internal: "DAD_PS.Zipper",
        name: "Zipper",
        weapon_type: "PS",
        manufacturer: "DAD",
    },
    LegendaryItem {
        internal: "DAD_PS.Rangefinder",
        name: "Rangefinder",
        weapon_type: "PS",
        manufacturer: "DAD",
    },
    LegendaryItem {
        internal: "DAD_SG.Durendal",
        name: "Durendal",
        weapon_type: "SG",
        manufacturer: "DAD",
    },
    // Jakobs
    LegendaryItem {
        internal: "JAK_AR.comp_05_legendary_rowan",
        name: "Rowan's Call",
        weapon_type: "AR",
        manufacturer: "JAK",
    },
    LegendaryItem {
        internal: "JAK_PS.comp_05_legendary_SeventhSense",
        name: "Seventh Sense",
        weapon_type: "PS",
        manufacturer: "JAK",
    },
    LegendaryItem {
        internal: "JAK_PS.comp_05_legendary_kingsgambit",
        name: "King's Gambit",
        weapon_type: "PS",
        manufacturer: "JAK",
    },
    LegendaryItem {
        internal: "JAK_PS.comp_05_legendary_phantom_flame",
        name: "Phantom Flame",
        weapon_type: "PS",
        manufacturer: "JAK",
    },
    LegendaryItem {
        internal: "JAK_SG.comp_05_legendary_RainbowVomit",
        name: "Rainbow Vomit",
        weapon_type: "SG",
        manufacturer: "JAK",
    },
    LegendaryItem {
        internal: "JAK_SR.comp_05_legendary_ballista",
        name: "Ballista",
        weapon_type: "SR",
        manufacturer: "JAK",
    },
    // Maliwan
    LegendaryItem {
        internal: "MAL_HW.comp_05_legendary_GammaVoid",
        name: "Gamma Void",
        weapon_type: "HW",
        manufacturer: "MAL",
    },
    LegendaryItem {
        internal: "MAL_SM.comp_05_legendary_OhmIGot",
        name: "Ohm I Got",
        weapon_type: "SM",
        manufacturer: "MAL",
    },
    // Borg
    LegendaryItem {
        internal: "BOR_SM.comp_05_legendary_p",
        name: "Unknown Borg SMG",
        weapon_type: "SM",
        manufacturer: "BOR",
    },
    // Tediore
    LegendaryItem {
        internal: "TED_AR.comp_05_legendary_Chuck",
        name: "Chuck",
        weapon_type: "AR",
        manufacturer: "TED",
    },
    LegendaryItem {
        internal: "TED_PS.comp_05_legendary_Sideshow",
        name: "Sideshow",
        weapon_type: "PS",
        manufacturer: "TED",
    },
    LegendaryItem {
        internal: "TED_SG.comp_05_legendary_a",
        name: "Unknown Tediore Shotgun",
        weapon_type: "SG",
        manufacturer: "TED",
    },
    // Torgue
    LegendaryItem {
        internal: "TOR_AR.comp_05_legendary_Trogdor",
        name: "Trogdor",
        weapon_type: "AR",
        manufacturer: "TOR",
    },
    LegendaryItem {
        internal: "TOR_HW.comp_05_legendary_ravenfire",
        name: "Ravenfire",
        weapon_type: "HW",
        manufacturer: "TOR",
    },
    LegendaryItem {
        internal: "TOR_SG.comp_05_legendary_Linebacker",
        name: "Linebacker",
        weapon_type: "SG",
        manufacturer: "TOR",
    },
    // Vladof
    LegendaryItem {
        internal: "VLA_AR.comp_05_legendary_WomboCombo",
        name: "Wombo Combo",
        weapon_type: "AR",
        manufacturer: "VLA",
    },
    LegendaryItem {
        internal: "VLA_HW.comp_05_legendary_AtlingGun",
        name: "Atling Gun",
        weapon_type: "HW",
        manufacturer: "VLA",
    },
    LegendaryItem {
        internal: "VLA_SM.comp_05_legendary_KaoSon",
        name: "Kaoson",
        weapon_type: "SM",
        manufacturer: "VLA",
    },
    LegendaryItem {
        internal: "VLA_SR.comp_05_legendary_Vyudazy",
        name: "Vyudazy",
        weapon_type: "SR",
        manufacturer: "VLA",
    },
];

/// Find legendary by internal name
pub fn legendary_by_internal(internal: &str) -> Option<&'static LegendaryItem> {
    KNOWN_LEGENDARIES.iter().find(|l| l.internal == internal)
}

/// Find legendary by display name
pub fn legendary_by_name(name: &str) -> Option<&'static LegendaryItem> {
    KNOWN_LEGENDARIES.iter().find(|l| l.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rarity_lookup() {
        assert_eq!(rarity_by_tier(1).map(|r| r.name), Some("Common"));
        assert_eq!(rarity_by_tier(5).map(|r| r.name), Some("Legendary"));
        assert_eq!(rarity_by_code("comp_03").map(|r| r.name), Some("Rare"));
    }

    #[test]
    fn test_element_lookup() {
        assert_eq!(element_by_code("fire").map(|e| e.name), Some("Fire"));
        assert_eq!(element_by_code("cryo").map(|e| e.name), Some("Cryo"));
    }

    #[test]
    fn test_manufacturer_lookup() {
        assert_eq!(manufacturer_by_code("JAK").map(|m| m.name), Some("Jakobs"));
        assert_eq!(manufacturer_by_code("TOR").map(|m| m.name), Some("Torgue"));
        assert_eq!(manufacturer_name_by_code("VLA"), Some("Vladof"));
    }

    #[test]
    fn test_weapon_type_lookup() {
        assert_eq!(
            weapon_type_by_code("AR").map(|w| w.name),
            Some("Assault Rifle")
        );
        assert_eq!(
            weapon_type_by_code("SR").map(|w| w.name),
            Some("Sniper Rifle")
        );
    }

    #[test]
    fn test_stat_description() {
        assert_eq!(stat_description("Damage"), Some("Base damage"));
        assert_eq!(stat_description("MagSize"), Some("Magazine size"));
        assert_eq!(stat_description("Unknown"), None);
    }

    #[test]
    fn test_legendary_lookup() {
        assert!(legendary_by_name("Seventh Sense").is_some());
        assert!(legendary_by_internal("JAK_PS.comp_05_legendary_SeventhSense").is_some());
    }
}
