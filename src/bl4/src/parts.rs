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
    // Shotguns (low IDs)
    1u64 => ("Daedalus", "Shotgun"),   // [I] DAD_SG - category 8
    3u64 => ("Torgue", "Shotgun"),     // [I] TOR_SG - category 11
    5u64 => ("Maliwan", "Shotgun"),    // [I] MAL_SG - category 19
    9u64 => ("Jakobs", "Shotgun"),     // [V] JAK_SG - Rainbow Vomit screenshot
    13u64 => ("Tediore", "Shotgun"),   // [I] TED_SG - category 10
    14u64 => ("Ripper", "Shotgun"),    // [V] BOR_SG - verified via NCS

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
    142u64 => ("Ripper", "Sniper"),    // [V] BOR_SR - verified via NCS

    // SMGs (high IDs, bit 7 set)
    130u64 => ("Daedalus", "SMG"),     // [I] DAD_SM - category 20
    134u64 => ("Ripper", "SMG"),       // [V] BOR_SM - verified via NCS
    138u64 => ("Maliwan", "SMG"),      // [I] MAL_SM - category 23
    140u64 => ("Vladof", "SMG"),       // [I] VLA_SM - category 22

    // Assault Rifles (high IDs, bit 7 set)
    132u64 => ("Vladof", "AR"),        // [I] VLA_AR - category 17
    136u64 => ("Torgue", "AR"),        // [I] TOR_AR - category 16
    141u64 => ("Jakobs", "AR"),        // [I] JAK_AR - category 14
};

/// Serial ID (first varint) to Parts Database Category mapping
///
/// Serial IDs in decoded serials differ from the NCS category IDs used in
/// parts_database.tsv. This table maps serial IDs to NCS-authoritative
/// category IDs extracted from inv*.bin root entries.
static SERIAL_TO_PARTS_CAT: phf::Map<u64, u64> = phf_map! {
    // Shotguns (NCS: bor_sg=7, dad_sg=8, jak_sg=9, mal_sg=10, ted_sg=11, tor_sg=12)
    1u64 => 8,    // DAD_SG
    3u64 => 12,   // TOR_SG
    5u64 => 10,   // MAL_SG
    9u64 => 9,    // JAK_SG
    13u64 => 11,  // TED_SG
    14u64 => 7,   // BOR_SG

    // Pistols (NCS: dad_ps=2, jak_ps=3, ord_ps=4, ted_ps=5, tor_ps=6)
    2u64 => 3,    // JAK_PS
    4u64 => 2,    // DAD_PS
    6u64 => 6,    // TOR_PS
    10u64 => 5,   // TED_PS
    12u64 => 3,   // JAK_PS (alternate)

    // Assault Rifles (NCS: dad_ar=13, ted_ar=14, ord_ar=15, tor_ar=17, vla_ar=18, jak_ar=27)
    7u64 => 14,   // TED_AR
    11u64 => 13,  // DAD_AR
    15u64 => 15,  // ORD_AR
    132u64 => 18, // VLA_AR
    136u64 => 17, // TOR_AR
    141u64 => 27, // JAK_AR

    // Snipers (NCS: vla_sr=16, bor_sr=23, jak_sr=24, mal_sr=25, ord_sr=26)
    128u64 => 16, // VLA_SR
    129u64 => 24, // JAK_SR
    133u64 => 26, // ORD_SR
    137u64 => 25, // MAL_SR
    142u64 => 23, // BOR_SR

    // SMGs (NCS: bor_sm=19, dad_sm=20, mal_sm=21, vla_sm=22)
    130u64 => 20, // DAD_SM
    134u64 => 19, // BOR_SM
    138u64 => 21, // MAL_SM
    140u64 => 22, // VLA_SM
};

/// Convert serial ID (first varint) to parts database category
pub fn serial_id_to_parts_category(serial_id: u64) -> u64 {
    SERIAL_TO_PARTS_CAT
        .get(&serial_id)
        .copied()
        .unwrap_or(serial_id)
}

/// VarBit magnitude threshold separating the two divisor regimes.
///
/// Empirically derived: all observed VarBit values with divisor 384 are <= 111,296,
/// and all with divisor 8192 are >= 133,824. The threshold 131,072 = 16 * 8192
/// cleanly separates them. Safe for equipment categories up to ~340.
const VARBIT_DIVISOR_THRESHOLD: u64 = 131_072;

/// Determine the correct divisor for a VarBit-first item based on value magnitude.
///
/// VarBit values encode `category * divisor + metadata`. Two regimes exist:
/// - Small values (< 131,072): divisor 384, categories are equipment/mixed (1-340)
/// - Large values (>= 131,072): divisor 8192, categories are weapons (16-31)
pub fn varbit_divisor(varbit: u64) -> u64 {
    if varbit >= VARBIT_DIVISOR_THRESHOLD {
        8192
    } else {
        384
    }
}

/// Extract NCS category ID from a VarBit value
pub fn category_from_varbit(varbit: u64) -> i64 {
    let divisor = varbit_divisor(varbit);
    (varbit / divisor) as i64
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
/// If raw_decoded_value > 50, our decoding may be wrong.
pub fn level_from_code(code: u64) -> Option<(u8, u8)> {
    const MAX_LEVEL: u8 = 50;

    if code >= 128 {
        let level = 2 * (code as i32 - 120);
        if level > 0 {
            let raw = level as u8;
            let capped = raw.min(MAX_LEVEL);
            Some((capped, raw))
        } else {
            None
        }
    } else if code <= 50 {
        Some((code as u8, code as u8))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_info_lookup() {
        assert_eq!(
            weapon_info_from_first_varint(12),
            Some(("Jakobs", "Pistol"))
        );
        assert_eq!(
            weapon_info_from_first_varint(128),
            Some(("Vladof", "Sniper"))
        );
        assert_eq!(weapon_info_from_first_varint(136), Some(("Torgue", "AR")));
        assert_eq!(weapon_info_from_first_varint(138), Some(("Maliwan", "SMG")));
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
    fn test_varbit_divisor() {
        // Below threshold: equipment divisor
        assert_eq!(varbit_divisor(107200), 384);
        assert_eq!(varbit_divisor(8896), 384);
        assert_eq!(varbit_divisor(111296), 384);
        assert_eq!(varbit_divisor(704), 384);
        assert_eq!(varbit_divisor(0), 384);

        // At/above threshold: weapon divisor
        assert_eq!(varbit_divisor(131072), 8192);
        assert_eq!(varbit_divisor(133824), 8192);
        assert_eq!(varbit_divisor(180928), 8192);
        assert_eq!(varbit_divisor(254656), 8192);
    }

    #[test]
    fn test_category_from_varbit() {
        // Equipment: VarBit / 384
        assert_eq!(category_from_varbit(107200), 279); // Maliwan Shield
        assert_eq!(category_from_varbit(8896), 23);    // Ripper Sniper
        assert_eq!(category_from_varbit(111296), 289);  // Maliwan Heavy Weapon

        // Weapons: VarBit / 8192
        assert_eq!(category_from_varbit(180928), 22);  // Vladof SMG
        assert_eq!(category_from_varbit(133824), 16);  // Vladof Sniper
        assert_eq!(category_from_varbit(254656), 31);  // Unknown (high weapon cat)
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
        assert_eq!(level_from_code(50), Some((50, 50)));
        assert_eq!(level_from_code(128), Some((16, 16)));
        assert_eq!(level_from_code(135), Some((30, 30)));
        assert_eq!(level_from_code(145), Some((50, 50)));
        assert_eq!(level_from_code(150), Some((50, 60)));
        assert_eq!(level_from_code(196), Some((50, 152)));
        assert_eq!(level_from_code(51), None);
        assert_eq!(level_from_code(127), None);
    }

    #[test]
    fn test_serial_id_to_parts_category() {
        assert_eq!(serial_id_to_parts_category(1), 8);
        assert_eq!(serial_id_to_parts_category(9), 9);
        assert_eq!(serial_id_to_parts_category(128), 16);
        assert_eq!(serial_id_to_parts_category(138), 21);
        assert_eq!(serial_id_to_parts_category(999), 999);
    }

    #[test]
    fn test_weapon_type_from_first_varint() {
        assert_eq!(weapon_type_from_first_varint(1), Some("Shotgun"));
        assert_eq!(weapon_type_from_first_varint(2), Some("Pistol"));
        assert_eq!(weapon_type_from_first_varint(128), Some("Sniper"));
        assert_eq!(weapon_type_from_first_varint(138), Some("SMG"));
        assert_eq!(weapon_type_from_first_varint(136), Some("AR"));
        assert_eq!(weapon_type_from_first_varint(999), None);
    }
}
