//! USMAP File Generation
//!
//! Functions for extracting UE5 reflection data and generating usmap files:
//! - extract_struct_properties - Extract properties from UStruct/UClass
//! - extract_enum_values - Extract enum values
//! - extract_reflection_data - Full reflection data extraction
//! - write_usmap - Write usmap file from reflection data

mod extraction;
mod writer;

pub use extraction::{extract_enum_values, extract_reflection_data, extract_struct_properties};
pub use writer::{format as usmap, write_usmap};
