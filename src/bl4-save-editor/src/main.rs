mod commands;
mod state;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "desktop")]
use state::AppState;

#[cfg(feature = "desktop")]
fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_save,
            commands::save_changes,
            commands::get_save_info,
            commands::get_character,
            commands::set_character,
            commands::get_inventory,
            commands::connect_db,
            commands::sync_to_bank,
            commands::sync_from_bank,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    server::run().await;
}

#[cfg(not(any(feature = "desktop", feature = "server")))]
fn main() {
    eprintln!("Error: Must enable either 'desktop' or 'server' feature");
    eprintln!("  cargo run -p bl4-save-editor              # desktop (default)");
    eprintln!("  cargo run -p bl4-save-editor --features server --no-default-features");
    std::process::exit(1);
}
