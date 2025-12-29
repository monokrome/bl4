//! NCS subcommand definitions

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum NcsCommand {
    /// Scan a directory of decompressed NCS files and list types
    Scan {
        /// Directory containing decompressed .bin files
        path: PathBuf,

        /// Show only files matching this type
        #[arg(short = 't', long)]
        filter_type: Option<String>,

        /// Show detailed info for each file
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show content of a specific NCS file
    Show {
        /// Path to decompressed NCS file
        path: PathBuf,

        /// Show all strings (not just entry names)
        #[arg(short, long)]
        all_strings: bool,

        /// Show raw hex dump
        #[arg(long)]
        hex: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search for NCS files containing a pattern
    Search {
        /// Directory to search
        path: PathBuf,

        /// Pattern to search for (case-insensitive)
        pattern: String,

        /// Search in all strings, not just entry names
        #[arg(short, long)]
        all: bool,

        /// Maximum results to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },

    /// Extract specific data types from NCS files
    Extract {
        /// Directory containing decompressed NCS files
        path: PathBuf,

        /// Type to extract (manufacturer, rarity, itempoollist, etc.)
        #[arg(short = 't', long)]
        extract_type: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show statistics about NCS files
    Stats {
        /// Directory containing decompressed NCS files
        path: PathBuf,

        /// Show format code breakdown
        #[arg(short, long)]
        formats: bool,
    },

    /// Decompress NCS data from a pak file or raw NCS
    Decompress {
        /// Input file (pak file or raw NCS)
        input: PathBuf,

        /// Output directory for decompressed files
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Offset in file (for pak files)
        #[arg(long)]
        offset: Option<usize>,

        /// Path to Oodle DLL for native decompression (Windows only)
        ///
        /// Load the official Oodle DLL (e.g., oo2core_9_win64.dll) for full
        /// compatibility. Only works on Windows.
        #[cfg(target_os = "windows")]
        #[arg(long, value_name = "DLL_PATH")]
        oodle_dll: Option<PathBuf>,

        /// External command for Oodle decompression (cross-platform)
        ///
        /// Execute an external program for decompression. The command is invoked as:
        ///   <command> decompress <decompressed_size>
        /// Compressed data is sent to stdin, decompressed data is read from stdout.
        #[arg(long, value_name = "COMMAND")]
        oodle_exec: Option<String>,
    },
}
