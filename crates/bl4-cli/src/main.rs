mod config;
mod memory;

use anyhow::{bail, Context, Result};
use byteorder::ByteOrder;
use clap::{Parser, Subcommand};
use config::Config;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "bl4")]
#[command(about = "Borderlands 4 Save Editor", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

    /// Inspect a save file (decrypt and display info)
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

    /// Configure default settings
    Configure {
        /// Set default Steam ID
        #[arg(long)]
        steam_id: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },

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
    },

    /// Read/analyze game memory (live process or dump file)
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
    Launch {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Show info about a usmap file
    UsmapInfo {
        /// Path to usmap file
        path: PathBuf,
    },

    /// Search usmap for struct/enum names
    UsmapSearch {
        /// Path to usmap file
        path: PathBuf,

        /// Search pattern (case-insensitive substring match)
        pattern: String,

        /// Show struct properties
        #[arg(short, long)]
        verbose: bool,
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
    },

    /// Extract part definitions from UObjects with authoritative Category/Index from SerialIndex
    ExtractParts {
        /// Output file for extracted parts with categories
        #[arg(short, long, default_value = "parts_with_categories.json")]
        output: PathBuf,
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
fn update_backup_metadata(input: &PathBuf) -> Result<()> {
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

        Commands::Decrypt {
            input,
            output,
            steam_id,
        } => {
            let steam_id = get_steam_id(steam_id)?;
            // Read from file or stdin
            let encrypted = if let Some(path) = input {
                fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?
            } else {
                let mut buf = Vec::new();
                io::stdin()
                    .read_to_end(&mut buf)
                    .context("Failed to read from stdin")?;
                buf
            };

            let yaml_data =
                bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

            // Write to file or stdout
            if let Some(path) = output {
                fs::write(&path, &yaml_data)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            } else {
                io::stdout()
                    .write_all(&yaml_data)
                    .context("Failed to write to stdout")?;
            }
        }

        Commands::Encrypt {
            input,
            output,
            steam_id,
        } => {
            let steam_id = get_steam_id(steam_id)?;
            // Read from file or stdin
            let yaml_data = if let Some(path) = input {
                fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?
            } else {
                let mut buf = Vec::new();
                io::stdin()
                    .read_to_end(&mut buf)
                    .context("Failed to read from stdin")?;
                buf
            };

            let encrypted =
                bl4::encrypt_sav(&yaml_data, &steam_id).context("Failed to encrypt YAML data")?;

            // Write to file or stdout
            if let Some(path) = output {
                fs::write(&path, &encrypted)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            } else {
                io::stdout()
                    .write_all(&encrypted)
                    .context("Failed to write to stdout")?;
            }
        }

        Commands::Edit {
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
            let encrypted =
                fs::read(&input).with_context(|| format!("Failed to read {}", input.display()))?;

            let yaml_data =
                bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

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

        Commands::Get {
            input,
            steam_id,
            query,
            level,
            money,
            info,
            all,
        } => {
            let steam_id = get_steam_id(steam_id)?;
            let encrypted =
                fs::read(&input).with_context(|| format!("Failed to read {}", input.display()))?;

            let yaml_data =
                bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

            let save = bl4::SaveFile::from_yaml(&yaml_data).context("Failed to parse save file")?;

            // Handle query path if provided
            if let Some(query_path) = query {
                let result = save.get(&query_path).context("Query failed")?;
                println!("{}", serde_yaml::to_string(&result)?);
                return Ok(());
            }

            let show_all = all || (!level && !money && !info);

            // Extract character info
            if show_all || info {
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

            // Extract level/XP info
            if show_all || level {
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

            // Extract currency info
            if show_all || money {
                if let Some(cash) = save.get_cash() {
                    println!("Cash: {}", cash);
                }
                if let Some(eridium) = save.get_eridium() {
                    println!("Eridium: {}", eridium);
                }
            }
        }

        Commands::Set {
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
            let encrypted =
                fs::read(&input).with_context(|| format!("Failed to read {}", input.display()))?;

            let yaml_data =
                bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

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

        Commands::Decode {
            serial,
            verbose,
            debug,
            analyze,
        } => {
            let item = bl4::ItemSerial::decode(&serial).context("Failed to decode serial")?;
            let parts_db = bl4::PartsDatabase::load_embedded();

            println!("Serial: {}", item.original);
            println!(
                "Item type: {} ({})",
                item.item_type,
                item.item_type_description()
            );
            if let Some(mfr) = item.manufacturer_name() {
                println!("Manufacturer: {}", mfr);
            }
            println!("Decoded bytes: {}", item.raw_bytes.len());
            println!("Hex: {}", item.hex_dump());
            println!("Tokens: {}", item.format_tokens());
            println!("Named:  {}", item.format_tokens_named(&parts_db));

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
                                let group_name = bl4::category_name(group_id as i64)
                                    .unwrap_or("Unknown");
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
                                println!("  Utility items - encoding formula not yet determined");
                                println!("  Raw value: {}", value);
                            }
                            '!' | '#' => {
                                // Class mods - formula TBD
                                println!("  Class mods - encoding formula not yet determined");
                                println!("  Raw value: {}", value);
                            }
                            _ => {
                                println!("  Unknown item type '{}' - encoding formula not determined", item.item_type);
                            }
                        }

                        println!();
                        println!("Bit split analysis (for research):");
                        for split in [8, 10, 12, 13, 14] {
                            let high = value >> split;
                            let low = value & ((1 << split) - 1);
                            println!("  Split at bit {:2}: high={:6}  low={:6}", split, high, low);
                        }
                    } else {
                        println!("\n=== First Token Analysis ===");
                        println!("First token is not numeric: {:?}", first_token);
                    }
                }
            }
        }

        Commands::Memory { preload, dump, maps, action } => {
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
                MemoryAction::BuildPartsDb { ref input, ref output } => {
                    // This command doesn't need memory access - just reads/writes JSON
                    println!("Building parts database from {}...", input.display());

                    // Known part group IDs (from serial analysis)
                    let known_groups: Vec<(&str, i64, &str)> = vec![
                        // Weapons (categories 2-30)
                        ("DAD_PS", 2, "Daedalus Pistol"),
                        ("JAK_PS", 3, "Jakobs Pistol"),
                        ("TED_PS", 4, "Tediore Pistol"),
                        ("TOR_PS", 5, "Torgue Pistol"),
                        ("ORD_PS", 6, "Order Pistol"),
                        ("VLA_PS", 7, "Vladof Pistol"),
                        ("DAD_SG", 8, "Daedalus Shotgun"),
                        ("JAK_SG", 9, "Jakobs Shotgun"),
                        ("TED_SG", 10, "Tediore Shotgun"),
                        ("TOR_SG", 11, "Torgue Shotgun"),
                        ("BOR_SG", 12, "Bor Shotgun"),
                        ("DAD_AR", 13, "Daedalus Assault Rifle"),
                        ("JAK_AR", 14, "Jakobs Assault Rifle"),
                        ("TED_AR", 15, "Tediore Assault Rifle"),
                        ("TOR_AR", 16, "Torgue Assault Rifle"),
                        ("VLA_AR", 17, "Vladof Assault Rifle"),
                        ("ORD_AR", 18, "Order Assault Rifle"),
                        ("MAL_SG", 19, "Maliwan Shotgun"),
                        ("DAD_SM", 20, "Daedalus SMG"),
                        ("BOR_SM", 21, "Bor SMG"),
                        ("VLA_SM", 22, "Vladof SMG"),
                        ("MAL_SM", 23, "Maliwan SMG"),
                        ("bor_sr", 25, "Bor Sniper"),
                        ("JAK_SR", 26, "Jakobs Sniper"),
                        ("VLA_SR", 27, "Vladof Sniper"),
                        ("ORD_SR", 28, "Order Sniper"),
                        ("MAL_SR", 29, "Maliwan Sniper"),
                        // Heavy weapons (categories 240+)
                        ("VLA_HW", 244, "Vladof Heavy Weapon"),
                        ("TOR_HW", 245, "Torgue Heavy Weapon"),
                        ("BOR_HW", 246, "Bor Heavy Weapon"),
                        ("MAL_HW", 247, "Maliwan Heavy Weapon"),
                        // Shields (categories 279+)
                        ("energy_shield", 279, "Energy Shield"),
                        ("bor_shield", 280, "Bor Shield"),
                        ("dad_shield", 281, "Daedalus Shield"),
                        ("jak_shield", 282, "Jakobs Shield"),
                        ("Armor_Shield", 283, "Armor Shield"),
                        ("mal_shield", 284, "Maliwan Shield"),
                        ("ord_shield", 285, "Order Shield"),
                        ("ted_shield", 286, "Tediore Shield"),
                        ("tor_shield", 287, "Torgue Shield"),
                        ("vla_shield", 288, "Vladof Shield"),
                        // Gadgets and gear
                        ("grenade_gadget", 300, "Grenade Gadget"),
                        ("turret_gadget", 310, "Turret Gadget"),
                        ("repair_kit", 320, "Repair Kit"),
                        ("Terminal_Gadget", 330, "Terminal Gadget"),
                        // Enhancements
                        ("DAD_Enhancement", 400, "Daedalus Enhancement"),
                        ("BOR_Enhancement", 401, "Bor Enhancement"),
                        ("JAK_Enhancement", 402, "Jakobs Enhancement"),
                        ("MAL_Enhancement", 403, "Maliwan Enhancement"),
                        ("ORD_Enhancement", 404, "Order Enhancement"),
                        ("TED_Enhancement", 405, "Tediore Enhancement"),
                        ("TOR_Enhancement", 406, "Torgue Enhancement"),
                        ("VLA_Enhancement", 407, "Vladof Enhancement"),
                        ("COV_Enhancement", 408, "COV Enhancement"),
                        ("ATL_Enhancement", 409, "Atlas Enhancement"),
                    ];

                    let parts_json = std::fs::read_to_string(input)
                        .context("Failed to read parts dump file")?;

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
                        if let Some(parts) = parts_by_prefix.get(*prefix) {
                            for (idx, part_name) in parts.iter().enumerate() {
                                db_entries.push((*category, idx as i16, part_name.clone(), description.to_string()));
                            }
                        }
                    }

                    let known_prefixes: std::collections::HashSet<&str> =
                        known_groups.iter().map(|(p, _, _)| *p).collect();

                    for (prefix, parts) in &parts_by_prefix {
                        if !known_prefixes.contains(prefix.as_str()) {
                            for (idx, part_name) in parts.iter().enumerate() {
                                db_entries.push((-1, idx as i16, part_name.clone(), format!("{} (unmapped)", prefix)));
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
                        if i < db_entries.len() - 1 { json.push(','); }
                        json.push('\n');
                    }
                    json.push_str("  ],\n  \"categories\": {\n");

                    let mut category_counts: std::collections::BTreeMap<i64, (usize, String)> =
                        std::collections::BTreeMap::new();
                    for (category, _, _, group) in &db_entries {
                        let entry = category_counts.entry(*category).or_insert((0, group.clone()));
                        entry.0 += 1;
                    }
                    let cat_count = category_counts.len();
                    for (i, (category, (count, name))) in category_counts.iter().enumerate() {
                        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
                        json.push_str(&format!("    \"{}\": {{\"count\": {}, \"name\": \"{}\"}}", category, count, escaped));
                        if i < cat_count - 1 { json.push(','); }
                        json.push('\n');
                    }
                    json.push_str("  }\n}\n");

                    std::fs::write(output, &json)?;
                    println!("Built parts database with {} entries across {} categories", db_entries.len(), category_counts.len());
                    println!("Written to: {}", output.display());
                    return Ok(());
                }
                MemoryAction::ExtractParts { ref output } => {
                    // This command requires a memory dump to extract UObjects
                    let dump_path = match dump {
                        Some(ref p) => p.clone(),
                        None => bail!("ExtractParts requires a memory dump file. Use --dump <path>"),
                    };

                    println!("Extracting part definitions from dump...");
                    let source: Box<dyn memory::MemorySource> =
                        Box::new(memory::DumpFile::open(&dump_path)?);

                    // First discover GNames
                    println!("Discovering GNames pool...");
                    let gnames = memory::discover_gnames(source.as_ref())?;
                    println!("  GNames at: {:#x}", gnames.address);

                    // Discover GUObjectArray
                    println!("Discovering GUObjectArray...");
                    let guobjects = memory::discover_guobject_array(source.as_ref(), gnames.address)?;
                    println!("  GUObjectArray at: {:#x}", guobjects.address);
                    println!("  NumElements: {}", guobjects.num_elements);

                    // Find InventoryPartDef UClass
                    println!("Finding InventoryPartDef UClass...");
                    let inventory_part_def_class = memory::find_uclass_by_name(
                        source.as_ref(),
                        gnames.address,
                        &guobjects,
                        "InventoryPartDef",
                    )?;
                    println!("  InventoryPartDef UClass at: {:#x}", inventory_part_def_class);

                    // Extract part definitions
                    println!("Extracting part definitions with SerialIndex...");
                    let parts = memory::extract_part_definitions(
                        source.as_ref(),
                        gnames.address,
                        &guobjects,
                        inventory_part_def_class,
                    )?;

                    println!("Found {} part definitions", parts.len());

                    // Group by category for summary
                    let mut by_category: std::collections::BTreeMap<i64, Vec<&memory::PartDefinition>> =
                        std::collections::BTreeMap::new();
                    for part in &parts {
                        by_category.entry(part.category).or_default().push(part);
                    }

                    println!("\nCategories found:");
                    for (category, cat_parts) in &by_category {
                        println!("  Category {}: {} parts", category, cat_parts.len());
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
                        json.push_str(&format!(
                            "    \"{}\": {}",
                            category,
                            cat_parts.len()
                        ));
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
                        None => bail!("FindObjectsByPattern requires a memory dump file. Use --dump <path>"),
                    };

                    println!("Searching for objects matching '{}'...", pattern);
                    let source: Box<dyn memory::MemorySource> =
                        Box::new(memory::DumpFile::open(&dump_path)?);

                    // Discover GUObjectArray
                    println!("Discovering GNames pool...");
                    let gnames = memory::discover_gnames(source.as_ref())?;
                    println!("  GNames at: {:#x}", gnames.address);

                    println!("Discovering GUObjectArray...");
                    let guobjects = memory::discover_guobject_array(source.as_ref(), gnames.address)?;
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
                        None => bail!("GenerateObjectMap requires a memory dump file. Use --dump <path>"),
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
                    let guobjects = memory::discover_guobject_array(source.as_ref(), gnames.address)?;
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
                    println!("  {} classes, {} total objects",
                             map.len(),
                             map.values().map(|v| v.len()).sum::<usize>());

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
                                eprintln!("Invalid template format: {} (expected key=value)", template);
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
                            println!("LD_PRELOAD={} {} %command%", lib_path.display(), env_vars.join(" "));
                        }

                        return Ok(());
                    }
                    MemoryAction::Monitor { .. } => {
                        // Monitor works the same in preload mode, fall through
                        // This is handled below in the main match
                    }
                    _ => {
                        bail!("This command is not available in --preload mode. \
                               Remove --preload to use direct memory injection.");
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
                        memory::DumpFile::open(dump_path)
                            .context("Failed to open dump file")?
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
                                    eprintln!("GNames not found (required for GUObjectArray): {}", e);
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
                    let gnames = memory::discover_gnames(source)
                        .context("Failed to find GNames pool")?;

                    println!("GNames at: {:#x}", gnames.address);

                    // For now, we can only search for class names in the FName pool
                    // Full object enumeration requires GUObjectArray
                    if let Some(class_name) = class {
                        println!("Searching for '{}' in FName pool...", class_name);

                        // Search for the class name in memory
                        let pattern = class_name.as_bytes();
                        let results = memory::scan_pattern(source, pattern, &vec![1u8; pattern.len()])?;

                        println!("Found {} occurrences of '{}':", results.len().min(limit), class_name);
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

                    let gnames = memory::discover_gnames(source)
                        .context("Failed to find GNames pool")?;

                    println!("Searching for \"{}\" in FName pool at {:#x}...", query, gnames.address);

                    // Search in block 0 (where common names like "Class" reside)
                    let search_bytes = query.as_bytes();

                    // Read block 0 data and search
                    let block0_data = source.read_bytes(gnames.address, 256 * 1024)?;

                    let mut found = Vec::new();
                    for (pos, window) in block0_data.windows(search_bytes.len()).enumerate() {
                        if window == search_bytes {
                            // Found match - try to find the entry start
                            // Look backwards for header bytes
                            if pos >= 2 {
                                let header = &block0_data[pos-2..pos];
                                let header_val = byteorder::LE::read_u16(header);
                                let len = (header_val >> 6) as usize;

                                // Alternative format check
                                let alt_len = ((header[0] >> 1) & 0x3F) as usize;

                                if len == search_bytes.len() || alt_len == search_bytes.len() {
                                    let byte_offset = pos - 2;
                                    let fname_index = byte_offset / 2;
                                    found.push((fname_index, pos - 2));
                                }
                            }
                        }
                    }

                    if found.is_empty() {
                        println!("No matches found for \"{}\"", query);
                    } else {
                        println!("Found {} potential matches:", found.len());
                        for (idx, byte_offset) in found.iter().take(20) {
                            println!("  FName index ~{} (byte offset {:#x})", idx, byte_offset);

                            // Read and show the entry
                            if *byte_offset + 16 < block0_data.len() {
                                let entry_data = &block0_data[*byte_offset..*byte_offset + 16.min(block0_data.len() - byte_offset)];
                                print!("    Raw: ");
                                for b in entry_data.iter().take(16) {
                                    print!("{:02x} ", b);
                                }
                                println!();
                            }
                        }
                    }
                }

                MemoryAction::FindClassUClass => {
                    let source = mem_source!();

                    // First discover FNamePool to resolve names
                    let gnames = memory::discover_gnames(source)
                        .context("Failed to find GNames pool")?;
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
                        println!("\nTrying {} - ClassPrivate={:#x}, NamePrivate={:#x}...", desc, class_off, name_off);

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
                                let vtable_ptr = byteorder::LE::read_u64(&data[offset..offset + 8]) as usize;
                                if vtable_ptr < 0x140000000 || vtable_ptr > 0x175000000 {
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
                                let class_ptr = byteorder::LE::read_u64(&data[offset + class_off..offset + class_off + 8]) as usize;
                                if class_ptr != obj_addr {
                                    continue;
                                }

                                // Self-referential! Read the name
                                let fname_idx = byteorder::LE::read_u32(&data[offset + name_off..offset + name_off + 4]);
                                let name = fname_reader.read_name(source, fname_idx)
                                    .unwrap_or_else(|_| format!("<idx:{}>", fname_idx));

                                found_self_refs.push((obj_addr, vtable_ptr, fname_idx, name.clone()));

                                if fname_idx == memory::FNAME_CLASS_INDEX || name == "Class" {
                                    println!("\n*** FOUND Class UClass at {:#x} ***", obj_addr);
                                    println!("  VTable: {:#x}, vtable[0]: {:#x}", vtable_ptr, first_func);
                                    println!("  FName index: {} = \"{}\"", fname_idx, name);
                                    found_class = true;
                                }
                            }
                        }

                        println!("  Found {} self-referential objects:", found_self_refs.len());
                        for (addr, vtable, fname_idx, name) in found_self_refs.iter().take(10) {
                            let marker = if *fname_idx == memory::FNAME_CLASS_INDEX { " <-- CLASS!" } else { "" };
                            println!("    {:#x}: vtable={:#x}, fname={} \"{}\"{}",
                                    addr, vtable, fname_idx, name, marker);
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
                    let gnames = memory::discover_gnames(source)
                        .context("Failed to find GNames pool")?;
                    let pool = memory::FNamePool::discover(source)
                        .context("Failed to discover FNamePool")?;
                    let mut fname_reader = memory::FNameReader::new(pool);

                    // Find all UClass instances
                    println!("Finding all UClass instances (ClassPrivate == {:#x})...\n",
                             memory::UCLASS_METACLASS_ADDR);

                    let classes = memory::find_all_uclasses(source, &mut fname_reader)
                        .context("Failed to enumerate UClass instances")?;

                    // Apply filter if provided
                    let filtered: Vec<_> = if let Some(ref pattern) = filter {
                        let pattern_lower = pattern.to_lowercase();
                        classes.iter()
                            .filter(|c| c.name.to_lowercase().contains(&pattern_lower))
                            .collect()
                    } else {
                        classes.iter().collect()
                    };

                    println!("Found {} UClass instances{}\n",
                             filtered.len(),
                             filter.as_ref().map(|f| format!(" matching '{}'", f)).unwrap_or_default());

                    // Show results
                    let show_count = if limit == 0 { filtered.len() } else { limit.min(filtered.len()) };
                    for class in filtered.iter().take(show_count) {
                        println!("  {:#x}: {} (FName {})",
                                class.address, class.name, class.name_index);
                    }

                    if show_count < filtered.len() {
                        println!("\n  ... and {} more (use --limit 0 to show all)", filtered.len() - show_count);
                    }

                    // Show some stats
                    let game_classes: Vec<_> = filtered.iter()
                        .filter(|c| c.name.starts_with("U") || c.name.starts_with("A") || c.name.contains("_"))
                        .collect();
                    let core_classes: Vec<_> = filtered.iter()
                        .filter(|c| !c.name.starts_with("U") && !c.name.starts_with("A") && !c.name.contains("_"))
                        .collect();

                    println!("\nClass categories:");
                    println!("  Game classes (U*/A*/*_*): {}", game_classes.len());
                    println!("  Core/Native classes: {}", core_classes.len());
                }

                MemoryAction::ListObjects { limit, class_filter, name_filter, stats } => {
                    let source = mem_source!();

                    // Discover GNames first (needed for FName resolution)
                    eprintln!("Searching for GNames pool...");
                    let gnames = memory::discover_gnames(source)
                        .context("Failed to discover GNames")?;
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
                    let mut class_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
                    let mut shown = 0usize;

                    let class_filter_lower = class_filter.as_ref().map(|s| s.to_lowercase());
                    let name_filter_lower = name_filter.as_ref().map(|s| s.to_lowercase());

                    // Iterate over all objects
                    for (idx, obj_ptr) in guobj.iter_objects(source) {
                        // Read UObject header
                        let obj_data = match source.read_bytes(obj_ptr, memory::UOBJECT_HEADER_SIZE) {
                            Ok(d) => d,
                            Err(_) => continue,
                        };

                        let class_ptr = byteorder::LE::read_u64(&obj_data[memory::UOBJECT_CLASS_OFFSET..memory::UOBJECT_CLASS_OFFSET + 8]) as usize;
                        let name_idx = byteorder::LE::read_u32(&obj_data[memory::UOBJECT_NAME_OFFSET..memory::UOBJECT_NAME_OFFSET + 4]);

                        // Read object name
                        let obj_name = fname_reader.read_name(source, name_idx).unwrap_or_else(|_| format!("FName_{}", name_idx));

                        // Read class name (need to read the class object's name)
                        let class_name = if class_ptr != 0 {
                            if let Ok(class_data) = source.read_bytes(class_ptr, memory::UOBJECT_HEADER_SIZE) {
                                let class_name_idx = byteorder::LE::read_u32(&class_data[memory::UOBJECT_NAME_OFFSET..memory::UOBJECT_NAME_OFFSET + 4]);
                                fname_reader.read_name(source, class_name_idx).unwrap_or_else(|_| format!("FName_{}", class_name_idx))
                            } else {
                                "Unknown".to_string()
                            }
                        } else {
                            "Null".to_string()
                        };

                        total_valid += 1;
                        *class_counts.entry(class_name.clone()).or_insert(0) += 1;

                        // Apply filters
                        let class_match = class_filter_lower.as_ref()
                            .map(|f| class_name.to_lowercase().contains(f))
                            .unwrap_or(true);
                        let name_match = name_filter_lower.as_ref()
                            .map(|f| obj_name.to_lowercase().contains(f))
                            .unwrap_or(true);

                        if class_match && name_match && !stats && shown < limit {
                            println!("[{}] {:#x}: {} ({})", idx, obj_ptr, obj_name, class_name);
                            shown += 1;
                        }

                        // Progress indicator
                        if total_valid % 50000 == 0 {
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
                        println!("\n... showing first {} matches (use --limit N to see more)", limit);
                    }
                }

                MemoryAction::AnalyzeDump => {
                    let source = mem_source!();

                    // Run comprehensive dump analysis
                    memory::analyze_dump(source)
                        .context("Dump analysis failed")?;
                }

                MemoryAction::DumpUsmap { output } => {
                    let source = mem_source!();
                    // Step 1: Find GNames pool
                    println!("Step 1: Finding GNames pool...");
                    let gnames = memory::discover_gnames(source)
                        .context("Failed to find GNames pool")?;
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
                    let reflection_objects = memory::walk_guobject_array(source, &guobj_array, &mut fname_reader)
                        .context("Failed to walk GUObjectArray")?;

                    // Print summary
                    let class_count = reflection_objects.iter().filter(|o| o.class_name == "Class").count();
                    let struct_count = reflection_objects.iter().filter(|o| o.class_name == "ScriptStruct").count();
                    let enum_count = reflection_objects.iter().filter(|o| o.class_name == "Enum").count();

                    println!("\nFound {} reflection objects:", reflection_objects.len());
                    println!("  {} UClass", class_count);
                    println!("  {} UScriptStruct", struct_count);
                    println!("  {} UEnum", enum_count);

                    // Print some samples
                    println!("\nSample classes:");
                    for obj in reflection_objects.iter().filter(|o| o.class_name == "Class").take(10) {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    println!("\nSample structs:");
                    for obj in reflection_objects.iter().filter(|o| o.class_name == "ScriptStruct").take(10) {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    println!("\nSample enums:");
                    for obj in reflection_objects.iter().filter(|o| o.class_name == "Enum").take(10) {
                        println!("  {}: {} at {:#x}", obj.class_name, obj.name, obj.address);
                    }

                    // Step 4: Extract properties from each struct/class
                    println!("\nStep 4: Extracting properties...");
                    let (structs, enums) = memory::extract_reflection_data(source, &reflection_objects, &mut fname_reader)
                        .context("Failed to extract reflection data")?;

                    // Print some sample properties
                    println!("\nSample struct properties:");
                    for s in structs.iter().filter(|s| !s.properties.is_empty()).take(5) {
                        println!("  {} ({}): {} props, super={:?}",
                                s.name, if s.is_class { "class" } else { "struct" },
                                s.properties.len(), s.super_name);
                        for prop in s.properties.iter().take(3) {
                            println!("    +{:#x} {} : {} ({:?})",
                                    prop.offset, prop.name, prop.type_name,
                                    prop.struct_type.as_ref().or(prop.enum_type.as_ref()));
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
                        usize::from_str_radix(&address[2..], 16)
                            .context("Invalid hex address")?
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
                    let proc = process.as_ref()
                        .context("Write requires a live process (not available in dump mode)")?;

                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16)
                            .context("Invalid hex address")?
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

                MemoryAction::Patch { address, nop, bytes } => {
                    // Patching requires a live process
                    let proc = process.as_ref()
                        .context("Patch requires a live process (not available in dump mode)")?;

                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16)
                            .context("Invalid hex address")?
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
                                        eprintln!("  Valid: legendary, epic, rare, uncommon, common");
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
                                        if let Some(end) = addr_str.find(|c: char| !c.is_ascii_hexdigit())
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

                MemoryAction::ScanString { query, before, after, limit } => {
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
                                    let hex: String = line_bytes.iter()
                                        .map(|b| format!("{:02x}", b))
                                        .collect::<Vec<_>>()
                                        .join(" ");

                                    // ASCII representation
                                    let ascii: String = line_bytes.iter()
                                        .map(|&b| if b >= 32 && b < 127 { b as char } else { '.' })
                                        .collect();

                                    // Mark if this line contains the match
                                    let marker = if ctx_start + j <= addr && addr < ctx_start + j + 16 { " <--" } else { "" };
                                    println!("{:#010x}: {:<48} {}{}", line_addr, hex, ascii, marker);
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
                    println!("Found {} occurrences of '.part_', analyzing...", results.len());

                    let mut parts: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();

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
                                        parts.entry(prefix.to_string())
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
                    println!("Found {} unique part names across {} weapon types",
                             total_unique, parts.len());
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
            let launch_options = format!(
                "LD_PRELOAD={} %command%",
                lib_path.display()
            );

            println!("Add to Steam launch options:\n");
            println!("  {}\n", launch_options);
            println!("Options: BL4_RNG_BIAS=max|high|low|min  BL4_PRELOAD_ALL=1  BL4_PRELOAD_STACKS=1");
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

        Commands::UsmapInfo { path } => {
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
            println!("Compression: {} ({})", compression,
                match compression { 0 => "None", 1 => "Oodle", 2 => "Brotli", 3 => "ZStandard", _ => "Unknown" });
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
                                26 => { // EnumProperty
                                    skip_property_type(r)?; // inner
                                    r.read_u32::<LE>()?; // enum name
                                }
                                9 => { // StructProperty
                                    r.read_u32::<LE>()?; // struct name
                                }
                                8 | 25 | 28 => { // Array/Set/Optional
                                    skip_property_type(r)?; // inner
                                }
                                24 => { // MapProperty
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

        Commands::UsmapSearch { path, pattern, verbose } => {
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
                "Byte", "Bool", "Int", "Float", "Object", "Name", "Delegate", "Double",
                "Array", "Struct", "Str", "Text", "Interface", "MulticastDelegate",
                "WeakObject", "LazyObject", "AssetObject", "SoftObject", "UInt64", "UInt32",
                "UInt16", "Int64", "Int16", "Int8", "Map", "Set", "Enum", "FieldPath",
                "Optional", "Utf8Str", "AnsiStr"
            ];

            fn read_property_type<R: std::io::Read>(r: &mut R, names: &[String], type_names: &[&str]) -> Result<String> {
                let type_id = r.read_u8()? as usize;
                let base_type = type_names.get(type_id).unwrap_or(&"Unknown");

                Ok(match type_id {
                    26 => { // EnumProperty
                        let inner = read_property_type(r, names, type_names)?;
                        let enum_idx = r.read_u32::<LE>()? as usize;
                        let enum_name = names.get(enum_idx).cloned().unwrap_or_default();
                        format!("Enum<{}>", enum_name)
                    }
                    9 => { // StructProperty
                        let struct_idx = r.read_u32::<LE>()? as usize;
                        let struct_name = names.get(struct_idx).cloned().unwrap_or_default();
                        format!("Struct<{}>", struct_name)
                    }
                    8 => { // ArrayProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Array<{}>", inner)
                    }
                    25 => { // SetProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Set<{}>", inner)
                    }
                    28 => { // OptionalProperty
                        let inner = read_property_type(r, names, type_names)?;
                        format!("Optional<{}>", inner)
                    }
                    24 => { // MapProperty
                        let key = read_property_type(r, names, type_names)?;
                        let value = read_property_type(r, names, type_names)?;
                        format!("Map<{}, {}>", key, value)
                    }
                    _ => base_type.to_string()
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
                println!("=== Enums matching '{}' ({}) ===", pattern, found_enums.len());
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
                println!("\n=== Structs matching '{}' ({}) ===", pattern, found_structs.len());
                for (name, super_name, properties) in &found_structs {
                    println!("\n{}{} ({} properties)",
                        name,
                        super_name.as_ref().map(|s| format!(" : {}", s)).unwrap_or_default(),
                        properties.len()
                    );
                    if verbose {
                        for (prop_name, prop_type, array_dim) in properties {
                            let dim_str = if *array_dim > 1 { format!("[{}]", array_dim) } else { String::new() };
                            println!("  {} {}{}", prop_type, prop_name, dim_str);
                        }
                    }
                }
            }

            if found_enums.is_empty() && found_structs.is_empty() {
                println!("No enums or structs found matching '{}'", pattern);
            }
        }
    }

    Ok(())
}
