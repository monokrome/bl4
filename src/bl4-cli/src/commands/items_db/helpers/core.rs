//! Core helper functions for items database operations

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Get a field value from an item, returning the base item value
pub fn get_item_field_value(item: &bl4_idb::Item, field: &str) -> String {
    match field {
        "serial" => item.serial.clone(),
        "name" => item.name.clone().unwrap_or_default(),
        "prefix" => item.prefix.clone().unwrap_or_default(),
        "manufacturer" => item.manufacturer.clone().unwrap_or_default(),
        "weapon_type" => item.weapon_type.clone().unwrap_or_default(),
        "item_type" => item.item_type.clone().unwrap_or_default(),
        "rarity" => item.rarity.clone().unwrap_or_default(),
        "level" => item.level.map(|l| l.to_string()).unwrap_or_default(),
        "element" => item.element.clone().unwrap_or_default(),
        "status" => item.verification_status.to_string(),
        "legal" => if item.legal { "true" } else { "false" }.to_string(),
        "source" => item.source.clone().unwrap_or_default(),
        "created_at" => item.created_at.clone(),
        _ => String::new(),
    }
}

/// Get a field value with override from item_values table
pub fn get_item_field_value_with_override(
    item: &bl4_idb::Item,
    field: &str,
    overrides: Option<&HashMap<String, String>>,
) -> String {
    if let Some(ovr) = overrides {
        if let Some(val) = ovr.get(field) {
            return val.clone();
        }
    }
    get_item_field_value(item, field)
}

/// Filter item fields with overrides for JSON output
pub fn filter_item_fields_with_overrides(
    item: &bl4_idb::Item,
    fields: &[&str],
    overrides: Option<&HashMap<String, String>>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in fields {
        let value = get_item_field_value_with_override(item, field, overrides);
        obj.insert(
            (*field).to_string(),
            if value.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(value)
            },
        );
    }
    serde_json::Value::Object(obj)
}

/// Escape a string for CSV output
pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Get display width for a field
pub fn field_display_width(field: &str) -> usize {
    match field {
        "serial" => 35,
        other => other
            .parse::<bl4_idb::ItemField>()
            .map(|f| f.display_width())
            .unwrap_or(15),
    }
}

/// Extract item serials from a YAML value recursively
pub fn extract_serials_from_yaml(value: &serde_yaml::Value, serials: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) => {
            if s.starts_with("@Ug") && s.len() >= 10 {
                serials.push(s.clone());
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    if key == "serial" {
                        if let serde_yaml::Value::String(s) = v {
                            if s.starts_with("@Ug") {
                                serials.push(s.clone());
                            }
                        }
                    }
                }
                extract_serials_from_yaml(v, serials);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for v in seq {
                extract_serials_from_yaml(v, serials);
            }
        }
        _ => {}
    }
}

/// Merge databases (legacy function for tier/notes migration)
pub fn merge_databases(source: &Path, dest: &Path) -> Result<()> {
    use rusqlite::{params, Connection};

    println!("Merging {} -> {}", source.display(), dest.display());

    let src_conn = Connection::open(source)?;
    let dest_conn = Connection::open(dest)?;

    let _ = dest_conn.execute("ALTER TABLE weapons ADD COLUMN tier TEXT", []);

    let mut stmt = src_conn.prepare(
        "SELECT id, name, tier, notes FROM weapons WHERE name IS NOT NULL OR tier IS NOT NULL OR notes IS NOT NULL"
    )?;

    #[allow(clippy::type_complexity)]
    let items: Vec<(i64, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    println!("Found {} items with user data to merge", items.len());

    let mut updated = 0;
    for (id, name, tier, notes) in &items {
        if let Some(name) = name {
            if !name.is_empty() {
                dest_conn.execute(
                    "UPDATE weapons SET name = ?1 WHERE id = ?2",
                    params![name, id],
                )?;
            }
        }
        if let Some(tier) = tier {
            dest_conn.execute(
                "UPDATE weapons SET tier = ?1 WHERE id = ?2",
                params![tier, id],
            )?;
        }
        if let Some(notes) = notes {
            if !notes.is_empty() {
                dest_conn.execute(
                    "UPDATE weapons SET notes = ?1 WHERE id = ?2",
                    params![notes, id],
                )?;
            }
        }
        updated += 1;
    }

    println!("Merged {} items", updated);
    let count: i64 = dest_conn.query_row(
        "SELECT COUNT(*) FROM weapons WHERE tier IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    println!("Destination now has {} tiered items", count);

    Ok(())
}
