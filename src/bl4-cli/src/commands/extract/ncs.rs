//! NCS file command handlers
//!
//! Handlers for checking, decompressing, and extracting NCS files.

use anyhow::Result;
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
    } else if bl4_ncs::is_gbx(&data) {
        let variant = bl4_ncs::legacy::get_variant(&data);
        println!("{:?}: valid gBx file (legacy)", input);
        if let Some(v) = variant {
            println!("  Variant: {:?}", v);
        }
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
/// Decompresses an NCS or gBx file.
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

    let decompressed = if bl4_ncs::is_ncs(&data) {
        println!("Parsing NCS header...");
        let header = bl4_ncs::NcsHeader::from_bytes(&data)?;
        println!(
            "  Version: {}, compressed: {}",
            header.version,
            header.is_compressed()
        );
        println!("Decompressing...");
        bl4_ncs::decompress_ncs(&data)?
    } else {
        println!("Parsing gBx header (legacy)...");
        bl4_ncs::decompress_gbx(&data)?
    };

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
    } else if bl4_ncs::is_gbx(&data) {
        let variant = bl4_ncs::legacy::get_variant(&data);

        println!("gBx File (legacy): {:?}", input);
        println!("  File size: {} bytes", data.len());
        if let Some(v) = variant {
            println!("  Variant: {:?}", v);
        }

        match bl4_ncs::decompress_gbx(&data) {
            Ok(decompressed) => {
                println!("  Decompressed size: {} bytes", decompressed.len());
            }
            Err(e) => {
                println!("  Decompression failed: {}", e);
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
        } else if bl4_ncs::is_gbx(&header) {
            let variant = bl4_ncs::legacy::get_variant(&header);
            println!("{}: gBx {:?}", path.display(), variant);
            *found += 1;
        }
    }
    Ok(())
}

/// Handle the ExtractCommand::NcsScan command
///
/// Scans a file for embedded NCS chunks.
pub fn handle_scan(input: &Path, all: bool) -> Result<()> {
    use std::io::BufReader;

    println!("Scanning {:?} for NCS chunks...", input);
    let file = fs::File::open(input)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    if all {
        let results = bl4_ncs::scan_for_gbx_all(&mut reader)?;
        if results.is_empty() {
            println!("No NCS magic found");
        } else {
            println!("Found {} NCS magic occurrences:\n", results.len());
            for (i, r) in results.iter().enumerate() {
                let status = if r.valid { "VALID" } else { "INVALID" };
                println!(
                    "  [{:4}] offset: 0x{:08x} ({:>12}), variant: {:?}, compressed: {:>12}, decompressed: {:>12} [{}]",
                    i,
                    r.offset,
                    r.offset,
                    r.variant,
                    r.compressed_size,
                    r.decompressed_size,
                    status
                );
                if let Some(reason) = &r.invalid_reason {
                    println!("         └─ {}", reason);
                }
            }
            println!();
            let valid_count = results.iter().filter(|r| r.valid).count();
            println!(
                "Valid: {}, Invalid: {}",
                valid_count,
                results.len() - valid_count
            );
        }
    } else {
        let chunks = bl4_ncs::scan_for_gbx(&mut reader)?;

        if chunks.is_empty() {
            println!("No NCS chunks found");
        } else {
            println!("Found {} NCS chunks:\n", chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                println!(
                    "  [{:4}] offset: 0x{:08x} ({:>12}), variant: {:?}, compressed: {:>8}, decompressed: {:>8}",
                    i,
                    chunk.offset,
                    chunk.offset,
                    chunk.header.variant,
                    chunk.header.compressed_size,
                    chunk.header.decompressed_size
                );
            }
            println!();

            let total_compressed: u64 = chunks
                .iter()
                .map(|c| c.header.compressed_size as u64)
                .sum();
            let total_decompressed: u64 = chunks
                .iter()
                .map(|c| c.header.decompressed_size as u64)
                .sum();

            println!("Total compressed size: {} bytes", total_compressed);
            println!("Total decompressed size: {} bytes", total_decompressed);
            println!(
                "Coverage: {:.2}% of file",
                (total_compressed as f64 / file_size as f64) * 100.0
            );
        }
    }

    Ok(())
}

/// Handle the ExtractCommand::NcsExtract command
///
/// Extracts NCS chunks from a file.
pub fn handle_extract(input: &Path, output: &Path, decompress: bool) -> Result<()> {
    use std::io::{BufReader, Seek, SeekFrom};

    println!("Scanning {:?} for NCS chunks...", input);
    let file = fs::File::open(input)?;
    let mut reader = BufReader::new(file);

    let chunks = bl4_ncs::scan_for_gbx(&mut reader)?;
    println!("Found {} NCS chunks", chunks.len());

    fs::create_dir_all(output)?;

    let mut extracted = 0;
    let mut failed = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        reader.seek(SeekFrom::Start(chunk.offset))?;
        let data = bl4_ncs::extract_gbx_chunk(&mut reader, chunk)?;

        let filename = if decompress {
            format!("chunk_{:05}.ncs", i)
        } else {
            format!("chunk_{:05}.gbx", i)
        };
        let out_path = output.join(&filename);

        let write_data = if decompress {
            match bl4_ncs::decompress_gbx(&data) {
                Ok(decompressed) => decompressed,
                Err(e) => {
                    eprintln!("Failed to decompress chunk {}: {}", i, e);
                    failed += 1;
                    continue;
                }
            }
        } else {
            data
        };

        fs::write(&out_path, &write_data)?;
        extracted += 1;

        if (i + 1) % 100 == 0 {
            println!("  Extracted {}/{} chunks...", i + 1, chunks.len());
        }
    }

    println!("\nExtracted: {}, Failed: {}", extracted, failed);

    Ok(())
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
}
