//! Manufacturer and weapon type extraction from game directories

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::types::{manufacturer_names, Manufacturer, ManufacturerRef, WeaponType};

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
