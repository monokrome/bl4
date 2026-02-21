//! Data table extraction from gbx_ue_data_table NCS files

use super::types::{DataTable, DataTableManifest, DataTableRow};
use crate::document::Value;
use std::collections::HashMap;

/// Strip GUID suffix from a field name.
///
/// Fields like `comment_109_f23a09ff4ca5cbdb25d9f5be50fb1941` become `comment`.
/// Fields like `cost_normal` or `type` are returned unchanged.
///
/// The GUID pattern is: `{name}_{digits}_{32 hex chars}`.
fn strip_guid_suffix(field: &str) -> &str {
    // Find the last underscore-separated segment
    // GUID suffix: _NN_<32 hex chars> at the end
    let bytes = field.as_bytes();
    let len = bytes.len();

    // Minimum GUID suffix: _N_<32 hex> = 1 + 1 + 1 + 32 = 35 chars
    if len < 35 {
        return field;
    }

    // Check if last 32 chars are hex
    let hex_start = len - 32;
    if !bytes[hex_start..].iter().all(|&b| b.is_ascii_hexdigit()) {
        return field;
    }

    // Check underscore before hex block
    if bytes[hex_start - 1] != b'_' {
        return field;
    }

    // Find the underscore before the digit sequence
    let before_digits = hex_start - 1;
    let mut digit_start = before_digits;
    while digit_start > 0 && bytes[digit_start - 1].is_ascii_digit() {
        digit_start -= 1;
    }

    // Must have at least one digit and an underscore before it
    if digit_start == before_digits || digit_start == 0 || bytes[digit_start - 1] != b'_' {
        return field;
    }

    &field[..digit_start - 1]
}

/// Extract a single row from a Value::Map
fn extract_row(value: &Value) -> Option<DataTableRow> {
    let map = match value {
        Value::Map(m) => m,
        _ => return None,
    };

    let row_name = match map.get("row_name") {
        Some(Value::Leaf(s)) => s.clone(),
        _ => return None,
    };

    let mut fields = HashMap::new();

    if let Some(Value::Map(row_value)) = map.get("row_value") {
        for (key, val) in row_value {
            let clean_key = strip_guid_suffix(key).to_string();
            if let Value::Leaf(s) = val {
                fields.insert(clean_key, s.clone());
            }
        }
    }

    Some(DataTableRow { row_name, fields })
}

/// Extract a DataTable from a single NCS entry
fn extract_table(key: &str, value: &Value) -> Option<DataTable> {
    let map = match value {
        Value::Map(m) => m,
        _ => return None,
    };

    let name = match map.get("gbx_ue_data_table") {
        Some(Value::Leaf(s)) => s.clone(),
        _ => key.to_string(),
    };

    let row_struct = match map.get("row_struct") {
        Some(Value::Leaf(s)) => s.clone(),
        _ => String::new(),
    };

    let mut rows = Vec::new();
    if let Some(Value::Array(data)) = map.get("data") {
        for row_value in data {
            if let Some(row) = extract_row(row_value) {
                rows.push(row);
            }
        }
    }

    Some(DataTable {
        key: key.to_string(),
        name,
        row_struct,
        rows,
    })
}

/// Parse a gbx_ue_data_table NCS binary and extract all data tables.
pub fn extract_data_tables(data: &[u8]) -> Option<DataTableManifest> {
    let doc = crate::parse::parse(data)?;

    let mut tables = HashMap::new();

    for table in doc.tables.values() {
        for record in &table.records {
            for entry in &record.entries {
                if let Some(dt) = extract_table(&entry.key, &entry.value) {
                    tables.insert(dt.key.clone(), dt);
                }
            }
        }
    }

    Some(DataTableManifest { tables })
}

/// Extract data tables from an NCS directory.
///
/// Scans for `gbx_ue_data_table.bin` (or `gbx_ue_data_table0.bin`) and parses it.
pub fn extract_data_tables_from_dir<P: AsRef<std::path::Path>>(
    ncs_dir: P,
) -> Result<DataTableManifest, std::io::Error> {
    let dir = ncs_dir.as_ref();

    let candidates = [
        "gbx_ue_data_table.bin",
        "gbx_ue_data_table0.bin",
        "Nexus-Data-gbx_ue_data_table0.bin",
    ];

    for name in &candidates {
        let path = dir.join(name);
        if path.exists() {
            let data = std::fs::read(&path)?;
            return extract_data_tables(&data).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse data tables from {}", path.display()),
                )
            });
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No gbx_ue_data_table file found in NCS directory",
    ))
}

/// Generate a summary TSV of all data tables.
pub fn tables_summary_tsv(manifest: &DataTableManifest) -> String {
    let mut tsv = String::from("key\tname\trow_struct\trow_count\n");

    let mut keys: Vec<&str> = manifest.tables.keys().map(|s| s.as_str()).collect();
    keys.sort();

    for key in keys {
        let table = &manifest.tables[key];
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            table.key,
            table.name,
            table.row_struct,
            table.rows.len()
        ));
    }

    tsv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_guid_suffix_with_guid() {
        assert_eq!(
            strip_guid_suffix("comment_109_f23a09ff4ca5cbdb25d9f5be50fb1941"),
            "comment"
        );
        assert_eq!(
            strip_guid_suffix("duration_119_c67ba50343734dff810fc1b1980cc75f"),
            "duration"
        );
        assert_eq!(
            strip_guid_suffix("maxvalue_110_2d75e0c54d3035bfceb34fa1ef7a9743"),
            "maxvalue"
        );
        assert_eq!(
            strip_guid_suffix("fire_52_4d6e5a8840f57dbd840197b3cb05686d"),
            "fire"
        );
        assert_eq!(
            strip_guid_suffix("sonic_59_98d060bb4ebf130785b55e974bef3ed1"),
            "sonic"
        );
    }

    #[test]
    fn test_strip_guid_suffix_without_guid() {
        assert_eq!(strip_guid_suffix("cost_normal"), "cost_normal");
        assert_eq!(strip_guid_suffix("type"), "type");
        assert_eq!(strip_guid_suffix("comment"), "comment");
        assert_eq!(
            strip_guid_suffix("damagemultiplier_levelbased"),
            "damagemultiplier_levelbased"
        );
        assert_eq!(
            strip_guid_suffix("healthmultiplier_01"),
            "healthmultiplier_01"
        );
    }

    #[test]
    fn test_strip_guid_suffix_edge_cases() {
        assert_eq!(strip_guid_suffix(""), "");
        assert_eq!(strip_guid_suffix("a"), "a");
        assert_eq!(strip_guid_suffix("_"), "_");
    }

    #[test]
    fn test_extract_row_basic() {
        let mut row_value = HashMap::new();
        row_value.insert(
            "fire_52_4d6e5a8840f57dbd840197b3cb05686d".to_string(),
            Value::Leaf("0.800000".to_string()),
        );
        row_value.insert(
            "shock_56_e6f748ac40e5205baa7c39b0a887cbf3".to_string(),
            Value::Leaf("0.800000".to_string()),
        );

        let mut map = HashMap::new();
        map.insert("row_name".to_string(), Value::Leaf("WeaponDamageScale".to_string()));
        map.insert("row_value".to_string(), Value::Map(row_value));

        let row = extract_row(&Value::Map(map)).unwrap();
        assert_eq!(row.row_name, "WeaponDamageScale");
        assert_eq!(row.fields.get("fire"), Some(&"0.800000".to_string()));
        assert_eq!(row.fields.get("shock"), Some(&"0.800000".to_string()));
    }

    #[test]
    fn test_extract_row_no_row_value() {
        let mut map = HashMap::new();
        map.insert("row_name".to_string(), Value::Leaf("Pistol".to_string()));

        let row = extract_row(&Value::Map(map)).unwrap();
        assert_eq!(row.row_name, "Pistol");
        assert!(row.fields.is_empty());
    }

    #[test]
    fn test_extract_table() {
        let mut data_arr = Vec::new();
        let mut row_map = HashMap::new();
        row_map.insert("row_name".to_string(), Value::Leaf("Row1".to_string()));
        let mut rv = HashMap::new();
        rv.insert("cost_normal".to_string(), Value::Leaf("600".to_string()));
        row_map.insert("row_value".to_string(), Value::Map(rv));
        data_arr.push(Value::Map(row_map));

        let mut entry_map = HashMap::new();
        entry_map.insert(
            "gbx_ue_data_table".to_string(),
            Value::Leaf("My_Table".to_string()),
        );
        entry_map.insert(
            "row_struct".to_string(),
            Value::Leaf("Asset'/Game/Test.Test'".to_string()),
        );
        entry_map.insert("data".to_string(), Value::Array(data_arr));

        let table = extract_table("my_table", &Value::Map(entry_map)).unwrap();
        assert_eq!(table.key, "my_table");
        assert_eq!(table.name, "My_Table");
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].fields.get("cost_normal"), Some(&"600".to_string()));
    }

    #[test]
    fn test_data_table_manifest_accessors() {
        let mut tables = HashMap::new();
        tables.insert(
            "test_table".to_string(),
            DataTable {
                key: "test_table".to_string(),
                name: "Test_Table".to_string(),
                row_struct: String::new(),
                rows: vec![
                    DataTableRow {
                        row_name: "Row1".to_string(),
                        fields: HashMap::new(),
                    },
                    DataTableRow {
                        row_name: "Row2".to_string(),
                        fields: HashMap::new(),
                    },
                ],
            },
        );

        let manifest = DataTableManifest { tables };
        assert_eq!(manifest.len(), 1);
        assert!(!manifest.is_empty());
        assert_eq!(manifest.total_rows(), 2);
        assert!(manifest.get("test_table").is_some());
        assert!(manifest.get("TEST_TABLE").is_some());
        assert_eq!(manifest.keys(), vec!["test_table"]);
    }
}
