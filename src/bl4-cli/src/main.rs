mod config;
mod file_io;
#[cfg(feature = "research")]
mod manifest;
mod memory;

use anyhow::{bail, Context, Result};
use bl4_idb::{AttachmentsRepository, ImportExportRepository, ItemsRepository};
use byteorder::ByteOrder;
use clap::{Parser, Subcommand};
use config::Config;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct PartCategoriesFile {
    categories: Vec<PartCategory>,
}

#[derive(Debug, Deserialize)]
struct PartCategory {
    prefix: String,
    category: i64,
    #[serde(default)]
    weapon_type: Option<String>,
    #[serde(default)]
    gear_type: Option<String>,
    #[serde(default)]
    manufacturer: Option<String>,
}

#[derive(Parser)]
#[command(name = "bl4")]
#[command(about = "Borderlands 4 Save Editor", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Save file operations (decrypt, encrypt, edit, get, set)
    #[command(visible_alias = "s")]
    Save {
        #[command(subcommand)]
        command: SaveCommand,
    },

    /// Inspect a save file (decrypt and display info)
    #[command(visible_alias = "i")]
    Inspect {
        /// Path to .sav file
        #[arg(short, long)]
        input: PathBuf,

        /// Steam ID for decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

        /// Show full YAML output
        #[arg(short, long)]
        full: bool,
    },

    /// Configure default settings
    #[command(visible_alias = "c")]
    Configure {
        /// Set default Steam ID
        #[arg(long)]
        steam_id: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },

    /// Item serial operations (decode, encode, compare, modify)
    #[command(visible_alias = "r")]
    Serial {
        #[command(subcommand)]
        command: SerialCommand,
    },

    /// Query parts database - find parts for a weapon type
    #[command(visible_alias = "p")]
    Parts {
        /// Weapon name (e.g. "Jakobs Pistol", "Vladof SMG")
        #[arg(short, long)]
        weapon: Option<String>,

        /// Category ID (e.g. 3 for Jakobs Pistol)
        #[arg(short, long)]
        category: Option<i64>,

        /// List all categories
        #[arg(short, long)]
        list: bool,

        /// Path to parts database
        #[arg(long, default_value = "share/manifest/parts_database.json")]
        parts_db: PathBuf,
    },

    /// Read/analyze game memory (live process or dump file)
    #[command(visible_alias = "m")]
    Memory {
        /// Use preload-based injection (requires game launched with LD_PRELOAD)
        #[arg(long)]
        preload: bool,

        /// Read from memory dump file instead of live process (for offline analysis)
        #[arg(long, short = 'd')]
        dump: Option<PathBuf>,

        /// Path to maps file for dump (optional, defaults to <dump>.maps)
        #[arg(long)]
        maps: Option<PathBuf>,

        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Launch Borderlands 4 with instrumentation
    #[command(visible_alias = "l")]
    Launch {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Usmap file utilities (requires 'research' feature)
    #[cfg(feature = "research")]
    Usmap {
        #[command(subcommand)]
        command: UsmapCommand,
    },

    /// Data extraction utilities (requires 'research' feature)
    #[cfg(feature = "research")]
    #[command(visible_alias = "e")]
    Extract {
        #[command(subcommand)]
        command: ExtractCommand,
    },

    /// Manage the verified items database
    Idb {
        /// Path to database file (can also set BL4_ITEMS_DB env var)
        #[arg(short, long, env = "BL4_ITEMS_DB", default_value = bl4_idb::DEFAULT_DB_PATH)]
        db: PathBuf,

        #[command(subcommand)]
        command: ItemsDbCommand,
    },

    /// Generate manifest files from game data (requires 'research' feature)
    #[cfg(feature = "research")]
    Manifest {
        /// Path to game's Paks directory containing .utoc/.ucas files
        #[arg(short, long)]
        paks: PathBuf,

        /// Path to .usmap file for property schema
        #[arg(short = 'm', long)]
        usmap: PathBuf,

        /// Output directory for manifest files
        #[arg(short, long, default_value = "share/manifest")]
        output: PathBuf,

        /// AES encryption key if paks are encrypted (base64 or hex)
        #[arg(long)]
        aes_key: Option<String>,

        /// Skip uextract step (use existing extracted files)
        #[arg(long)]
        skip_extract: bool,

        /// Path to existing extracted files (with --skip-extract)
        #[arg(long, default_value = "/tmp/bl4_extract")]
        extracted: PathBuf,
    },
}

#[derive(Subcommand)]
enum SaveCommand {
    /// Decrypt a .sav file to YAML (uses stdin/stdout if paths not specified)
    Decrypt {
        /// Path to encrypted .sav file (uses stdin if not specified)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Path to output YAML file (uses stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Steam ID for decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,
    },

    /// Encrypt a YAML file to .sav (uses stdin/stdout if paths not specified)
    Encrypt {
        /// Path to input YAML file (uses stdin if not specified)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Path to output .sav file (uses stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Steam ID for encryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,
    },

    /// Edit a save file in your $EDITOR
    Edit {
        /// Path to .sav file
        #[arg(short, long)]
        input: PathBuf,

        /// Steam ID for decryption/encryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

        /// Create backup before editing
        #[arg(short, long, default_value_t = true)]
        backup: bool,
    },

    /// Get specific values from a save file
    Get {
        /// Path to .sav file
        #[arg(short, long)]
        input: PathBuf,

        /// Steam ID for decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

        /// YAML path query (e.g. "state.currencies.cash" or "state.experience[0].level")
        query: Option<String>,

        /// Show character level and XP
        #[arg(long)]
        level: bool,

        /// Show currency (cash, eridium)
        #[arg(long)]
        money: bool,

        /// Show character info (name, class, difficulty)
        #[arg(long)]
        info: bool,

        /// Show all available data
        #[arg(long)]
        all: bool,
    },

    /// Set specific values in a save file
    Set {
        /// Path to .sav file
        #[arg(short, long)]
        input: PathBuf,

        /// Steam ID for encryption/decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

        /// YAML path to modify (e.g. "state.currencies.cash" or "state.experience[0].level")
        path: String,

        /// Value to set (auto-detects numbers vs strings, unless --raw is used)
        value: String,

        /// Treat value as raw YAML (for complex/unknown structures)
        #[arg(short, long)]
        raw: bool,

        /// Create backup before modifying
        #[arg(short, long, default_value_t = true)]
        backup: bool,
    },
}

#[derive(Subcommand)]
enum SerialCommand {
    /// Decode an item serial number
    Decode {
        /// Item serial to decode (e.g. @Ugr$ZCm/&tH!t{KgK/Shxu>k)
        serial: String,

        /// Show detailed byte-by-byte breakdown
        #[arg(short, long)]
        verbose: bool,

        /// Show bit-by-bit parsing debug output
        #[arg(short, long)]
        debug: bool,

        /// Analyze first token bit structure for group ID research
        #[arg(short, long)]
        analyze: bool,

        /// Path to parts database for resolving part names
        #[arg(long, default_value = "share/manifest/parts_database.json")]
        parts_db: PathBuf,
    },

    /// Re-encode a serial (for testing round-trip encoding)
    Encode {
        /// Item serial to decode and re-encode
        serial: String,
    },

    /// Compare two item serials side by side
    Compare {
        /// First serial
        serial1: String,

        /// Second serial
        serial2: String,
    },

    /// Modify a serial by swapping parts from another serial
    Modify {
        /// Base serial to modify
        #[arg(short, long)]
        base: String,

        /// Source serial to take parts from
        #[arg(short, long)]
        source: String,

        /// Part indices to copy from source (e.g. "4,12" for body and barrel)
        #[arg(short, long)]
        parts: String,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Show info about the attached process
    Info,

    /// Discover UE5 structures (GNames, GUObjectArray)
    Discover {
        /// What to discover (gnames, guobjectarray, all)
        #[arg(default_value = "all")]
        target: String,
    },

    /// List UObjects by class name
    Objects {
        /// Class name to filter by (e.g. "RarityWeightData", "ItemPoolDef")
        #[arg(short, long)]
        class: Option<String>,

        /// Maximum number of objects to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Dump usmap mappings file from live process
    DumpUsmap {
        /// Output path for usmap file
        #[arg(short, long, default_value = "BL4.usmap")]
        output: PathBuf,
    },

    /// Look up an FName by index
    Fname {
        /// FName index to look up
        index: u32,

        /// Show raw bytes at the FName entry (for debugging)
        #[arg(long)]
        debug: bool,
    },

    /// Search for an FName by string
    FnameSearch {
        /// String to search for in the FName pool
        query: String,
    },

    /// Search for Class UClass by scanning for self-referential objects
    FindClassUClass,

    /// List all UClass instances in memory (uses discovered metaclass address)
    ListUClasses {
        /// Maximum number of classes to show (0 = all)
        #[arg(short, long, default_value = "50")]
        limit: usize,

        /// Filter by class name pattern (case-insensitive)
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Enumerate UObjects from GUObjectArray
    ListObjects {
        /// Maximum number of objects to show
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Filter by class name pattern (case-insensitive)
        #[arg(short = 'c', long)]
        class_filter: Option<String>,

        /// Filter by object name pattern (case-insensitive)
        #[arg(short = 'n', long)]
        name_filter: Option<String>,

        /// Show statistics only (don't list individual objects)
        #[arg(long)]
        stats: bool,
    },

    /// Analyze dump file: discover UObject layout, FName pool, and UClass metaclass
    AnalyzeDump,

    /// List current inventory items
    ListInventory,

    /// Read a value from game memory
    Read {
        /// Memory address (hex, e.g. 0x7f1234567890)
        address: String,

        /// Number of bytes to read
        #[arg(short, long, default_value = "64")]
        size: usize,
    },

    /// Write bytes to game memory
    Write {
        /// Memory address (hex, e.g. 0x7f1234567890)
        address: String,

        /// Hex bytes to write (e.g. "90 90 90" for NOPs)
        bytes: String,
    },

    /// Scan for a pattern in memory
    Scan {
        /// Hex pattern to search for (e.g. "48 8B 05 ?? ?? ?? ??")
        pattern: String,
    },

    /// Patch a single instruction (replaces with NOPs or custom bytes)
    Patch {
        /// Memory address to patch (hex)
        address: String,

        /// Number of bytes to NOP out
        #[arg(short, long)]
        nop: Option<usize>,

        /// Custom replacement bytes (hex, e.g. "EB 05" for short jump)
        #[arg(short, long)]
        bytes: Option<String>,
    },

    /// Apply template modifications (e.g. dropRate=max, dropRarity=legendary)
    Apply {
        /// Template assignments (e.g. "dropRate=max" "dropRarity=legendary")
        #[arg(required = true)]
        templates: Vec<String>,
    },

    /// List available injection templates
    Templates,

    /// Monitor the preload library log file
    Monitor {
        /// Path to log file
        #[arg(short, long, default_value = "/tmp/bl4_preload.log")]
        log_file: PathBuf,

        /// Filter log entries by function name
        #[arg(short, long)]
        filter: Option<String>,

        /// Only show entries from addresses in game code (not libraries)
        #[arg(long)]
        game_only: bool,
    },

    /// Search for a string in memory and dump context around matches
    ScanString {
        /// String to search for
        query: String,

        /// Bytes to show before the match
        #[arg(short = 'B', long, default_value = "64")]
        before: usize,

        /// Bytes to show after the match
        #[arg(short = 'A', long, default_value = "64")]
        after: usize,

        /// Maximum number of matches to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Extract part definitions from memory dump (searches for XXX_YY.part_* patterns)
    DumpParts {
        /// Output file for parts JSON
        #[arg(short, long, default_value = "parts_dump.json")]
        output: PathBuf,
    },

    /// Build parts database with Category/Index mappings
    BuildPartsDb {
        /// Input parts dump JSON (from dump-parts command)
        #[arg(short, long, default_value = "share/manifest/parts_dump.json")]
        input: PathBuf,

        /// Output parts database JSON
        #[arg(short, long, default_value = "share/manifest/parts_database.json")]
        output: PathBuf,

        /// Part categories mapping JSON (prefix -> category ID)
        #[arg(short, long, default_value = "share/manifest/part_categories.json")]
        categories: PathBuf,
    },

    /// Extract part definitions from UObjects with authoritative Category/Index from SerialIndex
    ExtractParts {
        /// Output file for extracted parts with categories
        #[arg(short, long, default_value = "parts_with_categories.json")]
        output: PathBuf,

        /// Just list all FNames containing .part_ without extracting (for debugging)
        #[arg(long)]
        list_fnames: bool,
    },

    /// Find objects matching a name pattern to discover their class
    FindObjectsByPattern {
        /// Name pattern to search for (e.g. ".part_")
        pattern: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Generate an object map JSON for fast lookups on subsequent runs
    GenerateObjectMap {
        /// Output file for object map JSON
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Output format for idb list command
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Csv,
    Json,
}

#[cfg(feature = "research")]
#[derive(Subcommand)]
enum UsmapCommand {
    /// Show info about a usmap file
    Info {
        /// Path to usmap file
        path: PathBuf,
    },

    /// Search usmap for struct/enum names
    Search {
        /// Path to usmap file
        path: PathBuf,

        /// Search pattern (case-insensitive substring match)
        pattern: String,

        /// Show struct properties
        #[arg(short, long)]
        verbose: bool,
    },
}

#[cfg(feature = "research")]
#[derive(Subcommand)]
enum ExtractCommand {
    /// Extract part pools from the parts database
    #[command(visible_alias = "pp")]
    PartPools {
        /// Input parts database JSON
        #[arg(short, long, default_value = "share/manifest/parts_database.json")]
        input: PathBuf,

        /// Output part pools JSON
        #[arg(short, long, default_value = "share/manifest/part_pools.json")]
        output: PathBuf,
    },

    /// Extract manufacturer data from pak_manifest.json
    #[command(visible_alias = "m")]
    Manufacturers {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/manufacturers.json")]
        output: PathBuf,
    },

    /// Extract weapon type data from pak_manifest.json
    #[command(visible_alias = "wt")]
    WeaponTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/weapon_types.json")]
        output: PathBuf,
    },

    /// Extract gear type data from pak_manifest.json
    #[command(visible_alias = "gt")]
    GearTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/gear_types.json")]
        output: PathBuf,
    },

    /// Extract element types from pak_manifest.json
    #[command(visible_alias = "el")]
    Elements {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/elements.json")]
        output: PathBuf,
    },

    /// Extract rarity tiers from pak_manifest.json
    #[command(visible_alias = "ra")]
    Rarities {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/rarities.json")]
        output: PathBuf,
    },

    /// Extract stat types from pak_manifest.json
    #[command(visible_alias = "st")]
    Stats {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/stats.json")]
        output: PathBuf,
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

        /// Fields to include (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        fields: Option<Vec<String>>,
    },

    /// Show details for a specific item
    Show {
        /// Item serial
        serial: String,
    },

    /// Add an image attachment to an item
    Attach {
        /// Path to image file
        image: PathBuf,

        /// Item serial
        serial: String,

        /// Attachment name (defaults to filename without extension)
        #[arg(short, long)]
        name: Option<String>,

        /// Mark as POPUP view (item card)
        #[arg(long)]
        popup: bool,

        /// Mark as DETAIL view (3D inspect)
        #[arg(long)]
        detail: bool,
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
    Merge {
        /// Source database to merge FROM
        source: PathBuf,

        /// Destination database to merge TO
        dest: PathBuf,
    },

    /// Set a field value with source attribution
    SetValue {
        /// Item serial
        serial: String,

        /// Field name (e.g., level, rarity, manufacturer)
        field: String,

        /// Value to set
        value: String,

        /// Value source: ingame, decoder, community
        #[arg(long, short, default_value = "decoder")]
        source: String,

        /// Source detail (e.g., tool name)
        #[arg(long)]
        source_detail: Option<String>,

        /// Confidence: verified, inferred, uncertain
        #[arg(long, short, default_value = "inferred")]
        confidence: String,
    },

    /// Show all values for a field (from all sources)
    GetValues {
        /// Item serial
        serial: String,

        /// Field name (e.g., level, rarity)
        field: String,
    },

    /// Migrate existing column values to item_values table
    MigrateValues {
        /// Only show what would be migrated, don't actually migrate
        #[arg(long)]
        dry_run: bool,
    },

    /// Publish items to the community server
    Publish {
        /// Server URL
        #[arg(long, short, default_value = "https://bl4.monokro.me")]
        server: String,

        /// Only publish a specific item
        #[arg(long)]
        serial: Option<String>,

        /// Also upload attachments (screenshots)
        #[arg(long)]
        attachments: bool,

        /// Only show what would be published, don't actually publish
        #[arg(long)]
        dry_run: bool,
    },

    /// Pull items from a community server and merge into local database
    Pull {
        /// Server URL
        #[arg(long, short, default_value = "https://bl4.monokro.me")]
        server: String,

        /// Prefer remote values over local values (overwrite existing)
        #[arg(long)]
        authoritative: bool,

        /// Only show what would be pulled, don't actually pull
        #[arg(long)]
        dry_run: bool,
    },
}

/// Helper function to get Steam ID from argument or config
fn get_steam_id(provided: Option<String>) -> Result<String> {
    if let Some(id) = provided {
        return Ok(id);
    }

    let config = Config::load()?;
    config.get_steam_id().map(String::from).context(
        "Steam ID not provided. Run 'bl4 configure --steam-id YOUR_STEAM_ID' to set a default.",
    )
}

/// Helper function to update backup metadata after editing a save file
fn update_backup_metadata(input: &std::path::Path) -> Result<()> {
    let (_, metadata_path) = bl4::backup::backup_paths(input);
    bl4::update_after_edit(input, &metadata_path).context("Failed to update backup metadata")
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Configure { steam_id, show } => {
            let mut config = Config::load()?;

            if show {
                if let Some(id) = config.get_steam_id() {
                    println!("Steam ID: {}", id);
                } else {
                    println!("No Steam ID configured");
                }
                if let Ok(path) = Config::config_path() {
                    println!("Config file: {}", path.display());
                }
                return Ok(());
            }

            if let Some(id) = steam_id {
                config.set_steam_id(id.clone());
                config.save()?;
                println!("Steam ID configured: {}", id);
                if let Ok(path) = Config::config_path() {
                    println!("Config saved to: {}", path.display());
                }
            } else {
                println!("Usage: bl4 configure --steam-id YOUR_STEAM_ID");
                println!("   or: bl4 configure --show");
                println!();
                println!("Note: Borderlands 4 uses your Steam ID to encrypt saves.");
                println!("      Find it in the top left of your Steam account page.");
            }
        }

        Commands::Save { command } => match command {
            SaveCommand::Decrypt {
                input,
                output,
                steam_id,
            } => {
                let steam_id = get_steam_id(steam_id)?;
                let encrypted = file_io::read_input(input.as_deref())?;
                let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)
                    .context("Failed to decrypt save file")?;
                file_io::write_output(output.as_deref(), &yaml_data)?;
            }

            SaveCommand::Encrypt {
                input,
                output,
                steam_id,
            } => {
                let steam_id = get_steam_id(steam_id)?;
                let yaml_data = file_io::read_input(input.as_deref())?;
                let encrypted = bl4::encrypt_sav(&yaml_data, &steam_id)
                    .context("Failed to encrypt YAML data")?;
                file_io::write_output(output.as_deref(), &encrypted)?;
            }

            SaveCommand::Edit {
                input,
                steam_id,
                backup,
            } => {
                let steam_id = get_steam_id(steam_id)?;
                // Get editor from environment and parse it
                let editor_str = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                let editor_parts: Vec<&str> = editor_str.split_whitespace().collect();
                let (editor, editor_args) = if editor_parts.is_empty() {
                    ("vim", vec![])
                } else {
                    (editor_parts[0], editor_parts[1..].to_vec())
                };

                // Smart backup if requested
                if backup {
                    let _ = bl4::smart_backup(&input).context("Failed to manage backup")?;
                }

                // Decrypt to temp file
                let encrypted = fs::read(&input)
                    .with_context(|| format!("Failed to read {}", input.display()))?;

                let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)
                    .context("Failed to decrypt save file")?;

                let temp_path = input.with_extension("yaml.tmp");
                let abs_temp_path = std::fs::canonicalize(temp_path.parent().unwrap())
                    .unwrap()
                    .join(temp_path.file_name().unwrap());

                fs::write(&abs_temp_path, &yaml_data).with_context(|| {
                    format!("Failed to write temp file {}", abs_temp_path.display())
                })?;

                // Open editor
                let mut cmd = Command::new(editor);
                cmd.args(&editor_args);
                cmd.arg(&abs_temp_path);
                let status = cmd
                    .status()
                    .with_context(|| format!("Failed to launch editor: {}", editor))?;

                if !status.success() {
                    bail!("Editor exited with non-zero status");
                }

                // Re-encrypt
                let modified_yaml =
                    fs::read(&abs_temp_path).context("Failed to read modified temp file")?;

                let encrypted = bl4::encrypt_sav(&modified_yaml, &steam_id)
                    .context("Failed to encrypt modified YAML")?;

                fs::write(&input, &encrypted)
                    .with_context(|| format!("Failed to write {}", input.display()))?;

                // Clean up temp file
                fs::remove_file(&abs_temp_path).context("Failed to remove temp file")?;

                // Update hash tracking after edit
                if backup {
                    update_backup_metadata(&input)?;
                }
            }

            SaveCommand::Get {
                input,
                steam_id,
                query,
                level,
                money,
                info,
                all,
            } => {
                let steam_id = get_steam_id(steam_id)?;
                let encrypted = fs::read(&input)
                    .with_context(|| format!("Failed to read {}", input.display()))?;

                let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)
                    .context("Failed to decrypt save file")?;

                let save =
                    bl4::SaveFile::from_yaml(&yaml_data).context("Failed to parse save file")?;

                // Handle query path if provided
                if let Some(query_path) = query {
                    let result = save.get(&query_path).context("Query failed")?;
                    println!("{}", serde_yaml::to_string(&result)?);
                    return Ok(());
                }

                let show_all = all || (!level && !money && !info);

                if show_all || info {
                    // Character info
                    if let Some(name) = save.get_character_name() {
                        println!("Character: {}", name);
                    }
                    if let Some(class) = save.get_character_class() {
                        println!("Class: {}", class);
                    }
                    if let Some(diff) = save.get_difficulty() {
                        println!("Difficulty: {}", diff);
                    }
                    if show_all || info {
                        println!();
                    }
                }

                if show_all || level {
                    // Level info
                    if let Some((lvl, pts)) = save.get_character_level() {
                        println!("Character Level: {} ({} XP)", lvl, pts);
                    }
                    if let Some((lvl, pts)) = save.get_specialization_level() {
                        println!("Specialization Level: {} ({} XP)", lvl, pts);
                    }
                    if show_all || level {
                        println!();
                    }
                }

                if show_all || money {
                    // Currency info
                    if let Some(cash) = save.get_cash() {
                        println!("Cash: {}", cash);
                    }
                    if let Some(eridium) = save.get_eridium() {
                        println!("Eridium: {}", eridium);
                    }
                }
            }

            SaveCommand::Set {
                input,
                steam_id,
                path,
                value,
                raw,
                backup,
            } => {
                let steam_id = get_steam_id(steam_id)?;

                // Smart backup if requested
                if backup {
                    let _ = bl4::smart_backup(&input).context("Failed to manage backup")?;
                }

                // Read and decrypt
                let encrypted = fs::read(&input)
                    .with_context(|| format!("Failed to read {}", input.display()))?;

                let yaml_data = bl4::decrypt_sav(&encrypted, &steam_id)
                    .context("Failed to decrypt save file")?;

                let mut save =
                    bl4::SaveFile::from_yaml(&yaml_data).context("Failed to parse save file")?;

                // Parse and set the new value
                if raw {
                    eprintln!("Setting {} = {} (raw YAML)", path, value);
                    save.set_raw(&path, &value)
                        .context("Failed to set raw value")?;
                } else {
                    let new_value = bl4::SaveFile::parse_value(&value);
                    eprintln!("Setting {} = {}", path, value);
                    save.set(&path, new_value).context("Failed to set value")?;
                }

                // Re-serialize to YAML
                let modified_yaml = save.to_yaml().context("Failed to serialize YAML")?;

                // Re-encrypt
                let encrypted = bl4::encrypt_sav(&modified_yaml, &steam_id)
                    .context("Failed to encrypt save file")?;

                // Write back
                fs::write(&input, &encrypted)
                    .with_context(|| format!("Failed to write {}", input.display()))?;

                // Update hash tracking after edit
                if backup {
                    update_backup_metadata(&input)?;
                }
            }
        },

        Commands::Inspect {
            input,
            steam_id,
            full,
        } => {
            let steam_id = get_steam_id(steam_id)?;

            let encrypted =
                fs::read(&input).with_context(|| format!("Failed to read {}", input.display()))?;

            let yaml_data =
                bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

            if full {
                // Print entire YAML
                println!("{}", String::from_utf8_lossy(&yaml_data));
            } else {
                // Parse and show basic info
                let save: serde_yaml::Value =
                    serde_yaml::from_slice(&yaml_data).context("Failed to parse YAML")?;

                println!("Save file structure:");
                if let Some(obj) = save.as_mapping() {
                    for key in obj.keys() {
                        println!("  - {}", key.as_str().unwrap_or("?"));
                    }
                }

                println!("\nUse --full to see complete YAML");
            }
        }

        Commands::Serial { command } => match command {
            SerialCommand::Decode {
                serial,
                verbose,
                debug,
                analyze,
                parts_db,
            } => {
                let item = bl4::ItemSerial::decode(&serial).context("Failed to decode serial")?;

                println!("Serial: {}", item.original);
                println!(
                    "Item type: {} ({})",
                    item.item_type,
                    item.item_type_description()
                );

                // Show weapon info based on format type
                if let Some((mfr, weapon_type)) = item.weapon_info() {
                    // VarInt-first format: first VarInt encodes manufacturer + weapon type
                    println!("Weapon: {} {}", mfr, weapon_type);
                } else if let Some(group_id) = item.part_group_id() {
                    // VarBit-first format: use Part Group ID for category lookup
                    // Use type-aware lookup to handle overlapping category numbers
                    let category_name =
                        bl4::category_name_for_type(item.item_type, group_id).unwrap_or("Unknown");
                    println!("Category: {} ({})", category_name, group_id);
                }

                // Show elements if detected
                if let Some(elements) = item.element_names() {
                    println!("Element: {}", elements);
                }

                // Show rarity if detected
                if let Some(rarity) = item.rarity_name() {
                    println!("Rarity: {}", rarity);
                }

                // Show raw manufacturer ID if we couldn't resolve it
                if item.weapon_info().is_none() {
                    if let Some(mfr) = item.manufacturer_name() {
                        println!("Manufacturer: {}", mfr);
                    } else if let Some(mfr_id) = item.manufacturer {
                        println!("Manufacturer ID: {} (unknown)", mfr_id);
                    }
                }

                // Show level and seed for VarInt-first format
                if let Some(level) = item.level {
                    if let Some(raw) = item.raw_level {
                        if raw > level {
                            println!(
                            "Level: {} (WARNING: decoded as {}, capped - decoding may be wrong)",
                            level, raw
                        );
                        } else {
                            println!("Level: {}", level);
                        }
                    } else {
                        println!("Level: {}", level);
                    }
                }
                if let Some(seed) = item.seed {
                    println!("Seed: {}", seed);
                }

                println!("Decoded bytes: {}", item.raw_bytes.len());
                println!("Hex: {}", item.hex_dump());
                println!("Tokens: {}", item.format_tokens());

                // Try to resolve part names from database
                // Get category from VarBit-first format or convert VarInt serial ID
                let category: Option<i64> = item.parts_category();

                // Resolve part names if we have a category and parts database
                let parts = item.parts();
                if let (Some(category), false) = (category, parts.is_empty()) {
                    #[derive(Debug, Deserialize)]
                    struct PartsDb {
                        parts: Vec<PartDbEntry>,
                    }
                    #[derive(Debug, Deserialize)]
                    struct PartDbEntry {
                        name: String,
                        category: i64,
                        index: i64,
                    }

                    if let Ok(db_content) = fs::read_to_string(&parts_db) {
                        if let Ok(db) = serde_json::from_str::<PartsDb>(&db_content) {
                            let lookup: std::collections::HashMap<(i64, i64), &str> = db
                                .parts
                                .iter()
                                .map(|p| ((p.category, p.index), p.name.as_str()))
                                .collect();

                            println!("\nResolved parts:");
                            for (part_index, values) in &parts {
                                // Part index encoding: bit 7 = flag, bits 0-6 = actual index
                                let has_flag = *part_index >= 128;
                                let actual_index = if has_flag {
                                    (*part_index & 0x7F) as i64 // Mask off bit 7
                                } else {
                                    *part_index as i64
                                };
                                let flag_str = if has_flag { " [+]" } else { "" };
                                let extra = if values.is_empty() {
                                    String::new()
                                } else {
                                    format!(" (values: {:?})", values)
                                };
                                if let Some(name) = lookup.get(&(category, actual_index)) {
                                    println!("  {}{}{}", name, flag_str, extra);
                                } else {
                                    let idx_display = if has_flag {
                                        format!(
                                            "{} (0x{:02x} = flag + {})",
                                            part_index, part_index, actual_index
                                        )
                                    } else {
                                        format!("{}", part_index)
                                    };
                                    println!("  [unknown part index {}]{}", idx_display, extra);
                                }
                            }
                        }
                    }
                }

                if verbose {
                    println!("\n{}", item.detailed_dump());
                }

                if debug {
                    println!("\nDebug parsing:");
                    bl4::serial::parse_tokens_debug(&item.raw_bytes);
                }

                if analyze {
                    // Analyze first token for group ID research
                    use bl4::serial::Token;
                    if let Some(first_token) = item.tokens.first() {
                        let value = match first_token {
                            Token::VarInt(v) => Some((*v, "VarInt")),
                            Token::VarBit(v) => Some((*v, "VarBit")),
                            _ => None,
                        };

                        if let Some((value, token_type)) = value {
                            println!("\n=== First Token Analysis ===");
                            println!("Type:   {}", token_type);
                            println!("Value:  {} (decimal)", value);
                            println!("Hex:    0x{:x}", value);
                            println!("Binary: {:024b}", value);
                            println!();

                            // Decode Part Group ID based on item type
                            println!("Part Group ID decoding:");
                            match item.item_type {
                                'r' | 'a'..='d' | 'f' | 'g' | 'v'..='z' => {
                                    // Weapons: group_id = first_token / 8192
                                    let group_id = value / 8192;
                                    let offset = value % 8192;
                                    println!("  Formula: group_id = value / 8192 (weapons)");
                                    println!("  Group ID: {} (offset {})", group_id, offset);

                                    // Use the authoritative category_name function from parts.rs
                                    let group_name =
                                        bl4::category_name(group_id as i64).unwrap_or("Unknown");
                                    println!("  Identified: {}", group_name);
                                }
                                'e' => {
                                    // Equipment: group_id = first_token / 384
                                    let group_id = value / 384;
                                    let offset = value % 384;
                                    println!("  Formula: group_id = value / 384 (equipment)");
                                    println!("  Group ID: {} (offset {})", group_id, offset);

                                    // Use the authoritative category_name function from parts.rs
                                    let group_name = bl4::category_name(group_id as i64)
                                        .unwrap_or("Unknown Equipment");
                                    println!("  Identified: {}", group_name);
                                }
                                'u' => {
                                    // Utility items - formula TBD
                                    println!(
                                        "  Utility items - encoding formula not yet determined"
                                    );
                                    println!("  Raw value: {}", value);
                                }
                                '!' | '#' => {
                                    // Class mods - formula TBD
                                    println!("  Class mods - encoding formula not yet determined");
                                    println!("  Raw value: {}", value);
                                }
                                _ => {
                                    println!(
                                    "  Unknown item type '{}' - encoding formula not determined",
                                    item.item_type
                                );
                                }
                            }

                            println!();
                            println!("Bit split analysis (for research):");
                            for split in [8, 10, 12, 13, 14] {
                                let high = value >> split;
                                let low = value & ((1 << split) - 1);
                                println!(
                                    "  Split at bit {:2}: high={:6}  low={:6}",
                                    split, high, low
                                );
                            }
                        } else {
                            println!("\n=== First Token Analysis ===");
                            println!("First token is not numeric: {:?}", first_token);
                        }
                    }
                }
            }

            SerialCommand::Encode { serial } => {
                let item = bl4::ItemSerial::decode(&serial).context("Failed to decode serial")?;
                let re_encoded = item.encode();

                println!("Original:   {}", serial);
                println!("Re-encoded: {}", re_encoded);

                if serial == re_encoded {
                    println!("\n✓ Round-trip encoding successful!");
                } else {
                    println!("\n✗ Round-trip encoding differs");
                    println!("  Original length:   {}", serial.len());
                    println!("  Re-encoded length: {}", re_encoded.len());

                    // Decode both to compare tokens
                    let re_item = bl4::ItemSerial::decode(&re_encoded)?;
                    println!("\nOriginal tokens:   {}", item.format_tokens());
                    println!("Re-encoded tokens: {}", re_item.format_tokens());
                }
            }

            SerialCommand::Compare { serial1, serial2 } => {
                let item1 =
                    bl4::ItemSerial::decode(&serial1).context("Failed to decode serial 1")?;
                let item2 =
                    bl4::ItemSerial::decode(&serial2).context("Failed to decode serial 2")?;

                // Header comparison
                println!("=== SERIAL 1 ===");
                println!("Serial: {}", item1.original);
                println!(
                    "Type: {} ({})",
                    item1.item_type,
                    item1.item_type_description()
                );
                if let Some((mfr, wtype)) = item1.weapon_info() {
                    println!("Weapon: {} {}", mfr, wtype);
                }
                if let Some(level) = item1.level {
                    println!("Level: {}", level);
                }
                if let Some(seed) = item1.seed {
                    println!("Seed: {}", seed);
                }
                println!("Tokens: {}", item1.format_tokens());

                println!();
                println!("=== SERIAL 2 ===");
                println!("Serial: {}", item2.original);
                println!(
                    "Type: {} ({})",
                    item2.item_type,
                    item2.item_type_description()
                );
                if let Some((mfr, wtype)) = item2.weapon_info() {
                    println!("Weapon: {} {}", mfr, wtype);
                }
                if let Some(level) = item2.level {
                    println!("Level: {}", level);
                }
                if let Some(seed) = item2.seed {
                    println!("Seed: {}", seed);
                }
                println!("Tokens: {}", item2.format_tokens());

                // Byte comparison
                println!();
                println!("=== BYTE COMPARISON ===");
                println!(
                    "Lengths: {} vs {} bytes",
                    item1.raw_bytes.len(),
                    item2.raw_bytes.len()
                );

                let max_len = std::cmp::max(item1.raw_bytes.len(), item2.raw_bytes.len());
                let mut first_diff = None;
                let mut diff_count = 0;

                for i in 0..max_len {
                    let b1 = item1.raw_bytes.get(i);
                    let b2 = item2.raw_bytes.get(i);
                    if b1 != b2 {
                        diff_count += 1;
                        if first_diff.is_none() {
                            first_diff = Some(i);
                        }
                    }
                }

                if diff_count == 0 {
                    println!("Bytes: IDENTICAL");
                } else {
                    println!("Bytes: {} differences", diff_count);
                    if let Some(first) = first_diff {
                        println!("First diff at byte {}", first);
                        println!();
                        println!("Byte-by-byte (first 20 bytes or until divergence + 5):");
                        println!("{:>4}  {:>12}  {:>12}", "Idx", "Serial 1", "Serial 2");
                        let show_until = std::cmp::min(max_len, first + 10);
                        for i in 0..show_until {
                            let b1 = item1.raw_bytes.get(i).copied();
                            let b2 = item2.raw_bytes.get(i).copied();
                            let marker = if b1 != b2 { " <--" } else { "" };
                            let s1 = b1
                                .map(|b| format!("{:3} {:08b}", b, b))
                                .unwrap_or_else(|| "-".to_string());
                            let s2 = b2
                                .map(|b| format!("{:3} {:08b}", b, b))
                                .unwrap_or_else(|| "-".to_string());
                            println!("{:4}  {}  {}{}", i, s1, s2, marker);
                        }
                    }
                }
            }

            SerialCommand::Modify {
                base,
                source,
                parts,
            } => {
                use bl4::serial::Token;

                // Parse part indices
                let part_indices: Vec<u64> = parts
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();

                if part_indices.is_empty() {
                    bail!("No valid part indices provided");
                }

                let base_item =
                    bl4::ItemSerial::decode(&base).context("Failed to decode base serial")?;
                let source_item =
                    bl4::ItemSerial::decode(&source).context("Failed to decode source serial")?;

                println!("Base serial:   {}", base);
                println!("Source serial: {}", source);
                println!("Copying part indices: {:?}", part_indices);
                println!();

                // Build a map of source parts by index
                let source_parts: std::collections::HashMap<u64, Vec<u64>> = source_item
                    .tokens
                    .iter()
                    .filter_map(|t| {
                        if let Token::Part { index, values } = t {
                            Some((*index, values.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Modify base tokens - replace parts at specified indices
                let new_tokens: Vec<Token> = base_item
                    .tokens
                    .iter()
                    .map(|t| {
                        if let Token::Part { index, values } = t {
                            if part_indices.contains(index) {
                                if let Some(source_values) = source_parts.get(index) {
                                    println!(
                                        "  Swapping part {}: {:?} -> {:?}",
                                        index, values, source_values
                                    );
                                    return Token::Part {
                                        index: *index,
                                        values: source_values.clone(),
                                    };
                                }
                            }
                        }
                        t.clone()
                    })
                    .collect();

                // Encode the new serial
                let modified = base_item.with_tokens(new_tokens);
                let new_serial = modified.encode();

                println!();
                println!("New serial: {}", new_serial);
            }
        },

        Commands::Parts {
            weapon,
            category,
            list,
            parts_db,
        } => {
            // Load parts database
            let db_content = fs::read_to_string(&parts_db)
                .with_context(|| format!("Failed to read parts database: {:?}", parts_db))?;

            #[derive(Debug, Deserialize)]
            struct PartsDatabase {
                parts: Vec<PartEntry>,
            }

            #[derive(Debug, Deserialize)]
            struct PartEntry {
                name: String,
                category: i64,
                index: i64,
            }

            let db: PartsDatabase =
                serde_json::from_str(&db_content).context("Failed to parse parts database")?;

            // Build category -> parts mapping
            let mut by_category: std::collections::BTreeMap<i64, Vec<&PartEntry>> =
                std::collections::BTreeMap::new();
            for part in &db.parts {
                by_category.entry(part.category).or_default().push(part);
            }

            if list {
                // List all categories
                println!("Available categories:");
                println!();
                for (&cat_id, parts) in &by_category {
                    let cat_name = bl4::category_name(cat_id).unwrap_or("Unknown");
                    println!("  {:3}: {} ({} parts)", cat_id, cat_name, parts.len());
                }
                println!();
                println!(
                    "Total: {} categories, {} parts",
                    by_category.len(),
                    db.parts.len()
                );
                return Ok(());
            }

            // Find target category
            let target_cat: Option<i64> = if let Some(cat) = category {
                Some(cat)
            } else if let Some(ref wname) = weapon {
                // Search for category by weapon name
                let search = wname.to_lowercase();
                let mut found = None;
                for &cat_id in by_category.keys() {
                    if let Some(name) = bl4::category_name(cat_id) {
                        if name.to_lowercase().contains(&search) {
                            if found.is_some() {
                                println!("Multiple matches for '{}'. Please be more specific or use -c <category_id>", wname);
                                for &c in by_category.keys() {
                                    if let Some(n) = bl4::category_name(c) {
                                        if n.to_lowercase().contains(&search) {
                                            println!("  {:3}: {}", c, n);
                                        }
                                    }
                                }
                                return Ok(());
                            }
                            found = Some(cat_id);
                        }
                    }
                }
                found
            } else {
                None
            };

            if let Some(cat_id) = target_cat {
                let cat_name = bl4::category_name(cat_id).unwrap_or("Unknown");
                let parts = by_category.get(&cat_id);

                println!("Parts for {} (category {}):", cat_name, cat_id);
                println!();

                if let Some(parts) = parts {
                    // Group by part type (barrel, grip, mag, etc.)
                    let mut by_type: std::collections::BTreeMap<String, Vec<&&PartEntry>> =
                        std::collections::BTreeMap::new();

                    for part in parts {
                        // Extract part type from name (e.g., "DAD_PS.part_barrel_01" -> "barrel")
                        let part_type = part
                            .name
                            .split(".part_")
                            .nth(1)
                            .and_then(|s| s.split('_').next())
                            .unwrap_or("other")
                            .to_string();
                        by_type.entry(part_type).or_default().push(part);
                    }

                    for (ptype, type_parts) in &by_type {
                        println!("  {} ({} variants):", ptype, type_parts.len());
                        for part in type_parts {
                            println!("    [{}] {}", part.index, part.name);
                        }
                        println!();
                    }

                    println!("Total: {} parts", parts.len());
                } else {
                    println!("  No parts found for this category");
                }
            } else {
                println!("Usage: bl4 parts --weapon <name> OR --category <id> OR --list");
                println!();
                println!("Examples:");
                println!("  bl4 parts --list                 # List all categories");
                println!("  bl4 parts --weapon 'Jakobs'      # Find Jakobs weapons");
                println!("  bl4 parts --category 3           # Show parts for category 3");
            }
        }

        Commands::Memory {
            preload,
            dump,
            maps,
            action,
        } => {
            // Handle commands that don't require process attachment first
            match action {
                MemoryAction::Templates => {
                    println!("Available injection templates:");
                    println!();
                    println!("  dropRate=<value>     - Modify drop rate probability");
                    println!("    Values: max, high, normal, low");
                    println!();
                    println!("  dropRarity=<value>   - Bias loot toward specific rarity");
                    println!("    Values: legendary, epic, rare, uncommon, common");
                    println!();
                    println!("  luck=<value>         - Set luck modifier");
                    println!("    Values: max, high, normal");
                    println!();
                    println!("Example usage:");
                    println!("  bl4 inject apply dropRate=max dropRarity=legendary");
                    println!("  bl4 inject --preload apply dropRate=max  (preload mode)");
                    println!();
                    println!("Note: Without --preload, templates require finding memory");
                    println!("      addresses at runtime. Use 'bl4 inject scan' to locate");
                    println!("      the relevant game data first.");
                    println!();
                    println!("      With --preload, modifications work via the LD_PRELOAD");
                    println!("      library and affect RNG at the syscall level.");
                    return Ok(());
                }
                MemoryAction::BuildPartsDb {
                    ref input,
                    ref output,
                    ref categories,
                } => {
                    // This command doesn't need memory access - just reads/writes JSON
                    println!("Building parts database from {}...", input.display());
                    println!("Loading categories from {}...", categories.display());

                    // Load part categories from JSON file
                    let categories_json = std::fs::read_to_string(categories)
                        .context("Failed to read part categories file")?;
                    let categories_file: PartCategoriesFile =
                        serde_json::from_str(&categories_json)
                            .context("Failed to parse part categories JSON")?;

                    // Convert to internal format (prefix, category_id, description)
                    let known_groups: Vec<(String, i64, String)> = categories_file
                        .categories
                        .into_iter()
                        .map(|cat| {
                            let description = if let Some(wt) = &cat.weapon_type {
                                if let Some(mfr) = &cat.manufacturer {
                                    format!("{} {}", mfr, wt)
                                } else {
                                    wt.clone()
                                }
                            } else if let Some(gt) = &cat.gear_type {
                                if let Some(mfr) = &cat.manufacturer {
                                    format!("{} {}", mfr, gt)
                                } else {
                                    gt.clone()
                                }
                            } else {
                                cat.prefix.clone()
                            };
                            (cat.prefix, cat.category, description)
                        })
                        .collect();

                    println!("Loaded {} category mappings", known_groups.len());

                    let parts_json =
                        std::fs::read_to_string(input).context("Failed to read parts dump file")?;

                    let mut parts_by_prefix: std::collections::BTreeMap<String, Vec<String>> =
                        std::collections::BTreeMap::new();
                    let mut current_prefix = String::new();
                    let mut in_array = false;

                    for line in parts_json.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with('"') && trimmed.contains("\": [") {
                            if let Some(end_quote) = trimmed[1..].find('"') {
                                current_prefix = trimmed[1..end_quote + 1].to_string();
                                in_array = true;
                                parts_by_prefix.insert(current_prefix.clone(), Vec::new());
                            }
                        } else if in_array && trimmed.starts_with('"') && !trimmed.contains(": [") {
                            let name = trimmed
                                .trim_end_matches(',')
                                .trim_end_matches('"')
                                .trim_start_matches('"')
                                .to_string();
                            if !name.is_empty() {
                                if let Some(parts) = parts_by_prefix.get_mut(&current_prefix) {
                                    parts.push(name);
                                }
                            }
                        } else if trimmed == "]" || trimmed == "]," {
                            in_array = false;
                        }
                    }

                    let mut db_entries: Vec<(i64, i16, String, String)> = Vec::new();

                    for (prefix, category, description) in &known_groups {
                        if let Some(parts) = parts_by_prefix.get(prefix) {
                            for (idx, part_name) in parts.iter().enumerate() {
                                db_entries.push((
                                    *category,
                                    idx as i16,
                                    part_name.clone(),
                                    description.clone(),
                                ));
                            }
                        }
                    }

                    let known_prefixes: std::collections::HashSet<&str> =
                        known_groups.iter().map(|(p, _, _)| p.as_str()).collect();

                    for (prefix, parts) in &parts_by_prefix {
                        if !known_prefixes.contains(prefix.as_str()) {
                            for (idx, part_name) in parts.iter().enumerate() {
                                db_entries.push((
                                    -1,
                                    idx as i16,
                                    part_name.clone(),
                                    format!("{} (unmapped)", prefix),
                                ));
                            }
                        }
                    }

                    let mut json = String::from("{\n  \"version\": 1,\n  \"parts\": [\n");
                    for (i, (category, index, name, group)) in db_entries.iter().enumerate() {
                        let escaped_name = name.replace('\\', "\\\\").replace('"', "\\\"");
                        let escaped_group = group.replace('\\', "\\\\").replace('"', "\\\"");
                        json.push_str(&format!(
                            "    {{\"category\": {}, \"index\": {}, \"name\": \"{}\", \"group\": \"{}\"}}",
                            category, index, escaped_name, escaped_group
                        ));
                        if i < db_entries.len() - 1 {
                            json.push(',');
                        }
                        json.push('\n');
                    }
                    json.push_str("  ],\n  \"categories\": {\n");

                    let mut category_counts: std::collections::BTreeMap<i64, (usize, String)> =
                        std::collections::BTreeMap::new();
                    for (category, _, _, group) in &db_entries {
                        let entry = category_counts
                            .entry(*category)
                            .or_insert((0, group.clone()));
                        entry.0 += 1;
                    }
                    let cat_count = category_counts.len();
                    for (i, (category, (count, name))) in category_counts.iter().enumerate() {
                        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
                        json.push_str(&format!(
                            "    \"{}\": {{\"count\": {}, \"name\": \"{}\"}}",
                            category, count, escaped
                        ));
                        if i < cat_count - 1 {
                            json.push(',');
                        }
                        json.push('\n');
                    }
                    json.push_str("  }\n}\n");

                    std::fs::write(output, &json)?;
                    println!(
                        "Built parts database with {} entries across {} categories",
                        db_entries.len(),
                        category_counts.len()
                    );
                    println!("Written to: {}", output.display());
                    return Ok(());
                }
                MemoryAction::ExtractParts {
                    ref output,
                    list_fnames,
                } => {
                    // This command works with both dump files and live memory
                    let source: Box<dyn memory::MemorySource> = match dump {
                        Some(ref p) => {
                            println!("Extracting part definitions from dump...");
                            Box::new(memory::DumpFile::open(p)?)
                        }
                        None => {
                            println!("Extracting part definitions from live process...");
                            let proc = memory::Bl4Process::attach()
                                .context("Failed to attach to Borderlands 4 process")?;
                            Box::new(proc)
                        }
                    };

                    // If --list-fnames, just dump all FNames containing .part_ and exit
                    if list_fnames {
                        println!("Listing all FNames containing '.part_'...");
                        let fnames = memory::list_all_part_fnames(source.as_ref())?;
                        for name in &fnames {
                            println!("{}", name);
                        }
                        println!("\nTotal: {} FNames", fnames.len());
                        return Ok(());
                    }

                    // Use the new FName array pattern extraction
                    // This method scans for 0xFFFFFFFF markers in part registration arrays,
                    // then follows pointers to read GbxSerialNumberIndex at UObject+0x20
                    let parts = memory::extract_parts_from_fname_arrays(source.as_ref())?;

                    println!("Found {} part definitions", parts.len());

                    // Group by category for summary
                    let mut by_category: std::collections::BTreeMap<
                        i64,
                        Vec<&memory::PartDefinition>,
                    > = std::collections::BTreeMap::new();
                    for part in &parts {
                        by_category.entry(part.category).or_default().push(part);
                    }

                    println!("\nCategories found:");
                    for (category, cat_parts) in &by_category {
                        let max_idx = cat_parts.iter().map(|p| p.index).max().unwrap_or(0);
                        println!(
                            "  Category {:3}: {:3} parts (max index: {})",
                            category,
                            cat_parts.len(),
                            max_idx
                        );
                    }

                    // Write output JSON
                    let mut json = String::from("{\n  \"parts\": [\n");
                    for (i, part) in parts.iter().enumerate() {
                        let escaped_name = part.name.replace('\\', "\\\\").replace('"', "\\\"");
                        json.push_str(&format!(
                            "    {{\"name\": \"{}\", \"category\": {}, \"index\": {}}}",
                            escaped_name, part.category, part.index
                        ));
                        if i < parts.len() - 1 {
                            json.push(',');
                        }
                        json.push('\n');
                    }
                    json.push_str("  ],\n  \"summary\": {\n");

                    let cat_count = by_category.len();
                    for (i, (category, cat_parts)) in by_category.iter().enumerate() {
                        json.push_str(&format!("    \"{}\": {}", category, cat_parts.len()));
                        if i < cat_count - 1 {
                            json.push(',');
                        }
                        json.push('\n');
                    }
                    json.push_str("  }\n}\n");

                    std::fs::write(output, &json)?;
                    println!("\nWritten to: {}", output.display());
                    return Ok(());
                }
                MemoryAction::FindObjectsByPattern { ref pattern, limit } => {
                    // This command requires a memory dump
                    let dump_path = match dump {
                        Some(ref p) => p.clone(),
                        None => bail!(
                            "FindObjectsByPattern requires a memory dump file. Use --dump <path>"
                        ),
                    };

                    println!("Searching for objects matching '{}'...", pattern);
                    let source: Box<dyn memory::MemorySource> =
                        Box::new(memory::DumpFile::open(&dump_path)?);

                    // Discover GUObjectArray
                    println!("Discovering GNames pool...");
                    let gnames = memory::discover_gnames(source.as_ref())?;
                    println!("  GNames at: {:#x}", gnames.address);

                    println!("Discovering GUObjectArray...");
                    let guobjects =
                        memory::discover_guobject_array(source.as_ref(), gnames.address)?;
                    println!("  GUObjectArray at: {:#x}", guobjects.address);
                    println!("  NumElements: {}", guobjects.num_elements);

                    // Find objects
                    let results = memory::find_objects_by_pattern(
                        source.as_ref(),
                        &guobjects,
                        pattern,
                        limit,
                    )?;

                    println!("\nResults:");
                    for (name, class_name, class_ptr) in &results {
                        println!("  '{}' (class: {} @ {:#x})", name, class_name, class_ptr);
                    }

                    return Ok(());
                }
                MemoryAction::GenerateObjectMap { ref output } => {
                    // This command requires a memory dump
                    let dump_path = match dump {
                        Some(ref p) => p.clone(),
                        None => bail!(
                            "GenerateObjectMap requires a memory dump file. Use --dump <path>"
                        ),
                    };

                    // Default output path is next to the dump file
                    let output_path = output.clone().unwrap_or_else(|| {
                        let mut p = dump_path.clone();
                        p.set_extension("objects.json");
                        p
                    });

                    println!("Generating object map from {}...", dump_path.display());
                    let source: Box<dyn memory::MemorySource> =
                        Box::new(memory::DumpFile::open(&dump_path)?);

                    // Discover GUObjectArray
                    println!("Discovering GNames pool...");
                    let gnames = memory::discover_gnames(source.as_ref())?;
                    println!("  GNames at: {:#x}", gnames.address);

                    println!("Discovering GUObjectArray...");
                    let guobjects =
                        memory::discover_guobject_array(source.as_ref(), gnames.address)?;
                    println!("  GUObjectArray at: {:#x}", guobjects.address);
                    println!("  NumElements: {}", guobjects.num_elements);

                    // Generate object map
                    let map = memory::generate_object_map(source.as_ref(), &guobjects)?;

                    // Write JSON output
                    let mut json = String::from("{\n");
                    let class_count = map.len();
                    for (i, (class_name, objects)) in map.iter().enumerate() {
                        let escaped_class = class_name.replace('\\', "\\\\").replace('"', "\\\"");
                        json.push_str(&format!("  \"{}\": [\n", escaped_class));
                        for (j, obj) in objects.iter().enumerate() {
                            let escaped_name = obj.name.replace('\\', "\\\\").replace('"', "\\\"");
                            json.push_str(&format!(
                                "    {{\"name\": \"{}\", \"address\": \"{:#x}\", \"class_address\": \"{:#x}\"}}",
                                escaped_name, obj.address, obj.class_address
                            ));
                            if j < objects.len() - 1 {
                                json.push(',');
                            }
                            json.push('\n');
                        }
                        json.push_str("  ]");
                        if i < class_count - 1 {
                            json.push(',');
                        }
                        json.push('\n');
                    }
                    json.push_str("}\n");

                    std::fs::write(&output_path, &json)?;
                    println!("Object map written to: {}", output_path.display());
                    println!(
                        "  {} classes, {} total objects",
                        map.len(),
                        map.values().map(|v| v.len()).sum::<usize>()
                    );

                    return Ok(());
                }
                _ => {}
            }

            // Preload mode - communicate with the preload library via environment/signals
            if preload {
                match action {
                    MemoryAction::Apply { templates } => {
                        // Find preload library path
                        let exe_dir = std::env::current_exe()
                            .ok()
                            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

                        let lib_path = exe_dir
                            .as_ref()
                            .map(|d| d.join("libbl4_preload.so"))
                            .filter(|p| p.exists())
                            .or_else(|| {
                                let p = PathBuf::from("target/release/libbl4_preload.so");
                                if p.exists() {
                                    Some(std::fs::canonicalize(p).unwrap_or_default())
                                } else {
                                    None
                                }
                            });

                        let lib_path = match lib_path {
                            Some(p) => p,
                            None => {
                                bail!(
                                    "Preload library not found. Build it first:\n  \
                                    cargo build --release -p bl4-preload"
                                );
                            }
                        };

                        // Parse templates into env vars
                        let mut env_vars = Vec::new();
                        for template in &templates {
                            let parts: Vec<&str> = template.splitn(2, '=').collect();
                            if parts.len() != 2 {
                                eprintln!(
                                    "Invalid template format: {} (expected key=value)",
                                    template
                                );
                                continue;
                            }

                            let key = parts[0].to_lowercase();
                            let value = parts[1].to_lowercase();

                            match key.as_str() {
                                "droprate" | "droprarity" => {
                                    // Map to BL4_DROP_BIAS
                                    let bias = match value.as_str() {
                                        "max" | "legendary" => "max",
                                        "high" | "epic" => "high",
                                        "normal" => "", // no bias
                                        "low" | "uncommon" => "low",
                                        "min" | "common" => "min",
                                        _ => {
                                            eprintln!("Unknown value for {}: {}", key, value);
                                            continue;
                                        }
                                    };
                                    if !bias.is_empty() {
                                        env_vars.push(format!("BL4_RNG_BIAS={}", bias));
                                    }
                                }
                                "luck" => {
                                    let bias = match value.as_str() {
                                        "max" => "max",
                                        "high" => "high",
                                        "normal" => "",
                                        _ => {
                                            eprintln!("Unknown value for luck: {}", value);
                                            continue;
                                        }
                                    };
                                    if !bias.is_empty() {
                                        env_vars.push(format!("BL4_RNG_BIAS={}", bias));
                                    }
                                }
                                _ => {
                                    eprintln!("Unknown template: {}", key);
                                }
                            }
                        }

                        // Deduplicate env vars
                        env_vars.sort();
                        env_vars.dedup();

                        if env_vars.is_empty() {
                            println!("LD_PRELOAD={} %command%", lib_path.display());
                        } else {
                            println!(
                                "LD_PRELOAD={} {} %command%",
                                lib_path.display(),
                                env_vars.join(" ")
                            );
                        }

                        return Ok(());
                    }
                    MemoryAction::Monitor { .. } => {
                        // Monitor works the same in preload mode, fall through
                        // This is handled below in the main match
                    }
                    _ => {
                        bail!(
                            "This command is not available in --preload mode. \
                               Remove --preload to use direct memory injection."
                        );
                    }
                }
            }

            // Commands that require memory access (live process or dump file)
            // Create memory source based on options
            let (process, dump_file): (Option<memory::Bl4Process>, Option<memory::DumpFile>) =
                if let Some(dump_path) = &dump {
                    // Using dump file for offline analysis
                    let dump = if let Some(ref maps_path) = maps {
                        memory::DumpFile::open_with_maps(dump_path, maps_path)
                            .context("Failed to open dump file with maps")?
                    } else {
                        memory::DumpFile::open(dump_path).context("Failed to open dump file")?
                    };
                    (None, Some(dump))
                } else {
                    // Attach to live process
                    let proc = memory::Bl4Process::attach()
                        .context("Failed to attach to Borderlands 4 process")?;
                    (Some(proc), None)
                };

            // Helper macro to get memory source
            macro_rules! mem_source {
                () => {
                    if let Some(ref p) = process {
                        p as &dyn memory::MemorySource
                    } else if let Some(ref d) = dump_file {
                        d as &dyn memory::MemorySource
                    } else {
                        unreachable!()
                    }
                };
            }

            match action {
                MemoryAction::Templates => unreachable!(),
                MemoryAction::BuildPartsDb { .. } => unreachable!(), // Handled above before process attach
                MemoryAction::ExtractParts { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::FindObjectsByPattern { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::GenerateObjectMap { .. } => unreachable!(), // Handled above with dump file

                MemoryAction::Info => {
                    if let Some(ref proc) = process {
                        println!("{}", proc.info());
                    } else {
                        println!("Dump file mode - no live process info available");
                        println!("  Dump: {:?}", dump.as_ref().unwrap());
                        let source = mem_source!();
                        println!("  Regions: {}", source.regions().len());
                    }
                }

                MemoryAction::Discover { target } => {
                    let source = mem_source!();
                    match target.to_lowercase().as_str() {
                        "gnames" | "all" => {
                            println!("Searching for GNames pool...");
                            match memory::discover_gnames(source) {
                                Ok(gnames) => {
                                    println!("GNames found at: {:#x}", gnames.address);
                                    println!("\nSample names:");
                                    for (idx, name) in &gnames.sample_names {
                                        println!("  [{}] {}", idx, name);
                                    }

                                    if target == "all" {
                                        println!("\nSearching for GUObjectArray...");
                                        match memory::discover_guobject_array(
                                            source,
                                            gnames.address,
                                        ) {
                                            Ok(arr) => {
                                                println!(
                                                    "GUObjectArray found at: {:#x}",
                                                    arr.address
                                                );
                                                println!("  Objects ptr: {:#x}", arr.objects_ptr);
                                                println!("  NumElements: {}", arr.num_elements);
                                                println!("  MaxElements: {}", arr.max_elements);
                                            }
                                            Err(e) => {
                                                eprintln!("GUObjectArray not found: {}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("GNames not found: {}", e);
                                }
                            }
                        }
                        "guobjectarray" => {
                            // First we need GNames
                            println!("Searching for GNames pool first...");
                            match memory::discover_gnames(source) {
                                Ok(gnames) => {
                                    println!("GNames at: {:#x}", gnames.address);
                                    println!("\nSearching for GUObjectArray...");
                                    match memory::discover_guobject_array(source, gnames.address) {
                                        Ok(arr) => {
                                            println!("GUObjectArray found at: {:#x}", arr.address);
                                            println!("  Objects ptr: {:#x}", arr.objects_ptr);
                                            println!("  NumElements: {}", arr.num_elements);
                                            println!("  MaxElements: {}", arr.max_elements);
                                        }
                                        Err(e) => {
                                            eprintln!("GUObjectArray not found: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "GNames not found (required for GUObjectArray): {}",
                                        e
                                    );
                                }
                            }
                        }
                        "classuclass" => {
                            // Find Class UClass via self-referential pattern
                            println!("Searching for Class UClass (self-referential)...");
                            match memory::discover_class_uclass(source) {
                                Ok(addr) => {
                                    println!("Class UClass found at: {:#x}", addr);

                                    // Read and dump the UObject structure
                                    println!("\nUObject structure dump:");
                                    for offset in (0..0x40usize).step_by(8) {
                                        if let Ok(val) = source.read_u64(addr + offset) {
                                            println!("  +{:#04x}: {:#018x}", offset, val);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Class UClass not found: {}", e);
                                }
                            }
                        }
                        _ => {
                            eprintln!("Unknown target: {}. Use 'gnames', 'guobjectarray', 'classuclass', or 'all'", target);
                        }
                    }
                }

                MemoryAction::Objects { class, limit } => {
                    let source = mem_source!();
                    // First discover GNames
                    let gnames =
                        memory::discover_gnames(source).context("Failed to find GNames pool")?;

                    println!("GNames at: {:#x}", gnames.address);

                    // For now, we can only search for class names in the FName pool
                    // Full object enumeration requires GUObjectArray
                    if let Some(class_name) = class {
                        println!("Searching for '{}' in FName pool...", class_name);

                        // Search for the class name in memory
                        let pattern = class_name.as_bytes();
                        let results =
                            memory::scan_pattern(source, pattern, &vec![1u8; pattern.len()])?;

                        println!(
                            "Found {} occurrences of '{}':",
                            results.len().min(limit),
                            class_name
                        );
                        for (i, addr) in results.iter().take(limit).enumerate() {
                            println!("  {}: {:#x}", i + 1, addr);

                            // Try to read context around the match
                            if let Ok(context) = source.read_bytes(addr.saturating_sub(16), 64) {
                                // Show as hex + ascii
                                print!("      ");
                                for byte in &context[..32.min(context.len())] {
                                    print!("{:02x} ", byte);
                                }
                                println!();
                                print!("      ");
                                for byte in &context[..32.min(context.len())] {
                                    let c = *byte as char;
                                    if c.is_ascii_graphic() || c == ' ' {
                                        print!("{}", c);
                                    } else {
                                        print!(".");
                                    }
                                }
                                println!();
                            }
                        }

                        if results.len() > limit {
                            println!("... and {} more", results.len() - limit);
                        }
                    } else {
                        println!("No class filter specified. Showing FName pool sample:");
                        for (idx, name) in &gnames.sample_names {
                            println!("  [{}] {}", idx, name);
                        }
                        println!("\nUse --class <name> to search for specific classes");
                        println!("Example: bl4 inject objects --class RarityWeightData");
                    }
                }

                MemoryAction::Fname { index, debug } => {
                    let source = mem_source!();

                    // Try to discover the full FNamePool structure using known address
                    match memory::FNamePool::discover(source) {
                        Ok(pool) => {
                            println!("FNamePool found at {:#x}", pool.header_addr);
                            println!("  Blocks: {}", pool.blocks.len());
                            println!("  Cursor: {}", pool.current_cursor);

                            let reader = memory::FNameReader::new(pool);

                            // Always dump raw bytes when --debug is specified
                            if debug {
                                reader.debug_read(source, index)?;
                            }

                            let mut reader = reader;
                            match reader.read_name(source, index) {
                                Ok(name) => {
                                    println!("\nFName[{}] = \"{}\"", index, name);

                                    // Show index breakdown
                                    let block = (index & 0x3FFFFFFF) >> 16;
                                    let offset = ((index & 0xFFFF) * 2) as usize;
                                    println!("  Block: {}, Offset: {:#x}", block, offset);
                                }
                                Err(e) => {
                                    eprintln!("Failed to read FName[{}]: {}", index, e);
                                    if !debug {
                                        reader.debug_read(source, index)?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // Fall back to pattern-based discovery
                            eprintln!("FNamePool::discover failed: {}", e);
                            let gnames = memory::discover_gnames(source)
                                .context("Failed to find GNames pool")?;
                            println!("Using legacy FName reader (block 0 only)");
                            let mut reader = memory::FNameReader::new_legacy(gnames.address);
                            match reader.read_name(source, index) {
                                Ok(name) => println!("FName[{}] = \"{}\"", index, name),
                                Err(e) => eprintln!("Failed to read FName[{}]: {}", index, e),
                            }
                        }
                    }
                }

                MemoryAction::FnameSearch { query } => {
                    let source = mem_source!();

                    // Discover FNamePool to get all blocks
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;

                    println!(
                        "Searching for \"{}\" across {} FName blocks...",
                        query,
                        pool.blocks.len()
                    );

                    let search_bytes = query.as_bytes();
                    let mut found = Vec::new();

                    // Search all blocks
                    for (block_idx, &block_addr) in pool.blocks.iter().enumerate() {
                        if block_addr == 0 {
                            continue;
                        }

                        // Read block data (64KB per block)
                        let block_data = match source.read_bytes(block_addr, 64 * 1024) {
                            Ok(d) => d,
                            Err(_) => continue,
                        };

                        for (pos, window) in block_data.windows(search_bytes.len()).enumerate() {
                            if window == search_bytes {
                                // Found match - try to find the entry start
                                if pos >= 2 {
                                    let header = &block_data[pos - 2..pos];
                                    let header_val = byteorder::LE::read_u16(header);
                                    let len = (header_val >> 6) as usize;

                                    // Verify this is a valid entry header
                                    if len > 0 && len <= 1024 {
                                        // Read the full name from header position
                                        let name_start = pos - 2 + 2;
                                        let name_end = name_start + len;
                                        if name_end <= block_data.len() {
                                            let full_name = String::from_utf8_lossy(
                                                &block_data[name_start..name_end],
                                            );
                                            let byte_offset = pos - 2;
                                            // FName index = (block_idx << 16) | (byte_offset / 2)
                                            let fname_index = ((block_idx as u32) << 16)
                                                | ((byte_offset / 2) as u32);
                                            found.push((
                                                fname_index,
                                                block_idx,
                                                byte_offset,
                                                full_name.to_string(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if found.is_empty() {
                        println!("No matches found for \"{}\"", query);
                    } else {
                        println!("Found {} matches:", found.len());
                        for (fname_index, block_idx, byte_offset, name) in found.iter().take(50) {
                            println!(
                                "  FName[{:#x}] = \"{}\" (block {}, offset {:#x})",
                                fname_index, name, block_idx, byte_offset
                            );
                        }
                        if found.len() > 50 {
                            println!("  ... and {} more", found.len() - 50);
                        }
                    }
                }

                MemoryAction::FindClassUClass => {
                    let source = mem_source!();

                    // First discover FNamePool to resolve names
                    let _gnames =
                        memory::discover_gnames(source).context("Failed to find GNames pool")?;
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;
                    let mut fname_reader = memory::FNameReader::new(pool);

                    // Get code bounds for vtable validation
                    let code_bounds = memory::find_code_bounds(source)?;

                    println!("Searching for Class UClass...");
                    println!("  Code bounds: {} ranges", code_bounds.ranges.len());

                    // Try multiple offset combinations for ClassPrivate and NamePrivate
                    // Standard UE5: ClassPrivate=0x10, NamePrivate=0x18
                    // BL4 discovered: ClassPrivate=0x18, NamePrivate=0x30
                    let offset_combos: &[(usize, usize, &str)] = &[
                        (0x18, 0x30, "BL4 (0x18/0x30)"),
                        (0x10, 0x18, "Standard UE5"),
                        (0x10, 0x30, "Mixed A"),
                        (0x20, 0x38, "Offset +8"),
                    ];

                    for &(class_off, name_off, desc) in offset_combos {
                        println!(
                            "\nTrying {} - ClassPrivate={:#x}, NamePrivate={:#x}...",
                            desc, class_off, name_off
                        );

                        let mut found_self_refs: Vec<(usize, usize, u32, String)> = Vec::new();
                        let mut found_class = false;
                        let header_size = name_off + 8;

                        for region in source.regions() {
                            // Only require readable (data sections may be read-only)
                            if !region.is_readable() {
                                continue;
                            }

                            // Include both PE image range AND heap
                            // PE: 0x140000000-0x175000000
                            // Heap: typically starts around 0x1000000+
                            let in_pe = region.start >= 0x140000000 && region.start <= 0x175000000;
                            let in_heap = region.start >= 0x1000000 && region.start < 0x140000000;
                            if !in_pe && !in_heap {
                                continue;
                            }

                            // Skip very large regions
                            if region.size() > 100 * 1024 * 1024 {
                                continue;
                            }

                            let data = match source.read_bytes(region.start, region.size()) {
                                Ok(d) => d,
                                Err(_) => continue,
                            };

                            // Scan for potential UObjects (8-byte aligned)
                            for offset in (0..data.len().saturating_sub(header_size)).step_by(8) {
                                let obj_addr = region.start + offset;

                                // Check vtable pointer - must be in valid data range
                                let vtable_ptr =
                                    byteorder::LE::read_u64(&data[offset..offset + 8]) as usize;
                                if !(0x140000000..=0x175000000).contains(&vtable_ptr) {
                                    continue;
                                }

                                // Vtable's first entry must point to CODE
                                let first_func = match source.read_bytes(vtable_ptr, 8) {
                                    Ok(vt) => byteorder::LE::read_u64(&vt) as usize,
                                    Err(_) => continue,
                                };
                                if !code_bounds.contains(first_func) {
                                    continue;
                                }

                                // Check ClassPrivate for self-reference
                                let class_ptr = byteorder::LE::read_u64(
                                    &data[offset + class_off..offset + class_off + 8],
                                ) as usize;
                                if class_ptr != obj_addr {
                                    continue;
                                }

                                // Self-referential! Read the name
                                let fname_idx = byteorder::LE::read_u32(
                                    &data[offset + name_off..offset + name_off + 4],
                                );
                                let name = fname_reader
                                    .read_name(source, fname_idx)
                                    .unwrap_or_else(|_| format!("<idx:{}>", fname_idx));

                                found_self_refs.push((
                                    obj_addr,
                                    vtable_ptr,
                                    fname_idx,
                                    name.clone(),
                                ));

                                if fname_idx == memory::FNAME_CLASS_INDEX || name == "Class" {
                                    println!("\n*** FOUND Class UClass at {:#x} ***", obj_addr);
                                    println!(
                                        "  VTable: {:#x}, vtable[0]: {:#x}",
                                        vtable_ptr, first_func
                                    );
                                    println!("  FName index: {} = \"{}\"", fname_idx, name);
                                    found_class = true;
                                }
                            }
                        }

                        println!(
                            "  Found {} self-referential objects:",
                            found_self_refs.len()
                        );
                        for (addr, vtable, fname_idx, name) in found_self_refs.iter().take(10) {
                            let marker = if *fname_idx == memory::FNAME_CLASS_INDEX {
                                " <-- CLASS!"
                            } else {
                                ""
                            };
                            println!(
                                "    {:#x}: vtable={:#x}, fname={} \"{}\"{}",
                                addr, vtable, fname_idx, name, marker
                            );
                        }

                        if found_class {
                            println!("\n=== SUCCESS with {} offsets! ===", desc);
                            break;
                        }
                    }
                }

                MemoryAction::ListUClasses { limit, filter } => {
                    let source = mem_source!();

                    // Discover FNamePool to resolve names
                    let _gnames =
                        memory::discover_gnames(source).context("Failed to find GNames pool")?;
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;
                    let mut fname_reader = memory::FNameReader::new(pool);

                    // Find all UClass instances
                    println!(
                        "Finding all UClass instances (ClassPrivate == {:#x})...\n",
                        memory::UCLASS_METACLASS_ADDR
                    );

                    let classes = memory::find_all_uclasses(source, &mut fname_reader)
                        .context("Failed to enumerate UClass instances")?;

                    // Apply filter if provided
                    let filtered: Vec<_> = if let Some(ref pattern) = filter {
                        let pattern_lower = pattern.to_lowercase();
                        classes
                            .iter()
                            .filter(|c| c.name.to_lowercase().contains(&pattern_lower))
                            .collect()
                    } else {
                        classes.iter().collect()
                    };

                    println!(
                        "Found {} UClass instances{}\n",
                        filtered.len(),
                        filter
                            .as_ref()
                            .map(|f| format!(" matching '{}'", f))
                            .unwrap_or_default()
                    );

                    // Show results
                    let show_count = if limit == 0 {
                        filtered.len()
                    } else {
                        limit.min(filtered.len())
                    };
                    for class in filtered.iter().take(show_count) {
                        println!(
                            "  {:#x}: {} (FName {})",
                            class.address, class.name, class.name_index
                        );
                    }

                    if show_count < filtered.len() {
                        println!(
                            "\n  ... and {} more (use --limit 0 to show all)",
                            filtered.len() - show_count
                        );
                    }

                    // Show some stats
                    let game_classes: Vec<_> = filtered
                        .iter()
                        .filter(|c| {
                            c.name.starts_with("U")
                                || c.name.starts_with("A")
                                || c.name.contains("_")
                        })
                        .collect();
                    let core_classes: Vec<_> = filtered
                        .iter()
                        .filter(|c| {
                            !c.name.starts_with("U")
                                && !c.name.starts_with("A")
                                && !c.name.contains("_")
                        })
                        .collect();

                    println!("\nClass categories:");
                    println!("  Game classes (U*/A*/*_*): {}", game_classes.len());
                    println!("  Core/Native classes: {}", core_classes.len());
                }

                MemoryAction::ListObjects {
                    limit,
                    class_filter,
                    name_filter,
                    stats,
                } => {
                    let source = mem_source!();

                    // Discover GNames first (needed for FName resolution)
                    eprintln!("Searching for GNames pool...");
                    let gnames =
                        memory::discover_gnames(source).context("Failed to discover GNames")?;
                    eprintln!("GNames found at: {:#x}\n", gnames.address);

                    // Discover GUObjectArray via pattern-based search
                    eprintln!("Searching for GUObjectArray...");
                    let guobj = memory::discover_guobject_array(source, gnames.address)
                        .context("Failed to discover GUObjectArray")?;

                    // Discover FNamePool for name reading
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;
                    let mut fname_reader = memory::FNameReader::new(pool);

                    println!("Enumerating UObjects from GUObjectArray...");
                    println!("  Total objects: {}", guobj.num_elements);
                    println!("  Item size: {} bytes\n", guobj.item_size);

                    // Statistics tracking
                    let mut total_valid = 0usize;
                    let mut class_counts: std::collections::HashMap<String, usize> =
                        std::collections::HashMap::new();
                    let mut shown = 0usize;

                    let class_filter_lower = class_filter.as_ref().map(|s| s.to_lowercase());
                    let name_filter_lower = name_filter.as_ref().map(|s| s.to_lowercase());

                    // Iterate over all objects
                    for (idx, obj_ptr) in guobj.iter_objects(source) {
                        // Read UObject header
                        let obj_data = match source.read_bytes(obj_ptr, memory::UOBJECT_HEADER_SIZE)
                        {
                            Ok(d) => d,
                            Err(_) => continue,
                        };

                        let class_ptr = byteorder::LE::read_u64(
                            &obj_data
                                [memory::UOBJECT_CLASS_OFFSET..memory::UOBJECT_CLASS_OFFSET + 8],
                        ) as usize;
                        let name_idx = byteorder::LE::read_u32(
                            &obj_data[memory::UOBJECT_NAME_OFFSET..memory::UOBJECT_NAME_OFFSET + 4],
                        );

                        // Read object name
                        let obj_name = fname_reader
                            .read_name(source, name_idx)
                            .unwrap_or_else(|_| format!("FName_{}", name_idx));

                        // Read class name (need to read the class object's name)
                        let class_name = if class_ptr != 0 {
                            if let Ok(class_data) =
                                source.read_bytes(class_ptr, memory::UOBJECT_HEADER_SIZE)
                            {
                                let class_name_idx = byteorder::LE::read_u32(
                                    &class_data[memory::UOBJECT_NAME_OFFSET
                                        ..memory::UOBJECT_NAME_OFFSET + 4],
                                );
                                fname_reader
                                    .read_name(source, class_name_idx)
                                    .unwrap_or_else(|_| format!("FName_{}", class_name_idx))
                            } else {
                                "Unknown".to_string()
                            }
                        } else {
                            "Null".to_string()
                        };

                        total_valid += 1;
                        *class_counts.entry(class_name.clone()).or_insert(0) += 1;

                        // Apply filters
                        let class_match = class_filter_lower
                            .as_ref()
                            .map(|f| class_name.to_lowercase().contains(f))
                            .unwrap_or(true);
                        let name_match = name_filter_lower
                            .as_ref()
                            .map(|f| obj_name.to_lowercase().contains(f))
                            .unwrap_or(true);

                        if class_match && name_match && !stats && shown < limit {
                            println!("[{}] {:#x}: {} ({})", idx, obj_ptr, obj_name, class_name);
                            shown += 1;
                        }

                        // Progress indicator
                        if total_valid.is_multiple_of(50000) {
                            eprint!("\r  Scanned {} objects...", total_valid);
                        }
                    }

                    eprintln!("\r  Scanned {} valid objects total.", total_valid);

                    if stats || class_filter.is_some() || name_filter.is_some() {
                        println!("\nStatistics:");
                        println!("  Total valid objects: {}", total_valid);
                        println!("  Unique classes: {}", class_counts.len());

                        // Sort classes by count and show top 20
                        let mut sorted_classes: Vec<_> = class_counts.into_iter().collect();
                        sorted_classes.sort_by(|a, b| b.1.cmp(&a.1));

                        println!("\nTop 20 classes by instance count:");
                        for (class_name, count) in sorted_classes.iter().take(20) {
                            println!("  {:6} {}", count, class_name);
                        }
                    }

                    if !stats && shown >= limit && limit > 0 {
                        println!(
                            "\n... showing first {} matches (use --limit N to see more)",
                            limit
                        );
                    }
                }

                MemoryAction::AnalyzeDump => {
                    let source = mem_source!();

                    // Run comprehensive dump analysis
                    memory::analyze_dump(source).context("Dump analysis failed")?;
                }

                MemoryAction::DumpUsmap { output } => {
                    let source = mem_source!();
                    // Step 1: Find GNames pool
                    println!("Step 1: Finding GNames pool...");
                    let gnames =
                        memory::discover_gnames(source).context("Failed to find GNames pool")?;
                    println!("  GNames at: {:#x}", gnames.address);

                    // Step 2: Find GUObjectArray
                    println!("\nStep 2: Finding GUObjectArray...");
                    let guobj_array = memory::discover_guobject_array(source, gnames.address)
                        .context("Failed to find GUObjectArray")?;
                    println!("  GUObjectArray at: {:#x}", guobj_array.address);
                    println!("  Objects ptr: {:#x}", guobj_array.objects_ptr);
                    println!("  NumElements: {}", guobj_array.num_elements);

                    // Step 3: Walk GUObjectArray to find reflection objects
                    println!("\nStep 3: Walking GUObjectArray to find reflection objects...");
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;
                    let mut fname_reader = memory::FNameReader::new(pool);
                    let reflection_objects =
                        memory::walk_guobject_array(source, &guobj_array, &mut fname_reader)
                            .context("Failed to walk GUObjectArray")?;

                    // Print summary
                    let class_count = reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "Class")
                        .count();
                    let struct_count = reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "ScriptStruct")
                        .count();
                    let enum_count = reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "Enum")
                        .count();

                    println!("\nFound {} reflection objects:", reflection_objects.len());
                    println!("  {} UClass", class_count);
                    println!("  {} UScriptStruct", struct_count);
                    println!("  {} UEnum", enum_count);

                    // Print some samples
                    println!("\nSample classes:");
                    for obj in reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "Class")
                        .take(10)
                    {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    println!("\nSample structs:");
                    for obj in reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "ScriptStruct")
                        .take(10)
                    {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    println!("\nSample enums:");
                    for obj in reflection_objects
                        .iter()
                        .filter(|o| o.class_name == "Enum")
                        .take(10)
                    {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    // Step 4: Extract properties from each struct/class
                    println!("\nStep 4: Extracting properties...");
                    let (structs, enums) = memory::extract_reflection_data(
                        source,
                        &reflection_objects,
                        &mut fname_reader,
                    )
                    .context("Failed to extract reflection data")?;

                    // Print some sample properties
                    println!("\nSample struct properties:");
                    for s in structs.iter().filter(|s| !s.properties.is_empty()).take(5) {
                        println!(
                            "  {} ({}): {} props, super={:?}",
                            s.name,
                            if s.is_class { "class" } else { "struct" },
                            s.properties.len(),
                            s.super_name
                        );
                        for prop in s.properties.iter().take(3) {
                            println!(
                                "    +{:#x} {} : {} ({:?})",
                                prop.offset,
                                prop.name,
                                prop.type_name,
                                prop.struct_type.as_ref().or(prop.enum_type.as_ref())
                            );
                        }
                        if s.properties.len() > 3 {
                            println!("    ... and {} more", s.properties.len() - 3);
                        }
                    }

                    println!("\nSample enum values:");
                    for e in enums.iter().filter(|e| !e.values.is_empty()).take(5) {
                        println!("  {}: {} values", e.name, e.values.len());
                        for (name, val) in e.values.iter().take(3) {
                            println!("    {} = {}", name, val);
                        }
                        if e.values.len() > 3 {
                            println!("    ... and {} more", e.values.len() - 3);
                        }
                    }

                    // Step 5: Write usmap format
                    memory::write_usmap(&output, &structs, &enums)?;
                    println!("\nWrote usmap file: {}", output.display());
                }

                MemoryAction::ListInventory => {
                    // TODO: Find player controller, walk inventory array
                    bail!(
                        "Inventory listing not yet implemented. \
                        Need to locate player inventory structures first."
                    );
                }

                MemoryAction::Read { address, size } => {
                    let source = mem_source!();
                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16).context("Invalid hex address")?
                    } else {
                        address.parse::<usize>().context("Invalid address")?
                    };

                    let data = source.read_bytes(addr, size)?;

                    // Print hex dump
                    println!("Reading {} bytes at {:#x}:", size, addr);
                    for (i, chunk) in data.chunks(16).enumerate() {
                        print!("{:08x}  ", addr + i * 16);
                        for (j, byte) in chunk.iter().enumerate() {
                            print!("{:02x} ", byte);
                            if j == 7 {
                                print!(" ");
                            }
                        }
                        // Pad if last line is short
                        if chunk.len() < 16 {
                            for j in chunk.len()..16 {
                                print!("   ");
                                if j == 7 {
                                    print!(" ");
                                }
                            }
                        }
                        print!(" |");
                        for byte in chunk {
                            let c = *byte as char;
                            if c.is_ascii_graphic() || c == ' ' {
                                print!("{}", c);
                            } else {
                                print!(".");
                            }
                        }
                        println!("|");
                    }
                }

                MemoryAction::Write { address, bytes } => {
                    // Writing requires a live process
                    let proc = process
                        .as_ref()
                        .context("Write requires a live process (not available in dump mode)")?;

                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16).context("Invalid hex address")?
                    } else {
                        address.parse::<usize>().context("Invalid address")?
                    };

                    // Parse hex bytes
                    let parts: Vec<&str> = bytes.split_whitespace().collect();
                    let mut data = Vec::new();
                    for part in parts {
                        let byte = u8::from_str_radix(part, 16)
                            .with_context(|| format!("Invalid hex byte: {}", part))?;
                        data.push(byte);
                    }

                    // Show what we're about to write
                    println!("Writing {} bytes to {:#x}:", data.len(), addr);
                    print!("  ");
                    for byte in &data {
                        print!("{:02x} ", byte);
                    }
                    println!();

                    // Read original bytes first for safety
                    let original = proc.read_bytes(addr, data.len())?;
                    print!("Original: ");
                    for byte in &original {
                        print!("{:02x} ", byte);
                    }
                    println!();

                    // Write the new bytes
                    proc.write_bytes(addr, &data)?;
                    println!("Write successful!");
                }

                MemoryAction::Scan { pattern } => {
                    let source = mem_source!();
                    // Parse pattern like "48 8B 05 ?? ?? ?? ??"
                    let parts: Vec<&str> = pattern.split_whitespace().collect();
                    let mut bytes = Vec::new();
                    let mut mask = Vec::new();

                    for part in parts {
                        if part == "??" || part == "?" {
                            bytes.push(0u8);
                            mask.push(0u8); // 0 = wildcard
                        } else {
                            let byte = u8::from_str_radix(part, 16)
                                .with_context(|| format!("Invalid hex byte: {}", part))?;
                            bytes.push(byte);
                            mask.push(1u8); // 1 = must match
                        }
                    }

                    println!("Scanning for pattern: {}", pattern);
                    println!("This may take a while...");

                    let results = memory::scan_pattern(source, &bytes, &mask)?;

                    if results.is_empty() {
                        println!("No matches found.");
                    } else {
                        println!("Found {} matches:", results.len());
                        for (i, addr) in results.iter().take(20).enumerate() {
                            println!("  {}: {:#x}", i + 1, addr);
                        }
                        if results.len() > 20 {
                            println!("  ... and {} more", results.len() - 20);
                        }
                    }
                }

                MemoryAction::Patch {
                    address,
                    nop,
                    bytes,
                } => {
                    // Patching requires a live process
                    let proc = process
                        .as_ref()
                        .context("Patch requires a live process (not available in dump mode)")?;

                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16).context("Invalid hex address")?
                    } else {
                        address.parse::<usize>().context("Invalid address")?
                    };

                    let patch_bytes = if let Some(nop_count) = nop {
                        // Generate NOP bytes (0x90 on x86-64)
                        vec![0x90u8; nop_count]
                    } else if let Some(hex_bytes) = bytes {
                        // Parse custom bytes
                        let parts: Vec<&str> = hex_bytes.split_whitespace().collect();
                        let mut data = Vec::new();
                        for part in parts {
                            let byte = u8::from_str_radix(part, 16)
                                .with_context(|| format!("Invalid hex byte: {}", part))?;
                            data.push(byte);
                        }
                        data
                    } else {
                        bail!("Must specify either --nop <count> or --bytes <hex>");
                    };

                    // Read original bytes first
                    let original = proc.read_bytes(addr, patch_bytes.len())?;
                    println!("Patching {} bytes at {:#x}", patch_bytes.len(), addr);
                    print!("Original: ");
                    for byte in &original {
                        print!("{:02x} ", byte);
                    }
                    println!();
                    print!("New:      ");
                    for byte in &patch_bytes {
                        print!("{:02x} ", byte);
                    }
                    println!();

                    // Apply the patch
                    proc.write_bytes(addr, &patch_bytes)?;
                    println!("Patch applied!");
                }

                MemoryAction::Apply { templates } => {
                    println!("Applying {} template(s)...", templates.len());
                    println!();

                    for template in &templates {
                        // Parse template format: "key=value"
                        let parts: Vec<&str> = template.splitn(2, '=').collect();
                        if parts.len() != 2 {
                            eprintln!("Invalid template format: {} (expected key=value)", template);
                            continue;
                        }

                        let key = parts[0].to_lowercase();
                        let value = parts[1].to_lowercase();

                        match key.as_str() {
                            "droprate" => {
                                println!("Template: dropRate={}", value);
                                println!("  Status: Not yet implemented");
                                println!("  Requires: Finding RarityWeightData instances");
                                println!("  Known addresses:");
                                println!("    - RarityWeightData FName: 0x5f9548e");
                                println!("    - BaseWeight FName: 0x6f3a44c4");
                                println!("    - GrowthExponent FName: 0x6f3a44b4");
                            }
                            "droprarity" => {
                                println!("Template: dropRarity={}", value);
                                match value.as_str() {
                                    "legendary" => {
                                        println!("  Target: Force comp_05_legendary");
                                        println!("  Status: Not yet implemented");
                                        println!("  Requires: Patching ItemPool selection code");
                                    }
                                    "epic" => {
                                        println!("  Target: Force comp_04_epic");
                                        println!("  Status: Not yet implemented");
                                    }
                                    "rare" => {
                                        println!("  Target: Force comp_03_rare");
                                        println!("  Status: Not yet implemented");
                                    }
                                    _ => {
                                        eprintln!("  Unknown rarity: {}", value);
                                        eprintln!(
                                            "  Valid: legendary, epic, rare, uncommon, common"
                                        );
                                    }
                                }
                            }
                            "luck" => {
                                println!("Template: luck={}", value);
                                println!("  Status: Not yet implemented");
                                println!("  Requires: Finding LuckGlobals instance");
                                println!("  Known addresses:");
                                println!("    - LuckGlobals FName: 0x5f95658");
                                println!("    - LuckCategories FName: 0x6f3a4560");
                            }
                            _ => {
                                eprintln!("Unknown template: {}", key);
                                eprintln!("Run 'bl4 inject templates' to see available templates");
                            }
                        }
                        println!();
                    }

                    println!("Note: Template implementation is work-in-progress.");
                    println!("See docs/loot.md for current research findings.");
                }

                MemoryAction::Monitor {
                    log_file,
                    filter,
                    game_only,
                } => {
                    use std::io::BufRead;

                    println!("Monitoring: {}", log_file.display());
                    if let Some(ref f) = filter {
                        println!("Filter: {}", f);
                    }
                    if game_only {
                        println!("Showing only game code addresses (0x140000000+)");
                    }
                    println!("Press Ctrl+C to stop\n");

                    // Tail the log file
                    let file = std::fs::File::open(&log_file)
                        .with_context(|| format!("Failed to open {}", log_file.display()))?;
                    let mut reader = std::io::BufReader::new(file);

                    // Seek to end first
                    reader.seek_relative(
                        std::fs::metadata(&log_file)
                            .map(|m| m.len() as i64)
                            .unwrap_or(0),
                    )?;

                    loop {
                        let mut line = String::new();
                        match reader.read_line(&mut line) {
                            Ok(0) => {
                                // No new data, wait a bit
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            Ok(_) => {
                                let line = line.trim();

                                // Apply filter
                                if let Some(ref f) = filter {
                                    if !line.contains(f) {
                                        continue;
                                    }
                                }

                                // Apply game_only filter (addresses 0x140000000+)
                                if game_only {
                                    if let Some(caller_pos) = line.find("caller=0x") {
                                        let addr_str = &line[caller_pos + 9..];
                                        if let Some(end) =
                                            addr_str.find(|c: char| !c.is_ascii_hexdigit())
                                        {
                                            if let Ok(addr) =
                                                usize::from_str_radix(&addr_str[..end], 16)
                                            {
                                                // Skip addresses below game base
                                                if addr < 0x140000000 {
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }

                                println!("{}", line);
                            }
                            Err(e) => {
                                eprintln!("Read error: {}", e);
                                break;
                            }
                        }
                    }
                }

                MemoryAction::ScanString {
                    query,
                    before,
                    after,
                    limit,
                } => {
                    let source = mem_source!();

                    println!("Searching for \"{}\" in memory...", query);
                    let search_bytes = query.as_bytes();
                    let mask = vec![1u8; search_bytes.len()];

                    // Use scan_pattern to find all matches
                    let results = memory::scan_pattern(source, search_bytes, &mask)?;

                    if results.is_empty() {
                        println!("No matches found.");
                    } else {
                        let show_count = results.len().min(limit);
                        println!("Found {} matches, showing {}:", results.len(), show_count);

                        for (i, &addr) in results.iter().take(limit).enumerate() {
                            println!("\n=== Match {} at {:#x} ===", i + 1, addr);

                            // Read context around the match
                            let ctx_start = addr.saturating_sub(before);
                            let ctx_size = before + search_bytes.len() + after;

                            if let Ok(data) = source.read_bytes(ctx_start, ctx_size) {
                                // Print hex dump with context
                                for j in (0..data.len()).step_by(16) {
                                    let line_addr = ctx_start + j;
                                    let line_end = (j + 16).min(data.len());
                                    let line_bytes = &data[j..line_end];

                                    // Hex bytes
                                    let hex: String = line_bytes
                                        .iter()
                                        .map(|b| format!("{:02x}", b))
                                        .collect::<Vec<_>>()
                                        .join(" ");

                                    // ASCII representation
                                    let ascii: String = line_bytes
                                        .iter()
                                        .map(|&b| {
                                            if (32..127).contains(&b) {
                                                b as char
                                            } else {
                                                '.'
                                            }
                                        })
                                        .collect();

                                    // Mark if this line contains the match
                                    let marker =
                                        if ctx_start + j <= addr && addr < ctx_start + j + 16 {
                                            " <--"
                                        } else {
                                            ""
                                        };
                                    println!(
                                        "{:#010x}: {:<48} {}{}",
                                        line_addr, hex, ascii, marker
                                    );
                                }
                            }
                        }

                        if results.len() > limit {
                            println!("\n... and {} more matches", results.len() - limit);
                        }
                    }
                }

                MemoryAction::DumpParts { output } => {
                    let source = mem_source!();

                    println!("Extracting part definitions from memory dump...");

                    // Pattern: .part_ - we'll search for this and extract surrounding context
                    let pattern = b".part_";
                    let mask = vec![1u8; pattern.len()];

                    let results = memory::scan_pattern(source, pattern, &mask)?;
                    println!(
                        "Found {} occurrences of '.part_', analyzing...",
                        results.len()
                    );

                    let mut parts: std::collections::BTreeMap<String, Vec<String>> =
                        std::collections::BTreeMap::new();

                    for &addr in &results {
                        // Read 64 bytes before and 64 after the match
                        let ctx_start = addr.saturating_sub(32);
                        if let Ok(data) = source.read_bytes(ctx_start, 128) {
                            // Find the .part_ position in our buffer
                            let rel_offset = addr - ctx_start;

                            // Look backwards from .part_ for the prefix (XXX_YY)
                            let mut start = rel_offset;
                            while start > 0 {
                                let c = data[start - 1];
                                if c.is_ascii_alphanumeric() || c == b'_' {
                                    start -= 1;
                                } else {
                                    break;
                                }
                            }

                            // Look forward for the rest of the part name
                            let mut end = rel_offset + pattern.len();
                            while end < data.len() {
                                let c = data[end];
                                if c.is_ascii_alphanumeric() || c == b'_' {
                                    end += 1;
                                } else {
                                    break;
                                }
                            }

                            // Extract the full part name
                            if let Ok(name) = std::str::from_utf8(&data[start..end]) {
                                // Validate format: XXX_YY.part_*
                                if name.contains('.') && name.len() > 10 {
                                    let prefix = name.split('.').next().unwrap_or("");
                                    if prefix.len() >= 5 && prefix.contains('_') {
                                        parts
                                            .entry(prefix.to_string())
                                            .or_default()
                                            .push(name.to_string());
                                    }
                                }
                            }
                        }
                    }

                    // Deduplicate and sort
                    for names in parts.values_mut() {
                        names.sort();
                        names.dedup();
                    }

                    // Write JSON using manual formatting (no serde_json dependency needed)
                    let mut json = String::from("{\n");
                    let mut first_type = true;
                    for (prefix, names) in &parts {
                        if !first_type {
                            json.push_str(",\n");
                        }
                        first_type = false;
                        json.push_str(&format!("  \"{}\": [\n", prefix));
                        for (i, name) in names.iter().enumerate() {
                            json.push_str(&format!("    \"{}\"", name));
                            if i < names.len() - 1 {
                                json.push(',');
                            }
                            json.push('\n');
                        }
                        json.push_str("  ]");
                    }
                    json.push_str("\n}\n");

                    std::fs::write(&output, &json)?;

                    let total_unique: usize = parts.values().map(|v| v.len()).sum();
                    println!(
                        "Found {} unique part names across {} weapon types",
                        total_unique,
                        parts.len()
                    );
                    println!("Written to: {}", output.display());
                }
            }
        }

        Commands::Launch { yes } => {
            // Find the preload library
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()));

            let lib_path = exe_dir
                .as_ref()
                .map(|d| d.join("libbl4_preload.so"))
                .filter(|p| p.exists())
                .or_else(|| {
                    // Try relative to current dir
                    let p = PathBuf::from("target/release/libbl4_preload.so");
                    if p.exists() {
                        Some(std::fs::canonicalize(p).unwrap_or_default())
                    } else {
                        None
                    }
                });

            let lib_path = match lib_path {
                Some(p) => p,
                None => {
                    bail!(
                        "Preload library not found. Build it first:\n  \
                        cargo build --release -p bl4-preload"
                    );
                }
            };

            // Build the launch options string
            let launch_options = format!("LD_PRELOAD={} %command%", lib_path.display());

            println!("Add to Steam launch options:\n");
            println!("  {}\n", launch_options);
            println!(
                "Options: BL4_RNG_BIAS=max|high|low|min  BL4_PRELOAD_ALL=1  BL4_PRELOAD_STACKS=1"
            );
            println!("Log: /tmp/bl4_preload.log\n");

            // Prompt for confirmation
            if !yes {
                print!("Launch game? [y/N] ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    return Ok(());
                }
            }

            Command::new("steam")
                .arg("steam://rungameid/1285190")
                .status()
                .context("Failed to launch Steam")?;
        }

        #[cfg(feature = "research")]
        Commands::Usmap {
            command: UsmapCommand::Info { path },
        } => {
            use byteorder::{LittleEndian as LE, ReadBytesExt};
            use std::io::{BufReader, Seek, SeekFrom};

            let file = fs::File::open(&path)
                .with_context(|| format!("Failed to open {}", path.display()))?;
            let mut reader = BufReader::new(file);

            // Read header
            let magic = reader.read_u16::<LE>()?;
            if magic != 0x30C4 {
                bail!("Invalid usmap magic: expected 0x30C4, got {:#x}", magic);
            }

            let version = reader.read_u8()?;
            let has_version_info = if version >= 1 {
                reader.read_u8()? != 0
            } else {
                false
            };

            let compression = reader.read_u32::<LE>()?;
            let compressed_size = reader.read_u32::<LE>()?;
            let decompressed_size = reader.read_u32::<LE>()?;

            println!("=== {} ===", path.display());
            println!("Magic: {:#x}", magic);
            println!("Version: {}", version);
            println!("HasVersionInfo: {}", has_version_info);
            println!(
                "Compression: {} ({})",
                compression,
                match compression {
                    0 => "None",
                    1 => "Oodle",
                    2 => "Brotli",
                    3 => "ZStandard",
                    _ => "Unknown",
                }
            );
            println!("CompressedSize: {} bytes", compressed_size);
            println!("DecompressedSize: {} bytes", decompressed_size);

            if compression != 0 {
                println!("\n(Compressed payloads not yet supported for detailed analysis)");
            } else {
                // Read payload
                let name_count = reader.read_u32::<LE>()?;
                println!("\nNames: {}", name_count);

                // Skip names
                for _ in 0..name_count {
                    let len = reader.read_u16::<LE>()? as usize;
                    reader.seek(SeekFrom::Current(len as i64))?;
                }

                let enum_count = reader.read_u32::<LE>()?;
                println!("Enums: {}", enum_count);

                // Count enum values
                let mut total_enum_values = 0u64;
                for _ in 0..enum_count {
                    let _name_idx = reader.read_u32::<LE>()?;
                    let entry_count = reader.read_u16::<LE>()? as u64;
                    total_enum_values += entry_count;
                    // Version >= 4 uses ExplicitEnumValues (value u64 + name_idx u32 = 12 bytes)
                    // Version 3 uses just name indices (4 bytes each)
                    let bytes_per_entry = if version >= 4 { 12 } else { 4 };
                    reader.seek(SeekFrom::Current((entry_count * bytes_per_entry) as i64))?;
                }
                println!("Enum values: {}", total_enum_values);

                let struct_count = reader.read_u32::<LE>()?;
                println!("Structs: {}", struct_count);

                // Count properties
                let mut total_props = 0u64;
                for _ in 0..struct_count {
                    let _name_idx = reader.read_u32::<LE>()?;
                    let _super_idx = reader.read_u32::<LE>()?;
                    let _prop_count = reader.read_u16::<LE>()?;
                    let serializable_count = reader.read_u16::<LE>()? as u64;
                    total_props += serializable_count;

                    // Skip properties (need to parse each one due to variable size)
                    for _ in 0..serializable_count {
                        let _index = reader.read_u16::<LE>()?;
                        let _array_dim = reader.read_u8()?;
                        let _name_idx = reader.read_u32::<LE>()?;
                        // Read property type recursively
                        fn skip_property_type<R: std::io::Read>(r: &mut R) -> Result<()> {
                            let type_id = r.read_u8()?;
                            match type_id {
                                26 => {
                                    // EnumProperty
                                    skip_property_type(r)?; // inner
                                    r.read_u32::<LE>()?; // enum name
                                }
                                9 => {
                                    // StructProperty
                                    r.read_u32::<LE>()?; // struct name
                                }
                                8 | 25 | 28 => {
                                    // Array/Set/Optional
                                    skip_property_type(r)?; // inner
                                }
                                24 => {
                                    // MapProperty
                                    skip_property_type(r)?; // key
                                    skip_property_type(r)?; // value
                                }
                                _ => {} // Simple types have no extra data
                            }
                            Ok(())
                        }
                        skip_property_type(&mut reader)?;
                    }
                }
                println!("Properties: {}", total_props);
            }

            let file_size = fs::metadata(&path)?.len();
            println!("\nFile size: {} bytes", file_size);
        }

        #[cfg(feature = "research")]
        Commands::Usmap {
            command:
                UsmapCommand::Search {
                    path,
                    pattern,
                    verbose,
                },
        } => {
            use byteorder::{LittleEndian as LE, ReadBytesExt};
            use std::io::{BufReader, Read, Seek, SeekFrom};

            let file = fs::File::open(&path)
                .with_context(|| format!("Failed to open {}", path.display()))?;
            let mut reader = BufReader::new(file);

            // Read header
            let magic = reader.read_u16::<LE>()?;
            if magic != 0x30C4 {
                bail!("Invalid usmap magic: expected 0x30C4, got {:#x}", magic);
            }

            let version = reader.read_u8()?;
            let _has_version_info = if version >= 1 {
                reader.read_u8()? != 0
            } else {
                false
            };

            let compression = reader.read_u32::<LE>()?;
            let _compressed_size = reader.read_u32::<LE>()?;
            let _decompressed_size = reader.read_u32::<LE>()?;

            if compression != 0 {
                bail!("Compressed usmap files not yet supported for search");
            }

            // Read names table
            let name_count = reader.read_u32::<LE>()?;
            let mut names: Vec<String> = Vec::with_capacity(name_count as usize);
            for _ in 0..name_count {
                let len = reader.read_u16::<LE>()? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                names.push(String::from_utf8_lossy(&buf).into_owned());
            }

            // Read enums
            let enum_count = reader.read_u32::<LE>()?;
            let pattern_lower = pattern.to_lowercase();
            let mut found_enums = Vec::new();

            for _ in 0..enum_count {
                let name_idx = reader.read_u32::<LE>()? as usize;
                let entry_count = reader.read_u16::<LE>()? as usize;

                let name = names.get(name_idx).cloned().unwrap_or_default();
                if name.to_lowercase().contains(&pattern_lower) {
                    let mut entries = Vec::new();
                    for _ in 0..entry_count {
                        let entry_idx = reader.read_u32::<LE>()? as usize;
                        entries.push(names.get(entry_idx).cloned().unwrap_or_default());
                    }
                    found_enums.push((name, entries));
                } else {
                    // Skip entries
                    reader.seek(SeekFrom::Current((entry_count * 4) as i64))?;
                }
            }

            // Read structs
            let struct_count = reader.read_u32::<LE>()?;
            let mut found_structs = Vec::new();

            // Property type names for display
            let type_names = [
                "Byte",
                "Bool",
                "Int",
                "Float",
                "Object",
                "Name",
                "Delegate",
                "Double",
                "Array",
                "Struct",
                "Str",
                "Text",
                "Interface",
                "MulticastDelegate",
                "WeakObject",
                "LazyObject",
                "AssetObject",
                "SoftObject",
                "UInt64",
                "UInt32",
                "UInt16",
                "Int64",
                "Int16",
                "Int8",
                "Map",
                "Set",
                "Enum",
                "FieldPath",
                "Optional",
                "Utf8Str",
                "AnsiStr",
            ];

            fn read_property_type<R: std::io::Read>(
                r: &mut R,
                names: &[String],
                type_names: &[&str],
            ) -> Result<String> {
                let type_id = r.read_u8()? as usize;
                let base_type = type_names.get(type_id).unwrap_or(&"Unknown");

                Ok(match type_id {
                    26 => {
                        // EnumProperty
                        let _inner = read_property_type(r, names, type_names)?;
                        let enum_idx = r.read_u32::<LE>()? as usize;
                        let enum_name = names.get(enum_idx).cloned().unwrap_or_default();
                        format!("Enum<{}>", enum_name)
                    }
                    9 => {
                        // StructProperty
                        let struct_idx = r.read_u32::<LE>()? as usize;
                        let struct_name = names.get(struct_idx).cloned().unwrap_or_default();
                        format!("Struct<{}>", struct_name)
                    }
                    8 => {
                        // ArrayProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Array<{}>", inner)
                    }
                    25 => {
                        // SetProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Set<{}>", inner)
                    }
                    28 => {
                        // OptionalProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Optional<{}>", inner)
                    }
                    24 => {
                        // MapProperty
                        let key = read_property_type(r, names, type_names)?;
                        let value = read_property_type(r, names, type_names)?;
                        format!("Map<{}, {}>", key, value)
                    }
                    _ => base_type.to_string(),
                })
            }

            for _ in 0..struct_count {
                let name_idx = reader.read_u32::<LE>()? as usize;
                let super_idx = reader.read_u32::<LE>()? as usize;
                let _prop_count = reader.read_u16::<LE>()?;
                let serializable_count = reader.read_u16::<LE>()? as usize;

                let name = names.get(name_idx).cloned().unwrap_or_default();
                let super_name = if super_idx == 0xFFFFFFFF {
                    None
                } else {
                    names.get(super_idx).cloned()
                };

                // Read properties
                let mut properties = Vec::new();
                for _ in 0..serializable_count {
                    let _index = reader.read_u16::<LE>()?;
                    let array_dim = reader.read_u8()?;
                    let prop_name_idx = reader.read_u32::<LE>()? as usize;
                    let prop_name = names.get(prop_name_idx).cloned().unwrap_or_default();
                    let prop_type = read_property_type(&mut reader, &names, &type_names)?;

                    properties.push((prop_name, prop_type, array_dim));
                }

                if name.to_lowercase().contains(&pattern_lower) {
                    found_structs.push((name, super_name, properties));
                }
            }

            // Print results
            if !found_enums.is_empty() {
                println!(
                    "=== Enums matching '{}' ({}) ===",
                    pattern,
                    found_enums.len()
                );
                for (name, entries) in &found_enums {
                    println!("\n{} ({} values)", name, entries.len());
                    if verbose {
                        for (i, entry) in entries.iter().enumerate() {
                            println!("  {} = {}", i, entry);
                        }
                    }
                }
            }

            if !found_structs.is_empty() {
                println!(
                    "\n=== Structs matching '{}' ({}) ===",
                    pattern,
                    found_structs.len()
                );
                for (name, super_name, properties) in &found_structs {
                    println!(
                        "\n{}{} ({} properties)",
                        name,
                        super_name
                            .as_ref()
                            .map(|s| format!(" : {}", s))
                            .unwrap_or_default(),
                        properties.len()
                    );
                    if verbose {
                        for (prop_name, prop_type, array_dim) in properties {
                            let dim_str = if *array_dim > 1 {
                                format!("[{}]", array_dim)
                            } else {
                                String::new()
                            };
                            println!("  {} {}{}", prop_type, prop_name, dim_str);
                        }
                    }
                }
            }

            if found_enums.is_empty() && found_structs.is_empty() {
                println!("No enums or structs found matching '{}'", pattern);
            }
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::PartPools { input, output },
        } => {
            use std::collections::BTreeMap;

            // Read the parts database (memory-extracted names + verified category assignments)
            let data = fs::read_to_string(&input)
                .with_context(|| format!("Failed to read {}", input.display()))?;

            // Parse parts array from JSON
            // Structure: { "parts": [ { "category": N, "name": "...", ... }, ... ], "categories": {...} }
            let parts_start = data.find("\"parts\"").context("Missing 'parts' key")?;
            let array_start = data[parts_start..]
                .find('[')
                .context("Missing parts array")?
                + parts_start;

            // Find the matching closing bracket
            let mut depth = 0;
            let mut array_end = array_start;
            for (i, c) in data[array_start..].char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            array_end = array_start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let parts_json = &data[array_start..=array_end];

            // Parse part entries - only need category and name
            struct PartEntry {
                category: i64,
                name: String,
            }

            let mut parts: Vec<PartEntry> = Vec::new();
            let mut in_object = false;
            let mut current_category: i64 = -1;
            let mut current_name = String::new();
            let mut depth = 0;

            for (i, c) in parts_json.char_indices() {
                match c {
                    '{' => {
                        depth += 1;
                        if depth == 1 {
                            in_object = true;
                            current_category = -1;
                            current_name.clear();
                        }
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 && in_object {
                            if current_category > 0 && !current_name.is_empty() {
                                parts.push(PartEntry {
                                    category: current_category,
                                    name: std::mem::take(&mut current_name),
                                });
                            }
                            in_object = false;
                        }
                    }
                    '"' if in_object && depth == 1 => {
                        let rest = &parts_json[i + 1..];
                        if let Some(end) = rest.find('"') {
                            let key = &rest[..end];
                            let after_key = &rest[end + 1..];
                            if let Some(colon) = after_key.find(':') {
                                let value_start = after_key[colon + 1..].trim_start();
                                match key {
                                    "category" => {
                                        let num_end = value_start
                                            .find(|c: char| !c.is_ascii_digit() && c != '-')
                                            .unwrap_or(value_start.len());
                                        if let Ok(n) = value_start[..num_end].parse::<i64>() {
                                            current_category = n;
                                        }
                                    }
                                    "name" => {
                                        if let Some(name_rest) = value_start.strip_prefix('"') {
                                            if let Some(name_end) = name_rest.find('"') {
                                                current_name = name_rest[..name_end].to_string();
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Group parts by category
            let mut by_category: BTreeMap<i64, Vec<String>> = BTreeMap::new();
            for part in parts {
                by_category
                    .entry(part.category)
                    .or_default()
                    .push(part.name);
            }

            // Sort parts within each category alphabetically (consistent ordering)
            for parts_vec in by_category.values_mut() {
                parts_vec.sort();
            }

            // Parse category names from the input
            let mut category_names: BTreeMap<i64, String> = BTreeMap::new();
            if let Some(cats_start) = data.find("\"categories\"") {
                if let Some(obj_start) = data[cats_start..].find('{') {
                    let cats_section = &data[cats_start + obj_start..];
                    // Simple parsing for "N": {"name": "..."}
                    let mut pos = 0;
                    while let Some(quote_pos) = cats_section[pos..].find('"') {
                        let key_start = pos + quote_pos + 1;
                        if let Some(key_end) = cats_section[key_start..].find('"') {
                            let key = &cats_section[key_start..key_start + key_end];
                            if let Ok(cat_id) = key.parse::<i64>() {
                                // Look for "name": "..." after this
                                let after = &cats_section[key_start + key_end..];
                                if let Some(name_pos) = after.find("\"name\"") {
                                    let name_section = &after[name_pos + 7..];
                                    if let Some(val_start) = name_section.find('"') {
                                        let name_rest = &name_section[val_start + 1..];
                                        if let Some(val_end) = name_rest.find('"') {
                                            category_names
                                                .insert(cat_id, name_rest[..val_end].to_string());
                                        }
                                    }
                                }
                            }
                            pos = key_start + key_end + 1;
                        } else {
                            break;
                        }
                    }
                }
            }

            // Build output JSON with clear metadata
            let mut json = String::from("{\n");
            json.push_str(&format!(
                "  \"version\": \"{}\",\n",
                env!("CARGO_PKG_VERSION")
            ));
            json.push_str("  \"source\": \"parts_database.json (memory-extracted part names)\",\n");
            json.push_str("  \"notes\": {\n");
            json.push_str("    \"part_names\": \"Extracted from game memory via string pattern matching - AUTHORITATIVE\",\n");
            json.push_str("    \"category_assignments\": \"Based on name prefix matching, verified by serial decode - VERIFIED\",\n");
            json.push_str("    \"part_order\": \"Alphabetical within category - NOT authoritative, use memory extraction for true indices\"\n");
            json.push_str("  },\n");
            json.push_str("  \"pools\": {\n");

            let pool_count = by_category.len();
            for (i, (category, cat_parts)) in by_category.iter().enumerate() {
                let cat_name = category_names
                    .get(category)
                    .cloned()
                    .unwrap_or_else(|| format!("Category {}", category));

                json.push_str(&format!("    \"{}\": {{\n", category));
                json.push_str(&format!(
                    "      \"name\": \"{}\",\n",
                    cat_name.replace('"', "\\\"")
                ));
                json.push_str(&format!("      \"part_count\": {},\n", cat_parts.len()));
                json.push_str("      \"parts\": [\n");

                for (j, part) in cat_parts.iter().enumerate() {
                    let escaped = part.replace('\\', "\\\\").replace('"', "\\\"");
                    json.push_str(&format!("        \"{}\"", escaped));
                    if j < cat_parts.len() - 1 {
                        json.push(',');
                    }
                    json.push('\n');
                }

                json.push_str("      ]\n");
                json.push_str("    }");
                if i < pool_count - 1 {
                    json.push(',');
                }
                json.push('\n');
            }

            json.push_str("  },\n");

            // Summary
            json.push_str("  \"summary\": {\n");
            json.push_str(&format!("    \"total_pools\": {},\n", pool_count));
            let total_parts: usize = by_category.values().map(|v| v.len()).sum();
            json.push_str(&format!("    \"total_parts\": {}\n", total_parts));
            json.push_str("  }\n");
            json.push_str("}\n");

            fs::write(&output, &json)?;

            println!(
                "Extracted {} part pools with {} total parts",
                pool_count, total_parts
            );
            println!("\nData sources:");
            println!("  Part names: Memory extraction (authoritative)");
            println!("  Categories: Prefix matching (verified by decode)");
            println!("  Part order: Alphabetical (not authoritative)");
            println!("\nWritten to: {}", output.display());
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Manufacturers { input, output },
        } => {
            println!("Extracting manufacturer data from {:?}...", input);
            let manufacturers = manifest::extract_manufacturer_names_from_pak(&input)?;

            println!("\nDiscovered {} manufacturers:", manufacturers.len());
            for (code, mfr) in &manufacturers {
                println!("  {} = {} (source: {})", code, mfr.name, mfr.name_source);
            }

            let json = serde_json::to_string_pretty(&manufacturers)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::WeaponTypes { input, output },
        } => {
            println!("Extracting weapon type data from {:?}...", input);
            let weapon_types = manifest::extract_weapon_types_from_pak(&input)?;

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

            let json = serde_json::to_string_pretty(&weapon_types)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::GearTypes { input, output },
        } => {
            println!("Extracting gear type data from {:?}...", input);
            let gear_types = manifest::extract_gear_types_from_pak(&input)?;

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

            let json = serde_json::to_string_pretty(&gear_types)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Elements { input, output },
        } => {
            println!("Extracting element types from {:?}...", input);
            let elements = manifest::extract_elements_from_pak(&input)?;

            println!("\nDiscovered {} element types:", elements.len());
            for name in elements.keys() {
                println!("  {}", name);
            }

            let json = serde_json::to_string_pretty(&elements)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Rarities { input, output },
        } => {
            println!("Extracting rarity tiers from {:?}...", input);
            let rarities = manifest::extract_rarities_from_pak(&input)?;

            println!("\nDiscovered {} rarity tiers:", rarities.len());
            for rarity in &rarities {
                println!("  {} ({}) = {}", rarity.tier, rarity.code, rarity.name);
            }

            let json = serde_json::to_string_pretty(&rarities)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Stats { input, output },
        } => {
            println!("Extracting stat types from {:?}...", input);
            let stats = manifest::extract_stats_from_pak(&input)?;

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

            let json = serde_json::to_string_pretty(&stats)?;
            fs::write(&output, json)?;
            println!("\nSaved to {:?}", output);
        }

        Commands::Idb { db, command } => {
            handle_items_db_command(command, &db)?;
        }

        #[cfg(feature = "research")]
        Commands::Manifest {
            paks,
            usmap,
            output,
            aes_key,
            skip_extract,
            extracted,
        } => {
            use std::process::Command as ProcessCommand;

            let extract_dir = if skip_extract {
                extracted
            } else {
                // Run uextract to extract pak files
                println!("Extracting pak files with uextract...");
                let mut cmd = ProcessCommand::new("uextract");
                cmd.arg(&paks)
                    .arg("-o")
                    .arg(&extracted)
                    .arg("--usmap")
                    .arg(&usmap)
                    .arg("--format")
                    .arg("json");

                if let Some(key) = &aes_key {
                    cmd.arg("--aes-key").arg(key);
                }

                let status = cmd.status().context("Failed to run uextract")?;
                if !status.success() {
                    bail!("uextract failed with status: {}", status);
                }
                extracted
            };

            // Generate manifest from extracted files
            println!("Generating manifest files...");
            manifest::extract_manifest(&extract_dir, &output)?;
            println!("Manifest files written to {}", output.display());
        }
    }

    Ok(())
}

fn handle_items_db_command(cmd: ItemsDbCommand, db: &PathBuf) -> Result<()> {
    match cmd {
        ItemsDbCommand::Init => {
            if let Some(parent) = db.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let wdb = bl4_idb::SqliteDb::open(db)?;
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
            let wdb = bl4_idb::SqliteDb::open(db)?;
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
                let update = bl4_idb::ItemUpdate {
                    name: name.clone(),
                    prefix: prefix.clone(),
                    manufacturer: manufacturer.clone(),
                    weapon_type: weapon_type.clone(),
                    rarity: rarity.clone(),
                    level,
                    element: element.clone(),
                    ..Default::default()
                };
                wdb.update_item(&serial, &update)?;
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
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?; // Ensure item_values table exists
            let filter = bl4_idb::ItemFilter {
                manufacturer: manufacturer.clone(),
                weapon_type: weapon_type.clone(),
                element: element.clone(),
                rarity: rarity.clone(),
                ..Default::default()
            };
            let items = wdb.list_items(&filter)?;

            if items.is_empty() {
                println!("No items found");
                return Ok(());
            }

            // Fetch all best values in a single query (avoids N+1)
            let all_best_values = wdb.get_all_items_best_values()?;

            let default_fields = vec![
                "serial",
                "manufacturer",
                "name",
                "weapon_type",
                "level",
                "element",
            ];
            let field_list: Vec<&str> = fields
                .as_ref()
                .map(|f| f.iter().map(|s| s.as_str()).collect())
                .unwrap_or_else(|| default_fields);

            match format {
                OutputFormat::Json => {
                    let filtered: Vec<serde_json::Value> = items
                        .iter()
                        .map(|item| {
                            let overrides = all_best_values.get(&item.serial);
                            filter_item_fields_with_overrides(item, &field_list, overrides)
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                }
                OutputFormat::Csv => {
                    println!("{}", field_list.join(","));
                    for item in &items {
                        let overrides = all_best_values.get(&item.serial);
                        let values: Vec<String> = field_list
                            .iter()
                            .map(|f| get_item_field_value_with_override(item, f, overrides))
                            .map(|v| escape_csv(&v))
                            .collect();
                        println!("{}", values.join(","));
                    }
                }
                OutputFormat::Table => {
                    let col_widths: Vec<usize> =
                        field_list.iter().map(|f| field_display_width(f)).collect();

                    let header: String = field_list
                        .iter()
                        .zip(&col_widths)
                        .map(|(f, w)| format!("{:<width$}", f, width = w))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("{}", header);
                    println!("{}", "-".repeat(header.len()));

                    for item in &items {
                        let overrides = all_best_values.get(&item.serial);
                        let row: String = field_list
                            .iter()
                            .zip(&col_widths)
                            .map(|(f, w)| {
                                let val = get_item_field_value_with_override(item, f, overrides);
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

        ItemsDbCommand::Show { serial } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            let weapon = wdb.get_item(&serial)?;

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
                println!("\n--- Metadata ---");
                println!("Source:       {}", w.source.as_deref().unwrap_or("-"));
                println!("Legal:        {}", if w.legal { "yes" } else { "no" });
                println!("Status:       {}", w.verification_status);
                println!("Created:      {}", w.created_at);

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

                let attachments = wdb.get_attachments(&w.serial)?;
                if !attachments.is_empty() {
                    println!("\nAttachments:");
                    for a in attachments {
                        println!("  {} ({}, {})", a.name, a.view, a.mime_type);
                    }
                }
            } else {
                println!("Item not found: {}", serial);
            }
        }

        ItemsDbCommand::Attach {
            image,
            serial,
            name,
            popup,
            detail,
        } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            let view = if popup {
                "POPUP"
            } else if detail {
                "DETAIL"
            } else {
                "OTHER"
            };
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
            let attachment_id =
                wdb.add_attachment(&serial, &attachment_name, mime_type, &data, view)?;
            println!(
                "Added attachment '{}' (ID {}, view: {}) to item {}",
                attachment_name, attachment_id, view, serial
            );
        }

        ItemsDbCommand::Import { path } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            if path.join("serial.txt").exists() {
                let serial = wdb.import_from_dir(&path)?;
                println!("Imported item {} from {}", serial, path.display());
            } else {
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
                            Err(e) => eprintln!("Failed to import {}: {}", subdir.display(), e),
                        }
                    }
                }
                println!("\nImported {} items", imported);
            }
        }

        ItemsDbCommand::Export { serial, output } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.export_to_dir(&serial, &output)?;
            println!("Exported item {} to {}", serial, output.display());
        }

        ItemsDbCommand::Stats => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
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
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;
            let status: bl4_idb::VerificationStatus = status.parse()?;
            wdb.set_verification_status(&serial, status, notes.as_deref())?;
            println!("Updated item {} to status: {}", serial, status);
        }

        ItemsDbCommand::DecodeAll { force } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;
            let items = wdb.list_items(&bl4_idb::ItemFilter::default())?;

            let mut decoded = 0;
            let mut skipped = 0;
            let mut failed = 0;

            for item in &items {
                if !force && (item.manufacturer.is_some() || item.weapon_type.is_some()) {
                    skipped += 1;
                    continue;
                }

                match bl4::ItemSerial::decode(&item.serial) {
                    Ok(decoded_item) => {
                        let (mfg, wtype) = if let Some(mfg_id) = decoded_item.manufacturer {
                            bl4::parts::weapon_info_from_first_varint(mfg_id)
                                .map(|(m, w)| (Some(m.to_string()), Some(w.to_string())))
                                .unwrap_or((None, None))
                        } else if let Some(group_id) = decoded_item.part_group_id() {
                            let cat_name = bl4::parts::category_name_for_type(
                                decoded_item.item_type,
                                group_id,
                            );
                            (None, cat_name.map(|s| s.to_string()))
                        } else {
                            (None, None)
                        };

                        let level = decoded_item
                            .level
                            .and_then(bl4::parts::level_from_code)
                            .map(|(capped, _raw)| capped as i32);

                        let update = bl4_idb::ItemUpdate {
                            manufacturer: mfg,
                            weapon_type: wtype,
                            level,
                            ..Default::default()
                        };
                        wdb.update_item(&item.serial, &update)?;
                        wdb.set_item_type(&item.serial, &decoded_item.item_type.to_string())?;

                        if item.verification_status == bl4_idb::VerificationStatus::Unverified {
                            wdb.set_verification_status(
                                &item.serial,
                                bl4_idb::VerificationStatus::Decoded,
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
            let steam_id = save
                .to_string_lossy()
                .split('/')
                .find(|s| s.len() == 17 && s.chars().all(|c| c.is_ascii_digit()))
                .map(String::from)
                .context("Could not extract Steam ID from path")?;

            let save_data = std::fs::read(&save)?;
            let yaml_data = bl4::decrypt_sav(&save_data, &steam_id)?;
            let yaml_str = String::from_utf8(yaml_data)?;
            let yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_str)?;

            let mut serials = Vec::new();
            extract_serials_from_yaml(&yaml, &mut serials);
            serials.sort();
            serials.dedup();

            println!(
                "Found {} unique serials in {}",
                serials.len(),
                save.display()
            );

            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;
            let mut added = 0;
            let mut skipped = 0;

            for serial in &serials {
                match wdb.add_item(serial) {
                    Ok(_) => added += 1,
                    Err(_) => skipped += 1,
                }
            }
            println!("Added {} new items, {} already existed", added, skipped);

            if decode && added > 0 {
                println!("Decoding new items...");
                let items = wdb.list_items(&bl4_idb::ItemFilter::default())?;
                let mut decoded_count = 0;

                for item in &items {
                    if item.manufacturer.is_some() {
                        continue;
                    }
                    if let Ok(decoded_item) = bl4::ItemSerial::decode(&item.serial) {
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
                            .map(|(capped, _)| capped as i32);

                        let update = bl4_idb::ItemUpdate {
                            manufacturer: mfg,
                            weapon_type: wtype,
                            level,
                            ..Default::default()
                        };
                        let _ = wdb.update_item(&item.serial, &update);

                        if item.verification_status == bl4_idb::VerificationStatus::Unverified {
                            let _ = wdb.set_verification_status(
                                &item.serial,
                                bl4_idb::VerificationStatus::Decoded,
                                None,
                            );
                        }
                        decoded_count += 1;
                    }
                }
                println!("Decoded {} items", decoded_count);
            }

            if legal {
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
            let wdb = bl4_idb::SqliteDb::open(db)?;
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
            let wdb = bl4_idb::SqliteDb::open(db)?;
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

        ItemsDbCommand::SetValue {
            serial,
            field,
            value,
            source,
            source_detail,
            confidence,
        } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            let source: bl4_idb::ValueSource = source.parse()?;
            let confidence: bl4_idb::Confidence = confidence.parse()?;

            wdb.set_value(
                &serial,
                &field,
                &value,
                source,
                source_detail.as_deref(),
                confidence,
            )?;
            println!(
                "Set {}.{} = {} (source: {}, confidence: {})",
                serial, field, value, source, confidence
            );
        }

        ItemsDbCommand::GetValues { serial, field } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            let values = wdb.get_values(&serial, &field)?;

            if values.is_empty() {
                println!("No values found for {}.{}", serial, field);
            } else {
                println!("Values for {}.{}:", serial, field);
                for v in values {
                    println!(
                        "  {} ({}, {}): {}",
                        v.source,
                        v.confidence,
                        v.source_detail.as_deref().unwrap_or("-"),
                        v.value
                    );
                }
            }
        }

        ItemsDbCommand::MigrateValues { dry_run } => {
            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            if dry_run {
                println!("Dry run - showing what would be migrated:");
            }

            let stats = wdb.migrate_column_values(dry_run)?;

            println!();
            println!(
                "Migration {}:",
                if dry_run { "preview" } else { "complete" }
            );
            println!("  Items processed: {}", stats.items_processed);
            println!("  Values migrated: {}", stats.values_migrated);
            println!("  Values skipped (already exist): {}", stats.values_skipped);
        }

        ItemsDbCommand::Publish {
            server,
            serial,
            attachments,
            dry_run,
        } => {
            use bl4_idb::AttachmentsRepository;

            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            let items = if let Some(serial) = serial {
                match wdb.get_item(&serial)? {
                    Some(item) => vec![item],
                    None => bail!("Item not found: {}", serial),
                }
            } else {
                wdb.list_items(&bl4_idb::ItemFilter::default())?
            };

            if items.is_empty() {
                println!("No items to publish");
                return Ok(());
            }

            // Check server capabilities if attachments requested
            let server_supports_attachments = if attachments {
                let caps_url = format!("{}/capabilities", server.trim_end_matches('/'));
                match ureq::get(&caps_url).call() {
                    Ok(resp) => {
                        let caps: serde_json::Value = resp.into_json()?;
                        caps["attachments"].as_bool().unwrap_or(false)
                    }
                    Err(_) => {
                        println!(
                            "Warning: Could not check server capabilities, skipping attachments"
                        );
                        false
                    }
                }
            } else {
                false
            };

            println!("Publishing {} items to {}", items.len(), server);
            if attachments && server_supports_attachments {
                println!("  Attachments: enabled");
            } else if attachments {
                println!("  Attachments: requested but server doesn't support them");
            }

            if dry_run {
                println!("\nDry run - would publish:");
                for item in &items {
                    let attachment_count = wdb.get_attachments(&item.serial)?.len();
                    if attachment_count > 0 && server_supports_attachments {
                        println!("  {} ({} attachments)", item.serial, attachment_count);
                    } else {
                        println!("  {}", item.serial);
                    }
                }
                return Ok(());
            }

            // Build bulk request
            let bulk_items: Vec<serde_json::Value> = items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "serial": item.serial,
                        "name": item.name,
                        "source": "bl4-cli"
                    })
                })
                .collect();

            let url = format!("{}/items/bulk", server.trim_end_matches('/'));

            let response = ureq::post(&url)
                .set("Content-Type", "application/json")
                .send_json(serde_json::json!({ "items": bulk_items }));

            match response {
                Ok(resp) => {
                    let result: serde_json::Value = resp.into_json()?;
                    let succeeded = result["succeeded"].as_u64().unwrap_or(0);
                    let failed = result["failed"].as_u64().unwrap_or(0);

                    println!("\nPublish complete:");
                    println!("  Items succeeded: {}", succeeded);
                    println!("  Items failed: {}", failed);

                    if let Some(results) = result["results"].as_array() {
                        for r in results {
                            if !r["created"].as_bool().unwrap_or(true) {
                                println!("  {} - {}", r["serial"], r["message"]);
                            }
                        }
                    }
                }
                Err(ureq::Error::Status(code, resp)) => {
                    let body = resp.into_string().unwrap_or_default();
                    bail!("Server returned {}: {}", code, body);
                }
                Err(e) => {
                    bail!("Request failed: {}", e);
                }
            }

            // Upload attachments if enabled
            if server_supports_attachments {
                let mut attachments_uploaded = 0;
                let mut attachments_failed = 0;

                for item in &items {
                    let item_attachments = wdb.get_attachments(&item.serial)?;
                    for attachment in item_attachments {
                        let data = match wdb.get_attachment_data(attachment.id)? {
                            Some(d) => d,
                            None => continue,
                        };

                        let upload_url = format!(
                            "{}/items/{}/attachments",
                            server.trim_end_matches('/'),
                            urlencoding::encode(&item.serial)
                        );

                        // Build multipart form
                        let boundary = "----bl4clipublish";
                        let mut body = Vec::new();

                        // File field
                        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
                        body.extend_from_slice(
                            format!(
                                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                                attachment.name
                            )
                            .as_bytes(),
                        );
                        body.extend_from_slice(
                            format!("Content-Type: {}\r\n\r\n", attachment.mime_type).as_bytes(),
                        );
                        body.extend_from_slice(&data);
                        body.extend_from_slice(b"\r\n");

                        // View field
                        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
                        body.extend_from_slice(
                            b"Content-Disposition: form-data; name=\"view\"\r\n\r\n",
                        );
                        body.extend_from_slice(attachment.view.as_bytes());
                        body.extend_from_slice(b"\r\n");

                        // End boundary
                        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

                        let result = ureq::post(&upload_url)
                            .set(
                                "Content-Type",
                                &format!("multipart/form-data; boundary={}", boundary),
                            )
                            .send_bytes(&body);

                        match result {
                            Ok(_) => attachments_uploaded += 1,
                            Err(e) => {
                                eprintln!(
                                    "Failed to upload attachment {} for {}: {}",
                                    attachment.name, item.serial, e
                                );
                                attachments_failed += 1;
                            }
                        }
                    }
                }

                if attachments_uploaded > 0 || attachments_failed > 0 {
                    println!("\nAttachments:");
                    println!("  Uploaded: {}", attachments_uploaded);
                    if attachments_failed > 0 {
                        println!("  Failed: {}", attachments_failed);
                    }
                }
            }
        }

        ItemsDbCommand::Pull {
            server,
            authoritative,
            dry_run,
        } => {
            use bl4_idb::{Confidence, ItemsRepository, ValueSource};

            let wdb = bl4_idb::SqliteDb::open(db)?;
            wdb.init()?;

            println!("Fetching items from {}...", server);
            if authoritative {
                println!("  Mode: authoritative (remote values will overwrite local)");
            }

            // Fetch all items from server (paginated)
            let mut all_items: Vec<serde_json::Value> = Vec::new();
            let mut offset = 0;
            let limit = 1000;

            loop {
                let url = format!(
                    "{}/items?limit={}&offset={}",
                    server.trim_end_matches('/'),
                    limit,
                    offset
                );

                let response = ureq::get(&url).call();

                match response {
                    Ok(resp) => {
                        let result: serde_json::Value = resp.into_json()?;
                        let items = result["items"].as_array();
                        let total = result["total"].as_u64().unwrap_or(0);

                        if let Some(items) = items {
                            if items.is_empty() {
                                break;
                            }
                            all_items.extend(items.clone());
                            println!("  Fetched {} / {} items", all_items.len(), total);

                            if all_items.len() >= total as usize {
                                break;
                            }
                            offset += limit;
                        } else {
                            break;
                        }
                    }
                    Err(ureq::Error::Status(code, resp)) => {
                        let body = resp.into_string().unwrap_or_default();
                        bail!("Server returned {}: {}", code, body);
                    }
                    Err(e) => {
                        bail!("Request failed: {}", e);
                    }
                }
            }

            if all_items.is_empty() {
                println!("No items to pull");
                return Ok(());
            }

            println!("\nPulled {} items from server", all_items.len());

            if dry_run {
                println!("\nDry run - would process:");
                let mut new_items = 0;
                let mut existing_items = 0;
                for item in &all_items {
                    if let Some(serial) = item["serial"].as_str() {
                        if wdb.get_item(serial)?.is_none() {
                            println!("  [NEW] {}", serial);
                            new_items += 1;
                        } else {
                            existing_items += 1;
                        }
                    }
                }
                println!("\n{} new items, {} existing", new_items, existing_items);
                if authoritative {
                    println!(
                        "With --authoritative, values for all {} items would be updated",
                        all_items.len()
                    );
                } else {
                    println!("Without --authoritative, only new items would get values");
                }
                return Ok(());
            }

            // Merge into local database
            let mut new_items = 0;
            let mut updated_items = 0;
            let mut values_set = 0;

            // Field mappings: JSON key -> ItemField name
            let field_mappings = [
                ("name", "name"),
                ("prefix", "prefix"),
                ("manufacturer", "manufacturer"),
                ("weapon_type", "weapon_type"),
                ("rarity", "rarity"),
                ("level", "level"),
                ("element", "element"),
                ("item_type", "item_type"),
            ];

            for item in &all_items {
                let serial = match item["serial"].as_str() {
                    Some(s) => s,
                    None => continue,
                };

                let is_new = wdb.get_item(serial)?.is_none();

                if is_new {
                    // Add new item
                    if let Err(e) = wdb.add_item(serial) {
                        eprintln!("Failed to add {}: {}", serial, e);
                        continue;
                    }
                    new_items += 1;
                } else if !authoritative {
                    // Item exists and not authoritative - skip value updates
                    continue;
                } else {
                    updated_items += 1;
                }

                // Set values from community with CommunityTool source, Uncertain confidence
                for (json_key, field_name) in &field_mappings {
                    let value = if *json_key == "level" {
                        item[*json_key].as_i64().map(|v| v.to_string())
                    } else {
                        item[*json_key].as_str().map(String::from)
                    };

                    if let Some(val) = value {
                        if !val.is_empty() {
                            let _ = wdb.set_value(
                                serial,
                                field_name,
                                &val,
                                ValueSource::CommunityTool,
                                Some(&server),
                                Confidence::Uncertain,
                            );
                            values_set += 1;
                        }
                    }
                }

                // Set source metadata
                let _ = wdb.set_source(serial, "community-pull");
            }

            println!("\nPull complete:");
            println!("  New items: {}", new_items);
            if authoritative {
                println!("  Updated items: {}", updated_items);
            }
            println!("  Values set: {}", values_set);
        }
    }

    Ok(())
}

fn merge_databases(source: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    use rusqlite::{params, Connection};

    println!("Merging {} -> {}", source.display(), dest.display());

    let src_conn = Connection::open(source)?;
    let dest_conn = Connection::open(dest)?;

    let _ = dest_conn.execute("ALTER TABLE weapons ADD COLUMN tier TEXT", []);

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
    let count: i64 = dest_conn.query_row(
        "SELECT COUNT(*) FROM weapons WHERE tier IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    println!("Destination now has {} tiered items", count);

    Ok(())
}

fn extract_serials_from_yaml(value: &serde_yaml::Value, serials: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) => {
            if s.starts_with("@Ug") && s.len() >= 10 {
                serials.push(s.clone());
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
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

fn get_item_field_value(item: &bl4_idb::Item, field: &str) -> String {
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
        "status" => item.verification_status.to_string(),
        "legal" => if item.legal { "true" } else { "false" }.to_string(),
        "source" => item.source.clone().unwrap_or_default(),
        "created_at" => item.created_at.clone(),
        _ => String::new(),
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn get_item_field_value_with_override(
    item: &bl4_idb::Item,
    field: &str,
    overrides: Option<&std::collections::HashMap<String, String>>,
) -> String {
    // Check overrides first (from item_values table)
    if let Some(ovr) = overrides {
        if let Some(val) = ovr.get(field) {
            return val.clone();
        }
    }
    // Fall back to base item data
    get_item_field_value(item, field)
}

fn filter_item_fields_with_overrides(
    item: &bl4_idb::Item,
    fields: &[&str],
    overrides: Option<&std::collections::HashMap<String, String>>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in fields {
        let value = get_item_field_value_with_override(item, field, overrides);
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

/// Get display width for a field (including non-ItemField fields like "serial")
fn field_display_width(field: &str) -> usize {
    match field {
        "serial" => 35,
        other => other
            .parse::<bl4_idb::ItemField>()
            .map(|f| f.display_width())
            .unwrap_or(15),
    }
}
