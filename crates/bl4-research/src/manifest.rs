//! Manifest extraction from game files
//!
//! Extracts game data from unpacked .uasset files into organized JSON manifest files.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

/// Known manufacturer codes and their full names
pub fn manufacturer_names() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("BOR", "Borg");
    m.insert("DAD", "Daedalus");
    m.insert("DPL", "Dahl");
    m.insert("JAK", "Jakobs");
    m.insert("MAL", "Maliwan");
    m.insert("ORD", "Order");
    m.insert("RIP", "Ripper");
    m.insert("TED", "Tediore");
    m.insert("TOR", "Torgue");
    m.insert("VLA", "Vladof");
    m.insert("COV", "Children of the Vault");
    m
}

/// Known stat properties and their descriptions
pub fn stat_descriptions() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("Damage", "Base damage");
    m.insert("CritDamage", "Critical hit damage");
    m.insert("FireRate", "Firing rate");
    m.insert("ReloadTime", "Reload time");
    m.insert("MagSize", "Magazine size");
    m.insert("Accuracy", "Base accuracy");
    m.insert("AccImpulse", "Accuracy impulse (recoil recovery)");
    m.insert("AccRegen", "Accuracy regeneration");
    m.insert("AccDelay", "Accuracy delay");
    m.insert("Spread", "Projectile spread");
    m.insert("Recoil", "Weapon recoil");
    m.insert("Sway", "Weapon sway");
    m.insert("ProjectilesPerShot", "Pellets per shot");
    m.insert("AmmoCost", "Ammo consumption");
    m.insert("StatusChance", "Status effect chance");
    m.insert("StatusDamage", "Status effect damage");
    m.insert("EquipTime", "Weapon equip time");
    m.insert("PutDownTime", "Weapon holster time");
    m.insert("ZoomDuration", "ADS zoom time");
    m.insert("ElementalPower", "Elemental damage bonus");
    m.insert("DamageRadius", "Splash damage radius");
    m
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manufacturer {
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_data_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeaponType {
    pub name: String,
    pub path: String,
    pub manufacturers: Vec<ManufacturerRef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManufacturerRef {
    pub code: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatEntry {
    pub index: u32,
    pub guid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatProperty {
    pub stat: String,
    #[serde(rename = "type")]
    pub modifier_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entries: Vec<StatEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PropertyEntry {
    pub index: u32,
    pub guid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetInfo {
    pub name: String,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<HashMap<String, StatProperty>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Vec<PropertyEntry>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_strings: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceCategory {
    pub name: String,
    pub path: String,
    pub assets: Vec<AssetInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GearType {
    pub name: String,
    pub path: String,
    pub balance_data: Vec<AssetInfo>,
    pub manufacturers: Vec<ManufacturerRef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManifestIndex {
    pub version: String,
    pub source: String,
    pub extract_path: String,
    pub files: HashMap<String, String>,
}

/// Item pool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPool {
    /// Pool name (e.g., "itempool_guns_01_common")
    pub name: String,
    /// Full path to the pool definition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Assets that reference this pool
    pub referenced_by: Vec<String>,
    /// Items/pools this pool contains (if discoverable)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contains: Vec<String>,
}

/// Item stats with all modifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStats {
    /// Item name/path
    pub name: String,
    /// Item category (weapon, shield, grenade, etc.)
    pub category: String,
    /// Manufacturer code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Rarity (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rarity: Option<String>,
    /// Stat modifiers
    pub stats: HashMap<String, Vec<StatModifier>>,
    /// Drop pools this item appears in
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub drop_pools: Vec<String>,
}

/// Individual stat modifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatModifier {
    /// Modifier type (Scale, Add, Value, Percent)
    pub modifier_type: String,
    /// Modifier index
    pub index: u32,
    /// GUID reference
    pub guid: String,
}

/// Complete items database
#[derive(Debug, Serialize, Deserialize)]
pub struct ItemsDatabase {
    pub version: String,
    pub generated: String,
    pub item_pools: HashMap<String, ItemPool>,
    pub items: Vec<ItemStats>,
    pub stats_summary: StatsSummary,
}

/// Summary of all stats found
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsSummary {
    pub total_items: usize,
    pub total_pools: usize,
    pub stat_types: Vec<String>,
    pub categories: Vec<String>,
    pub manufacturers: Vec<String>,
}

/// Extract readable strings from a uasset file using the `strings` command
pub fn extract_strings(uasset_path: &Path) -> Result<String> {
    let output = Command::new("strings")
        .arg(uasset_path)
        .output()
        .context("Failed to run strings command")?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse property names and GUIDs from strings output
/// Pattern: PropertyName_Number_GUID
pub fn parse_property_strings(content: &str) -> HashMap<String, Vec<PropertyEntry>> {
    let pattern = Regex::new(r"([A-Za-z_]+)_(\d+)_([A-F0-9]{32})").unwrap();
    let mut properties: HashMap<String, Vec<PropertyEntry>> = HashMap::new();

    for cap in pattern.captures_iter(content) {
        let prop_name = cap[1].to_string();
        let prop_index: u32 = cap[2].parse().unwrap_or(0);
        let prop_guid = cap[3].to_string();

        properties
            .entry(prop_name)
            .or_default()
            .push(PropertyEntry {
                index: prop_index,
                guid: prop_guid,
            });
    }

    properties
}

/// Parse stat modifier properties (Scale, Add, Value, Percent, etc.)
/// Pattern: StatName_Type_Number_GUID
pub fn parse_stat_properties(content: &str) -> HashMap<String, StatProperty> {
    let pattern = Regex::new(r"([A-Za-z_]+)_(Scale|Add|Value|Percent)_(\d+)_([A-F0-9]{32})").unwrap();
    let stat_desc = stat_descriptions();
    let mut stats: HashMap<String, StatProperty> = HashMap::new();

    for cap in pattern.captures_iter(content) {
        let stat_name = cap[1].to_string();
        let modifier_type = cap[2].to_string();
        let stat_index: u32 = cap[3].parse().unwrap_or(0);
        let stat_guid = cap[4].to_string();

        let key = format!("{}_{}", stat_name, modifier_type);
        let entry = stats.entry(key).or_insert_with(|| StatProperty {
            stat: stat_name.clone(),
            modifier_type: modifier_type.clone(),
            description: stat_desc.get(stat_name.as_str()).map(|s| s.to_string()),
            entries: Vec::new(),
        });

        entry.entries.push(StatEntry {
            index: stat_index,
            guid: stat_guid,
        });
    }

    stats
}

/// Extract manufacturers from game files
pub fn extract_manufacturers(extract_dir: &Path) -> HashMap<String, Manufacturer> {
    let mfr_names = manufacturer_names();
    let mut manufacturers: HashMap<String, Manufacturer> = HashMap::new();

    // Search locations for manufacturer codes
    let search_paths = [
        "OakGame/Content/Gear/Weapons/_Manufacturer",
        "OakGame/Content/Gear/Weapons/_Shared/BalanceData",
        "OakGame/Content/Gear/Weapons/_Shared/Materials",
        "OakGame/Content/Gear/_Shared/Materials/Materials",
        "OakGame/Content/Gear/GrenadeGadgets/Manufacturer",
        "OakGame/Content/Gear/shields/Manufacturer",
        "OakGame/Content/Gear/Gadgets/Turrets",
    ];

    // Also scan weapon type directories for manufacturer subdirs
    let weapon_types_dir = extract_dir.join("OakGame/Content/Gear/Weapons");
    if weapon_types_dir.exists() {
        if let Ok(entries) = fs::read_dir(&weapon_types_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if !dir_name.starts_with('_') {
                        // Scan for manufacturer subdirs in each weapon type
                        if let Ok(mfr_entries) = fs::read_dir(&entry.path()) {
                            for mfr_entry in mfr_entries.flatten() {
                                if mfr_entry.path().is_dir() {
                                    let code = mfr_entry.file_name().to_string_lossy().to_string();
                                    if mfr_names.contains_key(code.as_str()) {
                                        manufacturers.entry(code.clone()).or_insert_with(|| {
                                            Manufacturer {
                                                code: code.clone(),
                                                name: mfr_names.get(code.as_str()).unwrap_or(&code.as_str()).to_string(),
                                                path: None,
                                                balance_data_path: None,
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for search_path in &search_paths {
        let search_dir = extract_dir.join(search_path);
        if !search_dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let code = entry.file_name().to_string_lossy().to_string();
                    // Only include known manufacturer codes
                    if mfr_names.contains_key(code.as_str()) {
                        let rel_path = entry
                            .path()
                            .strip_prefix(extract_dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .ok();

                        // Determine path type based on search location
                        let is_balance_data = search_path.contains("BalanceData");

                        manufacturers
                            .entry(code.clone())
                            .and_modify(|m| {
                                if is_balance_data && m.balance_data_path.is_none() {
                                    m.balance_data_path = rel_path.clone();
                                } else if !is_balance_data && m.path.is_none() {
                                    m.path = rel_path.clone();
                                }
                            })
                            .or_insert(Manufacturer {
                                code: code.clone(),
                                name: mfr_names.get(code.as_str()).unwrap_or(&code.as_str()).to_string(),
                                path: if is_balance_data { None } else { rel_path.clone() },
                                balance_data_path: if is_balance_data { rel_path } else { None },
                            });
                    }
                }
            }
        }
    }

    manufacturers
}

/// Extract weapon type data
pub fn extract_weapon_types(extract_dir: &Path) -> HashMap<String, WeaponType> {
    let mfr_names = manufacturer_names();
    let mut weapon_types: HashMap<String, WeaponType> = HashMap::new();

    let weapons_dir = extract_dir.join("OakGame/Content/Gear/Weapons");
    if !weapons_dir.exists() {
        return weapon_types;
    }

    if let Ok(entries) = fs::read_dir(&weapons_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let type_name = entry.file_name().to_string_lossy().to_string();
                // Skip internal directories
                if type_name.starts_with('_') {
                    continue;
                }

                let rel_path = entry
                    .path()
                    .strip_prefix(extract_dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let mut manufacturers = Vec::new();
                if let Ok(mfr_entries) = fs::read_dir(&entry.path()) {
                    for mfr_entry in mfr_entries.flatten() {
                        if mfr_entry.path().is_dir() {
                            let code = mfr_entry.file_name().to_string_lossy().to_string();
                            let mfr_rel_path = mfr_entry
                                .path()
                                .strip_prefix(extract_dir)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();

                            manufacturers.push(ManufacturerRef {
                                code: code.clone(),
                                name: mfr_names.get(code.as_str()).unwrap_or(&code.as_str()).to_string(),
                                path: mfr_rel_path,
                            });
                        }
                    }
                }

                weapon_types.insert(
                    type_name.clone(),
                    WeaponType {
                        name: type_name,
                        path: rel_path,
                        manufacturers,
                    },
                );
            }
        }
    }

    weapon_types
}

/// Extract balance data from game files
pub fn extract_balance_data(extract_dir: &Path) -> Result<HashMap<String, BalanceCategory>> {
    let mut balance_data: HashMap<String, BalanceCategory> = HashMap::new();

    let balance_dir = extract_dir.join("OakGame/Content/Gear/Weapons/_Shared/BalanceData");
    if !balance_dir.exists() {
        return Ok(balance_data);
    }

    if let Ok(entries) = fs::read_dir(&balance_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let category_name = entry.file_name().to_string_lossy().to_string();
                let rel_path = entry
                    .path()
                    .strip_prefix(extract_dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let mut assets = Vec::new();

                // Find all .uasset files in this category
                for asset_entry in WalkDir::new(&entry.path())
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "uasset")
                            .unwrap_or(false)
                    })
                {
                    let asset_path = asset_entry.path();
                    let mut asset_info = AssetInfo {
                        name: asset_path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        file: asset_path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        path: asset_path
                            .strip_prefix(extract_dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .ok(),
                        stats: None,
                        properties: None,
                        raw_strings: None,
                    };

                    // Extract strings and parse properties
                    if let Ok(content) = extract_strings(asset_path) {
                        let stats = parse_stat_properties(&content);
                        if !stats.is_empty() {
                            asset_info.stats = Some(stats);
                        }

                        let props = parse_property_strings(&content);
                        if !props.is_empty() {
                            asset_info.properties = Some(props);
                        }
                    }

                    assets.push(asset_info);
                }

                balance_data.insert(
                    category_name.clone(),
                    BalanceCategory {
                        name: category_name,
                        path: rel_path,
                        assets,
                    },
                );
            }
        }
    }

    Ok(balance_data)
}

/// Extract naming data
pub fn extract_naming_data(extract_dir: &Path) -> Result<HashMap<String, AssetInfo>> {
    let mut naming_data: HashMap<String, AssetInfo> = HashMap::new();

    let naming_dir = extract_dir.join("OakGame/Content/Gear/Weapons/_Shared/NamingStrategies");
    if !naming_dir.exists() {
        return Ok(naming_data);
    }

    for entry in WalkDir::new(&naming_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "uasset")
                .unwrap_or(false)
        })
    {
        let asset_path = entry.path();
        let name = asset_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut asset_info = AssetInfo {
            name: name.clone(),
            file: asset_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: asset_path
                .strip_prefix(extract_dir)
                .map(|p| p.to_string_lossy().to_string())
                .ok(),
            stats: None,
            properties: None,
            raw_strings: None,
        };

        if let Ok(content) = extract_strings(asset_path) {
            let props = parse_property_strings(&content);
            if !props.is_empty() {
                asset_info.properties = Some(props);
            }
        }

        naming_data.insert(name, asset_info);
    }

    Ok(naming_data)
}

/// Extract all gear types (shields, grenades, gadgets, etc.)
pub fn extract_gear_types(extract_dir: &Path) -> HashMap<String, GearType> {
    let mfr_names = manufacturer_names();
    let mut gear_types: HashMap<String, GearType> = HashMap::new();

    let gear_dir = extract_dir.join("OakGame/Content/Gear");
    if !gear_dir.exists() {
        return gear_types;
    }

    if let Ok(entries) = fs::read_dir(&gear_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }

            let type_name = entry.file_name().to_string_lossy().to_string();
            // Skip Weapons (handled separately) and internal directories
            if type_name == "Weapons" || type_name.starts_with('_') {
                continue;
            }

            let rel_path = entry
                .path()
                .strip_prefix(extract_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut balance_data = Vec::new();
            let mut manufacturers = Vec::new();

            // Find balance data
            for bd_entry in WalkDir::new(&entry.path())
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().to_string_lossy().contains("BalanceData")
                        && e.path()
                            .extension()
                            .map(|ext| ext == "uasset")
                            .unwrap_or(false)
                })
            {
                let asset_path = bd_entry.path();
                let mut asset_info = AssetInfo {
                    name: asset_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    file: asset_path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path: asset_path
                        .strip_prefix(extract_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .ok(),
                    stats: None,
                    properties: None,
                    raw_strings: None,
                };

                if let Ok(content) = extract_strings(asset_path) {
                    let stats = parse_stat_properties(&content);
                    if !stats.is_empty() {
                        asset_info.stats = Some(stats);
                    }
                }

                balance_data.push(asset_info);
            }

            // Find manufacturers
            let mfr_dir = entry.path().join("Manufacturer");
            if mfr_dir.exists() {
                if let Ok(mfr_entries) = fs::read_dir(&mfr_dir) {
                    for mfr_entry in mfr_entries.flatten() {
                        if mfr_entry.path().is_dir() {
                            let code = mfr_entry.file_name().to_string_lossy().to_string();
                            manufacturers.push(ManufacturerRef {
                                code: code.clone(),
                                name: mfr_names.get(code.as_str()).unwrap_or(&code.as_str()).to_string(),
                                path: mfr_entry
                                    .path()
                                    .strip_prefix(extract_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                }
            }

            gear_types.insert(
                type_name.clone(),
                GearType {
                    name: type_name,
                    path: rel_path,
                    balance_data,
                    manufacturers,
                },
            );
        }
    }

    gear_types
}

/// Extract rarity data
pub fn extract_rarity_data(extract_dir: &Path) -> HashMap<String, AssetInfo> {
    let mut rarity_data: HashMap<String, AssetInfo> = HashMap::new();

    let rarity_paths = [
        extract_dir.join("OakGame/Content/Gear/Weapons/_Shared/BalanceData/Rarity"),
        extract_dir.join("OakGame/Content/Gear/_Shared/BalanceData/Rarity"),
    ];

    for rarity_dir in &rarity_paths {
        if !rarity_dir.exists() {
            continue;
        }

        for entry in WalkDir::new(rarity_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "uasset")
                    .unwrap_or(false)
            })
        {
            let asset_path = entry.path();
            let name = asset_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut raw_strings = None;
            if let Ok(content) = extract_strings(asset_path) {
                let strings: Vec<String> = content
                    .lines()
                    .filter(|s| !s.is_empty() && s.len() < 200)
                    .take(50)
                    .map(String::from)
                    .collect();
                if !strings.is_empty() {
                    raw_strings = Some(strings);
                }
            }

            rarity_data.insert(
                name.clone(),
                AssetInfo {
                    name: name.clone(),
                    file: asset_path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path: asset_path
                        .strip_prefix(extract_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .ok(),
                    stats: None,
                    properties: None,
                    raw_strings,
                },
            );
        }
    }

    rarity_data
}

/// Extract elemental data
pub fn extract_elemental_data(extract_dir: &Path) -> HashMap<String, AssetInfo> {
    let mut elemental_data: HashMap<String, AssetInfo> = HashMap::new();

    let elemental_dir = extract_dir.join("OakGame/Content/Gear/Weapons/_Shared/BalanceData/Elemental");
    if !elemental_dir.exists() {
        return elemental_data;
    }

    for entry in WalkDir::new(&elemental_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "uasset")
                .unwrap_or(false)
        })
    {
        let asset_path = entry.path();
        let name = asset_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut raw_strings = None;
        if let Ok(content) = extract_strings(asset_path) {
            let strings: Vec<String> = content
                .lines()
                .filter(|s| !s.is_empty() && s.len() < 200)
                .take(50)
                .map(String::from)
                .collect();
            if !strings.is_empty() {
                raw_strings = Some(strings);
            }
        }

        elemental_data.insert(
            name.clone(),
            AssetInfo {
                name: name.clone(),
                file: asset_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
                path: asset_path
                    .strip_prefix(extract_dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .ok(),
                stats: None,
                properties: None,
                raw_strings,
            },
        );
    }

    elemental_data
}

// ============================================================================
// Static Reference Data
// ============================================================================

/// Rarity tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RarityTier {
    pub tier: u8,
    pub code: String,
    pub name: String,
    pub color: String,
}

pub fn rarity_tiers() -> Vec<RarityTier> {
    vec![
        RarityTier { tier: 1, code: "comp_01".into(), name: "Common".into(), color: "#FFFFFF".into() },
        RarityTier { tier: 2, code: "comp_02".into(), name: "Uncommon".into(), color: "#00FF00".into() },
        RarityTier { tier: 3, code: "comp_03".into(), name: "Rare".into(), color: "#0080FF".into() },
        RarityTier { tier: 4, code: "comp_04".into(), name: "Epic".into(), color: "#A020F0".into() },
        RarityTier { tier: 5, code: "comp_05".into(), name: "Legendary".into(), color: "#FFA500".into() },
    ]
}

/// Element types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub name: String,
    pub description: String,
    pub color: String,
}

pub fn element_types() -> Vec<ElementType> {
    vec![
        ElementType { code: "kinetic".into(), name: "Impact".into(), description: "Non-elemental kinetic damage".into(), color: "#808080".into() },
        ElementType { code: "fire".into(), name: "Fire".into(), description: "Incendiary damage, effective vs flesh".into(), color: "#FF4500".into() },
        ElementType { code: "shock".into(), name: "Electric".into(), description: "Shock damage, effective vs shields".into(), color: "#00BFFF".into() },
        ElementType { code: "corrosive".into(), name: "Corrosive".into(), description: "Acid damage, effective vs armor".into(), color: "#32CD32".into() },
        ElementType { code: "cryo".into(), name: "Cryo".into(), description: "Freezing damage, slows and can freeze enemies".into(), color: "#ADD8E6".into() },
        ElementType { code: "radiation".into(), name: "Radiation".into(), description: "Radiation damage, spreads to nearby enemies".into(), color: "#FFFF00".into() },
    ]
}

/// Known legendary items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendaryItem {
    pub internal: String,
    pub name: String,
    pub weapon_type: String,
    pub manufacturer: String,
}

pub fn known_legendaries() -> Vec<LegendaryItem> {
    vec![
        LegendaryItem { internal: "DAD_AR.comp_05_legendary_OM".into(), name: "OM".into(), weapon_type: "assaultrifle".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "DAD_AR_Lumberjack".into(), name: "Lumberjack".into(), weapon_type: "assaultrifle".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "DAD_SG.comp_05_legendary_HeartGUn".into(), name: "Heart Gun".into(), weapon_type: "shotgun".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "DAD_PS.Zipper".into(), name: "Zipper".into(), weapon_type: "pistol".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "DAD_PS.Rangefinder".into(), name: "Rangefinder".into(), weapon_type: "pistol".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "DAD_SG.Durendal".into(), name: "Durendal".into(), weapon_type: "shotgun".into(), manufacturer: "DAD".into() },
        LegendaryItem { internal: "JAK_AR.comp_05_legendary_rowan".into(), name: "Rowan's Call".into(), weapon_type: "assaultrifle".into(), manufacturer: "JAK".into() },
        LegendaryItem { internal: "JAK_PS.comp_05_legendary_kingsgambit".into(), name: "King's Gambit".into(), weapon_type: "pistol".into(), manufacturer: "JAK".into() },
        LegendaryItem { internal: "JAK_PS.comp_05_legendary_phantom_flame".into(), name: "Phantom Flame".into(), weapon_type: "pistol".into(), manufacturer: "JAK".into() },
        LegendaryItem { internal: "JAK_SR.comp_05_legendary_ballista".into(), name: "Ballista".into(), weapon_type: "sniper".into(), manufacturer: "JAK".into() },
        LegendaryItem { internal: "MAL_HW.comp_05_legendary_GammaVoid".into(), name: "Gamma Void".into(), weapon_type: "heavy".into(), manufacturer: "MAL".into() },
        LegendaryItem { internal: "MAL_SM.comp_05_legendary_OhmIGot".into(), name: "Ohm I Got".into(), weapon_type: "smg".into(), manufacturer: "MAL".into() },
        LegendaryItem { internal: "BOR_SM.comp_05_legendary_p".into(), name: "Unknown Borg SMG".into(), weapon_type: "smg".into(), manufacturer: "BOR".into() },
        LegendaryItem { internal: "TED_AR.comp_05_legendary_Chuck".into(), name: "Chuck".into(), weapon_type: "assaultrifle".into(), manufacturer: "TED".into() },
        LegendaryItem { internal: "TED_PS.comp_05_legendary_Sideshow".into(), name: "Sideshow".into(), weapon_type: "pistol".into(), manufacturer: "TED".into() },
        LegendaryItem { internal: "TED_SG.comp_05_legendary_a".into(), name: "Unknown Tediore Shotgun".into(), weapon_type: "shotgun".into(), manufacturer: "TED".into() },
        LegendaryItem { internal: "TOR_HW.comp_05_legendary_ravenfire".into(), name: "Ravenfire".into(), weapon_type: "heavy".into(), manufacturer: "TOR".into() },
        LegendaryItem { internal: "TOR_SG.comp_05_legendary_Linebacker".into(), name: "Linebacker".into(), weapon_type: "shotgun".into(), manufacturer: "TOR".into() },
        LegendaryItem { internal: "VLA_AR.comp_05_legendary_WomboCombo".into(), name: "Wombo Combo".into(), weapon_type: "assaultrifle".into(), manufacturer: "VLA".into() },
        LegendaryItem { internal: "VLA_HW.comp_05_legendary_AtlingGun".into(), name: "Atling Gun".into(), weapon_type: "heavy".into(), manufacturer: "VLA".into() },
        LegendaryItem { internal: "VLA_SM.comp_05_legendary_KaoSon".into(), name: "Kaoson".into(), weapon_type: "smg".into(), manufacturer: "VLA".into() },
    ]
}

/// Weapon type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponTypeInfo {
    pub code: String,
    pub name: String,
    pub description: String,
}

pub fn weapon_type_info() -> Vec<WeaponTypeInfo> {
    vec![
        WeaponTypeInfo { code: "AR".into(), name: "Assault Rifle".into(), description: "Full-auto/burst fire rifles".into() },
        WeaponTypeInfo { code: "HW".into(), name: "Heavy Weapon".into(), description: "Launchers and miniguns".into() },
        WeaponTypeInfo { code: "PS".into(), name: "Pistol".into(), description: "Semi-auto and full-auto handguns".into() },
        WeaponTypeInfo { code: "SG".into(), name: "Shotgun".into(), description: "High-damage spread weapons".into() },
        WeaponTypeInfo { code: "SM".into(), name: "SMG".into(), description: "Submachine guns".into() },
        WeaponTypeInfo { code: "SR".into(), name: "Sniper Rifle".into(), description: "Long-range precision weapons".into() },
    ]
}

/// Extended manufacturer info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturerInfo {
    pub code: String,
    pub name: String,
    pub weapon_types: Vec<String>,
    pub style: String,
}

pub fn manufacturer_info() -> Vec<ManufacturerInfo> {
    vec![
        ManufacturerInfo { code: "BOR".into(), name: "Borg".into(), weapon_types: vec!["smg".into(), "shotgun".into(), "heavy".into(), "sniper".into()], style: "Cult/organic aesthetics".into() },
        ManufacturerInfo { code: "DAD".into(), name: "Daedalus".into(), weapon_types: vec!["assaultrifle".into(), "smg".into(), "pistol".into(), "shotgun".into()], style: "High-tech precision".into() },
        ManufacturerInfo { code: "JAK".into(), name: "Jakobs".into(), weapon_types: vec!["assaultrifle".into(), "pistol".into(), "shotgun".into(), "sniper".into()], style: "Old West, semi-auto, high damage per shot".into() },
        ManufacturerInfo { code: "MAL".into(), name: "Maliwan".into(), weapon_types: vec!["smg".into(), "shotgun".into(), "sniper".into(), "heavy".into()], style: "Elemental weapons, energy-based".into() },
        ManufacturerInfo { code: "ORD".into(), name: "Order".into(), weapon_types: vec!["assaultrifle".into(), "pistol".into(), "sniper".into()], style: "Military precision".into() },
        ManufacturerInfo { code: "RIP".into(), name: "Ripper".into(), weapon_types: vec!["shotgun".into(), "sniper".into()], style: "Aggressive, high-damage".into() },
        ManufacturerInfo { code: "TED".into(), name: "Tediore".into(), weapon_types: vec!["assaultrifle".into(), "pistol".into(), "shotgun".into(), "smg".into()], style: "Disposable, thrown on reload".into() },
        ManufacturerInfo { code: "TOR".into(), name: "Torgue".into(), weapon_types: vec!["assaultrifle".into(), "pistol".into(), "shotgun".into(), "heavy".into()], style: "Explosive/gyrojet rounds".into() },
        ManufacturerInfo { code: "VLA".into(), name: "Vladof".into(), weapon_types: vec!["assaultrifle".into(), "smg".into(), "sniper".into(), "heavy".into()], style: "High fire rate, large magazines".into() },
        ManufacturerInfo { code: "GRV".into(), name: "Gravitar".into(), weapon_types: vec![], style: "Class mods manufacturer".into() },
    ]
}

/// Gear type definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearTypeInfo {
    pub code: String,
    pub name: String,
    pub description: String,
}

pub fn gear_type_info() -> Vec<GearTypeInfo> {
    vec![
        GearTypeInfo { code: "shield".into(), name: "Shield".into(), description: "Defensive equipment".into() },
        GearTypeInfo { code: "classmod".into(), name: "Class Mod".into(), description: "Character class modifications".into() },
        GearTypeInfo { code: "enhancement".into(), name: "Enhancement".into(), description: "Permanent character upgrades".into() },
        GearTypeInfo { code: "gadget".into(), name: "Gadget".into(), description: "Deployable equipment".into() },
        GearTypeInfo { code: "repair_kit".into(), name: "Repair Kit".into(), description: "Healing items".into() },
        GearTypeInfo { code: "grenade".into(), name: "Grenade".into(), description: "Throwable explosive devices".into() },
    ]
}

// ============================================================================
// Consolidated Manifest Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsolidatedManifest {
    pub version: String,
    pub game: String,
    pub description: String,
    pub manufacturers: Vec<ManufacturerInfo>,
    pub weapon_types: Vec<WeaponTypeInfo>,
    pub gear_types: Vec<GearTypeInfo>,
    pub rarities: Vec<RarityTier>,
    pub elements: Vec<ElementType>,
    pub stats: HashMap<String, String>,
    pub legendaries: Vec<LegendaryItem>,
}

/// Generate consolidated reference manifest with all static data
pub fn generate_reference_manifest(output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    println!("Generating consolidated reference manifest...");

    // Stats
    let stats: HashMap<String, String> = stat_descriptions()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let manifest = ConsolidatedManifest {
        version: "1.0".to_string(),
        game: "Borderlands 4".to_string(),
        description: "Comprehensive item manifest for BL4 save editing".to_string(),
        manufacturers: manufacturer_info(),
        weapon_types: weapon_type_info(),
        gear_types: gear_type_info(),
        rarities: rarity_tiers(),
        elements: element_types(),
        stats,
        legendaries: known_legendaries(),
    };

    // Write individual files
    let mfr_path = output_dir.join("manufacturers.json");
    fs::write(&mfr_path, serde_json::to_string_pretty(&manifest.manufacturers)?)?;
    println!("  manufacturers.json - {} entries", manifest.manufacturers.len());

    let wt_path = output_dir.join("weapon_types.json");
    fs::write(&wt_path, serde_json::to_string_pretty(&manifest.weapon_types)?)?;
    println!("  weapon_types.json - {} entries", manifest.weapon_types.len());

    let gt_path = output_dir.join("gear_types.json");
    fs::write(&gt_path, serde_json::to_string_pretty(&manifest.gear_types)?)?;
    println!("  gear_types.json - {} entries", manifest.gear_types.len());

    let rarity_path = output_dir.join("rarities.json");
    fs::write(&rarity_path, serde_json::to_string_pretty(&manifest.rarities)?)?;
    println!("  rarities.json - {} entries", manifest.rarities.len());

    let elem_path = output_dir.join("elements.json");
    fs::write(&elem_path, serde_json::to_string_pretty(&manifest.elements)?)?;
    println!("  elements.json - {} entries", manifest.elements.len());

    let stats_path = output_dir.join("stats.json");
    fs::write(&stats_path, serde_json::to_string_pretty(&manifest.stats)?)?;
    println!("  stats.json - {} entries", manifest.stats.len());

    let leg_path = output_dir.join("legendaries.json");
    fs::write(&leg_path, serde_json::to_string_pretty(&manifest.legendaries)?)?;
    println!("  legendaries.json - {} entries", manifest.legendaries.len());

    // Write consolidated manifest
    let consolidated_path = output_dir.join("reference.json");
    fs::write(&consolidated_path, serde_json::to_string_pretty(&manifest)?)?;
    println!("  reference.json - consolidated reference data");

    // Write index
    let mut files = HashMap::new();
    files.insert("manufacturers".to_string(), "manufacturers.json".to_string());
    files.insert("weapon_types".to_string(), "weapon_types.json".to_string());
    files.insert("gear_types".to_string(), "gear_types.json".to_string());
    files.insert("rarities".to_string(), "rarities.json".to_string());
    files.insert("elements".to_string(), "elements.json".to_string());
    files.insert("stats".to_string(), "stats.json".to_string());
    files.insert("legendaries".to_string(), "legendaries.json".to_string());
    files.insert("reference".to_string(), "reference.json".to_string());

    let index = ManifestIndex {
        version: "1.0".to_string(),
        source: "BL4 Game Data".to_string(),
        extract_path: output_dir.to_string_lossy().to_string(),
        files,
    };

    let index_path = output_dir.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
    println!("  index.json");

    println!("\nReference manifest saved to {:?}", output_dir);
    Ok(())
}

// ============================================================================
// Original Extract Functions
// ============================================================================

// ============================================================================
// PAK-Based Manifest Generation
// ============================================================================

/// Property value from uextract JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UextractProperty {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub float_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub int_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
}

/// Export metadata from uextract JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UextractExport {
    pub index: usize,
    pub object_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub super_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_export_hash: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooked_serial_offset: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooked_serial_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<UextractProperty>>,
}

/// Asset metadata from uextract JSON output
#[derive(Debug, Serialize, Deserialize)]
pub struct UextractAsset {
    pub path: String,
    pub package_name: String,
    pub package_flags: u32,
    pub is_unversioned: bool,
    pub name_count: usize,
    pub import_count: usize,
    pub export_count: usize,
    pub names: Vec<String>,
    pub imports: Vec<serde_json::Value>,
    pub exports: Vec<UextractExport>,
}

/// Stat value with name and value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatValue {
    pub name: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier_type: Option<String>,  // Scale, Add, Value, Percent
}

/// Parsed weapon/gear item from extracted data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedItem {
    pub path: String,
    pub asset_name: String,
    pub category: String,
    pub weapon_type: Option<String>,
    pub manufacturer: Option<String>,
    pub unique_id: Option<String>,
    pub property_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Vec<StatValue>>,
}

/// Manifest built from pak file extraction
#[derive(Debug, Serialize, Deserialize)]
pub struct PakManifest {
    pub version: String,
    pub source: String,
    pub description: String,
    pub extracted_at: String,
    pub total_assets: usize,
    pub manufacturers: Vec<String>,
    pub weapon_types: HashMap<String, Vec<String>>, // type -> manufacturers
    pub gear_types: Vec<String>,
    pub items: Vec<ExtractedItem>,
    pub balance_data: HashMap<String, Vec<String>>, // category -> asset names
    pub naming_strategies: Vec<String>,
    pub stats: HashMap<String, Vec<String>>, // stat name -> GUIDs
}

/// Parse a uextract JSON file
fn parse_uextract_json(json_path: &Path) -> Result<UextractAsset> {
    let content = fs::read_to_string(json_path)?;
    let asset: UextractAsset = serde_json::from_str(&content)?;
    Ok(asset)
}

/// Extract stats/properties from asset names
fn extract_stats_from_names(names: &[String]) -> HashMap<String, String> {
    let stat_pattern = Regex::new(r"^([A-Za-z_]+)_(\d+)_([A-F0-9]{32})$").unwrap();
    let mut stats = HashMap::new();

    for name in names {
        if let Some(caps) = stat_pattern.captures(name) {
            let stat_name = caps.get(1).unwrap().as_str().to_string();
            let guid = caps.get(3).unwrap().as_str().to_string();
            stats.insert(stat_name, guid);
        }
    }

    stats
}

/// Generate manifest from uextract output directory
pub fn generate_pak_manifest(extracted_dir: &Path, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    println!("Building manifest from pak extraction at {:?}", extracted_dir);

    let mfr_names = manufacturer_names();
    let mut manufacturers: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut weapon_types: HashMap<String, Vec<String>> = HashMap::new();
    let mut gear_types: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut items: Vec<ExtractedItem> = Vec::new();
    let mut balance_data: HashMap<String, Vec<String>> = HashMap::new();
    let mut naming_strategies: Vec<String> = Vec::new();
    let mut all_stats: HashMap<String, Vec<String>> = HashMap::new();
    let mut total_assets = 0;

    // Walk through all JSON files in the extracted directory
    for entry in WalkDir::new(extracted_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
    {
        let json_path = entry.path();

        // Parse the JSON
        let asset = match parse_uextract_json(json_path) {
            Ok(a) => a,
            Err(_) => continue,
        };

        total_assets += 1;

        // Determine category from path
        let path_str = asset.path.to_lowercase();
        let _package_name = &asset.package_name;

        // Extract manufacturer and weapon type from path
        let mut manufacturer: Option<String> = None;
        let mut weapon_type: Option<String> = None;
        let mut category = "unknown".to_string();

        // Check for weapon types
        if path_str.contains("gear/weapons") {
            category = "weapon".to_string();

            // Extract weapon type
            if path_str.contains("assaultrifles") { weapon_type = Some("AssaultRifle".to_string()); }
            else if path_str.contains("pistols") { weapon_type = Some("Pistol".to_string()); }
            else if path_str.contains("shotguns") { weapon_type = Some("Shotgun".to_string()); }
            else if path_str.contains("smg") { weapon_type = Some("SMG".to_string()); }
            else if path_str.contains("sniper") { weapon_type = Some("Sniper".to_string()); }
            else if path_str.contains("heavy") { weapon_type = Some("Heavy".to_string()); }

            // Extract manufacturer
            for (code, _) in &mfr_names {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("/{}/", code_lower)) ||
                   path_str.contains(&format!("/{}_", code_lower)) {
                    manufacturer = Some(code.to_string());
                    manufacturers.insert(code.to_string());

                    if let Some(ref wt) = weapon_type {
                        weapon_types
                            .entry(wt.clone())
                            .or_default()
                            .push(code.to_string());
                    }
                    break;
                }
            }
        } else if path_str.contains("gear/gadgets/heavyweapons") {
            // Heavy weapons are under Gadgets but are actually weapons
            category = "weapon".to_string();
            weapon_type = Some("Heavy".to_string());

            // Extract manufacturer for heavy weapons
            for (code, _) in &mfr_names {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("/{}/", code_lower)) ||
                   path_str.contains(&format!("/{}_", code_lower)) {
                    manufacturer = Some(code.to_string());
                    manufacturers.insert(code.to_string());
                    weapon_types
                        .entry("Heavy".to_string())
                        .or_default()
                        .push(code.to_string());
                    break;
                }
            }
        } else if path_str.contains("gear/classmods") {
            category = "classmod".to_string();
            gear_types.insert("ClassMod".to_string());

            // Extract class type from path
            if path_str.contains("gravitar") { manufacturer = Some("GRV".to_string()); }
            else if path_str.contains("paladin") { manufacturer = Some("PLD".to_string()); }
            else if path_str.contains("darksiren") || path_str.contains("dark_siren") { manufacturer = Some("SIR".to_string()); }
            else if path_str.contains("exo") { manufacturer = Some("EXO".to_string()); }
        } else if path_str.contains("gear/enhancements") {
            category = "enhancement".to_string();
            gear_types.insert("Enhancement".to_string());

            // Extract manufacturer from enhancement name
            for (code, _) in &mfr_names {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("_{}_", code_lower)) ||
                   path_str.contains(&format!("/{}/", code_lower)) {
                    manufacturer = Some(code.to_string());
                    break;
                }
            }
        } else if path_str.contains("gear/shields") {
            category = "shield".to_string();
            gear_types.insert("Shield".to_string());
        } else if path_str.contains("gear/grenadegadgets") {
            category = "grenade".to_string();
            gear_types.insert("Grenade".to_string());
        } else if path_str.contains("gear/gadgets") {
            category = "gadget".to_string();
            gear_types.insert("Gadget".to_string());
        } else if path_str.contains("gear/firmware") {
            category = "firmware".to_string();
            gear_types.insert("Firmware".to_string());
        } else if path_str.contains("gear/repairkits") {
            category = "repair_kit".to_string();
            gear_types.insert("RepairKit".to_string());
        }

        // Track balance data
        if path_str.contains("balancedata") {
            let bd_category = if let Some(ref wt) = weapon_type {
                wt.clone()
            } else {
                category.clone()
            };
            balance_data
                .entry(bd_category)
                .or_default()
                .push(asset.package_name.clone());
        }

        // Track naming strategies
        if path_str.contains("namingstrateg") {
            naming_strategies.push(asset.package_name.clone());
        }

        // Extract stats from names
        let stats = extract_stats_from_names(&asset.names);
        for (stat_name, guid) in stats {
            all_stats
                .entry(stat_name)
                .or_default()
                .push(guid);
        }

        // Build item entry
        let asset_name = json_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".uasset"))
            .unwrap_or("")
            .to_string();

        // Extract unique ID from asset names
        let unique_id = asset.names.iter()
            .find(|n| n.contains("comp_05") || n.contains("Unique") || n.contains("legendary"))
            .cloned();

        // Extract stat values from exports
        let mut stat_values: Vec<StatValue> = Vec::new();
        for export in &asset.exports {
            if let Some(ref props) = export.properties {
                for prop in props {
                    if let Some(val) = prop.float_value {
                        // Parse modifier type from property name (e.g., "Damage_Scale" -> Scale)
                        let parts: Vec<&str> = prop.name.split('_').collect();
                        let modifier_type = if parts.len() >= 2 {
                            let last = parts[parts.len() - 1];
                            if ["Scale", "Add", "Value", "Percent"].contains(&last) {
                                Some(last.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        stat_values.push(StatValue {
                            name: prop.name.clone(),
                            value: val,
                            modifier_type,
                        });
                    }
                }
            }
        }

        items.push(ExtractedItem {
            path: asset.path.clone(),
            asset_name,
            category,
            weapon_type,
            manufacturer,
            unique_id,
            property_names: asset.names.clone(),
            stats: if stat_values.is_empty() { None } else { Some(stat_values) },
        });
    }

    // Deduplicate manufacturer lists in weapon_types
    for manufacturers_list in weapon_types.values_mut() {
        manufacturers_list.sort();
        manufacturers_list.dedup();
    }

    // Deduplicate stats
    for guids in all_stats.values_mut() {
        guids.sort();
        guids.dedup();
    }

    let manifest = PakManifest {
        version: "1.0".to_string(),
        source: "BL4 Pak Files (uextract)".to_string(),
        description: "Manifest generated from BL4 pak file extraction".to_string(),
        extracted_at: chrono::Utc::now().to_rfc3339(),
        total_assets,
        manufacturers: manufacturers.into_iter().collect(),
        weapon_types,
        gear_types: gear_types.into_iter().collect(),
        items,
        balance_data,
        naming_strategies,
        stats: all_stats,
    };

    // Write manifest
    let manifest_path = output_dir.join("pak_manifest.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    println!("  pak_manifest.json - {} assets indexed", manifest.total_assets);

    // Write summary
    let summary = serde_json::json!({
        "version": manifest.version,
        "source": manifest.source,
        "total_assets": manifest.total_assets,
        "manufacturers": manifest.manufacturers,
        "weapon_types": manifest.weapon_types.keys().collect::<Vec<_>>(),
        "gear_types": manifest.gear_types,
        "balance_data_categories": manifest.balance_data.keys().collect::<Vec<_>>(),
        "naming_strategies_count": manifest.naming_strategies.len(),
        "stats_count": manifest.stats.len(),
    });

    let summary_path = output_dir.join("pak_summary.json");
    fs::write(&summary_path, serde_json::to_string_pretty(&summary)?)?;
    println!("  pak_summary.json");

    // Write weapon types breakdown
    let weapons_breakdown: HashMap<String, serde_json::Value> = manifest.weapon_types.iter()
        .map(|(wt, mfrs)| {
            (wt.clone(), serde_json::json!({
                "manufacturers": mfrs,
                "count": manifest.items.iter()
                    .filter(|i| i.weapon_type.as_ref() == Some(wt))
                    .count()
            }))
        })
        .collect();

    let weapons_path = output_dir.join("weapons_breakdown.json");
    fs::write(&weapons_path, serde_json::to_string_pretty(&weapons_breakdown)?)?;
    println!("  weapons_breakdown.json");

    // Update index
    let mut files = HashMap::new();
    files.insert("pak_manifest".to_string(), "pak_manifest.json".to_string());
    files.insert("pak_summary".to_string(), "pak_summary.json".to_string());
    files.insert("weapons_breakdown".to_string(), "weapons_breakdown.json".to_string());

    let index = ManifestIndex {
        version: "1.0".to_string(),
        source: "BL4 Pak Files".to_string(),
        extract_path: extracted_dir.to_string_lossy().to_string(),
        files,
    };

    let index_path = output_dir.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
    println!("  index.json");

    println!("\nManifest generated from {} pak assets", manifest.total_assets);
    println!("  Manufacturers: {:?}", manifest.manufacturers);
    println!("  Weapon types: {:?}", manifest.weapon_types.keys().collect::<Vec<_>>());
    println!("  Gear types: {:?}", manifest.gear_types);

    Ok(())
}

/// Extract all manifest data and save to output directory
pub fn extract_manifest(extract_dir: &Path, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    println!("Extracting manifest from {:?}", extract_dir);
    println!("Output directory: {:?}", output_dir);

    // Manufacturers
    print!("Extracting manufacturers...");
    let manufacturers = extract_manufacturers(extract_dir);
    let mfr_path = output_dir.join("manufacturers.json");
    fs::write(&mfr_path, serde_json::to_string_pretty(&manufacturers)?)?;
    println!(" {} entries", manufacturers.len());

    // Weapon types
    print!("Extracting weapon types...");
    let weapon_types = extract_weapon_types(extract_dir);
    let wt_path = output_dir.join("weapon_types.json");
    fs::write(&wt_path, serde_json::to_string_pretty(&weapon_types)?)?;
    println!(" {} entries", weapon_types.len());

    // Balance data
    print!("Extracting balance data...");
    let balance_data = extract_balance_data(extract_dir)?;
    let bd_path = output_dir.join("balance_data.json");
    fs::write(&bd_path, serde_json::to_string_pretty(&balance_data)?)?;
    println!(" {} categories", balance_data.len());

    // Naming data
    print!("Extracting naming data...");
    let naming_data = extract_naming_data(extract_dir)?;
    let nd_path = output_dir.join("naming.json");
    fs::write(&nd_path, serde_json::to_string_pretty(&naming_data)?)?;
    println!(" {} entries", naming_data.len());

    // Gear types
    print!("Extracting gear types...");
    let gear_types = extract_gear_types(extract_dir);
    let gt_path = output_dir.join("gear_types.json");
    fs::write(&gt_path, serde_json::to_string_pretty(&gear_types)?)?;
    println!(" {} types", gear_types.len());

    // Rarity data
    print!("Extracting rarity data...");
    let rarity_data = extract_rarity_data(extract_dir);
    let rd_path = output_dir.join("rarity.json");
    fs::write(&rd_path, serde_json::to_string_pretty(&rarity_data)?)?;
    println!(" {} entries", rarity_data.len());

    // Elemental data
    print!("Extracting elemental data...");
    let elemental_data = extract_elemental_data(extract_dir);
    let ed_path = output_dir.join("elemental.json");
    fs::write(&ed_path, serde_json::to_string_pretty(&elemental_data)?)?;
    println!(" {} entries", elemental_data.len());

    // Save manifest index
    let mut files = HashMap::new();
    files.insert("manufacturers".to_string(), "manufacturers.json".to_string());
    files.insert("weapon_types".to_string(), "weapon_types.json".to_string());
    files.insert("balance_data".to_string(), "balance_data.json".to_string());
    files.insert("naming".to_string(), "naming.json".to_string());
    files.insert("gear_types".to_string(), "gear_types.json".to_string());
    files.insert("rarity".to_string(), "rarity.json".to_string());
    files.insert("elemental".to_string(), "elemental.json".to_string());

    let index = ManifestIndex {
        version: "1.0".to_string(),
        source: "BL4 Game Files".to_string(),
        extract_path: extract_dir.to_string_lossy().to_string(),
        files,
    };

    let index_path = output_dir.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;

    println!("\nManifest saved to {:?}", output_dir);

    Ok(())
}

/// Extract item pools from pak_manifest.json
pub fn extract_item_pools(manifest_dir: &Path) -> Result<HashMap<String, ItemPool>> {
    let pak_manifest_path = manifest_dir.join("pak_manifest.json");
    if !pak_manifest_path.exists() {
        anyhow::bail!("pak_manifest.json not found in {:?}", manifest_dir);
    }

    let content = fs::read_to_string(&pak_manifest_path)?;
    let manifest: PakManifest = serde_json::from_str(&content)?;

    let mut pools: HashMap<String, ItemPool> = HashMap::new();

    // Pattern for ItemPool references
    let pool_pattern = Regex::new(r"(?:CItemPoolDef::)?[Ii]tem[Pp]ool[_A-Za-z0-9]*").unwrap();

    for item in &manifest.items {
        let asset_path = &item.path;
        let asset_name = &item.asset_name;

        for prop_str in &item.property_names {
            for cap in pool_pattern.find_iter(prop_str) {
                let pool_name = cap.as_str()
                    .trim_start_matches("CItemPoolDef::")
                    .to_string();

                let pool = pools.entry(pool_name.clone()).or_insert_with(|| ItemPool {
                    name: pool_name.clone(),
                    path: None,
                    referenced_by: Vec::new(),
                    contains: Vec::new(),
                });

                // Add this asset as a reference
                if !pool.referenced_by.contains(asset_name) {
                    pool.referenced_by.push(asset_name.clone());
                }

                // If this is the pool definition itself, set the path
                if asset_name.to_lowercase().contains(&pool_name.to_lowercase()) {
                    pool.path = Some(asset_path.clone());
                }
            }
        }
    }

    Ok(pools)
}

/// Extract item stats from pak_manifest.json (comprehensive extraction)
pub fn extract_item_stats(manifest_dir: &Path) -> Result<Vec<ItemStats>> {
    let pak_manifest_path = manifest_dir.join("pak_manifest.json");
    if !pak_manifest_path.exists() {
        anyhow::bail!("pak_manifest.json not found in {:?}", manifest_dir);
    }

    let content = fs::read_to_string(&pak_manifest_path)?;
    let manifest: PakManifest = serde_json::from_str(&content)?;

    let mut items: Vec<ItemStats> = Vec::new();

    // Pattern for stat modifiers: StatName_ModifierType_Index_GUID
    let stat_pattern = Regex::new(r"^([A-Za-z]+[A-Za-z0-9]*)_(Scale|Add|Value|Percent)_(\d+)_([A-F0-9]{32})$").unwrap();

    // Pattern for rarity detection
    let rarity_pattern = Regex::new(r"comp_0([1-5])").unwrap();
    let rarities = ["Common", "Uncommon", "Rare", "Epic", "Legendary"];

    for item in &manifest.items {
        // Only process items that are gear-related
        if item.category == "unknown" && !item.path.to_lowercase().contains("gear") {
            continue;
        }

        let mut stats: HashMap<String, Vec<StatModifier>> = HashMap::new();
        let mut rarity: Option<String> = None;

        for prop in &item.property_names {
            // Check for stat modifiers
            if let Some(cap) = stat_pattern.captures(prop) {
                let stat_name = cap[1].to_string();
                let modifier_type = cap[2].to_string();
                let index: u32 = cap[3].parse().unwrap_or(0);
                let guid = cap[4].to_string();

                let key = format!("{}_{}", stat_name, modifier_type);
                stats.entry(key).or_default().push(StatModifier {
                    modifier_type,
                    index,
                    guid,
                });
            }

            // Check for rarity
            if rarity.is_none() {
                if let Some(cap) = rarity_pattern.captures(prop) {
                    let tier: usize = cap[1].parse().unwrap_or(1);
                    if tier >= 1 && tier <= 5 {
                        rarity = Some(rarities[tier - 1].to_string());
                    }
                }
            }
        }

        // Extract manufacturer from path
        let manufacturer = {
            let path_lower = item.path.to_lowercase();
            let codes = ["BOR", "DAD", "DPL", "JAK", "MAL", "ORD", "RIP", "TED", "TOR", "VLA", "COV"];
            let mut found = None;
            for code in codes {
                let code_lower = code.to_lowercase();
                if path_lower.contains(&format!("/{}/", code_lower))
                    || path_lower.contains(&format!("/{}_", code_lower))
                    || path_lower.contains(&format!("_{}_", code_lower))
                {
                    found = Some(code.to_string());
                    break;
                }
            }
            found
        };

        // Add item if it has stats
        if !stats.is_empty() {
            items.push(ItemStats {
                name: item.asset_name.clone(),
                category: item.category.clone(),
                manufacturer,
                rarity,
                stats,
                drop_pools: Vec::new(), // Linked later
            });
        }
    }

    Ok(items)
}

/// Generate complete items database
pub fn generate_items_database(manifest_dir: &Path) -> Result<ItemsDatabase> {
    eprintln!("Extracting item pools...");
    let item_pools = extract_item_pools(manifest_dir)?;
    eprintln!("  Found {} unique pools", item_pools.len());

    eprintln!("Extracting item stats...");
    let items = extract_item_stats(manifest_dir)?;
    eprintln!("  Found {} items with stats", items.len());

    // Collect summary stats
    let mut stat_types: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut categories: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut manufacturers: std::collections::HashSet<String> = std::collections::HashSet::new();

    for item in &items {
        categories.insert(item.category.clone());
        if let Some(ref mfr) = item.manufacturer {
            manufacturers.insert(mfr.clone());
        }
        for key in item.stats.keys() {
            // Extract stat name from key (StatName_ModifierType)
            if let Some(stat_name) = key.split('_').next() {
                stat_types.insert(stat_name.to_string());
            }
        }
    }

    let stats_summary = StatsSummary {
        total_items: items.len(),
        total_pools: item_pools.len(),
        stat_types: stat_types.into_iter().collect(),
        categories: categories.into_iter().collect(),
        manufacturers: manufacturers.into_iter().collect(),
    };

    Ok(ItemsDatabase {
        version: "1.0".to_string(),
        generated: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        item_pools,
        items,
        stats_summary,
    })
}
