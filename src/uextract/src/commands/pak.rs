//! PAK extraction command implementation

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

use crate::pak::{find_pak_files, PakReader};

/// Extract files from PAK archives
pub fn extract_pak(
    input: &Path,
    output: &Path,
    extension: Option<&str>,
    filters: &[String],
    list_only: bool,
    verbose: bool,
) -> Result<()> {
    // Determine if input is a file or directory
    let pak_files = if input.is_file() {
        vec![input.to_path_buf()]
    } else if input.is_dir() {
        find_pak_files(input)?
    } else {
        anyhow::bail!("Input path does not exist: {:?}", input);
    };

    if pak_files.is_empty() {
        anyhow::bail!("No .pak files found in {:?}", input);
    }

    if verbose {
        eprintln!("Found {} PAK files", pak_files.len());
    }

    let mut total_files = 0;
    let mut total_extracted = 0;

    for pak_path in &pak_files {
        if verbose {
            eprintln!("\nProcessing: {:?}", pak_path);
        }

        let mut reader = PakReader::open(pak_path)
            .with_context(|| format!("Failed to open {:?}", pak_path))?;

        // Get list of files matching filters
        let all_files = reader.files();
        let matching: Vec<_> = all_files
            .into_iter()
            .filter(|f| {
                // Extension filter
                if let Some(ext) = extension {
                    let ext_lower = ext.to_lowercase();
                    let with_dot = if ext_lower.starts_with('.') {
                        ext_lower
                    } else {
                        format!(".{}", ext_lower)
                    };
                    if !f.to_lowercase().ends_with(&with_dot) {
                        return false;
                    }
                }

                // String filters (OR logic)
                if !filters.is_empty() {
                    let f_lower = f.to_lowercase();
                    if !filters.iter().any(|filter| f_lower.contains(&filter.to_lowercase())) {
                        return false;
                    }
                }

                true
            })
            .collect();

        total_files += matching.len();

        if verbose {
            eprintln!("  Mount point: {}", reader.mount_point());
            eprintln!("  Matching files: {}", matching.len());
        }

        if list_only {
            for f in &matching {
                println!("{}", f);
            }
            continue;
        }

        // Extract files
        let pak_name = pak_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("pak");

        let pak_output = output.join(pak_name);
        std::fs::create_dir_all(&pak_output)?;

        let pb = ProgressBar::new(matching.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        for filename in &matching {
            let data = match reader.read(filename) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Warning: Failed to read {}: {}", filename, e);
                    pb.inc(1);
                    continue;
                }
            };

            // Clean up the path
            let clean_name = filename
                .trim_start_matches(reader.mount_point())
                .trim_start_matches('/')
                .trim_start_matches("../");

            let out_path = pak_output.join(clean_name);

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&out_path, &data)?;
            total_extracted += 1;
            pb.inc(1);
        }

        pb.finish_and_clear();
    }

    if list_only {
        eprintln!("Found {} matching files", total_files);
    } else {
        eprintln!(
            "Extracted {} files from {} PAK archives to {:?}",
            total_extracted,
            pak_files.len(),
            output
        );
    }

    Ok(())
}
