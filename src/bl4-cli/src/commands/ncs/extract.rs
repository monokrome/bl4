//! NCS extract command - comprehensive extraction from NCS files
//!
//! Extracts all known data types from NCS files, either from:
//! - Pre-extracted .ncs/.bin files in a directory
//! - Directly from PAK files (streaming)

use anyhow::{Context, Result};
use bl4_ncs::{NcsContent, parse_header, parse_ncs_string_table};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use uextract::pak::{find_pak_files, PakReader};

/// Known NCS types that we can fully parse
const KNOWN_TYPES: &[&str] = &[
    "inv",
    "itempoollist",
    "itempool",
    "manufacturer",
    "rarity",
    "trait_pool",
    "damage_modifier",
    "damage_affinity",
    "DamageType",
    "DamageData",
    "DamageSource",
    "attribute",
    "achievement",
    "challenge",
    "vending_machine",
];

/// NCS types we can partially parse (string table only)
const PARTIAL_TYPES: &[&str] = &[
    "camera_mode",
    "camera_shake",
    "audio_event",
    "character_info",
    "credits",
    "cinematic_mode",
];

/// Extraction statistics
#[derive(Default)]
struct ExtractionStats {
    total_files: usize,
    parsed_ok: usize,
    parsed_partial: usize,
    unknown_type: usize,
    failed: usize,
    types_seen: BTreeMap<String, usize>,
    format_codes_seen: BTreeMap<String, usize>,
    unknown_types: BTreeSet<String>,
}

/// Result of extracting a single NCS file
struct ExtractedFile {
    type_name: String,
    format_code: String,
    entry_count: usize,
    strings: Vec<String>,
    status: ExtractStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExtractStatus {
    FullyParsed,
    PartiallyParsed,
    UnknownType,
    ParseFailed,
}

/// Main extraction entry point
pub fn extract_all(
    path: &Path,
    filter_type: Option<&str>,
    output: Option<&Path>,
    from_pak: bool,
    json: bool,
    verbose: bool,
) -> Result<()> {
    let mut stats = ExtractionStats::default();
    let mut all_extracted: Vec<ExtractedFile> = Vec::new();

    if from_pak {
        extract_from_pak_files(path, filter_type, &mut stats, &mut all_extracted, verbose)?;
    } else {
        extract_from_directory(path, filter_type, &mut stats, &mut all_extracted, verbose)?;
    }

    // Output results
    output_results(&stats, &all_extracted, output, json, verbose)?;

    // Print warnings for unknown types
    print_warnings(&stats);

    Ok(())
}

/// Extract NCS data directly from PAK files
fn extract_from_pak_files(
    paks_dir: &Path,
    filter_type: Option<&str>,
    stats: &mut ExtractionStats,
    extracted: &mut Vec<ExtractedFile>,
    verbose: bool,
) -> Result<()> {
    let pak_files = find_pak_files(paks_dir)?;

    if pak_files.is_empty() {
        anyhow::bail!("No PAK files found in {}", paks_dir.display());
    }

    eprintln!("Scanning {} PAK files for NCS data...", pak_files.len());

    for pak_path in &pak_files {
        let mut reader = match PakReader::open(pak_path) {
            Ok(r) => r,
            Err(e) => {
                if verbose {
                    eprintln!("  Skipping {:?}: {}", pak_path.file_name().unwrap_or_default(), e);
                }
                continue;
            }
        };

        let ncs_files = reader.files_with_extension("ncs");
        if ncs_files.is_empty() {
            continue;
        }

        if verbose {
            eprintln!("  {:?}: {} NCS files", pak_path.file_name().unwrap_or_default(), ncs_files.len());
        }

        for filename in &ncs_files {
            stats.total_files += 1;

            let raw_data = match reader.read(filename) {
                Ok(d) => d,
                Err(e) => {
                    if verbose {
                        eprintln!("    Failed to read {}: {}", filename, e);
                    }
                    stats.failed += 1;
                    continue;
                }
            };

            // Decompress NCS
            let data = match bl4_ncs::decompress_ncs(&raw_data) {
                Ok(d) => d,
                Err(e) => {
                    if verbose {
                        eprintln!("    Failed to decompress {}: {}", filename, e);
                    }
                    stats.failed += 1;
                    continue;
                }
            };

            // Extract type name from filename
            let type_name = bl4_ncs::type_from_filename(filename);

            // Apply filter if specified
            if let Some(filter) = filter_type {
                if !type_name.eq_ignore_ascii_case(filter) {
                    continue;
                }
            }

            process_ncs_data(&data, &type_name, stats, extracted, verbose);
        }
    }

    Ok(())
}

/// Extract NCS data from a directory of pre-extracted files
fn extract_from_directory(
    dir: &Path,
    filter_type: Option<&str>,
    stats: &mut ExtractionStats,
    extracted: &mut Vec<ExtractedFile>,
    verbose: bool,
) -> Result<()> {
    eprintln!("Scanning directory for NCS files: {}", dir.display());

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();

        // Accept .ncs or .bin files
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "ncs" && ext != "bin" {
            continue;
        }

        stats.total_files += 1;

        let data = match fs::read(file_path) {
            Ok(d) => d,
            Err(e) => {
                if verbose {
                    eprintln!("  Failed to read {:?}: {}", file_path, e);
                }
                stats.failed += 1;
                continue;
            }
        };

        // Get type name from filename
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let type_name = bl4_ncs::type_from_filename(filename);

        // Apply filter if specified
        if let Some(filter) = filter_type {
            if !type_name.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        process_ncs_data(&data, &type_name, stats, extracted, verbose);
    }

    Ok(())
}

/// Process a single NCS file's data
fn process_ncs_data(
    data: &[u8],
    type_name: &str,
    stats: &mut ExtractionStats,
    extracted: &mut Vec<ExtractedFile>,
    verbose: bool,
) {
    // Try to parse the content
    let content = NcsContent::parse(data);

    let (format_code, strings, status): (String, Vec<String>, ExtractStatus) = if let Some(ref c) = content {
        let fc = c.format_code().to_string();
        let strs: Vec<String> = c.strings.clone();

        // Determine parse status based on type
        let status = if KNOWN_TYPES.iter().any(|t| t.eq_ignore_ascii_case(type_name)) {
            ExtractStatus::FullyParsed
        } else if PARTIAL_TYPES.iter().any(|t| t.eq_ignore_ascii_case(type_name)) {
            ExtractStatus::PartiallyParsed
        } else {
            ExtractStatus::UnknownType
        };

        (fc, strs, status)
    } else {
        // Try basic header parsing
        match parse_header(data) {
            Some(header) => {
                let string_table = parse_ncs_string_table(data, &header);
                let strs: Vec<String> = string_table.strings.clone();
                let status = if KNOWN_TYPES.iter().any(|t| t.eq_ignore_ascii_case(type_name)) {
                    ExtractStatus::FullyParsed
                } else {
                    ExtractStatus::UnknownType
                };
                (header.format_code.clone(), strs, status)
            }
            None => {
                stats.failed += 1;
                if verbose {
                    eprintln!("  Failed to parse: {}", type_name);
                }
                return;
            }
        }
    };

    // Update stats
    *stats.types_seen.entry(type_name.to_string()).or_insert(0usize) += 1;
    *stats.format_codes_seen.entry(format_code.clone()).or_insert(0usize) += 1;

    match status {
        ExtractStatus::FullyParsed => stats.parsed_ok += 1,
        ExtractStatus::PartiallyParsed => stats.parsed_partial += 1,
        ExtractStatus::UnknownType => {
            stats.unknown_type += 1;
            stats.unknown_types.insert(type_name.to_string());
        }
        ExtractStatus::ParseFailed => stats.failed += 1,
    }

    if verbose {
        eprintln!(
            "  {} [{}] - {} strings, {:?}",
            type_name,
            format_code,
            strings.len(),
            status
        );
    }

    extracted.push(ExtractedFile {
        type_name: type_name.to_string(),
        format_code,
        entry_count: strings.len(),
        strings,
        status,
    });
}

/// Output extraction results
fn output_results(
    stats: &ExtractionStats,
    extracted: &[ExtractedFile],
    output: Option<&Path>,
    json: bool,
    _verbose: bool,
) -> Result<()> {
    // Create output directory if specified
    if let Some(out_dir) = output {
        fs::create_dir_all(out_dir)?;

        // Group by type and write separate files
        let mut by_type: BTreeMap<String, Vec<&ExtractedFile>> = BTreeMap::new();
        for file in extracted {
            by_type.entry(file.type_name.clone()).or_default().push(file);
        }

        for (type_name, files) in &by_type {
            let out_path = if json {
                out_dir.join(format!("{}.json", type_name))
            } else {
                out_dir.join(format!("{}.tsv", type_name))
            };

            let content = if json {
                let data: Vec<_> = files.iter().map(|f| {
                    serde_json::json!({
                        "type": f.type_name,
                        "format_code": f.format_code,
                        "entry_count": f.entry_count,
                        "strings": f.strings,
                    })
                }).collect();
                serde_json::to_string_pretty(&data)?
            } else {
                let mut lines = vec![format!("# Type: {} ({} files)", type_name, files.len())];
                for f in files {
                    lines.push(format!("# Format: {}, Entries: {}", f.format_code, f.entry_count));
                    for s in &f.strings {
                        lines.push(s.clone());
                    }
                }
                lines.join("\n")
            };

            fs::write(&out_path, content)?;
        }

        eprintln!("Wrote {} type files to {}", by_type.len(), out_dir.display());
    }

    // Print summary
    eprintln!("\n=== Extraction Summary ===");
    eprintln!("Total files:      {}", stats.total_files);
    eprintln!("Fully parsed:     {}", stats.parsed_ok);
    eprintln!("Partially parsed: {}", stats.parsed_partial);
    eprintln!("Unknown types:    {}", stats.unknown_type);
    eprintln!("Failed:           {}", stats.failed);
    eprintln!("\nTypes seen: {}", stats.types_seen.len());

    Ok(())
}

/// Print warnings about unknown types
fn print_warnings(stats: &ExtractionStats) {
    if !stats.unknown_types.is_empty() {
        eprintln!("\n=== WARNING: Unknown NCS Types ===");
        eprintln!("The following {} types have no dedicated parser:", stats.unknown_types.len());
        for type_name in &stats.unknown_types {
            let count = stats.types_seen.get(type_name).unwrap_or(&0);
            eprintln!("  - {} ({} files)", type_name, count);
        }
        eprintln!("\nThese files were processed for string tables only.");
        eprintln!("Binary data sections may contain additional structured data.");
    }

    // Show format code distribution for unknown types
    if stats.unknown_type > 0 {
        eprintln!("\nFormat codes seen:");
        for (code, count) in &stats.format_codes_seen {
            eprintln!("  {}: {} files", code, count);
        }
    }
}
