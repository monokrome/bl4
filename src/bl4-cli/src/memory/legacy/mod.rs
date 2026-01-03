//! Legacy memory module - remaining functionality to be further modularized.
//!
//! Contains GUObjectArray, FName reading, UClass discovery,
//! reflection data extraction, and part definitions.

#![allow(dead_code)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::single_match)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::needless_borrow)]
#![allow(unused_comparisons)]

mod object_search;
mod part_defs;
mod part_extraction;

pub use object_search::{find_objects_by_pattern, generate_object_map, ObjectMapEntry};
pub use part_defs::PartDefinition;
pub use part_extraction::{extract_parts_from_fname_arrays, list_all_part_fnames};

// Re-export for API completeness
#[allow(unused_imports)]
pub use part_defs::{get_category_for_part, GbxSerialNumberIndex};
#[allow(unused_imports)]
pub use part_extraction::extract_part_definitions;

#[cfg(test)]
mod tests {
    use crate::memory::source::find_bl4_process;

    #[test]
    fn test_find_process() {
        // This will fail if BL4 isn't running, which is expected
        let result = find_bl4_process();
        println!("Find process result: {:?}", result);
    }
}
