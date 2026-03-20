//! Bit-level data reading for NCS binary parsing
//!
//! Provides utilities for reading packed binary data at the bit level,
//! including variable-length integers and fixed-width arrays.
//!
//! Two implementations:
//! - `BitReader<'a>`: reads from a borrowed byte slice
//! - `StreamingBitReader<R>`: reads from any `std::io::Read` source

use std::io::Read;

/// Trait for bit-level reading, used by the NCS decode loop
pub trait BitRead {
    /// Read n bits as u32 (1..=32)
    fn read_bits(&mut self, n: u8) -> Option<u32>;

    /// Current absolute position in bits
    fn position(&self) -> usize;

    /// Total bits available (may trigger buffering on streaming readers)
    fn total_bits(&mut self) -> usize;

    /// Skip to byte boundary
    fn align_byte(&mut self);

    /// Seek to a specific bit position (forward only for streaming)
    fn seek(&mut self, bit_pos: usize);

    /// Read a single bit
    fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|v| v != 0)
    }

    /// Read variable-length integer (Elias gamma coding)
    fn read_varint(&mut self) -> Option<u32> {
        let mut zeros = 0u8;
        while !self.read_bit()? {
            zeros += 1;
            if zeros > 30 {
                return None;
            }
        }

        if zeros == 0 {
            return Some(1);
        }

        let value = self.read_bits(zeros)?;
        Some((1 << zeros) | value)
    }

    /// Check if n more bits are available
    fn has_bits(&mut self, n: usize) -> bool {
        self.position() + n <= self.total_bits()
    }

    /// Check if we've reached end of data
    fn is_empty(&mut self) -> bool {
        self.position() >= self.total_bits()
    }

    /// Read a FixedWidthIntArray24 header (24-bit count + 8-bit width)
    fn read_fixed_width_header(&mut self) -> Option<(u32, u8)> {
        let count = self.read_bits(24)?;
        let width = self.read_bits(8)? as u8;
        Some((count, width))
    }

    /// Read n entries of `width` bits each
    fn read_fixed_width_array(&mut self, count: u32, width: u8) -> Option<Vec<u32>> {
        if width == 0 || width > 32 {
            return Some(Vec::new());
        }
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(self.read_bits(width)?);
        }
        Some(values)
    }

    /// Skip n bits forward
    fn skip_bits(&mut self, n: usize) {
        let new_pos = self.position() + n;
        self.seek(new_pos);
    }
}

/// Bitstream reader for parsing packed binary data from a byte slice
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Get remaining bytes (for debugging)
    pub fn remaining_bytes(&self) -> &[u8] {
        if self.bit_pos == 0 {
            &self.data[self.byte_pos..]
        } else {
            &self.data[self.byte_pos + 1..]
        }
    }
}

impl BitRead for BitReader<'_> {
    fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 || n > 32 {
            return None;
        }

        let mut result: u32 = 0;
        let mut bits_read = 0u8;

        while bits_read < n {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let remaining_in_byte = 8 - self.bit_pos;
            let bits_to_read = remaining_in_byte.min(n - bits_read);

            let mask = ((1u32 << bits_to_read) - 1) as u8;
            let byte_val = self.data[self.byte_pos];
            let extracted = (byte_val >> self.bit_pos) & mask;

            result |= (extracted as u32) << bits_read;
            bits_read += bits_to_read;
            self.bit_pos += bits_to_read;

            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Some(result)
    }

    fn position(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }

    fn total_bits(&mut self) -> usize {
        self.data.len() * 8
    }

    fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    fn seek(&mut self, bit_pos: usize) {
        self.byte_pos = bit_pos / 8;
        self.bit_pos = (bit_pos % 8) as u8;
    }
}

const STREAM_BUF_CHUNK: usize = 8192;

/// Streaming bitstream reader that reads from any `Read` source.
///
/// Buffers data internally in chunks, supporting forward-only bit reads.
/// `total_bits` reads all remaining data to EOF on first call (then cached).
pub struct StreamingBitReader<R: Read> {
    reader: R,
    buffer: Vec<u8>,
    buf_offset: usize,
    bit_offset: u8,
    absolute_byte: usize,
    eof: bool,
}

impl<R: Read> StreamingBitReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            buf_offset: 0,
            bit_offset: 0,
            absolute_byte: 0,
            eof: false,
        }
    }

    /// Ensure at least `n` bytes are available from buf_offset
    fn ensure_bytes(&mut self, n: usize) -> bool {
        let available = self.buffer.len() - self.buf_offset;
        if available >= n {
            return true;
        }
        if self.eof {
            return false;
        }

        // Compact: move unconsumed data to front
        if self.buf_offset > 0 {
            self.buffer.drain(..self.buf_offset);
            self.buf_offset = 0;
        }

        let needed = n.saturating_sub(self.buffer.len());
        let read_target = needed.max(STREAM_BUF_CHUNK);
        let old_len = self.buffer.len();
        self.buffer.resize(old_len + read_target, 0);

        let mut total_read = 0;
        while total_read < needed {
            match self.reader.read(&mut self.buffer[old_len + total_read..]) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => total_read += n,
                Err(_) => {
                    self.eof = true;
                    break;
                }
            }
        }

        self.buffer.truncate(old_len + total_read);
        self.buffer.len() - self.buf_offset >= n
    }

    /// Buffer all remaining data from the reader (for total_bits)
    fn buffer_all(&mut self) {
        if self.eof {
            return;
        }

        if self.buf_offset > 0 {
            self.buffer.drain(..self.buf_offset);
            self.buf_offset = 0;
        }

        let mut chunk = [0u8; STREAM_BUF_CHUNK];
        loop {
            match self.reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(_) => break,
            }
        }
        self.eof = true;
    }
}

impl<R: Read> BitRead for StreamingBitReader<R> {
    fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 || n > 32 {
            return None;
        }

        let bytes_needed = (self.bit_offset as usize + n as usize).div_ceil(8);
        if !self.ensure_bytes(bytes_needed) {
            return None;
        }

        let mut result: u32 = 0;
        let mut bits_read = 0u8;

        while bits_read < n {
            if self.buf_offset >= self.buffer.len() && !self.ensure_bytes(1) {
                return None;
            }

            let remaining_in_byte = 8 - self.bit_offset;
            let bits_to_read = remaining_in_byte.min(n - bits_read);

            let mask = ((1u32 << bits_to_read) - 1) as u8;
            let byte_val = self.buffer[self.buf_offset];
            let extracted = (byte_val >> self.bit_offset) & mask;

            result |= (extracted as u32) << bits_read;
            bits_read += bits_to_read;
            self.bit_offset += bits_to_read;

            if self.bit_offset >= 8 {
                self.bit_offset = 0;
                self.buf_offset += 1;
                self.absolute_byte += 1;
            }
        }

        Some(result)
    }

    fn position(&self) -> usize {
        self.absolute_byte * 8 + self.bit_offset as usize
    }

    fn total_bits(&mut self) -> usize {
        self.buffer_all();
        (self.absolute_byte + self.buffer.len() - self.buf_offset) * 8
    }

    fn align_byte(&mut self) {
        if self.bit_offset != 0 {
            self.bit_offset = 0;
            self.buf_offset += 1;
            self.absolute_byte += 1;
        }
    }

    fn seek(&mut self, bit_pos: usize) {
        let current = self.position();
        if bit_pos <= current {
            return;
        }

        let mut remaining = bit_pos - current;

        // Align to byte boundary first if mid-byte
        if self.bit_offset != 0 {
            let skip_in_byte = (8 - self.bit_offset as usize).min(remaining);
            self.bit_offset += skip_in_byte as u8;
            remaining -= skip_in_byte;
            if self.bit_offset >= 8 {
                self.bit_offset = 0;
                self.buf_offset += 1;
                self.absolute_byte += 1;
            }
        }

        // Skip whole bytes
        let skip_bytes = remaining / 8;
        self.buf_offset += skip_bytes;
        self.absolute_byte += skip_bytes;
        remaining -= skip_bytes * 8;

        // Skip remaining bits
        if remaining > 0 {
            self.ensure_bytes(1);
            self.bit_offset = remaining as u8;
        }
    }
}

/// Calculate minimum bits needed to index a table of `count` entries
pub fn bit_width(count: u32) -> u8 {
    if count < 2 {
        return 1;
    }
    let mut n = count - 1;
    let mut bits = 0u8;
    while n > 0 {
        bits += 1;
        n >>= 1;
    }
    bits.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10110101, 0b11001010];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(1), Some(1));
        assert_eq!(reader.read_bits(1), Some(0));
        assert_eq!(reader.read_bits(4), Some(0b1011));
    }

    #[test]
    fn test_bit_reader_cross_byte() {
        let data = [0xFF, 0xFF];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(12), Some(0xFFF));
    }

    #[test]
    fn test_bit_width() {
        assert_eq!(bit_width(0), 1);
        assert_eq!(bit_width(1), 1);
        assert_eq!(bit_width(2), 1);
        assert_eq!(bit_width(3), 2);
        assert_eq!(bit_width(4), 2);
        assert_eq!(bit_width(5), 3);
        assert_eq!(bit_width(256), 8);
    }

    #[test]
    fn test_streaming_matches_slice() {
        let data = [0b10110101, 0b11001010, 0xFF, 0x00, 0xAB];

        let mut slice_reader = BitReader::new(&data);
        let mut stream_reader = StreamingBitReader::new(&data[..]);

        // Read same sequence of bits from both
        for bits in [1, 3, 5, 8, 12, 4, 2, 1] {
            let from_slice = slice_reader.read_bits(bits);
            let from_stream = stream_reader.read_bits(bits);
            assert_eq!(from_slice, from_stream, "mismatch reading {} bits", bits);
            assert_eq!(slice_reader.position(), stream_reader.position());
        }
    }

    #[test]
    fn test_streaming_position() {
        let data = [0xFF; 10];
        let mut reader = StreamingBitReader::new(&data[..]);

        assert_eq!(reader.position(), 0);
        reader.read_bits(5);
        assert_eq!(reader.position(), 5);
        reader.read_bits(8);
        assert_eq!(reader.position(), 13);
        reader.align_byte();
        assert_eq!(reader.position(), 16);
    }

    #[test]
    fn test_streaming_total_bits() {
        let data = [0xFF; 10];
        let mut reader = StreamingBitReader::new(&data[..]);
        assert_eq!(reader.total_bits(), 80);
    }

    #[test]
    fn test_streaming_has_bits() {
        let data = [0xFF; 2];
        let mut reader = StreamingBitReader::new(&data[..]);
        assert!(reader.has_bits(16));
        assert!(!reader.has_bits(17));
        reader.read_bits(8);
        assert!(reader.has_bits(8));
        assert!(!reader.has_bits(9));
    }

    #[test]
    fn test_streaming_seek_forward() {
        let data = [0b10110101, 0b11001010, 0xFF, 0x00];

        let mut slice_reader = BitReader::new(&data);
        let mut stream_reader = StreamingBitReader::new(&data[..]);

        // Read 4 bits, seek to bit 16, read 8 bits — should match
        slice_reader.read_bits(4);
        stream_reader.read_bits(4);

        slice_reader.seek(16);
        stream_reader.seek(16);
        assert_eq!(slice_reader.position(), stream_reader.position());

        let from_slice = slice_reader.read_bits(8);
        let from_stream = stream_reader.read_bits(8);
        assert_eq!(from_slice, from_stream);
    }

    #[test]
    fn test_streaming_varint() {
        let data = [0b10110101, 0b11001010, 0xFF, 0x00, 0xAB];

        let mut slice_reader = BitReader::new(&data);
        let mut stream_reader = StreamingBitReader::new(&data[..]);

        let from_slice = slice_reader.read_varint();
        let from_stream = stream_reader.read_varint();
        assert_eq!(from_slice, from_stream);
        assert_eq!(slice_reader.position(), stream_reader.position());
    }

    #[test]
    fn test_trait_default_methods() {
        let data = [0xFF; 4];
        let mut reader = StreamingBitReader::new(&data[..]);

        let (count, width) = reader.read_fixed_width_header().unwrap();
        // 24 bits of 0xFF = 16777215, 8 bits of 0xFF = 255
        assert_eq!(count, 0x00FFFFFF);
        assert_eq!(width, 255);
    }
}
