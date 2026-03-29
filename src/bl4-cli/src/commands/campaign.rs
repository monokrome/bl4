//! Campaign progression command handlers

use anyhow::{bail, Result};
use std::io::{self, BufRead, Write};

use crate::cli::{CampaignAction, SaveArgs};
use crate::commands::save::get_steam_id;

pub fn handle(args: &SaveArgs, action: &CampaignAction) -> Result<()> {
    match action {
        CampaignAction::List => list(args),
        CampaignAction::Set { mission, yes } => set(args, mission, *yes),
    }
}

fn list(args: &SaveArgs) -> Result<()> {
    let steam_id = get_steam_id(args.steam_id.clone())?;
    let encrypted = std::fs::read(&args.input)?;
    let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)?;
    let save = bl4::SaveFile::from_yaml(&yaml_data)?;

    let status = save.campaign_status();

    if status.is_empty() {
        println!("No main story missions found in manifest.");
        return Ok(());
    }

    let max_name = status
        .iter()
        .map(|e| short_name(&e.mission_set).len())
        .max()
        .unwrap_or(20);

    for entry in &status {
        let icon = match entry.status {
            bl4::save::campaign::CampaignStatus::Completed => "[x]",
            bl4::save::campaign::CampaignStatus::Active => "[>]",
            bl4::save::campaign::CampaignStatus::NotStarted => "[ ]",
        };

        let name = short_name(&entry.mission_set);
        let region = if entry.region.is_empty() {
            String::new()
        } else {
            format!("  ({})", entry.region)
        };

        println!("{} {:<width$}{}", icon, name, region, width = max_name + 2);
    }

    Ok(())
}

fn set(args: &SaveArgs, mission: &str, skip_confirm: bool) -> Result<()> {
    let resolved = bl4::missions::resolve_mission_set_name(mission);
    if resolved.is_none() {
        bail!(
            "Unknown mission: '{}'\n\nUse 'bl4 save <file> campaign list' to see available missions.\n\
            Short names like 'grasslands1', 'mountains2a', 'searchforlilith' are accepted.",
            mission
        );
    }

    let changes = bl4::save::campaign::plan_campaign_progress(mission)
        .ok_or_else(|| anyhow::anyhow!("Failed to compute campaign changes for '{}'", mission))?;

    println!("Campaign progress will be set to: {}", short_name(&changes.active_set));
    println!();

    if !changes.completed_sets.is_empty() {
        println!(
            "The following {} mission(s) will be marked completed:",
            changes.completed_sets.len()
        );
        for set_name in &changes.completed_sets {
            println!("  [x] {}", short_name(set_name));
        }
        println!();
    }

    println!(
        "Active mission: {} ({})",
        changes.active_mission,
        short_name(&changes.active_set)
    );
    println!();

    if !skip_confirm {
        print!("Apply these changes? [y/N] ");
        io::stdout().flush()?;

        let stdin = io::stdin();
        let response = stdin.lock().lines().next();
        let confirmed = response
            .and_then(|r| r.ok())
            .map(|r| r.trim().eq_ignore_ascii_case("y"))
            .unwrap_or(false);

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    crate::commands::save::with_save_file(args, |save| {
        save.apply_campaign_progress(&changes)?;
        Ok(())
    })?;

    println!("Campaign progress updated.");

    Ok(())
}

fn short_name(set_name: &str) -> &str {
    set_name
        .strip_prefix("missionset_main_")
        .unwrap_or(set_name)
}
