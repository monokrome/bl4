//! Items database command CLI definitions

use clap::Subcommand;
use std::path::PathBuf;

/// Output format for idb list command
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Csv,
    Json,
}

#[derive(Subcommand)]
pub enum ItemsDbCommand {
    /// Initialize the items database
    Init,

    /// Add an item to the database
    Add {
        /// Item serial code
        serial: String,

        /// Item name
        #[arg(long)]
        name: Option<String>,

        /// Item prefix (e.g., "Ambushing")
        #[arg(long)]
        prefix: Option<String>,

        /// Manufacturer code (e.g., "JAK")
        #[arg(long)]
        manufacturer: Option<String>,

        /// Item type code (e.g., "PS" for pistol)
        #[arg(long)]
        weapon_type: Option<String>,

        /// Rarity (e.g., "Legendary")
        #[arg(long)]
        rarity: Option<String>,

        /// Item level
        #[arg(long)]
        level: Option<i32>,

        /// Element type (e.g., "cryo")
        #[arg(long)]
        element: Option<String>,
    },

    /// List items in the database
    List {
        /// Filter by manufacturer
        #[arg(long)]
        manufacturer: Option<String>,

        /// Filter by item type
        #[arg(long)]
        weapon_type: Option<String>,

        /// Filter by element
        #[arg(long)]
        element: Option<String>,

        /// Filter by rarity
        #[arg(long)]
        rarity: Option<String>,

        /// Output format: table (default), csv, json
        #[arg(long, default_value = "table")]
        format: OutputFormat,

        /// Fields to include (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        fields: Option<Vec<String>>,
    },

    /// Show details for a specific item
    Show {
        /// Item serial
        serial: String,
    },

    /// Add an image attachment to an item
    Attach {
        /// Path to image file
        image: PathBuf,

        /// Item serial
        serial: String,

        /// Attachment name (defaults to filename without extension)
        #[arg(short, long)]
        name: Option<String>,

        /// Mark as POPUP view (item card)
        #[arg(long)]
        popup: bool,

        /// Mark as DETAIL view (3D inspect)
        #[arg(long)]
        detail: bool,
    },

    /// Import items from share/weapons directories
    Import {
        /// Directory to import from (or specific item directory)
        #[arg(default_value = "share/weapons")]
        path: PathBuf,
    },

    /// Export an item to a directory
    Export {
        /// Item serial
        serial: String,

        /// Output directory
        output: PathBuf,
    },

    /// Show database statistics
    Stats,

    /// Show the source salt (generates one if missing)
    Salt,

    /// Set verification status for an item
    Verify {
        /// Item serial
        serial: String,

        /// Verification status (unverified, decoded, screenshot, verified)
        status: String,

        /// Optional verification notes
        #[arg(short, long)]
        notes: Option<String>,
    },

    /// Decode all serials and populate item metadata
    DecodeAll {
        /// Also update items that already have decoded info
        #[arg(long)]
        force: bool,
    },

    /// Decode items and populate item_values table
    Decode {
        /// Specific serial to decode (omit for --all)
        serial: Option<String>,

        /// Decode all items in the database
        #[arg(long, conflicts_with = "serial")]
        all: bool,
    },

    /// Import items from a save file
    ImportSave {
        /// Path to .sav file
        save: PathBuf,

        /// Also decode the imported items
        #[arg(long)]
        decode: bool,

        /// Mark imported items as legal
        #[arg(long)]
        legal: bool,

        /// Source attribution for imported items
        #[arg(long)]
        source: Option<String>,
    },

    /// Mark items as legal (verified not modded)
    MarkLegal {
        /// Item IDs to mark as legal (or "all" to mark all items)
        ids: Vec<String>,
    },

    /// Set the source for items
    SetSource {
        /// Source name (e.g., monokrome, ryechews, community)
        source: String,

        /// Item IDs to update, or use --where for condition
        #[arg(required_unless_present = "where_clause")]
        ids: Vec<String>,

        /// SQL WHERE condition (e.g., "legal = 0" for community data)
        #[arg(long = "where")]
        where_clause: Option<String>,
    },

    /// Merge data from one database to another (like cp)
    Merge {
        /// Source database to merge FROM
        source: PathBuf,

        /// Destination database to merge TO
        dest: PathBuf,
    },

    /// Set a field value with source attribution
    SetValue {
        /// Item serial
        serial: String,

        /// Field name (e.g., level, rarity, manufacturer)
        field: String,

        /// Value to set
        value: String,

        /// Value source: ingame, decoder, community
        #[arg(long, short, default_value = "decoder")]
        source: String,

        /// Source detail (e.g., tool name)
        #[arg(long)]
        source_detail: Option<String>,

        /// Confidence: verified, inferred, uncertain
        #[arg(long, short, default_value = "inferred")]
        confidence: String,
    },

    /// Show all values for a field (from all sources)
    GetValues {
        /// Item serial
        serial: String,

        /// Field name (e.g., level, rarity)
        field: String,
    },

    /// Migrate existing column values to item_values table
    MigrateValues {
        /// Only show what would be migrated, don't actually migrate
        #[arg(long)]
        dry_run: bool,
    },

    /// Publish items to the community server
    Publish {
        /// Server URL
        #[arg(long, short, default_value = "https://items.bl4.dev")]
        server: String,

        /// Only publish a specific item
        #[arg(long)]
        serial: Option<String>,

        /// Also upload attachments (screenshots)
        #[arg(long)]
        attachments: bool,

        /// Only show what would be published, don't actually publish
        #[arg(long)]
        dry_run: bool,
    },

    /// Pull items from a community server and merge into local database
    Pull {
        /// Server URL
        #[arg(long, short, default_value = "https://items.bl4.dev")]
        server: String,

        /// Prefer remote values over local values (overwrite existing)
        #[arg(long)]
        authoritative: bool,

        /// Only show what would be pulled, don't actually pull
        #[arg(long)]
        dry_run: bool,
    },
}
