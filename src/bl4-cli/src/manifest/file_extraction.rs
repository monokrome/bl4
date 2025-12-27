//! File-based extraction from unpacked game directories
//!
//! Walks extracted game directories to find and catalog manufacturers,
//! weapon types, balance data, naming strategies, gear types, rarity data,
//! and elemental data from .uasset files.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use super::property_parsing::{
    extract_strings, parse_property_strings, parse_stat_properties, AssetInfo,
};

/// Get manufacturer names from bl4::reference
fn manufacturer_names() -> HashMap<&'static str, &'static str> {
    bl4::reference::MANUFACTURERS
        .iter()
        .map(|m| (m.code, m.name))
        .collect()
}

/// Manufacturer found during directory walking (distinct from ExtractedManufacturer)
#[derive(Debug, Serialize, Deserialize)]
pub struct Manufacturer {
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance_data_path: Option<String>,
}

/// Weapon type with associated manufacturers
#[derive(Debug, Serialize, Deserialize)]
pub struct WeaponType {
    pub name: String,
    pub path: String,
    pub manufacturers: Vec<ManufacturerRef>,
}

/// Reference to a manufacturer within a weapon/gear type
#[derive(Debug, Serialize, Deserialize)]
pub struct ManufacturerRef {
    pub code: String,
    pub name: String,
    pub path: String,
}

/// Category of balance data assets
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceCategory {
    pub name: String,
    pub path: String,
    pub assets: Vec<AssetInfo>,
}

/// Gear type (shields, grenades, gadgets, etc.) with associated data
#[derive(Debug, Serialize, Deserialize)]
pub struct GearType {
    pub name: String,
    pub path: String,
    pub balance_data: Vec<AssetInfo>,
    pub manufacturers: Vec<ManufacturerRef>,
}

/// Extract manufacturers from game files by walking directory structure
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
                        if let Ok(mfr_entries) = fs::read_dir(entry.path()) {
                            for mfr_entry in mfr_entries.flatten() {
                                if mfr_entry.path().is_dir() {
                                    let code = mfr_entry.file_name().to_string_lossy().to_string();
                                    if mfr_names.contains_key(code.as_str()) {
                                        manufacturers.entry(code.clone()).or_insert_with(|| {
                                            Manufacturer {
                                                code: code.clone(),
                                                name: mfr_names
                                                    .get(code.as_str())
                                                    .unwrap_or(&code.as_str())
                                                    .to_string(),
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
                                name: mfr_names
                                    .get(code.as_str())
                                    .unwrap_or(&code.as_str())
                                    .to_string(),
                                path: if is_balance_data {
                                    None
                                } else {
                                    rel_path.clone()
                                },
                                balance_data_path: if is_balance_data { rel_path } else { None },
                            });
                    }
                }
            }
        }
    }

    manufacturers
}

/// Extract weapon type data by walking the weapons directory
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
                if let Ok(mfr_entries) = fs::read_dir(entry.path()) {
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
                                name: mfr_names
                                    .get(code.as_str())
                                    .unwrap_or(&code.as_str())
                                    .to_string(),
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
                for asset_entry in WalkDir::new(entry.path())
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

/// Extract naming data from naming strategies directory
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
            for bd_entry in WalkDir::new(entry.path())
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
                                name: mfr_names
                                    .get(code.as_str())
                                    .unwrap_or(&code.as_str())
                                    .to_string(),
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

/// Extract rarity data from rarity directories
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

/// Extract elemental data from elemental directory
pub fn extract_elemental_data(extract_dir: &Path) -> HashMap<String, AssetInfo> {
    let mut elemental_data: HashMap<String, AssetInfo> = HashMap::new();

    let elemental_dir =
        extract_dir.join("OakGame/Content/Gear/Weapons/_Shared/BalanceData/Elemental");
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
