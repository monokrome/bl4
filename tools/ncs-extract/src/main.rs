use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use clap::Parser;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::os::raw::{c_int, c_uchar};
use std::path::PathBuf;

// FFI bindings to oozlin's Kraken decompressor (via our C wrapper)
extern "C" {
    fn ooz_kraken_decompress(
        src: *const c_uchar,
        src_len: usize,
        dst: *mut c_uchar,
        dst_len: usize,
    ) -> c_int;
}

#[derive(Parser, Debug)]
#[command(name = "ncs-extract")]
#[command(about = "Extract Gearbox NCS (Nexus Compiled Script) files")]
struct Args {
    /// Input NCS file
    input: PathBuf,

    /// Output file (defaults to input with .bin extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Show header information only
    #[arg(short = 'i', long)]
    info: bool,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Try all decompression methods
    #[arg(short = 'a', long)]
    try_all: bool,

    /// Skip first N bytes of payload when trying decompression
    #[arg(long, default_value = "0")]
    skip: usize,
}

#[derive(Debug)]
struct NcsHeader {
    magic: [u8; 4],
    version: u32,
    decompressed_size: u32,
    compressed_size: u32,
    hash_type: [u8; 4],
    hash_value: u32,
    flags: [u8; 8],
    metadata: Vec<u8>,
}

impl NcsHeader {
    fn parse(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < 0x30 {
            bail!("File too small for NCS header");
        }

        let mut cursor = Cursor::new(data);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;

        if &magic != b"\x01NCS" {
            bail!("Invalid NCS magic: {:02x?}", magic);
        }

        let version = cursor.read_u32::<LittleEndian>()?;
        let decompressed_size = cursor.read_u32::<LittleEndian>()?;
        let compressed_size = cursor.read_u32::<LittleEndian>()?;

        let mut hash_type = [0u8; 4];
        cursor.read_exact(&mut hash_type)?;

        let hash_value = cursor.read_u32::<LittleEndian>()?;

        let mut flags = [0u8; 8];
        cursor.read_exact(&mut flags)?;

        // Read remaining metadata until we find the compressed data
        // The header seems to be 0x60 bytes based on analysis
        let header_size = 0x60;
        let mut metadata = vec![0u8; header_size - 0x20];
        cursor.read_exact(&mut metadata)?;

        Ok((
            NcsHeader {
                magic,
                version,
                decompressed_size,
                compressed_size,
                hash_type,
                hash_value,
                flags,
                metadata,
            },
            header_size,
        ))
    }

    fn display(&self) {
        println!("NCS Header:");
        println!("  Magic: {:02x?} ({:?})", self.magic, std::str::from_utf8(&self.magic[1..]).unwrap_or("?"));
        println!("  Version: {}", self.version);
        println!("  Decompressed size: {} (0x{:x})", self.decompressed_size, self.decompressed_size);
        println!("  Compressed size: {} (0x{:x})", self.compressed_size, self.compressed_size);
        println!("  Hash type: {:02x?} ({:?})", self.hash_type, std::str::from_utf8(&self.hash_type).unwrap_or("?"));
        println!("  Hash value: 0x{:08x}", self.hash_value);
        println!("  Flags: {:02x?}", self.flags);
        println!("  Metadata (first 32 bytes): {:02x?}", &self.metadata[..32.min(self.metadata.len())]);
    }
}

fn try_zlib(data: &[u8], expected_size: u32) -> Result<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::with_capacity(expected_size as usize);
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn try_deflate(data: &[u8], expected_size: u32) -> Result<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    let mut decoder = DeflateDecoder::new(data);
    let mut output = Vec::with_capacity(expected_size as usize);
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn try_gzip(data: &[u8], expected_size: u32) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    let mut decoder = GzDecoder::new(data);
    let mut output = Vec::with_capacity(expected_size as usize);
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn try_lz4(data: &[u8], expected_size: u32) -> Result<Vec<u8>> {
    // Try LZ4 block decompression
    let output = lz4_flex::decompress(data, expected_size as usize)?;
    Ok(output)
}

fn try_zstd(data: &[u8], _expected_size: u32) -> Result<Vec<u8>> {
    let output = zstd::decode_all(data)?;
    Ok(output)
}

fn try_lzma(data: &[u8], _expected_size: u32) -> Result<Vec<u8>> {
    use xz2::read::XzDecoder;
    let mut decoder = XzDecoder::new(data);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn scan_for_compression_signatures(data: &[u8]) {
    println!("\nScanning for compression signatures...");

    // Common compression headers
    let signatures = [
        (&[0x78, 0x9C][..], "zlib (default compression)"),
        (&[0x78, 0xDA][..], "zlib (best compression)"),
        (&[0x78, 0x01][..], "zlib (no compression)"),
        (&[0x78, 0x5E][..], "zlib (fast compression)"),
        (&[0x1F, 0x8B][..], "gzip"),
        (&[0x28, 0xB5, 0x2F, 0xFD][..], "zstd"),
        (&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00][..], "xz/lzma"),
        (&[0x04, 0x22, 0x4D, 0x18][..], "lz4 frame"),
    ];

    for (offset, window) in data.windows(6).enumerate() {
        for (sig, name) in &signatures {
            if window.starts_with(sig) {
                println!("  Found {} at offset 0x{:x}", name, offset);
            }
        }
    }
}

/// Decompress using Kraken/Oodle via oozlin FFI
fn try_kraken(data: &[u8], expected_size: u32) -> Result<Vec<u8>> {
    let mut output = vec![0u8; expected_size as usize];

    let result = unsafe {
        ooz_kraken_decompress(
            data.as_ptr(),
            data.len(),
            output.as_mut_ptr(),
            expected_size as usize,
        )
    };

    if result < 0 {
        bail!("Kraken decompression failed with error code: {}", result);
    }

    // Result is the number of bytes written
    if result as u32 != expected_size {
        output.truncate(result as usize);
    }

    Ok(output)
}

fn analyze_byte_distribution(data: &[u8]) {
    println!("\nByte distribution analysis:");

    let mut freq = [0u32; 256];
    for &byte in data {
        freq[byte as usize] += 1;
    }

    // Find most common bytes
    let mut sorted: Vec<(u8, u32)> = freq.iter().enumerate()
        .map(|(i, &f)| (i as u8, f))
        .filter(|(_, f)| *f > 0)
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    println!("  Top 10 most common bytes:");
    for (byte, count) in sorted.iter().take(10) {
        let pct = (*count as f64 / data.len() as f64) * 100.0;
        let chr = if byte.is_ascii_graphic() || *byte == b' ' {
            format!("'{}'", *byte as char)
        } else {
            format!("   ")
        };
        println!("    0x{:02x} {}: {} times ({:.1}%)", byte, chr, count, pct);
    }

    // Check for potential XOR keys
    println!("\n  Checking for XOR patterns:");
    let zero_count = freq[0];
    if zero_count > data.len() as u32 / 50 {
        println!("    High null byte count ({}) - data may not be XOR encoded", zero_count);
    }

    // Check if data looks like it could be XOR'd text
    let high_byte_count: u32 = freq[0x80..].iter().sum();
    let low_byte_count: u32 = freq[..0x80].iter().sum();
    println!("    Low bytes (0x00-0x7F): {} ({:.1}%)",
             low_byte_count,
             (low_byte_count as f64 / data.len() as f64) * 100.0);
    println!("    High bytes (0x80-0xFF): {} ({:.1}%)",
             high_byte_count,
             (high_byte_count as f64 / data.len() as f64) * 100.0);
}

fn try_xor_decode(data: &[u8], key: u8) -> Vec<u8> {
    data.iter().map(|&b| b ^ key).collect()
}

fn check_for_readable_strings(data: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut start = 0;

    for (i, &byte) in data.iter().enumerate() {
        if byte.is_ascii_graphic() || byte == b' ' {
            if current.is_empty() {
                start = i;
            }
            current.push(byte as char);
        } else {
            if current.len() >= min_len {
                strings.push((start, current.clone()));
            }
            current.clear();
        }
    }

    if current.len() >= min_len {
        strings.push((start, current));
    }

    strings
}

fn analyze_entropy(data: &[u8], block_size: usize) {
    println!("\nEntropy analysis (block size {}):", block_size);

    for (i, chunk) in data.chunks(block_size).enumerate().take(10) {
        let mut freq = [0u32; 256];
        for &byte in chunk {
            freq[byte as usize] += 1;
        }

        let len = chunk.len() as f64;
        let entropy: f64 = freq.iter()
            .filter(|&&f| f > 0)
            .map(|&f| {
                let p = f as f64 / len;
                -p * p.log2()
            })
            .sum();

        let is_compressed = entropy > 7.0;
        println!("  Block {}: entropy {:.2} bits/byte {}",
                 i, entropy,
                 if is_compressed { "(likely compressed)" } else { "(may be header/uncompressed)" });
    }
}

fn try_decompress_at_offset(data: &[u8], offset: usize, expected_size: u32, verbose: bool) -> Option<(String, Vec<u8>)> {
    if offset >= data.len() {
        return None;
    }

    let payload = &data[offset..];

    let methods: Vec<(&str, Box<dyn Fn(&[u8], u32) -> Result<Vec<u8>>>)> = vec![
        ("zlib", Box::new(try_zlib)),
        ("deflate", Box::new(try_deflate)),
        ("gzip", Box::new(try_gzip)),
        ("lz4", Box::new(try_lz4)),
        ("zstd", Box::new(try_zstd)),
        ("lzma/xz", Box::new(try_lzma)),
    ];

    for (name, decompress) in methods {
        match decompress(payload, expected_size) {
            Ok(output) => {
                if output.len() == expected_size as usize {
                    println!("  SUCCESS: {} at offset 0x{:x} produced exact expected size", name, offset);
                    return Some((name.to_string(), output));
                } else if output.len() > 0 {
                    if verbose {
                        println!("  PARTIAL: {} at offset 0x{:x} produced {} bytes (expected {})",
                                 name, offset, output.len(), expected_size);
                    }
                    // Accept if reasonably close
                    if output.len() >= expected_size as usize / 2 {
                        return Some((name.to_string(), output));
                    }
                }
            }
            Err(e) => {
                if verbose {
                    println!("  {} at offset 0x{:x}: {}", name, offset, e);
                }
            }
        }
    }

    None
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut file = File::open(&args.input)
        .with_context(|| format!("Failed to open {}", args.input.display()))?;

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    println!("File: {} ({} bytes)", args.input.display(), data.len());

    let (header, header_size) = NcsHeader::parse(&data)?;
    header.display();

    if args.info {
        scan_for_compression_signatures(&data[header_size..]);
        analyze_entropy(&data, 256);
        return Ok(());
    }

    let payload = &data[header_size..];
    println!("\nPayload starts at offset 0x{:x}, size {} bytes", header_size, payload.len());
    println!("First 64 bytes of payload: {:02x?}", &payload[..64.min(payload.len())]);

    // Try to decompress
    println!("\nAttempting decompression...");

    // Try various offsets into the payload
    let offsets_to_try = if args.try_all {
        vec![0, 8, 16, 32, 48, 64, 128]
    } else {
        vec![args.skip]
    };

    let mut result = None;

    for offset in offsets_to_try {
        if let Some((method, output)) = try_decompress_at_offset(payload, offset, header.decompressed_size, args.verbose) {
            println!("\nDecompression successful with {} at payload offset {}", method, offset);
            result = Some(output);
            break;
        }
    }

    // Try Kraken/Oodle decompression (via oozlin)
    if result.is_none() {
        println!("\nTrying Kraken/Oodle decompression...");
        // Try various offsets - NCS seems to have a 4-byte prefix before compressed data
        for offset in &[0usize, 4, 8, 12, 16, 32, 64] {
            if *offset < payload.len() {
                match try_kraken(&payload[*offset..], header.decompressed_size) {
                    Ok(output) => {
                        println!("  SUCCESS: Kraken at offset {} produced {} bytes", offset, output.len());
                        result = Some(output);
                        break;
                    }
                    Err(e) => {
                        if args.verbose {
                            println!("  Kraken at offset {}: {}", offset, e);
                        }
                    }
                }
            }
        }
    }

    // Analyze the first bytes of compressed data for debugging
    if result.is_none() && payload.len() >= 16 {
        println!("\nAnalyzing payload structure:");
        println!("  Bytes 0-3: {:02x} {:02x} {:02x} {:02x} (prefix)", payload[0], payload[1], payload[2], payload[3]);
        if payload.len() >= 8 {
            println!("  Bytes 4-7: {:02x} {:02x} {:02x} {:02x}", payload[4], payload[5], payload[6], payload[7]);
            // Check if byte 4 could be an Oodle header (needs lower nibble = 0xC)
            let byte4 = payload[4];
            println!("  Byte 4 analysis: 0x{:02x} - lower nibble = 0x{:x} (needs 0xC for Kraken)", byte4, byte4 & 0xF);
        }
    }

    if result.is_none() {
        // If standard methods fail, do detailed analysis
        scan_for_compression_signatures(payload);
        analyze_entropy(payload, 256);
        analyze_byte_distribution(payload);

        // Check for readable strings in the payload
        let strings = check_for_readable_strings(payload, 6);
        if !strings.is_empty() {
            println!("\nFound {} readable strings (len >= 6):", strings.len());
            for (offset, s) in strings.iter().take(10) {
                println!("  0x{:04x}: {:?}", offset, s);
            }
            if strings.len() > 10 {
                println!("  ... and {} more", strings.len() - 10);
            }
        }

        println!("\nNo decompression method worked (including Kraken/Oodle).");
        println!("\nThe compression may be:");
        println!("  - A variant of Oodle not supported by oozlin");
        println!("  - Custom Gearbox compression");
        println!("  - Data is encrypted before compression");

        // Output raw payload for manual analysis
        let output_path = args.output.unwrap_or_else(|| {
            args.input.with_extension("raw")
        });

        let mut out_file = File::create(&output_path)?;
        out_file.write_all(payload)?;
        println!("\nRaw payload written to: {}", output_path.display());

        return Ok(());
    }

    let output_data = result.unwrap();

    // Write output
    let output_path = args.output.unwrap_or_else(|| {
        args.input.with_extension("bin")
    });

    let mut out_file = File::create(&output_path)?;
    out_file.write_all(&output_data)?;

    println!("Decompressed {} bytes to {}", output_data.len(), output_path.display());

    // Show preview of decompressed data
    println!("\nFirst 256 bytes of decompressed data:");
    for (i, chunk) in output_data.chunks(32).take(8).enumerate() {
        print!("{:04x}: ", i * 32);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        print!(" |");
        for byte in chunk {
            let c = *byte as char;
            print!("{}", if c.is_ascii_graphic() || c == ' ' { c } else { '.' });
        }
        println!("|");
    }

    Ok(())
}
