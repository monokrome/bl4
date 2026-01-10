//! Property and string parsing utilities for game asset files
//!
//! Provides utilities to extract and parse property names, GUIDs, and stat modifiers
//! from uasset files using pattern matching on strings output.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Entry for a property with index and GUID reference
#[derive(Debug, Serialize, Deserialize)]
pub struct PropertyEntry {
    pub index: u32,
    pub guid: String,
}

/// Entry for a stat with index and GUID reference
#[derive(Debug, Serialize, Deserialize)]
pub struct StatEntry {
    pub index: u32,
    pub guid: String,
}

/// Stat property with modifier type and entries
#[derive(Debug, Serialize, Deserialize)]
pub struct StatProperty {
    pub stat: String,
    #[serde(rename = "type")]
    pub modifier_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entries: Vec<StatEntry>,
}

/// Asset information with parsed properties and stats
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

impl AssetInfo {
    /// Create an AssetInfo from an asset path, optionally relative to an extract directory
    pub fn from_path(asset_path: &Path, extract_dir: Option<&Path>) -> Self {
        Self {
            name: asset_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            file: asset_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: extract_dir.and_then(|dir| {
                asset_path
                    .strip_prefix(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .ok()
            }),
            stats: None,
            properties: None,
            raw_strings: None,
        }
    }

    /// Create from a name and path string (for simpler cases)
    pub fn new(name: String, file: String) -> Self {
        Self {
            name,
            file,
            path: None,
            stats: None,
            properties: None,
            raw_strings: None,
        }
    }
}

/// Get stat descriptions from bl4::reference
#[deprecated(note = "Use bl4::reference::all_stat_descriptions() directly")]
pub fn stat_descriptions() -> HashMap<&'static str, &'static str> {
    bl4::reference::all_stat_descriptions()
}

/// Extract readable strings from a uasset file using the `strings` command
pub fn extract_strings(uasset_path: &Path) -> Result<String> {
    let output = Command::new("strings")
        .arg(uasset_path)
        .output()
        .context("Failed to run strings command")?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Extract a filtered sample of raw strings from content
/// Filters out empty strings and very long lines, takes first 50
pub fn extract_raw_string_sample(content: &str) -> Option<Vec<String>> {
    let strings: Vec<String> = content
        .lines()
        .filter(|s| !s.is_empty() && s.len() < 200)
        .take(50)
        .map(String::from)
        .collect();
    if strings.is_empty() {
        None
    } else {
        Some(strings)
    }
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
#[allow(deprecated)]
pub fn parse_stat_properties(content: &str) -> HashMap<String, StatProperty> {
    let pattern =
        Regex::new(r"([A-Za-z_]+)_(Scale|Add|Value|Percent)_(\d+)_([A-F0-9]{32})").unwrap();
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
