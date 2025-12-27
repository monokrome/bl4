//! Part extraction command handlers
//!
//! Handlers for extracting part definitions from memory dumps or live process.

use crate::memory::{self, MemorySource, PartDefinition};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Create a memory source from dump file or live process
fn create_memory_source(
    dump: Option<&Path>,
    dump_message: &str,
    live_message: &str,
) -> Result<Box<dyn MemorySource>> {
    match dump {
        Some(p) => {
            println!("{}", dump_message);
            Ok(Box::new(memory::DumpFile::open(p)?))
        }
        None => {
            println!("{}", live_message);
            let proc = memory::Bl4Process::attach()
                .context("Failed to attach to Borderlands 4 process")?;
            Ok(Box::new(proc))
        }
    }
}

/// Handle the ExtractParts command
///
/// Extracts part definitions with category/index from memory.
pub fn handle_extract_parts(output: &Path, dump: Option<&Path>, list_fnames: bool) -> Result<()> {
    let source = create_memory_source(
        dump,
        "Extracting part definitions from dump...",
        "Extracting part definitions from live process...",
    )?;

    // If --list-fnames, just dump all FNames containing .part_ and exit
    if list_fnames {
        println!("Listing all FNames containing '.part_'...");
        let fnames = memory::list_all_part_fnames(source.as_ref())?;
        for name in &fnames {
            println!("{}", name);
        }
        println!("\nTotal: {} FNames", fnames.len());
        return Ok(());
    }

    // Use the FName array pattern extraction
    let parts = memory::extract_parts_from_fname_arrays(source.as_ref())?;
    println!("Found {} part definitions", parts.len());

    // Group by category for summary
    let by_category = group_parts_by_category(&parts);
    print_category_summary(&by_category);

    // Write output JSON
    write_parts_json(output, &parts, &by_category)?;
    println!("\nWritten to: {}", output.display());

    Ok(())
}

/// Handle the ExtractPartsRaw command
///
/// Extracts raw part data without assumptions.
pub fn handle_extract_parts_raw(output: &Path, dump: Option<&Path>) -> Result<()> {
    let source = create_memory_source(
        dump,
        "Extracting raw part data from dump...",
        "Extracting raw part data from live process...",
    )?;

    let extraction = memory::extract_parts_raw(source.as_ref())?;
    println!(
        "Extracted {} parts with raw binary data",
        extraction.parts.len()
    );

    // Write output JSON
    let json = serde_json::to_string_pretty(&extraction)?;
    std::fs::write(output, &json)?;
    println!("Written to: {}", output.display());

    Ok(())
}

/// Group parts by category
fn group_parts_by_category(parts: &[PartDefinition]) -> BTreeMap<i64, Vec<&PartDefinition>> {
    let mut by_category: BTreeMap<i64, Vec<&PartDefinition>> = BTreeMap::new();
    for part in parts {
        by_category.entry(part.category).or_default().push(part);
    }
    by_category
}

/// Print a summary of categories found
fn print_category_summary(by_category: &BTreeMap<i64, Vec<&PartDefinition>>) {
    println!("\nCategories found:");
    for (category, cat_parts) in by_category {
        let max_idx = cat_parts.iter().map(|p| p.index).max().unwrap_or(0);
        println!(
            "  Category {:3}: {:3} parts (max index: {})",
            category,
            cat_parts.len(),
            max_idx
        );
    }
}

/// Write parts to JSON file
fn write_parts_json(
    output: &Path,
    parts: &[PartDefinition],
    by_category: &BTreeMap<i64, Vec<&PartDefinition>>,
) -> Result<()> {
    let mut json = String::from("{\n  \"parts\": [\n");
    for (i, part) in parts.iter().enumerate() {
        let escaped_name = part.name.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!(
            "    {{\"name\": \"{}\", \"category\": {}, \"index\": {}}}",
            escaped_name, part.category, part.index
        ));
        if i < parts.len() - 1 {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ],\n  \"summary\": {\n");

    let cat_count = by_category.len();
    for (i, (category, cat_parts)) in by_category.iter().enumerate() {
        json.push_str(&format!("    \"{}\": {}", category, cat_parts.len()));
        if i < cat_count - 1 {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  }\n}\n");

    std::fs::write(output, &json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_parts_by_category() {
        let parts = vec![
            PartDefinition {
                name: "part1".to_string(),
                category: 3,
                index: 0,
                object_address: 0x1000,
            },
            PartDefinition {
                name: "part2".to_string(),
                category: 3,
                index: 1,
                object_address: 0x2000,
            },
            PartDefinition {
                name: "part3".to_string(),
                category: 5,
                index: 0,
                object_address: 0x3000,
            },
        ];

        let by_category = group_parts_by_category(&parts);

        assert_eq!(by_category.len(), 2);
        assert_eq!(by_category.get(&3).map(|v| v.len()), Some(2));
        assert_eq!(by_category.get(&5).map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_write_parts_json() {
        use tempfile::TempDir;

        let parts = vec![
            PartDefinition {
                name: "JAK_PS.part_barrel_01".to_string(),
                category: 3,
                index: 0,
                object_address: 0x1000,
            },
            PartDefinition {
                name: "JAK_PS.part_barrel_02".to_string(),
                category: 3,
                index: 1,
                object_address: 0x2000,
            },
        ];

        let by_category = group_parts_by_category(&parts);

        let dir = TempDir::new().unwrap();
        let output = dir.path().join("parts.json");

        write_parts_json(&output, &parts, &by_category).unwrap();

        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("\"parts\""));
        assert!(content.contains("JAK_PS.part_barrel_01"));
        assert!(content.contains("JAK_PS.part_barrel_02"));
        assert!(content.contains("\"summary\""));
    }

    #[test]
    fn test_group_empty_parts() {
        let parts: Vec<PartDefinition> = vec![];
        let by_category = group_parts_by_category(&parts);
        assert!(by_category.is_empty());
    }
}
