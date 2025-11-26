// Quick minidump memory extractor
// Run with: cargo script tools/extract_minidump.rs <input.dump> <output.raw>

use std::env;
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input.dump> <output.raw>", args[0]);
        return Ok(());
    }

    let mut file = File::open(&args[1])?;
    let mut header = [0u8; 32];
    file.read_exact(&mut header)?;

    // Verify MDMP signature
    if &header[0..4] != b"MDMP" {
        eprintln!("Not a valid minidump file");
        return Ok(());
    }

    let num_streams = u32::from_le_bytes(header[8..12].try_into()?);
    let stream_dir_rva = u32::from_le_bytes(header[12..16].try_into()?);

    println!("Streams: {}, Directory RVA: 0x{:x}", num_streams, stream_dir_rva);

    // Find Memory64ListStream (type 9)
    file.seek(SeekFrom::Start(stream_dir_rva as u64))?;

    let mut memory64_rva = 0u32;
    let mut memory64_size = 0u32;

    for i in 0..num_streams {
        let mut entry = [0u8; 12];
        file.read_exact(&mut entry)?;

        let stream_type = u32::from_le_bytes(entry[0..4].try_into()?);
        let data_size = u32::from_le_bytes(entry[4..8].try_into()?);
        let rva = u32::from_le_bytes(entry[8..12].try_into()?);

        println!("Stream {}: type={}, size={}, rva=0x{:x}", i, stream_type, data_size, rva);

        if stream_type == 9 {
            memory64_rva = rva;
            memory64_size = data_size;
        }
    }

    if memory64_rva == 0 {
        eprintln!("No Memory64ListStream found");
        return Ok(());
    }

    // Parse Memory64ListStream
    file.seek(SeekFrom::Start(memory64_rva as u64))?;

    let mut mem_header = [0u8; 16];
    file.read_exact(&mut mem_header)?;

    let num_regions = u64::from_le_bytes(mem_header[0..8].try_into()?);
    let base_rva = u64::from_le_bytes(mem_header[8..16].try_into()?);

    println!("\nMemory64ListStream: {} regions, base RVA: 0x{:x}", num_regions, base_rva);

    // Read region descriptors
    let mut regions = Vec::new();
    for _ in 0..num_regions {
        let mut desc = [0u8; 16];
        file.read_exact(&mut desc)?;

        let start_addr = u64::from_le_bytes(desc[0..8].try_into()?);
        let data_size = u64::from_le_bytes(desc[8..16].try_into()?);
        regions.push((start_addr, data_size));
    }

    // Create output file and write raw memory with VA prefix for each region
    let mut output = File::create(&args[2])?;
    let mut current_rva = base_rva;

    println!("\nExtracting {} memory regions...", regions.len());

    // Also write a mapping file
    let mut mapping = File::create(format!("{}.map", &args[2]))?;
    writeln!(mapping, "# Virtual Address -> File Offset mapping")?;
    writeln!(mapping, "# VA_Start VA_End Size FileOffset")?;

    let mut file_offset = 0u64;
    for (i, (va_start, size)) in regions.iter().enumerate() {
        if i < 10 || i >= regions.len() - 3 {
            println!("  Region {}: VA 0x{:016x} - 0x{:016x} ({} bytes)",
                     i, va_start, va_start + size, size);
        } else if i == 10 {
            println!("  ... {} more regions ...", regions.len() - 13);
        }

        writeln!(mapping, "0x{:016x} 0x{:016x} {} 0x{:x}",
                 va_start, va_start + size, size, file_offset)?;

        // Read from minidump and write to output
        file.seek(SeekFrom::Start(current_rva))?;
        let mut buf = vec![0u8; *size as usize];
        file.read_exact(&mut buf)?;
        output.write_all(&buf)?;

        current_rva += size;
        file_offset += size;
    }

    println!("\nExtracted to {} with mapping in {}.map", args[2], args[2]);

    Ok(())
}
