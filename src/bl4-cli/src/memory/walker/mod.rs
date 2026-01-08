//! GUObjectArray Walking and Dump Analysis
//!
//! Functions for iterating over UE5 object arrays:
//! - analyze_dump - Full memory dump analysis
//! - walk_guobject_array - Iterator over all UObjects with class info
//! - extract_property - Extract FProperty data from memory

mod analyze;
mod extraction;
mod property;
mod type_reader;
mod validation;
mod walk;

pub use analyze::analyze_dump;
pub use extraction::extract_property;
pub use walk::walk_guobject_array;

