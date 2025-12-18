//! Parts lookup for Borderlands 4 items
//!
//! Maps manufacturer IDs, weapon types, and categories to human-readable names.
//! Part index data is stored in share/manifest/ for reference only.

/// First VarInt to (Manufacturer, Weapon Type) mapping
/// For VarInt-first serial format (types a-g, u-z)
/// Derived from verified in-game testing of CSV weapon data
pub fn weapon_info_from_first_varint(id: u64) -> Option<(&'static str, &'static str)> {
    match id {
        // Shotguns (low IDs)
        1 => Some(("Daedalus", "Shotgun")),
        3 => Some(("Torgue", "Shotgun")),
        5 => Some(("Maliwan", "Shotgun")),
        9 => Some(("Jakobs", "Shotgun")),
        13 => Some(("Tediore", "Shotgun")),
        14 => Some(("Ripper", "Shotgun")),
        // Pistols (low IDs)
        2 => Some(("Order", "Pistol")),
        4 => Some(("Daedalus", "Pistol")),
        6 => Some(("Torgue", "Pistol")),
        10 => Some(("Tediore", "Pistol")),
        12 => Some(("Jakobs", "Pistol")),
        // Assault Rifles (low IDs)
        7 => Some(("Tediore", "AR")),
        11 => Some(("Daedalus", "AR")),
        15 => Some(("Order", "AR")),
        // Snipers (high IDs, bit 7 set)
        128 => Some(("Vladof", "Sniper")),
        129 => Some(("Jakobs", "Sniper")),
        133 => Some(("Order", "Sniper")),
        137 => Some(("Maliwan", "Sniper")),
        142 => Some(("Ripper", "Sniper")),
        // SMGs (high IDs, bit 7 set)
        130 => Some(("Daedalus", "SMG")),
        134 => Some(("Vladof", "SMG")),
        138 => Some(("Maliwan", "SMG")),
        140 => Some(("Ripper", "SMG")),
        // Assault Rifles (high IDs, bit 7 set)
        132 => Some(("Vladof", "AR")),
        136 => Some(("Torgue", "AR")),
        141 => Some(("Jakobs", "AR")),
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
/// For tokens < 128: level = token directly
/// For tokens >= 128: level = 16 + bits[6:1] + 8*bit0
///   - bit 7 is always set (indicates high-level encoding)
///   - bits 1-6 provide base offset from 16
///   - bit 0 adds 8 if set
///
/// Verified in-game Dec 2025.
pub fn level_from_code(code: u64) -> Option<u8> {
    if code >= 128 {
        // High-level encoding: 16 + bits[6:1] + 8*bit0
        let bits_1_6 = ((code >> 1) & 0x3F) as u8;
        let bit0 = (code & 1) as u8;
        Some(16 + bits_1_6 + 8 * bit0)
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
        // Verified in-game: slot 1 Vladof Sniper = first VarInt 128
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
        assert_eq!(item_type_name('r'), "Item (variant r)");
        assert_eq!(item_type_name('v'), "Weapon (variant v-z)");
        assert_eq!(item_type_name('?'), "Unknown");
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
        // High-level encoding (bit 7 set)
        assert_eq!(level_from_code(196), Some(50)); // 16 + 34 + 0 = 50
        assert_eq!(level_from_code(128), Some(16)); // 16 + 0 + 0 = 16
        // Invalid
        assert_eq!(level_from_code(51), None);
    }
}
