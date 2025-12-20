//! Parts lookup for Borderlands 4 items
//!
//! Maps manufacturer IDs, weapon types, and categories to human-readable names.
//! Part index data is stored in share/manifest/ for reference only.

/// First VarInt to (Manufacturer, Weapon Type) mapping
/// For VarInt-first serial format (types a-g, u-z)
///
/// Verification status:
///   [V] = Verified in-game with screenshots
///   [I] = Inferred from category/parts data
///   [?] = Needs verification
pub fn weapon_info_from_first_varint(id: u64) -> Option<(&'static str, &'static str)> {
    match id {
        // Shotguns (low IDs)
        1 => Some(("Daedalus", "Shotgun")),   // [I] DAD_SG - category 8
        3 => Some(("Torgue", "Shotgun")),     // [I] TOR_SG - category 11
        5 => Some(("Maliwan", "Shotgun")),    // [I] MAL_SG - category 19
        9 => Some(("Jakobs", "Shotgun")),     // [V] JAK_SG - Rainbow Vomit screenshot
        13 => Some(("Tediore", "Shotgun")),   // [I] TED_SG - category 10
        14 => Some(("Ripper", "Shotgun")),    // [V] RIP_SG - Hungry Flensing Hellhound screenshot
        // Pistols (low IDs)
        2 => Some(("Jakobs", "Pistol")),      // [V] JAK_PS - Seventh Sense screenshot
        4 => Some(("Daedalus", "Pistol")),    // [I] DAD_PS - category 2
        6 => Some(("Torgue", "Pistol")),      // [I] TOR_PS - category 5
        10 => Some(("Tediore", "Pistol")),    // [I] TED_PS - category 4
        12 => Some(("Jakobs", "Pistol")),     // [V] JAK_PS - bank Seventh Senses
        // Assault Rifles (low IDs)
        7 => Some(("Tediore", "AR")),         // [I] TED_AR - category 15
        11 => Some(("Daedalus", "AR")),       // [I] DAD_AR - category 13
        15 => Some(("Order", "AR")),          // [I] ORD_AR - category 18
        // Snipers (high IDs, bit 7 set)
        128 => Some(("Vladof", "Sniper")),    // [V] VLA_SR - Rebellious Vyudazy screenshot
        129 => Some(("Jakobs", "Sniper")),    // [I] JAK_SR - category 26
        133 => Some(("Order", "Sniper")),     // [I] ORD_SR - category 28
        137 => Some(("Maliwan", "Sniper")),   // [I] MAL_SR - category 29
        142 => Some(("Bor", "Sniper")),       // [?] BOR_SR - category 25, needs verification
        // SMGs (high IDs, bit 7 set)
        130 => Some(("Daedalus", "SMG")),     // [I] DAD_SM - category 20
        134 => Some(("Bor", "SMG")),          // [?] BOR_SM - category 21, needs verification
        138 => Some(("Maliwan", "SMG")),      // [I] MAL_SM - category 23
        140 => Some(("Vladof", "SMG")),       // [I] VLA_SM - category 22
        // Assault Rifles (high IDs, bit 7 set)
        132 => Some(("Vladof", "AR")),        // [I] VLA_AR - category 17
        136 => Some(("Torgue", "AR")),        // [I] TOR_AR - category 16
        141 => Some(("Jakobs", "AR")),        // [I] JAK_AR - category 14
        _ => None,
    }
}

/// Extract just the manufacturer name from first VarInt
pub fn manufacturer_name(id: u64) -> Option<&'static str> {
    weapon_info_from_first_varint(id).map(|(mfg, _)| mfg)
}

/// Extract just the weapon type from first VarInt
pub fn weapon_type_from_first_varint(id: u64) -> Option<&'static str> {
    weapon_info_from_first_varint(id).map(|(_, wtype)| wtype)
}

/// Decode level from fourth token (level code)
///
/// For tokens < 128: level = token directly (levels 1-50)
/// For tokens >= 128: level = 2 * (code - 120)
///   - This gives level 16 at code 128, level 30 at code 135, etc.
///
/// Verified in-game Dec 2025.
pub fn level_from_code(code: u64) -> Option<u8> {
    if code >= 128 {
        // High-level encoding: level = 2 * (code - 120)
        let level = 2 * (code as i32 - 120);
        if level > 0 && level <= 255 {
            Some(level as u8)
        } else {
            None
        }
    } else if code <= 50 {
        // Direct encoding for levels 1-50
        Some(code as u8)
    } else {
        None
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
        // Maliwan Shotgun (gap filler)
        19 => Some("Maliwan Shotgun"),
        // SMGs
        20 => Some("Daedalus SMG"),
        21 => Some("Bor SMG"),
        22 => Some("Vladof SMG"),
        23 => Some("Maliwan SMG"),
        // Bor Sniper (gap filler)
        25 => Some("Bor Sniper"),
        // Snipers
        26 => Some("Jakobs Sniper"),
        27 => Some("Vladof Sniper"),
        28 => Some("Order Sniper"),
        29 => Some("Maliwan Sniper"),
        // Class Mods (derived from serial analysis - categories 44, 55, 97, 140)
        44 => Some("Dark Siren Class Mod"),
        55 => Some("Paladin Class Mod"),
        97 => Some("Gravitar Class Mod"),
        140 => Some("Exo Soldier Class Mod"),
        // Firmware (category 151)
        151 => Some("Firmware"),
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
        289 => Some("Shield Variant"),
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
///
/// Format types determined through analysis Dec 2025:
/// - Weapons: a-d, f-g, r, u-z (VarInt-first format, first VarInt = mfg+type)
/// - Equipment: e (VarBit-first, divisor 384 for category - shields, grenades, class mods, firmware)
/// - Class Mods: !, # (special format with fixed manufacturer IDs 247, 255)
pub fn item_type_name(type_char: char) -> &'static str {
    match type_char {
        'a'..='d' => "Weapon",
        'e' => "Equipment",
        'f' | 'g' => "Weapon",
        'r' => "Weapon",
        'u' => "Weapon",
        'v'..='z' => "Weapon",
        '!' => "Class Mod",
        '#' => "Class Mod",
        _ => "Unknown",
    }
}

/// Get equipment category name for 'e' type items
/// Category is derived from first VarBit / 384
pub fn equipment_category_name(category: i64) -> Option<&'static str> {
    match category {
        // Class Mods (derived from serial analysis)
        44 => Some("Dark Siren Class Mod"),
        55 => Some("Paladin Class Mod"),
        97 => Some("Gravitar Class Mod"),
        140 => Some("Exo Soldier Class Mod"),
        // Firmware
        151 => Some("Firmware"),
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
        289 => Some("Shield Variant"),
        // Gadgets
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_info_lookup() {
        // Verified in-game: slot 3 Jakobs Pistol = first VarInt 12
        assert_eq!(
            weapon_info_from_first_varint(12),
            Some(("Jakobs", "Pistol"))
        );
        // Verified in-game: Vladof Sniper (Vamoose) = first VarInt 128
        assert_eq!(
            weapon_info_from_first_varint(128),
            Some(("Vladof", "Sniper"))
        );
        // Verified in-game: slot 2 Torgue AR = first VarInt 136
        assert_eq!(weapon_info_from_first_varint(136), Some(("Torgue", "AR")));
        // Verified in-game: slot 4 Maliwan SMG = first VarInt 138
        assert_eq!(weapon_info_from_first_varint(138), Some(("Maliwan", "SMG")));
        // Unknown ID returns None
        assert_eq!(weapon_info_from_first_varint(999), None);
    }

    #[test]
    fn test_manufacturer_lookup() {
        assert_eq!(manufacturer_name(4), Some("Daedalus"));
        assert_eq!(manufacturer_name(136), Some("Torgue"));
        assert_eq!(manufacturer_name(138), Some("Maliwan"));
        assert_eq!(manufacturer_name(999), None);
    }

    #[test]
    fn test_item_type_lookup() {
        assert_eq!(item_type_name('r'), "Weapon");
        assert_eq!(item_type_name('v'), "Weapon");
        assert_eq!(item_type_name('e'), "Equipment");
        assert_eq!(item_type_name('!'), "Class Mod");
        assert_eq!(item_type_name('#'), "Class Mod");
        assert_eq!(item_type_name('?'), "Unknown");
    }

    #[test]
    fn test_equipment_category_name() {
        assert_eq!(equipment_category_name(279), Some("Energy Shield"));
        assert_eq!(equipment_category_name(300), Some("Grenade Gadget"));
        assert_eq!(equipment_category_name(44), Some("Dark Siren Class Mod"));
        assert_eq!(equipment_category_name(999), None);
    }

    #[test]
    fn test_category_name_lookup() {
        assert_eq!(category_name(2), Some("Daedalus Pistol"));
        assert_eq!(category_name(22), Some("Vladof SMG"));
        assert_eq!(category_name(283), Some("Armor Shield"));
        assert_eq!(category_name(999), None);
    }

    #[test]
    fn test_level_from_code() {
        // Direct encoding
        assert_eq!(level_from_code(1), Some(1));
        assert_eq!(level_from_code(50), Some(50));
        // High-level encoding: level = 2*(code-120)
        assert_eq!(level_from_code(128), Some(16)); // 2*(128-120) = 16
        assert_eq!(level_from_code(135), Some(30)); // 2*(135-120) = 30 (verified in-game)
        assert_eq!(level_from_code(145), Some(50)); // 2*(145-120) = 50
        // Invalid
        assert_eq!(level_from_code(51), None);
    }
}
