mod config;
mod inject;

use anyhow::{bail, Context, Result};
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
    },

    /// Attach to running game and interact with live state
    Inject {
        /// Use preload-based injection (requires game launched with LD_PRELOAD)
        #[arg(long)]
        preload: bool,

        #[command(subcommand)]
        action: InjectAction,
    },

    /// Launch Borderlands 4 with instrumentation
    Launch {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum InjectAction {
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
        }

        Commands::Inject { preload, action } => {
            // Handle commands that don't require process attachment first
            match &action {
                InjectAction::Templates => {
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
                _ => {}
            }

            // Preload mode - communicate with the preload library via environment/signals
            if preload {
                match action {
                    InjectAction::Apply { templates } => {
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
                    InjectAction::Monitor { .. } => {
                        // Monitor works the same in preload mode, fall through
                        // This is handled below in the main match
                    }
                    _ => {
                        bail!("This command is not available in --preload mode. \
                               Remove --preload to use direct memory injection.");
                    }
                }
            }

            // Commands that require process attachment
            let process = inject::Bl4Process::attach()
                .context("Failed to attach to Borderlands 4 process")?;

            match action {
                InjectAction::Templates => unreachable!(),

                InjectAction::Info => {
                    println!("{}", process.info());
                }

                InjectAction::Discover { target } => {
                    match target.to_lowercase().as_str() {
                        "gnames" | "all" => {
                            println!("Searching for GNames pool...");
                            match process.discover_gnames() {
                                Ok(gnames) => {
                                    println!("GNames found at: {:#x}", gnames.address);
                                    println!("\nSample names:");
                                    for (idx, name) in &gnames.sample_names {
                                        println!("  [{}] {}", idx, name);
                                    }

                                    if target == "all" {
                                        println!("\nSearching for GUObjectArray...");
                                        match process.discover_guobject_array(gnames.address) {
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
                            match process.discover_gnames() {
                                Ok(gnames) => {
                                    println!("GNames at: {:#x}", gnames.address);
                                    println!("\nSearching for GUObjectArray...");
                                    match process.discover_guobject_array(gnames.address) {
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
                        _ => {
                            eprintln!("Unknown target: {}. Use 'gnames', 'guobjectarray', or 'all'", target);
                        }
                    }
                }

                InjectAction::Objects { class, limit } => {
                    // First discover GNames
                    let gnames = process.discover_gnames()
                        .context("Failed to find GNames pool")?;

                    println!("GNames at: {:#x}", gnames.address);

                    // For now, we can only search for class names in the FName pool
                    // Full object enumeration requires GUObjectArray
                    if let Some(class_name) = class {
                        println!("Searching for '{}' in FName pool...", class_name);

                        // Search for the class name in memory
                        let pattern = class_name.as_bytes();
                        let results = process.scan_pattern(pattern, &vec![1u8; pattern.len()])?;

                        println!("Found {} occurrences of '{}':", results.len().min(limit), class_name);
                        for (i, addr) in results.iter().take(limit).enumerate() {
                            println!("  {}: {:#x}", i + 1, addr);

                            // Try to read context around the match
                            if let Ok(context) = process.read_bytes(addr.saturating_sub(16), 64) {
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

                InjectAction::DumpUsmap { output } => {
                    // Find UE5 structures needed for usmap generation
                    let _offsets = inject::find_ue5_offsets(&process)
                        .context("Failed to find UE5 structures")?;

                    // TODO: Walk GNames and GUObjectArray to build usmap
                    bail!(
                        "usmap generation not yet implemented. \
                        Output would be written to: {}",
                        output.display()
                    );
                }

                InjectAction::ListInventory => {
                    // TODO: Find player controller, walk inventory array
                    bail!(
                        "Inventory listing not yet implemented. \
                        Need to locate player inventory structures first."
                    );
                }

                InjectAction::Read { address, size } => {
                    // Parse hex address
                    let addr = if address.starts_with("0x") || address.starts_with("0X") {
                        usize::from_str_radix(&address[2..], 16)
                            .context("Invalid hex address")?
                    } else {
                        address.parse::<usize>().context("Invalid address")?
                    };

                    let data = process.read_bytes(addr, size)?;

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

                InjectAction::Write { address, bytes } => {
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
                    let original = process.read_bytes(addr, data.len())?;
                    print!("Original: ");
                    for byte in &original {
                        print!("{:02x} ", byte);
                    }
                    println!();

                    // Write the new bytes
                    process.write_bytes(addr, &data)?;
                    println!("Write successful!");
                }

                InjectAction::Scan { pattern } => {
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

                    let results = process.scan_pattern(&bytes, &mask)?;

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

                InjectAction::Patch { address, nop, bytes } => {
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
                    let original = process.read_bytes(addr, patch_bytes.len())?;
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
                    process.write_bytes(addr, &patch_bytes)?;
                    println!("Patch applied!");
                }

                InjectAction::Apply { templates } => {
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

                InjectAction::Monitor {
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
    }

    Ok(())
}
