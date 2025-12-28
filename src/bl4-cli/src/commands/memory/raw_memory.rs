//! Raw memory access command handlers
//!
//! Handlers for reading, writing, scanning, and patching memory.

use crate::memory::{self, Bl4Process, MemorySource};
use anyhow::{bail, Context, Result};

/// Parse a hex or decimal address string
fn parse_address(address: &str) -> Result<usize> {
    if address.starts_with("0x") || address.starts_with("0X") {
        usize::from_str_radix(&address[2..], 16).context("Invalid hex address")
    } else {
        address.parse::<usize>().context("Invalid address")
    }
}

/// Parse hex bytes from a space-separated string
fn parse_hex_bytes(bytes: &str) -> Result<Vec<u8>> {
    let parts: Vec<&str> = bytes.split_whitespace().collect();
    let mut data = Vec::new();
    for part in parts {
        let byte =
            u8::from_str_radix(part, 16).with_context(|| format!("Invalid hex byte: {}", part))?;
        data.push(byte);
    }
    Ok(data)
}

/// Handle the Read command
///
/// Reads bytes from memory and displays them as a hex dump.
pub fn handle_read(source: &dyn MemorySource, address: &str, size: usize) -> Result<()> {
    let addr = parse_address(address)?;
    let data = source.read_bytes(addr, size)?;

    // Print hex dump
    println!("Reading {} bytes at {:#x}:", size, addr);
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("{:08x}  ", addr + i * 16);
        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02x} ", byte);
            if j == 7 {
                print!(" ");
            }
        }
        // Pad if last line is short
        if chunk.len() < 16 {
            for j in chunk.len()..16 {
                print!("   ");
                if j == 7 {
                    print!(" ");
                }
            }
        }
        print!(" |");
        for byte in chunk {
            let c = *byte as char;
            if c.is_ascii_graphic() || c == ' ' {
                print!("{}", c);
            } else {
                print!(".");
            }
        }
        println!("|");
    }

    Ok(())
}

/// Handle the Write command
///
/// Writes bytes to memory (requires live process).
pub fn handle_write(proc: &Bl4Process, address: &str, bytes: &str) -> Result<()> {
    let addr = parse_address(address)?;
    let data = parse_hex_bytes(bytes)?;

    // Show what we're about to write
    println!("Writing {} bytes to {:#x}:", data.len(), addr);
    print!("  ");
    for byte in &data {
        print!("{:02x} ", byte);
    }
    println!();

    // Read original bytes first for safety
    let original = proc.read_bytes_direct(addr, data.len())?;
    print!("Original: ");
    for byte in &original {
        print!("{:02x} ", byte);
    }
    println!();

    // Write the new bytes
    proc.write_bytes(addr, &data)?;
    println!("Write successful!");

    Ok(())
}

/// Handle the Scan command
///
/// Scans memory for a byte pattern with wildcards.
pub fn handle_scan(source: &dyn MemorySource, pattern: &str) -> Result<()> {
    // Parse pattern like "48 8B 05 ?? ?? ?? ??"
    let parts: Vec<&str> = pattern.split_whitespace().collect();
    let mut bytes = Vec::new();
    let mut mask = Vec::new();

    for part in parts {
        if part == "??" || part == "?" {
            bytes.push(0u8);
            mask.push(0u8); // 0 = wildcard
        } else {
            let byte = u8::from_str_radix(part, 16)
                .with_context(|| format!("Invalid hex byte: {}", part))?;
            bytes.push(byte);
            mask.push(1u8); // 1 = must match
        }
    }

    println!("Scanning for pattern: {}", pattern);
    println!("This may take a while...");

    let results = memory::scan_pattern(source, &bytes, &mask)?;

    if results.is_empty() {
        println!("No matches found.");
    } else {
        println!("Found {} matches:", results.len());
        for (i, addr) in results.iter().take(20).enumerate() {
            println!("  {}: {:#x}", i + 1, addr);
        }
        if results.len() > 20 {
            println!("  ... and {} more", results.len() - 20);
        }
    }

    Ok(())
}

/// Handle the Patch command
///
/// Patches memory with NOPs or custom bytes (requires live process).
pub fn handle_patch(
    proc: &Bl4Process,
    address: &str,
    nop: Option<usize>,
    bytes: Option<&str>,
) -> Result<()> {
    let addr = parse_address(address)?;

    let patch_bytes = if let Some(nop_count) = nop {
        // Generate NOP bytes (0x90 on x86-64)
        vec![0x90u8; nop_count]
    } else if let Some(hex_bytes) = bytes {
        parse_hex_bytes(hex_bytes)?
    } else {
        bail!("Must specify either --nop <count> or --bytes <hex>");
    };

    // Read original bytes first
    let original = proc.read_bytes_direct(addr, patch_bytes.len())?;
    println!("Patching {} bytes at {:#x}", patch_bytes.len(), addr);
    print!("Original: ");
    for byte in &original {
        print!("{:02x} ", byte);
    }
    println!();
    print!("New:      ");
    for byte in &patch_bytes {
        print!("{:02x} ", byte);
    }
    println!();

    // Apply the patch
    proc.write_bytes(addr, &patch_bytes)?;
    println!("Patch applied!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::source::tests::MockMemorySource;

    #[test]
    fn test_parse_address_hex() {
        assert_eq!(parse_address("0x1000").unwrap(), 0x1000);
        assert_eq!(parse_address("0X1000").unwrap(), 0x1000);
        assert_eq!(parse_address("0xDEADBEEF").unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_parse_address_decimal() {
        assert_eq!(parse_address("4096").unwrap(), 4096);
        assert_eq!(parse_address("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_address_invalid() {
        assert!(parse_address("0xGGGG").is_err());
        assert!(parse_address("not_a_number").is_err());
    }

    #[test]
    fn test_parse_hex_bytes() {
        let result = parse_hex_bytes("48 8B 05").unwrap();
        assert_eq!(result, vec![0x48, 0x8B, 0x05]);
    }

    #[test]
    fn test_parse_hex_bytes_single() {
        let result = parse_hex_bytes("90").unwrap();
        assert_eq!(result, vec![0x90]);
    }

    #[test]
    fn test_parse_hex_bytes_invalid() {
        assert!(parse_hex_bytes("GG").is_err());
        assert!(parse_hex_bytes("48 GG 05").is_err());
    }

    #[test]
    fn test_handle_read() {
        let data = vec![0x48, 0x8B, 0x05, 0x00, 0x00, 0x00, 0x00];
        let source = MockMemorySource::new(data, 0x1000);
        let result = handle_read(&source, "0x1000", 7);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_read_invalid_address() {
        let source = MockMemorySource::new(vec![], 0x1000);
        let result = handle_read(&source, "invalid", 16);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_scan_pattern_parsing() {
        // Just test that pattern parsing works - actual scan would need mock with data
        let source = MockMemorySource::new(vec![], 0x1000);
        // Empty source means scan returns empty, but parsing should work
        let result = handle_scan(&source, "48 8B ?? ??");
        assert!(result.is_ok());
    }
}
