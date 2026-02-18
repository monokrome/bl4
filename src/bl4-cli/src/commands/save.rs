//! Save file command handlers

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::{MapAction, SaveArgs};
use crate::config::Config;

/// Get Steam ID from argument or config
pub fn get_steam_id(provided: Option<String>) -> Result<String> {
    if let Some(id) = provided {
        return Ok(id);
    }

    let config = Config::load()?;
    config.get_steam_id().map(String::from).context(
        "Steam ID not provided. Run 'bl4 configure --steam-id YOUR_STEAM_ID' to set a default.",
    )
}

/// Update backup metadata after editing a save file
pub fn update_backup_metadata(input: &Path) -> Result<()> {
    let (_, metadata_path) = bl4::backup::backup_paths(input);
    bl4::update_after_edit(input, &metadata_path).context("Failed to update backup metadata")
}

/// Common pattern for write operations: decrypt -> parse -> modify -> map -> serialize -> encrypt -> write
pub fn with_save_file(
    args: &SaveArgs,
    modify: impl FnOnce(&mut bl4::SaveFile) -> Result<()>,
) -> Result<()> {
    let steam_id = get_steam_id(args.steam_id.clone())?;

    if args.backup {
        let _ = bl4::smart_backup(&args.input).context("Failed to manage backup")?;
    }

    let encrypted =
        fs::read(&args.input).with_context(|| format!("Failed to read {}", args.input.display()))?;

    let yaml_data =
        bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

    let mut save = bl4::SaveFile::from_yaml(&yaml_data).context("Failed to parse save file")?;

    modify(&mut save)?;

    if let Some(ref map_action) = args.map {
        let zone = args.zone.as_deref();
        let (count, verb) = match map_action {
            MapAction::Reveal => (save.reveal_map(zone)?, "Reveal"),
            MapAction::Clear => (save.clear_map(zone)?, "Clear"),
        };
        match zone {
            Some(z) => eprintln!("{}ed map for zone: {}", verb, z),
            None => eprintln!("{}ed map for {} zone(s)", verb, count),
        }
    }

    let modified_yaml = save.to_yaml().context("Failed to serialize YAML")?;

    let encrypted =
        bl4::encrypt_sav(&modified_yaml, &steam_id).context("Failed to encrypt save file")?;

    fs::write(&args.input, &encrypted)
        .with_context(|| format!("Failed to write {}", args.input.display()))?;

    if args.backup {
        update_backup_metadata(&args.input)?;
    }

    Ok(())
}

/// Handle `save decrypt` command
pub fn decrypt(input: &Path, output: Option<&Path>, steam_id: Option<String>) -> Result<()> {
    let steam_id = get_steam_id(steam_id)?;
    let encrypted =
        fs::read(input).with_context(|| format!("Failed to read {}", input.display()))?;
    let yaml_data =
        bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;
    crate::file_io::write_output(output, &yaml_data)?;
    Ok(())
}

/// Handle `save encrypt` command
pub fn encrypt(sav_path: &Path, yaml_input: Option<&Path>, steam_id: Option<String>) -> Result<()> {
    let steam_id = get_steam_id(steam_id)?;
    let yaml_data = crate::file_io::read_input(yaml_input)?;
    let encrypted =
        bl4::encrypt_sav(&yaml_data, &steam_id).context("Failed to encrypt YAML data")?;
    fs::write(sav_path, &encrypted)
        .with_context(|| format!("Failed to write {}", sav_path.display()))?;
    Ok(())
}

/// Handle `save edit` command
pub fn edit(args: &SaveArgs) -> Result<()> {
    let editor_str = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let editor_parts: Vec<&str> = editor_str.split_whitespace().collect();
    let (editor, editor_args) = if editor_parts.is_empty() {
        ("vim", vec![])
    } else {
        (editor_parts[0], editor_parts[1..].to_vec())
    };

    with_save_file(args, |save| {
        let yaml_data = save.to_yaml().context("Failed to serialize YAML for editing")?;

        let temp_path = args.input.with_extension("yaml.tmp");
        let abs_temp_path = fs::canonicalize(temp_path.parent().unwrap())
            .unwrap()
            .join(temp_path.file_name().unwrap());

        fs::write(&abs_temp_path, &yaml_data)
            .with_context(|| format!("Failed to write temp file {}", abs_temp_path.display()))?;

        let status = Command::new(editor)
            .args(&editor_args)
            .arg(&abs_temp_path)
            .status()
            .with_context(|| format!("Failed to launch editor: {}", editor))?;

        if !status.success() {
            let _ = fs::remove_file(&abs_temp_path);
            bail!("Editor exited with non-zero status");
        }

        let modified_yaml =
            fs::read(&abs_temp_path).context("Failed to read modified temp file")?;

        fs::remove_file(&abs_temp_path).context("Failed to remove temp file")?;

        *save =
            bl4::SaveFile::from_yaml(&modified_yaml).context("Failed to parse modified YAML")?;

        Ok(())
    })
}

/// Handle `save get` command
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn get(
    input: &Path,
    steam_id: Option<String>,
    query: Option<&str>,
    level: bool,
    money: bool,
    info: bool,
    all: bool,
) -> Result<()> {
    let steam_id = get_steam_id(steam_id)?;
    let encrypted =
        fs::read(input).with_context(|| format!("Failed to read {}", input.display()))?;

    let yaml_data =
        bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

    let save = bl4::SaveFile::from_yaml(&yaml_data).context("Failed to parse save file")?;

    if let Some(query_path) = query {
        let result = save.get(query_path).context("Query failed")?;
        println!("{}", serde_yaml::to_string(&result)?);
        return Ok(());
    }

    let show_all = all || (!level && !money && !info);

    if show_all || info {
        if let Some(name) = save.get_character_name() {
            println!("Character: {}", name);
        }
        if let Some(class) = save.get_character_class() {
            println!("Class: {}", class);
        }
        if let Some(diff) = save.get_difficulty() {
            println!("Difficulty: {}", diff);
        }
        if show_all || info {
            println!();
        }
    }

    if show_all || level {
        if let Some((lvl, pts)) = save.get_character_level() {
            println!("Character Level: {} ({} XP)", lvl, pts);
        }
        if let Some((lvl, pts)) = save.get_specialization_level() {
            println!("Specialization Level: {} ({} XP)", lvl, pts);
        }
        if show_all || level {
            println!();
        }
    }

    if show_all || money {
        if let Some(cash) = save.get_cash() {
            println!("Cash: {}", cash);
        }
        if let Some(eridium) = save.get_eridium() {
            println!("Eridium: {}", eridium);
        }
    }

    Ok(())
}

/// Handle `save set` command
pub fn set(args: &SaveArgs, path: &str, value: &str, raw: bool) -> Result<()> {
    with_save_file(args, |save| {
        if raw {
            eprintln!("Setting {} = {} (raw YAML)", path, value);
            save.set_raw(path, value)
                .context("Failed to set raw value")?;
        } else {
            let new_value = bl4::SaveFile::parse_value(value);
            eprintln!("Setting {} = {}", path, value);
            save.set(path, new_value).context("Failed to set value")?;
        }
        Ok(())
    })
}

/// Handle standalone `--map` (no subcommand action)
pub fn map_only(args: &SaveArgs) -> Result<()> {
    with_save_file(args, |_| Ok(()))
}

/// Handle `inspect` command
pub fn inspect(input: &Path, steam_id: Option<String>, full: bool) -> Result<()> {
    let steam_id = get_steam_id(steam_id)?;

    let encrypted =
        fs::read(input).with_context(|| format!("Failed to read {}", input.display()))?;

    let yaml_data =
        bl4::decrypt_sav(&encrypted, &steam_id).context("Failed to decrypt save file")?;

    if full {
        println!("{}", String::from_utf8_lossy(&yaml_data));
    } else {
        let save: serde_yaml::Value =
            serde_yaml::from_slice(&yaml_data).context("Failed to parse YAML")?;

        println!("Save file structure:");
        if let Some(obj) = save.as_mapping() {
            for key in obj.keys() {
                println!("  - {}", key.as_str().unwrap_or("?"));
            }
        }

        println!("\nUse --full to see complete YAML");
    }

    Ok(())
}
