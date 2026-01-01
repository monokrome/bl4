//! NCS command handlers

use anyhow::{Context, Result};
use bl4_ncs::oodle::{self, OodleDecompressor};
use bl4_ncs::{decompress_ncs, decompress_ncs_with, is_ncs, parse_document, NcsContent};
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
            tsv,
        } => show_file(&path, all_strings, hex, json, tsv),

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

        #[cfg(target_os = "windows")]
        NcsCommand::Decompress {
            input,
            output,
            offset,
            raw,
            oodle_dll,
            oodle_exec,
        } => decompress_file(&input, output.as_deref(), offset, raw, oodle_dll.as_deref(), oodle_exec.as_deref()),

        #[cfg(not(target_os = "windows"))]
        NcsCommand::Decompress {
            input,
            output,
            offset,
            raw,
            oodle_exec,
        } => decompress_file(&input, output.as_deref(), offset, raw, oodle_exec.as_deref()),

        NcsCommand::Debug { path, hex, parse, offsets } => debug_file(&path, hex, parse, offsets),
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

fn show_file(path: &Path, all_strings: bool, hex: bool, json: bool, tsv: bool) -> Result<()> {
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
        if let Some(doc) = parse_document(&decompressed) {
            println!("{}", serde_json::to_string_pretty(&doc)?);
            return Ok(());
        }
        // Fall back to basic info if structured parse fails
    }

    // For TSV output, use the structured parser
    if tsv {
        if let Some(doc) = parse_document(&decompressed) {
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

/// Output parsed document as TSV (tab-separated values) to stdout
fn output_tsv(doc: &bl4_ncs::Document) {
    print!("{}", format_tsv(doc));
}

/// Format parsed document as TSV string
fn format_tsv(doc: &bl4_ncs::Document) -> String {
    use bl4_ncs::Value;
    use std::fmt::Write;

    let mut output = String::new();

    // Collect all field names across all records
    let mut all_fields: Vec<String> = Vec::new();
    for record in &doc.records {
        for key in record.fields.keys() {
            if !all_fields.contains(key) {
                all_fields.push(key.clone());
            }
        }
    }
    all_fields.sort();

    // Write header
    write!(output, "name").unwrap();
    for field in &all_fields {
        write!(output, "\t{}", field).unwrap();
    }
    writeln!(output).unwrap();

    // Write rows
    for record in &doc.records {
        write!(output, "{}", record.name).unwrap();
        for field in &all_fields {
            write!(output, "\t").unwrap();
            if let Some(value) = record.fields.get(field) {
                match value {
                    Value::String(s) => write!(output, "{}", s).unwrap(),
                    Value::Number(n) => write!(output, "{}", n).unwrap(),
                    Value::Integer(i) => write!(output, "{}", i).unwrap(),
                    Value::Boolean(b) => write!(output, "{}", b).unwrap(),
                    Value::Reference(r) => write!(output, "{}", r).unwrap(),
                    Value::Array(arr) => {
                        let items: Vec<String> = arr.iter().map(|v| format!("{:?}", v)).collect();
                        write!(output, "[{}]", items.join(",")).unwrap();
                    }
                    Value::Object(_) => write!(output, "{{...}}").unwrap(),
                    Value::Null => {}
                }
            }
        }
        writeln!(output).unwrap();
    }

    output
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

#[cfg(target_os = "windows")]
fn decompress_file(
    input: &Path,
    output: Option<&Path>,
    offset: Option<usize>,
    raw: bool,
    oodle_dll: Option<&Path>,
    oodle_exec: Option<&str>,
) -> Result<()> {
    // Create the appropriate decompressor backend
    let decompressor: Box<dyn OodleDecompressor> = if let Some(dll_path) = oodle_dll {
        println!("Using native Oodle backend from: {}", dll_path.display());
        oodle::native_backend(dll_path)
            .map_err(|e| anyhow::anyhow!("Failed to load Oodle DLL: {}", e))?
    } else if let Some(cmd) = oodle_exec {
        println!("Using exec Oodle backend: {}", cmd);
        oodle::exec_backend(cmd)
    } else {
        oodle::default_backend()
    };

    decompress_file_impl(input, output, offset, raw, decompressor)
}

#[cfg(not(target_os = "windows"))]
fn decompress_file(
    input: &Path,
    output: Option<&Path>,
    offset: Option<usize>,
    raw: bool,
    oodle_exec: Option<&str>,
) -> Result<()> {
    // Create the appropriate decompressor backend
    let decompressor: Box<dyn OodleDecompressor> = if let Some(cmd) = oodle_exec {
        println!("Using exec Oodle backend: {}", cmd);
        oodle::exec_backend(cmd)
    } else {
        oodle::default_backend()
    };

    decompress_file_impl(input, output, offset, raw, decompressor)
}

fn decompress_file_impl(
    input: &Path,
    output: Option<&Path>,
    offset: Option<usize>,
    raw: bool,
    decompressor: Box<dyn OodleDecompressor>,
) -> Result<()> {
    use bl4_ncs::scan_for_ncs;

    let data = fs::read(input).context("Failed to read input file")?;

    // If offset specified, decompress single chunk
    if let Some(off) = offset {
        let ncs_data = &data[off..];
        let decompressed = decompress_ncs_with(ncs_data, decompressor.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to decompress NCS data: {}", e))?;

        if let Some(output_path) = output {
            if raw {
                fs::write(output_path, &decompressed)?;
            } else if let Some(doc) = parse_document(&decompressed) {
                fs::write(output_path, format_tsv(&doc))?;
            } else {
                fs::write(output_path, &decompressed)?;
            }
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
        let decompressed = decompress_ncs_with(&data, decompressor.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to decompress NCS data: {}", e))?;
        if let Some(output_path) = output {
            if raw {
                fs::write(output_path, &decompressed)?;
            } else if let Some(doc) = parse_document(&decompressed) {
                fs::write(output_path, format_tsv(&doc))?;
            } else {
                fs::write(output_path, &decompressed)?;
            }
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

    println!(
        "Found {} NCS chunks (using {} backend)",
        chunks.len(),
        decompressor.name()
    );

    let output_dir = output.map(Path::to_path_buf).unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        PathBuf::from(format!("{}_ncs", stem))
    });
    fs::create_dir_all(&output_dir)?;

    let mut success = 0;
    let mut failed = 0;
    let mut failed_types: Vec<String> = Vec::new();

    for (offset, header) in &chunks {
        let chunk_data = &data[*offset..*offset + header.total_size()];
        match decompress_ncs_with(chunk_data, decompressor.as_ref()) {
            Ok(decompressed) => {
                if raw {
                    // Raw mode: save binary with type name if possible
                    if let Some(content) = NcsContent::parse(&decompressed) {
                        let filename = format!("{}.bin", content.type_name());
                        let out_path = output_dir.join(&filename);
                        fs::write(&out_path, &decompressed)?;
                    } else {
                        let filename = format!("0x{:08x}.bin", offset);
                        let out_path = output_dir.join(&filename);
                        fs::write(&out_path, &decompressed)?;
                    }
                    success += 1;
                } else if let Some(doc) = parse_document(&decompressed) {
                    // Parse and output as TSV
                    let filename = format!("{}.tsv", doc.type_name);
                    let out_path = output_dir.join(&filename);
                    let tsv_content = format_tsv(&doc);
                    fs::write(&out_path, &tsv_content)?;
                    success += 1;
                } else if let Some(content) = NcsContent::parse(&decompressed) {
                    // Fallback: couldn't parse structure, save raw with type name
                    let filename = format!("{}.bin", content.type_name());
                    let out_path = output_dir.join(&filename);
                    fs::write(&out_path, &decompressed)?;
                    success += 1;
                } else {
                    // Fallback: save raw binary
                    let filename = format!("0x{:08x}.bin", offset);
                    let out_path = output_dir.join(&filename);
                    fs::write(&out_path, &decompressed)?;
                    success += 1;
                }
            }
            Err(e) => {
                // Try to identify the type from the raw data if possible
                let type_hint = format!("offset 0x{:08x}", offset);
                eprintln!("  Failed {}: {}", type_hint, e);
                failed_types.push(type_hint);
                failed += 1;
            }
        }
    }

    println!(
        "\nExtracted {} chunks to {} ({} failed)",
        success,
        output_dir.display(),
        failed
    );

    // Show warning about failed files when using oozextract
    if failed > 0 && !decompressor.is_full_support() {
        eprintln!("\nWarning: {} files failed to decompress.", failed);
        eprintln!(
            "The oozextract backend does not support all Oodle compression variants."
        );
        #[cfg(target_os = "windows")]
        eprintln!("To decompress all files, use --oodle-dll <path-to-oo2core_9_win64.dll>");
        #[cfg(not(target_os = "windows"))]
        eprintln!("To decompress all files, use --oodle-exec <decompression-command>");
        eprintln!("\nFailed files:");
        for t in &failed_types {
            eprintln!("  - {}", t);
        }
    }

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

fn debug_file(path: &Path, show_hex: bool, do_parse: bool, show_offsets: bool) -> Result<()> {
    use bl4_ncs::{
        parse_header, parse_string_table, parse_binary_section, bit_width, BitReader,
        extract_inline_strings, extract_field_abbreviation, create_combined_string_table,
        find_packed_strings, UnpackedValue,
    };

    let data = fs::read(path).context("Failed to read file")?;
    println!("File: {}", path.display());
    println!("Size: {} bytes", data.len());

    // Parse header
    let header = parse_header(&data).context("Failed to parse header")?;
    println!("\n=== Header ===");
    println!("Type: {}", header.type_name);
    println!("Format: {}", header.format_code);
    println!("Field count: {}", header.field_count);

    if show_offsets {
        println!("\n=== Offsets ===");
        println!("Type offset: 0x{:x}", header.type_offset);
        println!("Format offset: 0x{:x}", header.format_offset);
        println!("Entry section: 0x{:x}", header.entry_section_offset);
        println!("String table: 0x{:x}", header.string_table_offset);
        if let Some(ctrl) = header.control_section_offset {
            println!("Control section: 0x{:x}", ctrl);
        }
        if let Some(cat) = header.category_names_offset {
            println!("Category names: 0x{:x}", cat);
        }
        println!("Binary section: 0x{:x}", header.binary_offset);
        if let Some(sc) = header.string_count {
            println!("String count (from header): {}", sc);
        }
    }

    // Parse string table
    let strings = parse_string_table(&data, &header);
    println!("\n=== String Table ({} strings) ===", strings.len());
    for (i, s) in strings.strings.iter().enumerate().take(20) {
        println!("  {:3}: {}", i, s);
    }
    if strings.len() > 20 {
        println!("  ... and {} more", strings.len() - 20);
    }

    // Show packed strings
    let packed = find_packed_strings(&strings.strings);
    if !packed.is_empty() {
        println!("\n=== Packed Strings ({} found) ===", packed.len());
        for unpacked in packed.iter().take(10) {
            let values_str: Vec<String> = unpacked.values.iter().map(|v| match v {
                UnpackedValue::Integer(n) => format!("int({})", n),
                UnpackedValue::Float(f) => format!("float({})", f),
                UnpackedValue::String(s) => format!("str(\"{}\")", s),
                UnpackedValue::Boolean(b) => format!("bool({})", b),
            }).collect();
            println!("  \"{}\" -> [{}]", unpacked.original, values_str.join(", "));
        }
        if packed.len() > 10 {
            println!("  ... and {} more", packed.len() - 10);
        }
    }

    // Find section markers
    println!("\n=== Section Markers ===");
    for i in 0..data.len().saturating_sub(3) {
        if data[i] != 0 && data[i+1] != 0 && data[i+2] == 0 && data[i+3] == 0 {
            if i > header.string_table_offset {
                println!("  0x{:03x}: {:02x} {:02x} 00 00", i, data[i], data[i+1]);
            }
        }
    }

    // Find 0x7a marker
    for i in 0..data.len().saturating_sub(5) {
        if data[i..i+6] == [0x7a, 0x00, 0x00, 0x00, 0x00, 0x00] {
            println!("  0x{:03x}: 7a 00 00 00 00 00 (section divider)", i);
        }
    }

    // Try reading from first section marker
    // Find first XX XX 00 00 pattern after string table
    let string_bits = bit_width(strings.len() as u32);
    println!("\n=== Entry Data Test (from first marker) ===");
    for i in header.string_table_offset..data.len().saturating_sub(3) {
        if data[i] != 0 && data[i+1] != 0 && data[i+2] == 0 && data[i+3] == 0 {
            println!("Testing offset 0x{:x}:", i);
            let test_data = &data[i..];
            let mut reader = BitReader::new(test_data);
            print!("  As {}-bit indices: ", string_bits);
            for _ in 0..8 {
                if let Some(v) = reader.read_bits(string_bits) {
                    let valid = (v as usize) < strings.len();
                    if valid {
                        print!("{} ", v);
                    } else {
                        print!("({}) ", v);
                    }
                }
            }
            println!();
            break;
        }
    }

    // Extract inline strings (category names) and field abbreviation
    let inline_strings = extract_inline_strings(&data, &header, strings.len());
    let field_abbrev = extract_field_abbreviation(&data, &header);

    // Build combined string table: primary + inline + field abbreviation + type name
    let mut all_inline = inline_strings.clone();
    if let Some(ref abbrev) = field_abbrev {
        all_inline.push(abbrev.clone());
    }
    // Add type name as final string (may be referenced by table_id)
    all_inline.push(header.type_name.clone());
    let combined_strings = create_combined_string_table(&strings, &all_inline);

    if !inline_strings.is_empty() || field_abbrev.is_some() {
        println!("\n=== Inline Strings ===");
        let mut idx = strings.len();
        for s in inline_strings.iter() {
            println!("  {:3}: {} (category)", idx, s);
            idx += 1;
        }
        if let Some(ref abbrev) = field_abbrev {
            println!("  {:3}: {} (field abbrev)", idx, abbrev);
            idx += 1;
        }
        println!("  {:3}: {} (type name)", idx, header.type_name);
    }

    let total_strings = combined_strings.len();
    let total_string_bits = bit_width(total_strings as u32);

    // Binary section analysis
    if header.binary_offset < data.len() {
        let binary_data = &data[header.binary_offset..];
        println!("\n=== Binary Section ===");
        println!("Starts at: 0x{:x}", header.binary_offset);
        println!("Length: {} bytes", binary_data.len());
        println!("Primary strings: {} ({} bits)", strings.len(), string_bits);
        println!("Total strings (with inline): {} ({} bits)", total_strings, total_string_bits);

        if show_hex {
            println!("\nFirst 64 bytes:");
            print_hex(&binary_data[..binary_data.len().min(64)]);
        }

        // Try bit reading
        println!("\n=== Bit Reader Test ===");
        let mut reader1 = BitReader::new(binary_data);

        // Read first few values different ways
        println!("Reading as bytes:");
        for i in 0..8.min(binary_data.len()) {
            let v = reader1.read_bits(8);
            if let Some(v) = v {
                let c = if (32..127).contains(&v) { v as u8 as char } else { '.' };
                println!("  Byte {}: 0x{:02x} ({:3}) '{}'", i, v, v, c);
            }
        }

        // Read with total_string_bits (including inline strings)
        let mut reader3 = BitReader::new(binary_data);
        println!("\nReading {} bit values (combined strings):", total_string_bits);
        for i in 0..10 {
            let v = reader3.read_bits(total_string_bits);
            if let Some(v) = v {
                let s = combined_strings.strings.get(v as usize).map(|s| s.as_str()).unwrap_or("(oob)");
                println!("  Value {}: {} -> {:?}", i, v, s);
            }
        }

        if do_parse {
            println!("\n=== Binary Parse Attempt ===");
            // Use combined string table for binary parsing
            match parse_binary_section(&data, header.binary_offset, &combined_strings) {
                Some(result) => {
                    println!("table_id: {} -> {:?}", result.table_id,
                        combined_strings.strings.get(result.table_id as usize));
                    println!("bit_indices: {} values", result.bit_indices.len());

                    // Show first few bit indices with string lookups
                    println!("\nFirst 20 bit-packed indices:");
                    for (i, &idx) in result.bit_indices.iter().take(20).enumerate() {
                        let s = combined_strings.strings.get(idx as usize)
                            .map(|s| s.as_str())
                            .unwrap_or("(oob)");
                        let marker = if idx as usize >= combined_strings.len() { "*" } else { "" };
                        println!("  [{:2}] {:2} -> {}{}", i, idx, s, marker);
                    }
                    if result.bit_indices.len() > 20 {
                        println!("  ... and {} more", result.bit_indices.len() - 20);
                    }

                    // Show entry groups
                    println!("\nEntry groups: {} found (matching entries)", result.entry_groups.len());
                    for (i, group) in result.entry_groups.iter().enumerate().take(10) {
                        println!("  Entry {}: values={:?}", i, group.values);
                    }
                    if result.entry_groups.len() > 10 {
                        println!("  ... and {} more entries", result.entry_groups.len() - 10);
                    }

                    // Show tail data
                    if !result.tail_data.is_empty() {
                        println!("\nTail data: {} bytes", result.tail_data.len());
                        let preview: Vec<String> = result.tail_data.iter().take(32)
                            .map(|b| format!("{:02x}", b)).collect();
                        println!("  {}", preview.join(" "));
                    }
                }
                None => {
                    println!("Failed to parse binary section");
                }
            }
        }
    }

    Ok(())
}
