//! BL4 Research Tools
//!
//! Tools for extracting and analyzing Borderlands 4 game data.
//!
//! Usage:
//!   bl4-research parse <file.uasset>         - Parse and dump a UAsset file
//!   bl4-research manifest                    - Extract all game data
//!   bl4-research manufacturers               - Extract manufacturer data only
//!   bl4-research weapons                     - Extract weapon type data only
//!   bl4-research balance                     - Extract balance/stats data only
//!   bl4-research naming                      - Extract naming tables only
//!   bl4-research gear                        - Extract non-weapon gear types
//!   bl4-research rarity                      - Extract rarity data only
//!   bl4-research elemental                   - Extract elemental data only
//!   bl4-research strings <path>              - Search strings in .uasset files

mod manifest;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::Cursor;
use std::path::PathBuf;
use unreal_asset::engine_version::EngineVersion;
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::Asset;

/// Default extraction directory
const DEFAULT_EXTRACT_DIR: &str = "/tmp/bl4_extract";

#[derive(Parser)]
#[command(name = "bl4-research")]
#[command(about = "Borderlands 4 Research Tools", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a .uasset file and dump export info
    Parse {
        /// Path to .uasset file
        file: PathBuf,
    },

    /// Extract all game data manifest
    Manifest {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output directory for manifest files
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract manufacturer data
    Manufacturers {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract weapon type data
    Weapons {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract balance/stats data
    Balance {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract naming tables
    Naming {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract non-weapon gear types (shields, grenades, etc.)
    Gear {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract rarity data
    Rarity {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract elemental data
    Elemental {
        /// Path to extracted game files
        #[arg(short, long, default_value = DEFAULT_EXTRACT_DIR)]
        extract_dir: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Search for strings in .uasset files
    Strings {
        /// Path to .uasset file or directory
        path: PathBuf,

        /// Pattern to search for (regex)
        #[arg(short, long)]
        pattern: Option<String>,
    },

    /// Generate consolidated reference manifest (no extraction needed)
    Reference {
        /// Output directory for manifest files
        #[arg(short, long, default_value = "share/manifest")]
        output: PathBuf,
    },

    /// Build manifest from uextract pak file extraction
    PakManifest {
        /// Path to uextract output directory (contains JSON files)
        #[arg(short, long, default_value = "share/manifest/extracted")]
        extracted_dir: PathBuf,

        /// Output directory for manifest files
        #[arg(short, long, default_value = "share/manifest")]
        output: PathBuf,
    },

    /// Generate items database with drop pools and stats
    ItemsDb {
        /// Path to manifest directory (containing pak_manifest.json)
        #[arg(short, long, default_value = "share/manifest")]
        manifest_dir: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn parse_uasset(path: &PathBuf) -> Result<()> {
    println!("Parsing: {}", path.display());

    let data = std::fs::read(path).context("Failed to read file")?;

    // Try different engine versions (UE5 first for BL4)
    // Note: unreal_asset only supports up to UE5.2
    let versions = [
        EngineVersion::VER_UE5_2,
        EngineVersion::VER_UE5_1,
        EngineVersion::VER_UE5_0,
        EngineVersion::VER_UE4_27,
        EngineVersion::VER_UE4_26,
        EngineVersion::VER_UE4_25,
        EngineVersion::UNKNOWN,
    ];

    for version in versions {
        let cursor = Cursor::new(&data);
        match Asset::new(cursor, None, version) {
            Ok(asset) => {
                println!("Successfully parsed with {:?}", version);
                println!("Exports: {}", asset.asset_data.exports.len());

                for (i, export) in asset.asset_data.exports.iter().enumerate() {
                    let base = export.get_base_export();
                    println!(
                        "  [{}] {:?} (class: {:?})",
                        i, base.object_name, base.class_index
                    );
                }
                return Ok(());
            }
            Err(e) => {
                println!("Failed with {:?}: {}", version, e);
                continue;
            }
        }
    }

    anyhow::bail!("Failed to parse with any engine version")
}

fn search_strings(path: &PathBuf, pattern: Option<&str>) -> Result<()> {
    use regex::Regex;
    use walkdir::WalkDir;

    let pattern_re = pattern.map(|p| Regex::new(p)).transpose()?;

    let files: Vec<PathBuf> = if path.is_file() {
        vec![path.clone()]
    } else {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "uasset")
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    for file_path in &files {
        let content = manifest::extract_strings(file_path)?;

        if let Some(ref re) = pattern_re {
            let matches: Vec<&str> = content.lines().filter(|line| re.is_match(line)).collect();
            if !matches.is_empty() {
                println!("\n=== {} ===", file_path.display());
                for m in matches {
                    println!("  {}", m);
                }
            }
        } else {
            println!("\n=== {} ===", file_path.display());
            for line in content.lines().take(100) {
                if !line.is_empty() && line.len() < 200 {
                    println!("  {}", line);
                }
            }
        }
    }

    Ok(())
}

/// Output JSON to file or stdout
fn output_json<T: serde::Serialize>(data: &T, output: Option<&PathBuf>) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    if let Some(path) = output {
        std::fs::write(path, &json)?;
        eprintln!("Saved to {}", path.display());
    } else {
        println!("{}", json);
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file } => {
            parse_uasset(&file)?;
        }

        Commands::Manifest { extract_dir, output } => {
            let output_dir = output.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap()
                    .join("share")
                    .join("manifest")
            });
            manifest::extract_manifest(&extract_dir, &output_dir)?;
        }

        Commands::Manufacturers { extract_dir, output } => {
            let data = manifest::extract_manufacturers(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Weapons { extract_dir, output } => {
            let data = manifest::extract_weapon_types(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Balance { extract_dir, output } => {
            let data = manifest::extract_balance_data(&extract_dir)?;
            output_json(&data, output.as_ref())?;
        }

        Commands::Naming { extract_dir, output } => {
            let data = manifest::extract_naming_data(&extract_dir)?;
            output_json(&data, output.as_ref())?;
        }

        Commands::Gear { extract_dir, output } => {
            let data = manifest::extract_gear_types(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Rarity { extract_dir, output } => {
            let data = manifest::extract_rarity_data(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Elemental { extract_dir, output } => {
            let data = manifest::extract_elemental_data(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Strings { path, pattern } => {
            search_strings(&path, pattern.as_deref())?;
        }

        Commands::Reference { output } => {
            manifest::generate_reference_manifest(&output)?;
        }

        Commands::PakManifest { extracted_dir, output } => {
            manifest::generate_pak_manifest(&extracted_dir, &output)?;
        }

        Commands::ItemsDb { manifest_dir, output } => {
            let db = manifest::generate_items_database(&manifest_dir)?;

            let output_path = output.unwrap_or_else(|| manifest_dir.join("items_database.json"));
            let json = serde_json::to_string_pretty(&db)?;
            std::fs::write(&output_path, &json)?;

            println!("\n=== Items Database Generated ===");
            println!("Total pools: {}", db.stats_summary.total_pools);
            println!("Total items with stats: {}", db.stats_summary.total_items);
            println!("Categories: {:?}", db.stats_summary.categories);
            println!("Manufacturers: {:?}", db.stats_summary.manufacturers);
            println!("Stat types: {:?}", db.stats_summary.stat_types);
            println!("\nSaved to: {}", output_path.display());
        }
    }

    Ok(())
}
