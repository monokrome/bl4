//! Native menu bar
//!
//! - macOS: Native system menu bar via muda
//! - Linux/Windows: Menus rendered in webview (see assets/index.html)

/// Menu item IDs for handling events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    Open,
    Save,
    SaveAs,
    Exit,
    Undo,
    Redo,
    Preferences,
    About,
}

// ============================================================================
// macOS: Native menu bar via muda
// ============================================================================

#[cfg(target_os = "macos")]
mod native {
    use super::MenuId;
    use muda::{
        accelerator::{Accelerator, Code, Modifiers},
        Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    };

    pub struct AppMenu {
        menu: Menu,
    }

    impl AppMenu {
        pub fn new() -> Self {
            let menu = Menu::new();

            // App menu (macOS convention)
            let app_menu = Submenu::new("BL4 Save Editor", true);
            let about = MenuItem::with_id("about", "About BL4 Save Editor", true, None);
            let preferences = MenuItem::with_id(
                "preferences",
                "Preferences...",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
            );
            app_menu.append(&about).ok();
            app_menu.append(&PredefinedMenuItem::separator()).ok();
            app_menu.append(&preferences).ok();
            app_menu.append(&PredefinedMenuItem::separator()).ok();
            app_menu.append(&PredefinedMenuItem::quit(None)).ok();

            // File menu
            let file_menu = Submenu::new("File", true);
            let open = MenuItem::with_id(
                "open",
                "Open...",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyO)),
            );
            let save = MenuItem::with_id(
                "save",
                "Save",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyS)),
            );
            let save_as = MenuItem::with_id(
                "save_as",
                "Save As...",
                true,
                Some(Accelerator::new(
                    Some(Modifiers::META | Modifiers::SHIFT),
                    Code::KeyS,
                )),
            );
            file_menu.append(&open).ok();
            file_menu.append(&PredefinedMenuItem::separator()).ok();
            file_menu.append(&save).ok();
            file_menu.append(&save_as).ok();

            // Edit menu
            let edit_menu = Submenu::new("Edit", true);
            let undo = MenuItem::with_id(
                "undo",
                "Undo",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyZ)),
            );
            let redo = MenuItem::with_id(
                "redo",
                "Redo",
                true,
                Some(Accelerator::new(
                    Some(Modifiers::META | Modifiers::SHIFT),
                    Code::KeyZ,
                )),
            );
            edit_menu.append(&undo).ok();
            edit_menu.append(&redo).ok();
            edit_menu.append(&PredefinedMenuItem::separator()).ok();
            edit_menu.append(&PredefinedMenuItem::cut(None)).ok();
            edit_menu.append(&PredefinedMenuItem::copy(None)).ok();
            edit_menu.append(&PredefinedMenuItem::paste(None)).ok();
            edit_menu.append(&PredefinedMenuItem::select_all(None)).ok();

            // Help menu
            let help_menu = Submenu::new("Help", true);

            // Build menu bar
            menu.append(&app_menu).ok();
            menu.append(&file_menu).ok();
            menu.append(&edit_menu).ok();
            menu.append(&help_menu).ok();

            Self { menu }
        }

        pub fn init_for_window(&self, _window: &winit::window::Window) {
            // macOS uses a global menu bar
            let _ = self.menu.init_for_nsapp();
        }

        pub fn poll_events(&self) -> Option<MenuId> {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                match event.id().0.as_str() {
                    "open" => Some(MenuId::Open),
                    "save" => Some(MenuId::Save),
                    "save_as" => Some(MenuId::SaveAs),
                    "undo" => Some(MenuId::Undo),
                    "redo" => Some(MenuId::Redo),
                    "preferences" => Some(MenuId::Preferences),
                    "about" => Some(MenuId::About),
                    _ => None,
                }
            } else {
                None
            }
        }
    }

    impl Default for AppMenu {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ============================================================================
// Linux/Windows: Stub (menus are rendered in webview)
// ============================================================================

#[cfg(not(target_os = "macos"))]
mod native {
    use super::MenuId;

    pub struct AppMenu;

    impl AppMenu {
        pub fn new() -> Self {
            Self
        }

        pub fn init_for_window(&self, _window: &winit::window::Window) {
            // Menus rendered in webview on Linux/Windows
        }

        pub fn poll_events(&self) -> Option<MenuId> {
            // No native menu events - handled via webview IPC
            None
        }
    }

    impl Default for AppMenu {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub use native::AppMenu;
