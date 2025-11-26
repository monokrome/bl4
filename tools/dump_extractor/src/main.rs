use anyhow::{Context, Result};
use minidump::{Minidump, MinidumpMemory64List, MinidumpMemoryList};
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <minidump.dmp> [output.raw]", args[0]);
        eprintln!("\nExtracts raw memory from a minidump with VA mapping.");
        eprintln!("Creates output.raw (memory) and output.raw.map (VA mappings)");
        return Ok(());
    }

    let input_path = &args[1];
    let output_path = args.get(2).map(|s| s.as_str()).unwrap_or("extracted.raw");

    println!("Opening minidump: {}", input_path);

    let dump = Minidump::read_path(input_path)
        .context("Failed to read minidump")?;

    println!("Minidump opened successfully");

    // Print available streams
    println!("\nAvailable streams:");
    for stream in dump.all_streams() {
        println!("  {:?}", stream);
    }

    // Try to get Memory64List first (full memory dumps)
    let memory_result = dump.get_stream::<MinidumpMemory64List>();

    match memory_result {
        Ok(memory64) => {
            println!("\nFound Memory64List with {} regions", memory64.iter().count());
            extract_memory64(&memory64, output_path)?;
        }
        Err(e) => {
            println!("No Memory64List: {:?}", e);

            // Try regular MemoryList
            match dump.get_stream::<MinidumpMemoryList>() {
                Ok(memory) => {
                    println!("\nFound MemoryList with {} regions", memory.iter().count());
                    extract_memory(&memory, output_path)?;
                }
                Err(e2) => {
                    eprintln!("No MemoryList either: {:?}", e2);
                    eprintln!("\nThis minidump doesn't contain extractable memory regions.");
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

fn extract_memory64(memory: &MinidumpMemory64List, output_path: &str) -> Result<()> {
    let raw_path = output_path;
    let map_path = format!("{}.map", output_path);

    let mut raw_file = BufWriter::new(File::create(raw_path)?);
    let mut map_file = BufWriter::new(File::create(&map_path)?);

    writeln!(map_file, "# Memory region mapping")?;
    writeln!(map_file, "# Format: VA_START VA_END SIZE RAW_FILE_OFFSET")?;
    writeln!(map_file, "#")?;

    let mut raw_offset: u64 = 0;
    let mut total_size: u64 = 0;
    let mut region_count = 0;

    for region in memory.iter() {
        let va_start = region.base_address;
        let size = region.size;
        let va_end = va_start + size;

        // Write mapping
        writeln!(
            map_file,
            "0x{:016x} 0x{:016x} {:12} 0x{:012x}",
            va_start, va_end, size, raw_offset
        )?;

        // Write raw data
        raw_file.write_all(region.bytes)?;

        raw_offset += size;
        total_size += size;
        region_count += 1;

        if region_count <= 5 || region_count % 1000 == 0 {
            println!(
                "  Region {}: VA 0x{:016x} - 0x{:016x} ({} bytes)",
                region_count, va_start, va_end, size
            );
        }
    }

    raw_file.flush()?;
    map_file.flush()?;

    println!("\nExtraction complete:");
    println!("  Regions: {}", region_count);
    println!("  Total size: {} bytes ({:.2} GB)", total_size, total_size as f64 / 1024.0 / 1024.0 / 1024.0);
    println!("  Raw memory: {}", raw_path);
    println!("  VA mapping: {}", map_path);

    Ok(())
}

fn extract_memory(memory: &MinidumpMemoryList, output_path: &str) -> Result<()> {
    let raw_path = output_path;
    let map_path = format!("{}.map", output_path);

    let mut raw_file = BufWriter::new(File::create(raw_path)?);
    let mut map_file = BufWriter::new(File::create(&map_path)?);

    writeln!(map_file, "# Memory region mapping")?;
    writeln!(map_file, "# Format: VA_START VA_END SIZE RAW_FILE_OFFSET")?;
    writeln!(map_file, "#")?;

    let mut raw_offset: u64 = 0;
    let mut total_size: u64 = 0;
    let mut region_count = 0;

    for region in memory.iter() {
        let va_start = region.base_address;
        let size = region.size;
        let va_end = va_start + size;

        writeln!(
            map_file,
            "0x{:016x} 0x{:016x} {:12} 0x{:012x}",
            va_start, va_end, size, raw_offset
        )?;

        raw_file.write_all(region.bytes)?;

        raw_offset += size;
        total_size += size;
        region_count += 1;

        if region_count <= 5 {
            println!(
                "  Region {}: VA 0x{:016x} - 0x{:016x} ({} bytes)",
                region_count, va_start, va_end, size
            );
        }
    }

    raw_file.flush()?;
    map_file.flush()?;

    println!("\nExtraction complete:");
    println!("  Regions: {}", region_count);
    println!("  Total size: {} bytes ({:.2} GB)", total_size, total_size as f64 / 1024.0 / 1024.0 / 1024.0);
    println!("  Raw memory: {}", raw_path);
    println!("  VA mapping: {}", map_path);

    Ok(())
}
