//! In-memory UAsset extraction from IoStore containers
//!
//! Scans PAK/IoStore files for specific asset classes and writes structured
//! manifest files. Uses custom deserializers for Gearbox-native classes
//! (GbxStatusEffectData, GbxSkillParamData) and standard property parsing
//! for UE5 classes (balance structs).

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use uextract::gbx;
use uextract::scanner::IoStoreScanner;

/// Summary of extracted UAsset manifest data.
#[derive(Debug, Serialize)]
pub struct UassetManifestSummary {
    pub status_effects_count: usize,
    pub skill_params_count: usize,
    pub balance_assets: usize,
    pub balance_categories: usize,
}

/// Summary of balance table extraction.
struct BalanceSummary {
    assets: usize,
    categories: usize,
}

/// Extract UAsset data from IoStore in-memory and write to manifest directory.
///
/// Scans for specific asset classes (status effects, skill params, balance structs)
/// and writes each as structured output to the output directory.
pub fn extract_uasset_manifest(
    paks_path: &Path,
    usmap_path: &Path,
    scriptobjects_path: &Path,
    output_dir: &Path,
    aes_key: Option<&str>,
) -> Result<UassetManifestSummary> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    let mut scanner = IoStoreScanner::open(paks_path, aes_key)?;
    scanner.load_scriptobjects(scriptobjects_path)?;
    scanner.load_usmap(usmap_path)?;

    let mut summary = UassetManifestSummary {
        status_effects_count: 0,
        skill_params_count: 0,
        balance_assets: 0,
        balance_categories: 0,
    };

    // GbxSkillParamData: custom native deserializer → TSV
    summary.skill_params_count = extract_skill_params(&scanner, output_dir)?;

    // GbxStatusEffectData: custom native deserializer → JSON
    summary.status_effects_count = extract_status_effects(&scanner, output_dir)?;

    // Balance structs: standard property parsing → per-category TSV
    let balance = extract_balance_tables(&scanner, output_dir)?;
    summary.balance_assets = balance.assets;
    summary.balance_categories = balance.categories;

    Ok(summary)
}

/// Extract GbxSkillParamData using custom binary deserializer.
fn extract_skill_params(scanner: &IoStoreScanner, output_dir: &Path) -> Result<usize> {
    print!("  Scanning for GbxSkillParamData...");
    match scanner.scan_class_raw("GbxSkillParamData") {
        Ok(raw_assets) => {
            let mut params: Vec<gbx::SkillParamData> = Vec::new();
            for asset in &raw_assets {
                for export in &asset.exports {
                    if let Some(param) =
                        gbx::parse_skill_param(&export.data, &export.name, &asset.path)
                    {
                        params.push(param);
                    }
                }
            }
            let count = params.len();
            write_json(&params, &output_dir.join("skill_params.json"))?;
            println!(" {} params (from {} assets)", count, raw_assets.len());
            Ok(count)
        }
        Err(e) => {
            println!(" failed: {}", e);
            Ok(0)
        }
    }
}

/// Extract GbxStatusEffectData using custom binary deserializer.
fn extract_status_effects(scanner: &IoStoreScanner, output_dir: &Path) -> Result<usize> {
    print!("  Scanning for GbxStatusEffectData...");
    match scanner.scan_class_raw("GbxStatusEffectData") {
        Ok(raw_assets) => {
            let mut effects: Vec<gbx::StatusEffectData> = Vec::new();
            for asset in &raw_assets {
                for export in &asset.exports {
                    if let Some(effect) =
                        gbx::parse_status_effect(&export.data, &export.name, &asset.path)
                    {
                        effects.push(effect);
                    }
                }
            }
            let count = effects.len();
            write_json(&effects, &output_dir.join("status_effects.json"))?;
            println!(" {} effects (from {} assets)", count, raw_assets.len());
            Ok(count)
        }
        Err(e) => {
            println!(" failed: {}", e);
            Ok(0)
        }
    }
}

/// Extract balance data as per-category TSV files.
///
/// Groups IoStore assets by their BalanceData subdirectory,
/// flattens ParsedProperty values into rows, writes one TSV per category.
fn extract_balance_tables(scanner: &IoStoreScanner, output_dir: &Path) -> Result<BalanceSummary> {
    print!("  Scanning for balance structs...");
    let assets = match scanner.scan_by_path(|path| path.to_lowercase().contains("balancedata")) {
        Ok(a) => a,
        Err(e) => {
            println!(" failed: {}", e);
            return Ok(BalanceSummary { assets: 0, categories: 0 });
        }
    };

    let total_assets = assets.len();

    // Group assets by category directory: category → list of column→value rows
    let mut categories: BTreeMap<String, Vec<BTreeMap<String, String>>> = BTreeMap::new();

    let mut populated = 0usize;

    for asset in &assets {
        let category = extract_category_from_path(&asset.path);
        let asset_name = extract_asset_name(&asset.path);

        // Flatten first export with properties into column→value map
        let mut row = BTreeMap::new();
        for export in &asset.exports {
            if let Some(props) = &export.properties {
                for prop in props {
                    let value = format_property_value(prop);
                    if !value.is_empty() {
                        row.insert(prop.name.clone(), value);
                    }
                }
                if !row.is_empty() {
                    populated += 1;
                    break;
                }
            }
        }

        row.insert("_asset_name".to_string(), asset_name);

        categories.entry(category).or_default().push(row);
    }

    let balance_dir = output_dir.join("balance");
    fs::create_dir_all(&balance_dir).context("Failed to create balance output directory")?;

    let total_rows = write_balance_tsvs(&categories, &balance_dir)?;

    let cat_count = categories.len();
    println!(
        " {} assets across {} categories ({} with properties, {} rows)",
        total_assets, cat_count, populated, total_rows
    );

    Ok(BalanceSummary {
        assets: total_assets,
        categories: cat_count,
    })
}

/// Write per-category TSVs and an index file. Returns total row count.
fn write_balance_tsvs(
    categories: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    output_dir: &Path,
) -> Result<usize> {
    let mut total_rows = 0usize;
    let mut index_lines: Vec<String> = Vec::new();

    for (category, entries) in categories {
        let mut columns: BTreeSet<String> = BTreeSet::new();
        for row in entries {
            for key in row.keys() {
                if key != "_asset_name" {
                    columns.insert(key.clone());
                }
            }
        }
        let columns: Vec<String> = columns.into_iter().collect();

        let mut tsv = String::from("asset_name");
        for col in &columns {
            tsv.push('\t');
            tsv.push_str(col);
        }
        tsv.push('\n');

        for row in entries {
            let name = row.get("_asset_name").map(|s| s.as_str()).unwrap_or("");
            tsv.push_str(name);
            for col in &columns {
                tsv.push('\t');
                if let Some(val) = row.get(col) {
                    tsv.push_str(val);
                }
            }
            tsv.push('\n');
            total_rows += 1;
        }

        let tsv_path = output_dir.join(format!("{}.tsv", category));
        fs::write(&tsv_path, &tsv)
            .with_context(|| format!("Failed to write {:?}", tsv_path))?;

        index_lines.push(format!("{}\t{}", category, entries.len()));
    }

    let mut index_tsv = String::from("category\tassets\n");
    for line in &index_lines {
        index_tsv.push_str(line);
        index_tsv.push('\n');
    }
    fs::write(output_dir.join("index.tsv"), &index_tsv)?;

    Ok(total_rows)
}

/// Extract the category directory name from a balance data asset path.
///
/// e.g. `.../BalanceData/BarrelData/DT_foo.uasset` → `"BarrelData"`
fn extract_category_from_path(path: &str) -> String {
    let lower = path.to_lowercase();
    if let Some(pos) = lower.find("balancedata/") {
        let after = &path[pos + "balancedata/".len()..];
        if let Some(slash) = after.find('/') {
            return after[..slash].to_string();
        }
        // No subdirectory — asset is directly in BalanceData/
        return "uncategorized".to_string();
    }
    "uncategorized".to_string()
}

/// Extract the asset name (filename without extension) from a path.
fn extract_asset_name(path: &str) -> String {
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename
        .strip_suffix(".uasset")
        .or_else(|| filename.strip_suffix(".uexp"))
        .unwrap_or(filename)
        .to_string()
}

/// Format a ParsedProperty value as a string for TSV output.
fn format_property_value(prop: &uextract::types::ParsedProperty) -> String {
    if let Some(f) = prop.float_value {
        return format!("{}", f);
    }
    if let Some(i) = prop.int_value {
        return format!("{}", i);
    }
    if let Some(ref s) = prop.string_value {
        return s.clone();
    }
    if let Some(ref s) = prop.enum_value {
        return s.clone();
    }
    if let Some(ref s) = prop.object_path {
        return s.clone();
    }
    String::new()
}

fn write_json<T: Serialize>(data: &T, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    fs::write(path, json).with_context(|| format!("Failed to write {:?}", path))
}
