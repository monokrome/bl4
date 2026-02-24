use crate::commands::{
    self, CharacterInfo, InventoryItem, ItemDetail, SaveInfo, SetCharacterRequest,
};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn open_save(
    state: State<AppState>,
    path: String,
    steam_id: String,
) -> Result<SaveInfo, String> {
    commands::open_save_impl(&state, path, steam_id)
}

#[tauri::command]
pub fn save_changes(state: State<AppState>) -> Result<(), String> {
    commands::save_changes_impl(&state)
}

#[tauri::command]
pub fn get_save_info(state: State<AppState>) -> Result<Option<SaveInfo>, String> {
    commands::get_save_info_impl(&state)
}

#[tauri::command]
pub fn get_character(state: State<AppState>) -> Result<CharacterInfo, String> {
    commands::get_character_impl(&state)
}

#[tauri::command]
pub fn set_character(
    state: State<AppState>,
    request: SetCharacterRequest,
) -> Result<(), String> {
    commands::set_character_impl(&state, request)
}

#[tauri::command]
pub fn get_inventory(state: State<AppState>) -> Result<Vec<InventoryItem>, String> {
    commands::get_inventory_impl(&state)
}

#[tauri::command]
pub fn get_item_detail(state: State<AppState>, serial: String) -> Result<ItemDetail, String> {
    commands::get_item_detail_impl(&state, &serial)
}

#[tauri::command]
pub fn get_bank(state: State<AppState>) -> Result<commands::BankInfo, String> {
    commands::get_bank_impl(&state)
}
