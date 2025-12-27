//! FNamePool and FNameReader for UE5
//!
//! Provides FName reading from the chunked FNamePool structure used in UE5.5+.
//! The pool uses block-based storage where each block is typically 64KB.

mod pool;
mod reader;

pub use pool::FNamePool;
pub use reader::FNameReader;
