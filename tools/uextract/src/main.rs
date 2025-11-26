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
            let result = extract_entry(chunk, path, &args, toc_version, container_header_version);
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
        match parse_zen_to_json(&data, path, toc_version, container_header_version) {
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

fn parse_zen_to_json(
    data: &[u8],
    path: &str,
    toc_version: EIoStoreTocVersion,
    container_header_version: EIoContainerHeaderVersion,
) -> Result<String> {
    use std::io::Seek;
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
            let properties = parse_export_properties(
                data,
                absolute_offset,
                export.cooked_serial_size as usize,
                &names,
            );

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
