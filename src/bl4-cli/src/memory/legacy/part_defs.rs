//! Part Definition Types
//!
//! Types and category mapping for part definitions.

/// Serial number index from the GBX format
pub struct GbxSerialNumberIndex {
    pub category: i64,
    pub scope: u8,
    pub status: u8,
    pub index: i16,
}

/// Extracted part definition with its serial number index
#[derive(Debug, Clone)]
pub struct PartDefinition {
    pub name: String,
    pub category: i64,
    pub index: i16,
    pub object_address: usize,
}

/// Map part name prefix to Part Group ID (category)
pub fn get_category_for_part(name: &str) -> Option<i64> {
    // Extract prefix (everything before ".part_")
    let prefix = name.split(".part_").next()?.to_lowercase();

    // Map prefixes to Part Group IDs (derived from reference data)
    match prefix.as_str() {
        // Pistols (2-6)
        "dad_ps" => Some(2),
        "jak_ps" => Some(3),
        "ted_ps" => Some(4),
        "tor_ps" => Some(5),
        "ord_ps" => Some(6),

        // Shotguns (8-12)
        "dad_sg" => Some(8),
        "jak_sg" => Some(9),
        "ted_sg" => Some(10),
        "tor_sg" => Some(11),
        "bor_sg" => Some(12),

        // Assault Rifles (13-18)
        "dad_ar" => Some(13),
        "jak_ar" => Some(14),
        "ted_ar" => Some(15),
        "tor_ar" => Some(16),
        "vla_ar" => Some(17),
        "ord_ar" => Some(18),

        // SMGs (19-24)
        "mal_sg" => Some(19), // Maliwan SG is actually an SMG category
        "dad_sm" => Some(20),
        "bor_sm" => Some(21),
        "vla_sm" => Some(22),
        "mal_sm" => Some(23),

        // Snipers (25-29)
        "bor_sr" => Some(25),
        "jak_sr" => Some(26),
        "ord_sr" => Some(28),
        "mal_sr" => Some(29),

        // Class mods
        "classmod_gravitar" | "classmod" => Some(97),

        // Heavy Weapons (244-247)
        "vla_hw" => Some(244),
        "tor_hw" => Some(245),
        "bor_hw" => Some(246),
        "mal_hw" => Some(247),

        // Shields (279-288)
        "energy_shield" => Some(279),
        "bor_shield" => Some(280),
        "dad_shield" => Some(281),
        "jak_shield" => Some(282),
        "armor_shield" => Some(283),
        "mal_shield" => Some(284),
        "ord_shield" => Some(285),
        "ted_shield" => Some(286),
        "tor_shield" => Some(287),

        // Gadgets (300-330)
        "grenade_gadget" | "mal_grenade_gadget" => Some(300),
        "turret_gadget" | "weapon_turret" => Some(310),
        "repair_kit" | "dad_repair_kit" => Some(320),
        "terminal_gadget" | "dad_terminal" | "mal_terminal" | "ord_terminal" | "ted_terminal" => {
            Some(330)
        }

        // Enhancements (400-409)
        "dad_enhancement" | "enhancement" => Some(400),
        "bor_enhancement" => Some(401),
        "jak_enhancement" => Some(402),
        "mal_enhancement" => Some(403),
        "ord_enhancement" => Some(404),
        "ted_enhancement" => Some(405),
        "tor_enhancement" => Some(406),
        "vla_enhancement" => Some(407),
        "cov_enhancement" => Some(408),
        "atl_enhancement" => Some(409),

        // Shield parts
        "shield" => Some(279),

        // Weapon parts for special weapons
        "weapon_brute" | "weapon_ripperturret" => Some(310),

        // Fallback: try to match partial prefixes
        other => {
            if other.ends_with("_ps") {
                Some(2)
            } else if other.ends_with("_sg") {
                Some(8)
            } else if other.ends_with("_ar") {
                Some(13)
            } else if other.ends_with("_sm") {
                Some(20)
            } else if other.ends_with("_sr") {
                Some(25)
            } else if other.ends_with("_hw") {
                Some(244)
            } else if other.contains("shield") {
                Some(279)
            } else if other.contains("gadget") {
                Some(300)
            } else if other.contains("enhancement") {
                Some(400)
            } else if other.contains("terminal") {
                Some(330)
            } else if other.contains("turret") {
                Some(310)
            } else if other.contains("repair") {
                Some(320)
            } else if other.contains("grenade") {
                Some(300)
            } else {
                None
            }
        }
    }
}
