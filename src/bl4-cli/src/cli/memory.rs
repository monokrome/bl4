//! Memory command CLI definitions

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PreloadAction {
    /// Show the LD_PRELOAD command to intercept file I/O
    Info,

    /// Run a command with the preload library
    Run {
        /// Directory to save captured file writes
        #[arg(short, long)]
        capture: Option<PathBuf>,

        /// Filter pattern for files to capture (e.g., "*.json,*.ncs")
        #[arg(short, long)]
        filter: Option<String>,

        /// WINEDEBUG settings for Wine/Proton tracing (e.g., "+file", "+relay")
        #[arg(short, long)]
        winedebug: Option<String>,

        /// The command to run
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Watch the preload log file
    Watch {
        /// Path to log file
        #[arg(short, long, default_value = "/tmp/bl4_preload.log")]
        log_file: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum MemoryAction {
    /// Show info about the attached process
    Info,

    /// Discover UE5 structures (GNames, GUObjectArray)
    Discover {
        /// What to discover (gnames, guobjectarray, all)
        #[arg(default_value = "all")]
        target: String,
    },

    /// List UObjects by class name
    Objects {
        /// Class name to filter by (e.g. "RarityWeightData", "ItemPoolDef")
        #[arg(short, long)]
        class: Option<String>,

        /// Maximum number of objects to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Dump usmap mappings file from live process
    DumpUsmap {
        /// Output path for usmap file
        #[arg(short, long, default_value = "BL4.usmap")]
        output: PathBuf,
    },

    /// Look up an FName by index
    Fname {
        /// FName index to look up
        index: u32,

        /// Show raw bytes at the FName entry (for debugging)
        #[arg(long)]
        debug: bool,
    },

    /// Search for an FName by string
    FnameSearch {
        /// String to search for in the FName pool
        query: String,
    },

    /// Search for Class UClass by scanning for self-referential objects
    FindClassUClass,

    /// List all UClass instances in memory (uses discovered metaclass address)
    ListUClasses {
        /// Maximum number of classes to show (0 = all)
        #[arg(short, long, default_value = "50")]
        limit: usize,

        /// Filter by class name pattern (case-insensitive)
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Enumerate UObjects from GUObjectArray
    ListObjects {
        /// Maximum number of objects to show
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Filter by class name pattern (case-insensitive)
        #[arg(short = 'c', long)]
        class_filter: Option<String>,

        /// Filter by object name pattern (case-insensitive)
        #[arg(short = 'n', long)]
        name_filter: Option<String>,

        /// Show statistics only (don't list individual objects)
        #[arg(long)]
        stats: bool,
    },

    /// Analyze dump file: discover UObject layout, FName pool, and UClass metaclass
    AnalyzeDump,

    /// List current inventory items
    ListInventory,

    /// Read a value from game memory
    Read {
        /// Memory address (hex, e.g. 0x7f1234567890)
        address: String,

        /// Number of bytes to read
        #[arg(short, long, default_value = "64")]
        size: usize,
    },

    /// Write bytes to game memory
    Write {
        /// Memory address (hex, e.g. 0x7f1234567890)
        address: String,

        /// Hex bytes to write (e.g. "90 90 90" for NOPs)
        bytes: String,
    },

    /// Scan for a pattern in memory
    Scan {
        /// Hex pattern to search for (e.g. "48 8B 05 ?? ?? ?? ??")
        pattern: String,
    },

    /// Patch a single instruction (replaces with NOPs or custom bytes)
    Patch {
        /// Memory address to patch (hex)
        address: String,

        /// Number of bytes to NOP out
        #[arg(short, long)]
        nop: Option<usize>,

        /// Custom replacement bytes (hex, e.g. "EB 05" for short jump)
        #[arg(short, long)]
        bytes: Option<String>,
    },

    /// Monitor the preload library log file
    Monitor {
        /// Path to log file
        #[arg(short, long, default_value = "/tmp/bl4_preload.log")]
        log_file: PathBuf,

        /// Filter log entries by function name
        #[arg(short, long)]
        filter: Option<String>,

        /// Only show entries from addresses in game code (not libraries)
        #[arg(long)]
        game_only: bool,
    },

    /// LD_PRELOAD library for intercepting file I/O (NCS extraction, etc.)
    Preload {
        #[command(subcommand)]
        action: PreloadAction,
    },

    /// Search for a string in memory and dump context around matches
    ScanString {
        /// String to search for
        query: String,

        /// Bytes to show before the match
        #[arg(short = 'B', long, default_value = "64")]
        before: usize,

        /// Bytes to show after the match
        #[arg(short = 'A', long, default_value = "64")]
        after: usize,

        /// Maximum number of matches to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Extract part definitions from memory dump (searches for XXX_YY.part_* patterns)
    DumpParts {
        /// Output file for parts JSON
        #[arg(short, long, default_value = "parts_dump.json")]
        output: PathBuf,
    },

    /// Build parts database with Category/Index mappings
    BuildPartsDb {
        /// Input parts dump JSON (from dump-parts command)
        #[arg(short, long, default_value = "share/manifest/parts_dump.json")]
        input: PathBuf,

        /// Output parts database JSON
        #[arg(short, long, default_value = "share/manifest/parts_database.json")]
        output: PathBuf,

        /// Part categories mapping JSON (prefix -> category ID)
        #[arg(short, long, default_value = "share/manifest/part_categories.json")]
        categories: PathBuf,
    },

    /// Extract part definitions from UObjects with authoritative Category/Index from SerialIndex
    ExtractParts {
        /// Output file for extracted parts with categories
        #[arg(short, long, default_value = "parts_with_categories.json")]
        output: PathBuf,

        /// Just list all FNames containing .part_ without extracting (for debugging)
        #[arg(long)]
        list_fnames: bool,
    },

    /// Extract raw part data without assumptions (stores actual bytes from memory)
    ExtractPartsRaw {
        /// Output file for raw extraction data
        #[arg(short, long, default_value = "share/ncs/parts_raw.json")]
        output: PathBuf,
    },

    /// Find objects matching a name pattern to discover their class
    FindObjectsByPattern {
        /// Name pattern to search for (e.g. ".part_")
        pattern: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Generate an object map JSON for fast lookups on subsequent runs
    GenerateObjectMap {
        /// Output file for object map JSON
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
