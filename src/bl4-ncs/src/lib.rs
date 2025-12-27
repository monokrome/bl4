//! NCS (Nexus Config Store) parser for Borderlands 4
//!
//! NCS files are Oodle-compressed configuration stores used by the game.
//!
//! # Format Overview
//!
//! ## NCS Data Format (`[version]NCS`)
//!
//! Compressed configuration data:
//! - Byte 0: Version byte (typically 0x01)
//! - Bytes 1-3: "NCS" magic
//! - Bytes 4-7: Compression flag
//! - Bytes 8-11: Decompressed size
//! - Bytes 12-15: Compressed size
//! - Bytes 16+: Payload
//!
//! ## NCS Manifest Format (`_NCS/`)
//!
//! Index files listing NCS data stores:
//! - Bytes 0-4: "_NCS/" magic
//! - Bytes 6-7: Entry count
//! - Remaining: Metadata and string table

mod data;
mod field;
mod hash;
pub mod legacy;
mod manifest;

// Re-export main types
pub use data::{decompress as decompress_ncs, scan as scan_for_ncs, Header as NcsHeader};
pub use field::{known as fields, Field, Type as FieldType};
pub use hash::fnv1a_hash;
pub use manifest::{scan as scan_for_ncs_manifests, Entry as NcsManifestEntry, Manifest as NcsManifest};

// Re-export legacy types for backwards compatibility
#[allow(deprecated)]
pub use legacy::{
    decompress as decompress_gbx, extract_chunk as extract_gbx_chunk, is_gbx,
    scan as scan_for_gbx, scan_all as scan_for_gbx_all, Chunk as GbxChunk,
    Header as GbxHeader, ScanResult as GbxScanResult, Variant as GbxVariant, GBX_MAGIC,
};

/// Magic bytes for NCS format: "NCS" (bytes 1-3 of header)
pub const NCS_MAGIC: [u8; 3] = [0x4e, 0x43, 0x53];

/// Magic bytes for NCS manifest format: "_NCS/"
pub const NCS_MANIFEST_MAGIC: [u8; 5] = [0x5f, 0x4e, 0x43, 0x53, 0x2f];

/// Inner compressed data magic (big-endian)
pub const OODLE_MAGIC: u32 = 0xb7756362;

/// Header size in bytes
pub const NCS_HEADER_SIZE: usize = data::HEADER_SIZE;

/// Manifest header size
pub const NCS_MANIFEST_HEADER_SIZE: usize = manifest::HEADER_SIZE;

/// Inner header minimum size
pub const NCS_INNER_HEADER_MIN: usize = data::INNER_HEADER_MIN;

/// Errors from NCS parsing
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid NCS magic: expected 'NCS', got {0:02x} {1:02x} {2:02x}")]
    InvalidNcsMagic(u8, u8, u8),

    #[error("Invalid NCS manifest magic: expected '_NCS/', got {0:?}")]
    InvalidManifestMagic([u8; 5]),

    #[error("Invalid magic bytes: expected 'gBx', got {0:02x} {1:02x} {2:02x}")]
    InvalidMagic(u8, u8, u8),

    #[error("Unknown gBx variant: 0x{0:02x}")]
    UnknownVariant(u8),

    #[error("Invalid inner magic: expected 0xb7756362, got 0x{0:08x}")]
    InvalidInnerMagic(u32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Oodle decompression error: {0}")]
    Oodle(String),

    #[error("Decompression size mismatch: expected {expected}, got {actual}")]
    DecompressionSize { expected: usize, actual: usize },

    #[error("Data too short: need {needed} bytes, got {actual}")]
    DataTooShort { needed: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Check if data starts with NCS data magic
pub fn is_ncs(data: &[u8]) -> bool {
    data.len() >= 4 && data[1..4] == NCS_MAGIC && data[0] != b'_'
}

/// Check if data starts with NCS manifest magic
pub fn is_ncs_manifest(data: &[u8]) -> bool {
    data.len() >= 5 && data[0..5] == NCS_MANIFEST_MAGIC
}

/// Legacy parser for backwards compatibility
#[deprecated(since = "0.5.0", note = "Use is_ncs(), decompress_ncs() instead")]
pub struct LegacyParser;

#[allow(deprecated)]
impl LegacyParser {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        decompress_gbx(data)
    }

    pub fn is_gbx(data: &[u8]) -> bool {
        is_gbx(data)
    }

    pub fn get_variant(data: &[u8]) -> Option<GbxVariant> {
        legacy::get_variant(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ncs() {
        // Valid NCS: version byte + "NCS"
        assert!(is_ncs(&[0x01, 0x4e, 0x43, 0x53, 0x00]));

        // Invalid: "_NCS/" manifest format
        assert!(!is_ncs(&[0x5f, 0x4e, 0x43, 0x53, 0x2f]));

        // Too short
        assert!(!is_ncs(&[0x01, 0x4e, 0x43]));
    }

    #[test]
    fn test_is_ncs_manifest() {
        assert!(is_ncs_manifest(&[0x5f, 0x4e, 0x43, 0x53, 0x2f, 0x00]));
        assert!(!is_ncs_manifest(&[0x01, 0x4e, 0x43, 0x53, 0x00]));
    }

    #[test]
    fn test_magic_constants() {
        assert_eq!(NCS_MAGIC, *b"NCS");
        assert_eq!(NCS_MANIFEST_MAGIC, *b"_NCS/");
        assert_eq!(OODLE_MAGIC, 0xb7756362);
    }

    #[test]
    fn test_header_size_constants() {
        assert_eq!(NCS_HEADER_SIZE, 16);
        assert_eq!(NCS_MANIFEST_HEADER_SIZE, 8);
        assert_eq!(NCS_INNER_HEADER_MIN, 0x40);
    }

    #[allow(deprecated)]
    #[test]
    fn test_ncs_parser_legacy() {
        let parser = LegacyParser::new().unwrap();

        // Test is_gbx static method
        assert!(LegacyParser::is_gbx(&[0x67, 0x42, 0x78, 0x39]));
        assert!(!LegacyParser::is_gbx(&[0x00, 0x00, 0x00, 0x00]));

        // Test get_variant static method
        assert_eq!(LegacyParser::get_variant(&[0x67, 0x42, 0x78, 0x39]), Some(GbxVariant::V9));
        assert_eq!(LegacyParser::get_variant(&[0x00, 0x00, 0x00, 0x00]), None);

        // Test decompress with invalid data (should error)
        let result = parser.decompress(&[0x00; 20]);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_display() {
        let err = Error::InvalidNcsMagic(0x00, 0x00, 0x00);
        assert!(err.to_string().contains("Invalid NCS magic"));

        let err = Error::InvalidManifestMagic([0x00; 5]);
        assert!(err.to_string().contains("Invalid NCS manifest magic"));

        let err = Error::InvalidMagic(0x00, 0x00, 0x00);
        assert!(err.to_string().contains("Invalid magic bytes"));

        let err = Error::UnknownVariant(0xFF);
        assert!(err.to_string().contains("Unknown gBx variant"));

        let err = Error::InvalidInnerMagic(0x00000000);
        assert!(err.to_string().contains("Invalid inner magic"));

        let err = Error::Oodle("test error".to_string());
        assert!(err.to_string().contains("Oodle decompression error"));

        let err = Error::DecompressionSize { expected: 100, actual: 50 };
        assert!(err.to_string().contains("Decompression size mismatch"));

        let err = Error::DataTooShort { needed: 16, actual: 8 };
        assert!(err.to_string().contains("Data too short"));
    }

    #[test]
    fn test_error_debug() {
        let err = Error::InvalidNcsMagic(0x00, 0x00, 0x00);
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidNcsMagic"));
    }
}
