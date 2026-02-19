//! Serial legality validation
//!
//! Checks whether an item serial's parts are plausible — i.e., whether
//! the parts could exist in an unmodified game. Returns tri-state results:
//! Legal, Illegal, or Unknown (insufficient manifest data).

use super::{ItemSerial, Rarity, Token};

/// Tri-state legality result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    /// All verifiable checks pass
    Legal,
    /// At least one check definitively fails
    Illegal,
    /// Insufficient manifest data to determine
    Unknown,
}

impl std::fmt::Display for Legality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legal => write!(f, "legal"),
            Self::Illegal => write!(f, "illegal"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Individual validation check
#[derive(Debug, Clone)]
pub struct ValidationCheck {
    pub name: &'static str,
    /// None = inconclusive, Some(true) = passed, Some(false) = failed
    pub passed: Option<bool>,
    pub detail: String,
}

/// Full validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub legality: Legality,
    pub checks: Vec<ValidationCheck>,
}

impl ValidationResult {
    /// Convert legality to Option<bool> for database storage
    pub fn to_legal_flag(&self) -> Option<bool> {
        match self.legality {
            Legality::Legal => Some(true),
            Legality::Illegal => Some(false),
            Legality::Unknown => None,
        }
    }
}

/// Shared firmware part indices (from dep_table `firmware` namespace)
const FIRMWARE_INDICES: &[u64] = &[103, 112];

/// Count Part tokens in a token stream
fn count_parts(tokens: &[Token]) -> usize {
    tokens
        .iter()
        .filter(|t| matches!(t, Token::Part { .. }))
        .count()
}

/// Check level range: must be 1-50 if present
fn check_level(item: &ItemSerial) -> ValidationCheck {
    match item.level {
        Some(level) if (1..=50).contains(&level) => ValidationCheck {
            name: "level_range",
            passed: Some(true),
            detail: format!("level {} in valid range 1-50", level),
        },
        Some(level) => ValidationCheck {
            name: "level_range",
            passed: Some(false),
            detail: format!("level {} outside valid range 1-50", level),
        },
        None => ValidationCheck {
            name: "level_range",
            passed: None,
            detail: "no level detected".to_string(),
        },
    }
}

/// Threshold below which part indices are category-specific.
/// Indices at or above this are shared dep_table parts (elements, stats, rarity,
/// firmware, payload) and cannot be bounds-checked against category manifests.
const SHARED_PART_THRESHOLD: u64 = 96;

/// Check part index bounds against manifest
fn check_part_bounds(item: &ItemSerial) -> ValidationCheck {
    let inconclusive = |detail: String| ValidationCheck {
        name: "part_bounds",
        passed: None,
        detail,
    };

    let category = match item.parts_category() {
        Some(cat) => cat,
        None => return inconclusive("no category detected".to_string()),
    };
    let max_known = match crate::manifest::max_part_index(category) {
        Some(max) => max,
        None => return inconclusive(format!("category {} not in manifest", category)),
    };

    for token in &item.tokens {
        if let Token::Part { index, .. } = token {
            if (128..=142).contains(index) {
                continue;
            }
            // Strip bit 7 (scope flag) for high indices
            let lookup_index = if *index >= 128 { *index & 0x7F } else { *index };

            // Shared dep_table indices (elements, stats, rarity, firmware) —
            // not category-specific, skip bounds checking
            if lookup_index >= SHARED_PART_THRESHOLD {
                continue;
            }
            if (lookup_index as i64) > max_known {
                return ValidationCheck {
                    name: "part_bounds",
                    passed: Some(false),
                    detail: format!(
                        "part index {} exceeds max known {} for category {}",
                        lookup_index, max_known, category
                    ),
                };
            }
        }
    }

    ValidationCheck {
        name: "part_bounds",
        passed: Some(true),
        detail: format!("all parts within bounds (max {} for category {})", max_known, category),
    }
}

/// Check part count sanity
fn check_part_count(item: &ItemSerial) -> ValidationCheck {
    let part_count = count_parts(&item.tokens);

    if part_count == 0 {
        // Utility items (type 'u') and some class mods may have no Part tokens
        let is_utility = item.item_type == 'u' || item.item_type == '!';
        if is_utility {
            return ValidationCheck {
                name: "part_count",
                passed: None,
                detail: "0 parts (utility/class mod item, may be normal)".to_string(),
            };
        }
        return ValidationCheck {
            name: "part_count",
            passed: Some(false),
            detail: "0 parts on non-utility item".to_string(),
        };
    }

    if part_count > 30 {
        return ValidationCheck {
            name: "part_count",
            passed: Some(false),
            detail: format!("{} parts exceeds maximum reasonable count of 30", part_count),
        };
    }

    ValidationCheck {
        name: "part_count",
        passed: Some(true),
        detail: format!("{} parts", part_count),
    }
}

/// Check rarity-part consistency (firmware parts only on legendaries)
fn check_rarity_consistency(item: &ItemSerial) -> ValidationCheck {
    let rarity = match item.rarity {
        Some(r) => r,
        None => {
            return ValidationCheck {
                name: "rarity_consistency",
                passed: None,
                detail: "no rarity detected".to_string(),
            };
        }
    };

    let has_firmware = item.tokens.iter().any(|t| {
        if let Token::Part { index, .. } = t {
            FIRMWARE_INDICES.contains(index)
        } else {
            false
        }
    });

    if has_firmware && rarity == Rarity::Common {
        return ValidationCheck {
            name: "rarity_consistency",
            passed: Some(false),
            detail: "firmware part on Common item (firmware is legendary-only)".to_string(),
        };
    }

    ValidationCheck {
        name: "rarity_consistency",
        passed: Some(true),
        detail: format!("{} rarity consistent with parts", rarity.name()),
    }
}

impl ItemSerial {
    /// Validate this serial's plausibility.
    ///
    /// Runs a set of checks and returns a tri-state result:
    /// - Legal: all verifiable checks pass
    /// - Illegal: at least one check definitively fails
    /// - Unknown: insufficient manifest data to determine
    pub fn validate(&self) -> ValidationResult {
        let checks = vec![
            check_level(self),
            check_part_bounds(self),
            check_part_count(self),
            check_rarity_consistency(self),
        ];

        let has_failure = checks.iter().any(|c| c.passed == Some(false));
        let has_success = checks.iter().any(|c| c.passed == Some(true));
        let all_conclusive_pass = checks
            .iter()
            .filter(|c| c.passed.is_some())
            .all(|c| c.passed == Some(true));

        let legality = if has_failure {
            Legality::Illegal
        } else if all_conclusive_pass && has_success {
            Legality::Legal
        } else {
            Legality::Unknown
        };

        ValidationResult { legality, checks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_serial_validates() {
        // Known weapon serial
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let result = item.validate();
        for check in &result.checks {
            if check.passed == Some(false) {
                panic!("Check '{}' failed: {}", check.name, check.detail);
            }
        }
        // Should not be Illegal — may be Legal or Unknown depending on manifest
        assert_ne!(result.legality, Legality::Illegal);
    }

    #[test]
    fn test_equipment_serial_validates() {
        // Known shield serial
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        let result = item.validate();
        for check in &result.checks {
            if check.passed == Some(false) {
                panic!("Check '{}' failed: {}", check.name, check.detail);
            }
        }
        assert_ne!(result.legality, Legality::Illegal);
    }

    #[test]
    fn test_legality_display() {
        assert_eq!(Legality::Legal.to_string(), "legal");
        assert_eq!(Legality::Illegal.to_string(), "illegal");
        assert_eq!(Legality::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_validation_result_to_legal_flag() {
        let result = ValidationResult {
            legality: Legality::Legal,
            checks: vec![],
        };
        assert_eq!(result.to_legal_flag(), Some(true));

        let result = ValidationResult {
            legality: Legality::Illegal,
            checks: vec![],
        };
        assert_eq!(result.to_legal_flag(), Some(false));

        let result = ValidationResult {
            legality: Legality::Unknown,
            checks: vec![],
        };
        assert_eq!(result.to_legal_flag(), None);
    }

    #[test]
    fn test_check_level_valid() {
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        let check = check_level(&item);
        // This shield has level 50, should pass
        if item.level.is_some() {
            assert_eq!(check.passed, Some(true));
        }
    }

    #[test]
    fn test_check_part_count_normal() {
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let check = check_part_count(&item);
        assert_eq!(check.passed, Some(true));
    }

    #[test]
    fn test_check_rarity_consistency_common() {
        // A common weapon should not have firmware
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let check = check_rarity_consistency(&item);
        // Should pass or be inconclusive, not fail (this is a real item)
        assert_ne!(check.passed, Some(false));
    }

    #[test]
    fn test_validation_checks_all_present() {
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let result = item.validate();
        assert_eq!(result.checks.len(), 4);

        let check_names: Vec<&str> = result.checks.iter().map(|c| c.name).collect();
        assert!(check_names.contains(&"level_range"));
        assert!(check_names.contains(&"part_bounds"));
        assert!(check_names.contains(&"part_count"));
        assert!(check_names.contains(&"rarity_consistency"));
    }
}
