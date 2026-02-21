//! UE data table extraction from gbx_ue_data_table NCS files
//!
//! Parses the structured data tables that contain game balance values,
//! enemy stats, anointment parameters, elemental damage scales, and more.

mod extract;
mod types;

pub use extract::{
    extract_data_tables, extract_data_tables_from_dir, table_to_tsv, write_data_tables,
};
pub use types::{DataTable, DataTableManifest, DataTableRow};
