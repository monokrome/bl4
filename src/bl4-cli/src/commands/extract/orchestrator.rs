//! Manifest orchestration command handler
//!
//! Orchestrates full manifest generation from memory dump and pak files.

use crate::commands::extract::handle_part_pools;
use crate::commands::ncs::extract_by_type as extract_ncs_by_type;
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

fn handle_memory_dump(dump: Option<&Path>, usmap_path: &Path, usmap_provided: bool) -> Result<()> {
    let Some(dump_path) = dump else {
        return Ok(());
    };

    println!("=== Memory Dump Extraction ===\n");
    let bl4_exe = std::env::current_exe().context("Failed to get current executable path")?;

    if usmap_provided {
        println!("Using provided usmap: {}\n", usmap_path.display());
        return Ok(());
    }

    println!("Generating usmap from memory dump...");
    let status = ProcessCommand::new(&bl4_exe)
        .args(["memory", "-d"])
        .arg(dump_path)
        .args(["dump-usmap", "-o"])
        .arg(usmap_path)
        .status()
        .context("Failed to run bl4 memory dump-usmap")?;
    if !status.success() {
        bail!("dump-usmap failed with status: {}", status);
    }
    println!("  Wrote usmap to: {}\n", usmap_path.display());
    Ok(())
}

fn run_uextract(
    paks: &Path,
    output: &Path,
    usmap_path: &Path,
    aes_key: Option<&str>,
) -> Result<()> {
    println!("=== Pak Extraction ===\n");
    println!("Extracting pak files with uextract...");
    let mut cmd = ProcessCommand::new("uextract");
    cmd.arg(paks)
        .arg("-o")
        .arg(output)
        .arg("--usmap")
        .arg(usmap_path)
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
    Ok(())
}

fn scan_uassets(
    paks: &Path,
    usmap_path: &Path,
    output: &Path,
    aes_key: Option<&str>,
) -> Result<()> {
    println!("=== UAsset Scanning ===\n");
    let scriptobjects_path = output.join("scriptobjects.json");
    if !scriptobjects_path.exists() {
        print!("  Generating scriptobjects...");
        uextract::commands::extract_script_objects(paks, &scriptobjects_path, aes_key)?;
        println!(" done");
    }

    println!("  Scanning IoStore for game data assets...");
    match manifest::extract_uasset_manifest(paks, usmap_path, &scriptobjects_path, output, aes_key)
    {
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
    Ok(())
}

fn extract_drops_section(
    ncs_dir: &Path,
    data_tables: Option<&bl4_ncs::DataTableManifest>,
    output: &Path,
) -> Result<()> {
    println!("\n=== Drops Manifest ===\n");
    println!("Generating drops manifest from NCS data...");
    let drops_result = bl4_ncs::generate_drops_manifest(ncs_dir, data_tables);
    match &drops_result {
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

            println!("\n=== Drop Pools ===\n");
            println!("Generating drop pools summary...");
            let drop_pools_tsv = bl4_ncs::generate_drop_pools_tsv(drops_manifest);
            let drop_pools_path = output.join("drop_pools.tsv");
            fs::write(&drop_pools_path, &drop_pools_tsv)?;
            println!(
                "  Wrote drop pools summary to {}",
                drop_pools_path.display()
            );
        }
        Err(e) => {
            eprintln!("  Warning: Failed to generate drops manifest: {}", e);
        }
    }
    Ok(())
}

fn extract_parts_section(ncs_dir: &Path, output: &Path) -> Result<()> {
    println!("\n=== Parts Database ===\n");
    println!("Extracting categorized parts from NCS data...");
    if let Err(e) = extract_ncs_by_type(ncs_dir, "manifest", Some(output), false) {
        eprintln!("  Warning: Failed to extract parts manifest: {}", e);
    }

    let parts_dir = output.join("parts");
    if parts_dir.exists() {
        println!("\n=== Part Pools ===\n");
        println!("Generating part pools from parts database...");
        let part_pools_path = output.join("part_pools.tsv");
        if let Err(e) = handle_part_pools(&parts_dir, &part_pools_path) {
            eprintln!("  Warning: Failed to generate part pools: {}", e);
        }
    }

    println!("\n=== Mission Structures ===\n");
    println!("Extracting mission data from NCS...");
    if let Err(e) = extract_ncs_by_type(ncs_dir, "missions", Some(output), false) {
        eprintln!("  Warning: Failed to extract mission structures: {}", e);
    }

    Ok(())
}

fn extract_weapon_stats_section(ncs_dir: &Path, output: &Path) -> Result<()> {
    println!("\n=== Weapon Base Stats ===\n");
    println!("Extracting weapon base stat mappings from inv_stat files...");
    let stat_rows = bl4_ncs::extract_weapon_base_stats(ncs_dir);
    if stat_rows.is_empty() {
        eprintln!("  Warning: No weapon base stats found in inv_stat files");
    } else {
        let stats_path = output.join("weapon_base_stats.tsv");
        match bl4_ncs::inv_stat::write_tsv(&stat_rows, &stats_path) {
            Ok(()) => {
                let classes = stat_rows
                    .iter()
                    .map(|r| r.weapon_class.as_str())
                    .collect::<std::collections::HashSet<_>>();
                println!(
                    "  {} weapon stat mappings across {} weapon classes → {}",
                    stat_rows.len(),
                    classes.len(),
                    stats_path.display()
                );
            }
            Err(e) => eprintln!("  Warning: Failed to write weapon base stats: {}", e),
        }
    }
    Ok(())
}

fn process_ncs_data(ncs_dir: &Path, _extract_dir: &Path, output: &Path) -> Result<()> {
    println!("\n=== Data Tables ===\n");
    println!("Extracting UE data tables from NCS...");
    let data_tables = match bl4_ncs::extract_data_tables_from_dir(ncs_dir) {
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

    println!("\n=== Item Names ===\n");
    println!("Extracting item names from NCS data...");
    let item_name_entries = bl4_ncs::extract_item_names(ncs_dir);
    if item_name_entries.is_empty() {
        eprintln!("  Warning: No item names found in NCS data");
    } else {
        let names_path = output.join("item_names.tsv");
        match bl4_ncs::item_names::write_tsv(&item_name_entries, &names_path) {
            Ok(()) => {
                let name_map = bl4_ncs::item_names::build_name_map(&item_name_entries);
                println!(
                    "  {} entries ({} unique names) → {}",
                    item_name_entries.len(),
                    name_map.len(),
                    names_path.display()
                );
            }
            Err(e) => eprintln!("  Warning: Failed to write item names: {}", e),
        }
    }

    println!("\n=== Mission Names ===\n");
    println!("Extracting mission display names from NCS data...");
    let mission_name_entries = bl4_ncs::extract_mission_names(ncs_dir);
    if mission_name_entries.is_empty() {
        eprintln!("  Warning: No mission names found in NCS data");
    } else {
        let missions_dir = output.join("missions");
        fs::create_dir_all(&missions_dir).ok();
        let names_path = missions_dir.join("mission_names.tsv");
        match bl4_ncs::mission_names::write_tsv(&mission_name_entries, &names_path) {
            Ok(()) => println!(
                "  {} mission names → {}",
                mission_name_entries.len(),
                names_path.display()
            ),
            Err(e) => eprintln!("  Warning: Failed to write mission names: {}", e),
        }
    }

    extract_drops_section(ncs_dir, data_tables.as_ref(), output)?;

    println!("\n=== Skill Trees ===\n");
    println!("Extracting skill trees from NCS data...");
    let skill_tree_entries = bl4_ncs::extract_skill_trees(ncs_dir);
    if skill_tree_entries.is_empty() {
        eprintln!("  Warning: No skill tree entries found in NCS data");
    } else {
        let st_path = output.join("skill_trees.tsv");
        match bl4_ncs::skill_trees::write_tsv(&skill_tree_entries, &st_path) {
            Ok(()) => {
                let categories = skill_tree_entries
                    .iter()
                    .map(|e| e.category)
                    .collect::<std::collections::HashSet<_>>();
                println!(
                    "  {} entries across {} categories → {}",
                    skill_tree_entries.len(),
                    categories.len(),
                    st_path.display()
                );
            }
            Err(e) => eprintln!("  Warning: Failed to write skill trees: {}", e),
        }
    }

    println!("\n=== Tooltips ===\n");
    println!("Extracting tooltip display names from NCS data...");
    let tooltip_entries = bl4_ncs::extract_tooltips(ncs_dir);
    if tooltip_entries.is_empty() {
        eprintln!("  Warning: No tooltips found in NCS data");
    } else {
        let tt_path = output.join("tooltips.tsv");
        match bl4_ncs::tooltips::write_tsv(&tooltip_entries, &tt_path) {
            Ok(()) => println!(
                "  {} tooltips → {}",
                tooltip_entries.len(),
                tt_path.display()
            ),
            Err(e) => eprintln!("  Warning: Failed to write tooltips: {}", e),
        }
    }

    extract_parts_section(ncs_dir, output)?;
    extract_weapon_stats_section(ncs_dir, output)?;

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
    let output = output.components().collect::<PathBuf>();
    fs::create_dir_all(&output).context("Failed to create output directory")?;

    let usmap_provided = usmap.is_some();
    let usmap_path = if let Some(usmap) = usmap {
        usmap
    } else if dump.is_some() {
        output.join("BL4.usmap")
    } else {
        bail!("Either --usmap or --dump must be provided");
    };

    if !skip_memory {
        handle_memory_dump(dump, &usmap_path, usmap_provided)?;
    }

    let extract_dir = if skip_extract {
        extracted
    } else {
        run_uextract(paks, &extracted, &usmap_path, aes_key)?;
        extracted
    };

    if !skip_extract {
        scan_uassets(paks, &usmap_path, &output, aes_key)?;
    }

    let ncs_dir = output.join("ncs");
    extract_ncs_from_paks(paks, &ncs_dir, oodle_exec, oodle_fifo)?;

    println!("=== Manifest Generation ===\n");
    println!("Generating manifest files...");
    manifest::extract_manifest(&extract_dir, &output)?;
    println!("\nManifest files written to {}", output.display());

    if ncs_dir.exists() {
        process_ncs_data(&ncs_dir, &extract_dir, &output)?;
    }

    Ok(())
}
