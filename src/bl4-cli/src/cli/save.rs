//! Save command CLI definitions

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum SaveCommand {
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
        input: PathBuf,

        /// YAML path query (e.g. "state.currencies.cash" or "state.experience[0].level")
        query: Option<String>,

        /// Steam ID for decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

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
        input: PathBuf,

        /// YAML path to modify (e.g. "state.currencies.cash" or "state.experience[0].level")
        path: String,

        /// Value to set (auto-detects numbers vs strings, unless --raw is used)
        value: String,

        /// Steam ID for encryption/decryption (uses configured default if not provided)
        #[arg(short, long)]
        steam_id: Option<String>,

        /// Treat value as raw YAML (for complex/unknown structures)
        #[arg(short, long)]
        raw: bool,

        /// Create backup before modifying
        #[arg(short, long, default_value_t = true)]
        backup: bool,
    },
}
