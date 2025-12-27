//! Memory Region Types
//!
//! Data structures for representing memory regions from /proc/pid/maps.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_region_size() {
        let region = MemoryRegion {
            start: 0x1000,
            end: 0x2000,
            perms: "rw-p".to_string(),
            offset: 0,
            path: None,
        };
        assert_eq!(region.size(), 0x1000);
    }

    #[test]
    fn test_memory_region_is_readable() {
        let readable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "r--p".to_string(),
            offset: 0,
            path: None,
        };
        assert!(readable.is_readable());

        let not_readable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "-w-p".to_string(),
            offset: 0,
            path: None,
        };
        assert!(!not_readable.is_readable());
    }

    #[test]
    fn test_memory_region_is_writable() {
        let writable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "rw-p".to_string(),
            offset: 0,
            path: None,
        };
        assert!(writable.is_writable());

        let not_writable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "r--p".to_string(),
            offset: 0,
            path: None,
        };
        assert!(!not_writable.is_writable());
    }

    #[test]
    fn test_memory_region_is_executable() {
        let executable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "r-xp".to_string(),
            offset: 0,
            path: None,
        };
        assert!(executable.is_executable());

        let not_executable = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "rw-p".to_string(),
            offset: 0,
            path: None,
        };
        assert!(!not_executable.is_executable());
    }

    #[test]
    fn test_memory_region_all_perms() {
        let full = MemoryRegion {
            start: 0,
            end: 0x1000,
            perms: "rwxp".to_string(),
            offset: 0,
            path: None,
        };
        assert!(full.is_readable());
        assert!(full.is_writable());
        assert!(full.is_executable());
    }
}
