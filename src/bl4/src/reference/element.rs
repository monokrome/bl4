//! Element type definitions

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
    ElementType {
        code: "sonic",
        name: "Sonic",
        description: "Sonic damage",
        color: "#9B59B6",
    },
];

/// Get element by code
pub fn element_by_code(code: &str) -> Option<&'static ElementType> {
    ELEMENT_TYPES.iter().find(|e| e.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_lookup() {
        assert_eq!(element_by_code("fire").map(|e| e.name), Some("Fire"));
        assert_eq!(element_by_code("cryo").map(|e| e.name), Some("Cryo"));
        assert_eq!(element_by_code("sonic").map(|e| e.name), Some("Sonic"));
    }
}
