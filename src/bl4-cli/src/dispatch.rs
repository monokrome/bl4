//! Command dispatch functions
//!
//! Breaks up the main match statement into focused dispatch functions.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::cli::*;
use crate::commands;
use crate::memory;

/// Dispatch save subcommands
pub fn dispatch_save(command: SaveCommand) -> Result<()> {
    match command {
        SaveCommand::Decrypt {
            input,
            output,
            steam_id,
        } => commands::save::decrypt(input.as_deref(), output.as_deref(), steam_id),

        SaveCommand::Encrypt {
            input,
            output,
            steam_id,
        } => commands::save::encrypt(input.as_deref(), output.as_deref(), steam_id),

        SaveCommand::Edit {
            input,
            steam_id,
            backup,
        } => commands::save::edit(&input, steam_id, backup),

        SaveCommand::Get {
            input,
            steam_id,
            query,
            level,
            money,
            info,
            all,
        } => commands::save::get(&input, steam_id, query.as_deref(), level, money, info, all),

        SaveCommand::Set {
            input,
            steam_id,
            path,
            value,
            raw,
            backup,
        } => commands::save::set(&input, steam_id, &path, &value, raw, backup),
    }
}

/// Dispatch serial subcommands
pub fn dispatch_serial(command: SerialCommand) -> Result<()> {
    match command {
        SerialCommand::Decode {
            serial,
            verbose,
            debug,
            analyze,
            parts_db,
        } => commands::serial::decode(&serial, verbose, debug, analyze, &parts_db),

        SerialCommand::Encode { serial } => commands::serial::encode(&serial),

        SerialCommand::Compare { serial1, serial2 } => {
            commands::serial::compare(&serial1, &serial2)
        }

        SerialCommand::Modify {
            base,
            source,
            parts,
        } => commands::serial::modify(&base, &source, &parts),

        SerialCommand::BatchDecode { input, output } => {
            commands::serial::batch_decode(&input, &output)
        }
    }
}

/// Dispatch items database subcommands
pub fn dispatch_idb(db: PathBuf, command: ItemsDbCommand) -> Result<()> {
    let db = db.as_path();

    match command {
        ItemsDbCommand::Init => commands::items_db::init(db),
        ItemsDbCommand::Stats => commands::items_db::stats(db),
        ItemsDbCommand::Salt => commands::items_db::salt(db),
        ItemsDbCommand::Add {
            serial,
            name,
            prefix,
            manufacturer,
            weapon_type,
            rarity,
            level,
            element,
        } => commands::items_db::add(
            db,
            &serial,
            name,
            prefix,
            manufacturer,
            weapon_type,
            rarity,
            level,
            element,
        ),
        ItemsDbCommand::Show { serial } => commands::items_db::show(db, &serial),
        ItemsDbCommand::List {
            manufacturer,
            weapon_type,
            element,
            rarity,
            format,
            fields,
        } => commands::items_db::list(
            db,
            manufacturer,
            weapon_type,
            element,
            rarity,
            format,
            fields,
        ),
        ItemsDbCommand::Attach {
            image,
            serial,
            name,
            popup,
            detail,
        } => commands::items_db::attach(db, &image, &serial, name, popup, detail),
        ItemsDbCommand::Import { path } => commands::items_db::import(db, &path),
        ItemsDbCommand::Export { serial, output } => {
            commands::items_db::export(db, &serial, &output)
        }
        ItemsDbCommand::Verify {
            serial,
            status,
            notes,
        } => commands::items_db::verify(db, &serial, &status, notes),
        ItemsDbCommand::DecodeAll { force } => commands::items_db::decode_all(db, force),
        ItemsDbCommand::Decode { serial, all } => commands::items_db::decode(db, serial, all),
        ItemsDbCommand::ImportSave {
            save,
            decode,
            legal,
            source,
        } => commands::items_db::import_save(db, &save, decode, legal, source),
        ItemsDbCommand::MarkLegal { ids } => commands::items_db::mark_legal(db, &ids),
        ItemsDbCommand::SetSource {
            source,
            ids,
            where_clause,
        } => commands::items_db::set_source(db, &source, &ids, where_clause),
        ItemsDbCommand::Merge { source, dest } => {
            commands::items_db::merge_databases(&source, &dest)
        }
        ItemsDbCommand::SetValue {
            serial,
            field,
            value,
            source,
            source_detail,
            confidence,
        } => commands::items_db::set_value(
            db,
            &serial,
            &field,
            &value,
            &source,
            source_detail,
            &confidence,
        ),
        ItemsDbCommand::GetValues { serial, field } => {
            commands::items_db::get_values(db, &serial, &field)
        }
        ItemsDbCommand::MigrateValues { dry_run } => {
            commands::items_db::migrate_values(db, dry_run)
        }
        ItemsDbCommand::Publish {
            server,
            serial,
            attachments,
            dry_run,
        } => commands::items_db::publish(db, &server, serial, attachments, dry_run),
        ItemsDbCommand::Pull {
            server,
            authoritative,
            dry_run,
        } => commands::items_db::pull(db, &server, authoritative, dry_run),
    }
}

/// Handle preload-specific actions (no memory attachment needed)
fn handle_preload_action(action: &PreloadAction) -> Result<()> {
    match action {
        PreloadAction::Info => commands::memory::handle_preload_info(),
        PreloadAction::Run {
            capture,
            filter,
            winedebug,
            command,
        } => commands::memory::handle_preload_run(
            capture.as_deref(),
            filter.as_deref(),
            winedebug.as_deref(),
            command,
        ),
        PreloadAction::Watch { log_file } => commands::memory::handle_preload_watch(log_file),
    }
}

/// Handle memory actions that don't require process attachment
fn handle_offline_memory_action(action: &MemoryAction, dump: Option<&Path>) -> Result<bool> {
    match action {
        MemoryAction::BuildPartsDb {
            input,
            output,
            categories,
        } => {
            commands::memory::handle_build_parts_db(input, output, categories)?;
            Ok(true)
        }
        MemoryAction::ExtractParts {
            output,
            list_fnames,
        } => {
            commands::memory::handle_extract_parts(output, dump, *list_fnames)?;
            Ok(true)
        }
        MemoryAction::ExtractPartsRaw { output } => {
            commands::memory::handle_extract_parts_raw(output, dump)?;
            Ok(true)
        }
        MemoryAction::FindObjectsByPattern { pattern, limit } => {
            commands::memory::handle_find_objects_by_pattern(pattern, *limit, dump)?;
            Ok(true)
        }
        MemoryAction::GenerateObjectMap { output } => {
            commands::memory::handle_generate_object_map(output.as_deref(), dump)?;
            Ok(true)
        }
        MemoryAction::Preload {
            action: preload_action,
        } => {
            handle_preload_action(preload_action)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Handle memory actions that require a memory source
fn handle_live_memory_action(
    action: MemoryAction,
    process: Option<&memory::Bl4Process>,
    dump_file: Option<&memory::DumpFile>,
    dump_path: &Option<PathBuf>,
) -> Result<()> {
    // Helper to get memory source
    let source: &dyn memory::MemorySource = if let Some(p) = process {
        p
    } else if let Some(d) = dump_file {
        d
    } else {
        unreachable!()
    };

    match action {
        MemoryAction::BuildPartsDb { .. }
        | MemoryAction::ExtractParts { .. }
        | MemoryAction::ExtractPartsRaw { .. }
        | MemoryAction::FindObjectsByPattern { .. }
        | MemoryAction::GenerateObjectMap { .. }
        | MemoryAction::Preload { .. } => unreachable!(),

        MemoryAction::Info => {
            if let Some(proc) = process {
                println!("{}", proc.info());
            } else {
                println!("Dump file mode - no live process info available");
                println!("  Dump: {:?}", dump_path.as_ref().unwrap());
                println!("  Regions: {}", source.regions().len());
            }
            Ok(())
        }

        MemoryAction::Discover { target } => commands::memory::handle_discover(source, &target),

        MemoryAction::Objects { class, limit } => {
            commands::memory::handle_objects(source, class.as_deref(), limit)
        }

        MemoryAction::Fname { index, debug } => {
            commands::memory::handle_fname(source, index, debug)
        }

        MemoryAction::FnameSearch { query } => {
            commands::memory::handle_fname_search(source, &query)
        }

        MemoryAction::FindClassUClass => commands::memory::handle_find_class_uclass(source),

        MemoryAction::ListUClasses { limit, filter } => {
            commands::memory::handle_list_uclasses(source, limit, filter.as_deref())
        }

        MemoryAction::ListObjects {
            limit,
            class_filter,
            name_filter,
            stats,
        } => commands::memory::handle_list_objects(
            source,
            limit,
            class_filter.as_deref(),
            name_filter.as_deref(),
            stats,
        ),

        MemoryAction::AnalyzeDump => commands::memory::handle_analyze_dump(source),

        MemoryAction::DumpUsmap { output } => commands::memory::handle_dump_usmap(source, &output),

        MemoryAction::ListInventory => {
            bail!(
                "Inventory listing not yet implemented. \
                Need to locate player inventory structures first."
            )
        }

        MemoryAction::Read { address, size } => {
            commands::memory::handle_read(source, &address, size)
        }

        MemoryAction::Write { address, bytes } => {
            let proc =
                process.context("Write requires a live process (not available in dump mode)")?;
            commands::memory::handle_write(proc, &address, &bytes)
        }

        MemoryAction::Scan { pattern } => commands::memory::handle_scan(source, &pattern),

        MemoryAction::Patch {
            address,
            nop,
            bytes,
        } => {
            let proc =
                process.context("Patch requires a live process (not available in dump mode)")?;
            commands::memory::handle_patch(proc, &address, nop, bytes.as_deref())
        }

        MemoryAction::Monitor {
            log_file,
            filter,
            game_only,
        } => commands::memory::handle_monitor(&log_file, filter.as_deref(), game_only),

        MemoryAction::ScanString {
            query,
            before,
            after,
            limit,
        } => commands::memory::handle_scan_string(source, &query, before, after, limit),

        MemoryAction::DumpParts { output } => commands::memory::handle_dump_parts(source, &output),
    }
}

/// Dispatch memory subcommands
pub fn dispatch_memory(
    preload: bool,
    dump: Option<PathBuf>,
    maps: Option<PathBuf>,
    action: MemoryAction,
) -> Result<()> {
    // Handle offline actions first
    if handle_offline_memory_action(&action, dump.as_deref())? {
        return Ok(());
    }

    // Check preload mode restrictions
    if preload {
        match action {
            MemoryAction::Monitor { .. } => {} // Monitor works in preload mode
            _ => {
                bail!(
                    "This command is not available in --preload mode. \
                    Remove --preload to use direct memory injection."
                );
            }
        }
    }

    // Create memory source
    let (process, dump_file): (Option<memory::Bl4Process>, Option<memory::DumpFile>) =
        if let Some(ref dump_path) = dump {
            let dump = if let Some(ref maps_path) = maps {
                memory::DumpFile::open_with_maps(dump_path, maps_path)
                    .context("Failed to open dump file with maps")?
            } else {
                memory::DumpFile::open(dump_path).context("Failed to open dump file")?
            };
            (None, Some(dump))
        } else {
            let proc = memory::Bl4Process::attach()
                .context("Failed to attach to Borderlands 4 process")?;
            (Some(proc), None)
        };

    handle_live_memory_action(action, process.as_ref(), dump_file.as_ref(), &dump)
}

/// Dispatch usmap subcommands (research feature)
#[cfg(feature = "research")]
pub fn dispatch_usmap(command: UsmapCommand) -> Result<()> {
    match command {
        UsmapCommand::Info { path } => commands::usmap::handle_info(&path),
        UsmapCommand::Search {
            path,
            pattern,
            verbose,
        } => commands::usmap::handle_search(&path, &pattern, verbose),
    }
}

/// Dispatch extract subcommands (research feature)
#[cfg(feature = "research")]
pub fn dispatch_extract(command: ExtractCommand) -> Result<()> {
    match command {
        ExtractCommand::PartPools { input, output } => {
            commands::extract::handle_part_pools(&input, &output)
        }
        ExtractCommand::Manufacturers { input, output } => {
            commands::extract::handle_manufacturers(&input, &output)
        }
        ExtractCommand::WeaponTypes { input, output } => {
            commands::extract::handle_weapon_types(&input, &output)
        }
        ExtractCommand::GearTypes { input, output } => {
            commands::extract::handle_gear_types(&input, &output)
        }
        ExtractCommand::Elements { input, output } => {
            commands::extract::handle_elements(&input, &output)
        }
        ExtractCommand::Rarities { input, output } => {
            commands::extract::handle_rarities(&input, &output)
        }
        ExtractCommand::Stats { input, output } => commands::extract::handle_stats(&input, &output),
        ExtractCommand::MinidumpToExe {
            input,
            output,
            base,
        } => commands::extract::handle_minidump_to_exe(&input, output, &base),
        ExtractCommand::NcsCheck { input } => commands::extract::handle_ncs_check(&input),
        ExtractCommand::NcsDecompress { input, output } => {
            commands::extract::handle_ncs_decompress(&input, output)
        }
        ExtractCommand::NcsInfo { input } => commands::extract::handle_ncs_info(&input),
        ExtractCommand::NcsFind { path, recursive } => {
            commands::extract::handle_ncs_find(&path, recursive)
        }
        ExtractCommand::NcsScan { input, all } => commands::extract::handle_ncs_scan(&input, all),
        ExtractCommand::NcsExtract {
            input,
            output,
            decompress,
        } => commands::extract::handle_ncs_extract(&input, &output, decompress),
    }
}
