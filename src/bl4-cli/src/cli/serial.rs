//! Serial command CLI definitions

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum SerialCommand {
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
        base: String,

        /// Source serial to take parts from
        source: String,

        /// Part indices to copy from source (e.g. "4,12" for body and barrel)
        parts: String,
    },

    /// Batch decode serials from a file to binary output
    BatchDecode {
        /// Input file with one serial per line
        input: PathBuf,

        /// Output binary file (length-prefixed records)
        output: PathBuf,
    },
}
