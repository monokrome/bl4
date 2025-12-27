//! Fast Pattern Scanning (Boyer-Moore style with SIMD acceleration)

/// Find the longest contiguous run of non-wildcard bytes in a pattern.
/// Returns (start_offset, bytes) of the best anchor substring.
fn find_best_anchor<'a>(pattern: &'a [u8], mask: &[u8]) -> (usize, &'a [u8]) {
    let mut best_start = 0;
    let mut best_len = 0;
    let mut current_start = 0;
    let mut current_len = 0;

    for (i, &m) in mask.iter().enumerate() {
        if m != 0 {
            if current_len == 0 {
                current_start = i;
            }
            current_len += 1;
        } else {
            if current_len > best_len {
                best_start = current_start;
                best_len = current_len;
            }
            current_len = 0;
        }
    }

    if current_len > best_len {
        best_start = current_start;
        best_len = current_len;
    }

    if best_len == 0 {
        (0, &[])
    } else {
        (best_start, &pattern[best_start..best_start + best_len])
    }
}

/// Verify a full pattern (with wildcards) at a given position in data.
#[inline]
fn verify_pattern(data: &[u8], pattern: &[u8], mask: &[u8]) -> bool {
    if data.len() < pattern.len() {
        return false;
    }
    for i in 0..pattern.len() {
        if mask[i] != 0 && data[i] != pattern[i] {
            return false;
        }
    }
    true
}

/// Fast pattern scan using memchr's SIMD-accelerated memmem finder.
///
/// This uses Boyer-Moore-style searching on the longest contiguous
/// non-wildcard substring, then verifies the full pattern at each hit.
///
/// For patterns without wildcards, this is equivalent to ripgrep's search.
/// For patterns with wildcards, we get O(n/m) average case on the anchor.
pub fn scan_pattern_fast(data: &[u8], pattern: &[u8], mask: &[u8]) -> Vec<usize> {
    if pattern.is_empty() {
        return vec![];
    }

    let (anchor_offset, anchor_bytes) = find_best_anchor(pattern, mask);

    if anchor_bytes.is_empty() {
        let mut results = Vec::new();
        for i in 0..=data.len().saturating_sub(pattern.len()) {
            if verify_pattern(&data[i..], pattern, mask) {
                results.push(i);
            }
        }
        return results;
    }

    let finder = memchr::memmem::Finder::new(anchor_bytes);
    let mut results = Vec::new();

    for anchor_pos in finder.find_iter(data) {
        let pattern_start = anchor_pos.saturating_sub(anchor_offset);

        if pattern_start + pattern.len() > data.len() {
            continue;
        }

        if anchor_pos != pattern_start + anchor_offset {
            continue;
        }

        if verify_pattern(&data[pattern_start..], pattern, mask) {
            results.push(pattern_start);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_pattern_exact_match() {
        let data = b"hello world hello";
        let pattern = b"hello";
        let mask = vec![1u8; pattern.len()];

        let results = scan_pattern_fast(data, pattern, &mask);
        assert_eq!(results, vec![0, 12]);
    }

    #[test]
    fn test_scan_pattern_with_wildcards() {
        let data = b"test1234test5678";
        let pattern = b"test????";
        let mask = vec![1, 1, 1, 1, 0, 0, 0, 0]; // "test" must match, ???? are wildcards

        let results = scan_pattern_fast(data, pattern, &mask);
        assert_eq!(results, vec![0, 8]);
    }

    #[test]
    fn test_scan_pattern_no_match() {
        let data = b"hello world";
        let pattern = b"xyz";
        let mask = vec![1u8; pattern.len()];

        let results = scan_pattern_fast(data, pattern, &mask);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_pattern_empty() {
        let data = b"hello";
        let pattern = b"";
        let mask = vec![];

        let results = scan_pattern_fast(data, pattern, &mask);
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_best_anchor() {
        // Pattern with wildcards in the middle
        let pattern = b"AB??CDEF";
        let mask = vec![1, 1, 0, 0, 1, 1, 1, 1];

        let (offset, anchor) = find_best_anchor(pattern, &mask);
        assert_eq!(offset, 4); // "CDEF" is the longest anchor
        assert_eq!(anchor, b"CDEF");
    }

    #[test]
    fn test_verify_pattern() {
        let data = b"hello";
        let pattern = b"h?llo";
        let mask = vec![1, 0, 1, 1, 1];

        assert!(verify_pattern(data, pattern, &mask));
        assert!(!verify_pattern(b"jello", pattern, &mask));
    }
}
