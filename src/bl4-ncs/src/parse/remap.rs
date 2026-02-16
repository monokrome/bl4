//! FixedWidthIntArray24 - remap array with 24-bit count
//!
//! Used to remap indices in the NCS binary section. Each table has two
//! remap arrays: one for key strings (pair_vec) and one for value strings.

use crate::bit_reader::{bit_width, BitReader};

/// Fixed-width integer array with 24-bit count + 8-bit width header
#[derive(Debug, Clone, Default)]
pub struct FixedWidthIntArray {
    pub count: u32,
    pub value_bit_width: u8,
    pub index_bit_width: u8,
    pub values: Vec<u32>,
}

impl FixedWidthIntArray {
    pub fn is_active(&self) -> bool {
        self.count > 0 && self.value_bit_width > 0 && self.values.len() == self.count as usize
    }

    /// Read from bit stream: 24-bit count, 8-bit width, then count values
    pub fn read(reader: &mut BitReader) -> Option<Self> {
        let count = reader.read_bits(24)?;
        let value_bit_width = reader.read_bits(8)? as u8;
        let index_bit_width = if count > 0 { bit_width(count) } else { 0 };

        if count == 0 || value_bit_width == 0 {
            return Some(Self {
                count,
                value_bit_width,
                index_bit_width,
                values: Vec::new(),
            });
        }

        if value_bit_width > 32 {
            let bits_to_skip = count as usize * value_bit_width as usize;
            reader.skip_bits(bits_to_skip);
            return Some(Self {
                count,
                value_bit_width,
                index_bit_width,
                values: Vec::new(),
            });
        }

        if count > 1_000_000 {
            return None;
        }

        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(reader.read_bits(value_bit_width)?);
        }

        Some(Self {
            count,
            value_bit_width,
            index_bit_width,
            values,
        })
    }

    /// Map raw index through remap array to get remapped value
    pub fn remap(&self, raw_index: u32) -> Option<u32> {
        if raw_index < self.count {
            Some(self.values[raw_index as usize])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_width_array_read() {
        // count=3 (24-bit), width=8 (8-bit), values=[10, 20, 30]
        let data = [
            0x03, 0x00, 0x00, // count = 3
            0x08, // width = 8
            0x0A, 0x14, 0x1E, // values = [10, 20, 30]
        ];
        let mut reader = BitReader::new(&data);
        let arr = FixedWidthIntArray::read(&mut reader).unwrap();

        assert_eq!(arr.count, 3);
        assert_eq!(arr.value_bit_width, 8);
        assert_eq!(arr.values, vec![10, 20, 30]);
        assert!(arr.is_active());
        assert_eq!(arr.index_bit_width, 2); // bit_width(3) = 2
    }

    #[test]
    fn test_remap() {
        let arr = FixedWidthIntArray {
            count: 3,
            value_bit_width: 8,
            index_bit_width: 2,
            values: vec![10, 20, 30],
        };

        assert_eq!(arr.remap(0), Some(10));
        assert_eq!(arr.remap(1), Some(20));
        assert_eq!(arr.remap(2), Some(30));
        assert_eq!(arr.remap(3), None);
    }

    #[test]
    fn test_empty_array() {
        // count=0
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut reader = BitReader::new(&data);
        let arr = FixedWidthIntArray::read(&mut reader).unwrap();

        assert_eq!(arr.count, 0);
        assert!(!arr.is_active());
    }
}
