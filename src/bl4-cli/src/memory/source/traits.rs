//! Memory Source Trait
//!
//! Core abstraction for reading memory from various sources.

use super::MemoryRegion;
use anyhow::Result;
use byteorder::{ByteOrder, LE};

/// Trait for reading memory from various sources (live process, dump file, etc.)
pub trait MemorySource: Send + Sync {
    /// Read bytes from a virtual address
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>>;

    /// Get the list of memory regions
    fn regions(&self) -> &[MemoryRegion];

    /// Check if this is a live (writable) source
    fn is_live(&self) -> bool;

    /// Read a u64 from memory
    fn read_u64(&self, address: usize) -> Result<u64> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes))
    }

    /// Read a u32 from memory
    fn read_u32(&self, address: usize) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        Ok(LE::read_u32(&bytes))
    }

    /// Read a pointer (usize) from memory
    fn read_ptr(&self, address: usize) -> Result<usize> {
        let bytes = self.read_bytes(address, 8)?;
        Ok(LE::read_u64(&bytes) as usize)
    }

    /// Read a null-terminated string from memory
    fn read_cstring(&self, address: usize, max_len: usize) -> Result<String> {
        let bytes = self.read_bytes(address, max_len)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).to_string())
    }

    /// Find a region containing the given address
    fn find_region(&self, address: usize) -> Option<&MemoryRegion> {
        self.regions()
            .iter()
            .find(|r| address >= r.start && address < r.end)
    }

    /// Check if an address is readable
    fn is_readable(&self, address: usize) -> bool {
        self.find_region(address)
            .map(|r| r.is_readable())
            .unwrap_or(false)
    }
}
