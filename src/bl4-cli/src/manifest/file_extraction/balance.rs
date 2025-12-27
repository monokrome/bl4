//! Balance and naming data extraction from game directories

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use super::super::property_parsing::{
    extract_strings, parse_property_strings, parse_stat_properties, AssetInfo,
};
use super::types::BalanceCategory;

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
