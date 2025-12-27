//! Minidump extraction command handler
//!
//! Extracts PE executables from Windows minidump files.

use anyhow::{bail, Context, Result};
use minidump::{Minidump, MinidumpMemory64List, MinidumpModuleList};
use std::fs;
use std::path::{Path, PathBuf};

/// Handle the ExtractCommand::MinidumpToExe command
///
/// Extracts a PE executable from a Windows minidump file.
pub fn handle_minidump_to_exe(
    input: &Path,
    output: Option<PathBuf>,
    base: &str,
) -> Result<()> {
    // Parse base address
    let base_addr = if base.starts_with("0x") || base.starts_with("0X") {
        u64::from_str_radix(&base[2..], 16)
            .with_context(|| format!("Invalid hex base address: {}", base))?
    } else {
        base.parse::<u64>()
            .with_context(|| format!("Invalid base address: {}", base))?
    };

    println!("Extracting PE from minidump: {:?}", input);
    println!("Base address: {:#x}", base_addr);

    // Open and parse the minidump
    let dump = Minidump::read_path(input)
        .with_context(|| format!("Failed to read minidump: {:?}", input))?;

    // Get module list to find the main executable
    if let Ok(modules) = dump.get_stream::<MinidumpModuleList>() {
        println!("\nModules in target range:");
        for module in modules.iter() {
            if module.raw.base_of_image >= base_addr
                && module.raw.base_of_image < base_addr + 0x100000000
            {
                println!(
                    "  {:#x} - {:#x}: {}",
                    module.raw.base_of_image,
                    module.raw.base_of_image + module.raw.size_of_image as u64,
                    module.name
                );
            }
        }
    }

    // Get the memory list
    let memory = dump
        .get_stream::<MinidumpMemory64List>()
        .context("Failed to get Memory64List from minidump (not a full dump?)")?;

    println!("\nSearching for PE at base {:#x}...", base_addr);

    // Collect memory regions within the PE address range
    let mut regions: Vec<_> = memory
        .iter()
        .filter(|mem| mem.base_address >= base_addr && mem.base_address < base_addr + 0x40000000)
        .collect();

    regions.sort_by_key(|m| m.base_address);

    if regions.is_empty() {
        bail!("No memory regions found at base address {:#x}", base_addr);
    }

    println!("Found {} memory regions in PE range", regions.len());

    // Read the DOS header to get size_of_image
    let first_region = regions
        .iter()
        .find(|m| m.base_address == base_addr)
        .context("No memory region at exact base address")?;

    let header_data = first_region.bytes;
    if header_data.len() < 64 || &header_data[0..2] != b"MZ" {
        bail!("No valid MZ header found at base address");
    }
    println!("Found MZ header at {:#x}", base_addr);

    // Parse PE header to get size_of_image
    let e_lfanew = u32::from_le_bytes(header_data[60..64].try_into().unwrap()) as usize;
    if e_lfanew + 4 + 20 + 60 > header_data.len() {
        bail!("PE header extends beyond first memory region");
    }

    let optional_header_offset = e_lfanew + 4 + 20;
    let size_of_image = u32::from_le_bytes(
        header_data[optional_header_offset + 56..optional_header_offset + 60]
            .try_into()
            .unwrap(),
    ) as u64;

    println!(
        "Size of image: {} bytes ({:.1} MB)",
        size_of_image,
        size_of_image as f64 / 1024.0 / 1024.0
    );

    // Allocate buffer for the entire image
    let mut image = vec![0u8; size_of_image as usize];

    // Copy memory regions into the buffer
    let mut bytes_copied: u64 = 0;
    for region in &regions {
        let offset = region.base_address - base_addr;
        if offset >= size_of_image {
            continue;
        }

        let end = (offset + region.bytes.len() as u64).min(size_of_image);
        let copy_len = (end - offset) as usize;
        image[offset as usize..offset as usize + copy_len].copy_from_slice(&region.bytes[..copy_len]);
        bytes_copied += copy_len as u64;
    }

    println!(
        "Copied {} bytes ({:.1} MB), {} gaps",
        bytes_copied,
        bytes_copied as f64 / 1024.0 / 1024.0,
        size_of_image.saturating_sub(bytes_copied)
    );

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        input.with_file_name(format!("{}_extracted.exe", stem))
    });

    // Write the extracted PE
    fs::write(&output_path, &image)
        .with_context(|| format!("Failed to write: {:?}", output_path))?;

    println!("\nExtracted PE written to: {:?}", output_path);
    println!("Note: This is a memory image - data directories may have invalid RVAs");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_minidump_to_exe_missing_file() {
        let result = handle_minidump_to_exe(
            Path::new("/nonexistent/dump.dmp"),
            None,
            "0x140000000",
        );
        assert!(result.is_err());
    }
}
