//! NCS command handlers

use anyhow::{Context, Result};
use bl4_ncs::{decompress_ncs, is_ncs, NcsContent};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::NcsCommand;

/// Result of scanning a directory
#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub total_files: usize,
    pub parsed_files: usize,
    pub types: HashMap<String, Vec<String>>,
    pub formats: HashMap<String, usize>,
}

/// Information about a single NCS file
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub type_name: String,
    pub format_code: String,
    pub entry_names: Vec<String>,
    pub guids: Vec<String>,
    pub numeric_values: Vec<(String, f64)>,
}

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub type_name: String,
    pub matches: Vec<String>,
}

pub fn handle_ncs_command(command: NcsCommand) -> Result<()> {
    match command {
        NcsCommand::Scan {
            path,
            filter_type,
            verbose,
            json,
        } => scan_directory(&path, filter_type.as_deref(), verbose, json),

        NcsCommand::Show {
            path,
            all_strings,
            hex,
            json,
        } => show_file(&path, all_strings, hex, json),

        NcsCommand::Search {
            path,
            pattern,
            all,
            limit,
        } => search_files(&path, &pattern, all, limit),

        NcsCommand::Extract {
            path,
            extract_type,
            output,
            json,
        } => extract_by_type(&path, &extract_type, output.as_deref(), json),

        NcsCommand::Stats { path, formats } => show_stats(&path, formats),

        NcsCommand::Decompress {
            input,
            output,
            offset,
        } => decompress_file(&input, output.as_deref(), offset),
    }
}

fn scan_directory(path: &Path, filter_type: Option<&str>, verbose: bool, json: bool) -> Result<()> {
    let mut result = ScanResult {
        total_files: 0,
        parsed_files: 0,
        types: HashMap::new(),
        formats: HashMap::new(),
    };

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        if !file_path.extension().map(|e| e == "bin").unwrap_or(false) {
            continue;
        }

        result.total_files += 1;

        if let Ok(data) = fs::read(file_path) {
            // Decompress if needed
            let decompressed = if is_ncs(&data) {
                decompress_ncs(&data).ok()
            } else {
                Some(data)
            };

            let Some(decompressed) = decompressed else {
                continue;
            };

            if let Some(content) = NcsContent::parse(&decompressed) {
                result.parsed_files += 1;

                let type_name = content.type_name().to_string();
                let format_code = content.format_code().to_string();

                // Apply filter
                if let Some(filter) = filter_type {
                    if !type_name.contains(filter) {
                        continue;
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
    }

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

fn show_file(path: &Path, all_strings: bool, hex: bool, json: bool) -> Result<()> {
    let data = fs::read(path).context("Failed to read file")?;

    if hex {
        print_hex(&data);
        return Ok(());
    }

    // Decompress if this is a compressed NCS file
    let decompressed = if is_ncs(&data) {
        decompress_ncs(&data).context("Failed to decompress NCS data")?
    } else {
        data
    };

    let content = NcsContent::parse(&decompressed).context("Failed to parse NCS content")?;

    let info = FileInfo {
        path: path.to_string_lossy().to_string(),
        type_name: content.type_name().to_string(),
        format_code: content.format_code().to_string(),
        entry_names: if all_strings {
            content.strings.clone()
        } else {
            content.entry_names().map(|s| s.to_string()).collect()
        },
        guids: content.guids().map(|s| s.to_string()).collect(),
        numeric_values: content
            .numeric_values()
            .map(|(s, v)| (s.to_string(), v))
            .collect(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("File: {}", info.path);
        println!("Type: {}", info.type_name);
        println!("Format: {}", info.format_code);

        println!("\nEntry Names ({}):", info.entry_names.len());
        for name in &info.entry_names {
            println!("  - {}", name);
        }

        if !info.guids.is_empty() {
            println!("\nGUIDs ({}):", info.guids.len());
            for guid in &info.guids {
                println!("  - {}", guid);
            }
        }

        if !info.numeric_values.is_empty() {
            println!("\nNumeric Values ({}):", info.numeric_values.len());
            for (s, v) in &info.numeric_values {
                println!("  - {} = {}", s, v);
            }
        }
    }

    Ok(())
}

fn search_files(path: &Path, pattern: &str, all: bool, limit: usize) -> Result<()> {
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
            m.path.split('/').last().unwrap_or(&m.path),
            m.type_name
        );
        for s in &m.matches {
            println!("  - {}", s);
        }
        println!();
    }

    Ok(())
}

fn extract_by_type(
    path: &Path,
    extract_type: &str,
    output: Option<&Path>,
    json: bool,
) -> Result<()> {
    let mut extracted = Vec::new();

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
                if content.type_name() == extract_type {
                    extracted.push(FileInfo {
                        path: file_path.to_string_lossy().to_string(),
                        type_name: content.type_name().to_string(),
                        format_code: content.format_code().to_string(),
                        entry_names: content.entry_names().map(|s| s.to_string()).collect(),
                        guids: content.guids().map(|s| s.to_string()).collect(),
                        numeric_values: content
                            .numeric_values()
                            .map(|(s, v)| (s.to_string(), v))
                            .collect(),
                    });
                }
            }
        }
    }

    let output_str = if json {
        serde_json::to_string_pretty(&extracted)?
    } else {
        let mut out = format!("=== Extracted {} entries ===\n\n", extracted.len());
        for info in &extracted {
            out.push_str(&format!("File: {}\n", info.path));
            out.push_str(&format!("Format: {}\n", info.format_code));
            out.push_str("Entries:\n");
            for name in &info.entry_names {
                out.push_str(&format!("  - {}\n", name));
            }
            out.push('\n');
        }
        out
    };

    if let Some(output_path) = output {
        fs::write(output_path, &output_str)?;
        println!(
            "Wrote {} entries to {}",
            extracted.len(),
            output_path.display()
        );
    } else {
        println!("{}", output_str);
    }

    Ok(())
}

fn show_stats(path: &Path, show_formats: bool) -> Result<()> {
    let mut total = 0;
    let mut parsed = 0;
    let mut types: HashMap<String, usize> = HashMap::new();
    let mut formats: HashMap<String, usize> = HashMap::new();
    let mut unparsed_samples: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        if !file_path.extension().map(|e| e == "bin").unwrap_or(false) {
            continue;
        }

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
    }

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

fn decompress_file(input: &Path, output: Option<&Path>, offset: Option<usize>) -> Result<()> {
    use bl4_ncs::scan_for_ncs;

    let data = fs::read(input).context("Failed to read input file")?;

    // If offset specified, decompress single chunk
    if let Some(off) = offset {
        let ncs_data = &data[off..];
        let decompressed =
            bl4_ncs::decompress_ncs(ncs_data).context("Failed to decompress NCS data")?;

        if let Some(output_path) = output {
            fs::write(output_path, &decompressed)?;
            println!(
                "Decompressed {} bytes -> {} bytes to {}",
                ncs_data.len(),
                decompressed.len(),
                output_path.display()
            );
        } else {
            show_decompressed_content(&decompressed);
        }
        return Ok(());
    }

    // If this is a single NCS file, decompress it
    if is_ncs(&data) {
        let decompressed =
            bl4_ncs::decompress_ncs(&data).context("Failed to decompress NCS data")?;
        if let Some(output_path) = output {
            fs::write(output_path, &decompressed)?;
            println!(
                "Decompressed {} bytes -> {} bytes to {}",
                data.len(),
                decompressed.len(),
                output_path.display()
            );
        } else {
            show_decompressed_content(&decompressed);
        }
        return Ok(());
    }

    // Scan for NCS chunks in the file (e.g., pak file)
    let chunks = scan_for_ncs(&data);
    if chunks.is_empty() {
        anyhow::bail!("No NCS chunks found in file");
    }

    println!("Found {} NCS chunks", chunks.len());

    let output_dir = output.map(Path::to_path_buf).unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        PathBuf::from(format!("{}_ncs", stem))
    });
    fs::create_dir_all(&output_dir)?;

    let mut success = 0;
    let mut failed = 0;

    for (offset, header) in &chunks {
        let chunk_data = &data[*offset..*offset + header.total_size()];
        match bl4_ncs::decompress_ncs(chunk_data) {
            Ok(decompressed) => {
                // Try to get type name for filename
                let filename = if let Some(content) = NcsContent::parse(&decompressed) {
                    format!("0x{:08x}_{}.bin", offset, content.type_name())
                } else {
                    format!("0x{:08x}.bin", offset)
                };

                let out_path = output_dir.join(&filename);
                fs::write(&out_path, &decompressed)?;
                success += 1;
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    println!(
        "Extracted {} chunks to {} ({} failed)",
        success,
        output_dir.display(),
        failed
    );

    Ok(())
}

fn show_decompressed_content(decompressed: &[u8]) {
    if let Some(content) = NcsContent::parse(decompressed) {
        println!("Type: {}", content.type_name());
        println!("Format: {}", content.format_code());
        println!("\nEntry Names:");
        for name in content.entry_names().take(20) {
            println!("  - {}", name);
        }
    } else {
        println!(
            "Decompressed {} bytes (could not parse content)",
            decompressed.len()
        );
        print_hex(&decompressed[..decompressed.len().min(256)]);
    }
}

fn print_hex(data: &[u8]) {
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("{:08x}  ", i * 16);
        for (j, byte) in chunk.iter().enumerate() {
            if j == 8 {
                print!(" ");
            }
            print!("{:02x} ", byte);
        }
        // Padding for incomplete lines
        for j in chunk.len()..16 {
            if j == 8 {
                print!(" ");
            }
            print!("   ");
        }
        print!(" |");
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }
}
