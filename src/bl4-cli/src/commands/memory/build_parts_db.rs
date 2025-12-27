//! Build parts database command handler
//!
//! Builds a parts database from a parts dump and category mappings.
//! This command doesn't require memory access - it only reads/writes JSON.

use crate::commands::parts::PartCategoriesFile;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// Result of building a parts database
pub struct BuildResult {
    pub entries_count: usize,
    pub categories_count: usize,
}

/// Build a parts database from input files
///
/// # Arguments
/// * `input` - Path to parts dump JSON
/// * `output` - Path to output database JSON
/// * `categories` - Path to category mappings JSON
pub fn handle_build_parts_db(input: &Path, output: &Path, categories: &Path) -> Result<BuildResult> {
    println!("Building parts database from {}...", input.display());
    println!("Loading categories from {}...", categories.display());

    // Load part categories from JSON file
    let known_groups = load_categories(categories)?;
    println!("Loaded {} category mappings", known_groups.len());

    // Parse parts dump
    let parts_by_prefix = parse_parts_dump(input)?;

    // Build database entries
    let db_entries = build_entries(&known_groups, &parts_by_prefix);

    // Write output
    let category_counts = write_database(output, &db_entries)?;

    println!(
        "Built parts database with {} entries across {} categories",
        db_entries.len(),
        category_counts.len()
    );
    println!("Written to: {}", output.display());

    Ok(BuildResult {
        entries_count: db_entries.len(),
        categories_count: category_counts.len(),
    })
}

/// Load and parse category mappings from JSON
fn load_categories(path: &Path) -> Result<Vec<(String, i64, String)>> {
    let categories_json =
        std::fs::read_to_string(path).context("Failed to read part categories file")?;
    let categories_file: PartCategoriesFile =
        serde_json::from_str(&categories_json).context("Failed to parse part categories JSON")?;

    Ok(categories_file
        .categories
        .into_iter()
        .map(|cat| {
            let description = build_category_description(&cat);
            (cat.prefix, cat.category, description)
        })
        .collect())
}

/// Build a description string for a category
fn build_category_description(cat: &crate::commands::parts::PartCategory) -> String {
    if let Some(wt) = &cat.weapon_type {
        if let Some(mfr) = &cat.manufacturer {
            format!("{} {}", mfr, wt)
        } else {
            wt.clone()
        }
    } else if let Some(gt) = &cat.gear_type {
        if let Some(mfr) = &cat.manufacturer {
            format!("{} {}", mfr, gt)
        } else {
            gt.clone()
        }
    } else {
        cat.prefix.clone()
    }
}

/// Parse parts dump JSON (custom line-by-line parser for the dump format)
fn parse_parts_dump(path: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let parts_json = std::fs::read_to_string(path).context("Failed to read parts dump file")?;

    let mut parts_by_prefix: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current_prefix = String::new();
    let mut in_array = false;

    for line in parts_json.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.contains("\": [") {
            if let Some(end_quote) = trimmed[1..].find('"') {
                current_prefix = trimmed[1..end_quote + 1].to_string();
                in_array = true;
                parts_by_prefix.insert(current_prefix.clone(), Vec::new());
            }
        } else if in_array && trimmed.starts_with('"') && !trimmed.contains(": [") {
            let name = trimmed
                .trim_end_matches(',')
                .trim_end_matches('"')
                .trim_start_matches('"')
                .to_string();
            if !name.is_empty() {
                if let Some(parts) = parts_by_prefix.get_mut(&current_prefix) {
                    parts.push(name);
                }
            }
        } else if trimmed == "]" || trimmed == "]," {
            in_array = false;
        }
    }

    Ok(parts_by_prefix)
}

/// Build database entries from categories and parts
fn build_entries(
    known_groups: &[(String, i64, String)],
    parts_by_prefix: &BTreeMap<String, Vec<String>>,
) -> Vec<(i64, i16, String, String)> {
    let mut db_entries: Vec<(i64, i16, String, String)> = Vec::new();

    // Add entries for known categories
    for (prefix, category, description) in known_groups {
        if let Some(parts) = parts_by_prefix.get(prefix) {
            for (idx, part_name) in parts.iter().enumerate() {
                db_entries.push((*category, idx as i16, part_name.clone(), description.clone()));
            }
        }
    }

    // Add unmapped parts with category -1
    let known_prefixes: HashSet<&str> = known_groups.iter().map(|(p, _, _)| p.as_str()).collect();

    for (prefix, parts) in parts_by_prefix {
        if !known_prefixes.contains(prefix.as_str()) {
            for (idx, part_name) in parts.iter().enumerate() {
                db_entries.push((
                    -1,
                    idx as i16,
                    part_name.clone(),
                    format!("{} (unmapped)", prefix),
                ));
            }
        }
    }

    db_entries
}

/// Write the database JSON to a file
fn write_database(
    output: &Path,
    entries: &[(i64, i16, String, String)],
) -> Result<BTreeMap<i64, (usize, String)>> {
    let mut json = String::from("{\n  \"version\": 1,\n  \"parts\": [\n");

    for (i, (category, index, name, group)) in entries.iter().enumerate() {
        let escaped_name = name.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_group = group.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!(
            "    {{\"category\": {}, \"index\": {}, \"name\": \"{}\", \"group\": \"{}\"}}",
            category, index, escaped_name, escaped_group
        ));
        if i < entries.len() - 1 {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ],\n  \"categories\": {\n");

    let mut category_counts: BTreeMap<i64, (usize, String)> = BTreeMap::new();
    for (category, _, _, group) in entries {
        let entry = category_counts
            .entry(*category)
            .or_insert((0, group.clone()));
        entry.0 += 1;
    }

    let cat_count = category_counts.len();
    for (i, (category, (count, name))) in category_counts.iter().enumerate() {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!(
            "    \"{}\": {{\"count\": {}, \"name\": \"{}\"}}",
            category, count, escaped
        ));
        if i < cat_count - 1 {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  }\n}\n");

    std::fs::write(output, &json)?;

    Ok(category_counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_categories(dir: &TempDir) -> std::path::PathBuf {
        let path = dir.path().join("categories.json");
        let content = r#"{
            "categories": [
                {"prefix": "JAK_PS", "category": 3, "weapon_type": "Pistol", "manufacturer": "Jakobs"},
                {"prefix": "VLA_AR", "category": 5, "weapon_type": "Assault Rifle", "manufacturer": "Vladof"}
            ]
        }"#;
        std::fs::write(&path, content).unwrap();
        path
    }

    fn create_test_parts_dump(dir: &TempDir) -> std::path::PathBuf {
        let path = dir.path().join("parts_dump.json");
        let content = r#"{
            "JAK_PS": [
                "JAK_PS.part_barrel_01",
                "JAK_PS.part_barrel_02",
                "JAK_PS.part_grip_01"
            ],
            "VLA_AR": [
                "VLA_AR.part_barrel_01",
                "VLA_AR.part_mag_01"
            ],
            "UNKNOWN": [
                "UNKNOWN.part_01"
            ]
        }"#;
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_build_category_description() {
        use crate::commands::parts::PartCategory;

        let cat = PartCategory {
            prefix: "JAK_PS".to_string(),
            category: 3,
            weapon_type: Some("Pistol".to_string()),
            manufacturer: Some("Jakobs".to_string()),
            gear_type: None,
        };
        assert_eq!(build_category_description(&cat), "Jakobs Pistol");

        let cat2 = PartCategory {
            prefix: "SHD".to_string(),
            category: 10,
            weapon_type: None,
            manufacturer: Some("Pangolin".to_string()),
            gear_type: Some("Shield".to_string()),
        };
        assert_eq!(build_category_description(&cat2), "Pangolin Shield");

        let cat3 = PartCategory {
            prefix: "XXX".to_string(),
            category: 99,
            weapon_type: None,
            manufacturer: None,
            gear_type: None,
        };
        assert_eq!(build_category_description(&cat3), "XXX");
    }

    #[test]
    fn test_parse_parts_dump() {
        let dir = TempDir::new().unwrap();
        let path = create_test_parts_dump(&dir);

        let result = parse_parts_dump(&path).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result.get("JAK_PS").map(|v| v.len()), Some(3));
        assert_eq!(result.get("VLA_AR").map(|v| v.len()), Some(2));
        assert_eq!(result.get("UNKNOWN").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_build_entries() {
        let known_groups = vec![
            ("JAK_PS".to_string(), 3i64, "Jakobs Pistol".to_string()),
            ("VLA_AR".to_string(), 5i64, "Vladof AR".to_string()),
        ];

        let mut parts_by_prefix = BTreeMap::new();
        parts_by_prefix.insert(
            "JAK_PS".to_string(),
            vec!["JAK_PS.part_01".to_string(), "JAK_PS.part_02".to_string()],
        );
        parts_by_prefix.insert("VLA_AR".to_string(), vec!["VLA_AR.part_01".to_string()]);
        parts_by_prefix.insert("UNKNOWN".to_string(), vec!["UNKNOWN.part_01".to_string()]);

        let entries = build_entries(&known_groups, &parts_by_prefix);

        // 2 JAK_PS + 1 VLA_AR + 1 UNKNOWN = 4 total
        assert_eq!(entries.len(), 4);

        // Check known category entries
        assert!(entries.iter().any(|(cat, _, name, _)| *cat == 3 && name.contains("JAK_PS")));
        assert!(entries.iter().any(|(cat, _, name, _)| *cat == 5 && name.contains("VLA_AR")));

        // Check unmapped entries have category -1
        assert!(entries.iter().any(|(cat, _, name, _)| *cat == -1 && name.contains("UNKNOWN")));
    }

    #[test]
    fn test_handle_build_parts_db() {
        let dir = TempDir::new().unwrap();
        let categories_path = create_test_categories(&dir);
        let parts_path = create_test_parts_dump(&dir);
        let output_path = dir.path().join("output.json");

        let result = handle_build_parts_db(&parts_path, &output_path, &categories_path).unwrap();

        assert!(result.entries_count > 0);
        assert!(result.categories_count > 0);
        assert!(output_path.exists());

        // Verify output is valid JSON
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("\"version\": 1"));
        assert!(content.contains("\"parts\""));
        assert!(content.contains("\"categories\""));
    }

    #[test]
    fn test_load_categories() {
        let dir = TempDir::new().unwrap();
        let path = create_test_categories(&dir);

        let result = load_categories(&path).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|(p, c, _)| p == "JAK_PS" && *c == 3));
        assert!(result.iter().any(|(p, c, _)| p == "VLA_AR" && *c == 5));
    }

    #[test]
    fn test_write_database() {
        let dir = TempDir::new().unwrap();
        let output = dir.path().join("db.json");

        let entries = vec![
            (3i64, 0i16, "part1".to_string(), "Group1".to_string()),
            (3i64, 1i16, "part2".to_string(), "Group1".to_string()),
            (5i64, 0i16, "part3".to_string(), "Group2".to_string()),
        ];

        let counts = write_database(&output, &entries).unwrap();

        assert_eq!(counts.len(), 2);
        assert_eq!(counts.get(&3).map(|(c, _)| *c), Some(2));
        assert_eq!(counts.get(&5).map(|(c, _)| *c), Some(1));

        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("part1"));
        assert!(content.contains("part2"));
        assert!(content.contains("part3"));
    }
}
