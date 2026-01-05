//! Manufacturer definitions

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
        name: "Ripper",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manufacturer_lookup() {
        assert_eq!(manufacturer_by_code("JAK").map(|m| m.name), Some("Jakobs"));
        assert_eq!(manufacturer_by_code("TOR").map(|m| m.name), Some("Torgue"));
        assert_eq!(manufacturer_name_by_code("VLA"), Some("Vladof"));
    }
}
