//! NCS show command

use anyhow::{Context, Result};
use bl4_ncs::{decompress_ncs, is_ncs, parse_ncs_binary, NcsContent};
use std::fs;
use std::path::Path;

use super::format::output_tsv;
use super::types::FileInfo;
use super::util::print_hex;

#[allow(clippy::fn_params_excessive_bools)]
pub fn show_file(path: &Path, all_strings: bool, hex: bool, json: bool, tsv: bool) -> Result<()> {
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

    // For JSON output, use the structured parser
    if json {
        if let Some(doc) = parse_ncs_binary(&decompressed) {
            println!("{}", serde_json::to_string_pretty(&doc)?);
            return Ok(());
        }
        // Fall back to basic info if structured parse fails
    }

    // For TSV output, use the structured parser
    if tsv {
        if let Some(doc) = parse_ncs_binary(&decompressed) {
            output_tsv(&doc);
            return Ok(());
        }
        // Fall back to basic info if structured parse fails
    }

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
