// CLI tool with many command handlers - individual functions are legitimately complex
#![allow(clippy::too_many_lines)]

mod cli;
mod commands;
mod config;
mod dispatch;
mod file_io;
#[cfg(feature = "research")]
mod manifest;
mod memory;

use anyhow::Result;
use clap::Parser;

use cli::*;

fn main() -> Result<()> {
    // Shorthand: `bl4 1.sav get --level` → `bl4 save 1.sav get --level`
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.len() > 1 && args[1].ends_with(".sav") {
        let mut rewritten = vec![args[0].clone(), "save".to_string()];
        rewritten.extend_from_slice(&args[1..]);
        Cli::parse_from(rewritten)
    } else {
        Cli::parse()
    };

    match cli.command {
        Commands::Configure { steam_id, show } => {
            commands::configure::handle(steam_id, show)?;
        }

        Commands::Save { args } => dispatch::dispatch_save(args)?,

        Commands::Inspect {
            input,
            steam_id,
            full,
        } => {
            commands::save::inspect(&input, steam_id, full)?;
        }

        Commands::Serial { command } => dispatch::dispatch_serial(command)?,

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
        } => dispatch::dispatch_memory(preload, dump, maps, action)?,

        Commands::Launch { yes } => {
            commands::launch::handle(yes)?;
        }

        #[cfg(feature = "research")]
        Commands::Usmap { command } => dispatch::dispatch_usmap(command)?,

        #[cfg(feature = "research")]
        Commands::Extract { command } => dispatch::dispatch_extract(command)?,

        Commands::Idb { db, command } => dispatch::dispatch_idb(db, command)?,

        Commands::Ncs { command } => commands::ncs::handle_ncs_command(command)?,

        Commands::Drops { command } => commands::drops::handle(command)?,

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
