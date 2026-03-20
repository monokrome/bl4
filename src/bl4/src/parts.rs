//! Parts lookup for Borderlands 4 items
//!
//! Maps manufacturer IDs, weapon types, and categories to human-readable names.
//! Part index data is stored in share/manifest/ for reference only.
//!
//! NOTE: Serial IDs (first varint) differ from Parts DB Categories!
//! Use `serial_id_to_parts_category()` to convert.

use phf::phf_map;

/// First VarInt to (Manufacturer, Weapon Type) mapping
/// For VarInt-first serial format
///
/// Verification status:
///   [V] = Verified in-game with screenshots
///   [I] = Inferred from category/parts data
static WEAPON_INFO: phf::Map<u64, (&'static str, &'static str)> = phf_map! {
    // Shotguns
    8u64 => ("Daedalus", "Shotgun"),   // [I] DAD_SG - category 8
    12u64 => ("Torgue", "Shotgun"),    // [I] TOR_SG - category 11
    10u64 => ("Maliwan", "Shotgun"),   // [I] MAL_SG - category 19
    9u64 => ("Jakobs", "Shotgun"),     // [V] JAK_SG - Rainbow Vomit screenshot
    11u64 => ("Tediore", "Shotgun"),   // [I] TED_SG - category 10
    7u64 => ("Ripper", "Shotgun"),     // [V] BOR_SG - verified via NCS

    // Pistols
    4u64 => ("Jakobs", "Pistol"),      // [V] JAK_PS - Seventh Sense screenshot
    2u64 => ("Daedalus", "Pistol"),    // [I] DAD_PS - category 2
    6u64 => ("Torgue", "Pistol"),      // [I] TOR_PS - category 5
    5u64 => ("Tediore", "Pistol"),     // [I] TED_PS - category 4
    3u64 => ("Jakobs", "Pistol"),      // [V] JAK_PS - bank Seventh Senses

    // Assault Rifles (single-nibble)
    14u64 => ("Tediore", "AR"),        // [I] TED_AR - category 15
    13u64 => ("Daedalus", "AR"),       // [I] DAD_AR - category 13
    15u64 => ("Order", "AR"),          // [I] ORD_AR - category 18

    // Snipers (two-nibble)
    16u64 => ("Vladof", "Sniper"),     // [V] VLA_SR - Rebellious Vyudazy screenshot
    24u64 => ("Jakobs", "Sniper"),     // [I] JAK_SR - category 26
    26u64 => ("Order", "Sniper"),      // [I] ORD_SR - category 28
    25u64 => ("Maliwan", "Sniper"),    // [I] MAL_SR - category 29
    23u64 => ("Ripper", "Sniper"),     // [V] BOR_SR - verified via NCS

    // SMGs (two-nibble)
    20u64 => ("Daedalus", "SMG"),      // [I] DAD_SM - category 20
    22u64 => ("Ripper", "SMG"),        // [V] BOR_SM - verified via NCS
    21u64 => ("Maliwan", "SMG"),       // [I] MAL_SM - category 23
    19u64 => ("Vladof", "SMG"),        // [I] VLA_SM - category 22

    // Assault Rifles (two-nibble)
    18u64 => ("Vladof", "AR"),         // [I] VLA_AR - category 17
    17u64 => ("Torgue", "AR"),         // [I] TOR_AR - category 16
    27u64 => ("Jakobs", "AR"),         // [I] JAK_AR - category 14
};

/// Serial ID (first varint) to Parts Database Category mapping
///
/// Serial IDs in decoded serials differ from the NCS category IDs used in
/// parts_database.tsv. This table maps serial IDs to NCS-authoritative
/// category IDs extracted from inv*.bin root entries.
static SERIAL_TO_PARTS_CAT: phf::Map<u64, u64> = phf_map! {
    // Shotguns (NCS: bor_sg=7, dad_sg=8, jak_sg=9, mal_sg=10, ted_sg=11, tor_sg=12)
    8u64 => 8,    // DAD_SG
    12u64 => 12,  // TOR_SG
    10u64 => 10,  // MAL_SG
    9u64 => 9,    // JAK_SG
    11u64 => 11,  // TED_SG
    7u64 => 7,    // BOR_SG

    // Pistols (NCS: dad_ps=2, jak_ps=3, ord_ps=4, ted_ps=5, tor_ps=6)
    4u64 => 3,    // JAK_PS
    2u64 => 2,    // DAD_PS
    6u64 => 6,    // TOR_PS
    5u64 => 5,    // TED_PS
    3u64 => 3,    // JAK_PS (alternate)

    // Assault Rifles (NCS: dad_ar=13, ted_ar=14, ord_ar=15, tor_ar=17, vla_ar=18, jak_ar=27)
    14u64 => 14,  // TED_AR
    13u64 => 13,  // DAD_AR
    15u64 => 15,  // ORD_AR
    18u64 => 18,  // VLA_AR
    17u64 => 17,  // TOR_AR
    27u64 => 27,  // JAK_AR

    // Snipers (NCS: vla_sr=16, bor_sr=23, jak_sr=24, mal_sr=25, ord_sr=26)
    16u64 => 16,  // VLA_SR
    24u64 => 24,  // JAK_SR
    26u64 => 26,  // ORD_SR
    25u64 => 25,  // MAL_SR
    23u64 => 23,  // BOR_SR

    // SMGs (NCS: bor_sm=19, dad_sm=20, mal_sm=21, vla_sm=22)
    20u64 => 20,  // DAD_SM
    22u64 => 19,  // BOR_SM
    21u64 => 21,  // MAL_SM
    19u64 => 22,  // VLA_SM
};

/// Convert serial ID (first varint) to parts database category
pub fn serial_id_to_parts_category(serial_id: u64) -> u64 {
    SERIAL_TO_PARTS_CAT
        .get(&serial_id)
        .copied()
        .unwrap_or(serial_id)
}

/// Determine the correct divisor for a VarBit-first item based on value magnitude.
///
/// With correct bit ordering, the VarBit value IS the NCS category ID directly.
pub fn varbit_divisor(_varbit: u64) -> u64 {
    1
}

/// Extract NCS category ID from a VarBit value.
///
/// With correct bit ordering, the VarBit value IS the category ID.
pub fn category_from_varbit(varbit: u64) -> i64 {
    varbit as i64
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

/// Decode a level from a raw code value.
/// Returns (decoded_level, raw_decoded_value) tuple.
///
/// With correct bit ordering, the VarInt value IS the level directly.
/// Valid levels are 1-50.
pub fn level_from_code(code: u64) -> Option<(u8, u8)> {
    if matches!(code, 1..=50) {
        Some((code as u8, code as u8))
    } else {
        None
    }
}

/// Encode a level into a raw code value (reverse of `level_from_code`).
///
/// With correct bit ordering, code = level directly.
pub fn code_from_level(level: u8) -> Option<u64> {
    if level == 0 || level > 50 {
        return None;
    }
    Some(level as u64)
}

/// Encode a weapon level code with rarity (VarInt-first format).
///
/// With correct bit ordering, the level code IS the level directly.
/// Rarity is not encoded in the level code.
pub fn weapon_level_code(level: u8, _rarity: crate::serial::Rarity) -> Option<u64> {
    code_from_level(level)
}

/// Reverse lookup: (manufacturer, weapon_type) → first VarInt ID.
///
/// Returns the first matching ID from WEAPON_INFO.
pub fn first_varint_from_weapon_info(manufacturer: &str, weapon_type: &str) -> Option<u64> {
    WEAPON_INFO.entries().find_map(|(id, (mfr, wtype))| {
        if *mfr == manufacturer && *wtype == weapon_type {
            Some(*id)
        } else {
            None
        }
    })
}

/// Encode a VarBit value from category ID.
///
/// With correct bit ordering, VarBit = category ID directly.
pub fn varbit_from_category(category: i64, _divisor: u64, _metadata: u64) -> u64 {
    category as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_info_lookup() {
        assert_eq!(weapon_info_from_first_varint(3), Some(("Jakobs", "Pistol")));
        assert_eq!(
            weapon_info_from_first_varint(16),
            Some(("Vladof", "Sniper"))
        );
        assert_eq!(weapon_info_from_first_varint(17), Some(("Torgue", "AR")));
        assert_eq!(weapon_info_from_first_varint(21), Some(("Maliwan", "SMG")));
        assert_eq!(weapon_info_from_first_varint(999), None);
    }

    #[test]
    fn test_manufacturer_lookup() {
        assert_eq!(manufacturer_name(2), Some("Daedalus"));
        assert_eq!(manufacturer_name(17), Some("Torgue"));
        assert_eq!(manufacturer_name(21), Some("Maliwan"));
        assert_eq!(manufacturer_name(999), None);
    }

    #[test]
    fn test_varbit_divisor() {
        // VarBit IS the category now, divisor always 1
        assert_eq!(varbit_divisor(279), 1);
        assert_eq!(varbit_divisor(22), 1);
        assert_eq!(varbit_divisor(0), 1);
    }

    #[test]
    fn test_category_from_varbit() {
        // VarBit value IS the category directly
        assert_eq!(category_from_varbit(279), 279); // Maliwan Shield
        assert_eq!(category_from_varbit(269), 269); // Vladof Repair Kit
        assert_eq!(category_from_varbit(289), 289); // Maliwan Heavy Weapon
        assert_eq!(category_from_varbit(22), 22); // Vladof SMG
        assert_eq!(category_from_varbit(16), 16); // Vladof Sniper
    }

    #[test]
    fn test_category_name_lookup() {
        assert_eq!(category_name(2), Some("Daedalus Pistol"));
        assert_eq!(category_name(22), Some("Vladof SMG"));
        assert_eq!(category_name(283), Some("Vladof Shield"));
        assert_eq!(category_name(999), None);
    }

    #[test]
    fn test_level_from_code() {
        assert_eq!(level_from_code(1), Some((1, 1)));
        assert_eq!(level_from_code(30), Some((30, 30)));
        assert_eq!(level_from_code(50), Some((50, 50)));
        assert_eq!(level_from_code(0), None);
        assert_eq!(level_from_code(51), None);
    }

    #[test]
    fn test_serial_id_to_parts_category() {
        assert_eq!(serial_id_to_parts_category(8), 8);
        assert_eq!(serial_id_to_parts_category(9), 9);
        assert_eq!(serial_id_to_parts_category(16), 16);
        assert_eq!(serial_id_to_parts_category(21), 21);
        assert_eq!(serial_id_to_parts_category(999), 999);
    }

    #[test]
    fn test_weapon_type_from_first_varint() {
        assert_eq!(weapon_type_from_first_varint(8), Some("Shotgun"));
        assert_eq!(weapon_type_from_first_varint(4), Some("Pistol"));
        assert_eq!(weapon_type_from_first_varint(16), Some("Sniper"));
        assert_eq!(weapon_type_from_first_varint(21), Some("SMG"));
        assert_eq!(weapon_type_from_first_varint(17), Some("AR"));
        assert_eq!(weapon_type_from_first_varint(999), None);
    }

    #[test]
    fn test_code_from_level() {
        // All valid levels encode directly
        assert_eq!(code_from_level(1), Some(1));
        assert_eq!(code_from_level(15), Some(15));
        assert_eq!(code_from_level(16), Some(16));
        assert_eq!(code_from_level(30), Some(30));
        assert_eq!(code_from_level(50), Some(50));
        // Invalid
        assert_eq!(code_from_level(0), None);
        assert_eq!(code_from_level(51), None);
    }

    #[test]
    fn test_code_from_level_roundtrip() {
        for level in 1..=50u8 {
            let code = code_from_level(level).unwrap();
            let (decoded, _) = level_from_code(code).unwrap();
            assert_eq!(decoded, level, "roundtrip failed for level {}", level);
        }
    }

    #[test]
    fn test_first_varint_from_weapon_info() {
        let id = first_varint_from_weapon_info("Jakobs", "Shotgun").unwrap();
        assert_eq!(id, 9);
        assert_eq!(
            weapon_info_from_first_varint(id),
            Some(("Jakobs", "Shotgun"))
        );

        let id = first_varint_from_weapon_info("Vladof", "SMG").unwrap();
        assert_eq!(id, 19);
        assert_eq!(weapon_info_from_first_varint(id), Some(("Vladof", "SMG")));

        assert!(first_varint_from_weapon_info("FakeManufacturer", "Pistol").is_none());
    }

    #[test]
    fn test_varbit_from_category_roundtrip() {
        // VarBit = category directly
        let varbit = varbit_from_category(279, 1, 0);
        assert_eq!(category_from_varbit(varbit), 279);

        let varbit = varbit_from_category(22, 1, 0);
        assert_eq!(category_from_varbit(varbit), 22);
    }
}
