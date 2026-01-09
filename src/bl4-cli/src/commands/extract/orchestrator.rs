//! Manifest orchestration command handler
//!
//! Orchestrates full manifest generation from memory dump and pak files.

use crate::manifest;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use uextract::pak::{find_pak_files, PakReader};

/// Handle the Commands::Manifest command
///
/// Orchestrates full manifest generation from memory dump and pak files.
#[allow(clippy::too_many_arguments)]
pub fn handle_manifest(
    dump: Option<&Path>,
    paks: &Path,
    usmap: Option<PathBuf>,
    output: &Path,
    aes_key: Option<&str>,
    skip_extract: bool,
    extracted: PathBuf,
    skip_memory: bool,
) -> Result<()> {
    // Ensure output directory exists
    fs::create_dir_all(output).context("Failed to create output directory")?;

    // Determine usmap path - either provided or generated from dump
    let usmap_provided = usmap.is_some();
    let usmap_path = if let Some(usmap) = usmap {
        usmap
    } else if dump.is_some() {
        output.join("BL4.usmap")
    } else {
        bail!("Either --usmap or --dump must be provided");
    };

    // Memory dump extraction (usmap, parts)
    if !skip_memory {
        if let Some(dump_path) = dump {
            println!("=== Memory Dump Extraction ===\n");

            // Find bl4 binary - use current exe
            let bl4_exe =
                std::env::current_exe().context("Failed to get current executable path")?;

            // Step 1: Generate usmap from dump
            if !usmap_provided {
                println!("Step 1: Generating usmap from memory dump...");
                let status = ProcessCommand::new(&bl4_exe)
                    .args(["memory", "-d"])
                    .arg(dump_path)
                    .args(["dump-usmap", "-o"])
                    .arg(&usmap_path)
                    .status()
                    .context("Failed to run bl4 memory dump-usmap")?;
                if !status.success() {
                    bail!("dump-usmap failed with status: {}", status);
                }
                println!("  Wrote usmap to: {}", usmap_path.display());
            } else {
                println!("Step 1: Using provided usmap: {}", usmap_path.display());
            }

            // Step 2: Extract parts with categories
            println!("\nStep 2: Extracting parts with categories...");
            let parts_with_cats_path = output.join("parts_with_categories.json");
            let status = ProcessCommand::new(&bl4_exe)
                .args(["memory", "-d"])
                .arg(dump_path)
                .args(["extract-parts", "-o"])
                .arg(&parts_with_cats_path)
                .status()
                .context("Failed to run bl4 memory extract-parts")?;
            if !status.success() {
                bail!("extract-parts failed with status: {}", status);
            }

            // Step 3: Dump raw parts
            println!("\nStep 3: Dumping raw parts...");
            let parts_dump_path = output.join("parts_dump.json");
            let status = ProcessCommand::new(&bl4_exe)
                .args(["memory", "-d"])
                .arg(dump_path)
                .args(["dump-parts", "-o"])
                .arg(&parts_dump_path)
                .status()
                .context("Failed to run bl4 memory dump-parts")?;
            if !status.success() {
                bail!("dump-parts failed with status: {}", status);
            }

            // Step 4: Generate part_categories.json from parts_with_categories.json
            println!("\nStep 4: Generating part categories mapping...");
            let categories_path = output.join("part_categories.json");

            // Read parts_with_categories.json and extract unique prefix -> category mappings
            let parts_json = fs::read_to_string(&parts_with_cats_path)
                .context("Failed to read parts_with_categories.json")?;
            let parts_data: serde_json::Value = serde_json::from_str(&parts_json)
                .context("Failed to parse parts_with_categories.json")?;

            let mut prefix_categories: std::collections::BTreeMap<String, i64> =
                std::collections::BTreeMap::new();

            if let Some(parts) = parts_data.get("parts").and_then(|p| p.as_array()) {
                for part in parts {
                    if let (Some(name), Some(category)) = (
                        part.get("name").and_then(|n| n.as_str()),
                        part.get("category").and_then(|c| c.as_i64()),
                    ) {
                        // Extract prefix (everything before the first dot)
                        if let Some(prefix) = name.split('.').next() {
                            prefix_categories
                                .entry(prefix.to_string())
                                .or_insert(category);
                        }
                    }
                }
            }

            // Write part_categories.json
            let categories: Vec<serde_json::Value> = prefix_categories
                .into_iter()
                .map(|(prefix, category)| {
                    serde_json::json!({
                        "prefix": prefix,
                        "category": category
                    })
                })
                .collect();

            let categories_json = serde_json::json!({ "categories": categories });
            fs::write(
                &categories_path,
                serde_json::to_string_pretty(&categories_json)?,
            )?;
            println!(
                "  Wrote {} category mappings to: {}",
                categories.len(),
                categories_path.display()
            );

            // Step 5: Build parts database
            println!("\nStep 5: Building parts database...");
            let parts_db_path = output.join("parts_database.json");
            let status = ProcessCommand::new(&bl4_exe)
                .args(["memory", "build-parts-db"])
                .args(["-i"])
                .arg(&parts_dump_path)
                .args(["-o"])
                .arg(&parts_db_path)
                .args(["-c"])
                .arg(&categories_path)
                .status()
                .context("Failed to run bl4 memory build-parts-db")?;
            if !status.success() {
                bail!("build-parts-db failed with status: {}", status);
            }

            // Step 6: Extract part pools from parts database
            println!("\nStep 6: Extracting part pools...");
            let part_pools_path = output.join("part_pools.json");
            let status = ProcessCommand::new(&bl4_exe)
                .args(["extract", "part-pools"])
                .args(["-i"])
                .arg(&parts_db_path)
                .args(["-o"])
                .arg(&part_pools_path)
                .status()
                .context("Failed to run bl4 extract part-pools")?;
            if !status.success() {
                bail!("extract part-pools failed with status: {}", status);
            }

            println!("\n=== Memory extraction complete ===\n");
        }
    }

    // Pak extraction
    let extract_dir = if skip_extract {
        extracted
    } else {
        // Run uextract to extract pak files
        println!("=== Pak Extraction ===\n");
        println!("Extracting pak files with uextract...");
        let mut cmd = ProcessCommand::new("uextract");
        cmd.arg(paks)
            .arg("-o")
            .arg(&extracted)
            .arg("--usmap")
            .arg(&usmap_path)
            .arg("--format")
            .arg("json");

        if let Some(key) = aes_key {
            cmd.arg("--aes-key").arg(key);
        }

        let status = cmd.status().context("Failed to run uextract")?;
        if !status.success() {
            bail!("uextract failed with status: {}", status);
        }
        println!();
        extracted
    };

    // Extract all files from traditional PAK archives
    println!("=== PAK Extraction ===\n");
    let pak_extract_dir = extract_dir.join("pak");
    extract_from_paks(paks, &pak_extract_dir)?;

    // Generate manifest from extracted files
    println!("=== Manifest Generation ===\n");
    println!("Generating manifest files...");
    manifest::extract_manifest(&extract_dir, output)?;
    println!("\nManifest files written to {}", output.display());

    // Generate drops manifest from NCS data if available
    // NCS files are extracted to pak/Engine/Content/_NCS/ as decompressed .ncs files
    let ncs_dir = pak_extract_dir.join("Engine/Content/_NCS");
    if ncs_dir.exists() {
        println!("\n=== Drops Manifest ===\n");
        println!("Generating drops manifest from NCS data...");
        match bl4_ncs::generate_drops_manifest(&ncs_dir) {
            Ok(drops_manifest) => {
                let drops_path = output.join("drops.json");
                let drops_json = serde_json::to_string_pretty(&drops_manifest)?;
                fs::write(&drops_path, drops_json)?;
                println!(
                    "  Wrote {} drops from {} sources to {}",
                    drops_manifest.drops.len(),
                    drops_manifest
                        .drops
                        .iter()
                        .map(|d| &d.source)
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    drops_path.display()
                );
            }
            Err(e) => {
                eprintln!("  Warning: Failed to generate drops manifest: {}", e);
            }
        }
    }

    Ok(())
}

/// Extract all files from traditional PAK archives
///
/// Searches for .pak files in the paks directory and extracts all files
/// using the PAK index. NCS files are decompressed automatically.
fn extract_from_paks(paks_dir: &Path, output: &Path) -> Result<()> {
    let pak_files = find_pak_files(paks_dir)?;

    if pak_files.is_empty() {
        println!("No traditional PAK files found in {:?}", paks_dir);
        return Ok(());
    }

    println!("Found {} PAK files to extract", pak_files.len());

    fs::create_dir_all(output)?;

    let mut total_extracted = 0;
    let mut total_failed = 0;
    let mut ncs_count = 0;

    for pak_path in &pak_files {
        let mut reader = match PakReader::open(pak_path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "  Warning: Skipping {:?}: {}",
                    pak_path.file_name().unwrap_or_default(),
                    e
                );
                continue;
            }
        };

        let all_files = reader.files();
        if all_files.is_empty() {
            continue;
        }

        println!(
            "  {:?}: {} files",
            pak_path.file_name().unwrap_or_default(),
            all_files.len()
        );

        for filename in &all_files {
            let raw_data = match reader.read(filename) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("    Failed to read {}: {}", filename, e);
                    total_failed += 1;
                    continue;
                }
            };

            // Clean up the path
            let clean_name = filename
                .trim_start_matches(reader.mount_point())
                .trim_start_matches('/')
                .trim_start_matches("../");

            let out_path = output.join(clean_name);

            // Handle NCS files specially - decompress them
            let is_ncs = filename.to_lowercase().ends_with(".ncs");
            let write_data = if is_ncs {
                ncs_count += 1;
                match bl4_ncs::decompress_ncs(&raw_data) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("    Failed to decompress {}: {}", filename, e);
                        total_failed += 1;
                        continue;
                    }
                }
            } else {
                raw_data
            };

            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&out_path, &write_data)?;
            total_extracted += 1;
        }
    }

    println!();
    println!(
        "Extracted {} files ({} NCS decompressed) to {:?}",
        total_extracted, ncs_count, output
    );

    if total_failed > 0 {
        eprintln!("Failed to extract {} files", total_failed);
    }

    Ok(())
}
