//! Rarity tier definitions

/// Rarity tier information
#[derive(Debug, Clone, PartialEq)]
pub struct RarityTier {
    pub tier: u8,
    pub code: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    /// Relative drop weight from NCS rarity_balance table.
    /// Higher = more common. Common=100, Legendary=0.0003.
    pub weight: f64,
}

/// All rarity tiers in order
pub const RARITY_TIERS: &[RarityTier] = &[
    RarityTier {
        tier: 1,
        code: "comp_01",
        name: "Common",
        color: "#FFFFFF",
        weight: 100.0,
    },
    RarityTier {
        tier: 2,
        code: "comp_02",
        name: "Uncommon",
        color: "#00FF00",
        weight: 6.0,
    },
    RarityTier {
        tier: 3,
        code: "comp_03",
        name: "Rare",
        color: "#0080FF",
        weight: 0.14,
    },
    RarityTier {
        tier: 4,
        code: "comp_04",
        name: "Epic",
        color: "#A020F0",
        weight: 0.045,
    },
    RarityTier {
        tier: 5,
        code: "comp_05",
        name: "Legendary",
        color: "#FFA500",
        weight: 0.0003,
    },
];

/// Sum of all rarity weights (precomputed for probability calculations)
const TOTAL_WEIGHT: f64 = 100.0 + 6.0 + 0.14 + 0.045 + 0.0003;

/// Get rarity tier by tier number
pub fn rarity_by_tier(tier: u8) -> Option<&'static RarityTier> {
    RARITY_TIERS.iter().find(|r| r.tier == tier)
}

/// Get rarity tier by code
pub fn rarity_by_code(code: &str) -> Option<&'static RarityTier> {
    RARITY_TIERS.iter().find(|r| r.code == code)
}

/// Get the probability of a rarity tier dropping (weight / total_weight)
pub fn rarity_probability(tier: u8) -> Option<f64> {
    rarity_by_tier(tier).map(|r| r.weight / TOTAL_WEIGHT)
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
    fn test_rarity_weights() {
        assert!((rarity_by_tier(1).unwrap().weight - 100.0).abs() < f64::EPSILON);
        assert!((rarity_by_tier(5).unwrap().weight - 0.0003).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rarity_probability() {
        let common_prob = rarity_probability(1).unwrap();
        let legendary_prob = rarity_probability(5).unwrap();

        // Common should be ~94% of all drops
        assert!(common_prob > 0.94 && common_prob < 0.95);
        // Legendary should be extremely rare
        assert!(legendary_prob < 0.000003);
        assert!(legendary_prob > 0.0000001);
        // All probabilities should sum to 1.0
        let total: f64 = (1..=5).filter_map(rarity_probability).sum();
        assert!((total - 1.0).abs() < 1e-10);
    }
}
