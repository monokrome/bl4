//! Part pools extraction command handler
//!
//! Extracts part pools from a parts database JSON file.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Handle the ExtractCommand::PartPools command
///
/// Extracts part pools from a parts database JSON file.
pub fn handle_part_pools(input: &Path, output: &Path) -> Result<()> {
    // Read the parts database (memory-extracted names + verified category assignments)
    let data = fs::read_to_string(input)
        .with_context(|| format!("Failed to read {}", input.display()))?;

    // Parse parts array from JSON
    // Structure: { "parts": [ { "category": N, "name": "...", ... }, ... ], "categories": {...} }
    let parts_start = data.find("\"parts\"").context("Missing 'parts' key")?;
    let array_start = data[parts_start..]
        .find('[')
        .context("Missing parts array")?
        + parts_start;

    // Find the matching closing bracket
    let mut depth = 0;
    let mut array_end = array_start;
    for (i, c) in data[array_start..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    array_end = array_start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let parts_json = &data[array_start..=array_end];

    // Parse part entries - only need category and name
    struct PartEntry {
        category: i64,
        name: String,
    }

    let mut parts: Vec<PartEntry> = Vec::new();
    let mut in_object = false;
    let mut current_category: i64 = -1;
    let mut current_name = String::new();
    let mut depth = 0;

    for (i, c) in parts_json.char_indices() {
        match c {
            '{' => {
                depth += 1;
                if depth == 1 {
                    in_object = true;
                    current_category = -1;
                    current_name.clear();
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 && in_object {
                    if current_category > 0 && !current_name.is_empty() {
                        parts.push(PartEntry {
                            category: current_category,
                            name: std::mem::take(&mut current_name),
                        });
                    }
                    in_object = false;
                }
            }
            '"' if in_object && depth == 1 => {
                let rest = &parts_json[i + 1..];
                if let Some(end) = rest.find('"') {
                    let key = &rest[..end];
                    let after_key = &rest[end + 1..];
                    if let Some(colon) = after_key.find(':') {
                        let value_start = after_key[colon + 1..].trim_start();
                        match key {
                            "category" => {
                                let num_end = value_start
                                    .find(|c: char| !c.is_ascii_digit() && c != '-')
                                    .unwrap_or(value_start.len());
                                if let Ok(n) = value_start[..num_end].parse::<i64>() {
                                    current_category = n;
                                }
                            }
                            "name" => {
                                if let Some(name_rest) = value_start.strip_prefix('"') {
                                    if let Some(name_end) = name_rest.find('"') {
                                        current_name = name_rest[..name_end].to_string();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Group parts by category
    let mut by_category: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for part in parts {
        by_category
            .entry(part.category)
            .or_default()
            .push(part.name);
    }

    // Sort parts within each category alphabetically (consistent ordering)
    for parts_vec in by_category.values_mut() {
        parts_vec.sort();
    }

    // Parse category names from the input
    let mut category_names: BTreeMap<i64, String> = BTreeMap::new();
    if let Some(cats_start) = data.find("\"categories\"") {
        if let Some(obj_start) = data[cats_start..].find('{') {
            let cats_section = &data[cats_start + obj_start..];
            // Simple parsing for "N": {"name": "..."}
            let mut pos = 0;
            while let Some(quote_pos) = cats_section[pos..].find('"') {
                let key_start = pos + quote_pos + 1;
                if let Some(key_end) = cats_section[key_start..].find('"') {
                    let key = &cats_section[key_start..key_start + key_end];
                    if let Ok(cat_id) = key.parse::<i64>() {
                        // Look for "name": "..." after this
                        let after = &cats_section[key_start + key_end..];
                        if let Some(name_pos) = after.find("\"name\"") {
                            let name_section = &after[name_pos + 7..];
                            if let Some(val_start) = name_section.find('"') {
                                let name_rest = &name_section[val_start + 1..];
                                if let Some(val_end) = name_rest.find('"') {
                                    category_names
                                        .insert(cat_id, name_rest[..val_end].to_string());
                                }
                            }
                        }
                    }
                    pos = key_start + key_end + 1;
                } else {
                    break;
                }
            }
        }
    }

    // Build output JSON with clear metadata
    let mut json = String::from("{\n");
    json.push_str(&format!(
        "  \"version\": \"{}\",\n",
        env!("CARGO_PKG_VERSION")
    ));
    json.push_str("  \"source\": \"parts_database.json (memory-extracted part names)\",\n");
    json.push_str("  \"notes\": {\n");
    json.push_str(
        "    \"part_names\": \"Extracted from game memory via string pattern matching - AUTHORITATIVE\",\n",
    );
    json.push_str(
        "    \"category_assignments\": \"Based on name prefix matching, verified by serial decode - VERIFIED\",\n",
    );
    json.push_str(
        "    \"part_order\": \"Alphabetical within category - NOT authoritative, use memory extraction for true indices\"\n",
    );
    json.push_str("  },\n");
    json.push_str("  \"pools\": {\n");

    let pool_count = by_category.len();
    for (i, (category, cat_parts)) in by_category.iter().enumerate() {
        let cat_name = category_names
            .get(category)
            .cloned()
            .unwrap_or_else(|| format!("Category {}", category));

        json.push_str(&format!("    \"{}\": {{\n", category));
        json.push_str(&format!(
            "      \"name\": \"{}\",\n",
            cat_name.replace('"', "\\\"")
        ));
        json.push_str(&format!("      \"part_count\": {},\n", cat_parts.len()));
        json.push_str("      \"parts\": [\n");

        for (j, part) in cat_parts.iter().enumerate() {
            let escaped = part.replace('\\', "\\\\").replace('"', "\\\"");
            json.push_str(&format!("        \"{}\"", escaped));
            if j < cat_parts.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }

        json.push_str("      ]\n");
        json.push_str("    }");
        if i < pool_count - 1 {
            json.push(',');
        }
        json.push('\n');
    }

    json.push_str("  },\n");

    // Summary
    json.push_str("  \"summary\": {\n");
    json.push_str(&format!("    \"total_pools\": {},\n", pool_count));
    let total_parts: usize = by_category.values().map(|v| v.len()).sum();
    json.push_str(&format!("    \"total_parts\": {}\n", total_parts));
    json.push_str("  }\n");
    json.push_str("}\n");

    fs::write(output, &json)?;

    println!(
        "Extracted {} part pools with {} total parts",
        pool_count, total_parts
    );
    println!("\nData sources:");
    println!("  Part names: Memory extraction (authoritative)");
    println!("  Categories: Prefix matching (verified by decode)");
    println!("  Part order: Alphabetical (not authoritative)");
    println!("\nWritten to: {}", output.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_part_pools_missing_file() {
        let result = handle_part_pools(
            Path::new("/nonexistent/input.json"),
            Path::new("/tmp/output.json"),
        );
        assert!(result.is_err());
    }
}
