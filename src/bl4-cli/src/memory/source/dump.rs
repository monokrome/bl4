//! Dump File Memory Source
//!
//! Memory source implementation for reading from dump files (MDMP and gcore formats).

use super::{MemoryRegion, MemorySource};
use crate::memory::constants::*;

use anyhow::{bail, Context, Result};
use byteorder::{ByteOrder, LE};
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Memory dump file source
///
/// Supports Linux gcore dumps where file offset ≈ virtual address for
/// Wine/Proton processes with the main executable loaded at 0x140000000.
pub struct DumpFile {
    /// Memory-mapped dump file
    mmap: Mmap,
    /// Virtual address regions parsed from dump or maps file
    regions: Vec<MemoryRegion>,
    /// Base address offset (file_offset = va - base_offset for linear dumps)
    base_offset: usize,
    /// Path to the dump file
    pub path: PathBuf,
}

impl DumpFile {
    /// MDMP signature "MDMP" in little-endian
    const MDMP_SIGNATURE: u32 = 0x504D444D; // "MDMP"

    /// MDMP stream types
    const MEMORY_64_LIST_STREAM: u32 = 9;

    /// Open a memory dump file
    ///
    /// Supports:
    /// - Windows Minidump (MDMP) format - auto-detected by "MDMP" signature
    /// - Raw/gcore dumps - file offset ≈ virtual address
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file =
            File::open(&path).with_context(|| format!("Failed to open dump file: {:?}", path))?;

        let mmap = unsafe { Mmap::map(&file) }
            .with_context(|| format!("Failed to mmap dump file: {:?}", path))?;

        eprintln!(
            "Opened dump file: {:?} ({} MB)",
            path,
            mmap.len() / 1_000_000
        );

        // Check for MDMP signature
        if mmap.len() >= 4 && LE::read_u32(&mmap[0..4]) == Self::MDMP_SIGNATURE {
            eprintln!("Detected Windows Minidump (MDMP) format");
            return Self::parse_mdmp(mmap, path);
        }

        // Try to find an accompanying .maps file
        let maps_path = path.with_extension("maps");
        let regions = if maps_path.exists() {
            Self::parse_maps_file(&maps_path)?
        } else {
            // Create synthetic regions based on typical BL4 layout
            Self::create_default_regions(mmap.len())
        };

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0, // Linear mapping: file_offset == VA
            path,
        })
    }

    /// Parse Windows Minidump format
    fn parse_mdmp(mmap: Mmap, path: PathBuf) -> Result<Self> {
        if mmap.len() < 32 {
            bail!("MDMP file too small for header");
        }

        let num_streams = LE::read_u32(&mmap[0x08..0x0C]) as usize;
        let stream_dir_rva = LE::read_u32(&mmap[0x0C..0x10]) as usize;

        eprintln!(
            "MDMP: {} streams, directory at {:#x}",
            num_streams, stream_dir_rva
        );

        let mut memory_ranges: Vec<(u64, u64, u64)> = Vec::new();

        for i in 0..num_streams {
            let entry_offset = stream_dir_rva + i * 12;
            if entry_offset + 12 > mmap.len() {
                break;
            }

            let stream_type = LE::read_u32(&mmap[entry_offset..entry_offset + 4]);
            let data_size = LE::read_u32(&mmap[entry_offset + 4..entry_offset + 8]) as usize;
            let rva = LE::read_u32(&mmap[entry_offset + 8..entry_offset + 12]) as usize;

            if stream_type == Self::MEMORY_64_LIST_STREAM {
                eprintln!(
                    "Found Memory64ListStream at RVA {:#x}, size {}",
                    rva, data_size
                );

                if rva + 16 > mmap.len() {
                    bail!("Memory64ListStream header out of bounds");
                }

                let num_ranges = LE::read_u64(&mmap[rva..rva + 8]) as usize;
                let base_rva = LE::read_u64(&mmap[rva + 8..rva + 16]);

                eprintln!(
                    "Memory64List: {} ranges, data starts at RVA {:#x}",
                    num_ranges, base_rva
                );

                let mut current_file_offset = base_rva;

                for j in 0..num_ranges {
                    let desc_offset = rva + 16 + j * 16;
                    if desc_offset + 16 > mmap.len() {
                        break;
                    }

                    let start_addr = LE::read_u64(&mmap[desc_offset..desc_offset + 8]);
                    let range_size = LE::read_u64(&mmap[desc_offset + 8..desc_offset + 16]);

                    memory_ranges.push((start_addr, range_size, current_file_offset));
                    current_file_offset += range_size;
                }

                eprintln!("Parsed {} memory ranges from MDMP", memory_ranges.len());
                break;
            }
        }

        if memory_ranges.is_empty() {
            bail!("No Memory64ListStream found in MDMP - dump may be incomplete");
        }

        let regions: Vec<MemoryRegion> = memory_ranges
            .iter()
            .map(|(base, size, file_offset)| MemoryRegion {
                start: *base as usize,
                end: (*base + *size) as usize,
                perms: "rw-p".to_string(),
                offset: *file_offset as usize,
                path: None,
            })
            .collect();

        // Print diagnostic info about key regions
        let gobjects_va = PE_IMAGE_BASE + GOBJECTS_OFFSET;
        let gnames_va = PE_IMAGE_BASE + GNAMES_OFFSET;

        eprintln!(
            "Memory ranges near SDK GObjects offset ({:#x}):",
            gobjects_va
        );
        for region in &regions {
            if region.end > gobjects_va.saturating_sub(0x100000)
                && region.start < gobjects_va.saturating_add(0x100000)
            {
                eprintln!(
                    "  {:#x}-{:#x} (size {:#x}, file offset {:#x})",
                    region.start,
                    region.end,
                    region.end - region.start,
                    region.offset
                );
            }
        }

        for region in &regions {
            if gobjects_va >= region.start && gobjects_va < region.end {
                eprintln!(
                    "GObjects ({:#x}) found in region {:#x}-{:#x}, file offset {:#x}",
                    gobjects_va, region.start, region.end, region.offset
                );
            }
            if gnames_va >= region.start && gnames_va < region.end {
                eprintln!(
                    "GNames ({:#x}) found in region {:#x}-{:#x}, file offset {:#x}",
                    gnames_va, region.start, region.end, region.offset
                );
            }
        }

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0,
            path,
        })
    }

    /// Open a dump with an explicit maps file
    pub fn open_with_maps<P: AsRef<Path>>(dump_path: P, maps_path: P) -> Result<Self> {
        let dump_path = dump_path.as_ref().to_path_buf();
        let file = File::open(&dump_path)
            .with_context(|| format!("Failed to open dump file: {:?}", dump_path))?;

        let mmap = unsafe { Mmap::map(&file) }
            .with_context(|| format!("Failed to mmap dump file: {:?}", dump_path))?;

        let regions = Self::parse_maps_file(maps_path.as_ref())?;

        eprintln!(
            "Opened dump file: {:?} ({} MB) with {} regions",
            dump_path,
            mmap.len() / 1_000_000,
            regions.len()
        );

        Ok(DumpFile {
            mmap,
            regions,
            base_offset: 0,
            path: dump_path,
        })
    }

    /// Parse a maps file (supports both /proc/pid/maps and custom dump format)
    fn parse_maps_file(path: &Path) -> Result<Vec<MemoryRegion>> {
        let file =
            File::open(path).with_context(|| format!("Failed to open maps file: {:?}", path))?;

        let reader = BufReader::new(file);
        let mut regions = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            if parts[0].starts_with("0x") {
                // Custom dump format: 0xSTART 0xEND SIZE FILE_OFFSET
                if parts.len() < 4 {
                    continue;
                }

                let start =
                    usize::from_str_radix(parts[0].trim_start_matches("0x"), 16).unwrap_or(0);
                let end = usize::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap_or(0);
                let file_offset =
                    usize::from_str_radix(parts[3].trim_start_matches("0x"), 16).unwrap_or(0);

                regions.push(MemoryRegion {
                    start,
                    end,
                    perms: "rw-p".to_string(),
                    offset: file_offset,
                    path: None,
                });
            } else {
                // Linux /proc/pid/maps format
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
        }

        Ok(regions)
    }

    /// Create default regions for a dump without maps info
    fn create_default_regions(dump_size: usize) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                start: 0x140000000,
                end: 0x140001000,
                perms: "r--p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            MemoryRegion {
                start: 0x140001000,
                end: 0x14e61c000,
                perms: "r-xp".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            MemoryRegion {
                start: 0x14e61c000,
                end: 0x15120e000,
                perms: "r--p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            MemoryRegion {
                start: 0x15120e000,
                end: 0x15175c000,
                perms: "rw-p".to_string(),
                offset: 0,
                path: Some("Borderlands4.exe".to_string()),
            },
            MemoryRegion {
                start: 0x15175c000,
                end: dump_size.min(0x800000000000),
                perms: "rw-p".to_string(),
                offset: 0,
                path: None,
            },
        ]
    }

    /// Convert virtual address to file offset
    fn va_to_offset(&self, va: usize) -> Option<usize> {
        for region in &self.regions {
            if va >= region.start && va < region.end {
                let region_offset = va - region.start;
                let file_offset = region.offset + region_offset;
                if file_offset < self.mmap.len() {
                    return Some(file_offset);
                }
            }
        }

        let offset = va.checked_sub(self.base_offset)?;
        if offset < self.mmap.len() {
            Some(offset)
        } else {
            None
        }
    }
}

impl MemorySource for DumpFile {
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        let offset = self
            .va_to_offset(address)
            .ok_or_else(|| anyhow::anyhow!("Address {:#x} out of dump range", address))?;

        if offset + size > self.mmap.len() {
            bail!("Read of {} bytes at {:#x} exceeds dump size", size, address);
        }

        Ok(self.mmap[offset..offset + size].to_vec())
    }

    fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    fn is_live(&self) -> bool {
        false
    }
}
