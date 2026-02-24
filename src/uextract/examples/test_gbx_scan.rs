//! Quick test: scan IoStore for GbxSkillParamData and GbxStatusEffectData,
//! deserialize with custom parsers, and print results.

use anyhow::Result;
use std::path::Path;
use uextract::gbx;
use uextract::scanner::IoStoreScanner;

const PAK_PATH: &str =
    "/home/polar/.local/share/Steam/steamapps/common/Borderlands 4/OakGame/Content/Paks";
const SCRIPTOBJECTS_PATH: &str = "/tmp/scriptobjects.json";
const USMAP_PATH: &str = "share/manifest/BL4.usmap";

fn main() -> Result<()> {
    let mut scanner = IoStoreScanner::open(Path::new(PAK_PATH), None)?;
    scanner.load_scriptobjects(Path::new(SCRIPTOBJECTS_PATH))?;
    scanner.load_usmap(Path::new(USMAP_PATH))?;

    // Skill params
    eprintln!("Scanning for GbxSkillParamData...");
    let raw_skills = scanner.scan_class_raw("GbxSkillParamData")?;
    let mut params: Vec<gbx::SkillParamData> = Vec::new();
    for asset in &raw_skills {
        for export in &asset.exports {
            if let Some(p) = gbx::parse_skill_param(&export.data, &export.name, &asset.path) {
                params.push(p);
            }
        }
    }
    eprintln!(
        "  {} assets, {} parsed skill params",
        raw_skills.len(),
        params.len()
    );
    for p in params.iter().take(10) {
        println!("  skill: {} variant={} guid={}", p.name, p.variant, p.guid);
    }
    if params.len() > 10 {
        println!("  ... and {} more", params.len() - 10);
    }

    // Status effects
    eprintln!("\nScanning for GbxStatusEffectData...");
    let raw_effects = scanner.scan_class_raw("GbxStatusEffectData")?;
    let mut effects: Vec<gbx::StatusEffectData> = Vec::new();
    for asset in &raw_effects {
        for export in &asset.exports {
            if let Some(e) = gbx::parse_status_effect(&export.data, &export.name, &asset.path) {
                effects.push(e);
            }
        }
    }
    eprintln!(
        "  {} assets, {} parsed status effects",
        raw_effects.len(),
        effects.len()
    );
    for e in effects.iter().take(10) {
        println!(
            "  effect: {} driver={} aspects={} notifies={} guids={} tags={:?}",
            e.name,
            e.driver.class_name,
            e.aspects.len(),
            e.notify_events.len(),
            e.guids.len(),
            e.tags
        );
    }
    if effects.len() > 10 {
        println!("  ... and {} more", effects.len() - 10);
    }

    // Write full JSON to /tmp for inspection
    std::fs::write(
        "/tmp/skill_params.json",
        serde_json::to_string_pretty(&params)?,
    )?;
    std::fs::write(
        "/tmp/status_effects.json",
        serde_json::to_string_pretty(&effects)?,
    )?;
    eprintln!("\nWrote /tmp/skill_params.json and /tmp/status_effects.json");

    // Balance structs
    eprintln!("\nScanning for balance structs...");
    let balances = scanner.scan_by_path(|p| p.to_lowercase().contains("balancedata"))?;
    eprintln!("  {} balance assets", balances.len());

    std::fs::write(
        "/tmp/balance_structs.json",
        serde_json::to_string_pretty(&balances)?,
    )?;
    eprintln!("Wrote /tmp/balance_structs.json");

    Ok(())
}
