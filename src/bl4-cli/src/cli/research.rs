//! Research command CLI definitions (requires 'research' feature)

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum UsmapCommand {
    /// Show info about a usmap file
    Info {
        /// Path to usmap file
        path: PathBuf,
    },

    /// Search usmap for struct/enum names
    Search {
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
pub enum ExtractCommand {
    /// Extract part pools from the parts database
    #[command(visible_alias = "pp")]
    PartPools {
        /// Input parts database JSON
        #[arg(short, long, default_value = "share/manifest/parts_database.json")]
        input: PathBuf,

        /// Output part pools JSON
        #[arg(short, long, default_value = "share/manifest/part_pools.json")]
        output: PathBuf,
    },

    /// Extract manufacturer data from pak_manifest.json
    #[command(visible_alias = "m")]
    Manufacturers {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/manufacturers.json")]
        output: PathBuf,
    },

    /// Extract weapon type data from pak_manifest.json
    #[command(visible_alias = "wt")]
    WeaponTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/weapon_types.json")]
        output: PathBuf,
    },

    /// Extract gear type data from pak_manifest.json
    #[command(visible_alias = "gt")]
    GearTypes {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/gear_types.json")]
        output: PathBuf,
    },

    /// Extract element types from pak_manifest.json
    #[command(visible_alias = "el")]
    Elements {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/elements.json")]
        output: PathBuf,
    },

    /// Extract rarity tiers from pak_manifest.json
    #[command(visible_alias = "ra")]
    Rarities {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/rarities.json")]
        output: PathBuf,
    },

    /// Extract stat types from pak_manifest.json
    #[command(visible_alias = "st")]
    Stats {
        /// Path to pak_manifest.json
        #[arg(short, long, default_value = "share/manifest/pak_manifest.json")]
        input: PathBuf,

        /// Output file
        #[arg(short, long, default_value = "share/manifest/stats.json")]
        output: PathBuf,
    },

    /// Extract executable from Windows minidump (bypasses Denuvo)
    #[command(name = "minidump-to-exe", visible_alias = "m2e")]
    MinidumpToExe {
        /// Path to Windows minidump file (.dmp)
        input: PathBuf,

        /// Output executable path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Base address of the executable in memory (default: 0x140000000)
        #[arg(short, long, default_value = "0x140000000")]
        base: String,
    },

    /// Check if a file is a valid NCS file
    #[command(name = "ncs-check")]
    NcsCheck {
        /// Path to file to check
        input: PathBuf,
    },

    /// Decompress an NCS file
    #[command(name = "ncs-decompress")]
    NcsDecompress {
        /// Path to NCS file
        input: PathBuf,

        /// Output path (default: input with .decompressed extension)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show information about an NCS file
    #[command(name = "ncs-info")]
    NcsInfo {
        /// Path to NCS file
        input: PathBuf,
    },

    /// Search for NCS files in a directory
    #[command(name = "ncs-find")]
    NcsFind {
        /// Directory to search
        path: PathBuf,

        /// Recursive search
        #[arg(short, long)]
        recursive: bool,
    },

    /// Scan a binary file for embedded NCS chunks
    #[command(name = "ncs-scan")]
    NcsScan {
        /// Path to file to scan (e.g., .pak file)
        input: PathBuf,

        /// Show all matches including invalid ones
        #[arg(short, long)]
        all: bool,
    },

    /// Extract all NCS chunks from a binary file
    #[command(name = "ncs-extract")]
    NcsExtract {
        /// Path to file to scan
        input: PathBuf,

        /// Output directory for extracted chunks
        #[arg(short, long)]
        output: PathBuf,

        /// Also decompress the chunks
        #[arg(short, long)]
        decompress: bool,
    },
}
