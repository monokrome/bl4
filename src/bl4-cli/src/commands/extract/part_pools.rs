//! Part pools extraction command handler
//!
//! Groups parts from a parts database TSV by category.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Handle the ExtractCommand::PartPools command
///
/// Groups parts from a parts database (single TSV file or directory of per-category TSVs)
/// by category.
pub fn handle_part_pools(input: &Path, output: &Path) -> Result<()> {
    let by_category = if input.is_dir() {
        load_parts_from_dir(input)?
    } else {
        load_parts_from_file(input)?
    };

    let mut tsv = String::from("category\tpart_name\n");
    for (category, parts) in &by_category {
        for part in parts {
            tsv.push_str(&format!("{}\t{}\n", category, part));
        }
    }

    fs::write(output, &tsv)?;

    let total_parts: usize = by_category.values().map(|v| v.len()).sum();
    println!(
        "Extracted {} part pools with {} total parts to {}",
        by_category.len(),
        total_parts,
        output.display()
    );

    Ok(())
}

/// Load parts from a single monolithic TSV (category\tindex\tname)
fn load_parts_from_file(path: &Path) -> Result<BTreeMap<i64, Vec<String>>> {
    let data =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut by_category: BTreeMap<i64, Vec<String>> = BTreeMap::new();

    for line in data.lines().skip(1) {
        let mut cols = line.splitn(3, '\t');
        let Some(cat_str) = cols.next() else { continue };
        let Ok(category) = cat_str.parse::<i64>() else { continue };
        let _ = cols.next(); // skip index
        let Some(name) = cols.next() else { continue };

        if category > 0 {
            by_category.entry(category).or_default().push(name.to_string());
        }
    }

    for parts in by_category.values_mut() {
        parts.sort();
    }

    Ok(by_category)
}

/// Extract category ID from a filename stem like "jakobs_pistol-3" or "3"
fn parse_category_id(stem: &str) -> Option<i64> {
    if let Some(pos) = stem.rfind('-') {
        if let Ok(id) = stem[pos + 1..].parse() {
            return Some(id);
        }
    }
    stem.parse().ok()
}

/// Load parts from a directory of per-category TSV files ({slug}-{id}.tsv with index\tname)
fn load_parts_from_dir(dir: &Path) -> Result<BTreeMap<i64, Vec<String>>> {
    let mut by_category: BTreeMap<i64, Vec<String>> = BTreeMap::new();

    for entry in fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if !path.extension().is_some_and(|e| e == "tsv") {
            continue;
        }

        let category: i64 = match path.file_stem().and_then(|s| s.to_str()).and_then(parse_category_id) {
            Some(id) if id > 0 => id,
            _ => continue,
        };

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let mut parts: Vec<String> = content
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut cols = line.splitn(2, '\t');
                let _ = cols.next()?; // skip index
                Some(cols.next()?.to_string())
            })
            .collect();

        parts.sort();
        by_category.insert(category, parts);
    }

    Ok(by_category)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_part_pools_missing_file() {
        let result = handle_part_pools(
            Path::new("/nonexistent/input.tsv"),
            Path::new("/tmp/output.tsv"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_part_pools_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("parts.tsv");
        let output = dir.path().join("pools.tsv");

        fs::write(
            &input,
            "category\tindex\tname\n3\t0\tJAK_PS_barrel_01\n3\t1\tJAK_PS_grip_01\n5\t0\tVLA_AR_barrel_01\n",
        ).unwrap();

        handle_part_pools(&input, &output).unwrap();

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.starts_with("category\tpart_name\n"));
        assert!(content.contains("3\tJAK_PS_barrel_01"));
        assert!(content.contains("5\tVLA_AR_barrel_01"));
    }

    #[test]
    fn test_handle_part_pools_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let parts_dir = dir.path().join("parts");
        fs::create_dir(&parts_dir).unwrap();
        let output = dir.path().join("pools.tsv");

        fs::write(parts_dir.join("jakobs_pistol-3.tsv"), "index\tname\n0\tJAK_PS_barrel_01\n1\tJAK_PS_grip_01\n").unwrap();
        fs::write(parts_dir.join("vladof_ar-5.tsv"), "index\tname\n0\tVLA_AR_barrel_01\n").unwrap();

        handle_part_pools(&parts_dir, &output).unwrap();

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.starts_with("category\tpart_name\n"));
        assert!(content.contains("3\tJAK_PS_barrel_01"));
        assert!(content.contains("5\tVLA_AR_barrel_01"));
    }
}
