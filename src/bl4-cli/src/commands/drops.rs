//! Drop rate and location command handlers

use crate::cli::DropsCommand;
use anyhow::{Context, Result};
use bl4_ncs::{generate_drops_manifest, DropsDb};
use std::path::Path;

/// Handle the drops command
pub fn handle(command: DropsCommand) -> Result<()> {
    match command {
        DropsCommand::Find { item, manifest } => find_item(&item.join(" "), &manifest),
        DropsCommand::Source { name, manifest } => find_source(&name.join(" "), &manifest),
        DropsCommand::List { sources, manifest } => list(&manifest, sources),
        DropsCommand::Generate { ncs_dir, output } => generate(&ncs_dir, &output),
    }
}

fn find_item(item: &str, manifest_path: &Path) -> Result<()> {
    let db = DropsDb::load(manifest_path).context("Failed to load drops manifest")?;

    let locations = db.find_by_name(item);

    if locations.is_empty() {
        println!("No drops found for '{}'", item);
        println!("\nTry a partial name like 'plasma' or 'hell'");
        return Ok(());
    }

    for loc in &locations {
        // Use display name if available, otherwise fall back to internal name
        let source_name = loc
            .source_display
            .as_ref()
            .unwrap_or(&loc.source);

        println!(
            "{:<20} {:<30} {:<12} {:<12} {:>8}",
            loc.item_name,
            source_name,
            loc.source_type.to_string(),
            if loc.tier.is_empty() { "-" } else { &loc.tier },
            loc.chance_display
        );
    }

    Ok(())
}

fn find_source(name: &str, manifest_path: &Path) -> Result<()> {
    let db = DropsDb::load(manifest_path).context("Failed to load drops manifest")?;

    let items = db.find_by_source(name);

    if items.is_empty() {
        println!("No drops found for source '{}'", name);
        println!("\nTry 'bl4 drops list --sources' to see all source names");
        println!("Or try 'Black Market', 'Fish Collector', etc.");
        return Ok(());
    }

    // Use display name from first item if available
    let display_name = items
        .first()
        .and_then(|i| i.source_display.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(name);

    println!("Drops from '{}' (sorted by drop rate):\n", display_name);
    println!(
        "{:<25} {:<8} {:<12} {:>8}",
        "Item", "Type", "Tier", "Chance"
    );
    println!("{}", "-".repeat(55));

    for item in &items {
        let chance_str = if item.drop_chance > 0.0 {
            format!("{:.0}%", item.drop_chance * 100.0)
        } else {
            "-".to_string()
        };
        println!(
            "{:<25} {:<8} {:<12} {:>8}",
            item.item_name,
            format!("{}_{}", item.manufacturer, item.gear_type),
            if item.drop_tier.is_empty() {
                "-"
            } else {
                &item.drop_tier
            },
            chance_str
        );
    }

    Ok(())
}

fn list(manifest_path: &Path, list_sources: bool) -> Result<()> {
    let db = DropsDb::load(manifest_path).context("Failed to load drops manifest")?;

    if list_sources {
        let sources = db.all_sources();
        println!("Known drop sources ({}):\n", sources.len());
        for source in sources {
            println!("  {}", source);
        }
    } else {
        let items = db.all_items();
        println!("Known legendary items ({}):\n", items.len());
        for item in items {
            println!("  {}", item);
        }
    }

    Ok(())
}

fn generate(ncs_dir: &Path, output: &Path) -> Result<()> {
    use bl4_ncs::DropSource;

    println!("Generating drops manifest from {}...", ncs_dir.display());

    let manifest = generate_drops_manifest(ncs_dir).context("Failed to generate drops manifest")?;

    // Ensure parent directory exists
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(output, &json)?;

    // Count by source type
    let boss_count = manifest
        .drops
        .iter()
        .filter(|d| d.source_type == DropSource::Boss)
        .map(|d| &d.source)
        .collect::<std::collections::HashSet<_>>()
        .len();
    let world_drop_count = manifest
        .drops
        .iter()
        .filter(|d| d.source_type == DropSource::WorldDrop)
        .count();
    let mission_count = manifest
        .drops
        .iter()
        .filter(|d| d.source_type == DropSource::Mission)
        .count();
    let black_market_count = manifest
        .drops
        .iter()
        .filter(|d| d.source_type == DropSource::BlackMarket)
        .count();
    let special_count = manifest
        .drops
        .iter()
        .filter(|d| d.source_type == DropSource::Special)
        .count();

    println!(
        "Wrote {} drops to {}",
        manifest.drops.len(),
        output.display()
    );
    println!("  {} bosses", boss_count);
    if world_drop_count > 0 {
        println!("  {} world drop items", world_drop_count);
    }
    if mission_count > 0 {
        println!("  {} mission rewards", mission_count);
    }
    if black_market_count > 0 {
        println!("  {} black market items", black_market_count);
    }
    if special_count > 0 {
        println!("  {} special sources", special_count);
    }

    Ok(())
}
