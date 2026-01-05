//! Parts lookup for Borderlands 4 items
//!
//! Maps manufacturer IDs, weapon types, and categories to human-readable names.
//! Part index data is stored in share/manifest/ for reference only.
//!
//! NOTE: Serial IDs (first varint) differ from Parts DB Categories!
//! Use `serial_id_to_parts_category()` to convert.

use phf::phf_map;

/// First VarInt to (Manufacturer, Weapon Type) mapping
/// For VarInt-first serial format (types a-g, u-z)
///
/// Verification status:
///   [V] = Verified in-game with screenshots
///   [I] = Inferred from category/parts data
///   [?] = Needs verification
static WEAPON_INFO: phf::Map<u64, (&'static str, &'static str)> = phf_map! {
    // Shotguns (low IDs)
    1u64 => ("Daedalus", "Shotgun"),   // [I] DAD_SG - category 8
    3u64 => ("Torgue", "Shotgun"),     // [I] TOR_SG - category 11
    5u64 => ("Maliwan", "Shotgun"),    // [I] MAL_SG - category 19
    9u64 => ("Jakobs", "Shotgun"),     // [V] JAK_SG - Rainbow Vomit screenshot
    13u64 => ("Tediore", "Shotgun"),   // [I] TED_SG - category 10
    14u64 => ("Ripper", "Shotgun"),      // [V] BOR_SG - verified via NCS NexusSerialized

    // Pistols (low IDs)
    2u64 => ("Jakobs", "Pistol"),      // [V] JAK_PS - Seventh Sense screenshot
    4u64 => ("Daedalus", "Pistol"),    // [I] DAD_PS - category 2
    6u64 => ("Torgue", "Pistol"),      // [I] TOR_PS - category 5
    10u64 => ("Tediore", "Pistol"),    // [I] TED_PS - category 4
    12u64 => ("Jakobs", "Pistol"),     // [V] JAK_PS - bank Seventh Senses

    // Assault Rifles (low IDs)
    7u64 => ("Tediore", "AR"),         // [I] TED_AR - category 15
    11u64 => ("Daedalus", "AR"),       // [I] DAD_AR - category 13
    15u64 => ("Order", "AR"),          // [I] ORD_AR - category 18

    // Snipers (high IDs, bit 7 set)
    128u64 => ("Vladof", "Sniper"),    // [V] VLA_SR - Rebellious Vyudazy screenshot
    129u64 => ("Jakobs", "Sniper"),    // [I] JAK_SR - category 26
    133u64 => ("Order", "Sniper"),     // [I] ORD_SR - category 28
    137u64 => ("Maliwan", "Sniper"),   // [I] MAL_SR - category 29
    142u64 => ("Ripper", "Sniper"),      // [V] BOR_SR - verified via NCS NexusSerialized

    // SMGs (high IDs, bit 7 set)
    130u64 => ("Daedalus", "SMG"),     // [I] DAD_SM - category 20
    134u64 => ("Ripper", "SMG"),         // [V] BOR_SM - verified via NCS NexusSerialized
    138u64 => ("Maliwan", "SMG"),      // [I] MAL_SM - category 23
    140u64 => ("Vladof", "SMG"),       // [I] VLA_SM - category 22

    // Assault Rifles (high IDs, bit 7 set)
    132u64 => ("Vladof", "AR"),        // [I] VLA_AR - category 17
    136u64 => ("Torgue", "AR"),        // [I] TOR_AR - category 16
    141u64 => ("Jakobs", "AR"),        // [I] JAK_AR - category 14

    // Shields (type 'r' format - verified from tagged bank items 2025-12-20)
    133824u64 => ("", "Armor Shield"),     // [V] Tagged bank items
    221888u64 => ("", "Armor Shield"),     // [V] Tagged bank items
    254656u64 => ("", "Armor Shield"),     // [V] Tagged bank items
    53952u64 => ("", "Energy Shield"),     // [V] Tagged bank items
    168640u64 => ("", "Energy Shield"),    // [V] Tagged bank items
    238272u64 => ("", "Energy Shield"),    // [V] Tagged bank items
};

/// Serial ID (first varint) to Parts Database Category mapping
/// Serial IDs in decoded serials differ from the category IDs used in parts_database.json
/// This table maps between them for correct part resolution
static SERIAL_TO_PARTS_CAT: phf::Map<u64, u64> = phf_map! {
    // Shotguns
    1u64 => 8,    // DAD_SG
    3u64 => 11,   // TOR_SG
    5u64 => 19,   // MAL_SG
    9u64 => 9,    // JAK_SG
    13u64 => 10,  // TED_SG
    14u64 => 12,  // BOR_SG

    // Pistols
    2u64 => 3,    // JAK_PS
    4u64 => 2,    // DAD_PS
    6u64 => 5,    // TOR_PS
    10u64 => 4,   // TED_PS
    12u64 => 3,   // JAK_PS (alternate)

    // Assault Rifles (low IDs)
    7u64 => 15,   // TED_AR
    11u64 => 13,  // DAD_AR
    15u64 => 18,  // ORD_AR

    // Snipers (high IDs) - VLA_SR and BOR_SR share category 25
    128u64 => 25, // VLA_SR
    129u64 => 26, // JAK_SR
    133u64 => 28, // ORD_SR
    137u64 => 29, // MAL_SR
    142u64 => 25, // BOR_SR

    // SMGs (high IDs)
    130u64 => 20, // DAD_SM
    134u64 => 21, // BOR_SM
    138u64 => 23, // MAL_SM
    140u64 => 22, // VLA_SM

    // Assault Rifles (high IDs)
    132u64 => 17, // VLA_AR
    136u64 => 16, // TOR_AR
    141u64 => 14, // JAK_AR
};

/// Convert serial ID (first varint) to parts database category
pub fn serial_id_to_parts_category(serial_id: u64) -> u64 {
    SERIAL_TO_PARTS_CAT
        .get(&serial_id)
        .copied()
        .unwrap_or(serial_id)
}

// Category names are now loaded from manifest data at compile time
// See crate::manifest for the source data

/// Serial format configuration
///
/// Defines how to parse a serial based on its type character.
#[derive(Debug, Clone, Copy)]
pub struct SerialFormat {
    /// Divisor to extract category from first VarBit (0 = use VarInt weapon info instead)
    pub category_divisor: u64,
    /// Whether first VarInt contains manufacturer+weapon type ID
    pub has_weapon_info: bool,
    /// Whether to extract level from 4th header VarInt
    pub extract_level: bool,
}

impl SerialFormat {
    const fn varint_weapon(extract_level: bool) -> Self {
        Self {
            category_divisor: 0,
            has_weapon_info: true,
            extract_level,
        }
    }

    const fn varbit(divisor: u64) -> Self {
        Self {
            category_divisor: divisor,
            has_weapon_info: false,
            extract_level: false,
        }
    }

    const fn class_mod() -> Self {
        Self {
            category_divisor: 0,
            has_weapon_info: false,
            extract_level: false,
        }
    }

    /// Extract category from VarBit value (returns None if this format doesn't use VarBit categories)
    pub fn extract_category(&self, varbit: u64) -> Option<i64> {
        if self.category_divisor > 0 {
            Some((varbit / self.category_divisor) as i64)
        } else {
            None
        }
    }
}

/// Serial format lookup by type character
static SERIAL_FORMATS: phf::Map<char, SerialFormat> = phf_map! {
    // VarInt-first weapons (first VarInt = manufacturer+type, 4th = level)
    'a' => SerialFormat::varint_weapon(true),
    'b' => SerialFormat::varint_weapon(true),
    'c' => SerialFormat::varint_weapon(true),
    'd' => SerialFormat::varint_weapon(true),
    'f' => SerialFormat::varint_weapon(true),
    'g' => SerialFormat::varint_weapon(true),
    'u' => SerialFormat::varint_weapon(true),
    'v' => SerialFormat::varint_weapon(true),
    'w' => SerialFormat::varint_weapon(true),
    'x' => SerialFormat::varint_weapon(true),
    'y' => SerialFormat::varint_weapon(true),
    'z' => SerialFormat::varint_weapon(true),
    // VarBit-first items (category = first VarBit / divisor)
    'e' => SerialFormat::varbit(384),
    // Shields (VarBit-first, category = VarBit / 8192)
    'r' => SerialFormat::varbit(8192),
    // Class mods
    '!' => SerialFormat::class_mod(),
    '#' => SerialFormat::class_mod(),
};

/// Get the serial format for a type character
pub fn serial_format(type_char: char) -> Option<&'static SerialFormat> {
    SERIAL_FORMATS.get(&type_char)
}

// Public API functions that wrap the static maps

pub fn weapon_info_from_first_varint(id: u64) -> Option<(&'static str, &'static str)> {
    WEAPON_INFO.get(&id).copied()
}

pub fn manufacturer_name(id: u64) -> Option<&'static str> {
    weapon_info_from_first_varint(id).map(|(mfg, _)| mfg)
}

pub fn weapon_type_from_first_varint(id: u64) -> Option<&'static str> {
    weapon_info_from_first_varint(id).map(|(_, wtype)| wtype)
}

pub fn category_name(category: i64) -> Option<&'static str> {
    // Delegate to manifest module (loads from compiled-in JSON data)
    if let Some(name) = crate::manifest::category_name(category) {
        return Some(name);
    }

    // For gadget range (300-399), try base type (category / 10 * 10)
    // e.g., 321 -> 320 (Repair Kit), 301 -> 300 (Grenade Gadget)
    if (300..400).contains(&category) {
        let base = category / 10 * 10;
        return crate::manifest::category_name(base);
    }

    None
}

/// Shield category names for r-type items
/// These overlap with weapon categories but have different meanings
/// Based on verified tagged bank items
static SHIELD_CATEGORY_NAMES: phf::Map<i64, &'static str> = phf_map! {
    16i64 => "Energy Shield",   // [I] r-type category 16
    20i64 => "Energy Shield",   // [I] r-type category 20
    21i64 => "Energy Shield",   // [I] r-type category 21
    24i64 => "Energy Shield",   // [I] r-type category 24
    28i64 => "Armor Shield",    // [I] r-type category 28
    31i64 => "Armor Shield",    // [V] r-type category 31 - verified from tagged bank
};

/// Get category name with item type awareness
/// For r-type (shields), uses shield-specific category map to avoid conflicts with weapons
pub fn category_name_for_type(item_type: char, category: i64) -> Option<&'static str> {
    match item_type {
        'r' => SHIELD_CATEGORY_NAMES
            .get(&category)
            .copied()
            .or_else(|| crate::manifest::category_name(category)),
        _ => crate::manifest::category_name(category),
    }
}

/// Get a human-readable description for a type character
///
/// Returns a description based on the serial format.
pub fn item_type_name(type_char: char) -> &'static str {
    match serial_format(type_char) {
        Some(fmt) if fmt.has_weapon_info => "Weapon",
        Some(fmt) if fmt.category_divisor > 0 => "Item",
        Some(_) => "Class Mod",
        None => "Unknown",
    }
}

/// Decode level from token (level code)
///
/// BL4 max level is 50. Encoding varies by context:
/// - For tokens 1-50: level = token directly
/// - For tokens >= 128: level = 2 * (code - 120), capped at 50
///   - Code 128 → 16, Code 135 → 30, Code 145 → 50
///   - Codes > 145 are capped at 50 (e.g., 196 → 50)
/// - Tokens 51-127: invalid (return None)
///
/// Note: This applies to VarBit-first equipment (level = 2nd VarBit)
/// and VarInt-first weapons (level = 4th VarInt). The encoding may
/// differ slightly between item types - needs more in-game verification.
/// Decode a level from a raw code value.
/// Returns (decoded_level, raw_decoded_value) tuple.
/// If raw_decoded_value > 50, our decoding may be wrong.
pub fn level_from_code(code: u64) -> Option<(u8, u8)> {
    const MAX_LEVEL: u8 = 50;

    if code >= 128 {
        // High-level encoding: level = 2 * (code - 120)
        let level = 2 * (code as i32 - 120);
        if level > 0 {
            let raw = level as u8;
            let capped = raw.min(MAX_LEVEL);
            Some((capped, raw))
        } else {
            None
        }
    } else if code <= 50 {
        // Direct encoding for levels 1-50
        Some((code as u8, code as u8))
    } else {
        // Codes 51-127 are invalid
        None
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
        // Verified shields from tagged bank items (type 'r' format)
        assert_eq!(
            weapon_info_from_first_varint(133824),
            Some(("", "Armor Shield"))
        );
        assert_eq!(
            weapon_info_from_first_varint(238272),
            Some(("", "Energy Shield"))
        );
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
        assert_eq!(item_type_name('a'), "Weapon");
        assert_eq!(item_type_name('r'), "Item");
        assert_eq!(item_type_name('e'), "Item");
        assert_eq!(item_type_name('!'), "Class Mod");
        assert_eq!(item_type_name('?'), "Unknown");
    }

    #[test]
    fn test_serial_format() {
        // VarInt-first weapons
        let fmt = serial_format('a').unwrap();
        assert!(fmt.has_weapon_info);
        assert!(fmt.extract_level);
        assert_eq!(fmt.category_divisor, 0);

        // VarBit-first items
        let fmt = serial_format('e').unwrap();
        assert!(!fmt.has_weapon_info);
        assert_eq!(fmt.category_divisor, 384);
        assert_eq!(fmt.extract_category(384 * 23), Some(23)); // Maliwan SMG

        let fmt = serial_format('r').unwrap();
        assert_eq!(fmt.category_divisor, 8192);

        // Class mods
        let fmt = serial_format('!').unwrap();
        assert!(!fmt.has_weapon_info);
        assert_eq!(fmt.category_divisor, 0);

        // Unknown
        assert!(serial_format('?').is_none());
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
        // Direct encoding - capped and raw are the same
        assert_eq!(level_from_code(1), Some((1, 1)));
        assert_eq!(level_from_code(50), Some((50, 50)));
        // High-level encoding: level = 2*(code-120)
        assert_eq!(level_from_code(128), Some((16, 16))); // 2*(128-120) = 16
        assert_eq!(level_from_code(135), Some((30, 30))); // 2*(135-120) = 30 (verified in-game)
        assert_eq!(level_from_code(145), Some((50, 50))); // 2*(145-120) = 50
                                                          // Capped at 50 for higher codes - raw shows true decoded value
        assert_eq!(level_from_code(150), Some((50, 60))); // raw=60, capped to 50
        assert_eq!(level_from_code(196), Some((50, 152))); // raw=152, capped to 50
                                                           // Invalid codes (51-127)
        assert_eq!(level_from_code(51), None);
        assert_eq!(level_from_code(127), None);
    }

    #[test]
    fn test_serial_id_to_parts_category() {
        // Known mappings from SERIAL_TO_PARTS_CAT
        assert_eq!(serial_id_to_parts_category(1), 8); // DAD_SG
        assert_eq!(serial_id_to_parts_category(9), 9); // JAK_SG
        assert_eq!(serial_id_to_parts_category(128), 25); // VLA_SR (corrected from 27)
        assert_eq!(serial_id_to_parts_category(138), 23); // MAL_SM

        // Unknown ID returns the ID itself as fallback
        assert_eq!(serial_id_to_parts_category(999), 999);
    }

    #[test]
    fn test_weapon_type_from_first_varint() {
        // Known weapon types
        assert_eq!(weapon_type_from_first_varint(1), Some("Shotgun"));
        assert_eq!(weapon_type_from_first_varint(2), Some("Pistol"));
        assert_eq!(weapon_type_from_first_varint(128), Some("Sniper"));
        assert_eq!(weapon_type_from_first_varint(138), Some("SMG"));
        assert_eq!(weapon_type_from_first_varint(136), Some("AR"));

        // Unknown returns None
        assert_eq!(weapon_type_from_first_varint(999), None);
    }

    #[test]
    fn test_category_name_for_type_regular() {
        // Non-shield items use manifest lookup
        assert_eq!(category_name_for_type('a', 2), Some("Daedalus Pistol"));
        assert_eq!(category_name_for_type('e', 22), Some("Vladof SMG"));
    }

    #[test]
    fn test_category_name_for_type_shield() {
        // Shield items (type 'r') use SHIELD_CATEGORY_NAMES first
        assert_eq!(category_name_for_type('r', 16), Some("Energy Shield"));
        assert_eq!(category_name_for_type('r', 20), Some("Energy Shield"));
        assert_eq!(category_name_for_type('r', 21), Some("Energy Shield"));
    }

    #[test]
    fn test_category_name_for_type_fallback() {
        // Unknown category falls back to manifest lookup
        assert_eq!(category_name_for_type('r', 283), Some("Armor Shield"));
        // Unknown category for unknown type returns None
        assert_eq!(category_name_for_type('a', 99999), None);
    }
}
