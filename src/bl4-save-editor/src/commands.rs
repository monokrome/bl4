use serde::Serialize;
use std::fs;
use std::sync::Mutex;
use tauri::State;

#[derive(Default)]
pub struct SaveState {
    save: Option<bl4::SaveFile>,
    steam_id: Option<String>,
    save_path: Option<String>,
}

#[derive(Serialize)]
pub struct CharacterInfo {
    name: Option<String>,
    class: Option<String>,
    difficulty: Option<String>,
    level: Option<u64>,
    xp: Option<u64>,
    cash: Option<u64>,
    eridium: Option<u64>,
}

#[tauri::command]
pub async fn load_save(
    path: String,
    steam_id: String,
    state: State<'_, Mutex<SaveState>>,
) -> Result<String, String> {
    let encrypted = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
    let yaml = bl4::decrypt_sav(&encrypted, &steam_id)
        .map_err(|e| format!("Failed to decrypt save: {e}"))?;
    let save =
        bl4::SaveFile::from_yaml(&yaml).map_err(|e| format!("Failed to parse save: {e}"))?;

    let name = save.get_character_name().map(String::from);

    let mut app = state.lock().map_err(|e| format!("Lock failed: {e}"))?;
    app.save = Some(save);
    app.steam_id = Some(steam_id);
    app.save_path = Some(path);

    Ok(name.unwrap_or_else(|| "Unknown".into()))
}

#[tauri::command]
pub async fn get_character_info(
    state: State<'_, Mutex<SaveState>>,
) -> Result<CharacterInfo, String> {
    let app = state.lock().map_err(|e| format!("Lock failed: {e}"))?;
    let save = app.save.as_ref().ok_or("No save loaded")?;

    let (level, xp) = save
        .get_character_level()
        .map(|(l, x)| (Some(l as u64), Some(x)))
        .unwrap_or((None, None));

    Ok(CharacterInfo {
        name: save.get_character_name().map(String::from),
        class: save.get_character_class().map(String::from),
        difficulty: save.get_difficulty().map(String::from),
        level,
        xp,
        cash: save.get_cash(),
        eridium: save.get_eridium(),
    })
}

#[tauri::command]
pub async fn set_item_level(
    level: u8,
    state: State<'_, Mutex<SaveState>>,
) -> Result<u32, String> {
    let mut app = state.lock().map_err(|e| format!("Lock failed: {e}"))?;
    let save = app.save.as_mut().ok_or("No save loaded")?;

    let mut changeset = bl4::ChangeSet::new();
    let count = changeset.set_all_item_levels(save, level);
    changeset
        .apply(save)
        .map_err(|e| format!("Failed to apply changes: {e}"))?;

    Ok(count)
}
