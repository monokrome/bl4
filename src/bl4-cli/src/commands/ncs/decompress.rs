//! NCS decompression command handlers

use anyhow::{Context, Result};
use bl4_ncs::oodle::{self, OodleDecompressor};
use bl4_ncs::{decompress_ncs_with, is_ncs, parse_document, NcsContent};
use std::fs;
use std::path::{Path, PathBuf};

use super::format::format_tsv;
use super::util::print_hex;

#[cfg(target_os = "windows")]
pub fn decompress_file(
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
pub fn decompress_file(
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

    // Try PAK index-based extraction first for .pak files (finds all NCS files)
    if input.extension().map(|e| e == "pak").unwrap_or(false) {
        return decompress_pak_index(input, output, raw, decompressor);
    }

    // Fallback: Scan for NCS chunks via magic bytes (for non-PAK files)
    let chunks = scan_for_ncs(&data);
    if chunks.is_empty() {
        anyhow::bail!("No NCS chunks found in file");
    }

    println!(
        "Found {} NCS chunks via magic scan (using {} backend)",
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

/// Extract NCS files from PAK using proper index (finds all files including previously missing ones)
fn decompress_pak_index(
    input: &Path,
    output: Option<&Path>,
    raw: bool,
    _decompressor: Box<dyn OodleDecompressor>,
) -> Result<()> {
    use bl4_ncs::{decompress_ncs, type_from_filename};
    use uextract::pak::PakReader;

    let mut reader = PakReader::open(input)?;
    let ncs_files = reader.files_with_extension("ncs");

    println!(
        "Found {} NCS files in PAK index (repak with oodle support)",
        ncs_files.len()
    );

    let output_dir = output.map(Path::to_path_buf).unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        PathBuf::from(format!("{}_ncs", stem))
    });
    fs::create_dir_all(&output_dir)?;

    let mut success = 0;
    let mut failed = 0;
    let mut failed_types: Vec<String> = Vec::new();

    for filename in &ncs_files {
        let type_name = type_from_filename(filename);

        // Read raw data from PAK
        let raw_data = match reader.read(filename) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Failed to read {}: {}", type_name, e);
                failed_types.push(type_name);
                failed += 1;
                continue;
            }
        };

        // Decompress NCS data
        let decompressed = match decompress_ncs(&raw_data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  Failed to decompress {}: {}", type_name, e);
                failed_types.push(type_name);
                failed += 1;
                continue;
            }
        };

        if raw {
            // Raw mode: save binary
            let out_path = output_dir.join(format!("{}.bin", type_name));
            fs::write(&out_path, &decompressed)?;
            success += 1;
        } else if let Some(doc) = parse_document(&decompressed) {
            // Parse and output as TSV
            let out_path = output_dir.join(format!("{}.tsv", doc.type_name));
            let tsv_content = format_tsv(&doc);
            fs::write(&out_path, &tsv_content)?;
            success += 1;
        } else if let Some(content) = NcsContent::parse(&decompressed) {
            // Fallback: couldn't parse structure, save raw
            let out_path = output_dir.join(format!("{}.bin", content.type_name()));
            fs::write(&out_path, &decompressed)?;
            success += 1;
        } else {
            // Fallback: save raw binary with type from filename
            let out_path = output_dir.join(format!("{}.bin", type_name));
            fs::write(&out_path, &decompressed)?;
            success += 1;
        }
    }

    println!(
        "\nExtracted {} files to {} ({} failed)",
        success,
        output_dir.display(),
        failed
    );

    if !failed_types.is_empty() {
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
