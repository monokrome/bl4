//! Manifest orchestration command handler
//!
//! Orchestrates full manifest generation from memory dump and pak files.

use crate::manifest;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

/// Extract and decompress NCS files from PAK files in a directory.
///
/// Finds all .pak files in the given directory and runs `bl4 ncs decompress`
/// on each, using the specified Oodle backend for full decompression support.
fn extract_ncs_from_paks(
    paks_dir: &Path,
    ncs_output: &Path,
    oodle_exec: Option<&str>,
    oodle_fifo: bool,
) -> Result<()> {
    println!("=== NCS Extraction ===\n");

    // Find all .pak files
    let mut pak_files: Vec<PathBuf> = Vec::new();
    if paks_dir.is_file() {
        pak_files.push(paks_dir.to_path_buf());
    } else if paks_dir.is_dir() {
        for entry in fs::read_dir(paks_dir).context("Failed to read paks directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "pak").unwrap_or(false) {
                pak_files.push(path);
            }
        }
        // Sort by UE5 mount priority: (patch_level, chunk) ascending.
        // Last-write-wins, so highest-priority PAK processes last and overrides.
        // Unknown filenames sort last (conservative — they win over everything).
        pak_files.sort_by(|a, b| {
            use uextract::pak::parse_pak_filename;
            let pa = parse_pak_filename(a);
            let pb = parse_pak_filename(b);
            match (pa, pb) {
                (Some(a), Some(b)) => (a.patch_level, a.chunk).cmp(&(b.patch_level, b.chunk)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.cmp(b),
            }
        });
    }

    if pak_files.is_empty() {
        println!("No .pak files found, skipping NCS extraction\n");
        return Ok(());
    }

    let bl4_exe = std::env::current_exe().context("Failed to get current executable path")?;
    fs::create_dir_all(ncs_output).context("Failed to create NCS output directory")?;

    let backend_name = if oodle_exec.is_some() {
        if oodle_fifo { "fifo-exec" } else { "exec" }
    } else {
        "oozextract"
    };
    println!(
        "Extracting NCS from {} PAK files (backend: {})...",
        pak_files.len(),
        backend_name
    );

    let mut total_extracted = 0;

    for pak_path in &pak_files {
        let mut cmd = ProcessCommand::new(&bl4_exe);
        cmd.args(["ncs", "decompress"])
            .arg(pak_path)
            .arg("-o")
            .arg(ncs_output)
            .arg("--raw");

        if let Some(exec_cmd) = oodle_exec {
            cmd.arg("--oodle-exec").arg(exec_cmd);
            if oodle_fifo {
                cmd.arg("--oodle-fifo");
            }
        }

        let status = cmd
            .status()
            .with_context(|| format!("Failed to run ncs decompress on {}", pak_path.display()))?;

        if status.success() {
            total_extracted += 1;
        } else {
            eprintln!(
                "  Warning: NCS extraction failed for {} (status: {})",
                pak_path.display(),
                status
            );
        }
    }

    println!(
        "  NCS extraction complete: {}/{} PAKs processed\n",
        total_extracted,
        pak_files.len()
    );

    Ok(())
}

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
    oodle_exec: Option<&str>,
    oodle_fifo: bool,
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
            let parts_db_path = output.join("parts");
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
            let part_pools_path = output.join("part_pools.tsv");
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

    // In-memory UAsset scanning
    if !skip_extract {
        println!("=== UAsset Scanning ===\n");

        // Generate scriptobjects.json (required for class resolution)
        let scriptobjects_path = output.join("scriptobjects.json");
        if !scriptobjects_path.exists() {
            print!("  Generating scriptobjects...");
            uextract::commands::extract_script_objects(paks, &scriptobjects_path, aes_key)?;
            println!(" done");
        }

        println!("  Scanning IoStore for game data assets...");
        match manifest::extract_uasset_manifest(
            paks,
            &usmap_path,
            &scriptobjects_path,
            output,
            aes_key,
        ) {
            Ok(summary) => {
                let total = summary.status_effects_count
                    + summary.skill_params_count
                    + summary.balance_structs_count;
                println!("  UAsset scanning complete: {} assets extracted\n", total);
            }
            Err(e) => {
                eprintln!("  Warning: UAsset scanning failed: {}\n", e);
            }
        }
    }

    // NCS extraction from PAK files
    let ncs_dir = output.join("ncs");
    extract_ncs_from_paks(paks, &ncs_dir, oodle_exec, oodle_fifo)?;

    // Generate manifest from extracted files
    println!("=== Manifest Generation ===\n");
    println!("Generating manifest files...");
    manifest::extract_manifest(&extract_dir, output)?;
    println!("\nManifest files written to {}", output.display());

    // Generate drops manifest from NCS data if available
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
