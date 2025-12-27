mod cli;
mod commands;
mod config;
mod file_io;
#[cfg(feature = "research")]
mod manifest;
mod memory;

use anyhow::{bail, Context, Result};
use clap::Parser;
use config::Config;

use cli::*;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Configure { steam_id, show } => {
            commands::configure::handle(steam_id, show)?;
        }

        Commands::Save { command } => match command {
            SaveCommand::Decrypt {
                input,
                output,
                steam_id,
            } => {
                commands::save::decrypt(input.as_deref(), output.as_deref(), steam_id)?;
            }

            SaveCommand::Encrypt {
                input,
                output,
                steam_id,
            } => {
                commands::save::encrypt(input.as_deref(), output.as_deref(), steam_id)?;
            }

            SaveCommand::Edit {
                input,
                steam_id,
                backup,
            } => {
                commands::save::edit(&input, steam_id, backup)?;
            }

            SaveCommand::Get {
                input,
                steam_id,
                query,
                level,
                money,
                info,
                all,
            } => {
                commands::save::get(&input, steam_id, query.as_deref(), level, money, info, all)?;
            }

            SaveCommand::Set {
                input,
                steam_id,
                path,
                value,
                raw,
                backup,
            } => {
                commands::save::set(&input, steam_id, &path, &value, raw, backup)?;
            }
        },

        Commands::Inspect {
            input,
            steam_id,
            full,
        } => {
            commands::save::inspect(&input, steam_id, full)?;
        }

        Commands::Serial { command } => match command {
            SerialCommand::Decode {
                serial,
                verbose,
                debug,
                analyze,
                parts_db,
            } => {
                commands::serial::decode(&serial, verbose, debug, analyze, &parts_db)?;
            }

            SerialCommand::Encode { serial } => {
                commands::serial::encode(&serial)?;
            }

            SerialCommand::Compare { serial1, serial2 } => {
                commands::serial::compare(&serial1, &serial2)?;
            }

            SerialCommand::Modify {
                base,
                source,
                parts,
            } => {
                commands::serial::modify(&base, &source, &parts)?;
            }

            SerialCommand::BatchDecode { input, output } => {
                commands::serial::batch_decode(&input, &output)?;
            }
        },

        Commands::Parts {
            weapon,
            category,
            list,
            parts_db,
        } => {
            commands::parts::handle(weapon, category, list, &parts_db)?;
        }

        Commands::Memory {
            preload,
            dump,
            maps,
            action,
        } => {
            // Handle commands that don't require process attachment first
            match action {
                MemoryAction::BuildPartsDb {
                    ref input,
                    ref output,
                    ref categories,
                } => {
                    commands::memory::handle_build_parts_db(input, output, categories)?;
                    return Ok(());
                }
                MemoryAction::ExtractParts {
                    ref output,
                    list_fnames,
                } => {
                    commands::memory::handle_extract_parts(output, dump.as_deref(), list_fnames)?;
                    return Ok(());
                }
                MemoryAction::ExtractPartsRaw { ref output } => {
                    commands::memory::handle_extract_parts_raw(output, dump.as_deref())?;
                    return Ok(());
                }
                MemoryAction::FindObjectsByPattern { ref pattern, limit } => {
                    commands::memory::handle_find_objects_by_pattern(pattern, limit, dump.as_deref())?;
                    return Ok(());
                }
                MemoryAction::GenerateObjectMap { ref output } => {
                    commands::memory::handle_generate_object_map(
                        output.as_deref(),
                        dump.as_deref(),
                    )?;
                    return Ok(());
                }
                _ => {}
            }

            // Preload mode - only Monitor is supported, other commands need direct memory access
            if preload {
                match action {
                    MemoryAction::Monitor { .. } => {
                        // Monitor works the same in preload mode, fall through
                        // This is handled below in the main match
                    }
                    _ => {
                        bail!(
                            "This command is not available in --preload mode. \
                               Remove --preload to use direct memory injection."
                        );
                    }
                }
            }

            // Handle Preload commands (no memory attachment needed)
            if let MemoryAction::Preload { action: preload_action } = &action {
                match preload_action {
                    PreloadAction::Info => {
                        commands::memory::handle_preload_info()?;
                    }
                    PreloadAction::Run { capture, filter, winedebug, command } => {
                        commands::memory::handle_preload_run(
                            capture.as_deref(),
                            filter.as_deref(),
                            winedebug.as_deref(),
                            command,
                        )?;
                    }
                    PreloadAction::Watch { log_file } => {
                        commands::memory::handle_preload_watch(log_file)?;
                    }
                }
                return Ok(());
            }

            // Commands that require memory access (live process or dump file)
            // Create memory source based on options
            let (process, dump_file): (Option<memory::Bl4Process>, Option<memory::DumpFile>) =
                if let Some(dump_path) = &dump {
                    // Using dump file for offline analysis
                    let dump = if let Some(ref maps_path) = maps {
                        memory::DumpFile::open_with_maps(dump_path, maps_path)
                            .context("Failed to open dump file with maps")?
                    } else {
                        memory::DumpFile::open(dump_path).context("Failed to open dump file")?
                    };
                    (None, Some(dump))
                } else {
                    // Attach to live process
                    let proc = memory::Bl4Process::attach()
                        .context("Failed to attach to Borderlands 4 process")?;
                    (Some(proc), None)
                };

            // Helper macro to get memory source
            macro_rules! mem_source {
                () => {
                    if let Some(ref p) = process {
                        p as &dyn memory::MemorySource
                    } else if let Some(ref d) = dump_file {
                        d as &dyn memory::MemorySource
                    } else {
                        unreachable!()
                    }
                };
            }

            match action {
                MemoryAction::BuildPartsDb { .. } => unreachable!(), // Handled above before process attach
                MemoryAction::ExtractParts { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::ExtractPartsRaw { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::FindObjectsByPattern { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::GenerateObjectMap { .. } => unreachable!(), // Handled above with dump file
                MemoryAction::Preload { .. } => unreachable!(), // Handled above before process attach

                MemoryAction::Info => {
                    if let Some(ref proc) = process {
                        println!("{}", proc.info());
                    } else {
                        println!("Dump file mode - no live process info available");
                        println!("  Dump: {:?}", dump.as_ref().unwrap());
                        let source = mem_source!();
                        println!("  Regions: {}", source.regions().len());
                    }
                }

                MemoryAction::Discover { target } => {
                    let source = mem_source!();
                    commands::memory::handle_discover(source, &target)?;
                }

                MemoryAction::Objects { class, limit } => {
                    let source = mem_source!();
                    commands::memory::handle_objects(source, class.as_deref(), limit)?;
                }

                MemoryAction::Fname { index, debug } => {
                    let source = mem_source!();
                    commands::memory::handle_fname(source, index, debug)?;
                }

                MemoryAction::FnameSearch { query } => {
                    let source = mem_source!();
                    commands::memory::handle_fname_search(source, &query)?;
                }

                MemoryAction::FindClassUClass => {
                    let source = mem_source!();
                    commands::memory::handle_find_class_uclass(source)?;
                }

                MemoryAction::ListUClasses { limit, filter } => {
                    let source = mem_source!();
                    commands::memory::handle_list_uclasses(source, limit, filter.as_deref())?;
                }

                MemoryAction::ListObjects {
                    limit,
                    class_filter,
                    name_filter,
                    stats,
                } => {
                    let source = mem_source!();
                    commands::memory::handle_list_objects(
                        source,
                        limit,
                        class_filter.as_deref(),
                        name_filter.as_deref(),
                        stats,
                    )?;
                }

                MemoryAction::AnalyzeDump => {
                    let source = mem_source!();
                    commands::memory::handle_analyze_dump(source)?;
                }

                MemoryAction::DumpUsmap { output } => {
                    let source = mem_source!();
                    commands::memory::handle_dump_usmap(source, &output)?;
                }

                MemoryAction::ListInventory => {
                    // TODO: Find player controller, walk inventory array
                    bail!(
                        "Inventory listing not yet implemented. \
                        Need to locate player inventory structures first."
                    );
                }

                MemoryAction::Read { address, size } => {
                    let source = mem_source!();
                    commands::memory::handle_read(source, &address, size)?;
                }

                MemoryAction::Write { address, bytes } => {
                    let proc = process
                        .as_ref()
                        .context("Write requires a live process (not available in dump mode)")?;
                    commands::memory::handle_write(proc, &address, &bytes)?;
                }

                MemoryAction::Scan { pattern } => {
                    let source = mem_source!();
                    commands::memory::handle_scan(source, &pattern)?;
                }

                MemoryAction::Patch {
                    address,
                    nop,
                    bytes,
                } => {
                    let proc = process
                        .as_ref()
                        .context("Patch requires a live process (not available in dump mode)")?;
                    commands::memory::handle_patch(proc, &address, nop, bytes.as_deref())?;
                }

                MemoryAction::Monitor {
                    log_file,
                    filter,
                    game_only,
                } => {
                    commands::memory::handle_monitor(&log_file, filter.as_deref(), game_only)?;
                }

                MemoryAction::ScanString {
                    query,
                    before,
                    after,
                    limit,
                } => {
                    let source = mem_source!();
                    commands::memory::handle_scan_string(source, &query, before, after, limit)?;
                }

                MemoryAction::DumpParts { output } => {
                    let source = mem_source!();
                    commands::memory::handle_dump_parts(source, &output)?;
                }
            }
        }

        Commands::Launch { yes } => {
            commands::launch::handle(yes)?;
        }

        #[cfg(feature = "research")]
        Commands::Usmap {
            command: UsmapCommand::Info { path },
        } => {
            commands::usmap::handle_info(&path)?;
        }

        #[cfg(feature = "research")]
        Commands::Usmap {
            command:
                UsmapCommand::Search {
                    path,
                    pattern,
                    verbose,
                },
        } => {
            commands::usmap::handle_search(&path, &pattern, verbose)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::PartPools { input, output },
        } => {
            commands::extract::handle_part_pools(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Manufacturers { input, output },
        } => {
            commands::extract::handle_manufacturers(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::WeaponTypes { input, output },
        } => {
            commands::extract::handle_weapon_types(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::GearTypes { input, output },
        } => {
            commands::extract::handle_gear_types(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Elements { input, output },
        } => {
            commands::extract::handle_elements(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Rarities { input, output },
        } => {
            commands::extract::handle_rarities(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::Stats { input, output },
        } => {
            commands::extract::handle_stats(&input, &output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::MinidumpToExe { input, output, base },
        } => {
            commands::extract::handle_minidump_to_exe(&input, output, &base)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsCheck { input },
        } => {
            commands::extract::handle_ncs_check(&input)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsDecompress { input, output },
        } => {
            commands::extract::handle_ncs_decompress(&input, output)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsInfo { input },
        } => {
            commands::extract::handle_ncs_info(&input)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsFind { path, recursive },
        } => {
            commands::extract::handle_ncs_find(&path, recursive)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsScan { input, all },
        } => {
            commands::extract::handle_ncs_scan(&input, all)?;
        }

        #[cfg(feature = "research")]
        Commands::Extract {
            command: ExtractCommand::NcsExtract { input, output, decompress },
        } => {
            commands::extract::handle_ncs_extract(&input, &output, decompress)?;
        }

        Commands::Idb { db, command } => {
            match command {
                ItemsDbCommand::Init => commands::items_db::init(&db)?,
                ItemsDbCommand::Stats => commands::items_db::stats(&db)?,
                ItemsDbCommand::Salt => commands::items_db::salt(&db)?,
                ItemsDbCommand::Add {
                    serial,
                    name,
                    prefix,
                    manufacturer,
                    weapon_type,
                    rarity,
                    level,
                    element,
                } => commands::items_db::add(&db, &serial, name, prefix, manufacturer, weapon_type, rarity, level, element)?,
                ItemsDbCommand::Show { serial } => commands::items_db::show(&db, &serial)?,
                ItemsDbCommand::List {
                    manufacturer,
                    weapon_type,
                    element,
                    rarity,
                    format,
                    fields,
                } => commands::items_db::list(&db, manufacturer, weapon_type, element, rarity, format, fields)?,
                ItemsDbCommand::Attach {
                    image,
                    serial,
                    name,
                    popup,
                    detail,
                } => commands::items_db::attach(&db, &image, &serial, name, popup, detail)?,
                ItemsDbCommand::Import { path } => commands::items_db::import(&db, &path)?,
                ItemsDbCommand::Export { serial, output } => commands::items_db::export(&db, &serial, &output)?,
                ItemsDbCommand::Verify { serial, status, notes } => commands::items_db::verify(&db, &serial, &status, notes)?,
                ItemsDbCommand::DecodeAll { force } => commands::items_db::decode_all(&db, force)?,
                ItemsDbCommand::Decode { serial, all } => commands::items_db::decode(&db, serial, all)?,
                ItemsDbCommand::ImportSave { save, decode, legal, source } => {
                    commands::items_db::import_save(&db, &save, decode, legal, source)?
                }
                ItemsDbCommand::MarkLegal { ids } => commands::items_db::mark_legal(&db, &ids)?,
                ItemsDbCommand::SetSource { source, ids, where_clause } => {
                    commands::items_db::set_source(&db, &source, &ids, where_clause)?
                }
                ItemsDbCommand::Merge { source, dest } => commands::items_db::merge_databases(&source, &dest)?,
                ItemsDbCommand::SetValue {
                    serial,
                    field,
                    value,
                    source,
                    source_detail,
                    confidence,
                } => commands::items_db::set_value(&db, &serial, &field, &value, &source, source_detail, &confidence)?,
                ItemsDbCommand::GetValues { serial, field } => commands::items_db::get_values(&db, &serial, &field)?,
                ItemsDbCommand::MigrateValues { dry_run } => commands::items_db::migrate_values(&db, dry_run)?,
                ItemsDbCommand::Publish {
                    server,
                    serial,
                    attachments,
                    dry_run,
                } => commands::items_db::publish(&db, &server, serial, attachments, dry_run)?,
                ItemsDbCommand::Pull {
                    server,
                    authoritative,
                    dry_run,
                } => commands::items_db::pull(&db, &server, authoritative, dry_run)?,
            }
        }

        #[cfg(feature = "research")]
        Commands::Manifest {
            dump,
            paks,
            usmap,
            output,
            aes_key,
            skip_extract,
            extracted,
            skip_memory,
        } => {
            commands::extract::handle_manifest(
                dump.as_deref(),
                &paks,
                usmap,
                &output,
                aes_key.as_deref(),
                skip_extract,
                extracted,
                skip_memory,
            )?;
        }
    }

    Ok(())
}
