//! Mock Memory Source
//!
//! A mock memory source for testing pattern scanning and discovery.

use super::{MemoryRegion, MemorySource};
use anyhow::{bail, Result};

/// A mock memory source for testing pattern scanning and discovery
pub struct MockMemorySource {
    /// Raw memory data (contiguous, starting at base_address)
    pub data: Vec<u8>,
    /// Base virtual address for the data
    pub base_address: usize,
    /// Memory regions (for region-based operations)
    pub regions: Vec<MemoryRegion>,
}

impl MockMemorySource {
    /// Create a new mock with data at given base address
    pub fn new(data: Vec<u8>, base_address: usize) -> Self {
        let end = base_address + data.len();
        Self {
            data,
            base_address,
            regions: vec![MemoryRegion {
                start: base_address,
                end,
                perms: "rw-p".to_string(),
                offset: 0,
                path: None,
            }],
        }
    }

    /// Create with multiple regions
    pub fn with_regions(data: Vec<u8>, base_address: usize, regions: Vec<MemoryRegion>) -> Self {
        Self {
            data,
            base_address,
            regions,
        }
    }

    /// Create a mock with executable code region (for PE parsing tests)
    pub fn with_code_region(data: Vec<u8>, base_address: usize) -> Self {
        let end = base_address + data.len();
        Self {
            data,
            base_address,
            regions: vec![MemoryRegion {
                start: base_address,
                end,
                perms: "r-xp".to_string(),
                offset: 0,
                path: Some("test.exe".to_string()),
            }],
        }
    }
}

impl MemorySource for MockMemorySource {
    fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>> {
        if address < self.base_address {
            bail!("Address {:#x} below base {:#x}", address, self.base_address);
        }

        let offset = address - self.base_address;
        if offset + size > self.data.len() {
            bail!(
                "Read of {} bytes at {:#x} exceeds data size {}",
                size,
                address,
                self.data.len()
            );
        }

        Ok(self.data[offset..offset + size].to_vec())
    }

    fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    fn is_live(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_source_read_bytes() {
        let data = vec![0x41, 0x42, 0x43, 0x44]; // "ABCD"
        let source = MockMemorySource::new(data, 0x1000);

        let result = source.read_bytes(0x1000, 4).unwrap();
        assert_eq!(result, vec![0x41, 0x42, 0x43, 0x44]);

        let partial = source.read_bytes(0x1001, 2).unwrap();
        assert_eq!(partial, vec![0x42, 0x43]);
    }

    #[test]
    fn test_mock_source_read_u64() {
        let data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let source = MockMemorySource::new(data, 0x1000);

        let value = source.read_u64(0x1000).unwrap();
        assert_eq!(value, 0x0807060504030201); // Little-endian
    }

    #[test]
    fn test_mock_source_read_u32() {
        let data = vec![0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];
        let source = MockMemorySource::new(data, 0x1000);

        let value = source.read_u32(0x1000).unwrap();
        assert_eq!(value, 0x04030201); // Little-endian
    }

    #[test]
    fn test_mock_source_read_ptr() {
        let data = vec![0x00, 0x10, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let source = MockMemorySource::new(data, 0x1000);

        let ptr = source.read_ptr(0x1000).unwrap();
        assert_eq!(ptr, 0x0000000100001000);
    }

    #[test]
    fn test_mock_source_read_cstring() {
        let data = b"Hello\0World\0padding".to_vec(); // 19 bytes total
        let source = MockMemorySource::new(data, 0x1000);

        let s = source.read_cstring(0x1000, 10).unwrap();
        assert_eq!(s, "Hello");

        let s2 = source.read_cstring(0x1006, 10).unwrap();
        assert_eq!(s2, "World");
    }

    #[test]
    fn test_mock_source_read_out_of_bounds() {
        let data = vec![0x41, 0x42, 0x43, 0x44];
        let source = MockMemorySource::new(data, 0x1000);

        // Reading past end should fail
        assert!(source.read_bytes(0x1002, 10).is_err());

        // Reading before base should fail
        assert!(source.read_bytes(0x500, 4).is_err());
    }

    #[test]
    fn test_mock_source_find_region() {
        let data = vec![0; 0x2000];
        let source = MockMemorySource::with_regions(
            data,
            0x1000,
            vec![
                MemoryRegion {
                    start: 0x1000,
                    end: 0x2000,
                    perms: "r--p".to_string(),
                    offset: 0,
                    path: None,
                },
                MemoryRegion {
                    start: 0x2000,
                    end: 0x3000,
                    perms: "rw-p".to_string(),
                    offset: 0x1000,
                    path: None,
                },
            ],
        );

        // Find first region
        let r1 = source.find_region(0x1500);
        assert!(r1.is_some());
        assert_eq!(r1.unwrap().start, 0x1000);

        // Find second region
        let r2 = source.find_region(0x2500);
        assert!(r2.is_some());
        assert_eq!(r2.unwrap().start, 0x2000);

        // Address not in any region
        let none = source.find_region(0x5000);
        assert!(none.is_none());
    }

    #[test]
    fn test_mock_source_is_readable() {
        let data = vec![0; 0x2000];
        let source = MockMemorySource::with_regions(
            data,
            0x1000,
            vec![
                MemoryRegion {
                    start: 0x1000,
                    end: 0x2000,
                    perms: "r--p".to_string(),
                    offset: 0,
                    path: None,
                },
                MemoryRegion {
                    start: 0x2000,
                    end: 0x3000,
                    perms: "-w-p".to_string(),
                    offset: 0x1000,
                    path: None,
                },
            ],
        );

        assert!(source.is_readable(0x1500));
        assert!(!source.is_readable(0x2500));
        assert!(!source.is_readable(0x5000));
    }

    #[test]
    fn test_mock_source_is_not_live() {
        let source = MockMemorySource::new(vec![0; 100], 0x1000);
        assert!(!source.is_live());
    }
}
