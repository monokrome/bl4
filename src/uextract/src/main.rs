//! uextract - Unreal Engine IoStore asset extractor

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use retoc::{
    container_header::EIoContainerHeaderVersion, iostore, AesKey, Config, EIoStoreTocVersion,
    FGuid,
};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use usmap::Usmap;

mod cli;
mod commands;
mod filter;
pub mod pak;
mod property;
pub mod texture;
mod types;
mod zen;

use cli::{Args, Commands, OutputFormat};
use filter::matches_filters;
use zen::parse_zen_to_json;

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(command) = args.command {
        return match command {
            Commands::Pak {
                input,
                output,
                extension,
                filter,
                list,
                verbose,
            } => commands::extract_pak(
                &input,
                &output,
                extension.as_deref(),
                &filter,
                list,
                verbose,
            ),
            Commands::Texture {
                ubulk,
                width,
                height,
                output,
                mip,
                format,
            } => commands::extract_texture_cmd(&ubulk, width, height, &output, mip, &format),
            Commands::ScriptObjects {
                input,
                output,
                aes_key,
            } => commands::extract_script_objects(&input, &output, aes_key.as_deref()),
            Commands::FindByClass {
                input,
                class_name,
                scriptobjects,
                aes_key,
                output,
            } => commands::find_assets_by_class(
                &input,
                &class_name,
                &scriptobjects,
                aes_key.as_deref(),
                output.as_deref(),
            ),
            Commands::ListClasses {
                input,
                scriptobjects,
                aes_key,
                samples,
            } => commands::list_classes(&input, &scriptobjects, aes_key.as_deref(), samples),
        };
    }

    let input = args
        .input
        .clone()
        .context("Input path is required for extraction")?;

    let mut aes_keys = HashMap::new();
    if let Some(key) = &args.aes_key {
        let parsed_key: AesKey = key
            .parse()
            .context("Invalid AES key format (use hex or base64)")?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    let class_lookup: Option<Arc<HashMap<String, String>>> =
        if let Some(so_path) = &args.scriptobjects {
            let so_data = std::fs::read_to_string(so_path)
                .with_context(|| format!("Failed to read scriptobjects file {:?}", so_path))?;
            let so_json: serde_json::Value = serde_json::from_str(&so_data)
                .with_context(|| format!("Failed to parse scriptobjects file {:?}", so_path))?;

            let hash_to_path = so_json
                .get("hash_to_path")
                .and_then(|v| v.as_object())
                .context("scriptobjects.json missing hash_to_path")?;

            let lookup: HashMap<String, String> = hash_to_path
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect();

            eprintln!("Loaded {} class hashes from scriptobjects", lookup.len());
            Some(Arc::new(lookup))
        } else {
            if !args.class_filter.is_empty() {
                eprintln!("Warning: --class-filter requires --scriptobjects to be set");
            }
            None
        };

    let usmap_schema: Option<Arc<Usmap>> = if let Some(usmap_path) = &args.usmap {
        let usmap_data = std::fs::read(usmap_path)
            .with_context(|| format!("Failed to read usmap file {:?}", usmap_path))?;
        let usmap = Usmap::read(&mut Cursor::new(usmap_data))
            .with_context(|| format!("Failed to parse usmap file {:?}", usmap_path))?;
        eprintln!(
            "Loaded usmap with {} structs, {} enums",
            usmap.structs.len(),
            usmap.enums.len()
        );

        if args.verbose {
            eprintln!("First 10 struct names:");
            for s in usmap.structs.iter().take(10) {
                eprintln!("  - {} (super: {:?})", s.name, s.super_struct);
            }
        }

        Some(Arc::new(usmap))
    } else {
        None
    };

    let store = iostore::open(&input, config.clone())
        .with_context(|| format!("Failed to open {:?}", input))?;

    if args.verbose {
        eprintln!("Opened IoStore: {}", store.container_name());
        store.print_info(0);
    }

    let toc_version = store
        .container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store
        .container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    let entries: Vec<_> = store
        .chunks()
        .filter_map(|chunk| chunk.path().map(|path| (chunk, path)))
        .filter(|(_, path)| matches_filters(path, &args))
        .collect();

    if args.verbose || args.list {
        eprintln!("Found {} matching entries", entries.len());
    }

    if args.list {
        for (_, path) in &entries {
            println!("{}", path);
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output)?;

    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let results: Vec<_> = entries
        .par_iter()
        .map(|(chunk, path)| {
            let result = extract_entry(
                chunk,
                path,
                &args,
                toc_version,
                container_header_version,
                usmap_schema.as_ref(),
                class_lookup.as_ref(),
            );
            pb.inc(1);
            if let Err(ref e) = result {
                eprintln!("Error {}: {:?}", path, e);
            }
            result
        })
        .collect();

    pb.finish_with_message("Done");

    let success = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len() - success;
    eprintln!("Extracted: {}, Failed: {}", success, failed);

    // Also extract from traditional PAK files in the same directory
    if !args.list {
        eprintln!();
        extract_from_paks(&input, &args.output, args.verbose)?;
    }

    Ok(())
}

/// Extract all files from traditional PAK archives in a directory
fn extract_from_paks(paks_dir: &std::path::Path, output: &std::path::Path, verbose: bool) -> Result<()> {
    use pak::{find_pak_files, PakReader};

    let pak_files = find_pak_files(paks_dir)?;

    if pak_files.is_empty() {
        if verbose {
            eprintln!("No traditional PAK files found");
        }
        return Ok(());
    }

    eprintln!("Extracting from {} traditional PAK files...", pak_files.len());

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

        if verbose {
            eprintln!(
                "  {:?}: {} files",
                pak_path.file_name().unwrap_or_default(),
                all_files.len()
            );
        }

        for filename in &all_files {
            let raw_data = match reader.read(filename) {
                Ok(d) => d,
                Err(e) => {
                    if verbose {
                        eprintln!("    Failed to read {}: {}", filename, e);
                    }
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
                        if verbose {
                            eprintln!("    Failed to decompress {}: {}", filename, e);
                        }
                        total_failed += 1;
                        continue;
                    }
                }
            } else {
                raw_data
            };

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&out_path, &write_data)?;
            total_extracted += 1;
        }
    }

    if total_extracted > 0 {
        eprintln!(
            "PAK: Extracted {} files ({} NCS decompressed)",
            total_extracted, ncs_count
        );
    }

    if total_failed > 0 {
        eprintln!("PAK: Failed to extract {} files", total_failed);
    }

    Ok(())
}

/// Extract a single entry from the IoStore
#[allow(clippy::too_many_arguments)]
fn extract_entry(
    chunk: &iostore::ChunkInfo,
    path: &str,
    args: &Args,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
    usmap_schema: Option<&Arc<Usmap>>,
    class_lookup: Option<&Arc<HashMap<String, String>>>,
) -> Result<()> {
    let data = chunk.read()?;

    let mut clean_path = path;
    while clean_path.starts_with("../") {
        clean_path = &clean_path[3..];
    }
    while clean_path.starts_with("./") {
        clean_path = &clean_path[2..];
    }
    let clean_path = clean_path.trim_start_matches('/');

    if args.format == OutputFormat::Uasset || args.format == OutputFormat::Both {
        let out_path = args.output.join(clean_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, &data)?;
    }

    if (args.format == OutputFormat::Json || args.format == OutputFormat::Both)
        && path.ends_with(".uasset")
    {
        match parse_zen_to_json(
            &data,
            path,
            toc_version,
            container_header_version,
            usmap_schema,
            class_lookup,
            args.verbose,
        ) {
            Ok(json) => {
                let json_path = args.output.join(format!("{}.json", clean_path));
                if let Some(parent) = json_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&json_path, json)?;
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {:?}", path, e);
            }
        }
    }

    Ok(())
}
