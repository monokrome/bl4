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
    if ncs_output.exists() {
        fs::remove_dir_all(ncs_output).context("Failed to clear existing NCS output directory")?;
    }
    fs::create_dir_all(ncs_output).context("Failed to create NCS output directory")?;

    let backend_name = if oodle_exec.is_some() {
        if oodle_fifo {
            "fifo-exec"
        } else {
            "exec"
        }
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

    // Memory dump extraction (usmap only)
    if !skip_memory {
        if let Some(dump_path) = dump {
            println!("=== Memory Dump Extraction ===\n");

            let bl4_exe =
                std::env::current_exe().context("Failed to get current executable path")?;

            if !usmap_provided {
                println!("Generating usmap from memory dump...");
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
                println!("  Wrote usmap to: {}\n", usmap_path.display());
            } else {
                println!("Using provided usmap: {}\n", usmap_path.display());
            }
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
                println!(
                    "  UAsset scanning complete: {} skill params, {} status effects, {} balance assets across {} categories\n",
                    summary.skill_params_count,
                    summary.status_effects_count,
                    summary.balance_assets,
                    summary.balance_categories,
                );
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

    // Generate data tables and drops manifest from NCS data if available
    if ncs_dir.exists() {
        // Extract data tables first (needed for boss name resolution in drops)
        println!("\n=== Data Tables ===\n");
        println!("Extracting UE data tables from NCS...");
        let data_tables = match bl4_ncs::extract_data_tables_from_dir(&ncs_dir) {
            Ok(dt_manifest) => {
                let dt_dir = output.join("data_tables");
                bl4_ncs::write_data_tables(&dt_manifest, &dt_dir)?;
                println!(
                    "  {} tables, {} rows → {}",
                    dt_manifest.len(),
                    dt_manifest.total_rows(),
                    dt_dir.display()
                );
                Some(dt_manifest)
            }
            Err(e) => {
                eprintln!("  Warning: Failed to extract data tables: {}", e);
                None
            }
        };

        // Generate drops manifest (uses data tables for boss names)
        println!("\n=== Drops Manifest ===\n");
        println!("Generating drops manifest from NCS data...");
        match bl4_ncs::generate_drops_manifest(&ncs_dir, data_tables.as_ref()) {
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
