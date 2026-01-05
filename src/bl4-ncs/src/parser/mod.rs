//! NCS content parser for structured JSON output
//!
//! Parses decompressed NCS content into structured data that can be
//! serialized to JSON.

mod binary;
mod differential;
mod document;
mod entries;
mod header;
mod unpack;

// Re-export public API
pub use binary::{debug_binary_section, parse_binary_section};
pub use document::parse_document;
pub use header::{parse_header, find_binary_section_with_count};
pub use unpack::{find_packed_strings, unpack_string};
