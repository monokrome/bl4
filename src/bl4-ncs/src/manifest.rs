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

    /// Create valid manifest header
    fn make_manifest_header(entry_count: u16) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&NCS_MANIFEST_MAGIC);  // _NCS/
        data.push(0);  // null byte at position 5
        data.extend_from_slice(&entry_count.to_le_bytes());
        data
    }

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
    fn test_extract_entry_too_short() {
        let data = b"abc\0";
        assert!(extract_entry_at(data).is_none());
    }

    #[test]
    fn test_extract_entry_non_printable() {
        let data = b"Nexus-Data-\x01attribute.ncs\0";
        assert!(extract_entry_at(data).is_none());
    }

    #[test]
    fn test_extract_entry_no_null() {
        // String extends to end of data without null terminator
        let data = b"Nexus-Data-test.ncs";
        let entry = extract_entry_at(data).unwrap();
        assert_eq!(entry.filename, "Nexus-Data-test.ncs");
    }

    #[test]
    fn test_manifest_parse_too_short() {
        let data = [0u8; 4];
        let result = Manifest::parse(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::DataTooShort { needed: 8, actual: 4 }));
    }

    #[test]
    fn test_manifest_parse_invalid_magic() {
        let data = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = Manifest::parse(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidManifestMagic(_)));
    }

    #[test]
    fn test_manifest_parse_empty() {
        let data = make_manifest_header(0);
        let manifest = Manifest::parse(&data).unwrap();
        assert_eq!(manifest.entry_count, 0);
        assert!(manifest.entries.is_empty());
    }

    #[test]
    fn test_manifest_parse_with_entries() {
        let mut data = make_manifest_header(2);
        // Add some padding and entries
        data.extend_from_slice(b"\0\0\0\0");
        data.extend_from_slice(b"Nexus-Data-weapons.ncs\0");
        data.extend_from_slice(b"Nexus-Data-shields.ncs\0");

        let manifest = Manifest::parse(&data).unwrap();
        assert_eq!(manifest.entry_count, 2);
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].filename, "Nexus-Data-weapons.ncs");
        assert_eq!(manifest.entries[1].filename, "Nexus-Data-shields.ncs");
    }

    #[test]
    fn test_manifest_filenames() {
        let mut data = make_manifest_header(2);
        data.extend_from_slice(b"Nexus-Data-a.ncs\0");
        data.extend_from_slice(b"Nexus-Data-b.ncs\0");

        let manifest = Manifest::parse(&data).unwrap();
        let filenames: Vec<_> = manifest.filenames().collect();
        assert_eq!(filenames.len(), 2);
        assert_eq!(filenames[0], "Nexus-Data-a.ncs");
        assert_eq!(filenames[1], "Nexus-Data-b.ncs");
    }

    #[test]
    fn test_header_size() {
        assert_eq!(HEADER_SIZE, 8);
    }

    #[test]
    fn test_scan_empty() {
        let results = scan(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_no_manifest() {
        let data = b"Some random data without manifest";
        let results = scan(data);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_single_manifest() {
        let mut data = vec![0u8; 10];  // Padding
        let manifest_start = data.len();
        data.extend_from_slice(&make_manifest_header(1));
        data.extend_from_slice(b"Nexus-Data-test.ncs\0");

        let results = scan(&data);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, manifest_start);
        assert_eq!(results[0].1.entry_count, 1);
    }

    #[test]
    fn test_scan_multiple_manifests() {
        let mut data = vec![];

        // First manifest
        let m1_start = 0;
        data.extend_from_slice(&make_manifest_header(1));
        data.extend_from_slice(b"Nexus-Data-first.ncs\0");

        // Padding
        data.extend_from_slice(&[0xFFu8; 20]);

        // Second manifest
        let m2_start = data.len();
        data.extend_from_slice(&make_manifest_header(2));
        data.extend_from_slice(b"Nexus-Data-second.ncs\0");
        data.extend_from_slice(b"Nexus-Data-third.ncs\0");

        let results = scan(&data);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, m1_start);
        assert_eq!(results[0].1.entry_count, 1);
        assert_eq!(results[1].0, m2_start);
        assert_eq!(results[1].1.entry_count, 2);
    }

    #[test]
    fn test_manifest_debug() {
        let data = make_manifest_header(0);
        let manifest = Manifest::parse(&data).unwrap();
        let debug = format!("{:?}", manifest);
        assert!(debug.contains("Manifest"));
        assert!(debug.contains("entry_count"));
    }

    #[test]
    fn test_entry_equality() {
        let entry1 = Entry { filename: "test.ncs".to_string() };
        let entry2 = Entry { filename: "test.ncs".to_string() };
        let entry3 = Entry { filename: "other.ncs".to_string() };

        assert_eq!(entry1, entry2);
        assert_ne!(entry1, entry3);
    }

    #[test]
    fn test_entry_debug() {
        let entry = Entry { filename: "Nexus-Data-test.ncs".to_string() };
        let debug = format!("{:?}", entry);
        assert!(debug.contains("Entry"));
        assert!(debug.contains("Nexus-Data-test.ncs"));
    }
}
