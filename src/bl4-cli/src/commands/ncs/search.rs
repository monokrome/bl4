//! NCS search command

use anyhow::Result;
use bl4_ncs::NcsContent;
use std::fs;
use std::path::Path;

use super::types::SearchMatch;

pub fn search_files(path: &Path, pattern: &str, all: bool, limit: usize) -> Result<()> {
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        if !file_path.extension().map(|e| e == "bin").unwrap_or(false) {
            continue;
        }

        if let Ok(data) = fs::read(file_path) {
            if let Some(content) = NcsContent::parse(&data) {
                let search_strings: Vec<&str> = if all {
                    content.strings.iter().map(|s| s.as_str()).collect()
                } else {
                    content.entry_names().collect()
                };

                let matching: Vec<String> = search_strings
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&pattern_lower))
                    .map(|s| s.to_string())
                    .collect();

                if !matching.is_empty() {
                    matches.push(SearchMatch {
                        path: file_path.to_string_lossy().to_string(),
                        type_name: content.type_name().to_string(),
                        matches: matching,
                    });

                    if matches.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    println!("=== Search Results for '{}' ===", pattern);
    println!("Found {} files with matches\n", matches.len());

    for m in &matches {
        println!(
            "{} ({})",
            m.path.split('/').next_back().unwrap_or(&m.path),
            m.type_name
        );
        for s in &m.matches {
            println!("  - {}", s);
        }
        println!();
    }

    Ok(())
}
