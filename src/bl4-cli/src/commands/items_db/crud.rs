//! Basic CRUD operations for items database

use anyhow::Result;
use bl4_idb::{AttachmentsRepository, ItemsRepository};
use std::path::Path;

use super::helpers::{
    escape_csv, field_display_width, filter_item_fields_with_overrides,
    get_item_field_value_with_override,
};
use crate::OutputFormat;

/// Handle `idb init`
pub fn init(db: &Path) -> Result<()> {
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    println!("Your database is ready at {}", db.display());
    Ok(())
}

/// Handle `idb stats`
pub fn stats(db: &Path) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    let stats = wdb.stats()?;
    println!("Items Database Statistics");
    println!("  Items:       {}", stats.item_count);
    println!("  Parts:       {}", stats.part_count);
    println!("  Attachments: {}", stats.attachment_count);
    Ok(())
}

/// Handle `idb salt`
pub fn salt(db: &Path) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    let salt = wdb.get_or_create_salt()?;
    println!("{}", salt);
    Ok(())
}

/// Handle `idb add`
pub fn add(
    db: &Path,
    serial: &str,
    name: Option<String>,
    prefix: Option<String>,
    manufacturer: Option<String>,
    weapon_type: Option<String>,
    rarity: Option<String>,
    level: Option<i32>,
    element: Option<String>,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;
    wdb.add_item(serial)?;

    if name.is_some()
        || prefix.is_some()
        || manufacturer.is_some()
        || weapon_type.is_some()
        || rarity.is_some()
        || level.is_some()
        || element.is_some()
    {
        let update = bl4_idb::ItemUpdate {
            name,
            prefix,
            manufacturer,
            weapon_type,
            rarity,
            level,
            element,
            ..Default::default()
        };
        wdb.update_item(serial, &update)?;
    }

    println!("Added item: {}", serial);
    Ok(())
}

/// Handle `idb show`
pub fn show(db: &Path, serial: &str) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    let weapon = wdb.get_item(serial)?;

    if let Some(w) = weapon {
        println!("Serial:       {}", w.serial);
        println!("Name:         {}", w.name.as_deref().unwrap_or("-"));
        println!("Prefix:       {}", w.prefix.as_deref().unwrap_or("-"));
        println!("Manufacturer: {}", w.manufacturer.as_deref().unwrap_or("-"));
        println!("Type:         {}", w.weapon_type.as_deref().unwrap_or("-"));
        println!("Rarity:       {}", w.rarity.as_deref().unwrap_or("-"));
        println!(
            "Level:        {}",
            w.level.map(|l| l.to_string()).unwrap_or("-".to_string())
        );
        println!("Element:      {}", w.element.as_deref().unwrap_or("-"));
        println!("\n--- Metadata ---");
        println!("Source:       {}", w.source.as_deref().unwrap_or("-"));
        println!("Legal:        {}", if w.legal { "yes" } else { "no" });
        println!("Status:       {}", w.verification_status);
        println!("Created:      {}", w.created_at);

        let parts = wdb.get_parts(&w.serial)?;
        if !parts.is_empty() {
            println!("\nParts:");
            for p in parts {
                println!(
                    "  {} - {} ({})",
                    p.slot,
                    p.manufacturer.as_deref().unwrap_or("-"),
                    p.effect.as_deref().unwrap_or("-")
                );
            }
        }

        let attachments = wdb.get_attachments(&w.serial)?;
        if !attachments.is_empty() {
            println!("\nAttachments:");
            for a in attachments {
                println!("  {} ({}, {})", a.name, a.view, a.mime_type);
            }
        }
    } else {
        println!("Item not found: {}", serial);
    }
    Ok(())
}

/// Handle `idb list`
pub fn list(
    db: &Path,
    manufacturer: Option<String>,
    weapon_type: Option<String>,
    element: Option<String>,
    rarity: Option<String>,
    format: OutputFormat,
    fields: Option<Vec<String>>,
) -> Result<()> {
    let wdb = bl4_idb::SqliteDb::open(db)?;
    wdb.init()?;

    let filter = bl4_idb::ItemFilter {
        manufacturer,
        weapon_type,
        element,
        rarity,
        ..Default::default()
    };
    let items = wdb.list_items(&filter)?;

    if items.is_empty() {
        println!("No items found");
        return Ok(());
    }

    let all_best_values = wdb.get_all_items_best_values()?;

    let default_fields = vec![
        "serial",
        "manufacturer",
        "name",
        "weapon_type",
        "level",
        "element",
    ];
    let field_list: Vec<&str> = fields
        .as_ref()
        .map(|f| f.iter().map(|s| s.as_str()).collect())
        .unwrap_or_else(|| default_fields);

    match format {
        OutputFormat::Json => {
            let filtered: Vec<serde_json::Value> = items
                .iter()
                .map(|item| {
                    let overrides = all_best_values.get(&item.serial);
                    filter_item_fields_with_overrides(item, &field_list, overrides)
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        }
        OutputFormat::Csv => {
            println!("{}", field_list.join(","));
            for item in &items {
                let overrides = all_best_values.get(&item.serial);
                let values: Vec<String> = field_list
                    .iter()
                    .map(|f| get_item_field_value_with_override(item, f, overrides))
                    .map(|v| escape_csv(&v))
                    .collect();
                println!("{}", values.join(","));
            }
        }
        OutputFormat::Table => {
            let col_widths: Vec<usize> =
                field_list.iter().map(|f| field_display_width(f)).collect();

            let header: String = field_list
                .iter()
                .zip(&col_widths)
                .map(|(f, w)| format!("{:<width$}", f, width = w))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{}", header);
            println!("{}", "-".repeat(header.len()));

            for item in &items {
                let overrides = all_best_values.get(&item.serial);
                let row: String = field_list
                    .iter()
                    .zip(&col_widths)
                    .map(|(f, w)| {
                        let val = get_item_field_value_with_override(item, f, overrides);
                        let truncated = if val.len() > *w {
                            format!("{}…", &val[..*w - 1])
                        } else {
                            val
                        };
                        format!("{:<width$}", truncated, width = w)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{}", row);
            }
        }
    }
    Ok(())
}
