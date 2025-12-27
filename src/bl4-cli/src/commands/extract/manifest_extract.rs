//! Manifest extraction command handlers
//!
//! Handlers for extracting manufacturer, weapon type, gear type, element, rarity, and stat data.

use crate::manifest;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Handle the ExtractCommand::Manufacturers command
///
/// Extracts manufacturer data from a pak file.
pub fn handle_manufacturers(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting manufacturer data from {:?}...", input);
    let manufacturers = manifest::extract_manufacturer_names_from_pak(input)?;

    println!("\nDiscovered {} manufacturers:", manufacturers.len());
    for (code, mfr) in &manufacturers {
        println!("  {} = {} (source: {})", code, mfr.name, mfr.name_source);
    }

    let json = serde_json::to_string_pretty(&manufacturers)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}

/// Handle the ExtractCommand::WeaponTypes command
///
/// Extracts weapon type data from a pak file.
pub fn handle_weapon_types(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting weapon type data from {:?}...", input);
    let weapon_types = manifest::extract_weapon_types_from_pak(input)?;

    println!("\nDiscovered {} weapon types:", weapon_types.len());
    for (name, wt) in &weapon_types {
        println!(
            "  {} ({}) - {} manufacturers: {:?}",
            name,
            wt.code,
            wt.manufacturers.len(),
            wt.manufacturers
        );
    }

    let json = serde_json::to_string_pretty(&weapon_types)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}

/// Handle the ExtractCommand::GearTypes command
///
/// Extracts gear type data from a pak file.
pub fn handle_gear_types(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting gear type data from {:?}...", input);
    let gear_types = manifest::extract_gear_types_from_pak(input)?;

    println!("\nDiscovered {} gear types:", gear_types.len());
    for (name, gt) in &gear_types {
        if gt.manufacturers.is_empty() {
            println!("  {} (no manufacturers)", name);
        } else {
            println!(
                "  {} - {} manufacturers: {:?}",
                name,
                gt.manufacturers.len(),
                gt.manufacturers
            );
        }
        if !gt.subcategories.is_empty() {
            println!("    subcategories: {:?}", gt.subcategories);
        }
    }

    let json = serde_json::to_string_pretty(&gear_types)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}

/// Handle the ExtractCommand::Elements command
///
/// Extracts element type data from a pak file.
pub fn handle_elements(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting element types from {:?}...", input);
    let elements = manifest::extract_elements_from_pak(input)?;

    println!("\nDiscovered {} element types:", elements.len());
    for name in elements.keys() {
        println!("  {}", name);
    }

    let json = serde_json::to_string_pretty(&elements)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}

/// Handle the ExtractCommand::Rarities command
///
/// Extracts rarity tier data from a pak file.
pub fn handle_rarities(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting rarity tiers from {:?}...", input);
    let rarities = manifest::extract_rarities_from_pak(input)?;

    println!("\nDiscovered {} rarity tiers:", rarities.len());
    for rarity in &rarities {
        println!("  {} ({}) = {}", rarity.tier, rarity.code, rarity.name);
    }

    let json = serde_json::to_string_pretty(&rarities)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}

/// Handle the ExtractCommand::Stats command
///
/// Extracts stat type data from a pak file.
pub fn handle_stats(input: &Path, output: &Path) -> Result<()> {
    println!("Extracting stat types from {:?}...", input);
    let stats = manifest::extract_stats_from_pak(input)?;

    println!(
        "\nDiscovered {} stat types (top 20 by occurrence):",
        stats.len()
    );
    for stat in stats.iter().take(20) {
        if stat.modifier_types.is_empty() {
            println!("  {} ({} occurrences)", stat.name, stat.occurrences);
        } else {
            println!(
                "  {} [{:?}] ({} occurrences)",
                stat.name, stat.modifier_types, stat.occurrences
            );
        }
    }
    if stats.len() > 20 {
        println!("  ... and {} more", stats.len() - 20);
    }

    let json = serde_json::to_string_pretty(&stats)?;
    fs::write(output, json)?;
    println!("\nSaved to {:?}", output);

    Ok(())
}
