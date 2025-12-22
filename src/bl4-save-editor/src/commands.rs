use crate::state::{AppState, LoadedSave};
use bl4::{decrypt_sav, encrypt_sav, SaveFile};
use bl4_idb::{ItemsRepository, SqliteDb};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveInfo {
    pub path: String,
    pub is_profile: bool,
    pub modified: bool,
    pub character_name: Option<String>,
    pub character_class: Option<String>,
    pub difficulty: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub name: Option<String>,
    pub class: Option<String>,
    pub difficulty: Option<String>,
    pub level: Option<u64>,
    pub xp: Option<u64>,
    pub specialization_level: Option<u64>,
    pub specialization_xp: Option<u64>,
    pub cash: Option<u64>,
    pub eridium: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryItem {
    pub slot: u32,
    pub serial: String,
    pub state_flags: u32,
    pub is_favorite: bool,
    pub is_junk: bool,
    pub is_equipped: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetCharacterRequest {
    pub name: Option<String>,
    pub cash: Option<u64>,
    pub eridium: Option<u64>,
    pub xp: Option<u64>,
    pub specialization_xp: Option<u64>,
}

/// Open and decrypt a save file.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn open_save(
    state: tauri::State<AppState>,
    path: String,
    steam_id: String,
) -> Result<SaveInfo, String> {
    open_save_impl(&state, path, steam_id)
}

/// Save and encrypt changes.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn save_changes(state: tauri::State<AppState>) -> Result<(), String> {
    save_changes_impl(&state)
}

/// Get info about the currently loaded save.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_save_info(state: tauri::State<AppState>) -> Result<Option<SaveInfo>, String> {
    get_save_info_impl(&state)
}

/// Get character attributes.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_character(state: tauri::State<AppState>) -> Result<CharacterInfo, String> {
    get_character_impl(&state)
}

/// Update character attributes.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn set_character(
    state: tauri::State<AppState>,
    request: SetCharacterRequest,
) -> Result<(), String> {
    set_character_impl(&state, request)
}

/// Get inventory items.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_inventory(state: tauri::State<AppState>) -> Result<Vec<InventoryItem>, String> {
    get_inventory_impl(&state)
}

/// Connect to items database.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn connect_db(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    connect_db_impl(&state, path)
}

/// Sync items from save to bank.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn sync_to_bank(state: tauri::State<AppState>, serials: Vec<String>) -> Result<u32, String> {
    sync_to_bank_impl(&state, serials)
}

/// Sync items from bank to save.
#[cfg(feature = "desktop")]
#[tauri::command]
pub fn sync_from_bank(state: tauri::State<AppState>, serials: Vec<String>) -> Result<u32, String> {
    sync_from_bank_impl(&state, serials)
}

// Implementation functions (shared between desktop and server)

pub fn open_save_impl(state: &AppState, path: String, steam_id: String) -> Result<SaveInfo, String> {
    let path_buf = PathBuf::from(&path);
    let is_profile = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase().contains("profile"))
        .unwrap_or(false);

    let encrypted = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    let yaml_data =
        decrypt_sav(&encrypted, &steam_id).map_err(|e| format!("Failed to decrypt: {}", e))?;

    let save =
        SaveFile::from_yaml(&yaml_data).map_err(|e| format!("Failed to parse save: {}", e))?;

    let info = SaveInfo {
        path: path.clone(),
        is_profile,
        modified: false,
        character_name: save.get_character_name().map(String::from),
        character_class: save.get_character_class().map(String::from),
        difficulty: save.get_difficulty().map(String::from),
    };

    let loaded = LoadedSave {
        path: path_buf,
        save,
        is_profile,
        steam_id,
        modified: false,
    };

    let mut current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    *current = Some(loaded);

    Ok(info)
}

pub fn save_changes_impl(state: &AppState) -> Result<(), String> {
    let mut current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    let loaded = current.as_mut().ok_or("No save file loaded")?;

    let yaml_data = loaded
        .save
        .to_yaml()
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    let encrypted = encrypt_sav(&yaml_data, &loaded.steam_id)
        .map_err(|e| format!("Failed to encrypt: {}", e))?;

    fs::write(&loaded.path, encrypted).map_err(|e| format!("Failed to write file: {}", e))?;

    loaded.modified = false;
    Ok(())
}

pub fn get_save_info_impl(state: &AppState) -> Result<Option<SaveInfo>, String> {
    let current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    Ok(current.as_ref().map(|loaded| SaveInfo {
        path: loaded.path.to_string_lossy().to_string(),
        is_profile: loaded.is_profile,
        modified: loaded.modified,
        character_name: loaded.save.get_character_name().map(String::from),
        character_class: loaded.save.get_character_class().map(String::from),
        difficulty: loaded.save.get_difficulty().map(String::from),
    }))
}

pub fn get_character_impl(state: &AppState) -> Result<CharacterInfo, String> {
    let current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    let loaded = current.as_ref().ok_or("No save file loaded")?;
    let save = &loaded.save;

    let (level, xp) = save.get_character_level().unwrap_or((0, 0));
    let (spec_level, spec_xp) = save.get_specialization_level().unwrap_or((0, 0));

    Ok(CharacterInfo {
        name: save.get_character_name().map(String::from),
        class: save.get_character_class().map(String::from),
        difficulty: save.get_difficulty().map(String::from),
        level: Some(level),
        xp: Some(xp),
        specialization_level: Some(spec_level),
        specialization_xp: Some(spec_xp),
        cash: save.get_cash(),
        eridium: save.get_eridium(),
    })
}

pub fn set_character_impl(state: &AppState, request: SetCharacterRequest) -> Result<(), String> {
    let mut current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    let loaded = current.as_mut().ok_or("No save file loaded")?;
    let save = &mut loaded.save;

    if let Some(name) = request.name {
        save.set_character_name(&name)
            .map_err(|e| format!("Failed to set name: {}", e))?;
    }

    if let Some(cash) = request.cash {
        save.set_cash(cash)
            .map_err(|e| format!("Failed to set cash: {}", e))?;
    }

    if let Some(eridium) = request.eridium {
        save.set_eridium(eridium)
            .map_err(|e| format!("Failed to set eridium: {}", e))?;
    }

    if let Some(xp) = request.xp {
        save.set_character_xp(xp)
            .map_err(|e| format!("Failed to set XP: {}", e))?;
    }

    if let Some(spec_xp) = request.specialization_xp {
        save.set_specialization_xp(spec_xp)
            .map_err(|e| format!("Failed to set specialization XP: {}", e))?;
    }

    loaded.modified = true;
    Ok(())
}

pub fn get_inventory_impl(state: &AppState) -> Result<Vec<InventoryItem>, String> {
    let current = state
        .current_save
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    let loaded = current.as_ref().ok_or("No save file loaded")?;
    let save = &loaded.save;

    let mut items = Vec::new();

    // Try to get backpack items (character save)
    for slot in 0..50 {
        let serial_path = format!("state.inventory.items.backpack.slot_{}.serial", slot);
        let flags_path = format!("state.inventory.items.backpack.slot_{}.state_flags", slot);

        if let Ok(serial_val) = save.get(&serial_path) {
            if let Some(serial) = serial_val.as_str() {
                let state_flags = save
                    .get(&flags_path)
                    .ok()
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;

                let flags = bl4::StateFlags::from_raw(state_flags);
                items.push(InventoryItem {
                    slot,
                    serial: serial.to_string(),
                    state_flags,
                    is_favorite: flags.is_favorite(),
                    is_junk: flags.is_junk(),
                    is_equipped: flags.is_equipped(),
                });
            }
        }
    }

    Ok(items)
}

pub fn connect_db_impl(state: &AppState, path: String) -> Result<(), String> {
    let db = SqliteDb::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    db.init().map_err(|e| format!("Failed to init database: {}", e))?;

    let mut db_lock = state
        .items_db
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    *db_lock = Some(db);

    Ok(())
}

pub fn sync_to_bank_impl(state: &AppState, serials: Vec<String>) -> Result<u32, String> {
    let db_lock = state
        .items_db
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;

    let db = db_lock.as_ref().ok_or("No database connected")?;

    let mut count = 0;
    for serial in serials {
        if db.add_item(&serial).is_ok() {
            count += 1;
        }
    }

    Ok(count)
}

#[allow(dead_code)]
pub fn sync_from_bank_impl(_state: &AppState, _serials: Vec<String>) -> Result<u32, String> {
    // TODO: Implement adding items from bank to save inventory
    // This requires finding empty slots and using ChangeSet to add items
    Err("Not yet implemented".to_string())
}
