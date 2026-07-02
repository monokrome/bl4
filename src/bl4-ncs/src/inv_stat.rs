//! Weapon base stats extraction from inv_stat NCS files
//!
//! Parses `inv_stat*.bin` files to produce `weapon_base_stats.tsv`:
//! a mapping of weapon class → stat → data table lookup coordinates.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use crate::data_table::extract::strip_guid_suffix;
use crate::document::Value;
use crate::{decompress_ncs, is_ncs, parse_ncs_binary_from_reader};

/// One row in weapon_base_stats.tsv
#[derive(Debug, Clone)]
pub struct WeaponStatRow {
    pub weapon_class: String,
    pub parent: String,
    pub stat: String,
    pub data_table: String,
    pub row: String,
    pub column: String,
}

/// Extract the table name from a `gbx_ue_data_table'Name'` reference.
fn extract_table_name(datatable_ref: &str) -> &str {
    if let Some(start) = datatable_ref.find('\'') {
        if let Some(end) = datatable_ref[start + 1..].find('\'') {
            return &datatable_ref[start + 1..][..end];
        }
    }
    datatable_ref
}

/// Extract the parent name from a `inv_stat'Name'` reference.
fn extract_parent_name(parent_ref: &str) -> &str {
    if let Some(start) = parent_ref.find('\'') {
        if let Some(end) = parent_ref[start + 1..].find('\'') {
            return &parent_ref[start + 1..][..end];
        }
    }
    parent_ref
}

/// Extract stat rows from a single inv_stat NCS entry.
fn extract_entry_stats(weapon_class: &str, value: &Value) -> Vec<WeaponStatRow> {
    let map = match value {
        Value::Map(m) => m,
        _ => return Vec::new(),
    };

    // Determine parent: use `parent` field if present, otherwise own key
    let parent = match map.get("parent") {
        Some(Value::Leaf(s)) => extract_parent_name(s).to_string(),
        _ => weapon_class.to_string(),
    };

    // Get the attributes array
    let attributes = match map.get("attributes") {
        Some(Value::Map(attr_map)) => match attr_map.get("attribute") {
            Some(Value::Array(arr)) => arr,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    let mut rows = Vec::new();

    for attr_entry in attributes {
        let attr_map = match attr_entry {
            Value::Map(m) => m,
            _ => continue,
        };

        // Each attribute is a map with a single key = stat name
        for (stat_name, stat_value) in attr_map {
            let stat_map = match stat_value {
                Value::Map(m) => m,
                _ => continue,
            };

            // Navigate to datatablevalue
            let scalar = match stat_map.get("stattoattributemodifierscalar") {
                Some(Value::Map(m)) => m,
                _ => continue,
            };
            let dtv = match scalar.get("datatablevalue") {
                Some(Value::Map(m)) => m,
                _ => continue,
            };

            let data_table = match dtv.get("datatable") {
                Some(Value::Leaf(s)) => extract_table_name(s).to_string(),
                _ => continue,
            };
            let row = match dtv.get("rowname") {
                Some(Value::Leaf(s)) => s.clone(),
                _ => continue,
            };
            let column = match dtv.get("columnname") {
                Some(Value::Leaf(s)) => {
                    let cleaned = strip_guid_suffix(s);
                    if cleaned.is_empty() {
                        "Default".to_string()
                    } else {
                        cleaned.to_string()
                    }
                }
                _ => "Default".to_string(),
            };

            rows.push(WeaponStatRow {
                weapon_class: weapon_class.to_string(),
                parent: parent.clone(),
                stat: stat_name.clone(),
                data_table,
                row,
                column,
            });
        }
    }

    rows
}

/// Extract weapon base stats from a single NCS binary (inv_stat*.bin).
pub fn extract_from_binary(data: &[u8]) -> Vec<WeaponStatRow> {
    let decompressed = if is_ncs(data) {
        match decompress_ncs(data) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        }
    } else {
        data.to_vec()
    };

    let doc = match parse_ncs_binary_from_reader(&mut Cursor::new(&decompressed)) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let table = match doc.tables.get("inv_stat") {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut rows = Vec::new();

    for record in &table.records {
        for entry in &record.entries {
            if entry.key == "inv_stats_default" {
                continue;
            }
            rows.extend(extract_entry_stats(&entry.key, &entry.value));
        }
    }

    rows
}

/// Sort key for filename-based ordering of inv_stat*.bin files.
fn inv_stat_file_order(name: &str) -> u32 {
    let stem = name.strip_suffix(".bin").unwrap_or(name);
    let prefix = "inv_stat";
    if stem == prefix {
        return 0;
    }
    let suffix = &stem[prefix.len()..];
    let num_str = suffix.strip_prefix('_').unwrap_or(suffix);
    num_str.parse::<u32>().unwrap_or(u32::MAX)
}

/// Extract weapon base stats from an NCS directory.
///
/// Scans for all `inv_stat*.bin` files, parses each in suffix order,
/// and merges entries. Later files override earlier files' entries.
pub fn extract_from_directory<P: AsRef<Path>>(dir: P) -> Vec<WeaponStatRow> {
    let dir = dir.as_ref();

    let mut files: Vec<(u32, std::path::PathBuf)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("inv_stat") && name.ends_with(".bin") {
                let order = inv_stat_file_order(name);
                files.push((order, path));
            }
        }
    }

    files.sort_by_key(|(order, _)| *order);

    let mut merged: HashMap<String, WeaponStatRow> = HashMap::new();

    for (_, path) in &files {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let rows = extract_from_binary(&data);
        for row in rows {
            // Key: weapon_class + stat + data_table + row + column ensures
            // uniqueness per stat mapping. Later files override earlier ones.
            let key = format!(
                "{}\t{}\t{}\t{}",
                row.weapon_class, row.stat, row.data_table, row.row
            );
            merged.insert(key, row);
        }
    }

    let mut result: Vec<WeaponStatRow> = merged.into_values().collect();
    result.sort_by(|a, b| {
        a.weapon_class
            .cmp(&b.weapon_class)
            .then_with(|| a.stat.cmp(&b.stat))
            .then_with(|| a.data_table.cmp(&b.data_table))
            .then_with(|| a.row.cmp(&b.row))
    });
    result
}

/// Write weapon base stats rows to a TSV file.
pub fn write_tsv<P: AsRef<Path>>(rows: &[WeaponStatRow], path: P) -> Result<(), std::io::Error> {
    use std::fs;
    use std::io::Write;

    let mut out = fs::File::create(path.as_ref())?;
    writeln!(out, "weapon_class\tparent\tstat\tdata_table\trow\tcolumn")?;
    for row in rows {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}",
            row.weapon_class, row.parent, row.stat, row.data_table, row.row, row.column
        )?;
    }
    Ok(())
}
