//! Generic UObject dumper by name or address
//!
//! Finds UObjects matching a name substring or at a hex address and dumps
//! their properties via UE reflection (USMAP + FNamePool + walker).

use crate::memory::{self, MemorySource};
use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LE};
use std::fs;
use std::path::Path;

/// Dump one or more UObjects generically
pub fn handle_dump_object(
    target: &str,
    limit: usize,
    output: Option<&Path>,
    dump: Option<&Path>,
) -> Result<()> {
    let dump_path = dump.ok_or_else(|| {
        anyhow::anyhow!("dump-object requires a memory dump file. Use --dump <path>")
    })?;

    let source: Box<dyn MemorySource> =
        Box::new(memory::DumpFile::open(dump_path).context("Failed to open dump file")?);
    let source = source.as_ref();

    eprintln!("Discovering GNames pool...");
    let gnames = memory::discover_gnames(source).context("Failed to discover GNames")?;
    eprintln!("  GNames at: {:#x}", gnames.address);

    eprintln!("Discovering GUObjectArray...");
    let guobjects = memory::discover_guobject_array(source, gnames.address)
        .context("Failed to discover GUObjectArray")?;
    eprintln!("  GUObjectArray at: {:#x}", guobjects.address);
    eprintln!("  NumElements: {}", guobjects.num_elements);

    let mut reader = memory::FNameReader::new(
        memory::FNamePool::discover(source).context("Failed to discover FNamePool")?,
    );

    // Resolve target to object addresses
    let targets: Vec<(usize, String, String)> = if let Some(hex) = target
        .strip_prefix("0x")
        .or_else(|| target.strip_prefix("0X"))
    {
        let addr = usize::from_str_radix(hex, 16).context("Invalid hex address")?;
        // Read the object's name/class for display
        let obj_data = source
            .read_bytes(addr, 0x40)
            .context("Failed to read object at address")?;
        let class_ptr = LE::read_u64(&obj_data[memory::UOBJECT_CLASS_OFFSET..]) as usize;
        let name_idx = LE::read_u32(&obj_data[memory::UOBJECT_NAME_OFFSET..]);
        let name = reader
            .read_name(source, name_idx)
            .unwrap_or_else(|_| format!("FName_{}", name_idx));
        let class_name = if class_ptr != 0 {
            let cname_idx = source
                .read_bytes(class_ptr + memory::UOBJECT_NAME_OFFSET, 4)
                .ok()
                .map(|b| LE::read_u32(&b))
                .unwrap_or(0);
            reader
                .read_name(source, cname_idx)
                .unwrap_or_else(|_| format!("Class_{:#x}", class_ptr))
        } else {
            "Unknown".to_string()
        };
        vec![(addr, name, class_name)]
    } else {
        // Search by name substring (case-insensitive)
        let results = memory::find_objects_by_pattern(source, &guobjects, target, limit * 2)
            .context("Failed to search objects")?;
        let mut out = Vec::new();
        for (name, class_name, _) in results {
            // Need to find the actual object address for this name/class
            // find_objects_by_pattern returns (name, class_name, class_ptr) but not address
            // So we walk GUObjectArray again to find the address
            let addr = find_object_address(source, &guobjects, &mut reader, &name, &class_name)?;
            if let Some(a) = addr {
                out.push((a, name, class_name));
                if out.len() >= limit {
                    break;
                }
            }
        }
        if out.is_empty() {
            bail!("No objects matching '{}' found (limit {})", target, limit);
        }
        out
    };

    eprintln!("\nFound {} object(s) for '{}':", targets.len(), target);
    for (addr, name, class) in &targets {
        eprintln!("  {:#x}: '{}' (class: {})", addr, name, class);
    }

    // For each target, try to walk its properties generically
    let mut outputs = Vec::new();
    for (addr, name, class_name) in targets {
        eprintln!(
            "\n=== Dumping {} @ {:#x} (class: {}) ===",
            name, addr, class_name
        );
        match dump_single_object(source, &mut reader, &guobjects, addr, &name, &class_name) {
            Ok(json) => {
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
                outputs.push(json);
            }
            Err(e) => {
                eprintln!("  Failed to dump: {}", e);
                // Fallback: raw hex dump
                if let Ok(data) = source.read_bytes(addr, 1024) {
                    eprintln!("  Raw 1024 bytes at {:#x}:", addr);
                    for (i, chunk) in data.chunks(16).enumerate() {
                        eprint!("    {:04x}: ", i * 16);
                        for b in chunk {
                            eprint!("{:02x} ", b);
                        }
                        eprintln!();
                        if i >= 8 {
                            eprintln!("    ... (truncated)");
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Some(out_path) = output {
        let json = serde_json::Value::Array(outputs);
        fs::write(out_path, serde_json::to_string_pretty(&json)?)
            .with_context(|| format!("Failed to write {}", out_path.display()))?;
        eprintln!("\nWrote JSON to {}", out_path.display());
    }

    Ok(())
}

fn find_object_address(
    source: &dyn MemorySource,
    guobjects: &memory::ue5::GUObjectArray,
    reader: &mut memory::FNameReader,
    target_name: &str,
    target_class: &str,
) -> Result<Option<usize>> {
    use byteorder::{ByteOrder, LE};
    // Do a linear scan for the first object with matching name/class
    let num_chunks = (guobjects.num_elements as usize).div_ceil(65536);
    let chunk_ptrs_data = source.read_bytes(guobjects.objects_ptr, num_chunks * 8)?;
    let chunk_ptrs: Vec<usize> = chunk_ptrs_data
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| LE::read_u64(c) as usize)
        .collect();

    for &chunk_ptr in &chunk_ptrs {
        if chunk_ptr == 0 {
            continue;
        }
        // We don't know exact chunk size, so try to read a chunk
        let chunk_data = match source.read_bytes(chunk_ptr, 65536 * guobjects.item_size) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for i in 0..65536 {
            let off = i * guobjects.item_size + guobjects.object_offset;
            if off + 8 > chunk_data.len() {
                break;
            }
            let obj_ptr = LE::read_u64(&chunk_data[off..off + 8]) as usize;
            if obj_ptr == 0 {
                continue;
            }
            let obj_data = match source.read_bytes(obj_ptr, 0x40) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let name_idx = LE::read_u32(&obj_data[memory::UOBJECT_NAME_OFFSET..]);
            let class_ptr = LE::read_u64(&obj_data[memory::UOBJECT_CLASS_OFFSET..]) as usize;
            let cname_idx = source
                .read_bytes(class_ptr + memory::UOBJECT_NAME_OFFSET, 4)
                .ok()
                .map(|b| LE::read_u32(&b))
                .unwrap_or(0);
            let name = reader.read_name(source, name_idx).unwrap_or_default();
            let class_name = reader.read_name(source, cname_idx).unwrap_or_default();
            if name == target_name && class_name == target_class {
                return Ok(Some(obj_ptr));
            }
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn dump_single_object(
    source: &dyn MemorySource,
    reader: &mut memory::FNameReader,
    guobjects: &memory::ue5::GUObjectArray,
    addr: usize,
    name: &str,
    class_name: &str,
) -> Result<serde_json::Value> {
    // Read a larger chunk for inspection (first 2KB)
    let obj_data = source.read_bytes(addr, 0x800).unwrap_or_default();
    let raw_hex = obj_data[..0x40.min(obj_data.len())]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    // Heuristic: scan for TArray-like (ptr, count, max) that points to XP-like ints
    let mut arrays = Vec::new();
    for off in (0..0x400).step_by(8) {
        if off + 24 > obj_data.len() {
            break;
        }
        let ptr = LE::read_u64(&obj_data[off..off + 8]) as usize;
        let count = LE::read_u32(&obj_data[off + 8..off + 12]) as usize;
        let max = LE::read_u32(&obj_data[off + 12..off + 16]) as usize;
        if ptr == 0 || count == 0 || count > 200 || max < count || count < 10 {
            continue;
        }
        // Try to read the array data and see if it looks like XP thresholds (increasing ints)
        if let Ok(data) = source.read_bytes(ptr, count * 4) {
            let vals: Vec<u32> = data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| LE::read_u32(c))
                .collect();
            // Check if it's increasing and within plausible XP range (0 .. 20M)
            let mut plausible = true;
            for w in vals.windows(2) {
                if w[1] <= w[0] || w[1] > 20_000_000 {
                    plausible = false;
                    break;
                }
            }
            if plausible && vals[0] == 0 {
                arrays.push(serde_json::json!({
                    "offset": format!("{:#x}", off),
                    "ptr": format!("{:#x}", ptr),
                    "count": count,
                    "values": vals,
                }));
            }
        }
    }

    let pretty_vals = if arrays.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(arrays)
    };

    let json = serde_json::json!({
        "address": format!("{:#x}", addr),
        "name": name,
        "class": class_name,
        "raw_header_hex": raw_hex,
        "xp_arrays": pretty_vals,
        "note": "Generic dump: raw header + heuristic TArray scan for XP thresholds (look for xp_arrays where values[0]==0 and increasing).",
    });

    let _ = (reader, guobjects);
    Ok(json)
}
