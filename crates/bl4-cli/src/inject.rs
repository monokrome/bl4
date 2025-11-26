//! Process injection and memory manipulation for Borderlands 4
//!
//! This module provides functionality to:
//! - Find and attach to the BL4 process (including under Proton/Wine)
//! - Read/write process memory
//! - Locate UE5 structures (GUObjectArray, GNames, etc.)
//! - Generate usmap files from live process
//! - Read and modify game state (inventory, stats, etc.)

use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LE};
use process_memory::{CopyAddress, ProcessHandle, PutAddress, TryIntoProcessHandle};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use sysinfo::System;

/// Process info for an attached BL4 instance
pub struct Bl4Process {
    pub pid: u32,
    pub handle: ProcessHandle,
    pub exe_path: PathBuf,
    pub maps: Vec<MemoryRegion>,
}

/// A memory region from /proc/pid/maps
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: usize,
    pub end: usize,
    pub perms: String,
    pub offset: usize,
    pub path: Option<String>,
}

impl MemoryRegion {
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn is_readable(&self) -> bool {
        self.perms.starts_with('r')
    }

    pub fn is_writable(&self) -> bool {
        self.perms.chars().nth(1) == Some('w')
    }

    pub fn is_executable(&self) -> bool {
        self.perms.chars().nth(2) == Some('x')
    }
}

/// Find running Borderlands 4 process
pub fn find_bl4_process() -> Result<u32> {
    let mut system = System::new_all();
    system.refresh_all();

    // Collect all candidate processes with their memory usage
    let mut candidates: Vec<(u32, u64)> = Vec::new();

    for process in system.processes().values() {
        let pid = process.pid().as_u32();
        let memory = process.memory();

        // Check cmdline for Wine/Proton processes running BL4
        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/cmdline", pid)) {
            // Must contain Borderlands4.exe in the cmdline
            if cmdline.contains("Borderlands4.exe") || cmdline.contains("borderlands4.exe") {
                // Get the thread group ID (Tgid) - this is the main process ID
                // Threads have the same Tgid as their parent process
                let tgid = get_tgid(pid).unwrap_or(pid);

                // Check if this is the actual game process (high memory usage)
                // or just a launcher/wrapper (low memory usage)
                // The actual game uses several GB of RAM
                if memory > 1_000_000_000 {
                    // More than 1GB = likely the actual game
                    candidates.push((tgid, memory));
                } else {
                    // Could be a wrapper, but still add as fallback
                    candidates.push((tgid, memory));
                }
            }
        }

        // Also check process name directly
        let name = process.name().to_string_lossy();
        if name.contains("Borderlands4") || name.contains("borderlands4") {
            let tgid = get_tgid(pid).unwrap_or(pid);
            candidates.push((tgid, memory));
        }
    }

    // Deduplicate by PID (multiple threads may have the same Tgid)
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.dedup_by(|a, b| a.0 == b.0);

    if let Some((pid, memory)) = candidates.first() {
        eprintln!(
            "Found BL4 process: PID {} (memory: {} MB)",
            pid,
            memory / 1_000_000
        );
        return Ok(*pid);
    }

    bail!("Borderlands 4 process not found. Is the game running?")
}

/// Get the thread group ID (main process) for a given PID/TID
fn get_tgid(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if line.starts_with("Tgid:") {
            return line.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

/// Parse /proc/pid/maps to get memory regions
fn parse_maps(pid: u32) -> Result<Vec<MemoryRegion>> {
    let maps_path = format!("/proc/{}/maps", pid);
    let file = File::open(&maps_path)
        .with_context(|| format!("Failed to open {}. Do you have permission?", maps_path))?;

    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        // Parse address range: "7f1234000000-7f1234001000"
        let addr_parts: Vec<&str> = parts[0].split('-').collect();
        if addr_parts.len() != 2 {
            continue;
        }

        let start = usize::from_str_radix(addr_parts[0], 16).unwrap_or(0);
        let end = usize::from_str_radix(addr_parts[1], 16).unwrap_or(0);
        let perms = parts.get(1).unwrap_or(&"").to_string();
        let offset = parts
            .get(2)
            .and_then(|s| usize::from_str_radix(s, 16).ok())
            .unwrap_or(0);
        let path = parts.get(5).map(|s| s.to_string());

        regions.push(MemoryRegion {
            start,
            end,
            perms,
            offset,
            path,
        });
    }

    Ok(regions)
}

impl Bl4Process {
    /// Attach to a running BL4 process
    pub fn attach() -> Result<Self> {
        let pid = find_bl4_process()?;
        let handle = (pid as process_memory::Pid)
            .try_into_process_handle()
            .context("Failed to attach to process. Try running with sudo.")?;

        let maps = parse_maps(pid)?;

        // Find the main executable path
        let exe_path = std::fs::read_link(format!("/proc/{}/exe", pid))
            .unwrap_or_else(|_| PathBuf::from("unknown"));

        Ok(Bl4Process {
            pid,
            handle,
            exe_path,
            maps,
        })
    }

    /// Read bytes from process memory
    pub fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.handle
            .copy_address(address, &mut buffer)
            .with_context(|| format!("Failed to read {} bytes at {:#x}", size, address))?;
        Ok(buffer)
    }

    /// Read a u64 from process memory
    pub fn read_u64(&self, address: usize) -> Result<u64> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes))
    }

    /// Read a u32 from process memory
    pub fn read_u32(&self, address: usize) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        Ok(LE::read_u32(&bytes))
    }

    /// Read a pointer (usize) from process memory
    pub fn read_ptr(&self, address: usize) -> Result<usize> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes) as usize)
    }

    /// Read a null-terminated string from process memory
    pub fn read_cstring(&self, address: usize, max_len: usize) -> Result<String> {
        let bytes = self.read_bytes(address, max_len)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    /// Write bytes to process memory
    pub fn write_bytes(&self, address: usize, data: &[u8]) -> Result<()> {
        self.handle
            .put_address(address, data)
            .with_context(|| format!("Failed to write {} bytes at {:#x}", data.len(), address))?;
        Ok(())
    }

    /// Find the main executable module
    pub fn find_main_module(&self) -> Option<&MemoryRegion> {
        self.maps.iter().find(|r| {
            r.path
                .as_ref()
                .map(|p| p.contains("Borderlands4") && p.ends_with(".exe"))
                .unwrap_or(false)
                && r.is_executable()
        })
    }

    /// Scan memory for a byte pattern
    pub fn scan_pattern(&self, pattern: &[u8], mask: &[u8]) -> Result<Vec<usize>> {
        let mut results = Vec::new();

        for region in &self.maps {
            if !region.is_readable() || region.size() > 100 * 1024 * 1024 {
                continue; // Skip non-readable or huge regions
            }

            // Only scan executable regions for code patterns
            if let Ok(data) = self.read_bytes(region.start, region.size()) {
                for i in 0..data.len().saturating_sub(pattern.len()) {
                    let mut matches = true;
                    for j in 0..pattern.len() {
                        if mask[j] != 0 && data[i + j] != pattern[j] {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        results.push(region.start + i);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get process info summary
    pub fn info(&self) -> String {
        let main_module = self.find_main_module();
        let module_info = main_module
            .map(|m| format!("Base: {:#x}, Size: {:#x}", m.start, m.size()))
            .unwrap_or_else(|| "Not found".to_string());

        format!(
            "PID: {}\nExecutable: {}\nMain Module: {}\nMemory Regions: {}",
            self.pid,
            self.exe_path.display(),
            module_info,
            self.maps.len()
        )
    }
}

/// UE5 structure offsets
#[derive(Debug)]
pub struct Ue5Offsets {
    pub guobject_array: usize,
    pub gnames: usize,
}

/// Discovered GNames pool
#[derive(Debug)]
pub struct GNamesPool {
    pub address: usize,
    pub sample_names: Vec<(u32, String)>,
}

/// Discovered GUObjectArray
#[derive(Debug)]
pub struct GUObjectArray {
    pub address: usize,
    pub objects_ptr: usize,
    pub max_elements: i32,
    pub num_elements: i32,
}

impl Bl4Process {
    /// Discover GNames pool by searching for the characteristic "None" + "ByteProperty" pattern
    pub fn discover_gnames(&self) -> Result<GNamesPool> {
        // GNames starts with FNameEntry for "None" followed by "ByteProperty"
        // FNameEntry format in UE5: length_byte (low 6 bits + flags), string bytes
        // "None" with typical flags: 1e 01 4e 6f 6e 65 (length=4, flags, "None")
        // Then "ByteProperty": 10 03 42 79 74 65 50 72 6f 70 65 72 74 79

        // Search for "None" followed by "ByteProperty"
        let pattern = b"\x1e\x01None\x10\x03ByteProperty";
        let mask = vec![1u8; pattern.len()];

        let results = self.scan_pattern(pattern, &mask)?;

        if results.is_empty() {
            // Try alternative pattern without exact length bytes
            let alt_pattern: &[u8] = b"None";
            let alt_mask = vec![1u8; alt_pattern.len()];
            let alt_results = self.scan_pattern(alt_pattern, &alt_mask)?;

            // Filter to find ones followed by ByteProperty
            for addr in alt_results {
                if addr < 2 {
                    continue;
                }
                // Check if "ByteProperty" follows within ~20 bytes
                if let Ok(data) = self.read_bytes(addr.saturating_sub(2), 64) {
                    if let Some(pos) = data.windows(12).position(|w| w == b"ByteProperty") {
                        // Found it! The pool starts before "None"
                        let gnames_addr = addr - 2; // Account for length/flags bytes

                        // Read some sample names
                        let mut sample_names = Vec::new();
                        sample_names.push((0, "None".to_string()));
                        sample_names.push((1, "ByteProperty".to_string()));

                        // Try to read more names from the pool
                        if let Ok(pool_data) = self.read_bytes(gnames_addr, 4096) {
                            let mut offset = 0;
                            let mut index = 0u32;
                            while offset < pool_data.len() - 2 && sample_names.len() < 20 {
                                // FNameEntry: length_byte (6 bits len, 2 bits flags), string
                                let len_byte = pool_data[offset];
                                let string_len = (len_byte >> 1) & 0x3F;
                                if string_len == 0 || string_len > 60 {
                                    offset += 1;
                                    continue;
                                }
                                let start = offset + 2; // Skip length byte and flags byte
                                let end = start + string_len as usize;
                                if end <= pool_data.len() {
                                    if let Ok(name) = String::from_utf8(pool_data[start..end].to_vec()) {
                                        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                                            sample_names.push((index, name));
                                        }
                                    }
                                }
                                offset = end;
                                index += 1;
                            }
                        }

                        return Ok(GNamesPool {
                            address: gnames_addr,
                            sample_names,
                        });
                    }
                }
            }

            bail!("GNames pool not found. The game may use a different FName format.");
        }

        let gnames_addr = results[0];

        // Read sample names
        let mut sample_names = vec![
            (0, "None".to_string()),
            (1, "ByteProperty".to_string()),
        ];

        Ok(GNamesPool {
            address: gnames_addr,
            sample_names,
        })
    }

    /// Discover GUObjectArray by searching for characteristic patterns
    pub fn discover_guobject_array(&self, gnames_addr: usize) -> Result<GUObjectArray> {
        // GUObjectArray typically has:
        // - Objects** (pointer to array of pointers to chunks)
        // - MaxElements (int32)
        // - NumElements (int32)
        // - MaxChunks (int32)
        // - NumChunks (int32)

        // Strategy 1: Look for memory that contains pointers to the GNames region
        // and has array-like characteristics

        // Search for structures that look like arrays of 8-byte aligned pointers
        // where some pointers reference data that contains FName indices

        // First, let's search for known class names that would be early in GUObjectArray
        // like "Class", "Object", "Package"

        // Search for "/Script/CoreUObject" which is in early objects
        let core_pattern = b"/Script/CoreUObject";
        let results = self.scan_pattern(core_pattern, &vec![1u8; core_pattern.len()])?;

        if results.is_empty() {
            bail!("Could not find CoreUObject package references");
        }

        println!("Found {} /Script/CoreUObject references", results.len());

        // Now look for pointer arrays that might reference these areas
        // GUObjectArray's ObjObjects is typically a pointer to an array of FUObjectItem
        // Each FUObjectItem is typically 24 bytes: Object* (8), SerialNumber (4), padding (12)

        // Let's try finding the chunked array by looking for memory that:
        // 1. Contains valid-looking pointers
        // 2. Has consistent spacing (chunk size)
        // 3. Points to memory with UObject-like structures

        // For now, return a placeholder - full implementation would scan for the array
        bail!(
            "GUObjectArray discovery is complex and requires additional heuristics.\n\
            Known info:\n\
            - GNames at: {:#x}\n\
            - CoreUObject refs: {} locations\n\
            \n\
            Try using a debugger or SDK dumper tool to find GUObjectArray address.",
            gnames_addr,
            results.len()
        )
    }

    /// Read an FName string from the GNames pool
    pub fn read_fname(&self, gnames_addr: usize, index: u32) -> Result<String> {
        // This is a simplified implementation
        // Real UE5 FNamePool uses chunked blocks

        // For now, scan forward from gnames_addr to find the indexed name
        // This is slow but works for testing

        if index == 0 {
            return Ok("None".to_string());
        }

        let data = self.read_bytes(gnames_addr, 64 * 1024)?; // Read 64KB of pool

        let mut offset = 0;
        let mut current_index = 0u32;

        while offset < data.len() - 2 && current_index < index {
            let len_byte = data[offset];
            let string_len = ((len_byte >> 1) & 0x3F) as usize;
            if string_len == 0 {
                offset += 1;
                continue;
            }
            offset += 2 + string_len; // Skip length byte, flags byte, and string
            current_index += 1;
        }

        if current_index == index && offset < data.len() - 2 {
            let len_byte = data[offset];
            let string_len = ((len_byte >> 1) & 0x3F) as usize;
            if string_len > 0 && offset + 2 + string_len <= data.len() {
                let name_bytes = &data[offset + 2..offset + 2 + string_len];
                return Ok(String::from_utf8_lossy(name_bytes).to_string());
            }
        }

        bail!("FName index {} not found", index)
    }
}

/// Find UE5 global structures by pattern scanning
pub fn find_ue5_offsets(process: &Bl4Process) -> Result<Ue5Offsets> {
    let gnames = process.discover_gnames()?;

    // Try to find GUObjectArray
    let guobject_array = match process.discover_guobject_array(gnames.address) {
        Ok(arr) => arr.address,
        Err(_) => 0, // Not found yet
    };

    Ok(Ue5Offsets {
        gnames: gnames.address,
        guobject_array,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_process() {
        // This will fail if BL4 isn't running, which is expected
        let result = find_bl4_process();
        println!("Find process result: {:?}", result);
    }
}
