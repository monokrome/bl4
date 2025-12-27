//! PAK manifest generation from uextract output
//!
//! Processes JSON files produced by uextract to build a comprehensive manifest
//! of game assets including weapons, gear, manufacturers, and stats.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use super::ManifestIndex;

/// Get manufacturer names from bl4::reference
/// DEPRECATED: Use `extract_manufacturer_names_from_pak` for authoritative data.
pub fn manufacturer_names() -> HashMap<&'static str, &'static str> {
    bl4::reference::MANUFACTURERS
        .iter()
        .map(|m| (m.code, m.name))
        .collect()
}

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
    pub modifier_type: Option<String>, // Scale, Add, Value, Percent
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
pub fn parse_uextract_json(json_path: &Path) -> Result<UextractAsset> {
    let content = fs::read_to_string(json_path)?;
    let asset: UextractAsset = serde_json::from_str(&content)?;
    Ok(asset)
}

/// Extract stats/properties from asset names
pub fn extract_stats_from_names(names: &[String]) -> HashMap<String, String> {
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

    println!(
        "Building manifest from pak extraction at {:?}",
        extracted_dir
    );

    let mfr_names = manufacturer_names();
    let mut manufacturers: HashSet<String> = HashSet::new();
    let mut weapon_types: HashMap<String, Vec<String>> = HashMap::new();
    let mut gear_types: HashSet<String> = HashSet::new();
    let mut items: Vec<ExtractedItem> = Vec::new();
    let mut balance_data: HashMap<String, Vec<String>> = HashMap::new();
    let mut naming_strategies: Vec<String> = Vec::new();
    let mut all_stats: HashMap<String, Vec<String>> = HashMap::new();
    let mut total_assets = 0;

    // Walk through all JSON files in the extracted directory
    for entry in WalkDir::new(extracted_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
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
            if path_str.contains("assaultrifles") {
                weapon_type = Some("AssaultRifle".to_string());
            } else if path_str.contains("pistols") {
                weapon_type = Some("Pistol".to_string());
            } else if path_str.contains("shotguns") {
                weapon_type = Some("Shotgun".to_string());
            } else if path_str.contains("smg") {
                weapon_type = Some("SMG".to_string());
            } else if path_str.contains("sniper") {
                weapon_type = Some("Sniper".to_string());
            } else if path_str.contains("heavy") {
                weapon_type = Some("Heavy".to_string());
            }

            // Extract manufacturer
            for code in mfr_names.keys() {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("/{}/", code_lower))
                    || path_str.contains(&format!("/{}_", code_lower))
                {
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
            for code in mfr_names.keys() {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("/{}/", code_lower))
                    || path_str.contains(&format!("/{}_", code_lower))
                {
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
            if path_str.contains("gravitar") {
                manufacturer = Some("GRV".to_string());
            } else if path_str.contains("paladin") {
                manufacturer = Some("PLD".to_string());
            } else if path_str.contains("darksiren") || path_str.contains("dark_siren") {
                manufacturer = Some("SIR".to_string());
            } else if path_str.contains("exo") {
                manufacturer = Some("EXO".to_string());
            }
        } else if path_str.contains("gear/enhancements") {
            category = "enhancement".to_string();
            gear_types.insert("Enhancement".to_string());

            // Extract manufacturer from enhancement name
            for code in mfr_names.keys() {
                let code_lower = code.to_lowercase();
                if path_str.contains(&format!("_{}_", code_lower))
                    || path_str.contains(&format!("/{}/", code_lower))
                {
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
            all_stats.entry(stat_name).or_default().push(guid);
        }

        // Build item entry
        let asset_name = json_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".uasset"))
            .unwrap_or("")
            .to_string();

        // Extract unique ID from asset names
        let unique_id = asset
            .names
            .iter()
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
            stats: if stat_values.is_empty() {
                None
            } else {
                Some(stat_values)
            },
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
        version: env!("CARGO_PKG_VERSION").to_string(),
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
    println!(
        "  pak_manifest.json - {} assets indexed",
        manifest.total_assets
    );

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
    let weapons_breakdown: HashMap<String, serde_json::Value> = manifest
        .weapon_types
        .iter()
        .map(|(wt, mfrs)| {
            (
                wt.clone(),
                serde_json::json!({
                    "manufacturers": mfrs,
                    "count": manifest.items.iter()
                        .filter(|i| i.weapon_type.as_ref() == Some(wt))
                        .count()
                }),
            )
        })
        .collect();

    let weapons_path = output_dir.join("weapons_breakdown.json");
    fs::write(
        &weapons_path,
        serde_json::to_string_pretty(&weapons_breakdown)?,
    )?;
    println!("  weapons_breakdown.json");

    // Update index
    let mut files = HashMap::new();
    files.insert("pak_manifest".to_string(), "pak_manifest.json".to_string());
    files.insert("pak_summary".to_string(), "pak_summary.json".to_string());
    files.insert(
        "weapons_breakdown".to_string(),
        "weapons_breakdown.json".to_string(),
    );

    let index = ManifestIndex {
        version: env!("CARGO_PKG_VERSION").to_string(),
        source: "BL4 Pak Files".to_string(),
        extract_path: extracted_dir.to_string_lossy().to_string(),
        files,
    };

    let index_path = output_dir.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
    println!("  index.json");

    println!(
        "\nManifest generated from {} pak assets",
        manifest.total_assets
    );
    println!("  Manufacturers: {:?}", manifest.manufacturers);
    println!(
        "  Weapon types: {:?}",
        manifest.weapon_types.keys().collect::<Vec<_>>()
    );
    println!("  Gear types: {:?}", manifest.gear_types);

    Ok(())
}
