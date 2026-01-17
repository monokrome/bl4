//! Static reference data wrappers for JSON serialization
//!
//! WARNING: The data in this module is HARDCODED for reference purposes only.
//! It should NOT be used as authoritative game data in implementation.
//! These functions exist to provide a starting point for understanding
//! the game's data structures, but actual values must be extracted from
//! the game files themselves.
//!
//! Output from generate_reference_manifest() goes to share/manifest/reference/
//! to clearly separate it from extracted authoritative data.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::ManifestIndex;

// ============================================================================
// Serializable Wrapper Types
// ============================================================================

/// Rarity tiers (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RarityTier {
    pub tier: u8,
    pub code: String,
    pub name: String,
    pub color: String,
}

/// Get rarity tiers from bl4::reference for JSON serialization
pub fn rarity_tiers() -> Vec<RarityTier> {
    bl4::reference::RARITY_TIERS
        .iter()
        .map(|r| RarityTier {
            tier: r.tier,
            code: r.code.to_string(),
            name: r.name.to_string(),
            color: r.color.to_string(),
        })
        .collect()
}

/// Element types (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub name: String,
    pub description: String,
    pub color: String,
}

/// Get element types from bl4::reference for JSON serialization
pub fn element_types() -> Vec<ElementType> {
    bl4::reference::ELEMENT_TYPES
        .iter()
        .map(|e| ElementType {
            code: e.code.to_string(),
            name: e.name.to_string(),
            description: e.description.to_string(),
            color: e.color.to_string(),
        })
        .collect()
}

/// Known legendary items (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendaryItem {
    pub internal: String,
    pub name: String,
    pub weapon_type: String,
    pub manufacturer: String,
}

/// Get known legendaries from bl4::reference for JSON serialization
pub fn known_legendaries() -> Vec<LegendaryItem> {
    bl4::reference::KNOWN_LEGENDARIES
        .iter()
        .map(|l| LegendaryItem {
            internal: l.internal.to_string(),
            name: l.name.to_string(),
            weapon_type: l.weapon_type.to_string(),
            manufacturer: l.manufacturer.to_string(),
        })
        .collect()
}

/// Weapon type definitions (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponTypeInfo {
    pub code: String,
    pub name: String,
    pub description: String,
}

/// Get weapon type info from bl4::reference for JSON serialization
pub fn weapon_type_info() -> Vec<WeaponTypeInfo> {
    bl4::reference::WEAPON_TYPES
        .iter()
        .map(|w| WeaponTypeInfo {
            code: w.code.to_string(),
            name: w.name.to_string(),
            description: w.description.to_string(),
        })
        .collect()
}

/// Extended manufacturer info (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturerInfo {
    pub code: String,
    pub name: String,
    pub weapon_types: Vec<String>,
    pub style: String,
}

/// Get manufacturer info from bl4::reference for JSON serialization
pub fn manufacturer_info() -> Vec<ManufacturerInfo> {
    bl4::reference::MANUFACTURERS
        .iter()
        .map(|m| ManufacturerInfo {
            code: m.code.to_string(),
            name: m.name.to_string(),
            weapon_types: m.weapon_types.iter().map(|s| s.to_string()).collect(),
            style: m.style.to_string(),
        })
        .collect()
}

/// Gear type definitions (serializable wrapper for JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearTypeInfo {
    pub code: String,
    pub name: String,
    pub description: String,
}

/// Get gear type info from bl4::reference for JSON serialization
pub fn gear_type_info() -> Vec<GearTypeInfo> {
    bl4::reference::GEAR_TYPES
        .iter()
        .map(|g| GearTypeInfo {
            code: g.code.to_string(),
            name: g.name.to_string(),
            description: g.description.to_string(),
        })
        .collect()
}

// ============================================================================
// Consolidated Manifest Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsolidatedManifest {
    pub version: String,
    pub game: String,
    pub description: String,
    pub manufacturers: Vec<ManufacturerInfo>,
    pub weapon_types: Vec<WeaponTypeInfo>,
    pub gear_types: Vec<GearTypeInfo>,
    pub rarities: Vec<RarityTier>,
    pub elements: Vec<ElementType>,
    pub stats: HashMap<String, String>,
    pub legendaries: Vec<LegendaryItem>,
}

/// Generate consolidated reference manifest with all static data.
///
/// WARNING: This outputs HARDCODED REFERENCE DATA, not extracted game data.
/// Output should go to share/manifest/reference/ to separate it from
/// authoritative extracted data.
///
/// This data is useful as a guide for:
/// - Understanding what data structures exist
/// - Knowing what to look for when extracting
/// - Quick prototyping before extraction is implemented
///
/// But it MUST NOT be used in production code paths.
pub fn generate_reference_manifest(output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    println!("Generating consolidated reference manifest (HARDCODED - NOT AUTHORITATIVE)...");

    // Write README explaining this is reference data
    let readme = r#"# Reference Data

WARNING: The files in this directory contain HARDCODED REFERENCE DATA.
They are NOT extracted from game files and should NOT be used in implementation.

These files exist to:
- Document known game data structures
- Provide a starting point for extraction work
- Allow quick prototyping before proper extraction

For authoritative data, use files in the parent directory (share/manifest/)
which are generated by extraction commands that read actual game files.

## Files

- manufacturers.json - Known manufacturer codes and names (HARDCODED)
- weapon_types.json - Weapon type codes (HARDCODED)
- gear_types.json - Gear type codes (HARDCODED)
- rarities.json - Rarity tiers with colors (HARDCODED)
- elements.json - Element types (HARDCODED)
- stats.json - Stat property names (HARDCODED)
- legendaries.json - Known legendary items (HARDCODED)
- reference.json - All above consolidated (HARDCODED)
- parts_database_spreadsheet.json - Community spreadsheet data (EXTERNAL)
"#;
    fs::write(output_dir.join("README.md"), readme)?;
    println!("  README.md - documentation");

    // Stats
    let stats: HashMap<String, String> = bl4::reference::all_stat_descriptions()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let manifest = ConsolidatedManifest {
        version: env!("CARGO_PKG_VERSION").to_string(),
        game: "Borderlands 4".to_string(),
        description: "REFERENCE DATA ONLY - Hardcoded, not extracted from game".to_string(),
        manufacturers: manufacturer_info(),
        weapon_types: weapon_type_info(),
        gear_types: gear_type_info(),
        rarities: rarity_tiers(),
        elements: element_types(),
        stats,
        legendaries: known_legendaries(),
    };

    // Write individual files
    let mfr_path = output_dir.join("manufacturers.json");
    fs::write(
        &mfr_path,
        serde_json::to_string_pretty(&manifest.manufacturers)?,
    )?;
    println!(
        "  manufacturers.json - {} entries",
        manifest.manufacturers.len()
    );

    let wt_path = output_dir.join("weapon_types.json");
    fs::write(
        &wt_path,
        serde_json::to_string_pretty(&manifest.weapon_types)?,
    )?;
    println!(
        "  weapon_types.json - {} entries",
        manifest.weapon_types.len()
    );

    let gt_path = output_dir.join("gear_types.json");
    fs::write(
        &gt_path,
        serde_json::to_string_pretty(&manifest.gear_types)?,
    )?;
    println!("  gear_types.json - {} entries", manifest.gear_types.len());

    let rarity_path = output_dir.join("rarities.json");
    fs::write(
        &rarity_path,
        serde_json::to_string_pretty(&manifest.rarities)?,
    )?;
    println!("  rarities.json - {} entries", manifest.rarities.len());

    let elem_path = output_dir.join("elements.json");
    fs::write(
        &elem_path,
        serde_json::to_string_pretty(&manifest.elements)?,
    )?;
    println!("  elements.json - {} entries", manifest.elements.len());

    let stats_path = output_dir.join("stats.json");
    fs::write(&stats_path, serde_json::to_string_pretty(&manifest.stats)?)?;
    println!("  stats.json - {} entries", manifest.stats.len());

    let leg_path = output_dir.join("legendaries.json");
    fs::write(
        &leg_path,
        serde_json::to_string_pretty(&manifest.legendaries)?,
    )?;
    println!(
        "  legendaries.json - {} entries",
        manifest.legendaries.len()
    );

    // Write consolidated manifest
    let consolidated_path = output_dir.join("reference.json");
    fs::write(&consolidated_path, serde_json::to_string_pretty(&manifest)?)?;
    println!("  reference.json - consolidated reference data");

    // Write index
    let mut files = HashMap::new();
    files.insert(
        "manufacturers".to_string(),
        "manufacturers.json".to_string(),
    );
    files.insert("weapon_types".to_string(), "weapon_types.json".to_string());
    files.insert("gear_types".to_string(), "gear_types.json".to_string());
    files.insert("rarities".to_string(), "rarities.json".to_string());
    files.insert("elements".to_string(), "elements.json".to_string());
    files.insert("stats".to_string(), "stats.json".to_string());
    files.insert("legendaries".to_string(), "legendaries.json".to_string());
    files.insert("reference".to_string(), "reference.json".to_string());

    let index = ManifestIndex {
        version: env!("CARGO_PKG_VERSION").to_string(),
        source: "HARDCODED REFERENCE DATA - NOT EXTRACTED FROM GAME".to_string(),
        extract_path: output_dir.to_string_lossy().to_string(),
        files,
    };

    let index_path = output_dir.join("index.json");
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;
    println!("  index.json");

    println!("\nReference manifest saved to {:?}", output_dir);
    println!("WARNING: This is REFERENCE DATA ONLY - do not use in implementation!");
    Ok(())
}
