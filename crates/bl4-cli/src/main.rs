mod config;

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
                eprintln!("Decrypting {} ...", path.display());
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
                eprintln!("Decrypted to {}", path.display());
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
                eprintln!("Encrypting {} ...", path.display());
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
                eprintln!("Encrypted to {}", path.display());
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
                let backup_created =
                    bl4::smart_backup(&input).context("Failed to manage backup")?;

                if backup_created {
                    let (backup_path, _) = bl4::backup::backup_paths(&input);
                    eprintln!("Created backup: {}", backup_path.display());
                } else {
                    eprintln!("Backup exists (preserving original)");
                }
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
            eprintln!("Opening {} in {}...", abs_temp_path.display(), editor_str);
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
                let (_, metadata_path) = bl4::backup::backup_paths(&input);
                bl4::update_after_edit(&input, &metadata_path)
                    .context("Failed to update backup metadata")?;
            }

            eprintln!("Saved changes to {}", input.display());
        }

        Commands::Inspect {
            input,
            steam_id,
            full,
        } => {
            let steam_id = get_steam_id(steam_id)?;
            eprintln!("Inspecting {} ...\n", input.display());

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
                let backup_created =
                    bl4::smart_backup(&input).context("Failed to manage backup")?;

                if backup_created {
                    let (backup_path, _) = bl4::backup::backup_paths(&input);
                    eprintln!("Created backup: {}", backup_path.display());
                } else {
                    eprintln!("Backup exists (preserving original)");
                }
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
                let (_, metadata_path) = bl4::backup::backup_paths(&input);
                bl4::update_after_edit(&input, &metadata_path)
                    .context("Failed to update backup metadata")?;
            }

            eprintln!("Saved changes to {}", input.display());
        }

        Commands::Decode { serial, verbose } => {
            let item = bl4::ItemSerial::decode(&serial).context("Failed to decode serial")?;

            println!("Serial: {}", item.original);
            println!("Item type: {}", item.item_type);
            println!("Decoded bytes: {}", item.raw_bytes.len());
            println!("Hex: {}", item.hex_dump());

            if verbose {
                println!("\n{}", item.detailed_dump());
            }
        }
    }

    Ok(())
}
