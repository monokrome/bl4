mod commands;
mod state;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "tauri")]
mod tauri_commands;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    server::run().await;
}

#[cfg(feature = "tauri")]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            tauri_commands::open_save,
            tauri_commands::save_changes,
            tauri_commands::get_save_info,
            tauri_commands::get_character,
            tauri_commands::set_character,
            tauri_commands::get_inventory,
            tauri_commands::get_item_detail,
            tauri_commands::get_bank,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

#[cfg(not(any(feature = "server", feature = "tauri")))]
fn main() {
    eprintln!("Error: Must enable 'server' or 'tauri' feature");
    eprintln!("  cargo run -p bl4-save-editor --features tauri");
    eprintln!("  cargo run -p bl4-save-editor --features server");
    std::process::exit(1);
}
