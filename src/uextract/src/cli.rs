//! CLI argument definitions for uextract

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "uextract")]
#[command(about = "UE5 IoStore extractor with JSON output")]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to Paks directory containing .utoc/.ucas files
    pub input: Option<PathBuf>,

    /// Output directory (default: ./extracted)
    #[arg(short, long, default_value = "extracted")]
    pub output: PathBuf,

    /// Select specific paths to extract (glob patterns, can specify multiple)
    #[arg(short, long)]
    pub select: Vec<String>,

    /// Filter paths containing this string (can specify multiple, OR logic)
    #[arg(short, long)]
    pub filter: Vec<String>,

    /// Case-insensitive filter (can specify multiple, OR logic)
    #[arg(short = 'i', long)]
    pub ifilter: Vec<String>,

    /// Exclude paths matching pattern (can specify multiple)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Output format: json, uasset, or both
    #[arg(long, value_enum, default_value = "both")]
    pub format: OutputFormat,

    /// List matching files without extracting (dry run)
    #[arg(short, long)]
    pub list: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// AES encryption key (base64 or hex) if pak is encrypted
    #[arg(long)]
    pub aes_key: Option<String>,

    /// Path to .usmap file for property schema
    #[arg(long)]
    pub usmap: Option<PathBuf>,

    /// Path to scriptobjects.json for class resolution
    #[arg(long)]
    pub scriptobjects: Option<PathBuf>,

    /// Filter by class name (requires --scriptobjects, can specify multiple, OR logic)
    #[arg(long)]
    pub class_filter: Vec<String>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Extract files from traditional .pak files (not IoStore)
    Pak {
        /// Path to .pak file or directory containing .pak files
        input: PathBuf,
        /// Output directory
        #[arg(short, long, default_value = "extracted")]
        output: PathBuf,
        /// Filter by extension (e.g., "ncs", "uasset")
        #[arg(short, long)]
        extension: Option<String>,
        /// Filter paths containing this string
        #[arg(short, long)]
        filter: Vec<String>,
        /// List files without extracting
        #[arg(short, long)]
        list: bool,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Extract a texture to PNG (for testing)
    Texture {
        /// Path to the .ubulk file
        ubulk: PathBuf,
        /// Width of the texture
        #[arg(short = 'W', long)]
        width: u32,
        /// Height of the texture
        #[arg(short = 'H', long)]
        height: u32,
        /// Output PNG path
        #[arg(short, long)]
        output: PathBuf,
        /// Mip level to extract (0 = highest resolution)
        #[arg(short, long, default_value = "0")]
        mip: usize,
        /// Texture format: bc7 or bc1
        #[arg(short = 'F', long, default_value = "bc7")]
        format: String,
    },
    /// Dump ScriptObjects from global.utoc to JSON (for class resolution)
    ScriptObjects {
        /// Path to Paks directory containing global.utoc
        input: PathBuf,
        /// Output JSON file path
        #[arg(short, long, default_value = "scriptobjects.json")]
        output: PathBuf,
        /// AES encryption key (base64 or hex) if pak is encrypted
        #[arg(long)]
        aes_key: Option<String>,
    },
    /// Find assets by class type (requires scriptobjects.json)
    FindByClass {
        /// Path to Paks directory
        input: PathBuf,
        /// Class name to search for (e.g. "InventoryPartDef")
        class_name: String,
        /// Path to scriptobjects.json
        #[arg(long, default_value = "scriptobjects.json")]
        scriptobjects: PathBuf,
        /// AES encryption key if pak is encrypted
        #[arg(long)]
        aes_key: Option<String>,
        /// Output matching paths to file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List all unique class hashes found in pak files (debug)
    ListClasses {
        /// Path to Paks directory
        input: PathBuf,
        /// Path to scriptobjects.json for resolving class names
        #[arg(long, default_value = "scriptobjects.json")]
        scriptobjects: PathBuf,
        /// AES encryption key if pak is encrypted
        #[arg(long)]
        aes_key: Option<String>,
        /// Max number of sample assets to show per class
        #[arg(long, default_value = "3")]
        samples: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Uasset,
    Both,
}
