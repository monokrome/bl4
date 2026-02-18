//! Save command CLI definitions

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct SaveArgs {
    /// Path to .sav file
    pub input: PathBuf,

    /// Reveal or clear the fog-of-discovery map
    #[arg(long, value_name = "ACTION")]
    pub map: Option<MapAction>,

    /// Only affect a specific zone (with --map)
    #[arg(long, requires = "map")]
    pub zone: Option<String>,

    /// Steam ID (uses configured default if not provided)
    #[arg(short, long)]
    pub steam_id: Option<String>,

    /// Create backup before modifying
    #[arg(short, long, default_value_t = true)]
    pub backup: bool,

    #[command(subcommand)]
    pub action: Option<SaveAction>,
}

#[derive(Clone, clap::ValueEnum)]
pub enum MapAction {
    Reveal,
    Clear,
}

#[derive(Subcommand)]
pub enum SaveAction {
    /// Decrypt to YAML (stdout or -o file)
    Decrypt {
        /// Path to output YAML file (uses stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Encrypt from YAML (positional file or stdin) to .sav
    Encrypt {
        /// YAML input file (reads stdin if not provided)
        yaml: Option<PathBuf>,
    },

    /// Edit in $EDITOR
    Edit,

    /// Query values
    Get {
        /// YAML path query (e.g. "state.currencies.cash")
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

    /// Set a value
    Set {
        /// YAML path to modify (e.g. "state.currencies.cash")
        path: String,

        /// Value to set (auto-detects numbers vs strings, unless --raw is used)
        value: String,

        /// Treat value as raw YAML (for complex/unknown structures)
        #[arg(short, long)]
        raw: bool,
    },
}
