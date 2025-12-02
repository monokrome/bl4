use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use retoc::{AesKey, Config, FGuid, iostore, zen::FZenPackageHeader, container_header::EIoContainerHeaderVersion, EIoStoreTocVersion};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use usmap::Usmap;

pub mod texture;

#[derive(Parser, Debug)]
#[command(name = "uextract")]
#[command(about = "UE5 IoStore extractor with JSON output")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to Paks directory containing .utoc/.ucas files
    input: Option<PathBuf>,

    /// Output directory (default: ./extracted)
    #[arg(short, long, default_value = "extracted")]
    output: PathBuf,

    /// Select specific paths to extract (glob patterns, can specify multiple)
    #[arg(short, long)]
    select: Vec<String>,

    /// Filter paths containing this string (can specify multiple, OR logic)
    #[arg(short, long)]
    filter: Vec<String>,

    /// Case-insensitive filter (can specify multiple, OR logic)
    #[arg(short = 'i', long)]
    ifilter: Vec<String>,

    /// Exclude paths matching pattern (can specify multiple)
    #[arg(short, long)]
    exclude: Vec<String>,

    /// Output format: json, uasset, or both
    #[arg(long, value_enum, default_value = "both")]
    format: OutputFormat,

    /// List matching files without extracting (dry run)
    #[arg(short, long)]
    list: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// AES encryption key (base64 or hex) if pak is encrypted
    #[arg(long)]
    aes_key: Option<String>,

    /// Path to .usmap file for property schema
    #[arg(long)]
    usmap: Option<PathBuf>,

    /// Path to scriptobjects.json for class resolution
    #[arg(long)]
    scriptobjects: Option<PathBuf>,

    /// Filter by class name (requires --scriptobjects, can specify multiple, OR logic)
    #[arg(long)]
    class_filter: Vec<String>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
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
enum OutputFormat {
    Json,
    Uasset,
    Both,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle subcommands
    if let Some(command) = args.command {
        return match command {
            Commands::Texture { ubulk, width, height, output, mip, format } => {
                extract_texture_cmd(&ubulk, width, height, &output, mip, &format)
            }
            Commands::ScriptObjects { input, output, aes_key } => {
                extract_script_objects(&input, &output, aes_key.as_deref())
            }
            Commands::FindByClass { input, class_name, scriptobjects, aes_key, output } => {
                find_assets_by_class(&input, &class_name, &scriptobjects, aes_key.as_deref(), output.as_deref())
            }
            Commands::ListClasses { input, scriptobjects, aes_key, samples } => {
                list_classes(&input, &scriptobjects, aes_key.as_deref(), samples)
            }
        };
    }

    // Need input for main extraction mode
    let input = args.input.clone().context("Input path is required for extraction")?;

    // Build retoc config
    let mut aes_keys = HashMap::new();
    if let Some(key) = &args.aes_key {
        let parsed_key: AesKey = key.parse()
            .context("Invalid AES key format (use hex or base64)")?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    // Load scriptobjects if provided (for class resolution)
    let _class_lookup: Option<Arc<HashMap<String, String>>> = if let Some(so_path) = &args.scriptobjects {
        let so_data = std::fs::read_to_string(so_path)
            .with_context(|| format!("Failed to read scriptobjects file {:?}", so_path))?;
        let so_json: serde_json::Value = serde_json::from_str(&so_data)
            .with_context(|| format!("Failed to parse scriptobjects file {:?}", so_path))?;

        let hash_to_path = so_json.get("hash_to_path")
            .and_then(|v| v.as_object())
            .context("scriptobjects.json missing hash_to_path")?;

        let lookup: HashMap<String, String> = hash_to_path.iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
            .collect();

        eprintln!("Loaded {} class hashes from scriptobjects", lookup.len());
        Some(Arc::new(lookup))
    } else {
        if !args.class_filter.is_empty() {
            eprintln!("Warning: --class-filter requires --scriptobjects to be set");
        }
        None
    };

    // Load usmap if provided
    let usmap_schema: Option<Arc<Usmap>> = if let Some(usmap_path) = &args.usmap {
        let usmap_data = std::fs::read(usmap_path)
            .with_context(|| format!("Failed to read usmap file {:?}", usmap_path))?;
        let usmap = Usmap::read(&mut Cursor::new(usmap_data))
            .with_context(|| format!("Failed to parse usmap file {:?}", usmap_path))?;
        eprintln!("Loaded usmap with {} structs, {} enums", usmap.structs.len(), usmap.enums.len());

        // In verbose mode, show some struct examples
        if args.verbose {
            eprintln!("First 10 struct names:");
            for s in usmap.structs.iter().take(10) {
                eprintln!("  - {} (super: {:?})", s.name, s.super_struct);
            }
        }

        Some(Arc::new(usmap))
    } else {
        None
    };

    // Open IoStore
    let store = iostore::open(&input, config.clone())
        .with_context(|| format!("Failed to open {:?}", input))?;

    if args.verbose {
        eprintln!("Opened IoStore: {}", store.container_name());
        store.print_info(0);
    }

    // Get container versions for Zen parsing
    let toc_version = store.container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store.container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    // Collect matching entries (only .uasset files for JSON)
    let entries: Vec<_> = store
        .chunks()
        .filter_map(|chunk| {
            chunk.path().map(|path| (chunk, path))
        })
        .filter(|(_, path)| matches_filters(path, &args))
        .collect();

    if args.verbose || args.list {
        eprintln!("Found {} matching entries", entries.len());
    }

    if args.list {
        for (_, path) in &entries {
            println!("{}", path);
        }
        return Ok(());
    }

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    // Progress bar
    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Extract entries (parallel)
    let results: Vec<_> = entries
        .par_iter()
        .map(|(chunk, path)| {
            let result = extract_entry(chunk, path, &args, toc_version, container_header_version, usmap_schema.as_ref());
            pb.inc(1);
            if let Err(ref e) = result {
                eprintln!("Error {}: {:?}", path, e);
            }
            result
        })
        .collect();

    pb.finish_with_message("Done");

    // Summary
    let success = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len() - success;
    eprintln!("Extracted: {}, Failed: {}", success, failed);

    Ok(())
}

fn matches_filters(path: &str, args: &Args) -> bool {
    // Check excludes first
    for pattern in &args.exclude {
        if glob_match::glob_match(pattern, path) {
            return false;
        }
    }

    // If no positive filters, match all
    if args.select.is_empty() && args.filter.is_empty() && args.ifilter.is_empty() {
        return true;
    }

    // Check select patterns (glob)
    for pattern in &args.select {
        if glob_match::glob_match(pattern, path) {
            return true;
        }
    }

    // Check filter (substring)
    for f in &args.filter {
        if path.contains(f) {
            return true;
        }
    }

    // Check ifilter (case-insensitive substring)
    let path_lower = path.to_lowercase();
    for f in &args.ifilter {
        if path_lower.contains(&f.to_lowercase()) {
            return true;
        }
    }

    false
}

fn extract_entry(
    chunk: &iostore::ChunkInfo,
    path: &str,
    args: &Args,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
    usmap_schema: Option<&Arc<Usmap>>,
) -> Result<()> {
    let data = chunk.read()?;

    // Normalize path - remove leading ../ components
    let mut clean_path = path;
    while clean_path.starts_with("../") {
        clean_path = &clean_path[3..];
    }
    while clean_path.starts_with("./") {
        clean_path = &clean_path[2..];
    }
    let clean_path = clean_path.trim_start_matches('/');

    // Output raw asset data
    if args.format == OutputFormat::Uasset || args.format == OutputFormat::Both {
        let out_path = args.output.join(clean_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, &data)?;
    }

    // Output JSON (for .uasset files - parse Zen format directly)
    if (args.format == OutputFormat::Json || args.format == OutputFormat::Both)
        && path.ends_with(".uasset")
    {
        match parse_zen_to_json(&data, path, toc_version, container_header_version, usmap_schema, args.verbose) {
            Ok(json) => {
                let json_path = args.output.join(format!("{}.json", clean_path));
                if let Some(parent) = json_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&json_path, json)?;
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {:?}", path, e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Zen Format Parsing → JSON
// ============================================================================

#[derive(Debug, Serialize)]
struct ZenAssetInfo {
    path: String,
    package_name: String,
    package_flags: u32,
    is_unversioned: bool,
    name_count: usize,
    import_count: usize,
    export_count: usize,
    names: Vec<String>,
    imports: Vec<ZenImportInfo>,
    exports: Vec<ZenExportInfo>,
}

#[derive(Debug, Serialize)]
struct ZenImportInfo {
    index: usize,
    type_name: String,
}

#[derive(Debug, Serialize)]
struct ZenExportInfo {
    index: usize,
    object_name: String,
    class_index: String,
    super_index: String,
    template_index: String,
    outer_index: String,
    public_export_hash: u64,
    cooked_serial_offset: u64,
    cooked_serial_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Vec<ParsedProperty>>,
}

#[derive(Debug, Serialize)]
struct ParsedProperty {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    float_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    int_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_value: Option<String>,
}

/// Parse property values from export serialized data
/// UE5 unversioned properties store numeric values at the end of the export data
fn parse_export_properties(
    data: &[u8],
    offset: usize,
    size: usize,
    names: &[String],
) -> Option<Vec<ParsedProperty>> {
    if offset >= data.len() || size == 0 {
        return None;
    }

    let end = (offset + size).min(data.len());
    let export_data = &data[offset..end];

    if export_data.len() < 8 {
        return None;
    }

    let mut properties = Vec::new();

    // Check if this is a DoubleProperty or FloatProperty asset
    let has_double = names.iter().any(|n| n == "DoubleProperty");
    let has_float = names.iter().any(|n| n == "FloatProperty");

    // Find property names with value indicators (sorted by their index)
    let mut value_props: Vec<&String> = names
        .iter()
        .filter(|n| {
            // Match stat properties with index_GUID pattern
            let parts: Vec<&str> = n.split('_').collect();
            parts.len() >= 3 &&
            parts.iter().any(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_hexdigit())) &&
            !n.starts_with('/') && !n.contains("Property")
        })
        .collect();

    // Sort by the index number in the name (e.g., "Damage_Scale_14_..." -> 14)
    value_props.sort_by_key(|n| {
        n.split('_')
            .filter_map(|s| s.parse::<u32>().ok())
            .next()
            .unwrap_or(9999)
    });

    if value_props.is_empty() {
        return None;
    }

    // Try doubles first (8 bytes), then floats (4 bytes)
    if has_double {
        let scan_start = export_data.len().saturating_sub(value_props.len() * 8 + 32);
        let mut double_values: Vec<f64> = Vec::new();

        for i in (scan_start..export_data.len().saturating_sub(7)).step_by(8) {
            if let Ok(bytes) = export_data[i..i+8].try_into() {
                let bytes: [u8; 8] = bytes;
                let val = f64::from_le_bytes(bytes);
                if val.is_finite() && (val == 0.0 || (val.abs() >= 0.0001 && val.abs() <= 1_000_000.0)) {
                    double_values.push(val);
                }
            }
        }

        let num_to_map = value_props.len().min(double_values.len());
        let value_start = double_values.len().saturating_sub(num_to_map);

        for (i, prop_name) in value_props.iter().take(num_to_map).enumerate() {
            let val_idx = value_start + i;
            if val_idx < double_values.len() {
                let parts: Vec<&str> = prop_name.split('_').collect();
                let base_name = if parts.len() >= 2 {
                    format!("{}_{}", parts[0], parts[1])
                } else {
                    prop_name.to_string()
                };

                properties.push(ParsedProperty {
                    name: base_name,
                    value_type: Some("Double".to_string()),
                    float_value: Some(double_values[val_idx]),
                    int_value: None,
                    string_value: None,
                });
            }
        }
    }

    // Try floats (4 bytes) if we have FloatProperty and no doubles found
    if has_float && properties.is_empty() {
        let scan_start = export_data.len().saturating_sub(value_props.len() * 4 + 16);
        let mut float_values: Vec<f32> = Vec::new();

        for i in (scan_start..export_data.len().saturating_sub(3)).step_by(4) {
            if let Ok(bytes) = export_data[i..i+4].try_into() {
                let bytes: [u8; 4] = bytes;
                let val = f32::from_le_bytes(bytes);
                if val.is_finite() && (val == 0.0 || (val.abs() >= 0.0001 && val.abs() <= 1_000_000.0)) {
                    float_values.push(val);
                }
            }
        }

        let num_to_map = value_props.len().min(float_values.len());
        let value_start = float_values.len().saturating_sub(num_to_map);

        for (i, prop_name) in value_props.iter().take(num_to_map).enumerate() {
            let val_idx = value_start + i;
            if val_idx < float_values.len() {
                let parts: Vec<&str> = prop_name.split('_').collect();
                let base_name = if parts.len() >= 2 {
                    format!("{}_{}", parts[0], parts[1])
                } else {
                    prop_name.to_string()
                };

                properties.push(ParsedProperty {
                    name: base_name,
                    value_type: Some("Float".to_string()),
                    float_value: Some(float_values[val_idx] as f64),
                    int_value: None,
                    string_value: None,
                });
            }
        }
    }

    if properties.is_empty() {
        None
    } else {
        Some(properties)
    }
}

// ============================================================================
// UE5 Unversioned Property Parsing
// ============================================================================

/// FFragment from UE5 unversioned header - packed into 16 bits
#[derive(Debug, Clone, Default)]
struct FFragment {
    skip_num: u8,        // 7 bits: properties to skip
    has_any_zeroes: bool, // 1 bit: zero mask follows
    is_last: bool,       // 1 bit: final fragment marker
    value_count: u8,     // 7 bits: property count in this fragment
}

impl FFragment {
    fn unpack(packed: u16) -> Self {
        Self {
            skip_num: (packed & 0x7f) as u8,
            has_any_zeroes: (packed & 0x80) != 0,
            is_last: (packed & 0x100) != 0,
            value_count: (packed >> 9) as u8,
        }
    }
}

/// Parse the FUnversionedHeader from export data
/// Returns (fragments, zero_mask, bytes_consumed)
fn parse_unversioned_header(data: &[u8]) -> Option<(Vec<FFragment>, Vec<u8>, usize)> {
    if data.len() < 2 {
        return None;
    }

    let mut pos = 0;
    let mut fragments = Vec::new();
    let mut total_zero_bits = 0;

    // Read fragments until we hit the last one
    loop {
        if pos + 2 > data.len() {
            return None;
        }

        let packed = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;

        let fragment = FFragment::unpack(packed);

        if fragment.has_any_zeroes {
            total_zero_bits += fragment.value_count as usize;
        }

        let is_last = fragment.is_last;
        fragments.push(fragment);

        if is_last {
            break;
        }
    }

    // Read zero mask if any fragments have zeroes
    let zero_mask = if total_zero_bits > 0 {
        // Zero mask is bit-packed, round up to bytes
        let num_bytes = (total_zero_bits + 7) / 8;
        if pos + num_bytes > data.len() {
            return None;
        }
        let mask = data[pos..pos + num_bytes].to_vec();
        pos += num_bytes;
        mask
    } else {
        Vec::new()
    };

    Some((fragments, zero_mask, pos))
}

/// Get property indices that should be serialized based on fragments and zero mask
fn get_serialized_property_indices(fragments: &[FFragment], zero_mask: &[u8]) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut current_index = 0;
    let mut zero_bit_index = 0;

    for fragment in fragments {
        // Skip properties
        current_index += fragment.skip_num as usize;

        // Process value_count properties
        for _ in 0..fragment.value_count {
            // Check if this property is zeroed (in the zero mask)
            let is_zeroed = if fragment.has_any_zeroes && !zero_mask.is_empty() {
                let byte_idx = zero_bit_index / 8;
                let bit_idx = zero_bit_index % 8;
                zero_bit_index += 1;

                if byte_idx < zero_mask.len() {
                    (zero_mask[byte_idx] & (1 << bit_idx)) != 0
                } else {
                    false
                }
            } else {
                false
            };

            if !is_zeroed {
                indices.push(current_index);
            }
            current_index += 1;
        }
    }

    indices
}

/// Get all properties for a struct, including inherited properties from super_struct
fn get_all_struct_properties<'a>(
    struct_name: &str,
    struct_lookup: &'a HashMap<String, &usmap::Struct>,
) -> Vec<&'a usmap::Property> {
    let mut all_props = Vec::new();
    let mut current_name = Some(struct_name.to_string());

    // Walk up the inheritance chain
    while let Some(name) = current_name {
        if let Some(struct_def) = struct_lookup.get(&name) {
            // Properties are added in reverse order (super first)
            // We'll reverse at the end
            for prop in struct_def.properties.iter().rev() {
                all_props.push(prop);
            }
            current_name = struct_def.super_struct.clone();
        } else {
            break;
        }
    }

    // Reverse to get proper order (super -> derived)
    all_props.reverse();

    // Sort by property index to ensure correct ordering
    all_props.sort_by_key(|p| p.index);
    all_props
}

/// Calculate the serialized size of a property value
fn get_property_size(inner: &usmap::PropertyInner) -> Option<usize> {
    match inner {
        usmap::PropertyInner::Bool => Some(0), // Bools are encoded in the header
        usmap::PropertyInner::Byte => Some(1),
        usmap::PropertyInner::Int8 => Some(1),
        usmap::PropertyInner::Int16 => Some(2),
        usmap::PropertyInner::UInt16 => Some(2),
        usmap::PropertyInner::Int => Some(4),
        usmap::PropertyInner::UInt32 => Some(4),
        usmap::PropertyInner::Int64 => Some(8),
        usmap::PropertyInner::UInt64 => Some(8),
        usmap::PropertyInner::Float => Some(4),
        usmap::PropertyInner::Double => Some(8),
        // Complex types - we can't determine size without more context
        _ => None,
    }
}

/// Parse a single property value from data
fn parse_property_value(
    data: &[u8],
    pos: usize,
    inner: &usmap::PropertyInner,
) -> Option<(ParsedProperty, usize)> {
    let size = get_property_size(inner)?;

    if pos + size > data.len() {
        return None;
    }

    let slice = &data[pos..pos + size];

    let (value_type, float_value, int_value) = match inner {
        usmap::PropertyInner::Bool => {
            // Bools are handled separately in zero mask
            return None;
        }
        usmap::PropertyInner::Byte => {
            ("Byte", None, Some(slice[0] as i64))
        }
        usmap::PropertyInner::Int8 => {
            ("Int8", None, Some(slice[0] as i8 as i64))
        }
        usmap::PropertyInner::Int16 => {
            let val = i16::from_le_bytes([slice[0], slice[1]]);
            ("Int16", None, Some(val as i64))
        }
        usmap::PropertyInner::UInt16 => {
            let val = u16::from_le_bytes([slice[0], slice[1]]);
            ("UInt16", None, Some(val as i64))
        }
        usmap::PropertyInner::Int => {
            let val = i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
            ("Int", None, Some(val as i64))
        }
        usmap::PropertyInner::UInt32 => {
            let val = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
            ("UInt32", None, Some(val as i64))
        }
        usmap::PropertyInner::Int64 => {
            let val = i64::from_le_bytes(slice.try_into().ok()?);
            ("Int64", None, Some(val))
        }
        usmap::PropertyInner::UInt64 => {
            let val = u64::from_le_bytes(slice.try_into().ok()?);
            ("UInt64", None, Some(val as i64))
        }
        usmap::PropertyInner::Float => {
            let val = f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
            ("Float", Some(val as f64), None)
        }
        usmap::PropertyInner::Double => {
            let val = f64::from_le_bytes(slice.try_into().ok()?);
            ("Double", Some(val), None)
        }
        _ => return None,
    };

    Some((
        ParsedProperty {
            name: String::new(), // Will be filled in by caller
            value_type: Some(value_type.to_string()),
            float_value,
            int_value,
            string_value: None,
        },
        size,
    ))
}

/// Extract property info from asset name table
/// Property names in DataTables follow the pattern: PropertyName_Index_GUID
fn extract_property_info_from_names(names: &[String]) -> Vec<(String, u32, String)> {
    let mut props = Vec::new();

    for name in names {
        // Skip internal names
        if name.starts_with('/') || name.contains("Property") || name == "None" {
            continue;
        }

        // Parse pattern: PropertyName_Index_GUID
        let parts: Vec<&str> = name.split('_').collect();
        if parts.len() >= 3 {
            // Find the GUID part (32 hex chars)
            if let Some(guid_idx) = parts.iter().position(|p| p.len() == 32 && p.chars().all(|c| c.is_ascii_hexdigit())) {
                // The index should be the part before GUID
                if guid_idx >= 2 {
                    if let Ok(index) = parts[guid_idx - 1].parse::<u32>() {
                        // Property name is everything before the index
                        let prop_name = parts[..guid_idx - 1].join("_");
                        let guid = parts[guid_idx].to_string();
                        props.push((prop_name, index, guid));
                    }
                }
            }
        }
    }

    // Sort by index
    props.sort_by_key(|(_, idx, _)| *idx);
    props
}

/// Parse properties using usmap schema for proper field names and types
fn parse_export_properties_with_schema(
    data: &[u8],
    offset: usize,
    size: usize,
    names: &[String],
    struct_lookup: &HashMap<String, &usmap::Struct>,
    _verbose: bool,
) -> Option<Vec<ParsedProperty>> {
    if offset >= data.len() || size == 0 {
        return None;
    }

    let end = (offset + size).min(data.len());
    let export_data = &data[offset..end];

    if export_data.len() < 2 {
        return None;
    }

    // Check if this has DoubleProperty or FloatProperty (for type detection)
    let has_double = names.iter().any(|n| n == "DoubleProperty");
    let has_float = names.iter().any(|n| n == "FloatProperty");

    // Try to find the struct type from usmap
    let struct_type = names.iter()
        .find(|n| struct_lookup.contains_key(*n))
        .cloned()
        .or_else(|| {
            names.iter()
                .find(|n| {
                    let prefixed = format!("F{}", n);
                    struct_lookup.contains_key(&prefixed)
                })
                .map(|n| format!("F{}", n))
        });

    // If we have a usmap struct definition, use it
    if let Some(ref type_name) = struct_type {
        if struct_lookup.contains_key(type_name) {
            // Parse unversioned header
            if let Some((fragments, zero_mask, header_size)) = parse_unversioned_header(export_data) {
                let serialized_indices = get_serialized_property_indices(&fragments, &zero_mask);

                if !serialized_indices.is_empty() {
                    let all_props = get_all_struct_properties(type_name, struct_lookup);
                    let index_to_prop: HashMap<usize, &usmap::Property> = all_props
                        .iter()
                        .map(|p| (p.index as usize, *p))
                        .collect();

                    let mut properties = Vec::new();
                    let mut pos = header_size;

                    for prop_index in serialized_indices {
                        if let Some(prop_def) = index_to_prop.get(&prop_index) {
                            if let Some((mut parsed, consumed)) = parse_property_value(export_data, pos, &prop_def.inner) {
                                parsed.name = prop_def.name.clone();
                                properties.push(parsed);
                                pos += consumed;
                            } else {
                                break;
                            }
                        }
                    }

                    if !properties.is_empty() {
                        return Some(properties);
                    }
                }
            }
        }
    }

    // For DataTables and other assets without usmap struct:
    // Extract property info from name table
    let prop_info = extract_property_info_from_names(names);

    if prop_info.is_empty() {
        return parse_export_properties(data, offset, size, names);
    }

    // Try to parse using unversioned header with name-table derived properties
    if let Some((fragments, zero_mask, header_size)) = parse_unversioned_header(export_data) {
        let serialized_indices = get_serialized_property_indices(&fragments, &zero_mask);

        if !serialized_indices.is_empty() {
            // Build index -> prop_info mapping
            let index_to_name: HashMap<usize, &str> = prop_info
                .iter()
                .map(|(name, idx, _)| (*idx as usize, name.as_str()))
                .collect();

            let mut properties = Vec::new();
            let mut pos = header_size;

            // Determine value size based on property types in names
            let value_size = if has_double { 8 } else if has_float { 4 } else { 8 };

            for prop_index in serialized_indices {
                if pos + value_size > export_data.len() {
                    break;
                }

                let prop_name = index_to_name
                    .get(&prop_index)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("prop_{}", prop_index));

                if value_size == 8 {
                    let val = f64::from_le_bytes(export_data[pos..pos+8].try_into().ok()?);
                    if val.is_finite() {
                        properties.push(ParsedProperty {
                            name: prop_name,
                            value_type: Some("Double".to_string()),
                            float_value: Some(val),
                            int_value: None,
                            string_value: None,
                        });
                    }
                } else {
                    let val = f32::from_le_bytes(export_data[pos..pos+4].try_into().ok()?);
                    if val.is_finite() {
                        properties.push(ParsedProperty {
                            name: prop_name,
                            value_type: Some("Float".to_string()),
                            float_value: Some(val as f64),
                            int_value: None,
                            string_value: None,
                        });
                    }
                }

                pos += value_size;
            }

            if !properties.is_empty() {
                return Some(properties);
            }
        }
    }

    // Fall back to heuristic parsing
    parse_export_properties(data, offset, size, names)
}

fn parse_zen_to_json(
    data: &[u8],
    path: &str,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
    usmap_schema: Option<&Arc<Usmap>>,
    verbose: bool,
) -> Result<String> {
    let mut cursor = Cursor::new(data);

    // Parse Zen package header using retoc
    let header = FZenPackageHeader::deserialize(
        &mut cursor,
        None, // store_entry - not available for individual chunks
        toc_version,
        container_header_version,
        None, // package_version_override
    )?;

    // Get the position after header - this is where export data starts
    let header_end = cursor.position() as usize;

    // Extract names from name map
    let names: Vec<String> = header.name_map.copy_raw_names();

    // Build struct lookup from usmap if available
    let struct_lookup: HashMap<String, &usmap::Struct> = usmap_schema
        .map(|schema| {
            schema.structs.iter()
                .map(|s| (s.name.clone(), s))
                .collect()
        })
        .unwrap_or_default();

    // Extract imports
    let imports: Vec<ZenImportInfo> = header.import_map
        .iter()
        .enumerate()
        .map(|(i, import)| {
            ZenImportInfo {
                index: i,
                type_name: format!("{:?}", import), // FPackageObjectIndex debug representation
            }
        })
        .collect();

    // Extract exports with property data
    let exports: Vec<ZenExportInfo> = header.export_map
        .iter()
        .enumerate()
        .map(|(i, export)| {
            // cooked_serial_offset is relative to start of export data section
            // which comes after the header
            let absolute_offset = header_end + export.cooked_serial_offset as usize;

            // Try schema-based parsing first if usmap available
            let properties = if usmap_schema.is_some() {
                parse_export_properties_with_schema(
                    data,
                    absolute_offset,
                    export.cooked_serial_size as usize,
                    &names,
                    &struct_lookup,
                    verbose,
                )
            } else {
                parse_export_properties(
                    data,
                    absolute_offset,
                    export.cooked_serial_size as usize,
                    &names,
                )
            };

            ZenExportInfo {
                index: i,
                object_name: header.name_map.get(export.object_name).to_string(),
                class_index: format!("{:?}", export.class_index),
                super_index: format!("{:?}", export.super_index),
                template_index: format!("{:?}", export.template_index),
                outer_index: format!("{:?}", export.outer_index),
                public_export_hash: export.public_export_hash,
                cooked_serial_offset: export.cooked_serial_offset,
                cooked_serial_size: export.cooked_serial_size,
                properties,
            }
        })
        .collect();

    let info = ZenAssetInfo {
        path: path.to_string(),
        package_name: header.package_name(),
        package_flags: header.summary.package_flags,
        is_unversioned: header.is_unversioned,
        name_count: names.len(),
        import_count: imports.len(),
        export_count: exports.len(),
        names,
        imports,
        exports,
    };

    Ok(serde_json::to_string_pretty(&info)?)
}

// ============================================================================
// Texture Extraction Command
// ============================================================================

fn extract_texture_cmd(
    ubulk_path: &std::path::Path,
    width: u32,
    height: u32,
    output_path: &std::path::Path,
    mip_level: usize,
    format: &str,
) -> Result<()> {
    use std::io::Read;

    let bytes_per_block: u64 = match format {
        "bc1" | "dxt1" => 8,
        "bc7" => 16,
        _ => 16,
    };

    eprintln!("Reading texture: {:?}", ubulk_path);
    eprintln!("Dimensions: {}x{}, format: {}", width, height, format);

    // Read the ubulk file
    let mut file = std::fs::File::open(ubulk_path)
        .context("Failed to open ubulk file")?;

    // Calculate mip dimensions and offsets
    let mut mip_width = width;
    let mut mip_height = height;
    let mut offset: u64 = 0;

    for i in 0..mip_level {
        // Calculate size of this mip
        let blocks_x = (mip_width as u64 + 3) / 4;
        let blocks_y = (mip_height as u64 + 3) / 4;
        let mip_size = blocks_x * blocks_y * bytes_per_block;

        offset += mip_size;
        mip_width = (mip_width / 2).max(1);
        mip_height = (mip_height / 2).max(1);

        eprintln!("Skipping mip {}: {}x{} ({} bytes)", i, mip_width * 2, mip_height * 2, mip_size);
    }

    eprintln!("Extracting mip {}: {}x{} at offset {}", mip_level, mip_width, mip_height, offset);

    // Calculate size needed for this mip
    let blocks_x = (mip_width as usize + 3) / 4;
    let blocks_y = (mip_height as usize + 3) / 4;
    let mip_size = blocks_x * blocks_y * bytes_per_block as usize;

    // Seek to the mip and read it
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(offset))?;

    let mut mip_data = vec![0u8; mip_size];
    file.read_exact(&mut mip_data)
        .context("Failed to read mip data")?;

    eprintln!("Read {} bytes of {} data", mip_data.len(), format);

    // Decode to RGBA based on format
    let rgba = match format {
        "bc1" | "dxt1" => texture::decode_bc1(&mip_data, mip_width, mip_height)
            .context("Failed to decode BC1 texture")?,
        "bc7" => texture::decode_bc7(&mip_data, mip_width, mip_height)
            .context("Failed to decode BC7 texture")?,
        _ => anyhow::bail!("Unsupported format: {}", format),
    };

    eprintln!("Decoded to {} bytes of RGBA", rgba.len());

    // Save as PNG
    texture::save_png(&rgba, mip_width, mip_height, output_path)
        .context("Failed to save PNG")?;

    eprintln!("Saved to {:?}", output_path);

    Ok(())
}

// ============================================================================
// ScriptObjects Extraction Command
// ============================================================================

/// Entry in the ScriptObjects lookup table
#[derive(Debug, Serialize)]
struct ScriptObjectEntry {
    /// Object name (class name like "InventoryPartDef")
    name: String,
    /// Full object path (like "/Script/GbxInventory.InventoryPartDef")
    path: String,
    /// The hash used in FPackageObjectIndex::ScriptImport
    hash: String,
    /// Raw hash value as u64
    hash_value: u64,
    /// Outer object hash (parent)
    outer_hash: Option<String>,
    /// CDO class hash
    cdo_class_hash: Option<String>,
}

/// Full ScriptObjects dump
#[derive(Debug, Serialize)]
struct ScriptObjectsDump {
    /// Total count
    count: usize,
    /// All script objects with their hashes
    objects: Vec<ScriptObjectEntry>,
    /// Hash to path lookup (for quick resolution)
    hash_to_path: HashMap<String, String>,
}

fn extract_script_objects(
    input: &std::path::Path,
    output: &std::path::Path,
    aes_key: Option<&str>,
) -> Result<()> {
    use retoc::script_objects::FPackageObjectIndexType;

    eprintln!("Loading ScriptObjects from {:?}", input);

    // Build retoc config
    let mut aes_keys = HashMap::new();
    if let Some(key) = aes_key {
        let parsed_key: AesKey = key.parse()
            .context("Invalid AES key format (use hex or base64)")?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    // Open IoStore
    let store = iostore::open(input, config)
        .with_context(|| format!("Failed to open {:?}", input))?;

    // Load ScriptObjects
    let script_objects = store.load_script_objects()
        .context("Failed to load ScriptObjects (is this the Paks directory with global.utoc?)")?;

    eprintln!("Found {} script objects", script_objects.script_objects.len());

    // Build the entries
    let mut objects = Vec::new();
    let mut hash_to_path = HashMap::new();

    for obj in &script_objects.script_objects {
        let name = script_objects.global_name_map.get(obj.object_name).to_string();

        // Build the full path by resolving outer chain
        let path = resolve_script_object_path(obj, &script_objects);

        // Get the hash from global_index
        let hash_value = obj.global_index.raw_index();
        let hash = format!("{:X}", hash_value);

        // Get outer and cdo hashes
        let outer_hash = if obj.outer_index.kind() == FPackageObjectIndexType::ScriptImport {
            Some(format!("{:X}", obj.outer_index.raw_index()))
        } else {
            None
        };

        let cdo_class_hash = if obj.cdo_class_index.kind() == FPackageObjectIndexType::ScriptImport {
            Some(format!("{:X}", obj.cdo_class_index.raw_index()))
        } else {
            None
        };

        hash_to_path.insert(hash.clone(), path.clone());

        objects.push(ScriptObjectEntry {
            name,
            path,
            hash,
            hash_value,
            outer_hash,
            cdo_class_hash,
        });
    }

    let dump = ScriptObjectsDump {
        count: objects.len(),
        objects,
        hash_to_path,
    };

    // Write to JSON
    let json = serde_json::to_string_pretty(&dump)?;
    std::fs::write(output, &json)
        .with_context(|| format!("Failed to write {:?}", output))?;

    eprintln!("Wrote {} script objects to {:?}", dump.count, output);

    // Print some stats
    let inventory_parts: Vec<_> = dump.objects.iter()
        .filter(|o| o.name.contains("InventoryPart") || o.name.contains("PartDef"))
        .collect();
    if !inventory_parts.is_empty() {
        eprintln!("\nInventoryPart-related objects:");
        for obj in inventory_parts.iter().take(10) {
            eprintln!("  {} -> {}", obj.hash, obj.path);
        }
        if inventory_parts.len() > 10 {
            eprintln!("  ... and {} more", inventory_parts.len() - 10);
        }
    }

    Ok(())
}

/// Resolve the full path of a script object by walking the outer chain
fn resolve_script_object_path(
    obj: &retoc::script_objects::FScriptObjectEntry,
    script_objects: &retoc::script_objects::ZenScriptObjects,
) -> String {
    use retoc::script_objects::FPackageObjectIndexType;

    let name = script_objects.global_name_map.get(obj.object_name).to_string();

    // If no outer, this is a top-level package
    if obj.outer_index.kind() != FPackageObjectIndexType::ScriptImport {
        return name;
    }

    // Look up the outer object
    if let Some(outer) = script_objects.script_object_lookup.get(&obj.outer_index) {
        let outer_path = resolve_script_object_path(outer, script_objects);
        format!("{}.{}", outer_path, name)
    } else {
        // Outer not found, just return the name
        name
    }
}

// ============================================================================
// Find Assets by Class Command
// ============================================================================

fn find_assets_by_class(
    input: &std::path::Path,
    class_name: &str,
    scriptobjects_path: &std::path::Path,
    aes_key: Option<&str>,
    output: Option<&std::path::Path>,
) -> Result<()> {
    use retoc::script_objects::FPackageObjectIndexType;

    eprintln!("Searching for assets of class: {}", class_name);

    // Load scriptobjects
    let so_data = std::fs::read_to_string(scriptobjects_path)
        .with_context(|| format!("Failed to read {:?}", scriptobjects_path))?;
    let so_json: serde_json::Value = serde_json::from_str(&so_data)?;

    // Build hash→class lookup
    let hash_to_path: HashMap<String, String> = so_json
        .get("hash_to_path")
        .and_then(|v| v.as_object())
        .context("Missing hash_to_path")?
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    // Find the target class hash
    let target_hash: Option<String> = so_json
        .get("objects")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find(|obj| {
                obj.get("name").and_then(|n| n.as_str()) == Some(class_name) ||
                obj.get("path").and_then(|p| p.as_str()).map(|p| p.ends_with(&format!(".{}", class_name))).unwrap_or(false)
            })
        })
        .and_then(|obj| obj.get("hash").and_then(|h| h.as_str()).map(|s| s.to_string()));

    let target_hash = target_hash.context(format!("Class '{}' not found in scriptobjects", class_name))?;
    let target_path = hash_to_path.get(&target_hash).cloned().unwrap_or_default();
    eprintln!("Target class: {} -> {}", target_hash, target_path);

    // Build retoc config
    let mut aes_keys = HashMap::new();
    if let Some(key) = aes_key {
        let parsed_key: AesKey = key.parse()?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    // Open IoStore
    let store = iostore::open(input, config)?;

    // Get container versions
    let toc_version = store.container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store.container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    // Scan all .uasset files
    let uasset_entries: Vec<_> = store
        .chunks()
        .filter_map(|chunk| {
            chunk.path().and_then(|path| {
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
        })
        .collect();

    eprintln!("Scanning {} .uasset files...", uasset_entries.len());

    let pb = ProgressBar::new(uasset_entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Check each asset's class_index
    let matching_paths: Vec<String> = uasset_entries
        .par_iter()
        .filter_map(|(chunk, path)| {
            pb.inc(1);

            // Read the asset data
            let data = chunk.read().ok()?;

            // Quick parse to get export class_index
            let mut cursor = Cursor::new(&data);
            let header = FZenPackageHeader::deserialize(
                &mut cursor,
                None,
                toc_version,
                container_header_version,
                None,
            ).ok()?;

            // Check each export's class_index
            for export in &header.export_map {
                if export.class_index.kind() == FPackageObjectIndexType::ScriptImport {
                    let class_hash = format!("{:X}", export.class_index.raw_index());
                    if class_hash == target_hash {
                        return Some(path.clone());
                    }
                }
            }
            None
        })
        .collect();

    pb.finish_and_clear();

    eprintln!("Found {} assets of class {}", matching_paths.len(), class_name);

    // Output results
    for path in &matching_paths {
        println!("{}", path);
    }

    // Write to file if requested
    if let Some(out_path) = output {
        let content = matching_paths.join("\n");
        std::fs::write(out_path, content)?;
        eprintln!("Wrote paths to {:?}", out_path);
    }

    Ok(())
}

/// List all unique class hashes found in pak files
fn list_classes(
    input: &PathBuf,
    scriptobjects_path: &PathBuf,
    aes_key: Option<&str>,
    samples: usize,
) -> Result<()> {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use retoc::script_objects::FPackageObjectIndexType;

    // Load scriptobjects for name resolution
    let so_data = std::fs::read_to_string(scriptobjects_path)
        .with_context(|| format!("Failed to read scriptobjects file {:?}", scriptobjects_path))?;
    let so_json: serde_json::Value = serde_json::from_str(&so_data)?;

    let hash_to_path: HashMap<String, String> = so_json.get("hash_to_path")
        .and_then(|v| v.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();

    // Build retoc config
    let mut aes_keys = HashMap::new();
    if let Some(key) = aes_key {
        let parsed_key: AesKey = key.parse()?;
        aes_keys.insert(FGuid::default(), parsed_key);
    }
    let config = Arc::new(Config {
        aes_keys,
        container_header_version_override: None,
        toc_version_override: None,
    });

    // Open IoStore
    let store = iostore::open(input, config)?;

    // Get container versions
    let toc_version = store.container_file_version()
        .unwrap_or(EIoStoreTocVersion::ReplaceIoChunkHashWithIoHash);
    let container_header_version = store.container_header_version()
        .unwrap_or(EIoContainerHeaderVersion::NoExportInfo);

    // Scan all .uasset files
    let uasset_entries: Vec<_> = store
        .chunks()
        .filter_map(|chunk| {
            chunk.path().and_then(|path| {
                if path.ends_with(".uasset") {
                    Some((chunk, path))
                } else {
                    None
                }
            })
        })
        .collect();

    eprintln!("Scanning {} .uasset files...", uasset_entries.len());

    // Collect classes: hash -> (class_name, count, sample_paths)
    let class_map: Arc<Mutex<BTreeMap<String, (String, usize, Vec<String>)>>> = Arc::new(Mutex::new(BTreeMap::new()));

    let pb = ProgressBar::new(uasset_entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("#>-"),
    );

    uasset_entries.par_iter().for_each(|(chunk, path)| {
        pb.inc(1);

        if let Ok(data) = chunk.read() {
            let mut cursor = Cursor::new(&data);
            if let Ok(header) = FZenPackageHeader::deserialize(
                &mut cursor,
                None,
                toc_version,
                container_header_version,
                None,
            ) {
                for export in &header.export_map {
                    if export.class_index.kind() == FPackageObjectIndexType::ScriptImport {
                        let class_hash = format!("{:X}", export.class_index.raw_index());
                        let mut map = class_map.lock().unwrap();
                        let entry = map.entry(class_hash.clone()).or_insert_with(|| {
                            let name = hash_to_path.get(&class_hash).cloned().unwrap_or_else(|| "UNKNOWN".to_string());
                            (name, 0, Vec::new())
                        });
                        entry.1 += 1;
                        if entry.2.len() < samples {
                            entry.2.push(path.clone());
                        }
                    }
                }
            }
        }
    });

    pb.finish_and_clear();

    // Print results sorted by count
    let map = class_map.lock().unwrap();
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|a, b| b.1.1.cmp(&a.1.1));

    eprintln!("\n{} unique class types found:", entries.len());
    println!("{:<20} {:<60} {}", "Hash", "Class Name", "Count");
    println!("{}", "-".repeat(100));

    for (hash, (name, count, sample_paths)) in entries {
        println!("{:<20} {:<60} {}", hash, name, count);
        for path in sample_paths {
            println!("  -> {}", path);
        }
    }

    Ok(())
}
