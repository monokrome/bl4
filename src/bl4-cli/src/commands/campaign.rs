//! Mission progression command handlers

use anyhow::{bail, Result};
use std::io::{self, BufRead, Write};

use crate::cli::{MissionsAction, SaveArgs};
use crate::commands::save::get_steam_id;

pub fn handle(args: &SaveArgs, action: &MissionsAction) -> Result<()> {
    match action {
        MissionsAction::List { category } => list(args, category),
        MissionsAction::Set { mission, yes } => set(args, mission, *yes),
    }
}

fn list(args: &SaveArgs, category: &str) -> Result<()> {
    let steam_id = get_steam_id(args.steam_id.clone())?;
    let encrypted = std::fs::read(&args.input)?;
    let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)?;
    let save = bl4::SaveFile::from_yaml(&yaml_data)?;

    let filter = if category == "all" { None } else { Some(category.as_ref()) };

    let status = save.mission_status(filter);

    if status.is_empty() {
        println!("No missions found for category '{}'.", category);
        return Ok(());
    }

    let max_name = status
        .iter()
        .map(|e| short_name(&e.mission_set).len())
        .max()
        .unwrap_or(20);

    // Group by category if showing all
    let mut last_category = String::new();
    for entry in &status {
        if filter.is_none() && entry.category != last_category {
            if !last_category.is_empty() {
                println!();
            }
            println!("{}:", entry.category.to_uppercase());
            last_category = entry.category.clone();
        }

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
    // Try DLC completion first, then main story progression
    let changes = bl4::save::campaign::plan_dlc_completion(mission)
        .or_else(|| bl4::save::campaign::plan_campaign_progress(mission));

    let changes = match changes {
        Some(c) => c,
        None => {
            bail!(
                "Unknown mission: '{}'\n\nUse 'bl4 save <file> missions list' to see available missions.\n\
                Short names like 'grasslands1', 'cowbell', 'cello' are accepted.",
                mission
            );
        }
    };

    let all_completed = changes.completed_sets.contains(&changes.active_set);

    if all_completed {
        println!("DLC will be marked as completed:");
    } else {
        println!(
            "Mission progress will be set to: {}",
            short_name(&changes.active_set)
        );
    }
    println!();

    if !changes.completed_sets.is_empty() {
        println!(
            "The following {} mission set(s) will be marked completed:",
            changes.completed_sets.len()
        );
        for set_name in &changes.completed_sets {
            println!("  [x] {}", short_name(set_name));
        }
        println!();
    }

    if !all_completed {
        println!(
            "Active mission: {} ({})",
            changes.active_mission,
            short_name(&changes.active_set)
        );
        println!();
    }

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

    println!("Mission progress updated.");

    Ok(())
}

fn short_name(set_name: &str) -> &str {
    set_name
        .strip_prefix("missionset_main_")
        .or_else(|| set_name.strip_prefix("missionset_dlc_"))
        .or_else(|| set_name.strip_prefix("missionset_"))
        .unwrap_or(set_name)
}
