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

mod items;
mod manifest;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::params;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use unreal_asset::engine_version::EngineVersion;
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::Asset;

/// Output format for list command
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Csv,
    Json,
}

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

    /// Generate HARDCODED reference manifest (NOT extracted from game)
    ///
    /// WARNING: This outputs reference data that is hardcoded in the source.
    /// It is NOT authoritative game data and should NOT be used in implementation.
    /// Use only as a guide for understanding data structures.
    Reference {
        /// Output directory for reference files (should be share/manifest/reference/)
        #[arg(short, long, default_value = "share/manifest/reference")]
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

    /// Extract manufacturer data from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers manufacturer code→name mappings from actual game data
    /// without using hardcoded lookups.
    ExtractManufacturers {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted manufacturers
        #[arg(
            short,
            long,
            default_value = "share/manifest/manufacturers_extracted.json"
        )]
        output: PathBuf,
    },

    /// Extract weapon type data from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers weapon types and their manufacturers from game paths.
    ExtractWeaponTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted weapon types
        #[arg(
            short,
            long,
            default_value = "share/manifest/weapon_types_extracted.json"
        )]
        output: PathBuf,
    },

    /// Extract gear type data from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers gear types (shields, grenades, etc.) and their manufacturers.
    ExtractGearTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted gear types
        #[arg(
            short,
            long,
            default_value = "share/manifest/gear_types_extracted.json"
        )]
        output: PathBuf,
    },

    /// Extract element types from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers element types from effect/texture paths.
    ExtractElements {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted elements
        #[arg(short, long, default_value = "share/manifest/elements_extracted.json")]
        output: PathBuf,
    },

    /// Extract rarity tiers from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers rarity tiers from UI assets and part names.
    ExtractRarities {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted rarities
        #[arg(short, long, default_value = "share/manifest/rarities_extracted.json")]
        output: PathBuf,
    },

    /// Extract stat types from pak_manifest.json (AUTHORITATIVE)
    ///
    /// Discovers stat types from property names in game assets.
    ExtractStats {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        pak_manifest: PathBuf,

        /// Output file for extracted stats
        #[arg(short, long, default_value = "share/manifest/stats_extracted.json")]
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

    /// Manage the verified items database
    Idb {
        /// Path to database file (can also set BL4_ITEMS_DB env var)
        #[arg(short, long, env = "BL4_ITEMS_DB", default_value = items::DEFAULT_DB_PATH)]
        db: PathBuf,

        #[command(subcommand)]
        command: ItemsDbCommand,
    },
}

#[derive(Subcommand)]
enum ItemsDbCommand {
    /// Initialize the items database
    Init,

    /// Add an item to the database
    Add {
        /// Item serial code
        serial: String,

        /// Item name
        #[arg(long)]
        name: Option<String>,

        /// Item prefix (e.g., "Ambushing")
        #[arg(long)]
        prefix: Option<String>,

        /// Manufacturer code (e.g., "JAK")
        #[arg(long)]
        manufacturer: Option<String>,

        /// Item type code (e.g., "PS" for pistol)
        #[arg(long)]
        weapon_type: Option<String>,

        /// Rarity (e.g., "Legendary")
        #[arg(long)]
        rarity: Option<String>,

        /// Item level
        #[arg(long)]
        level: Option<i32>,

        /// Element type (e.g., "cryo")
        #[arg(long)]
        element: Option<String>,
    },

    /// List items in the database
    List {
        /// Filter by manufacturer
        #[arg(long)]
        manufacturer: Option<String>,

        /// Filter by item type
        #[arg(long)]
        weapon_type: Option<String>,

        /// Filter by element
        #[arg(long)]
        element: Option<String>,

        /// Filter by rarity
        #[arg(long)]
        rarity: Option<String>,

        /// Output format: table (default), csv, json
        #[arg(long, default_value = "table")]
        format: OutputFormat,

        /// Fields to include (comma-separated). Available: serial, name, prefix,
        /// manufacturer, weapon_type, item_type, rarity, level, element, dps,
        /// damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
        /// notes, status, legal, source, created_at
        #[arg(short, long, value_delimiter = ',')]
        fields: Option<Vec<String>>,
    },

    /// Show details for a specific item
    Show {
        /// Item ID or serial
        id_or_serial: String,
    },

    /// Add an image attachment to an item
    Attach {
        /// Item serial
        serial: String,

        /// Path to image file
        image: PathBuf,

        /// Attachment name (defaults to filename without extension)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Import items from share/weapons directories
    Import {
        /// Directory to import from (or specific item directory)
        #[arg(default_value = "share/weapons")]
        path: PathBuf,
    },

    /// Export an item to a directory
    Export {
        /// Item serial
        serial: String,

        /// Output directory
        output: PathBuf,
    },

    /// Show database statistics
    Stats,

    /// Set verification status for an item
    Verify {
        /// Item serial
        serial: String,

        /// Verification status (unverified, decoded, screenshot, verified)
        status: String,

        /// Optional verification notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Decode all serials and populate item metadata
    DecodeAll {
        /// Also update items that already have decoded info
        #[arg(long)]
        force: bool,
    },

    /// Import items from a save file
    ImportSave {
        /// Path to .sav file
        save: PathBuf,

        /// Also decode the imported items
        #[arg(long)]
        decode: bool,

        /// Mark imported items as legal
        #[arg(long)]
        legal: bool,
    },

    /// Mark items as legal (verified not modded)
    MarkLegal {
        /// Item IDs to mark as legal (or "all" to mark all items)
        ids: Vec<String>,
    },

    /// Set the source for items
    SetSource {
        /// Source name (e.g., monokrome, ryechews, community)
        source: String,

        /// Item IDs to update, or use --where for condition
        #[arg(required_unless_present = "where_clause")]
        ids: Vec<String>,

        /// SQL WHERE condition (e.g., "legal = 0" for community data)
        #[arg(long = "where")]
        where_clause: Option<String>,
    },

    /// Merge data from one database to another (like cp)
    ///
    /// Copies tier, name, notes, and other user-editable fields from source to target.
    /// Matches items by ID. Use this to sync local changes to a shared database.
    Merge {
        /// Source database to merge FROM
        source: PathBuf,

        /// Destination database to merge TO
        dest: PathBuf,
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

    let pattern_re = pattern.map(Regex::new).transpose()?;

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

        Commands::Manifest {
            extract_dir,
            output,
        } => {
            let output_dir = output.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap()
                    .join("share")
                    .join("manifest")
            });
            manifest::extract_manifest(&extract_dir, &output_dir)?;
        }

        Commands::Manufacturers {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_manufacturers(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Weapons {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_weapon_types(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Balance {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_balance_data(&extract_dir)?;
            output_json(&data, output.as_ref())?;
        }

        Commands::Naming {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_naming_data(&extract_dir)?;
            output_json(&data, output.as_ref())?;
        }

        Commands::Gear {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_gear_types(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Rarity {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_rarity_data(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Elemental {
            extract_dir,
            output,
        } => {
            let data = manifest::extract_elemental_data(&extract_dir);
            output_json(&data, output.as_ref())?;
        }

        Commands::Strings { path, pattern } => {
            search_strings(&path, pattern.as_deref())?;
        }

        Commands::Reference { output } => {
            manifest::generate_reference_manifest(&output)?;
        }

        Commands::ExtractManufacturers {
            pak_manifest,
            output,
        } => {
            println!("Extracting manufacturer data from {:?}...", pak_manifest);
            let manufacturers = manifest::extract_manufacturer_names_from_pak(&pak_manifest)?;

            println!("\nDiscovered {} manufacturers:", manufacturers.len());
            for (code, mfr) in &manufacturers {
                println!("  {} = {} (source: {})", code, mfr.name, mfr.name_source);
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&manufacturers)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::ExtractWeaponTypes {
            pak_manifest,
            output,
        } => {
            println!("Extracting weapon type data from {:?}...", pak_manifest);
            let weapon_types = manifest::extract_weapon_types_from_pak(&pak_manifest)?;

            println!("\nDiscovered {} weapon types:", weapon_types.len());
            for (name, wt) in &weapon_types {
                println!(
                    "  {} ({}) - {} manufacturers: {:?}",
                    name,
                    wt.code,
                    wt.manufacturers.len(),
                    wt.manufacturers
                );
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&weapon_types)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::ExtractGearTypes {
            pak_manifest,
            output,
        } => {
            println!("Extracting gear type data from {:?}...", pak_manifest);
            let gear_types = manifest::extract_gear_types_from_pak(&pak_manifest)?;

            println!("\nDiscovered {} gear types:", gear_types.len());
            for (name, gt) in &gear_types {
                if gt.manufacturers.is_empty() {
                    println!("  {} (no manufacturers)", name);
                } else {
                    println!(
                        "  {} - {} manufacturers: {:?}",
                        name,
                        gt.manufacturers.len(),
                        gt.manufacturers
                    );
                }
                if !gt.subcategories.is_empty() {
                    println!("    subcategories: {:?}", gt.subcategories);
                }
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&gear_types)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::ExtractElements {
            pak_manifest,
            output,
        } => {
            println!("Extracting element types from {:?}...", pak_manifest);
            let elements = manifest::extract_elements_from_pak(&pak_manifest)?;

            println!("\nDiscovered {} element types:", elements.len());
            for name in elements.keys() {
                println!("  {}", name);
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&elements)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::ExtractRarities {
            pak_manifest,
            output,
        } => {
            println!("Extracting rarity tiers from {:?}...", pak_manifest);
            let rarities = manifest::extract_rarities_from_pak(&pak_manifest)?;

            println!("\nDiscovered {} rarity tiers:", rarities.len());
            for rarity in &rarities {
                println!("  {} ({}) = {}", rarity.tier, rarity.code, rarity.name);
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&rarities)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::ExtractStats {
            pak_manifest,
            output,
        } => {
            println!("Extracting stat types from {:?}...", pak_manifest);
            let stats = manifest::extract_stats_from_pak(&pak_manifest)?;

            println!(
                "\nDiscovered {} stat types (top 20 by occurrence):",
                stats.len()
            );
            for stat in stats.iter().take(20) {
                if stat.modifier_types.is_empty() {
                    println!("  {} ({} occurrences)", stat.name, stat.occurrences);
                } else {
                    println!(
                        "  {} [{:?}] ({} occurrences)",
                        stat.name, stat.modifier_types, stat.occurrences
                    );
                }
            }
            if stats.len() > 20 {
                println!("  ... and {} more", stats.len() - 20);
            }

            // Write to output file
            let json = serde_json::to_string_pretty(&stats)?;
            std::fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::PakManifest {
            extracted_dir,
            output,
        } => {
            manifest::generate_pak_manifest(&extracted_dir, &output)?;
        }

        Commands::ItemsDb {
            manifest_dir,
            output,
        } => {
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

        Commands::Idb { db, command } => {
            handle_items_db_command(command, &db)?;
        }
    }

    Ok(())
}

fn handle_items_db_command(cmd: ItemsDbCommand, db: &PathBuf) -> Result<()> {
    match cmd {
        ItemsDbCommand::Init => {
            // Create parent directory if needed
            if let Some(parent) = db.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;
            println!("Your database is ready at {}", db.display());
        }

        ItemsDbCommand::Add {
            serial,
            name,
            prefix,
            manufacturer,
            weapon_type,
            rarity,
            level,
            element,
        } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;
            wdb.add_item(&serial)?;

            if name.is_some()
                || prefix.is_some()
                || manufacturer.is_some()
                || weapon_type.is_some()
                || rarity.is_some()
                || level.is_some()
                || element.is_some()
            {
                wdb.update_item(
                    &serial,
                    name.as_deref(),
                    prefix.as_deref(),
                    manufacturer.as_deref(),
                    weapon_type.as_deref(),
                    rarity.as_deref(),
                    level,
                    element.as_deref(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )?;
            }

            println!("Added item: {}", serial);
        }

        ItemsDbCommand::List {
            manufacturer,
            weapon_type,
            element,
            rarity,
            format,
            fields,
        } => {
            let wdb = items::ItemsDb::open(db)?;
            let items = wdb.list_items(
                manufacturer.as_deref(),
                weapon_type.as_deref(),
                element.as_deref(),
                rarity.as_deref(),
            )?;

            if items.is_empty() {
                println!("No items found");
                return Ok(());
            }

            // Default fields if none specified
            let default_fields = vec![
                "serial", "manufacturer", "name", "weapon_type", "level", "element",
            ];
            let field_list: Vec<&str> = fields
                .as_ref()
                .map(|f| f.iter().map(|s| s.as_str()).collect())
                .unwrap_or_else(|| default_fields);

            match format {
                OutputFormat::Json => {
                    // Filter items to only include requested fields
                    let filtered: Vec<serde_json::Value> = items
                        .iter()
                        .map(|item| filter_item_fields(item, &field_list))
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                }
                OutputFormat::Csv => {
                    // Header
                    println!("{}", field_list.join(","));
                    // Rows
                    for item in &items {
                        let values: Vec<String> = field_list
                            .iter()
                            .map(|f| get_item_field_value(item, f))
                            .map(|v| escape_csv(&v))
                            .collect();
                        println!("{}", values.join(","));
                    }
                }
                OutputFormat::Table => {
                    // Column widths (truncate long values)
                    let col_widths: Vec<usize> = field_list
                        .iter()
                        .map(|f| match *f {
                            "serial" => 35,
                            "name" => 20,
                            "prefix" => 15,
                            "manufacturer" => 12,
                            "weapon_type" => 8,
                            "item_type" => 6,
                            "rarity" => 10,
                            "level" => 4,
                            "element" => 10,
                            "dps" | "damage" | "accuracy" | "mag_size" | "value" => 8,
                            "fire_rate" | "reload_time" => 10,
                            "red_text" | "notes" => 25,
                            "status" => 10,
                            "legal" => 5,
                            "source" => 12,
                            "created_at" => 20,
                            _ => 15,
                        })
                        .collect();

                    // Header
                    let header: String = field_list
                        .iter()
                        .zip(&col_widths)
                        .map(|(f, w)| format!("{:<width$}", f, width = w))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("{}", header);
                    println!("{}", "-".repeat(header.len()));

                    // Rows
                    for item in &items {
                        let row: String = field_list
                            .iter()
                            .zip(&col_widths)
                            .map(|(f, w)| {
                                let val = get_item_field_value(item, f);
                                let truncated = if val.len() > *w {
                                    format!("{}…", &val[..*w - 1])
                                } else {
                                    val
                                };
                                format!("{:<width$}", truncated, width = w)
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", row);
                    }
                }
            }
        }

        ItemsDbCommand::Show { id_or_serial } => {
            let wdb = items::ItemsDb::open(db)?;

            // Look up by serial
            let weapon = wdb.get_item(&id_or_serial)?;

            if let Some(w) = weapon {
                println!("Serial:       {}", w.serial);
                println!("Name:         {}", w.name.as_deref().unwrap_or("-"));
                println!("Prefix:       {}", w.prefix.as_deref().unwrap_or("-"));
                println!("Manufacturer: {}", w.manufacturer.as_deref().unwrap_or("-"));
                println!("Type:         {}", w.weapon_type.as_deref().unwrap_or("-"));
                println!("Rarity:       {}", w.rarity.as_deref().unwrap_or("-"));
                println!(
                    "Level:        {}",
                    w.level.map(|l| l.to_string()).unwrap_or("-".to_string())
                );
                println!("Element:      {}", w.element.as_deref().unwrap_or("-"));
                println!(
                    "DPS:          {}",
                    w.dps.map(|d| d.to_string()).unwrap_or("-".to_string())
                );
                println!(
                    "Damage:       {}",
                    w.damage.map(|d| d.to_string()).unwrap_or("-".to_string())
                );
                println!(
                    "Accuracy:     {}",
                    w.accuracy
                        .map(|a| format!("{}%", a))
                        .unwrap_or("-".to_string())
                );
                println!(
                    "Fire Rate:    {}",
                    w.fire_rate
                        .map(|f| format!("{}/s", f))
                        .unwrap_or("-".to_string())
                );
                println!(
                    "Reload:       {}",
                    w.reload_time
                        .map(|r| format!("{}s", r))
                        .unwrap_or("-".to_string())
                );
                println!(
                    "Mag Size:     {}",
                    w.mag_size.map(|m| m.to_string()).unwrap_or("-".to_string())
                );
                println!(
                    "Value:        {}",
                    w.value
                        .map(|v| format!("${}", v))
                        .unwrap_or("-".to_string())
                );
                println!("Red Text:     {}", w.red_text.as_deref().unwrap_or("-"));
                println!("Notes:        {}", w.notes.as_deref().unwrap_or("-"));
                println!("\n--- Metadata ---");
                println!("Source:       {}", w.source.as_deref().unwrap_or("-"));
                println!("Legal:        {}", if w.legal { "yes" } else { "no" });
                println!("Status:       {}", w.verification_status);
                println!(
                    "Ver. Notes:   {}",
                    w.verification_notes.as_deref().unwrap_or("-")
                );
                println!("Verified At:  {}", w.verified_at.as_deref().unwrap_or("-"));
                println!("Created:      {}", w.created_at);

                // Show parts
                let parts = wdb.get_parts(&w.serial)?;
                if !parts.is_empty() {
                    println!("\nParts:");
                    for p in parts {
                        println!(
                            "  {} - {} ({})",
                            p.slot,
                            p.manufacturer.as_deref().unwrap_or("-"),
                            p.effect.as_deref().unwrap_or("-")
                        );
                    }
                }

                // Show attachments
                let attachments = wdb.get_attachments(&w.serial)?;
                if !attachments.is_empty() {
                    println!("\nAttachments:");
                    for a in attachments {
                        println!("  {} ({})", a.name, a.mime_type);
                    }
                }
            } else {
                println!("Item not found: {}", id_or_serial);
            }
        }

        ItemsDbCommand::Attach {
            serial,
            image,
            name,
        } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;

            let attachment_name = name.unwrap_or_else(|| {
                image
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

            let mime_type = match image.extension().and_then(|e| e.to_str()) {
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                _ => "application/octet-stream",
            };

            let data = std::fs::read(&image)?;
            let attachment_id = wdb.add_attachment(&serial, &attachment_name, mime_type, &data)?;
            println!(
                "Added attachment '{}' (ID {}) to item {}",
                attachment_name, attachment_id, serial
            );
        }

        ItemsDbCommand::Import { path } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?; // Ensure schema exists

            // Check if path is a single weapon directory or a parent directory
            if path.join("serial.txt").exists() {
                // Single weapon directory
                let serial = wdb.import_from_dir(&path)?;
                println!(
                    "Imported item {} from {}",
                    serial,
                    path.display()
                );
            } else {
                // Parent directory - import all subdirectories
                let mut imported = 0;
                for entry in std::fs::read_dir(&path)? {
                    let entry = entry?;
                    let subdir = entry.path();
                    if subdir.is_dir() && subdir.join("serial.txt").exists() {
                        match wdb.import_from_dir(&subdir) {
                            Ok(serial) => {
                                println!(
                                    "Imported {} ({})",
                                    subdir.file_name().unwrap_or_default().to_string_lossy(),
                                    &serial[..serial.len().min(30)]
                                );
                                imported += 1;
                            }
                            Err(e) => {
                                eprintln!("Failed to import {}: {}", subdir.display(), e);
                            }
                        }
                    }
                }
                println!("\nImported {} items", imported);
            }
        }

        ItemsDbCommand::Export { serial, output } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.export_to_dir(&serial, &output)?;
            println!("Exported item {} to {}", serial, output.display());
        }

        ItemsDbCommand::Stats => {
            let wdb = items::ItemsDb::open(db)?;
            let stats = wdb.stats()?;
            println!("Items Database Statistics");
            println!("  Items:       {}", stats.item_count);
            println!("  Parts:       {}", stats.part_count);
            println!("  Attachments: {}", stats.attachment_count);
        }

        ItemsDbCommand::Verify {
            serial,
            status,
            notes,
        } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;
            let status: items::VerificationStatus = status.parse()?;
            wdb.set_verification_status(&serial, status, notes.as_deref())?;
            println!("Updated item {} to status: {}", serial, status);
        }

        ItemsDbCommand::DecodeAll { force } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;
            let items = wdb.list_items(None, None, None, None)?;

            let mut decoded = 0;
            let mut skipped = 0;
            let mut failed = 0;

            for item in &items {
                // Skip if already has manufacturer or weapon_type info (unless force)
                // Shields/gear use weapon_type for category name
                if !force && (item.manufacturer.is_some() || item.weapon_type.is_some()) {
                    skipped += 1;
                    continue;
                }

                // Decode the serial
                match bl4::serial::ItemSerial::decode(&item.serial) {
                    Ok(decoded_item) => {
                        // Get manufacturer and weapon type based on serial format
                        let (mfg, wtype) = if let Some(mfg_id) = decoded_item.manufacturer {
                            // VarInt-first format: use weapon info lookup
                            bl4::parts::weapon_info_from_first_varint(mfg_id)
                                .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
                                .unwrap_or((None, None))
                        } else if let Some(group_id) = decoded_item.part_group_id() {
                            // VarBit-first format: use type-aware category lookup
                            let cat_name = bl4::parts::category_name_for_type(
                                decoded_item.item_type,
                                group_id,
                            );
                            // For shields/gear, category name goes in weapon_type field
                            (None, cat_name.map(|s| s.to_string()))
                        } else {
                            (None, None)
                        };

                        // Get level from level code
                        let level = decoded_item
                            .level
                            .and_then(bl4::parts::level_from_code)
                            .map(|l| l as i32);

                        // Update the item in database
                        wdb.update_item(
                            &item.serial,
                            None, // name
                            None, // prefix
                            mfg.as_deref(),
                            wtype.as_deref(),
                            None, // rarity
                            level,
                            None, // element
                            None, // dps
                            None, // damage
                            None, // accuracy
                            None, // fire_rate
                            None, // reload_time
                            None, // mag_size
                            None, // value
                            None, // red_text
                            None, // notes
                        )?;

                        // Set item type from serial type character
                        wdb.set_item_type(&item.serial, &decoded_item.item_type.to_string())?;

                        // Update status to decoded (only if currently unverified)
                        if item.verification_status == items::VerificationStatus::Unverified {
                            wdb.set_verification_status(
                                &item.serial,
                                items::VerificationStatus::Decoded,
                                None,
                            )?;
                        }

                        decoded += 1;
                    }
                    Err(e) => {
                        eprintln!("Failed to decode {}: {}", item.serial, e);
                        failed += 1;
                    }
                }
            }

            println!(
                "Decoded {} items, skipped {} (already decoded), {} failed",
                decoded, skipped, failed
            );
        }

        ItemsDbCommand::ImportSave {
            save,
            decode,
            legal,
        } => {
            // Extract steam_id from path (e.g., .../76561197960521364/Profiles/...)
            let steam_id = save.to_string_lossy()
                .split('/')
                .find(|s| s.len() == 17 && s.chars().all(|c| c.is_ascii_digit()))
                .map(String::from)
                .context("Could not extract Steam ID from path. Expected path like .../76561197960521364/...")?;

            // Read and decrypt the save file
            let save_data = std::fs::read(&save)
                .with_context(|| format!("Failed to read save file: {}", save.display()))?;

            let yaml_data = bl4::crypto::decrypt_sav(&save_data, &steam_id)
                .context("Failed to decrypt save file")?;
            let yaml_str =
                String::from_utf8(yaml_data).context("Decrypted data is not valid UTF-8")?;

            // Parse YAML and extract serials
            let yaml: serde_yaml::Value =
                serde_yaml::from_str(&yaml_str).context("Failed to parse save YAML")?;

            let mut serials = Vec::new();
            extract_serials_from_yaml(&yaml, &mut serials);

            // Deduplicate
            serials.sort();
            serials.dedup();

            println!(
                "Found {} unique serials in {}",
                serials.len(),
                save.display()
            );

            // Add to database
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;
            let mut added = 0;
            let mut skipped = 0;

            for serial in &serials {
                match wdb.add_item(serial) {
                    Ok(_) => added += 1,
                    Err(_) => skipped += 1, // Already exists
                }
            }

            println!("Added {} new items, {} already existed", added, skipped);

            // Optionally decode
            if decode && added > 0 {
                println!("Decoding new items...");
                let items = wdb.list_items(None, None, None, None)?;
                let mut decoded_count = 0;

                for item in &items {
                    if item.manufacturer.is_some() {
                        continue;
                    }

                    if let Ok(decoded_item) = bl4::serial::ItemSerial::decode(&item.serial) {
                        let (mfg, wtype) = if let Some(mfg_id) = decoded_item.manufacturer {
                            bl4::parts::weapon_info_from_first_varint(mfg_id)
                                .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
                                .unwrap_or((None, None))
                        } else {
                            (None, None)
                        };

                        let level = decoded_item
                            .level
                            .and_then(bl4::parts::level_from_code)
                            .map(|l| l as i32);

                        let _ = wdb.update_item(
                            &item.serial,
                            None,
                            None,
                            mfg.as_deref(),
                            wtype.as_deref(),
                            None,
                            level,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        );

                        if item.verification_status == items::VerificationStatus::Unverified {
                            let _ = wdb.set_verification_status(
                                &item.serial,
                                items::VerificationStatus::Decoded,
                                None,
                            );
                        }
                        decoded_count += 1;
                    }
                }
                println!("Decoded {} items", decoded_count);
            }

            // Mark items as legal if requested
            if legal {
                // Get all items that match the serials we imported
                let mut marked = 0;
                for serial in &serials {
                    if let Ok(Some(item)) = wdb.get_item(serial) {
                        if !item.legal {
                            let _ = wdb.set_legal(&item.serial, true);
                            marked += 1;
                        }
                    }
                }
                println!("Marked {} items as legal", marked);
            }
        }

        ItemsDbCommand::MarkLegal { ids } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;

            if ids.len() == 1 && ids[0] == "all" {
                let count = wdb.set_all_legal(true)?;
                println!("Marked all {} items as legal", count);
            } else {
                let mut marked = 0;
                for serial in &ids {
                    wdb.set_legal(serial, true)?;
                    marked += 1;
                }
                println!("Marked {} items as legal", marked);
            }
        }

        ItemsDbCommand::SetSource {
            source,
            ids,
            where_clause,
        } => {
            let wdb = items::ItemsDb::open(db)?;
            wdb.init()?;

            if let Some(condition) = where_clause {
                let count = wdb.set_source_where(&source, &condition)?;
                println!("Set source to '{}' for {} items", source, count);
            } else if ids.len() == 1 && ids[0] == "null" {
                let count = wdb.set_source_for_null(&source)?;
                println!(
                    "Set source to '{}' for {} items with no source",
                    source, count
                );
            } else {
                let mut updated = 0;
                for serial in &ids {
                    wdb.set_source(serial, &source)?;
                    updated += 1;
                }
                println!("Set source to '{}' for {} items", source, updated);
            }
        }

        ItemsDbCommand::Merge { source, dest } => {
            merge_databases(&source, &dest)?;
        }
    }

    Ok(())
}

/// Merge user-editable fields from source database to destination database
fn merge_databases(source: &Path, dest: &Path) -> Result<()> {
    use rusqlite::Connection;

    println!("Merging {} -> {}", source.display(), dest.display());

    let src_conn = Connection::open(source)
        .with_context(|| format!("Failed to open source database: {}", source.display()))?;
    let dest_conn = Connection::open(dest)
        .with_context(|| format!("Failed to open destination database: {}", dest.display()))?;

    // Ensure tier column exists in destination
    let _ = dest_conn.execute("ALTER TABLE weapons ADD COLUMN tier TEXT", []);

    // Get all items with user-editable data from source
    let mut stmt = src_conn.prepare(
        "SELECT id, name, tier, notes FROM weapons WHERE name IS NOT NULL OR tier IS NOT NULL OR notes IS NOT NULL"
    )?;

    #[allow(clippy::type_complexity)]
    let items: Vec<(i64, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    println!("Found {} items with user data to merge", items.len());

    let mut updated = 0;
    for (id, name, tier, notes) in &items {
        // Update each field if it has a value
        if let Some(name) = name {
            if !name.is_empty() {
                dest_conn.execute(
                    "UPDATE weapons SET name = ?1 WHERE id = ?2",
                    params![name, id],
                )?;
            }
        }
        if let Some(tier) = tier {
            dest_conn.execute(
                "UPDATE weapons SET tier = ?1 WHERE id = ?2",
                params![tier, id],
            )?;
        }
        if let Some(notes) = notes {
            if !notes.is_empty() {
                dest_conn.execute(
                    "UPDATE weapons SET notes = ?1 WHERE id = ?2",
                    params![notes, id],
                )?;
            }
        }
        updated += 1;
    }

    println!("Merged {} items", updated);

    // Verify
    let count: i64 = dest_conn.query_row(
        "SELECT COUNT(*) FROM weapons WHERE tier IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    println!("Destination now has {} tiered items", count);

    Ok(())
}

/// Recursively extract serial strings from YAML
fn extract_serials_from_yaml(value: &serde_yaml::Value, serials: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) => {
            if s.starts_with("@Ug") && s.len() >= 10 {
                serials.push(s.clone());
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                // Check if key contains "serial"
                if let serde_yaml::Value::String(key) = k {
                    if key == "serial" {
                        if let serde_yaml::Value::String(s) = v {
                            if s.starts_with("@Ug") {
                                serials.push(s.clone());
                            }
                        }
                    }
                }
                extract_serials_from_yaml(v, serials);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for v in seq {
                extract_serials_from_yaml(v, serials);
            }
        }
        _ => {}
    }
}

/// Get field value from an item
fn get_item_field_value(item: &items::Item, field: &str) -> String {
    match field {
        "serial" => item.serial.clone(),
        "name" => item.name.clone().unwrap_or_default(),
        "prefix" => item.prefix.clone().unwrap_or_default(),
        "manufacturer" => item.manufacturer.clone().unwrap_or_default(),
        "weapon_type" => item.weapon_type.clone().unwrap_or_default(),
        "item_type" => item.item_type.clone().unwrap_or_default(),
        "rarity" => item.rarity.clone().unwrap_or_default(),
        "level" => item.level.map(|l| l.to_string()).unwrap_or_default(),
        "element" => item.element.clone().unwrap_or_default(),
        "dps" => item.dps.map(|v| v.to_string()).unwrap_or_default(),
        "damage" => item.damage.map(|v| v.to_string()).unwrap_or_default(),
        "accuracy" => item.accuracy.map(|v| v.to_string()).unwrap_or_default(),
        "fire_rate" => item.fire_rate.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        "reload_time" => item.reload_time.map(|v| format!("{:.2}", v)).unwrap_or_default(),
        "mag_size" => item.mag_size.map(|v| v.to_string()).unwrap_or_default(),
        "value" => item.value.map(|v| v.to_string()).unwrap_or_default(),
        "red_text" => item.red_text.clone().unwrap_or_default(),
        "notes" => item.notes.clone().unwrap_or_default(),
        "status" => item.verification_status.to_string(),
        "legal" => if item.legal { "true" } else { "false" }.to_string(),
        "source" => item.source.clone().unwrap_or_default(),
        "created_at" => item.created_at.clone(),
        _ => String::new(),
    }
}

/// Filter item to only requested fields (for JSON output)
fn filter_item_fields(item: &items::Item, fields: &[&str]) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in fields {
        let value = get_item_field_value(item, field);
        obj.insert(
            (*field).to_string(),
            if value.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(value)
            },
        );
    }
    serde_json::Value::Object(obj)
}

/// Escape a value for CSV output
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
