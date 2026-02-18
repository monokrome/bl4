//! NCS command handlers

mod debug;
mod decompress;
mod extract;
mod format;
mod scan;
mod search;
mod show;
mod types;
mod util;

use anyhow::Result;

use crate::cli::NcsCommand;

// Re-export types for external use
#[allow(unused_imports)]
pub use types::{FileInfo, PartIndex, ScanResult, SearchMatch};

pub fn handle_ncs_command(command: NcsCommand) -> Result<()> {
    match command {
        NcsCommand::Scan {
            path,
            filter_type,
            verbose,
            json,
        } => scan::scan_directory(&path, filter_type.as_deref(), verbose, json),

        NcsCommand::Show {
            path,
            all_strings,
            hex,
            json,
            tsv,
        } => show::show_file(&path, all_strings, hex, json, tsv),

        NcsCommand::Search {
            path,
            pattern,
            all,
            limit,
        } => search::search_files(&path, &pattern, all, limit),

        NcsCommand::Extract {
            path,
            extract_type,
            output,
            json,
        } => extract::extract_by_type(&path, &extract_type, output.as_deref(), json),

        NcsCommand::Stats { path, formats } => scan::show_stats(&path, formats),

        #[cfg(target_os = "windows")]
        NcsCommand::Decompress {
            input,
            output,
            offset,
            raw,
            oodle_dll,
            oodle_exec,
        } => decompress::decompress_file(&input, output.as_deref(), offset, raw, oodle_dll.as_deref(), oodle_exec.as_deref()),

        #[cfg(not(target_os = "windows"))]
        NcsCommand::Decompress {
            input,
            output,
            offset,
            raw,
            oodle_exec,
            oodle_fifo,
        } => decompress::decompress_file(&input, output.as_deref(), offset, raw, oodle_exec.as_deref(), oodle_fifo),

        NcsCommand::Debug { path, hex, parse, offsets } => debug::debug_file(&path, hex, parse, offsets),
    }
}
