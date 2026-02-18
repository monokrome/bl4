//! Rarity estimation for decoded item serials

use super::{ItemSerial, Rarity, Token};
use crate::manifest;
use crate::reference::{
    manufacturer_by_name, rarity_probability, weapon_type_by_name, GEAR_TYPES,
};

/// Estimated rarity information for a decoded item serial
#[derive(Debug, Clone)]
pub struct RarityEstimate {
    /// Detected rarity tier
    pub rarity: Rarity,
    /// Probability of this rarity tier dropping (0.0-1.0)
    pub tier_probability: f64,
    /// Number of legendaries in this manufacturer+type pool (Legendary only)
    pub pool_size: Option<u32>,
    /// Total legendaries across all manufacturers in the world drop pool
    pub world_pool_size: Option<u32>,
    /// Per-item probability within the pool (tier_prob / pool_size)
    pub per_item_probability: Option<f64>,
    /// Estimated 1-in-N odds (uses most specific probability available)
    pub one_in: u64,
    /// Category description (e.g., "Jakobs Pistol")
    pub category: Option<String>,
    /// Number of known boss sources for this category
    pub boss_sources: Option<u32>,
}

impl RarityEstimate {
    /// Human-readable odds string
    pub fn odds_display(&self) -> String {
        if self.one_in <= 1 {
            "~100%".to_string()
        } else {
            format!("~1 in {}", format_number(self.one_in))
        }
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn rarity_tier_number(rarity: &Rarity) -> u8 {
    match rarity {
        Rarity::Common => 1,
        Rarity::Uncommon => 2,
        Rarity::Rare => 3,
        Rarity::Epic => 4,
        Rarity::Legendary => 5,
    }
}

/// Extract manufacturer code and gear type code from a decoded serial.
///
/// For VarInt-first (weapons): uses weapon_info() → names → reverse lookup to codes.
/// For VarBit-first (equipment): uses category_name → parse manufacturer + type.
fn extract_codes(serial: &ItemSerial) -> Option<(String, String)> {
    // VarInt-first: weapon_info() returns (manufacturer_name, weapon_type_name)
    if matches!(serial.tokens.first(), Some(Token::VarInt(_))) {
        let (mfr_name, type_name) = serial.weapon_info()?;
        let mfr_code = manufacturer_by_name(mfr_name)?.code;
        let type_code = weapon_type_by_name(type_name)?.code;
        return Some((mfr_code.to_string(), type_code.to_string()));
    }

    // VarBit-first: parse category name
    let category_id = serial.parts_category()?;
    let cat_name = crate::parts::category_name(category_id)?;

    // Category names are like "Maliwan Shield", "Jakobs Repair Kit"
    // Try to split into manufacturer + type
    let first_space = cat_name.find(' ')?;
    let mfr_part = &cat_name[..first_space];
    let type_part = &cat_name[first_space + 1..];

    let mfr_code = manufacturer_by_name(mfr_part)?.code;

    // Try weapon types first, then gear types
    let type_code = if let Some(wt) = weapon_type_by_name(type_part) {
        wt.code.to_string()
    } else {
        // Try gear types (case-insensitive match on name)
        GEAR_TYPES
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(type_part))
            .map(|g| g.code.to_uppercase())?
    };

    Some((mfr_code.to_string(), type_code))
}

impl ItemSerial {
    /// Estimate how rare this item is based on its rarity tier and drop pool data.
    ///
    /// Returns tier probability, pool-adjusted probability for legendaries,
    /// and known boss source counts from the manifest.
    pub fn rarity_estimate(&self) -> Option<RarityEstimate> {
        let rarity = self.rarity?;
        let tier_num = rarity_tier_number(&rarity);
        let tier_probability = rarity_probability(tier_num)?;

        let codes = extract_codes(self);
        let pool = codes
            .as_ref()
            .and_then(|(mfr, gtype)| manifest::drop_pool(mfr, gtype));

        let category = codes.as_ref().and_then(|(mfr, gtype)| {
            let mfr_name = crate::reference::manufacturer_name_by_code(mfr)?;
            let type_name = crate::reference::weapon_type_by_code(gtype)
                .map(|w| w.name)
                .or_else(|| {
                    crate::reference::GEAR_TYPES
                        .iter()
                        .find(|g| g.code.eq_ignore_ascii_case(gtype))
                        .map(|g| g.name)
                })?;
            Some(format!("{} {}", mfr_name, type_name))
        });

        let (pool_size, world_pool_size, per_item, boss_sources) = match (&rarity, pool) {
            (Rarity::Legendary, Some(p)) if p.legendary_count > 0 => {
                let world_total = manifest::world_pool_legendary_count(&p.world_pool_name);
                let per = tier_probability / world_total as f64;
                (
                    Some(p.legendary_count),
                    Some(world_total),
                    Some(per),
                    Some(p.boss_source_count),
                )
            }
            _ => (None, None, None, pool.map(|p| p.boss_source_count)),
        };

        let effective_probability = per_item.unwrap_or(tier_probability);
        let one_in = if effective_probability > 0.0 {
            (1.0 / effective_probability).round() as u64
        } else {
            0
        };

        Some(RarityEstimate {
            rarity,
            tier_probability,
            pool_size,
            world_pool_size,
            per_item_probability: per_item,
            one_in,
            category,
            boss_sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rarity_tier_number() {
        assert_eq!(rarity_tier_number(&Rarity::Common), 1);
        assert_eq!(rarity_tier_number(&Rarity::Legendary), 5);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1), "1");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
        assert_eq!(format_number(353_490), "353,490");
    }

    #[test]
    fn test_odds_display() {
        let est = RarityEstimate {
            rarity: Rarity::Common,
            tier_probability: 0.94,
            pool_size: None,
            world_pool_size: None,
            per_item_probability: None,
            one_in: 1,
            category: None,
            boss_sources: None,
        };
        assert_eq!(est.odds_display(), "~100%");

        let est = RarityEstimate {
            one_in: 353_490,
            ..est
        };
        assert_eq!(est.odds_display(), "~1 in 353,490");
    }

    #[test]
    fn test_rarity_estimate_without_serial() {
        // A minimal serial without enough data returns None
        let serial = ItemSerial {
            original: String::new(),
            raw_bytes: Vec::new(),
            item_type: 'r',
            tokens: Vec::new(),
            manufacturer: None,
            level: None,
            raw_level: None,
            seed: None,
            elements: Vec::new(),
            rarity: None,
        };
        assert!(serial.rarity_estimate().is_none());
    }

    #[test]
    fn test_rarity_estimate_common() {
        let serial = ItemSerial {
            original: String::new(),
            raw_bytes: Vec::new(),
            item_type: 'r',
            tokens: Vec::new(),
            manufacturer: None,
            level: Some(10),
            raw_level: Some(10),
            seed: None,
            elements: Vec::new(),
            rarity: Some(Rarity::Common),
        };
        let est = serial.rarity_estimate().unwrap();
        assert_eq!(est.rarity, Rarity::Common);
        assert!(est.tier_probability > 0.94);
        assert_eq!(est.one_in, 1);
        assert!(est.pool_size.is_none());
    }
}
