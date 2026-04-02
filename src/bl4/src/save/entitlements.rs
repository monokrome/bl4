//! Entitlement detection from profile.sav unlockable data.
//!
//! Detects pre-order, premium/deluxe edition, and other paid content
//! ownership by checking which cosmetic unlockables are present.

use serde::Serialize;

/// Detected entitlements from a profile save file.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Entitlements {
    pub preorder: bool,
    pub premium_edition: bool,
    pub golden_power: bool,
}

/// Marker suffixes that indicate entitlement ownership.
const PREORDER_MARKER: &str = "PreOrder";
const PREMIUM_MARKERS: &[&str] = &["Body02_Premium", "Head16_Premium", "Skin44_Premium"];
const GOLDEN_POWER_MARKER: &str = "GoldenPower";

/// Detect entitlements from profile.sav YAML data.
///
/// Scans all unlockable entry lists under `domains.local.unlockables`
/// for known entitlement markers.
pub fn detect_entitlements(data: &serde_yaml::Value) -> Entitlements {
    let mut result = Entitlements::default();

    let unlockables = data
        .get("domains")
        .and_then(|d| d.get("local"))
        .and_then(|l| l.get("unlockables"));

    let Some(unlockables) = unlockables else {
        return result;
    };

    let Some(map) = unlockables.as_mapping() else {
        return result;
    };

    for (_key, section) in map {
        let Some(entries) = section.get("entries").and_then(|e| e.as_sequence()) else {
            continue;
        };

        for entry in entries {
            let Some(name) = entry.as_str() else {
                continue;
            };

            let suffix = name.split('.').next_back().unwrap_or(name);

            if suffix.contains(PREORDER_MARKER) {
                result.preorder = true;
            }
            if PREMIUM_MARKERS.contains(&suffix) {
                result.premium_edition = true;
            }
            if suffix.contains(GOLDEN_POWER_MARKER) {
                result.golden_power = true;
            }
        }
    }

    result
}
