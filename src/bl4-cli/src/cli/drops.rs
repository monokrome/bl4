//! CLI definitions for the drops command

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DropsCommand {
    /// Find where an item drops (sorted by drop rate, highest first)
    Find {
        /// Item name to search for (e.g., "Hellwalker", "PlasmaCoil", "Guardian Angel")
        #[arg(required = true, num_args = 1..)]
        item: Vec<String>,

        /// Path to drops manifest
        #[arg(long, default_value = "share/manifest/drops.json")]
        manifest: PathBuf,
    },

    /// List all items from a specific source (boss, Black Market, mission, etc.)
    Source {
        /// Source name to look up (e.g., "Timekeeper", "Black Market", "Fish Collector")
        #[arg(required = true, num_args = 1..)]
        name: Vec<String>,

        /// Path to drops manifest
        #[arg(long, default_value = "share/manifest/drops.json")]
        manifest: PathBuf,
    },

    /// List all known drop sources or items
    List {
        /// List sources instead of items (includes bosses, Black Market, missions, etc.)
        #[arg(long)]
        sources: bool,

        /// Path to drops manifest
        #[arg(long, default_value = "share/manifest/drops.json")]
        manifest: PathBuf,
    },

    /// Generate drops manifest from NCS data
    Generate {
        /// Path to NCS data directory
        ncs_dir: PathBuf,

        /// Output path for drops.json
        #[arg(short, long, default_value = "share/manifest/drops.json")]
        output: PathBuf,
    },
}
