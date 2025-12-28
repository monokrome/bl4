//! Parts database query command handlers
//!
//! Provides functions to query and display parts from the parts database.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Part categories file structure (for BuildPartsDb command)
#[derive(Debug, Deserialize)]
pub struct PartCategoriesFile {
    pub categories: Vec<PartCategory>,
}

/// Individual part category mapping
#[derive(Debug, Deserialize)]
pub struct PartCategory {
    pub prefix: String,
    pub category: i64,
    #[serde(default)]
    pub weapon_type: Option<String>,
    #[serde(default)]
    pub gear_type: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
}

/// Parts database structure
#[derive(Debug, Deserialize)]
pub struct PartsDatabase {
    pub parts: Vec<PartEntry>,
}

/// Individual part entry in the database
#[derive(Debug, Deserialize, Clone)]
pub struct PartEntry {
    pub name: String,
    pub category: i64,
    pub index: i64,
}

/// Result of querying the parts database
pub struct PartsQueryResult {
    pub categories: BTreeMap<i64, Vec<PartEntry>>,
    pub total_parts: usize,
}

/// Load and parse the parts database from a file
pub fn load_database(path: &Path) -> Result<PartsDatabase> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read parts database: {:?}", path))?;

    serde_json::from_str(&content).context("Failed to parse parts database")
}

/// Build a category-to-parts mapping from the database
pub fn build_category_map(db: &PartsDatabase) -> BTreeMap<i64, Vec<&PartEntry>> {
    let mut by_category: BTreeMap<i64, Vec<&PartEntry>> = BTreeMap::new();
    for part in &db.parts {
        by_category.entry(part.category).or_default().push(part);
    }
    by_category
}

/// Find a category ID by searching for a weapon name
pub fn find_category_by_name(
    by_category: &BTreeMap<i64, Vec<&PartEntry>>,
    search: &str,
) -> Option<FindCategoryResult> {
    let search_lower = search.to_lowercase();
    let mut found: Option<i64> = None;
    let mut matches: Vec<(i64, String)> = Vec::new();

    for &cat_id in by_category.keys() {
        if let Some(name) = bl4::category_name(cat_id) {
            if name.to_lowercase().contains(&search_lower) {
                matches.push((cat_id, name.to_string()));
                if found.is_none() {
                    found = Some(cat_id);
                } else {
                    // Multiple matches
                    return Some(FindCategoryResult::Multiple(matches));
                }
            }
        }
    }

    found.map(FindCategoryResult::Single)
}

/// Result of searching for a category
pub enum FindCategoryResult {
    Single(i64),
    Multiple(Vec<(i64, String)>),
}

/// Group parts by type (barrel, grip, mag, etc.)
pub fn group_parts_by_type<'a>(parts: &[&'a PartEntry]) -> BTreeMap<String, Vec<&'a PartEntry>> {
    let mut by_type: BTreeMap<String, Vec<&'a PartEntry>> = BTreeMap::new();

    for &part in parts {
        let part_type = part
            .name
            .split(".part_")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or("other")
            .to_string();
        by_type.entry(part_type).or_default().push(part);
    }

    by_type
}

/// List all available categories
pub fn list_categories(by_category: &BTreeMap<i64, Vec<&PartEntry>>, total_parts: usize) {
    println!("Available categories:");
    println!();
    for (&cat_id, parts) in by_category {
        let cat_name = bl4::category_name(cat_id).unwrap_or("Unknown");
        println!("  {:3}: {} ({} parts)", cat_id, cat_name, parts.len());
    }
    println!();
    println!(
        "Total: {} categories, {} parts",
        by_category.len(),
        total_parts
    );
}

/// Show parts for a specific category
pub fn show_category_parts(cat_id: i64, parts: Option<&Vec<&PartEntry>>) {
    let cat_name = bl4::category_name(cat_id).unwrap_or("Unknown");

    println!("Parts for {} (category {}):", cat_name, cat_id);
    println!();

    if let Some(parts) = parts {
        let by_type = group_parts_by_type(parts);

        for (ptype, type_parts) in &by_type {
            println!("  {} ({} variants):", ptype, type_parts.len());
            for part in type_parts {
                println!("    [{}] {}", part.index, part.name);
            }
            println!();
        }

        println!("Total: {} parts", parts.len());
    } else {
        println!("  No parts found for this category");
    }
}

/// Show usage help for the parts command
pub fn show_usage() {
    println!("Usage: bl4 parts --weapon <name> OR --category <id> OR --list");
    println!();
    println!("Examples:");
    println!("  bl4 parts --list                 # List all categories");
    println!("  bl4 parts --weapon 'Jakobs'      # Find Jakobs weapons");
    println!("  bl4 parts --category 3           # Show parts for category 3");
}

/// Main handler for the parts command
pub fn handle(
    weapon: Option<String>,
    category: Option<i64>,
    list: bool,
    parts_db: &Path,
) -> Result<()> {
    let db = load_database(parts_db)?;
    let by_category = build_category_map(&db);

    if list {
        list_categories(&by_category, db.parts.len());
        return Ok(());
    }

    // Find target category
    let target_cat: Option<i64> = if let Some(cat) = category {
        Some(cat)
    } else if let Some(ref wname) = weapon {
        match find_category_by_name(&by_category, wname) {
            Some(FindCategoryResult::Single(cat_id)) => Some(cat_id),
            Some(FindCategoryResult::Multiple(matches)) => {
                println!(
                    "Multiple matches for '{}'. Please be more specific or use -c <category_id>",
                    wname
                );
                for (c, n) in matches {
                    println!("  {:3}: {}", c, n);
                }
                return Ok(());
            }
            None => None,
        }
    } else {
        None
    };

    if let Some(cat_id) = target_cat {
        show_category_parts(cat_id, by_category.get(&cat_id));
    } else {
        show_usage();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_database() -> PartsDatabase {
        PartsDatabase {
            parts: vec![
                PartEntry {
                    name: "JAK_PS.part_barrel_01".to_string(),
                    category: 3,
                    index: 0,
                },
                PartEntry {
                    name: "JAK_PS.part_barrel_02".to_string(),
                    category: 3,
                    index: 1,
                },
                PartEntry {
                    name: "JAK_PS.part_grip_01".to_string(),
                    category: 3,
                    index: 2,
                },
                PartEntry {
                    name: "VLA_AR.part_barrel_01".to_string(),
                    category: 5,
                    index: 0,
                },
                PartEntry {
                    name: "VLA_AR.part_mag_01".to_string(),
                    category: 5,
                    index: 1,
                },
            ],
        }
    }

    #[test]
    fn test_build_category_map() {
        let db = create_test_database();
        let by_category = build_category_map(&db);

        assert_eq!(by_category.len(), 2);
        assert_eq!(by_category.get(&3).map(|v| v.len()), Some(3));
        assert_eq!(by_category.get(&5).map(|v| v.len()), Some(2));
    }

    #[test]
    fn test_group_parts_by_type() {
        let db = create_test_database();
        let by_category = build_category_map(&db);
        let parts = by_category.get(&3).unwrap();
        let by_type = group_parts_by_type(parts);

        assert_eq!(by_type.len(), 2); // barrel and grip
        assert_eq!(by_type.get("barrel").map(|v| v.len()), Some(2));
        assert_eq!(by_type.get("grip").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_part_entry_structure() {
        let part = PartEntry {
            name: "TEST.part_barrel_01".to_string(),
            category: 1,
            index: 42,
        };

        assert_eq!(part.name, "TEST.part_barrel_01");
        assert_eq!(part.category, 1);
        assert_eq!(part.index, 42);
    }

    #[test]
    fn test_parts_database_deserialize() {
        let json = r#"{
            "parts": [
                {"name": "TEST.part_01", "category": 1, "index": 0},
                {"name": "TEST.part_02", "category": 1, "index": 1}
            ]
        }"#;

        let db: PartsDatabase = serde_json::from_str(json).unwrap();
        assert_eq!(db.parts.len(), 2);
        assert_eq!(db.parts[0].name, "TEST.part_01");
    }

    #[test]
    fn test_empty_database() {
        let db = PartsDatabase { parts: vec![] };
        let by_category = build_category_map(&db);

        assert!(by_category.is_empty());
    }

    #[test]
    fn test_group_parts_with_unknown_type() {
        let parts = vec![PartEntry {
            name: "UNKNOWN_FORMAT".to_string(),
            category: 1,
            index: 0,
        }];
        let refs: Vec<&PartEntry> = parts.iter().collect();
        let by_type = group_parts_by_type(&refs);

        // Should fall back to "other" type
        assert!(by_type.contains_key("other"));
    }
}
