//! NCS scan and stats commands

use anyhow::Result;
use bl4_ncs::{decompress_ncs, is_ncs, NcsContent};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::file_utils::walk_files_with_extension;
use super::types::ScanResult;

pub fn scan_directory(path: &Path, filter_type: Option<&str>, verbose: bool, json: bool) -> Result<()> {
    let mut result = ScanResult {
        total_files: 0,
        parsed_files: 0,
        types: HashMap::new(),
        formats: HashMap::new(),
    };

    walk_files_with_extension(path, &["bin"], |file_path| {
        result.total_files += 1;

        if let Ok(data) = fs::read(file_path) {
            // Decompress if needed
            let decompressed = if is_ncs(&data) {
                decompress_ncs(&data).ok()
            } else {
                Some(data)
            };

            let Some(decompressed) = decompressed else {
                return Ok(());
            };

            if let Some(content) = NcsContent::parse(&decompressed) {
                result.parsed_files += 1;

                let type_name = content.type_name().to_string();
                let format_code = content.format_code().to_string();

                // Apply filter
                if let Some(filter) = filter_type {
                    if !type_name.contains(filter) {
                        return Ok(());
                    }
                }

                result
                    .types
                    .entry(type_name.clone())
                    .or_default()
                    .push(file_path.to_string_lossy().to_string());

                *result.formats.entry(format_code.clone()).or_insert(0) += 1;

                if verbose && !json {
                    println!(
                        "{}: {} ({})",
                        file_path.file_name().unwrap().to_string_lossy(),
                        type_name,
                        format_code
                    );
                }
            }
        }

        Ok(())
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("\n=== Scan Results ===");
        println!("Total files: {}", result.total_files);
        println!("Parsed files: {}", result.parsed_files);
        println!(
            "Parse rate: {:.1}%",
            (result.parsed_files as f64 / result.total_files as f64) * 100.0
        );

        println!("\n=== Types ({}) ===", result.types.len());
        let mut types: Vec<_> = result.types.iter().collect();
        types.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        for (type_name, files) in types.iter().take(30) {
            println!("  {:40} {}", type_name, files.len());
        }

        println!("\n=== Format Codes ===");
        for (format, count) in &result.formats {
            println!("  {}: {}", format, count);
        }
    }

    Ok(())
}

pub fn show_stats(path: &Path, show_formats: bool) -> Result<()> {
    let mut total = 0;
    let mut parsed = 0;
    let mut types: HashMap<String, usize> = HashMap::new();
    let mut formats: HashMap<String, usize> = HashMap::new();
    let mut unparsed_samples: Vec<String> = Vec::new();

    walk_files_with_extension(path, &["bin"], |file_path| {
        total += 1;

        if let Ok(data) = fs::read(file_path) {
            if let Some(content) = NcsContent::parse(&data) {
                parsed += 1;
                *types.entry(content.type_name().to_string()).or_insert(0) += 1;
                *formats
                    .entry(content.format_code().to_string())
                    .or_insert(0) += 1;
            } else if unparsed_samples.len() < 5 {
                unparsed_samples.push(file_path.to_string_lossy().to_string());
            }
        }

        Ok(())
    })?;

    println!("=== NCS Statistics ===");
    println!("Total files: {}", total);
    println!("Parsed files: {}", parsed);
    println!("Unparsed files: {}", total - parsed);
    println!("Parse rate: {:.1}%", (parsed as f64 / total as f64) * 100.0);
    println!("Unique types: {}", types.len());

    if show_formats {
        println!("\n=== Format Code Distribution ===");
        let mut fmt_list: Vec<_> = formats.iter().collect();
        fmt_list.sort_by(|a, b| b.1.cmp(a.1));
        for (fmt, count) in fmt_list {
            println!(
                "  {}: {} ({:.1}%)",
                fmt,
                count,
                (*count as f64 / parsed as f64) * 100.0
            );
        }
    }

    if !unparsed_samples.is_empty() {
        println!("\n=== Sample Unparsed Files ===");
        for sample in &unparsed_samples {
            println!("  {}", sample);
        }
    }

    Ok(())
}
