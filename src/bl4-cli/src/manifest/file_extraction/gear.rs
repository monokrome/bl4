//! Gear type, rarity, and elemental data extraction from game directories

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use super::super::property_parsing::{extract_strings, parse_stat_properties, AssetInfo};
use super::types::{manufacturer_names, GearType, ManufacturerRef};

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
