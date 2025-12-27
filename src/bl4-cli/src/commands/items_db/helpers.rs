//! Helper functions for items database operations

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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_item() -> bl4_idb::Item {
        bl4_idb::Item {
            serial: "@Ug12345678901234567890".to_string(),
            name: Some("Test Weapon".to_string()),
            prefix: Some("Amplified".to_string()),
            manufacturer: Some("Hyperion".to_string()),
            weapon_type: Some("Pistol".to_string()),
            item_type: Some("Weapon".to_string()),
            rarity: Some("Legendary".to_string()),
            level: Some(50),
            element: Some("Fire".to_string()),
            source: Some("ingame".to_string()),
            legal: true,
            verification_status: bl4_idb::VerificationStatus::Verified,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            ..Default::default()
        }
    }

    fn make_minimal_item() -> bl4_idb::Item {
        bl4_idb::Item {
            serial: "@Ug00000000".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            ..Default::default()
        }
    }

    // Tests for get_item_field_value
    mod get_item_field_value_tests {
        use super::*;

        #[test]
        fn returns_serial() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "serial"), "@Ug12345678901234567890");
        }

        #[test]
        fn returns_name() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "name"), "Test Weapon");
        }

        #[test]
        fn returns_prefix() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "prefix"), "Amplified");
        }

        #[test]
        fn returns_manufacturer() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "manufacturer"), "Hyperion");
        }

        #[test]
        fn returns_weapon_type() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "weapon_type"), "Pistol");
        }

        #[test]
        fn returns_item_type() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "item_type"), "Weapon");
        }

        #[test]
        fn returns_rarity() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "rarity"), "Legendary");
        }

        #[test]
        fn returns_level_as_string() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "level"), "50");
        }

        #[test]
        fn returns_element() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "element"), "Fire");
        }

        #[test]
        fn returns_status() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "status"), "verified");
        }

        #[test]
        fn returns_legal_true() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "legal"), "true");
        }

        #[test]
        fn returns_legal_false() {
            let item = make_minimal_item();
            assert_eq!(get_item_field_value(&item, "legal"), "false");
        }

        #[test]
        fn returns_source() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "source"), "ingame");
        }

        #[test]
        fn returns_created_at() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "created_at"), "2024-01-01T00:00:00Z");
        }

        #[test]
        fn returns_empty_for_unknown_field() {
            let item = make_test_item();
            assert_eq!(get_item_field_value(&item, "unknown_field"), "");
        }

        #[test]
        fn returns_empty_for_none_values() {
            let item = make_minimal_item();
            assert_eq!(get_item_field_value(&item, "name"), "");
            assert_eq!(get_item_field_value(&item, "prefix"), "");
            assert_eq!(get_item_field_value(&item, "manufacturer"), "");
            assert_eq!(get_item_field_value(&item, "level"), "");
        }
    }

    // Tests for get_item_field_value_with_override
    mod get_item_field_value_with_override_tests {
        use super::*;

        #[test]
        fn returns_override_when_present() {
            let item = make_test_item();
            let mut overrides = HashMap::new();
            overrides.insert("name".to_string(), "Overridden Name".to_string());

            let result = get_item_field_value_with_override(&item, "name", Some(&overrides));
            assert_eq!(result, "Overridden Name");
        }

        #[test]
        fn returns_item_value_when_no_override() {
            let item = make_test_item();
            let overrides = HashMap::new();

            let result = get_item_field_value_with_override(&item, "name", Some(&overrides));
            assert_eq!(result, "Test Weapon");
        }

        #[test]
        fn returns_item_value_when_overrides_none() {
            let item = make_test_item();

            let result = get_item_field_value_with_override(&item, "name", None);
            assert_eq!(result, "Test Weapon");
        }

        #[test]
        fn override_takes_precedence_for_all_fields() {
            let item = make_test_item();
            let mut overrides = HashMap::new();
            overrides.insert("manufacturer".to_string(), "Custom Mfg".to_string());
            overrides.insert("level".to_string(), "99".to_string());

            assert_eq!(
                get_item_field_value_with_override(&item, "manufacturer", Some(&overrides)),
                "Custom Mfg"
            );
            assert_eq!(
                get_item_field_value_with_override(&item, "level", Some(&overrides)),
                "99"
            );
            // Non-overridden field
            assert_eq!(
                get_item_field_value_with_override(&item, "rarity", Some(&overrides)),
                "Legendary"
            );
        }
    }

    // Tests for filter_item_fields_with_overrides
    mod filter_item_fields_with_overrides_tests {
        use super::*;

        #[test]
        fn filters_to_requested_fields() {
            let item = make_test_item();
            let fields = vec!["serial", "name"];

            let result = filter_item_fields_with_overrides(&item, &fields, None);

            assert!(result.get("serial").is_some());
            assert!(result.get("name").is_some());
            assert!(result.get("manufacturer").is_none());
        }

        #[test]
        fn applies_overrides() {
            let item = make_test_item();
            let fields = vec!["name", "manufacturer"];
            let mut overrides = HashMap::new();
            overrides.insert("name".to_string(), "Override".to_string());

            let result = filter_item_fields_with_overrides(&item, &fields, Some(&overrides));

            assert_eq!(result["name"], "Override");
            assert_eq!(result["manufacturer"], "Hyperion");
        }

        #[test]
        fn empty_values_become_null() {
            let item = make_minimal_item();
            let fields = vec!["name", "prefix"];

            let result = filter_item_fields_with_overrides(&item, &fields, None);

            assert!(result["name"].is_null());
            assert!(result["prefix"].is_null());
        }

        #[test]
        fn empty_fields_list_returns_empty_object() {
            let item = make_test_item();
            let fields: Vec<&str> = vec![];

            let result = filter_item_fields_with_overrides(&item, &fields, None);

            assert_eq!(result, serde_json::json!({}));
        }
    }

    // Tests for escape_csv
    mod escape_csv_tests {
        use super::*;

        #[test]
        fn no_escaping_for_simple_string() {
            assert_eq!(escape_csv("hello"), "hello");
        }

        #[test]
        fn escapes_comma() {
            assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
        }

        #[test]
        fn escapes_double_quotes() {
            assert_eq!(escape_csv("say \"hello\""), "\"say \"\"hello\"\"\"");
        }

        #[test]
        fn escapes_newline() {
            assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
        }

        #[test]
        fn escapes_combination() {
            assert_eq!(escape_csv("a,b\"c\nd"), "\"a,b\"\"c\nd\"");
        }

        #[test]
        fn empty_string_unchanged() {
            assert_eq!(escape_csv(""), "");
        }

        #[test]
        fn string_with_spaces_unchanged() {
            assert_eq!(escape_csv("hello world"), "hello world");
        }

        #[test]
        fn string_with_special_chars_unchanged() {
            assert_eq!(escape_csv("hello!@#$%^&*()"), "hello!@#$%^&*()");
        }
    }

    // Tests for field_display_width
    mod field_display_width_tests {
        use super::*;

        #[test]
        fn serial_has_fixed_width() {
            assert_eq!(field_display_width("serial"), 35);
        }

        #[test]
        fn known_fields_have_widths() {
            // These should parse to ItemField and return display_width()
            let width = field_display_width("name");
            assert!(width > 0);
        }

        #[test]
        fn unknown_field_defaults_to_15() {
            assert_eq!(field_display_width("unknown_xyz"), 15);
        }
    }

    // Tests for extract_serials_from_yaml
    mod extract_serials_from_yaml_tests {
        use super::*;

        #[test]
        fn extracts_from_string_value() {
            let yaml = serde_yaml::Value::String("@Ug1234567890".to_string());
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert_eq!(serials, vec!["@Ug1234567890"]);
        }

        #[test]
        fn ignores_short_strings() {
            let yaml = serde_yaml::Value::String("@Ug123".to_string()); // Only 7 chars
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.is_empty());
        }

        #[test]
        fn ignores_non_ug_strings() {
            let yaml = serde_yaml::Value::String("not_a_serial_1234567890".to_string());
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.is_empty());
        }

        #[test]
        fn extracts_from_serial_key_in_mapping() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"
                serial: "@Ug9876543210"
                name: "Test Item"
                "#,
            )
            .unwrap();
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            // Note: may contain duplicates (found via key and via recursion)
            assert!(serials.contains(&"@Ug9876543210".to_string()));
        }

        #[test]
        fn extracts_from_nested_sequence() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"
                items:
                  - serial: "@UgAAAAAAAAA"
                  - serial: "@UgBBBBBBBBB"
                "#,
            )
            .unwrap();
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.contains(&"@UgAAAAAAAAA".to_string()));
            assert!(serials.contains(&"@UgBBBBBBBBB".to_string()));
        }

        #[test]
        fn extracts_inline_serials_from_values() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"
                data: "@UgINLINEVALUE"
                "#,
            )
            .unwrap();
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.contains(&"@UgINLINEVALUE".to_string()));
        }

        #[test]
        fn handles_empty_yaml() {
            let yaml = serde_yaml::Value::Null;
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.is_empty());
        }

        #[test]
        fn handles_number_values() {
            let yaml = serde_yaml::Value::Number(42.into());
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            assert!(serials.is_empty());
        }

        #[test]
        fn handles_deeply_nested_structure() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"
                level1:
                  level2:
                    level3:
                      items:
                        - serial: "@UgDEEPNESTED"
                "#,
            )
            .unwrap();
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            // May contain duplicates, but serial is found
            assert!(serials.contains(&"@UgDEEPNESTED".to_string()));
        }

        #[test]
        fn extracts_multiple_from_complex_structure() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"
                equipped:
                  - serial: "@UgEQUIPPED01"
                  - serial: "@UgEQUIPPED02"
                backpack:
                  - serial: "@UgBACKPACK1"
                bank:
                  items:
                    - "@UgBANKITEM01"
                "#,
            )
            .unwrap();
            let mut serials = Vec::new();

            extract_serials_from_yaml(&yaml, &mut serials);

            // Deduplicate to count unique serials
            serials.sort();
            serials.dedup();
            assert_eq!(serials.len(), 4);
        }
    }
}
