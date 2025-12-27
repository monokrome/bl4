//! NCS Manifest format parsing (`_NCS/`)
//!
//! Manifest files list references to NCS data stores with metadata.

use memchr::memmem;

use crate::{Error, Result, NCS_MANIFEST_MAGIC};

/// Manifest header size (magic + null + count)
pub const HEADER_SIZE: usize = 8;

/// Prefix for all NCS data store filenames
const NEXUS_DATA_PREFIX: &[u8] = b"Nexus-Data-";

/// An entry in an NCS manifest
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The NCS filename (e.g., "Nexus-Data-attribute6.ncs")
    pub filename: String,
}

/// NCS manifest file header and entries
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Number of entries declared in header
    pub entry_count: u16,
    /// Parsed manifest entries
    pub entries: Vec<Entry>,
}

impl Manifest {
    /// Parse an NCS manifest from data
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(Error::DataTooShort {
                needed: HEADER_SIZE,
                actual: data.len(),
            });
        }

        if data[0..5] != NCS_MANIFEST_MAGIC {
            let mut magic = [0u8; 5];
            magic.copy_from_slice(&data[0..5]);
            return Err(Error::InvalidManifestMagic(magic));
        }

        let entry_count = u16::from_le_bytes([data[6], data[7]]);
        let entries = extract_entries(data, entry_count as usize);

        Ok(Self { entry_count, entries })
    }

    /// Get all referenced NCS filenames
    #[inline]
    pub fn filenames(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.filename.as_str())
    }
}

/// Extract NCS filename entries using SIMD-accelerated pattern matching
fn extract_entries(data: &[u8], capacity_hint: usize) -> Vec<Entry> {
    let finder = memmem::Finder::new(NEXUS_DATA_PREFIX);
    let mut entries = Vec::with_capacity(capacity_hint);

    for start in finder.find_iter(data) {
        if let Some(entry) = extract_entry_at(&data[start..]) {
            entries.push(entry);
        }
    }

    entries
}

/// Extract a single entry starting at the given position
#[inline]
fn extract_entry_at(data: &[u8]) -> Option<Entry> {
    // Find end of string: null byte or non-printable ASCII
    // Use memchr for SIMD-accelerated null search, then validate range
    let null_pos = memchr::memchr(0, data);
    let end = match null_pos {
        Some(pos) => pos,
        None => data.iter().position(|&b| b < 0x20 || b > 0x7e).unwrap_or(data.len()),
    };

    if end < 5 {
        return None;
    }

    // Validate printable ASCII range for the portion before null
    if data[..end].iter().any(|&b| b < 0x20 || b > 0x7e) {
        return None;
    }

    let filename = std::str::from_utf8(&data[..end]).ok()?;

    if !filename.ends_with(".ncs") {
        return None;
    }

    Some(Entry { filename: filename.to_string() })
}

/// Scan for NCS manifest chunks in data
pub fn scan(data: &[u8]) -> Vec<(usize, Manifest)> {
    let finder = memmem::Finder::new(&NCS_MANIFEST_MAGIC);
    let mut results = Vec::new();

    for offset in finder.find_iter(data) {
        if let Ok(manifest) = Manifest::parse(&data[offset..]) {
            results.push((offset, manifest));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_entry() {
        let data = b"Nexus-Data-attribute6.ncs\0extra";
        let entry = extract_entry_at(data).unwrap();
        assert_eq!(entry.filename, "Nexus-Data-attribute6.ncs");
    }

    #[test]
    fn test_extract_entry_no_extension() {
        let data = b"Nexus-Data-attribute6\0";
        assert!(extract_entry_at(data).is_none());
    }

    #[test]
    fn test_manifest_parse_too_short() {
        let data = [0u8; 4];
        assert!(Manifest::parse(&data).is_err());
    }
}
