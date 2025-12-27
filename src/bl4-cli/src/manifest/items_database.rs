//! Items database extraction from pak_manifest.json
//!
//! Extracts item pools, item stats, and generates a complete items database
//! from the game's pak manifest data.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::PakManifest;

/// Manifest index for tracking extracted files
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

/// Extract manifest data and save to output directory
///
/// This is the main orchestrator function that extracts all manifest data
/// and writes individual JSON files for each category.
pub fn extract_manifest(extract_dir: &Path, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    println!("Extracting manifest from {:?}", extract_dir);
    println!("Output directory: {:?}", output_dir);

    // Manufacturers
    print!("Extracting manufacturers...");
    let manufacturers = super::extract_manufacturers(extract_dir);
    let mfr_path = output_dir.join("manufacturers.json");
    fs::write(&mfr_path, serde_json::to_string_pretty(&manufacturers)?)?;
    println!(" {} entries", manufacturers.len());

    // Weapon types
    print!("Extracting weapon types...");
    let weapon_types = super::extract_weapon_types(extract_dir);
    let wt_path = output_dir.join("weapon_types.json");
    fs::write(&wt_path, serde_json::to_string_pretty(&weapon_types)?)?;
    println!(" {} entries", weapon_types.len());

    // Balance data
    print!("Extracting balance data...");
    let balance_data = super::extract_balance_data(extract_dir)?;
    let bd_path = output_dir.join("balance_data.json");
    fs::write(&bd_path, serde_json::to_string_pretty(&balance_data)?)?;
    println!(" {} categories", balance_data.len());

    // Naming data
    print!("Extracting naming data...");
    let naming_data = super::extract_naming_data(extract_dir)?;
    let nd_path = output_dir.join("naming.json");
    fs::write(&nd_path, serde_json::to_string_pretty(&naming_data)?)?;
    println!(" {} entries", naming_data.len());

    // Gear types
    print!("Extracting gear types...");
    let gear_types = super::extract_gear_types(extract_dir);
    let gt_path = output_dir.join("gear_types.json");
    fs::write(&gt_path, serde_json::to_string_pretty(&gear_types)?)?;
    println!(" {} types", gear_types.len());

    // Rarity data
    print!("Extracting rarity data...");
    let rarity_data = super::extract_rarity_data(extract_dir);
    let rd_path = output_dir.join("rarity.json");
    fs::write(&rd_path, serde_json::to_string_pretty(&rarity_data)?)?;
    println!(" {} entries", rarity_data.len());

    // Elemental data
    print!("Extracting elemental data...");
    let elemental_data = super::extract_elemental_data(extract_dir);
    let ed_path = output_dir.join("elemental.json");
    fs::write(&ed_path, serde_json::to_string_pretty(&elemental_data)?)?;
    println!(" {} entries", elemental_data.len());

    // Save manifest index
    let mut files = HashMap::new();
    files.insert(
        "manufacturers".to_string(),
        "manufacturers.json".to_string(),
    );
    files.insert("weapon_types".to_string(), "weapon_types.json".to_string());
    files.insert("balance_data".to_string(), "balance_data.json".to_string());
    files.insert("naming".to_string(), "naming.json".to_string());
    files.insert("gear_types".to_string(), "gear_types.json".to_string());
    files.insert("rarity".to_string(), "rarity.json".to_string());
    files.insert("elemental".to_string(), "elemental.json".to_string());

    let index = ManifestIndex {
        version: env!("CARGO_PKG_VERSION").to_string(),
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
                let pool_name = cap
                    .as_str()
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
                if asset_name
                    .to_lowercase()
                    .contains(&pool_name.to_lowercase())
                {
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
    let stat_pattern =
        Regex::new(r"^([A-Za-z]+[A-Za-z0-9]*)_(Scale|Add|Value|Percent)_(\d+)_([A-F0-9]{32})$")
            .unwrap();

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
                    if (1..=5).contains(&tier) {
                        rarity = Some(rarities[tier - 1].to_string());
                    }
                }
            }
        }

        // Extract manufacturer from path
        let manufacturer = {
            let path_lower = item.path.to_lowercase();
            let codes = [
                "BOR", "DAD", "DPL", "JAK", "MAL", "ORD", "RIP", "TED", "TOR", "VLA", "COV",
            ];
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
        version: env!("CARGO_PKG_VERSION").to_string(),
        generated: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
        item_pools,
        items,
        stats_summary,
    })
}
