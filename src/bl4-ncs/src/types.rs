//! Core types for NCS parsing support

/// Represents an unpacked value from a packed NCS string
#[derive(Debug, Clone, PartialEq)]
pub enum UnpackedValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

/// Result of unpacking a packed string
#[derive(Debug, Clone)]
pub struct UnpackedString {
    pub original: String,
    pub values: Vec<UnpackedValue>,
    pub was_packed: bool,
}
