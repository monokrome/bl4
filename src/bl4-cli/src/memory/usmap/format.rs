//! USMAP file format constants and types

/// Magic number for usmap files
pub const MAGIC: u16 = 0x30C4;

/// Usmap version enum
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum EUsmapVersion {
    Initial = 0,
    PackageVersioning = 1,
    LongFName = 2,
    LargeEnums = 3,
}

/// Compression method
#[repr(u8)]
pub enum EUsmapCompression {
    None = 0,
    Oodle = 1,
    Brotli = 2,
    ZStandard = 3,
}
