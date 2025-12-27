//! Live Process Memory Source
//!
//! Memory source implementation for reading from a live BL4 process.

use super::{MemoryRegion, MemorySource};
use crate::memory::pattern::scan_pattern_fast;

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

// SAFETY: Windows HANDLEs are process-wide and can be safely used from any thread.
unsafe impl Send for Bl4Process {}
unsafe impl Sync for Bl4Process {}

impl MemorySource for Bl4Process {
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.handle
            .copy_address(address, &mut buffer)
            .with_context(|| format!("Failed to read {} bytes at {:#x}", size, address))?;
        Ok(buffer)
    }

    fn regions(&self) -> &[MemoryRegion] {
        &self.maps
    }

    fn is_live(&self) -> bool {
        true
    }
}

impl Bl4Process {
    /// Attach to a running BL4 process
    pub fn attach() -> Result<Self> {
        let pid = find_bl4_process()?;
        let handle = (pid as process_memory::Pid)
            .try_into_process_handle()
            .context("Failed to attach to process. Try running with sudo.")?;

        let maps = parse_maps(pid)?;

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
    pub fn read_bytes_direct(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        self.handle
            .copy_address(address, &mut buffer)
            .with_context(|| format!("Failed to read {} bytes at {:#x}", size, address))?;
        Ok(buffer)
    }

    /// Read a u64 from process memory
    pub fn read_u64(&self, address: usize) -> Result<u64> {
        let bytes = self.read_bytes_direct(address, 8)?;
        Ok(LE::read_u64(&bytes))
    }

    /// Read a u32 from process memory
    pub fn read_u32(&self, address: usize) -> Result<u32> {
        let bytes = self.read_bytes_direct(address, 4)?;
        Ok(LE::read_u32(&bytes))
    }

    /// Read a pointer (usize) from process memory
    pub fn read_ptr(&self, address: usize) -> Result<usize> {
        let bytes = self.read_bytes_direct(address, 8)?;
        Ok(LE::read_u64(&bytes) as usize)
    }

    /// Read a null-terminated string from process memory
    pub fn read_cstring(&self, address: usize, max_len: usize) -> Result<String> {
        let bytes = self.read_bytes_direct(address, max_len)?;
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

    /// Scan memory for a byte pattern (SIMD-accelerated Boyer-Moore style)
    pub fn scan_pattern(&self, pattern: &[u8], mask: &[u8]) -> Result<Vec<usize>> {
        let mut results = Vec::new();

        for region in &self.maps {
            if !region.is_readable() || region.size() > 100 * 1024 * 1024 {
                continue;
            }

            if let Ok(data) = self.read_bytes_direct(region.start, region.size()) {
                for offset in scan_pattern_fast(&data, pattern, mask) {
                    results.push(region.start + offset);
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

/// Find running Borderlands 4 process
pub fn find_bl4_process() -> Result<u32> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut candidates: Vec<(u32, u64)> = Vec::new();

    for process in system.processes().values() {
        let pid = process.pid().as_u32();
        let memory = process.memory();

        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/cmdline", pid)) {
            if cmdline.contains("Borderlands4.exe") || cmdline.contains("borderlands4.exe") {
                let tgid = get_tgid(pid).unwrap_or(pid);

                if memory > 1_000_000_000 {
                    candidates.push((tgid, memory));
                } else {
                    candidates.push((tgid, memory));
                }
            }
        }

        let name = process.name().to_string_lossy();
        if name.contains("Borderlands4") || name.contains("borderlands4") {
            let tgid = get_tgid(pid).unwrap_or(pid);
            candidates.push((tgid, memory));
        }
    }

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
pub fn get_tgid(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if line.starts_with("Tgid:") {
            return line.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

/// Parse /proc/pid/maps to get memory regions
pub fn parse_maps(pid: u32) -> Result<Vec<MemoryRegion>> {
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
