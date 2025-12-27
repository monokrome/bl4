//! Extract command handlers (requires 'research' feature)
//!
//! Handlers for data extraction from pak files and memory dumps.

mod manifest_extract;
mod minidump;
mod ncs;
mod orchestrator;
mod part_pools;

pub use manifest_extract::{
    handle_elements, handle_gear_types, handle_manufacturers, handle_rarities, handle_stats,
    handle_weapon_types,
};
pub use minidump::handle_minidump_to_exe;
pub use ncs::{
    handle_check as handle_ncs_check, handle_decompress as handle_ncs_decompress,
    handle_extract as handle_ncs_extract, handle_find as handle_ncs_find,
    handle_info as handle_ncs_info, handle_scan as handle_ncs_scan,
};
pub use orchestrator::handle_manifest;
pub use part_pools::handle_part_pools;
