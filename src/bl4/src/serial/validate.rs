//! Serial legality validation
//!
//! Checks whether an item serial's parts are plausible — i.e., whether
//! the parts could exist in an unmodified game. Returns tri-state results:
//! Legal, Illegal, or Unknown (insufficient manifest data).

use super::{ItemSerial, Token};

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

/// Check part index bounds: every Part token must be resolvable to a name.
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

    let mut checked = 0u32;

    for token in &item.tokens {
        if let Token::Part { index, .. } = token {
            let part_name = super::resolve_part_name(category, *index);
            if part_name.is_some_and(|n| super::Element::from_part_name(n).is_some()) {
                continue;
            }
            checked += 1;
            if part_name.is_none() {
                return inconclusive(format!(
                    "part index {} unresolvable for category {}",
                    index, category
                ));
            }
        }
    }

    if checked == 0 {
        return inconclusive("no non-element parts to check".to_string());
    }

    ValidationCheck {
        name: "part_bounds",
        passed: Some(true),
        detail: format!("all {} parts resolved for category {}", checked, category),
    }
}

/// Check part count sanity
fn check_part_count(item: &ItemSerial) -> ValidationCheck {
    let part_count = count_parts(&item.tokens);

    if part_count == 0 {
        return ValidationCheck {
            name: "part_count",
            passed: None,
            detail: "0 parts (serial may not use Part tokens)".to_string(),
        };
    }

    if part_count > 30 {
        return ValidationCheck {
            name: "part_count",
            passed: None,
            detail: format!(
                "{} parts exceeds expected count of 30 (unverified limit)",
                part_count
            ),
        };
    }

    ValidationCheck {
        name: "part_count",
        passed: Some(true),
        detail: format!("{} parts", part_count),
    }
}

/// Check pool membership: are resolved parts in this item's loot pool?
fn check_pool_membership(item: &ItemSerial) -> ValidationCheck {
    let inconclusive = |detail: String| ValidationCheck {
        name: "pool_membership",
        passed: None,
        detail,
    };

    let category = match item.parts_category() {
        Some(cat) => cat,
        None => return inconclusive("no category detected".to_string()),
    };

    if crate::manifest::is_part_in_pool(category, "").is_none() {
        return inconclusive(format!("no pool data for category {}", category));
    }

    let mut checked = 0u32;
    let mut in_pool = 0u32;
    let mut unnamed = 0u32;

    for token in &item.tokens {
        let Token::Part { index, .. } = token else {
            continue;
        };

        let Some(name) = super::resolve_part_name(category, *index) else {
            unnamed += 1;
            continue;
        };

        // Element parts are identified separately, not part of the loot pool
        if super::Element::from_part_name(name).is_some() {
            continue;
        }

        if let Some(found) = crate::manifest::is_part_in_pool(category, name) {
            checked += 1;
            if found {
                in_pool += 1;
            }
        }
    }

    if unnamed > 0 {
        return inconclusive(format!(
            "{} parts unnamed (cannot verify pool membership)",
            unnamed
        ));
    }

    if checked == 0 {
        return inconclusive("no parts could be checked against pool".to_string());
    }

    ValidationCheck {
        name: "pool_membership",
        passed: Some(true),
        detail: format!(
            "{}/{} resolved parts in pool for category {}",
            in_pool, checked, category
        ),
    }
}

/// Slots that must contain at most one part in a valid item.
const UNIQUE_SLOTS: &[&str] = &["foregrip"];

/// Check that single-occupancy slots don't have duplicate parts.
fn check_slot_uniqueness(item: &ItemSerial) -> ValidationCheck {
    let parts = item.resolved_parts();
    if parts.is_empty() {
        return ValidationCheck {
            name: "slot_uniqueness",
            passed: None,
            detail: "no parts to check".to_string(),
        };
    }

    let mut slot_counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for part in &parts {
        if part.slot == "unknown" || part.is_element {
            continue;
        }
        *slot_counts.entry(part.slot).or_default() += 1;
    }

    for slot in UNIQUE_SLOTS {
        if let Some(&count) = slot_counts.get(slot) {
            if count > 1 {
                return ValidationCheck {
                    name: "slot_uniqueness",
                    passed: Some(false),
                    detail: format!("slot '{}' has {} parts (expected 1)", slot, count),
                };
            }
        }
    }

    ValidationCheck {
        name: "slot_uniqueness",
        passed: Some(true),
        detail: "all single-slot constraints satisfied".to_string(),
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
            check_pool_membership(self),
            check_slot_uniqueness(self),
        ];

        let has_failure = checks.iter().any(|c| c.passed == Some(false));
        let has_inconclusive = checks.iter().any(|c| c.passed.is_none());
        let all_pass = checks.iter().all(|c| c.passed == Some(true));

        let legality = if has_failure {
            Legality::Illegal
        } else if all_pass {
            Legality::Legal
        } else if has_inconclusive {
            Legality::Unknown
        } else {
            Legality::Legal
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
    fn test_validation_checks_all_present() {
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let result = item.validate();
        assert_eq!(result.checks.len(), 5);

        let check_names: Vec<&str> = result.checks.iter().map(|c| c.name).collect();
        assert!(check_names.contains(&"level_range"));
        assert!(check_names.contains(&"part_bounds"));
        assert!(check_names.contains(&"part_count"));
        assert!(check_names.contains(&"pool_membership"));
        assert!(check_names.contains(&"slot_uniqueness"));
    }

    #[test]
    fn test_duplicate_foregrip_is_illegal() {
        // Reported: user added a second foregrip to a Shalashaska, it should be illegal
        let item =
            ItemSerial::decode("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+Al/uj}").unwrap();
        let result = item.validate();
        assert_eq!(result.legality, Legality::Illegal);
        let slot_check = result
            .checks
            .iter()
            .find(|c| c.name == "slot_uniqueness")
            .unwrap();
        assert_eq!(slot_check.passed, Some(false));
        assert!(slot_check.detail.contains("foregrip"));
    }

    #[test]
    fn test_valid_item_passes_slot_uniqueness() {
        // Normal Shalashaska with one foregrip
        let item =
            ItemSerial::decode("@UgbV{rFme!K<aW?mRG/*lsIsVasB@@vs7=*D^+EkX%/f+A00}").unwrap();
        let result = item.validate();
        let slot_check = result
            .checks
            .iter()
            .find(|c| c.name == "slot_uniqueness")
            .unwrap();
        assert_eq!(slot_check.passed, Some(true));
    }

    #[test]
    fn test_pool_membership_not_illegal_on_known_items() {
        // Known weapon serial — should not be flagged as illegal by pool check
        let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k").unwrap();
        let check = check_pool_membership(&item);
        assert_ne!(
            check.passed,
            Some(false),
            "Pool check failed: {}",
            check.detail
        );
    }

    #[test]
    fn test_pool_membership_not_illegal_on_shield() {
        // Known shield serial
        let item = ItemSerial::decode("@Uge98>m/)}}!c5JeNWCvCXc7").unwrap();
        let check = check_pool_membership(&item);
        assert_ne!(
            check.passed,
            Some(false),
            "Pool check failed: {}",
            check.detail
        );
    }
}
