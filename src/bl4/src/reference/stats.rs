//! Stat description definitions

use std::collections::HashMap;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stat_description() {
        assert_eq!(stat_description("Damage"), Some("Base damage"));
        assert_eq!(stat_description("MagSize"), Some("Magazine size"));
        assert_eq!(stat_description("Unknown"), None);
    }

    #[test]
    fn test_all_stat_descriptions() {
        let stats = all_stat_descriptions();

        assert_eq!(stats.get("Damage"), Some(&"Base damage"));
        assert_eq!(stats.get("CritDamage"), Some(&"Critical hit damage"));
        assert_eq!(stats.get("FireRate"), Some(&"Firing rate"));
        assert_eq!(stats.get("ReloadTime"), Some(&"Reload time"));
        assert_eq!(stats.get("MagSize"), Some(&"Magazine size"));
        assert_eq!(stats.get("Accuracy"), Some(&"Base accuracy"));
        assert_eq!(stats.get("Spread"), Some(&"Projectile spread"));
        assert_eq!(stats.get("Recoil"), Some(&"Weapon recoil"));
        assert_eq!(stats.get("ProjectilesPerShot"), Some(&"Pellets per shot"));
        assert_eq!(stats.get("StatusChance"), Some(&"Status effect chance"));
        assert_eq!(stats.get("DamageRadius"), Some(&"Splash damage radius"));

        assert_eq!(stats.len(), 21);
        assert!(!stats.contains_key("Unknown"));
    }
}
