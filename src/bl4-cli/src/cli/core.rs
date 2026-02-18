//! Core CLI definitions

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use super::drops::DropsCommand;
use super::idb::ItemsDbCommand;
use super::memory::MemoryAction;
use super::ncs::NcsCommand;
#[cfg(feature = "research")]
use super::research::{ExtractCommand, UsmapCommand};
use super::save::SaveArgs;
use super::serial::SerialCommand;

#[derive(Parser)]
#[command(name = "bl4")]
#[command(about = "Borderlands 4 Save Editor", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Save file operations (decrypt, encrypt, edit, get, set)
    #[command(visible_alias = "s")]
    Save {
        #[command(flatten)]
        args: SaveArgs,
    },

    /// Inspect a save file (decrypt and display info)
    #[command(visible_alias = "i")]
    Inspect {
        /// Path to .sav file
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

        /// Path to parts database (directory of per-category TSVs or single file)
        #[arg(long, default_value = "share/manifest/parts")]
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

    /// NCS (Nexus Config Store) file operations
    #[command(visible_alias = "n")]
    Ncs {
        #[command(subcommand)]
        command: NcsCommand,
    },

    /// Query drop rates and locations for legendary items
    #[command(visible_alias = "d")]
    Drops {
        #[command(subcommand)]
        command: DropsCommand,
    },

    /// Generate manifest files from game data (requires 'research' feature)
    #[cfg(feature = "research")]
    Manifest {
        /// Path to game's Paks directory containing .utoc/.ucas files
        paks: PathBuf,

        /// Path to memory dump file for usmap and parts extraction
        #[arg(short, long)]
        dump: Option<PathBuf>,

        /// Path to .usmap file (generated from dump if not provided)
        #[arg(short = 'm', long)]
        usmap: Option<PathBuf>,

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

        /// Skip memory dump extraction (usmap, parts)
        #[arg(long)]
        skip_memory: bool,
    },
}
