//! In-memory UAsset extraction from IoStore containers
//!
//! Scans PAK/IoStore files for specific asset classes and writes structured
//! manifest files. Uses custom deserializers for Gearbox-native classes
//! (GbxStatusEffectData, GbxSkillParamData) and standard property parsing
//! for UE5 classes (balance structs).

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

use uextract::gbx;
use uextract::scanner::IoStoreScanner;
use uextract::types::ZenAssetInfo;

/// Summary of extracted UAsset manifest data.
#[derive(Debug, Serialize)]
pub struct UassetManifestSummary {
    pub status_effects_count: usize,
    pub skill_params_count: usize,
    pub balance_structs_count: usize,
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
        balance_structs_count: 0,
    };

    // GbxSkillParamData: custom native deserializer → TSV
    summary.skill_params_count = extract_skill_params(&scanner, output_dir)?;

    // GbxStatusEffectData: custom native deserializer → JSON
    summary.status_effects_count = extract_status_effects(&scanner, output_dir)?;

    // Balance structs: standard property parsing → JSON
    summary.balance_structs_count = extract_balance_structs(&scanner, output_dir)?;

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

/// Extract balance structs using standard property parsing.
fn extract_balance_structs(scanner: &IoStoreScanner, output_dir: &Path) -> Result<usize> {
    print!("  Scanning for balance structs...");
    match scanner.scan_by_path(|path| path.to_lowercase().contains("balancedata")) {
        Ok(assets) => {
            let count = assets.len();
            write_json(&assets, &output_dir.join("balance_structs.json"))?;
            println!(" {} assets", count);
            Ok(count)
        }
        Err(e) => {
            println!(" failed: {}", e);
            Ok(0)
        }
    }
}

fn write_json<T: Serialize>(data: &T, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    fs::write(path, json).with_context(|| format!("Failed to write {:?}", path))
}
