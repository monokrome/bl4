//! FNV-1a hash function for NCS field name lookups

/// FNV-1a 64-bit offset basis
pub const FNV1A_OFFSET_BASIS: u64 = 0xcbf29ce484222325;

/// FNV-1a 64-bit prime
pub const FNV1A_PRIME: u64 = 0x100000001b3;

/// Compute FNV-1a 64-bit hash of a byte slice
///
/// This is the hash function used by NCS for field name lookups.
///
/// # Example
///
/// ```
/// use bl4_ncs::fnv1a_hash;
///
/// let hash = fnv1a_hash(b"children|map");
/// ```
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash = FNV1A_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV1A_PRIME);
    }
    hash
}

/// Compute FNV-1a hash of a string
#[allow(dead_code)]
pub fn fnv1a_hash_str(s: &str) -> u64 {
    fnv1a_hash(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_empty() {
        assert_eq!(fnv1a_hash(b""), FNV1A_OFFSET_BASIS);
    }

    #[test]
    fn test_fnv1a_basic() {
        // Known test vectors for FNV-1a
        // "a" should hash to a specific value
        let hash = fnv1a_hash(b"a");
        assert_ne!(hash, FNV1A_OFFSET_BASIS);
    }

    #[test]
    fn test_fnv1a_field_names() {
        // Hash some NCS field names - these should be deterministic
        let h1 = fnv1a_hash(b"children|map");
        let h2 = fnv1a_hash(b"children|map");
        assert_eq!(h1, h2);

        // Different names should have different hashes
        let h3 = fnv1a_hash(b"sections|map");
        assert_ne!(h1, h3);
    }
}
