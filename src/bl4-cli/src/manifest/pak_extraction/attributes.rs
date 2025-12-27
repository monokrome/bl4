//! Attribute extraction from PAK files
//!
//! Extracts authoritative element, rarity, and stat data from pak_manifest.json.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::manifest::PakManifest;

/// Extracted element type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedElement {
    /// Internal name from game paths (e.g., "Fire", "Cryo", "Shock")
    pub internal_name: String,
    /// Example paths where this element appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract element types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers element types from game effect/texture paths:
/// - /Common/Effects/Textures/Elements/<ElementType>/
/// - /Common/Effects/Materials/Elements/<ElementType>/
pub fn extract_elements_from_pak(
    pak_manifest_path: &Path,
) -> Result<HashMap<String, ExtractedElement>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut elements: HashMap<String, ExtractedElement> = HashMap::new();

    // Pattern to find element types from effect paths
    // e.g., /Common/Effects/Textures/Elements/Fire/
    let element_pattern =
        Regex::new(r"/(?:Effects|Materials)/(?:Textures|Materials)?/?Elements/([A-Za-z]+)/")
            .unwrap();

    for item in &manifest.items {
        let path = &item.path;

        if let Some(cap) = element_pattern.captures(path) {
            let element_name = cap[1].to_string();

            // Skip partial matches (like "M" or "T" from file prefixes)
            if element_name.len() < 3 {
                continue;
            }

            let elem = elements
                .entry(element_name.clone())
                .or_insert_with(|| ExtractedElement {
                    internal_name: element_name.clone(),
                    example_paths: Vec::new(),
                });

            if elem.example_paths.len() < 3 {
                elem.example_paths.push(path.clone());
            }
        }
    }

    Ok(elements)
}

/// Extracted rarity tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRarity {
    /// Tier number (1-5)
    pub tier: u8,
    /// Internal code (e.g., "comp_01")
    pub code: String,
    /// Name from game (e.g., "common", "legendary")
    pub name: String,
    /// Example paths where this rarity appears
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub example_paths: Vec<String>,
}

/// Extract rarity tiers from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers rarity tiers from UI rarity pip assets:
/// - rarity_pip_01_common, rarity_pip_02_uncommon, etc.
pub fn extract_rarities_from_pak(pak_manifest_path: &Path) -> Result<Vec<ExtractedRarity>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut rarities: HashMap<u8, ExtractedRarity> = HashMap::new();

    // Pattern to find rarity tiers from UI pip assets
    // e.g., rarity_pip_01_common, rarity_pip_05_legendary
    let rarity_pip_pattern = Regex::new(r"rarity_pip_(\d{2})_([a-z]+)").unwrap();

    // Also look for comp_XX patterns in part names
    let comp_pattern = Regex::new(r"comp_(\d{2})_([a-z]+)").unwrap();

    for item in &manifest.items {
        let path = &item.path;
        let path_lower = path.to_lowercase();

        // Check for rarity pip assets
        if let Some(cap) = rarity_pip_pattern.captures(&path_lower) {
            let tier: u8 = cap[1].parse().unwrap_or(0);
            let name = cap[2].to_string();

            if (1..=5).contains(&tier) {
                let rarity = rarities.entry(tier).or_insert_with(|| ExtractedRarity {
                    tier,
                    code: format!("comp_{:02}", tier),
                    name: name.clone(),
                    example_paths: Vec::new(),
                });

                if rarity.example_paths.len() < 3 {
                    rarity.example_paths.push(path.clone());
                }
            }
        }

        // Also check property names for comp_XX patterns
        for prop in &item.property_names {
            let prop_lower = prop.to_lowercase();
            if let Some(cap) = comp_pattern.captures(&prop_lower) {
                let tier: u8 = cap[1].parse().unwrap_or(0);
                let name = cap[2].to_string();

                if (1..=5).contains(&tier) {
                    let rarity = rarities.entry(tier).or_insert_with(|| ExtractedRarity {
                        tier,
                        code: format!("comp_{:02}", tier),
                        name: name.clone(),
                        example_paths: Vec::new(),
                    });

                    // Update name if we find it from parts (more reliable than UI)
                    if rarity.name.is_empty() || rarity.name == "unknown" {
                        rarity.name = name;
                    }
                }
            }
        }
    }

    // Convert to sorted vector
    let mut result: Vec<ExtractedRarity> = rarities.into_values().collect();
    result.sort_by_key(|r| r.tier);

    Ok(result)
}

/// Extracted stat type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedStat {
    /// Stat name (e.g., "Damage", "Accuracy", "Spread")
    pub name: String,
    /// Modifier types found for this stat (Scale, Value, Add, Percent)
    pub modifier_types: Vec<String>,
    /// Number of occurrences in game data
    pub occurrences: usize,
}

/// Extract stat types from pak_manifest.json (AUTHORITATIVE)
///
/// Discovers stat types from property names in game assets:
/// - Pattern: StatName_ModifierType_Index_GUID
/// - e.g., Damage_Scale_44_..., Accuracy_Value_38_...
pub fn extract_stats_from_pak(pak_manifest_path: &Path) -> Result<Vec<ExtractedStat>> {
    let content =
        fs::read_to_string(pak_manifest_path).context("Failed to read pak_manifest.json")?;
    let manifest: PakManifest =
        serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;

    let mut stats: HashMap<String, (std::collections::HashSet<String>, usize)> = HashMap::new();

    // Pattern to find stat properties
    // e.g., Damage_Scale_44_GUID, Accuracy_Value_38_GUID
    let stat_pattern =
        Regex::new(r"^([A-Z][a-zA-Z]+)_(Scale|Add|Value|Percent)_\d+_[A-F0-9]{32}$").unwrap();

    // Also simpler patterns like StatName_Index_GUID
    let simple_stat_pattern = Regex::new(r"^([A-Z][a-zA-Z]+)_\d+_[A-F0-9]{32}$").unwrap();

    // Known stat-like property prefixes to look for
    let stat_prefixes = [
        "Accuracy",
        "Damage",
        "CritDamage",
        "FireRate",
        "ReloadTime",
        "ReloadSpeed",
        "MagSize",
        "Spread",
        "Recoil",
        "Sway",
        "Ammo",
        "AmmoCost",
        "Capacity",
        "Cooldown",
        "Duration",
        "Healing",
        "Health",
        "Impulse",
        "Projectile",
        "Radius",
        "Regen",
        "Speed",
        "StatusChance",
        "StatusDamage",
        "ElementalPower",
        "DamageRadius",
        "EquipTime",
        "PutDownTime",
        "ZoomDuration",
        "AccImpulse",
        "AccRegen",
        "AccDelay",
        "ProjectilesPerShot",
    ];

    for item in &manifest.items {
        for prop in &item.property_names {
            // Check for StatName_ModifierType_Index_GUID pattern
            if let Some(cap) = stat_pattern.captures(prop) {
                let stat_name = cap[1].to_string();
                let modifier_type = cap[2].to_string();

                let entry = stats
                    .entry(stat_name)
                    .or_insert_with(|| (std::collections::HashSet::new(), 0));
                entry.0.insert(modifier_type);
                entry.1 += 1;
            }

            // Also check simple pattern for known stats
            if let Some(cap) = simple_stat_pattern.captures(prop) {
                let stat_name = cap[1].to_string();
                if stat_prefixes.contains(&stat_name.as_str()) {
                    let entry = stats
                        .entry(stat_name)
                        .or_insert_with(|| (std::collections::HashSet::new(), 0));
                    entry.1 += 1;
                }
            }
        }
    }

    // Convert to sorted vector
    let mut result: Vec<ExtractedStat> = stats
        .into_iter()
        .map(|(name, (modifiers, count))| {
            let mut modifier_types: Vec<String> = modifiers.into_iter().collect();
            modifier_types.sort();
            ExtractedStat {
                name,
                modifier_types,
                occurrences: count,
            }
        })
        .collect();

    result.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));

    Ok(result)
}
