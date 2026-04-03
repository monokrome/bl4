#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Workaround for Wayland DMA-BUF protocol error in WebKitGTK (tauri#10702)
    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
