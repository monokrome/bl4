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

    let filter = if category == "all" {
        None
    } else {
        Some(category)
    };

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

        // Try to find the display name from the first mission in this set
        let friendly = bl4::missions::first_mission_in_set(&entry.mission_set)
            .and_then(|m| bl4::missions::display_name(&m.name));

        let suffix = match (friendly, entry.region.as_str()) {
            (Some(f), "") => format!("  ({})", f),
            (Some(f), r) => format!("  ({}, {})", f, r),
            (None, "") => String::new(),
            (None, r) => format!("  ({})", r),
        };

        println!("{} {:<width$}{}", icon, name, suffix, width = max_name + 2);
    }

    Ok(())
}

fn set(args: &SaveArgs, mission: &str, skip_confirm: bool) -> Result<()> {
    // Try individual mission first
    if let Some(m) = bl4::missions::resolve_mission_name(mission) {
        println!("Will mark mission as completed: {}", m.name);
        println!("  Set: {}", m.mission_set);
        println!();

        if !skip_confirm && !confirm()? {
            println!("Aborted.");
            return Ok(());
        }

        crate::commands::save::with_save_file(args, |save| {
            save.complete_mission(&m.name)?;
            Ok(())
        })?;

        println!("Mission completed.");
        return Ok(());
    }

    // Try DLC completion, then main story progression, then generic set completion
    let changes = bl4::save::campaign::plan_dlc_completion(mission)
        .or_else(|| bl4::save::campaign::plan_campaign_progress(mission))
        .or_else(|| plan_generic_set_completion(mission));

    let changes = match changes {
        Some(c) => c,
        None => {
            bail!(
                "Unknown mission: '{}'\n\nUse 'bl4 save <file> missions list all' to see available missions.\n\
                Accepts: mission names, set names, or DLC names (cowbell, cello, etc.)",
                mission
            );
        }
    };

    let all_completed = changes.completed_sets.contains(&changes.active_set);

    if all_completed {
        println!("Will be marked as completed:");
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

    if !skip_confirm && !confirm()? {
        println!("Aborted.");
        return Ok(());
    }

    crate::commands::save::with_save_file(args, |save| {
        save.apply_campaign_progress(&changes)?;
        Ok(())
    })?;

    println!("Mission progress updated.");

    Ok(())
}

fn confirm() -> Result<bool> {
    print!("Apply these changes? [y/N] ");
    io::stdout().flush()?;
    let stdin = io::stdin();
    let response = stdin.lock().lines().next();
    Ok(response
        .and_then(|r| r.ok())
        .map(|r| r.trim().eq_ignore_ascii_case("y"))
        .unwrap_or(false))
}

fn plan_generic_set_completion(name: &str) -> Option<bl4::save::campaign::CampaignChanges> {
    let resolved = bl4::missions::resolve_mission_set_name(name)?;
    Some(bl4::save::campaign::CampaignChanges {
        completed_sets: vec![resolved.to_string()],
        active_set: resolved.to_string(),
        active_mission: bl4::missions::mission_name_for_set(resolved),
    })
}

fn short_name(set_name: &str) -> &str {
    set_name
        .strip_prefix("missionset_main_")
        .or_else(|| set_name.strip_prefix("missionset_dlc_"))
        .or_else(|| set_name.strip_prefix("missionset_"))
        .unwrap_or(set_name)
}
