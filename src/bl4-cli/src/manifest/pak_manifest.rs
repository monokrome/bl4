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

use super::items_database::ManifestIndex;

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

impl PakManifest {
    /// Load manifest from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read pak_manifest.json")?;
        let mut manifest: Self =
            serde_json::from_str(&content).context("Failed to parse pak_manifest.json")?;
        for item in &mut manifest.items {
            item.path = item.path.replace('\\', "/");
        }
        Ok(manifest)
    }
}

/// Parse a uextract JSON file
pub fn parse_uextract_json(json_path: &Path) -> Result<UextractAsset> {
    let content = fs::read_to_string(json_path)?;
    let mut asset: UextractAsset = serde_json::from_str(&content)?;
    asset.path = asset.path.replace('\\', "/");
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

struct ItemCategory {
    category: String,
    weapon_type: Option<String>,
    manufacturer: Option<String>,
}

fn categorize_weapon_path(path_str: &str, mfr_names: &HashMap<&str, &str>) -> ItemCategory {
    let weapon_type = if path_str.contains("assaultrifles") {
        Some("AssaultRifle".to_string())
    } else if path_str.contains("pistols") {
        Some("Pistol".to_string())
    } else if path_str.contains("shotguns") {
        Some("Shotgun".to_string())
    } else if path_str.contains("smg") {
        Some("SMG".to_string())
    } else if path_str.contains("sniper") {
        Some("Sniper".to_string())
    } else if path_str.contains("heavy") || path_str.contains("heavyweapons") {
        Some("Heavy".to_string())
    } else {
        None
    };

    let manufacturer = mfr_names.keys().find_map(|code| {
        let code_lower = code.to_lowercase();
        if path_str.contains(&format!("/{}/", code_lower))
            || path_str.contains(&format!("/{}_", code_lower))
        {
            Some(code.to_string())
        } else {
            None
        }
    });

    ItemCategory {
        category: "weapon".to_string(),
        weapon_type,
        manufacturer,
    }
}

fn categorize_classmod_path(path_str: &str) -> ItemCategory {
    let manufacturer = if path_str.contains("gravitar") {
        Some("GRV".to_string())
    } else if path_str.contains("paladin") {
        Some("PLD".to_string())
    } else if path_str.contains("darksiren") || path_str.contains("dark_siren") {
        Some("SIR".to_string())
    } else if path_str.contains("exo") {
        Some("EXO".to_string())
    } else {
        None
    };

    ItemCategory {
        category: "classmod".to_string(),
        weapon_type: None,
        manufacturer,
    }
}

fn categorize_enhancement_path(path_str: &str, mfr_names: &HashMap<&str, &str>) -> ItemCategory {
    let manufacturer = mfr_names.keys().find_map(|code| {
        let code_lower = code.to_lowercase();
        if path_str.contains(&format!("_{}_", code_lower))
            || path_str.contains(&format!("/{}/", code_lower))
        {
            Some(code.to_string())
        } else {
            None
        }
    });

    ItemCategory {
        category: "enhancement".to_string(),
        weapon_type: None,
        manufacturer,
    }
}

fn determine_category(
    path_str: &str,
    mfr_names: &HashMap<&str, &str>,
    gear_types: &mut HashSet<String>,
) -> ItemCategory {
    if path_str.contains("gear/weapons") || path_str.contains("gear/gadgets/heavyweapons") {
        return categorize_weapon_path(path_str, mfr_names);
    }

    if path_str.contains("gear/classmods") {
        gear_types.insert("ClassMod".to_string());
        return categorize_classmod_path(path_str);
    }

    if path_str.contains("gear/enhancements") {
        gear_types.insert("Enhancement".to_string());
        return categorize_enhancement_path(path_str, mfr_names);
    }

    if path_str.contains("gear/shields") {
        gear_types.insert("Shield".to_string());
        return ItemCategory {
            category: "shield".to_string(),
            weapon_type: None,
            manufacturer: None,
        };
    }

    if path_str.contains("gear/grenadegadgets") {
        gear_types.insert("Grenade".to_string());
        return ItemCategory {
            category: "grenade".to_string(),
            weapon_type: None,
            manufacturer: None,
        };
    }

    if path_str.contains("gear/gadgets") {
        gear_types.insert("Gadget".to_string());
        return ItemCategory {
            category: "gadget".to_string(),
            weapon_type: None,
            manufacturer: None,
        };
    }

    if path_str.contains("gear/firmware") {
        gear_types.insert("Firmware".to_string());
        return ItemCategory {
            category: "firmware".to_string(),
            weapon_type: None,
            manufacturer: None,
        };
    }

    if path_str.contains("gear/repairkits") {
        gear_types.insert("RepairKit".to_string());
        return ItemCategory {
            category: "repair_kit".to_string(),
            weapon_type: None,
            manufacturer: None,
        };
    }

    ItemCategory {
        category: "unknown".to_string(),
        weapon_type: None,
        manufacturer: None,
    }
}

fn extract_stat_values(asset: &UextractAsset) -> Vec<StatValue> {
    let mut stat_values = Vec::new();
    for export in &asset.exports {
        let Some(ref props) = export.properties else {
            continue;
        };
        for prop in props {
            let Some(val) = prop.float_value else {
                continue;
            };
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
    stat_values
}

fn write_manifest_output(
    manifest: &PakManifest,
    extracted_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    let manifest_path = output_dir.join("pak_manifest.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(manifest)?)?;
    println!(
        "  pak_manifest.json - {} assets indexed",
        manifest.total_assets
    );

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

    let weapons_breakdown: HashMap<String, serde_json::Value> = manifest
        .weapon_types
        .iter()
        .map(|(wt, mfrs)| {
            (wt.clone(), serde_json::json!({
                "manufacturers": mfrs,
                "count": manifest.items.iter().filter(|i| i.weapon_type.as_ref() == Some(wt)).count()
            }))
        })
        .collect();
    let weapons_path = output_dir.join("weapons_breakdown.json");
    fs::write(
        &weapons_path,
        serde_json::to_string_pretty(&weapons_breakdown)?,
    )?;
    println!("  weapons_breakdown.json");

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
        let asset = match parse_uextract_json(json_path) {
            Ok(a) => a,
            Err(_) => continue,
        };

        total_assets += 1;
        let path_str = asset.path.to_lowercase();

        let cat = determine_category(&path_str, &mfr_names, &mut gear_types);

        if let Some(ref wt) = cat.weapon_type {
            if let Some(ref mfr) = cat.manufacturer {
                manufacturers.insert(mfr.clone());
                weapon_types
                    .entry(wt.clone())
                    .or_default()
                    .push(mfr.clone());
            }
        }

        if path_str.contains("balancedata") {
            let bd_cat = cat
                .weapon_type
                .as_ref()
                .cloned()
                .unwrap_or_else(|| cat.category.clone());
            balance_data
                .entry(bd_cat)
                .or_default()
                .push(asset.package_name.clone());
        }

        if path_str.contains("namingstrateg") {
            naming_strategies.push(asset.package_name.clone());
        }

        for (stat_name, guid) in extract_stats_from_names(&asset.names) {
            all_stats.entry(stat_name).or_default().push(guid);
        }

        let asset_name = json_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".uasset"))
            .unwrap_or("")
            .to_string();

        let unique_id = asset
            .names
            .iter()
            .find(|n| n.contains("comp_05") || n.contains("Unique") || n.contains("legendary"))
            .cloned();

        let stat_values = extract_stat_values(&asset);
        let stats = if stat_values.is_empty() {
            None
        } else {
            Some(stat_values)
        };

        items.push(ExtractedItem {
            path: asset.path.clone(),
            asset_name,
            category: cat.category,
            weapon_type: cat.weapon_type,
            manufacturer: cat.manufacturer,
            unique_id,
            property_names: asset.names.clone(),
            stats,
        });
    }

    for mfrs_list in weapon_types.values_mut() {
        mfrs_list.sort();
        mfrs_list.dedup();
    }
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

    write_manifest_output(&manifest, extracted_dir, output_dir)
}
