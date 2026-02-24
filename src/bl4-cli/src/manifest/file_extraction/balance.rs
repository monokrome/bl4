//! Naming data extraction from game directories

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

use super::super::property_parsing::{extract_strings, parse_property_strings, AssetInfo};

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
