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
    },
}
