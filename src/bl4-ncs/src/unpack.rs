//! String unpacking for NCS packed value strings
//!
//! NCS uses aggressive value packing where multiple values are concatenated
//! in a single string field.

use crate::types::{UnpackedString, UnpackedValue};

/// Unpack a potentially packed NCS string into its component values.
///
/// NCS uses aggressive value packing where multiple values are concatenated:
/// - "1airship" -> [Integer(1), String("airship")]
/// - "0.175128Session" -> [Float(0.175128), String("Session")]
/// - "5true" -> [Integer(5), Boolean(true)]
/// - "simple" -> [String("simple")] (not packed)
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub fn unpack_string(s: &str) -> UnpackedString {
    let original = s.to_string();

    if s.is_empty() {
        return UnpackedString {
            original,
            values: vec![],
            was_packed: false,
        };
    }

    // Pure numeric string (integer)
    if s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = s.parse::<i64>() {
            return UnpackedString {
                original,
                values: vec![UnpackedValue::Integer(n)],
                was_packed: false,
            };
        }
    }

    // Pure float string
    if s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && s.contains('.') {
        if let Ok(f) = s.parse::<f64>() {
            return UnpackedString {
                original,
                values: vec![UnpackedValue::Float(f)],
                was_packed: false,
            };
        }
    }

    // Check for packed patterns
    let mut values = Vec::new();
    let mut remaining = s;

    // Pattern 1: Float prefix (e.g., "0.175128Session")
    if let Some(float_end) = find_float_end(remaining) {
        if float_end < remaining.len() {
            let float_str = &remaining[..float_end];
            if let Ok(f) = float_str.parse::<f64>() {
                values.push(UnpackedValue::Float(f));
                remaining = &remaining[float_end..];
            }
        }
    }

    // Pattern 2: Integer prefix (e.g., "1airship", "5true")
    if values.is_empty() {
        if let Some(int_end) = find_integer_end(remaining) {
            if int_end < remaining.len() {
                let int_str = &remaining[..int_end];
                if let Ok(n) = int_str.parse::<i64>() {
                    values.push(UnpackedValue::Integer(n));
                    remaining = &remaining[int_end..];
                }
            }
        }
    }

    // Check for boolean suffix
    if remaining.eq_ignore_ascii_case("true") {
        values.push(UnpackedValue::Boolean(true));
        remaining = "";
    } else if remaining.eq_ignore_ascii_case("false") {
        values.push(UnpackedValue::Boolean(false));
        remaining = "";
    }

    // Remaining string (if any)
    if !remaining.is_empty() {
        values.push(UnpackedValue::String(remaining.to_string()));
    }

    // If we only got one value and it's a string equal to original, not packed
    let was_packed = values.len() > 1
        || (values.len() == 1
            && !matches!(&values[0], UnpackedValue::String(s) if s == &original));

    // If nothing was unpacked, treat as plain string
    if values.is_empty() {
        values.push(UnpackedValue::String(original.clone()));
    }

    UnpackedString {
        original,
        values,
        was_packed,
    }
}

/// Find the end position of a float at the start of a string
fn find_float_end(s: &str) -> Option<usize> {
    let mut chars = s.chars().peekable();
    let mut pos = 0;
    let mut has_dot = false;
    let mut has_digit = false;

    if chars.peek() == Some(&'-') {
        chars.next();
        pos += 1;
    }

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            has_digit = true;
            chars.next();
            pos += 1;
        } else {
            break;
        }
    }

    if chars.peek() == Some(&'.') {
        has_dot = true;
        chars.next();
        pos += 1;

        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                has_digit = true;
                chars.next();
                pos += 1;
            } else {
                break;
            }
        }
    }

    if has_dot && has_digit && pos > 0 {
        Some(pos)
    } else {
        None
    }
}

/// Find the end position of an integer at the start of a string
fn find_integer_end(s: &str) -> Option<usize> {
    let mut pos = 0;

    for c in s.chars() {
        if c.is_ascii_digit() {
            pos += 1;
        } else {
            break;
        }
    }

    if pos > 0 { Some(pos) } else { None }
}

/// Batch unpack multiple strings, returning only those that were packed
pub fn find_packed_strings(strings: &[String]) -> Vec<UnpackedString> {
    strings
        .iter()
        .map(|s| unpack_string(s))
        .filter(|u| u.was_packed)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack_string_simple() {
        let result = unpack_string("123");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::Integer(123)]);

        let result = unpack_string("1.5");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::Float(1.5)]);

        let result = unpack_string("hello");
        assert!(!result.was_packed);
        assert_eq!(result.values, vec![UnpackedValue::String("hello".into())]);
    }

    #[test]
    fn test_unpack_string_packed_int_string() {
        let result = unpack_string("1airship");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Integer(1));
        assert_eq!(result.values[1], UnpackedValue::String("airship".into()));

        let result = unpack_string("12ships");
        assert!(result.was_packed);
        assert_eq!(result.values[0], UnpackedValue::Integer(12));
        assert_eq!(result.values[1], UnpackedValue::String("ships".into()));
    }

    #[test]
    fn test_unpack_string_packed_float_string() {
        let result = unpack_string("0.175128Session");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Float(0.175128));
        assert_eq!(result.values[1], UnpackedValue::String("Session".into()));
    }

    #[test]
    fn test_unpack_string_packed_int_bool() {
        let result = unpack_string("5true");
        assert!(result.was_packed);
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], UnpackedValue::Integer(5));
        assert_eq!(result.values[1], UnpackedValue::Boolean(true));

        let result = unpack_string("0false");
        assert!(result.was_packed);
        assert_eq!(result.values[0], UnpackedValue::Integer(0));
        assert_eq!(result.values[1], UnpackedValue::Boolean(false));
    }

    #[test]
    fn test_find_packed_strings() {
        let strings = vec![
            "hello".to_string(),
            "123".to_string(),
            "1airship".to_string(),
            "0.5test".to_string(),
            "world".to_string(),
        ];
        let packed = find_packed_strings(&strings);
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0].original, "1airship");
        assert_eq!(packed[1].original, "0.5test");
    }
}
