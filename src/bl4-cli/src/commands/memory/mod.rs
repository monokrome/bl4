//! Memory command handlers for bl4 CLI
//!
//! This module contains handlers for memory-related subcommands.

mod analysis;
mod build_parts_db;
mod discover;
mod extract_ncs_schema;
mod extract_parts;
mod fname;
mod listing;
mod objects;
mod preload;
mod raw_memory;

pub use analysis::{
    handle_analyze_dump, handle_dump_parts, handle_dump_usmap, handle_monitor, handle_scan_string,
};
pub use build_parts_db::handle_build_parts_db;
pub use discover::{handle_discover, handle_find_class_uclass, handle_objects};
pub use extract_ncs_schema::handle_extract_ncs_schema;
pub use extract_parts::{handle_extract_parts, handle_extract_parts_raw};
pub use fname::{handle_fname, handle_fname_search};
pub use listing::{handle_list_objects, handle_list_uclasses};
pub use objects::{handle_find_objects_by_pattern, handle_generate_object_map};
pub use preload::{handle_preload_info, handle_preload_run, handle_preload_watch};
pub use raw_memory::{handle_patch, handle_read, handle_scan, handle_write};
