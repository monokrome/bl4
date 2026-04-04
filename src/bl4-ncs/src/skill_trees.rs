//! Skill tree extraction from NCS skilltrees_data files
//!
//! Extracts skill grid positions and their tooltip references from the
//! skill tree structure. The tooltip keys can be joined with tooltip
//! display names (from uitooltipdata NCS files) to get human-readable names.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use crate::document::Value;
use crate::{decompress_ncs, is_ncs, parse_ncs_binary_from_reader};

/// A skill node extracted from NCS skill tree data
#[derive(Debug, Clone)]
pub struct SkillTreeEntry {
    pub category: u32,
    pub tree_color: String,
    pub tree_name: String,
    pub position: String,
    pub tooltip_key: String,
}

/// Class identifier → category ID mapping
const CLASS_CATEGORIES: &[(&str, u32)] = &[
    ("dark_siren", 254),
    ("exo", 256),
    ("gravitar", 259),
    ("paladin", 255),
    ("robodealer", 404),
];

fn category_for_class(key: &str) -> Option<u32> {
    let lower = key.to_lowercase();
    CLASS_CATEGORIES.iter()
        .find(|(name, _)| lower.contains(name))
        .map(|(_, cat)| *cat)
}

/// Extract skill tree entries from a single NCS binary.
pub fn extract_from_binary(data: &[u8]) -> Vec<SkillTreeEntry> {
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

    let table = match doc.tables.get("skilltrees_data") {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    for record in &table.records {
        for entry in &record.entries {
            let category = match category_for_class(&entry.key) {
                Some(c) => c,
                None => continue,
            };

            let skill_trees = match &entry.value {
                Value::Map(m) => match m.get("skilltrees") {
                    Some(Value::Array(arr)) => arr,
                    _ => continue,
                },
                _ => continue,
            };

            for tree in skill_trees {
                extract_tree(tree, category, &mut entries);
            }
        }
    }

    entries
}

/// Extract skills from a single skill tree (one of 3 per class).
fn extract_tree(tree: &Value, category: u32, entries: &mut Vec<SkillTreeEntry>) {
    let map = match tree {
        Value::Map(m) => m,
        _ => return,
    };

    let tree_name = extract_tree_name(map);
    let tree_color = extract_tree_color(map);

    let segments = match map.get("segments") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };

    // First segment is the trunk, rest are branches (left, mid, right)
    let branch_labels = ["left", "mid", "right"];

    for (seg_idx, segment) in segments.iter().enumerate() {
        let is_trunk = seg_idx == 0;
        let branch = if is_trunk { None } else { branch_labels.get(seg_idx - 1).copied() };

        extract_segment(segment, category, &tree_color, &tree_name, is_trunk, branch, entries);
    }
}

/// Extract skills from a segment (trunk or branch).
fn extract_segment(
    segment: &Value,
    category: u32,
    tree_color: &str,
    tree_name: &str,
    is_trunk: bool,
    branch: Option<&str>,
    entries: &mut Vec<SkillTreeEntry>,
) {
    let tiers = match segment {
        Value::Map(m) => match m.get("tiers") {
            Some(Value::Array(arr)) => arr,
            _ => return,
        },
        _ => return,
    };

    for (tier_idx, tier) in tiers.iter().enumerate() {
        let nodes = match tier {
            Value::Map(m) => match m.get("nodes") {
                Some(Value::Array(arr)) => arr,
                _ => continue,
            },
            _ => continue,
        };

        for (node_idx, node) in nodes.iter().enumerate() {
            let node_map = match node {
                Value::Map(m) => m,
                _ => continue,
            };

            // Skip augment and empty nodes
            if matches!(node_map.get("nodetype"), Some(Value::Leaf(s)) if s == "Augment" || s == "None") {
                continue;
            }

            let tooltip_key = match node_map.get("tooltip") {
                Some(Value::Leaf(s)) => extract_tooltip_key(s),
                _ => continue,
            };

            let row = if is_trunk { tier_idx + 1 } else { tier_idx + 4 };
            let col = node_idx + 1;

            let position = if let Some(b) = branch {
                format!("{}_{}_{}_{}", tree_color, b, row, col)
            } else {
                format!("{}_{}_{}", tree_color, row, col)
            };

            entries.push(SkillTreeEntry {
                category,
                tree_color: tree_color.to_string(),
                tree_name: tree_name.to_string(),
                position,
                tooltip_key,
            });
        }
    }
}

/// Extract tree display name from treename field.
/// Format: "dark_siren, <GUID>, Here Comes Trouble"
fn extract_tree_name(map: &HashMap<String, Value>) -> String {
    match map.get("treename") {
        Some(Value::Leaf(s)) => {
            let parts: Vec<&str> = s.splitn(3, ", ").collect();
            if parts.len() >= 3 {
                parts[2].trim().to_string()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Extract tree color from the color field.
/// Falls back to "red" if not present (the third tree per class omits it).
fn extract_tree_color(map: &HashMap<String, Value>) -> String {
    match map.get("color") {
        Some(Value::Leaf(s)) => s.to_lowercase(),
        _ => "red".to_string(),
    }
}

/// Strip the uitooltipdata wrapper to get the raw key.
/// "uitooltipdata'ToolTip_DS_P_GraveSustain'" → "ToolTip_DS_P_GraveSustain"
fn extract_tooltip_key(raw: &str) -> String {
    raw.strip_prefix("uitooltipdata'")
        .and_then(|s| s.strip_suffix("'"))
        .unwrap_or(raw)
        .to_string()
}

/// Extract skill tree entries from all skilltrees_data NCS files in a directory.
pub fn extract_from_directory(ncs_dir: &Path) -> Vec<SkillTreeEntry> {
    let mut all: HashMap<(u32, String), SkillTreeEntry> = HashMap::new();

    for entry in walkdir::WalkDir::new(ncs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let fname = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();

        if !fname.starts_with("skilltrees_data") {
            continue;
        }

        if let Ok(data) = std::fs::read(path) {
            for entry in extract_from_binary(&data) {
                let key = (entry.category, entry.position.clone());
                all.entry(key).or_insert(entry);
            }
        }
    }

    let mut result: Vec<SkillTreeEntry> = all.into_values().collect();
    result.sort_by(|a, b| a.category.cmp(&b.category).then(a.position.cmp(&b.position)));
    result
}

/// Write skill tree entries to a TSV file.
pub fn write_tsv(entries: &[SkillTreeEntry], path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "category\tposition\ttooltip_key\ttree_color\ttree_name")?;
    for entry in entries {
        writeln!(f, "{}\t{}\t{}\t{}\t{}", entry.category, entry.position, entry.tooltip_key, entry.tree_color, entry.tree_name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tooltip_key() {
        assert_eq!(
            extract_tooltip_key("uitooltipdata'ToolTip_DS_P_GraveSustain'"),
            "ToolTip_DS_P_GraveSustain"
        );
        assert_eq!(
            extract_tooltip_key("uitooltipdata'tooltip_exo_passive_26_Sitiar'"),
            "tooltip_exo_passive_26_Sitiar"
        );
    }

    #[test]
    fn test_category_for_class() {
        assert_eq!(category_for_class("dark_siren_skill_trees"), Some(254));
        assert_eq!(category_for_class("exo_skill_trees"), Some(256));
        assert_eq!(category_for_class("gravitar_skill_trees"), Some(259));
        assert_eq!(category_for_class("paladin_skill_trees"), Some(255));
        assert_eq!(category_for_class("robodealer_skill_trees"), Some(404));
        assert_eq!(category_for_class("unknown_class"), None);
    }

    #[test]
    #[ignore] // Requires NCS data files
    fn test_extract_and_write() {
        let ncs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("share/manifest/ncs");

        if !ncs_dir.exists() {
            return;
        }

        let entries = extract_from_directory(&ncs_dir);
        assert!(!entries.is_empty(), "Should find skill tree entries");

        let out_path = ncs_dir.parent().unwrap().join("skill_trees.tsv");
        write_tsv(&entries, &out_path).unwrap();

        let mut by_cat: HashMap<u32, usize> = HashMap::new();
        for entry in &entries {
            *by_cat.entry(entry.category).or_default() += 1;
            eprintln!("  {} {} → {} ({})", entry.category, entry.position, entry.tooltip_key, entry.tree_name);
        }
        eprintln!("\nWrote {} entries to {}", entries.len(), out_path.display());
        eprintln!("Per-class counts: {:?}", by_cat);
    }
}
