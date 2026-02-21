//! UE data table extraction from gbx_ue_data_table NCS files
//!
//! Parses the structured data tables that contain game balance values,
//! enemy stats, anointment parameters, elemental damage scales, and more.

mod extract;
mod types;

pub use extract::{extract_data_tables, extract_data_tables_from_dir, tables_summary_tsv};
pub use types::{DataTable, DataTableManifest, DataTableRow};
