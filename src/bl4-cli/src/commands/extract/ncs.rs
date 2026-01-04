//! NCS file command handlers
//!
//! Handlers for checking, decompressing, and extracting NCS files.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Handle the ExtractCommand::NcsCheck command
///
/// Checks if a file is a valid NCS file.
pub fn handle_check(input: &Path) -> Result<()> {
    let data = fs::read(input)?;

    if bl4_ncs::is_ncs_manifest(&data) {
        let manifest = bl4_ncs::NcsManifest::parse(&data)?;
        println!("{:?}: valid NCS manifest file", input);
        println!("  Entry count: {}", manifest.entry_count);
        println!("  Entries found: {}", manifest.entries.len());
    } else if bl4_ncs::is_ncs(&data) {
        let header = bl4_ncs::NcsHeader::from_bytes(&data)?;
        println!("{:?}: valid NCS data file", input);
        println!("  Version: {}", header.version);
        println!("  Compressed: {}", header.is_compressed());
    } else {
        println!("{:?}: NOT a valid NCS file", input);
        if data.len() >= 5 {
            println!(
                "  Header bytes: {:02x} {:02x} {:02x} {:02x} {:02x}",
                data[0], data[1], data[2], data[3], data[4]
            );
        }
    }

    Ok(())
}

/// Handle the ExtractCommand::NcsDecompress command
///
/// Decompresses an NCS file.
pub fn handle_decompress(input: &Path, output: Option<std::path::PathBuf>) -> Result<()> {
    let data = fs::read(input)?;

    if bl4_ncs::is_ncs_manifest(&data) {
        let manifest = bl4_ncs::NcsManifest::parse(&data)?;
        println!("NCS manifest file (not compressed)");
        println!("  Entry count: {}", manifest.entry_count);
        println!("  Entries:");
        for entry in &manifest.entries {
            println!("    - {}", entry.filename);
        }
        return Ok(());
    }

    if !bl4_ncs::is_ncs(&data) {
        anyhow::bail!("Not a valid NCS file");
    }

    println!("Parsing NCS header...");
    let header = bl4_ncs::NcsHeader::from_bytes(&data)?;
    println!(
        "  Version: {}, compressed: {}",
        header.version,
        header.is_compressed()
    );
    println!("Decompressing...");
    let decompressed = bl4_ncs::decompress_ncs(&data)?;

    let out_path = output.unwrap_or_else(|| {
        let mut p = input.to_path_buf();
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let ext = p.extension().map(|e| e.to_string_lossy().to_string());
        let new_name = if let Some(ext) = ext {
            format!("{}.decompressed.{}", stem, ext)
        } else {
            format!("{}.decompressed", stem)
        };
        p.set_file_name(new_name);
        p
    });

    fs::write(&out_path, &decompressed)?;
    println!(
        "Decompressed {} bytes -> {} bytes",
        data.len(),
        decompressed.len()
    );
    println!("Written to: {:?}", out_path);

    Ok(())
}

/// Handle the ExtractCommand::NcsInfo command
///
/// Displays detailed information about an NCS file.
pub fn handle_info(input: &Path) -> Result<()> {
    let data = fs::read(input)?;

    if bl4_ncs::is_ncs_manifest(&data) {
        let manifest = bl4_ncs::NcsManifest::parse(&data)?;

        println!("NCS Manifest: {:?}", input);
        println!("  File size: {} bytes", data.len());
        println!("  Entry count (header): {}", manifest.entry_count);
        println!("  Entries found: {}", manifest.entries.len());
        println!();
        println!("  Referenced NCS files:");
        for entry in &manifest.entries {
            println!("    - {}", entry.filename);
        }
    } else if bl4_ncs::is_ncs(&data) {
        let header = bl4_ncs::NcsHeader::from_bytes(&data)?;

        println!("NCS Data File: {:?}", input);
        println!("  File size: {} bytes", data.len());
        println!("  Version: {}", header.version);
        println!("  Compressed: {}", header.is_compressed());

        if header.is_compressed() {
            match bl4_ncs::decompress_ncs(&data) {
                Ok(decompressed) => {
                    println!("  Decompressed size: {} bytes", decompressed.len());
                }
                Err(e) => {
                    println!("  Decompression failed: {}", e);
                }
            }
        }
    } else {
        println!("Unknown file format: {:?}", input);
        println!("  File size: {} bytes", data.len());
        if data.len() >= 8 {
            println!(
                "  Header bytes: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
            );
        }
    }

    Ok(())
}

/// Handle the ExtractCommand::NcsFind command
///
/// Searches for NCS files in a directory.
pub fn handle_find(path: &Path, recursive: bool) -> Result<()> {
    let mut found = 0;

    if recursive {
        search_dir(path, &mut found)?;
    } else {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.is_file() {
                check_file(&file_path, &mut found)?;
            }
        }
    }

    println!("\nFound {} NCS files", found);
    Ok(())
}

/// Recursively search for NCS files in a directory
fn search_dir(dir: &Path, found: &mut usize) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            search_dir(&path, found)?;
        } else if path.is_file() {
            check_file(&path, found)?;
        }
    }
    Ok(())
}

/// Check if a file is an NCS file and print info
fn check_file(path: &Path, found: &mut usize) -> Result<()> {
    use std::io::Read;
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut header = [0u8; 5];
    if reader.read_exact(&mut header).is_ok() {
        if bl4_ncs::is_ncs(&header) {
            println!("{}: NCS data", path.display());
            *found += 1;
        } else if bl4_ncs::is_ncs_manifest(&header) {
            println!("{}: NCS manifest", path.display());
            *found += 1;
        }
    }
    Ok(())
}

/// Extract NCS files using proper PAK index (finds all files including previously missing ones)
fn handle_extract_pak_index(input: &Path, output: &Path, do_decompress: bool) -> Result<()> {
    use bl4_ncs::{decompress_ncs, type_from_filename};
    use uextract::pak::PakReader;

    let mut reader = PakReader::open(input)?;
    let ncs_files = reader.files_with_extension("ncs");

    println!("Found {} NCS files in PAK index", ncs_files.len());

    fs::create_dir_all(output)?;

    let mut extracted = 0;
    let mut failed = 0;

    for (i, filename) in ncs_files.iter().enumerate() {
        let type_name = type_from_filename(filename);

        // Read raw data
        let raw_data = match reader.read(filename) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read {}: {}", filename, e);
                failed += 1;
                continue;
            }
        };

        // Optionally decompress
        let data = if do_decompress {
            match decompress_ncs(&raw_data) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to decompress {}: {}", filename, e);
                    failed += 1;
                    continue;
                }
            }
        } else {
            raw_data
        };

        // Output filename based on type
        let out_path = if do_decompress {
            output.join(format!("{}.bin", type_name))
        } else {
            output.join(format!("{}.ncs", type_name))
        };

        fs::write(&out_path, &data)?;
        extracted += 1;

        if extracted <= 10 {
            println!(
                "  {} -> {:?} ({} bytes)",
                type_name,
                out_path.file_name().unwrap(),
                data.len()
            );
        }

        if (i + 1) % 100 == 0 {
            println!("  Extracted {}/{}...", i + 1, ncs_files.len());
        }
    }

    println!();
    println!("Extracted: {} (from PAK index)", extracted);
    if failed > 0 {
        println!("Failed: {}", failed);
    }

    Ok(())
}

/// Handle the ExtractCommand::NcsScan command
///
/// Scans a file for embedded NCS chunks.
pub fn handle_scan(input: &Path, _all: bool) -> Result<()> {
    println!("Scanning {:?} for NCS chunks...", input);
    let data = fs::read(input)?;
    let file_size = data.len();

    let chunks = bl4_ncs::scan_for_ncs(&data);

    if chunks.is_empty() {
        println!("No NCS chunks found");
    } else {
        println!("Found {} NCS chunks:\n", chunks.len());
        for (i, (offset, header)) in chunks.iter().enumerate() {
            println!(
                "  [{:4}] offset: 0x{:08x} ({:>12}), version: {}, compressed: {:>8}, decompressed: {:>8}",
                i,
                offset,
                offset,
                header.version,
                header.compressed_size,
                header.decompressed_size
            );
        }
        println!();

        let total_compressed: u64 = chunks.iter().map(|(_, h)| h.compressed_size as u64).sum();
        let total_decompressed: u64 = chunks.iter().map(|(_, h)| h.decompressed_size as u64).sum();

        println!("Total compressed size: {} bytes", total_compressed);
        println!("Total decompressed size: {} bytes", total_decompressed);
        println!(
            "Coverage: {:.2}% of file",
            (total_compressed as f64 / file_size as f64) * 100.0
        );
    }

    Ok(())
}

/// Handle the ExtractCommand::NcsExtract command
///
/// Extracts NCS chunks from a file using proper PAK index.
pub fn handle_extract(input: &Path, output: &Path, decompress: bool) -> Result<()> {
    println!("Extracting NCS from {:?}...", input);

    // Try new PAK index-based extraction first (finds all files)
    if let Some(ext) = input.extension() {
        if ext == "pak" {
            return handle_extract_pak_index(input, output, decompress);
        }
    }

    // Fallback to magic byte scanning for non-PAK files
    let data = fs::read(input)?;

    // Use manifest-based extraction for proper correlation
    let result = bl4_ncs::extract_from_pak(&data);

    if result.files.is_empty() && result.orphan_chunks.is_empty() {
        println!("No NCS data found in this file");
        return Ok(());
    }

    println!(
        "Found {} NCS files, {} missing entries, {} orphan chunks",
        result.files.len(),
        result.missing_chunks.len(),
        result.orphan_chunks.len()
    );

    fs::create_dir_all(output)?;

    let mut extracted = 0;
    let mut failed = 0;

    for (i, file) in result.files.iter().enumerate() {
        // Decompress the chunk
        let decompressed = match file.decompress(&data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "Failed to decompress {}: {}",
                    file.filename, e
                );
                failed += 1;
                continue;
            }
        };

        // Use manifest filename (strip Nexus-Data- prefix for cleaner output)
        let clean_name = file.filename
            .strip_prefix("Nexus-Data-")
            .unwrap_or(&file.filename);

        let out_path = if decompress {
            output.join(format!("{}.bin", clean_name.strip_suffix(".ncs").unwrap_or(clean_name)))
        } else {
            output.join(clean_name)
        };

        // Write decompressed or raw
        let chunk_end = file.offset + file.header.total_size();
        let write_data = if decompress {
            decompressed
        } else {
            data[file.offset..chunk_end].to_vec()
        };

        fs::write(&out_path, &write_data)?;
        extracted += 1;

        if extracted <= 10 {
            println!("  {} -> {:?}", file.type_name, out_path.file_name().unwrap());
        }

        if (i + 1) % 100 == 0 {
            println!("  Extracted {}/{}...", i + 1, result.files.len());
        }
    }

    println!();
    println!("Extracted: {}", extracted);
    println!("Failed: {}", failed);

    if !result.missing_chunks.is_empty() {
        println!("\nMissing (manifest entries without data):");
        for entry in result.missing_chunks.iter().take(10) {
            println!("  - {}", entry.filename);
        }
        if result.missing_chunks.len() > 10 {
            println!("  ... and {} more", result.missing_chunks.len() - 10);
        }
    }

    if !result.orphan_chunks.is_empty() {
        println!(
            "\nWarning: {} orphan chunks (no manifest) - likely false positives",
            result.orphan_chunks.len()
        );
    }

    Ok(())
}

/// Build a map from lowercase type_name to clean manifest filename
///
/// Manifest entries like "Nexus-Data-achievement0.ncs" become:
/// - Key: "achievement" (lowercase type_name)
/// - Value: "achievement" (clean name for output file)
fn build_manifest_map(
    manifests: &[(usize, bl4_ncs::NcsManifest)],
) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for (_offset, manifest) in manifests {
        for entry in &manifest.entries {
            // Parse "Nexus-Data-{type_name}{N}.ncs" -> type_name
            if let Some(clean_name) = parse_manifest_filename(&entry.filename) {
                map.insert(clean_name.to_lowercase(), clean_name);
            }
        }
    }

    map
}

/// Parse manifest filename to extract clean type name
///
/// "Nexus-Data-achievement0.ncs" -> "achievement"
/// "Nexus-Data-ItemPoolList0.ncs" -> "ItemPoolList"
fn parse_manifest_filename(filename: &str) -> Option<String> {
    // Strip "Nexus-Data-" prefix
    let without_prefix = filename.strip_prefix("Nexus-Data-")?;

    // Strip ".ncs" suffix
    let without_ext = without_prefix.strip_suffix(".ncs")?;

    // Strip trailing digit(s) (pak number)
    let name = without_ext.trim_end_matches(|c: char| c.is_ascii_digit());

    if name.is_empty() {
        return None;
    }

    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_ncs_check_missing_file() {
        let result = handle_check(Path::new("/nonexistent/file.ncs"));
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ncs_decompress_missing_file() {
        let result = handle_decompress(Path::new("/nonexistent/file.ncs"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ncs_info_missing_file() {
        let result = handle_info(Path::new("/nonexistent/file.ncs"));
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ncs_find_missing_dir() {
        let result = handle_find(Path::new("/nonexistent/dir"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ncs_scan_missing_file() {
        let result = handle_scan(Path::new("/nonexistent/file.pak"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ncs_extract_missing_file() {
        let result = handle_extract(
            Path::new("/nonexistent/file.pak"),
            Path::new("/tmp/output"),
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_manifest_filename_basic() {
        assert_eq!(
            parse_manifest_filename("Nexus-Data-achievement0.ncs"),
            Some("achievement".to_string())
        );
    }

    #[test]
    fn test_parse_manifest_filename_mixed_case() {
        assert_eq!(
            parse_manifest_filename("Nexus-Data-ItemPoolList0.ncs"),
            Some("ItemPoolList".to_string())
        );
    }

    #[test]
    fn test_parse_manifest_filename_multi_digit() {
        assert_eq!(
            parse_manifest_filename("Nexus-Data-gbxactor123.ncs"),
            Some("gbxactor".to_string())
        );
    }

    #[test]
    fn test_parse_manifest_filename_underscore() {
        assert_eq!(
            parse_manifest_filename("Nexus-Data-damage_modifier0.ncs"),
            Some("damage_modifier".to_string())
        );
    }

    #[test]
    fn test_parse_manifest_filename_no_prefix() {
        assert_eq!(parse_manifest_filename("achievement0.ncs"), None);
    }

    #[test]
    fn test_parse_manifest_filename_no_suffix() {
        assert_eq!(parse_manifest_filename("Nexus-Data-achievement0"), None);
    }

    #[test]
    fn test_parse_manifest_filename_only_digits() {
        // "Nexus-Data-0.ncs" -> name would be empty after stripping digits
        assert_eq!(parse_manifest_filename("Nexus-Data-0.ncs"), None);
    }

    #[test]
    fn test_build_manifest_map_empty() {
        let manifests: Vec<(usize, bl4_ncs::NcsManifest)> = vec![];
        let map = build_manifest_map(&manifests);
        assert!(map.is_empty());
    }
}
