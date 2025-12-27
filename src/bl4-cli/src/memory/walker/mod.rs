//! GUObjectArray Walking and Dump Analysis
//!
//! Functions for iterating over UE5 object arrays:
//! - analyze_dump - Full memory dump analysis
//! - walk_guobject_array - Iterator over all UObjects with class info
//! - extract_property - Extract FProperty data from memory

mod analyze;
mod property;
mod walk;

pub use analyze::analyze_dump;
pub use property::{extract_property, read_property_type};
pub use walk::walk_guobject_array;

#[cfg(test)]
pub mod tests {
    pub use super::property::tests::{create_mock_property, create_mock_uobject};
}
