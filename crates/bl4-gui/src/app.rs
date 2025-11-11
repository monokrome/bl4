use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct SaveFileEntry {
    pub path: PathBuf,
    pub name: String,
    pub save_data: Option<bl4::SaveFile>,
    pub is_modified: bool,
    pub changeset: bl4::ChangeSet,
}

impl SaveFileEntry {
    pub fn save_type(&self) -> SaveType {
        if self.name == "profile.sav" {
            SaveType::Profile
        } else {
            SaveType::Character
        }
    }
}

pub struct AppState {
    pub save_directory: Option<PathBuf>,
    pub save_files: HashMap<String, SaveFileEntry>,
    pub selected_save: Option<String>,
    pub steam_id: String,
    pub change_count: usize,
}

impl Default for AppState {
    fn default() -> Self {
        // Try to load saved config
        let config = bl4_cli_config::Config::load().ok();
        let steam_id = config
            .and_then(|c| c.get_steam_id().map(String::from))
            .unwrap_or_default();

        Self {
            save_directory: None,
            save_files: HashMap::new(),
            selected_save: None,
            steam_id,
            change_count: 0,
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum SaveType {
    Profile,
    Character,
}

// Character save tabs
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum CharacterTab {
    Character,
    Inventory,
    Currency,
    Missions,
    Locations,
}

// Mission sub-tabs
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MissionTab {
    Missions,
    Challenges,
}

// Profile save tabs
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ProfileTab {
    Cosmetics,
    Bank,
    Settings,
}

#[derive(PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    FileManager,
    CharacterTab(CharacterTab),
    ProfileTab(ProfileTab),
    ItemDecoder,
}

pub struct BL4App {
    pub current_tab: Tab,
    pub current_mission_tab: MissionTab,
    pub state: AppState,
    pub error_message: Option<String>,
    pub status_message: Option<String>,

    // File drop handling
    pub dropped_files: Vec<egui::DroppedFile>,

    // Backup manager visibility
    pub show_backup_manager: bool,

    // Backup dialogs
    pub show_manual_backup_dialog: bool,
    pub manual_backup_save_selection: Option<String>,
    pub manual_backup_tag: String,
    pub manual_backup_description: String,

    // Edit backup version dialog
    pub show_edit_version_dialog: bool,
    pub edit_version_id: String,
    pub edit_version_save_path: PathBuf,
    pub edit_version_tag: String,
    pub edit_version_description: String,
}

impl BL4App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            current_tab: Tab::default(),
            current_mission_tab: MissionTab::Missions,
            state: AppState::default(),
            error_message: None,
            status_message: None,
            dropped_files: Vec::new(),
            show_backup_manager: false,
            show_manual_backup_dialog: false,
            manual_backup_save_selection: None,
            manual_backup_tag: String::new(),
            manual_backup_description: String::new(),
            show_edit_version_dialog: false,
            edit_version_id: String::new(),
            edit_version_save_path: PathBuf::new(),
            edit_version_tag: String::new(),
            edit_version_description: String::new(),
        };

        // Try to load CWD first if it's a save directory
        let cwd_is_save_dir = std::env::current_dir().ok().and_then(|cwd| {
            // Check if CWD contains .sav files
            std::fs::read_dir(&cwd).ok().and_then(|entries| {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("sav") {
                        return Some(cwd.clone());
                    }
                }
                None
            })
        });

        if let Some(cwd) = cwd_is_save_dir {
            if let Err(e) = app.load_save_directory(cwd.clone()) {
                app.set_error(format!(
                    "Found save files in current directory but failed to load: {}",
                    e
                ));
            } else {
                // Auto-select profile.sav if it exists
                if app.state.save_files.contains_key("profile.sav") {
                    app.state.selected_save = Some("profile.sav".to_string());
                    // Try to load and decrypt it
                    match app.load_current_save() {
                        Ok(_) => app.set_status("Loaded profile.sav".to_string()),
                        Err(e) => app.set_error(format!("Could not decrypt profile.sav: {}", e)),
                    }
                } else {
                    app.set_status(format!("Loaded current directory: {}", cwd.display()));
                }
            }
        } else {
            // Otherwise, try to auto-detect standard save directory
            if let Some(save_dir) = bl4::detect_save_directory() {
                if let Err(e) = app.load_save_directory(save_dir.clone()) {
                    app.set_error(format!("Found save directory but failed to load: {}", e));
                } else {
                    // Auto-select profile.sav if it exists
                    if app.state.save_files.contains_key("profile.sav") {
                        app.state.selected_save = Some("profile.sav".to_string());
                        // Try to load and decrypt it
                        match app.load_current_save() {
                            Ok(_) => app.set_status("Loaded profile.sav".to_string()),
                            Err(e) => {
                                app.set_error(format!("Could not decrypt profile.sav: {}", e))
                            }
                        }
                    } else {
                        app.set_status("Auto-detected save directory".to_string());
                    }
                }
            }
        }

        app
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
        self.status_message = None;
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.error_message = None;
    }

    #[allow(dead_code)]
    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.status_message = None;
    }

    /// Get a reference to the changeset for the currently selected save file
    pub fn current_changeset(&self) -> Option<&bl4::ChangeSet> {
        self.state
            .selected_save
            .as_ref()
            .and_then(|name| self.state.save_files.get(name))
            .map(|entry| &entry.changeset)
    }

    /// Get a mutable reference to the changeset for the currently selected save file
    #[allow(dead_code)]
    pub fn current_changeset_mut(&mut self) -> Option<&mut bl4::ChangeSet> {
        self.state
            .selected_save
            .as_ref()
            .and_then(|name| self.state.save_files.get_mut(name))
            .map(|entry| &mut entry.changeset)
    }

    /// Execute a closure with mutable access to the current changeset
    pub fn with_changeset_mut<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut bl4::ChangeSet) -> R,
    {
        let name = self.state.selected_save.clone()?;
        self.state.save_files.get_mut(&name).map(|entry| {
            let result = f(&mut entry.changeset);
            self.state.change_count = entry.changeset.len();
            result
        })
    }

    /// Check if the current changeset has a change for the given path
    pub fn has_change(&self, path: &str) -> bool {
        self.current_changeset()
            .map(|cs| cs.has_change(path))
            .unwrap_or(false)
    }

    pub fn load_save_directory(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if !path.is_dir() {
            anyhow::bail!("Path is not a directory");
        }

        // Try to extract Steam ID from the path if not already set
        if self.state.steam_id.is_empty() {
            if let Some(steam_id) = bl4::extract_steam_id_from_path(&path) {
                self.state.steam_id = steam_id;
            }
        }

        self.state.save_directory = Some(path.clone());
        self.state.save_files.clear();
        self.discover_save_files()?;

        // Default to profile.sav if it exists
        if self.state.save_files.contains_key("profile.sav") {
            self.state.selected_save = Some("profile.sav".to_string());
        } else if let Some(first_key) = self.state.save_files.keys().next().cloned() {
            self.state.selected_save = Some(first_key);
        }

        Ok(())
    }

    pub fn discover_save_files(&mut self) -> anyhow::Result<()> {
        let Some(dir) = &self.state.save_directory else {
            return Ok(());
        };

        // Look for profile.sav
        let profile_path = dir.join("profile.sav");
        if profile_path.exists() {
            self.state.save_files.insert(
                "profile.sav".to_string(),
                SaveFileEntry {
                    path: profile_path,
                    name: "profile.sav".to_string(),
                    save_data: None,
                    is_modified: false,
                    changeset: bl4::ChangeSet::new(),
                },
            );
        }

        // Look for numbered character saves (1.sav, 2.sav, etc.)
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename.ends_with(".sav") && filename != "profile.sav" {
                    // Check if it's a numbered save
                    if let Some(num_str) = filename.strip_suffix(".sav") {
                        if num_str.parse::<u32>().is_ok() {
                            self.state.save_files.insert(
                                filename.to_string(),
                                SaveFileEntry {
                                    path: path.clone(),
                                    name: filename.to_string(),
                                    save_data: None,
                                    is_modified: false,
                                    changeset: bl4::ChangeSet::new(),
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn load_current_save(&mut self) -> anyhow::Result<()> {
        let Some(selected) = &self.state.selected_save else {
            anyhow::bail!("No save file selected");
        };

        if self.state.steam_id.is_empty() {
            anyhow::bail!("Steam ID not configured. Please set your Steam ID first.");
        }

        let entry = self
            .state
            .save_files
            .get_mut(selected)
            .ok_or_else(|| anyhow::anyhow!("Selected save file not found"))?;

        // Auto-backup: Check if we should create a backup before loading
        let should_backup =
            bl4::should_create_versioned_backup(&entry.path, Some(&self.state.steam_id))?;

        if should_backup {
            // Create auto-backup with timestamp tag
            match bl4::create_versioned_backup(
                &entry.path,
                Some(&self.state.steam_id),
                Some("Auto-backup before load".to_string()),
                None, // No description for auto-backups
                true, // This is an auto-created backup
            ) {
                Ok(_) => {
                    // Silently created backup
                }
                Err(e) => {
                    // Log but don't fail the load
                    eprintln!("Warning: Failed to create auto-backup: {}", e);
                }
            }
        }

        let encrypted = std::fs::read(&entry.path)?;
        let yaml_data = bl4::decrypt_sav(&encrypted, &self.state.steam_id)?;
        let save = bl4::SaveFile::from_yaml(&yaml_data)?;

        entry.save_data = Some(save);
        entry.is_modified = false;

        Ok(())
    }

    pub fn save_current_file(&mut self) -> anyhow::Result<()> {
        let Some(selected) = &self.state.selected_save else {
            anyhow::bail!("No save file selected");
        };

        if self.state.steam_id.is_empty() {
            anyhow::bail!("Steam ID not configured");
        }

        let entry = self
            .state
            .save_files
            .get_mut(selected)
            .ok_or_else(|| anyhow::anyhow!("Selected save file not found"))?;

        let Some(save_data) = &mut entry.save_data else {
            anyhow::bail!("No save data loaded");
        };

        // Apply pending changes from changeset
        entry.changeset.apply(save_data)?;

        // Create versioned backup before saving (if hash differs)
        let should_backup =
            bl4::should_create_versioned_backup(&entry.path, Some(&self.state.steam_id))?;

        if should_backup {
            match bl4::create_versioned_backup(
                &entry.path,
                Some(&self.state.steam_id),
                Some("Auto-backup before save".to_string()),
                None,
                true, // Auto-created
            ) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Warning: Failed to create auto-backup: {}", e);
                }
            }
        }

        // Serialize and encrypt
        let yaml = save_data.to_yaml()?;
        let encrypted = bl4::encrypt_sav(&yaml, &self.state.steam_id)?;

        // Write to file
        std::fs::write(&entry.path, encrypted)?;

        // Update metadata
        entry.is_modified = false;
        self.state.change_count = 0;
        entry.changeset = bl4::ChangeSet::new();

        Ok(())
    }

    #[allow(dead_code)]
    pub fn mark_modified(&mut self) {
        if let Some(selected) = &self.state.selected_save {
            if let Some(entry) = self.state.save_files.get_mut(selected) {
                if !entry.is_modified {
                    entry.is_modified = true;
                    self.state.change_count += 1;
                }
            }
        }
    }

    pub fn current_save_type(&self) -> Option<SaveType> {
        self.state.selected_save.as_ref().and_then(|name| {
            self.state
                .save_files
                .get(name)
                .map(|entry| entry.save_type())
        })
    }
}

impl eframe::App for BL4App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle dropped files
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                self.dropped_files = i.raw.dropped_files.clone();
            }
        });

        // Process dropped files
        if !self.dropped_files.is_empty() {
            let dropped = std::mem::take(&mut self.dropped_files);
            for file in &dropped {
                if let Some(path) = &file.path {
                    // If it's a .sav file, load the parent directory (which contains all saves)
                    let dir_to_load = if path.is_file()
                        && path.extension().and_then(|e| e.to_str()) == Some("sav")
                    {
                        path.parent().map(|p| p.to_path_buf())
                    } else if path.is_dir() {
                        Some(path.clone())
                    } else {
                        None
                    };

                    if let Some(dir) = dir_to_load {
                        let selected_file = if path.is_file() {
                            path.file_name().and_then(|f| f.to_str()).map(String::from)
                        } else {
                            None
                        };

                        match self.load_save_directory(dir.clone()) {
                            Ok(_) => {
                                // If a specific file was dropped, select it
                                if let Some(filename) = selected_file {
                                    self.state.selected_save = Some(filename);
                                }
                                self.set_status(format!(
                                    "Loaded save directory: {}",
                                    dir.display()
                                ));
                            }
                            Err(e) => {
                                let error_msg = if self.state.steam_id.is_empty() {
                                    "Cannot load saves: Steam ID not configured. Please set your Steam ID in the Files tab.".to_string()
                                } else if !dir.exists() {
                                    format!("Directory does not exist: {}", dir.display())
                                } else if !dir.is_dir() {
                                    format!("Path is not a directory: {}", dir.display())
                                } else {
                                    format!("Cannot read directory {}: {}", dir.display(), e)
                                };
                                self.set_error(error_msg);
                            }
                        }
                    }
                }
            }
        }

        // Menu bar with tabs and save selector - dynamic based on save type
        egui::TopBottomPanel::top("menu_bar")
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(35, 37, 41))
                .inner_margin(egui::Margin::symmetric(20.0, 12.0)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left side: Tab menu
                ui.selectable_value(&mut self.current_tab, Tab::FileManager, "Files");

                // Show different tabs based on what type of save is loaded
                match self.current_save_type() {
                    Some(SaveType::Character) => {
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::CharacterTab(CharacterTab::Character)),
                            "Character"
                        ).clicked() {
                            self.current_tab = Tab::CharacterTab(CharacterTab::Character);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::CharacterTab(CharacterTab::Inventory)),
                            "Inventory"
                        ).clicked() {
                            self.current_tab = Tab::CharacterTab(CharacterTab::Inventory);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::CharacterTab(CharacterTab::Currency)),
                            "Currency"
                        ).clicked() {
                            self.current_tab = Tab::CharacterTab(CharacterTab::Currency);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::CharacterTab(CharacterTab::Missions)),
                            "Missions"
                        ).clicked() {
                            self.current_tab = Tab::CharacterTab(CharacterTab::Missions);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::CharacterTab(CharacterTab::Locations)),
                            "Locations"
                        ).clicked() {
                            self.current_tab = Tab::CharacterTab(CharacterTab::Locations);
                        }
                    }
                    Some(SaveType::Profile) => {
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::ProfileTab(ProfileTab::Cosmetics)),
                            "Cosmetics"
                        ).clicked() {
                            self.current_tab = Tab::ProfileTab(ProfileTab::Cosmetics);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::ProfileTab(ProfileTab::Bank)),
                            "Bank"
                        ).clicked() {
                            self.current_tab = Tab::ProfileTab(ProfileTab::Bank);
                        }
                        if ui.selectable_label(
                            matches!(self.current_tab, Tab::ProfileTab(ProfileTab::Settings)),
                            "Settings"
                        ).clicked() {
                            self.current_tab = Tab::ProfileTab(ProfileTab::Settings);
                        }
                    }
                    None => {
                        // No save loaded - show generic tabs
                    }
                }

                ui.selectable_value(&mut self.current_tab, Tab::ItemDecoder, "Item Decoder");

                // Right side: Save file selector
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let display_text = if self.show_backup_manager {
                        "💾 Backups"
                    } else {
                        self.state.selected_save.as_deref().unwrap_or("No save loaded")
                    };

                    egui::ComboBox::from_id_salt("save_select")
                        .selected_text(display_text)
                        .height(600.0)
                        .show_ui(ui, |ui| {
                            // Backups option at the top
                            if ui.selectable_label(self.show_backup_manager, "💾 Backups").clicked() {
                                self.show_backup_manager = true;
                            }
                            ui.add_space(5.0);

                            // Regular save files
                            let mut keys: Vec<_> = self.state.save_files.keys().cloned().collect();
                            keys.sort();

                            for key in keys {
                                let is_modified = self.state.save_files.get(&key)
                                    .map(|e| e.is_modified)
                                    .unwrap_or(false);

                                let label = if is_modified {
                                    format!("{} *", key)
                                } else {
                                    key.clone()
                                };

                                if ui.selectable_label(!self.show_backup_manager && self.state.selected_save.as_deref() == Some(&key), label).clicked() {
                                    self.show_backup_manager = false;
                                    self.state.selected_save = Some(key.clone());

                                    // Switch to the first tab for this file type
                                    if let Some(entry) = self.state.save_files.get(&key) {
                                        match entry.save_type() {
                                            SaveType::Profile => {
                                                self.current_tab = Tab::ProfileTab(ProfileTab::Cosmetics);
                                            }
                                            SaveType::Character => {
                                                self.current_tab = Tab::CharacterTab(CharacterTab::Character);
                                            }
                                        }
                                    }

                                    // Immediately load and decrypt the selected save file
                                    match self.load_current_save() {
                                        Ok(_) => self.set_status(format!("Loaded {}", key)),
                                        Err(e) => {
                                            let error_msg = if self.state.steam_id.is_empty() {
                                                format!("Cannot decrypt {}: Steam ID not configured", key)
                                            } else {
                                                format!("Failed to decrypt {}: {}", key, e)
                                            };
                                            self.set_error(error_msg);
                                        }
                                    }
                                }
                            }
                        });
                });
            });
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("bottom_panel")
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(35, 37, 41))
                    .inner_margin(egui::Margin::symmetric(20.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Small);

                    // Status message on the left
                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                    } else if let Some(status) = &self.status_message {
                        ui.colored_label(egui::Color32::from_rgb(100, 200, 100), status);
                    } else {
                        ui.label("Ready");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Save button (only show if there are changes)
                        if self.state.change_count > 0 {
                            if ui.button("Save").clicked() {
                                match self.save_current_file() {
                                    Ok(_) => self.set_status("Saved successfully".to_string()),
                                    Err(e) => self.set_error(format!("Save failed: {}", e)),
                                }
                            }
                            ui.label(format!("{} changes", self.state.change_count));
                        }
                    });
                });
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Show backup manager if disk icon is active
            if self.show_backup_manager {
                crate::ui::backup_manager::show(ui, self);
            } else {
                // Otherwise show the selected tab
                match &self.current_tab {
                    Tab::FileManager => crate::ui::file_manager::show(ui, self),
                    Tab::CharacterTab(char_tab) => match char_tab {
                        CharacterTab::Character => crate::ui::character::show(ui, self),
                        CharacterTab::Inventory => crate::ui::inventory::show(ui, self),
                        CharacterTab::Currency => crate::ui::currency::show(ui, self),
                        CharacterTab::Missions => crate::ui::missions::show(ui, self),
                        CharacterTab::Locations => crate::ui::locations::show(ui, self),
                    },
                    Tab::ProfileTab(profile_tab) => match profile_tab {
                        ProfileTab::Cosmetics => crate::ui::cosmetics::show(ui, self),
                        ProfileTab::Bank => crate::ui::bank::show(ui, self),
                        ProfileTab::Settings => crate::ui::settings::show(ui, self),
                    },
                    Tab::ItemDecoder => crate::ui::item_decoder::show(ui, self),
                }
            }
        });
    }
}

// Temporary module for config access
mod bl4_cli_config {
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Config {
        steam_id: Option<String>,
    }

    impl Config {
        pub fn load() -> anyhow::Result<Self> {
            let path = Self::config_path()?;
            if !path.exists() {
                return Ok(Self::default());
            }
            let contents = std::fs::read_to_string(path)?;
            Ok(serde_yaml::from_str(&contents)?)
        }

        pub fn config_path() -> anyhow::Result<PathBuf> {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map_err(|_| anyhow::anyhow!("Could not determine home directory"))?;
            Ok(PathBuf::from(home)
                .join(".config")
                .join("bl4")
                .join("config.yaml"))
        }

        pub fn get_steam_id(&self) -> Option<&str> {
            self.steam_id.as_deref()
        }
    }
}
