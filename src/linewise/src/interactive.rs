use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame, Terminal,
};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    U8,
    U16Le,
    U16Be,
    U32Le,
    U32Be,
    VarInt,
    Hex,
    Binary,
    Ascii,
}

impl DataType {
    fn all() -> &'static [DataType] {
        &[
            DataType::U8,
            DataType::U16Le,
            DataType::U16Be,
            DataType::U32Le,
            DataType::U32Be,
            DataType::VarInt,
            DataType::Hex,
            DataType::Binary,
            DataType::Ascii,
        ]
    }

    fn name(&self) -> &'static str {
        match self {
            DataType::U8 => "u8",
            DataType::U16Le => "u16le",
            DataType::U16Be => "u16be",
            DataType::U32Le => "u32le",
            DataType::U32Be => "u32be",
            DataType::VarInt => "varint",
            DataType::Hex => "hex",
            DataType::Binary => "binary",
            DataType::Ascii => "ascii",
        }
    }

    fn byte_size(&self) -> Option<usize> {
        match self {
            DataType::U8 | DataType::Hex | DataType::Binary | DataType::Ascii => Some(1),
            DataType::U16Le | DataType::U16Be => Some(2),
            DataType::U32Le | DataType::U32Be => Some(4),
            DataType::VarInt => None, // Variable
        }
    }

    fn next(&self) -> DataType {
        let all = Self::all();
        let idx = all.iter().position(|t| t == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    fn prev(&self) -> DataType {
        let all = Self::all();
        let idx = all.iter().position(|t| t == self).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }

    fn from_name(name: &str) -> Option<DataType> {
        match name {
            "u8" => Some(DataType::U8),
            "u16le" => Some(DataType::U16Le),
            "u16be" => Some(DataType::U16Be),
            "u32le" => Some(DataType::U32Le),
            "u32be" => Some(DataType::U32Be),
            "varint" => Some(DataType::VarInt),
            "hex" => Some(DataType::Hex),
            "binary" => Some(DataType::Binary),
            "ascii" => Some(DataType::Ascii),
            _ => None,
        }
    }

    fn decode(&self, data: &[u8]) -> String {
        match self {
            DataType::U8 => data.first().map(|&v| format!("{}", v)).unwrap_or_default(),
            DataType::Hex => data
                .first()
                .map(|&v| format!("{:02x}", v))
                .unwrap_or_default(),
            DataType::Binary => data
                .first()
                .map(|&v| format!("{:08b}", v))
                .unwrap_or_default(),
            DataType::Ascii => data
                .first()
                .map(|&v| {
                    if v.is_ascii_graphic() || v == b' ' {
                        (v as char).to_string()
                    } else {
                        format!("\\x{:02x}", v)
                    }
                })
                .unwrap_or_default(),
            DataType::U16Le if data.len() >= 2 => {
                format!("{}", u16::from_le_bytes([data[0], data[1]]))
            }
            DataType::U16Be if data.len() >= 2 => {
                format!("{}", u16::from_be_bytes([data[0], data[1]]))
            }
            DataType::U32Le if data.len() >= 4 => {
                format!(
                    "{}",
                    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
                )
            }
            DataType::U32Be if data.len() >= 4 => {
                format!(
                    "{}",
                    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
                )
            }
            DataType::VarInt => Self::decode_varint(data),
            _ => String::new(),
        }
    }

    fn decode_varint(data: &[u8]) -> String {
        let mut value: u64 = 0;
        let mut shift = 0;
        for &byte in data {
            if shift >= 64 {
                break;
            }
            value |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return format!("{}", value);
            }
            shift += 7;
        }
        String::new()
    }

    fn display_width(&self) -> usize {
        match self {
            DataType::U8 => 4,                       // "255 "
            DataType::Hex => 3,                      // "ff "
            DataType::Binary => 9,                   // "00000000 "
            DataType::U16Le | DataType::U16Be => 6,  // "65535 "
            DataType::U32Le | DataType::U32Be => 11, // "4294967295 "
            DataType::VarInt => 11,
            DataType::Ascii => 2, // "X "
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockedField {
    pub byte_offset: usize,
    pub byte_length: usize,
    pub data_type: DataType,
}

/// Toggle targets for yo*, [*, ]* prefix commands
enum ToggleTarget {
    Frequency,
    Wrap,
    ShowLocks,
    ShowGutter,
}

pub struct InteractiveState {
    records: Vec<Vec<u8>>,
    current_record: usize,
    // Byte offset where field interpretation starts (h/l shifts this)
    field_offset: usize,
    // Which field the cursor is on (w/b moves this)
    current_field: usize,
    current_type: DataType,
    locked_fields: Vec<LockedField>,
    scroll_offset: usize,
    visible_records: usize,
    message: Option<String>,
    // Command mode
    command_mode: bool,
    command_buffer: String,
    // Current preset
    current_preset: Option<String>,
    // Count prefix for vim-like navigation (e.g., 10j)
    count_buffer: String,
    // Frequency analysis mode
    frequency_mode: bool,
    byte_frequencies: Vec<[u32; 256]>,
    // Pending 'g' for two-char commands (gg, etc.)
    pending_g: bool,
    // Pending 'y' for yank/toggle commands
    pending_y: bool,
    // Pending 'yo' for toggle commands (yof = toggle frequency)
    pending_yo: bool,
    // Pending '[' for enable commands ([f = freq on)
    pending_open_bracket: bool,
    // Pending ']' for disable commands (]f = freq off)
    pending_close_bracket: bool,
    // Wrap mode: true = wrap lines, false = horizontal scroll
    wrap_mode: bool,
    // Horizontal scroll offset (in fields) when not wrapping
    horizontal_scroll: usize,
    // Terminal width for scroll calculations
    terminal_width: usize,
    // Show locked fields (toggle with yol, [l, ]l)
    show_locks: bool,
    // Show gutter/padding (toggle with yog, [g, ]g)
    show_gutter: bool,
}

impl InteractiveState {
    pub fn new(records: Vec<Vec<u8>>) -> Self {
        Self {
            records,
            current_record: 0,
            field_offset: 0,
            current_field: 0,
            current_type: DataType::U8,
            locked_fields: Vec::new(),
            scroll_offset: 0,
            visible_records: 10,
            message: None,
            command_mode: false,
            command_buffer: String::new(),
            current_preset: None,
            count_buffer: String::new(),
            frequency_mode: false,
            byte_frequencies: Vec::new(),
            pending_g: false,
            pending_y: false,
            pending_yo: false,
            pending_open_bracket: false,
            pending_close_bracket: false,
            wrap_mode: false,
            horizontal_scroll: 0,
            terminal_width: 80,
            show_locks: true,
            show_gutter: true,
        }
    }

    /// Get the byte position of the current field
    fn current_field_byte(&self) -> usize {
        let type_size = self.current_type.byte_size().unwrap_or(1);
        self.field_offset + (self.current_field * type_size)
    }

    /// Get the number of fields that fit in a record
    fn field_count(&self, record_len: usize) -> usize {
        let type_size = self.current_type.byte_size().unwrap_or(1);
        if record_len <= self.field_offset {
            0
        } else {
            (record_len - self.field_offset) / type_size
        }
    }

    fn clear_pending(&mut self) {
        self.pending_g = false;
        self.pending_y = false;
        self.pending_yo = false;
        self.pending_open_bracket = false;
        self.pending_close_bracket = false;
    }

    /// Handle toggle prefix commands (yo*, [*, ]*)
    /// Returns true if a toggle prefix was matched (and handled), false otherwise
    fn handle_toggle(&mut self, target: ToggleTarget) -> bool {
        let (field_ref, on_msg, off_msg) = match target {
            ToggleTarget::Frequency => (
                &mut self.frequency_mode,
                "Frequency mode ON",
                "Frequency mode OFF",
            ),
            ToggleTarget::Wrap => (&mut self.wrap_mode, "Wrap ON", "Wrap OFF"),
            ToggleTarget::ShowLocks => (&mut self.show_locks, "Locks ON", "Locks OFF"),
            ToggleTarget::ShowGutter => (&mut self.show_gutter, "Gutter ON", "Gutter OFF"),
        };

        if self.pending_yo {
            *field_ref = !*field_ref;
            self.message = Some((if *field_ref { on_msg } else { off_msg }).to_string());
        } else if self.pending_open_bracket {
            *field_ref = true;
            self.message = Some(on_msg.to_string());
        } else if self.pending_close_bracket {
            *field_ref = false;
            self.message = Some(off_msg.to_string());
        } else {
            return false;
        }

        self.clear_pending();
        self.count_buffer.clear();
        true
    }

    fn compute_frequencies(&mut self) {
        let max_len = self.records.iter().map(|r| r.len()).max().unwrap_or(0);
        self.byte_frequencies = vec![[0u32; 256]; max_len];

        for record in &self.records {
            for (pos, &byte) in record.iter().enumerate() {
                self.byte_frequencies[pos][byte as usize] += 1;
            }
        }
    }

    fn get_frequency_color(&self, pos: usize, byte: u8) -> Color {
        if pos >= self.byte_frequencies.len() {
            return Color::DarkGray;
        }

        let freq = self.byte_frequencies[pos][byte as usize];
        let total = self.records.len() as u32;

        if total == 0 {
            return Color::DarkGray;
        }

        // Calculate percentage (0-100)
        let pct = (freq * 100) / total;

        // High frequency (>80%) = red (constant/magic bytes)
        // Medium-high (60-80%) = yellow
        // Medium (40-60%) = green
        // Low-medium (20-40%) = cyan
        // Low (<20%) = blue (high entropy)
        match pct {
            80..=100 => Color::Red,
            60..=79 => Color::Yellow,
            40..=59 => Color::Green,
            20..=39 => Color::Cyan,
            _ => Color::Blue,
        }
    }

    fn get_count(&mut self) -> usize {
        let count = self.count_buffer.parse::<usize>().unwrap_or(1);
        self.count_buffer.clear();
        count.max(1)
    }

    fn save_preset(&self, name: &str) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let preset_dir = format!("{}/.config/linewise/presets", home);
        fs::create_dir_all(&preset_dir)
            .map_err(|e| format!("Failed to create preset dir: {}", e))?;

        let path = format!("{}/{}.lwpreset", preset_dir, name);

        let mut content = String::new();
        content.push_str("# Locked fields: offset length type\n");
        for field in &self.locked_fields {
            content.push_str(&format!(
                "{} {} {}\n",
                field.byte_offset,
                field.byte_length,
                field.data_type.name()
            ));
        }

        // Include rules section placeholder for manual editing
        content.push_str("\n# Detection rules (uncomment and edit to enable auto-detection)\n");
        content.push_str("# @rules\n");
        content.push_str("# byte_equals 0 33\n");
        content.push_str("# min_length 30\n");

        fs::write(&path, content).map_err(|e| format!("Failed to save: {}", e))
    }

    fn save_config(&self) -> Result<String, String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let config_dir = format!("{}/.config/linewise", home);
        let config_path = format!("{}/settings.json", config_dir);

        // Create config directory if it doesn't exist
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;

        // Build settings JSON
        let content = format!(
            r#"{{
  "wrap_mode": {},
  "frequency_mode": {}
}}
"#,
            self.wrap_mode, self.frequency_mode
        );

        fs::write(&config_path, content).map_err(|e| format!("Failed to save: {}", e))?;

        Ok(config_path)
    }

    /// Resolve preset path:
    /// - Absolute path: use as-is
    /// - Relative path: resolve relative to config_dir (or cwd if None)
    /// - Plain name: use ~/.config/linewise/presets/{name}.lwpreset
    pub fn resolve_preset_path(name: &str, config_dir: Option<&Path>) -> String {
        let path = Path::new(name);

        // Absolute path - use as-is
        if path.is_absolute() {
            return name.to_string();
        }

        // Has path separators - treat as relative
        if name.contains('/') || name.contains('\\') {
            if let Some(base) = config_dir {
                return base.join(path).to_string_lossy().to_string();
            }
            return name.to_string();
        }

        // Plain name - check multiple locations
        let home = std::env::var("HOME").unwrap_or_default();

        // Try ~/.config/linewise/presets/{name}.lwpreset first
        let config_preset = format!("{}/.config/linewise/presets/{}.lwpreset", home, name);
        if Path::new(&config_preset).exists() {
            return config_preset;
        }

        // Try ~/.local/etc/linewise/presets/{name}.lwpreset
        let local_preset = format!("{}/.local/etc/linewise/presets/{}.lwpreset", home, name);
        if Path::new(&local_preset).exists() {
            return local_preset;
        }

        // Try current directory
        let cwd_preset = format!("{}.lwpreset", name);
        if Path::new(&cwd_preset).exists() {
            return cwd_preset;
        }

        // Default to config location (will error on read if not found)
        config_preset
    }

    fn load_preset(&mut self, name: &str) -> Result<(), String> {
        let path = Self::resolve_preset_path(name, None);

        let content =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read '{}': {}", path, e))?;

        let mut new_fields = Vec::new();
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let byte_offset: usize = parts[0].parse().map_err(|_| "Invalid offset")?;
                let byte_length: usize = parts[1].parse().map_err(|_| "Invalid length")?;
                let data_type = DataType::from_name(parts[2]).ok_or("Invalid type")?;
                new_fields.push(LockedField {
                    byte_offset,
                    byte_length,
                    data_type,
                });
            }
        }

        self.locked_fields = new_fields;
        self.locked_fields.sort_by_key(|f| f.byte_offset);
        Ok(())
    }

    fn cmd_write(&mut self, arg: Option<&str>, force: bool) {
        let name = arg
            .map(String::from)
            .or_else(|| self.current_preset.clone());
        let Some(name) = name else {
            let cmd = if force { ":w!" } else { ":w" };
            self.message = Some(format!("No preset loaded. Usage: {} <preset_name>", cmd));
            return;
        };

        if !force && arg.is_some() {
            let path = Self::resolve_preset_path(&name, None);
            if Path::new(&path).exists() {
                self.message = Some(format!("'{}' exists. Use :w! {} to overwrite", name, name));
                return;
            }
        }

        self.message = Some(match self.save_preset(&name) {
            Ok(()) => format!("Saved to '{}'", name),
            Err(e) => e,
        });
    }

    fn cmd_preset(&mut self, arg: Option<&str>) {
        let name = arg
            .map(String::from)
            .or_else(|| self.current_preset.clone());
        let Some(name) = name else {
            self.message = Some("No preset loaded. Usage: :p <preset_name>".to_string());
            return;
        };

        match self.load_preset(&name) {
            Ok(()) => {
                let count = self.locked_fields.len();
                self.current_preset = Some(name.clone());
                self.message = Some(format!("Loaded preset '{}' ({} fields)", name, count));
            }
            Err(e) => self.message = Some(e),
        }
    }

    fn cmd_open(&mut self, arg: Option<&str>) {
        let Some(path) = arg else {
            self.message = Some("Usage: :e <filename>".to_string());
            return;
        };

        self.message = Some(match self.open_file(path) {
            Ok(count) => format!("Opened '{}' ({} records)", path, count),
            Err(e) => e,
        });
    }

    fn execute_command(&mut self) -> bool {
        let cmd = self.command_buffer.trim().to_string();
        self.command_buffer.clear();
        self.command_mode = false;

        if cmd.is_empty() {
            return false;
        }

        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let arg = parts.get(1).copied();

        match parts[0] {
            "w" | "write" => self.cmd_write(arg, false),
            "w!" | "write!" => self.cmd_write(arg, true),
            "p" | "preset" => self.cmd_preset(arg),
            "e" | "o" | "open" | "edit" => self.cmd_open(arg),
            "clear" => {
                self.locked_fields.clear();
                self.message = Some("Cleared all locked fields".to_string());
            }
            "s" | "save" => {
                self.message = Some(match self.save_config() {
                    Ok(path) => format!("Saved config to {}", path),
                    Err(e) => e,
                });
            }
            "q" | "quit" => return true,
            _ => self.message = Some(format!("Unknown command: {}", parts[0])),
        }
        false
    }

    fn open_file(&mut self, path: &str) -> Result<usize, String> {
        let file = fs::File::open(path).map_err(|e| format!("Failed to open '{}': {}", path, e))?;

        let mut reader = std::io::BufReader::new(file);
        let mut records = Vec::new();

        // Read length16 format
        loop {
            let mut len_buf = [0u8; 2];
            match std::io::Read::read_exact(&mut reader, &mut len_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(format!("Read error: {}", e)),
            }

            let len = u16::from_le_bytes(len_buf) as usize;
            if len == 0 {
                records.push(Vec::new());
                continue;
            }

            let mut data = vec![0u8; len];
            std::io::Read::read_exact(&mut reader, &mut data)
                .map_err(|e| format!("Read error: {}", e))?;
            records.push(data);
        }

        let count = records.len();
        self.records = records;
        self.current_record = 0;
        self.scroll_offset = 0;
        self.field_offset = 0;
        self.current_field = 0;
        // Keep locked fields - user may want to apply same preset to new file
        Ok(count)
    }

    /// Max number of fields in the current record
    fn max_fields(&self) -> usize {
        let record_len = self
            .records
            .get(self.current_record)
            .map(|r| r.len())
            .unwrap_or(0);
        self.field_count(record_len)
    }

    /// Lock the current field position as the current type
    /// count specifies how many consecutive fields to lock as one region
    fn lock_current(&mut self, count: usize) {
        let byte_off = self.current_field_byte();
        let type_size = self.current_type.byte_size().unwrap_or(1);
        let byte_len = type_size * count;

        // Check for overlap with existing locked fields
        let overlaps = self.locked_fields.iter().any(|f| {
            let f_end = f.byte_offset + f.byte_length;
            let new_end = byte_off + byte_len;
            !(new_end <= f.byte_offset || byte_off >= f_end)
        });

        if overlaps {
            self.message = Some("Cannot lock: overlaps with existing field".to_string());
            return;
        }

        // Check if we have enough bytes in the record
        let record_len = self
            .records
            .get(self.current_record)
            .map(|r| r.len())
            .unwrap_or(0);
        if byte_off + byte_len > record_len {
            self.message = Some(format!(
                "Cannot lock: {} bytes needed, only {} available",
                byte_len,
                record_len.saturating_sub(byte_off)
            ));
            return;
        }

        self.locked_fields.push(LockedField {
            byte_offset: byte_off,
            byte_length: byte_len,
            data_type: self.current_type,
        });
        self.locked_fields.sort_by_key(|f| f.byte_offset);

        if count > 1 {
            self.message = Some(format!(
                "Locked {}x{} ({} bytes) at byte {}",
                count,
                self.current_type.name(),
                byte_len,
                byte_off
            ));
        } else {
            self.message = Some(format!(
                "Locked {} at byte {}",
                self.current_type.name(),
                byte_off
            ));
        }
    }

    /// Unlock the field at the cursor position
    fn unlock_at_cursor(&mut self) {
        let byte_off = self.current_field_byte();
        let before_len = self.locked_fields.len();
        self.locked_fields.retain(|f| {
            let f_end = f.byte_offset + f.byte_length;
            !(byte_off >= f.byte_offset && byte_off < f_end)
        });
        if self.locked_fields.len() < before_len {
            self.message = Some("Unlocked field".to_string());
        }
    }

    /// Move to the next field (w key)
    fn move_to_next_field(&mut self) {
        let max = self.max_fields();
        if self.current_field + 1 < max {
            self.current_field += 1;
        }
    }

    /// Move to the previous field (b key)
    fn move_to_prev_field(&mut self) {
        if self.current_field > 0 {
            self.current_field -= 1;
        }
    }

    /// Shift field alignment forward by 1 byte (l key)
    fn shift_offset_forward(&mut self) {
        let max_offset = self
            .records
            .get(self.current_record)
            .map(|r| r.len())
            .unwrap_or(0);
        let type_size = self.current_type.byte_size().unwrap_or(1);
        if self.field_offset + 1 < type_size.min(max_offset) {
            self.field_offset += 1;
        }
    }

    /// Shift field alignment backward by 1 byte (h key)
    fn shift_offset_backward(&mut self) {
        if self.field_offset > 0 {
            self.field_offset -= 1;
        }
    }

    /// Move down by count records (j key)
    fn move_down(&mut self, count: usize) {
        let max_idx = self.records.len().saturating_sub(1);
        self.current_record = (self.current_record + count).min(max_idx);
        if self.current_record >= self.scroll_offset + self.visible_records {
            self.scroll_offset = self.current_record.saturating_sub(self.visible_records - 1);
        }
    }

    /// Move up by count records (k key)
    fn move_up(&mut self, count: usize) {
        self.current_record = self.current_record.saturating_sub(count);
        if self.current_record < self.scroll_offset {
            self.scroll_offset = self.current_record;
        }
    }

    /// Page down (Ctrl+d)
    fn page_down(&mut self) {
        let jump = self.visible_records / 2;
        self.current_record =
            (self.current_record + jump).min(self.records.len().saturating_sub(1));
        if self.current_record >= self.scroll_offset + self.visible_records {
            self.scroll_offset = self.current_record.saturating_sub(self.visible_records - 1);
        }
    }

    /// Page up (Ctrl+u)
    fn page_up(&mut self) {
        let jump = self.visible_records / 2;
        self.current_record = self.current_record.saturating_sub(jump);
        if self.current_record < self.scroll_offset {
            self.scroll_offset = self.current_record;
        }
    }

    /// Jump to first record (gg)
    fn jump_to_start(&mut self) {
        self.current_record = 0;
        self.scroll_offset = 0;
        self.message = Some("Jumped to first record".to_string());
    }

    /// Jump to last record (G)
    fn jump_to_end(&mut self) {
        self.current_record = self.records.len().saturating_sub(1);
        if self.current_record >= self.visible_records {
            self.scroll_offset = self.current_record.saturating_sub(self.visible_records - 1);
        }
        self.message = Some("Jumped to last record".to_string());
    }

    /// Handle command mode input. Returns Some(true) to quit, Some(false) to continue, None if not in command mode.
    fn handle_command_input(&mut self, code: KeyCode) -> Option<bool> {
        if !self.command_mode {
            return None;
        }
        match code {
            KeyCode::Enter => Some(self.execute_command()),
            KeyCode::Esc => {
                self.command_mode = false;
                self.command_buffer.clear();
                self.message = None;
                Some(false)
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
                Some(false)
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                Some(false)
            }
            _ => Some(false),
        }
    }

    /// Handle a normal mode key event
    #[allow(clippy::too_many_lines)] // Keymap dispatch - each arm is a distinct key binding
    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match (code, modifiers) {
            (KeyCode::Char(':'), _) => {
                self.command_mode = true;
                self.command_buffer.clear();
                self.message = None;
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.current_type = self.current_type.next();
                self.message = Some(format!("Type: {}", self.current_type.name()));
            }
            (KeyCode::BackTab, _) => {
                self.current_type = self.current_type.prev();
                self.message = Some(format!("Type: {}", self.current_type.name()));
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.shift_offset_backward();
            }
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.move_to_prev_field();
            }
            (KeyCode::Char(c @ '1'..='9'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.count_buffer.push(c);
            }
            (KeyCode::Char('0'), KeyModifiers::NONE) => {
                if !self.count_buffer.is_empty() {
                    self.count_buffer.push('0');
                } else {
                    self.clear_pending();
                    self.current_field = 0;
                }
            }
            (KeyCode::Char('$'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.count_buffer.clear();
                self.current_field = self.max_fields().saturating_sub(1);
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                self.pending_g = false;
                let count = self.get_count();
                self.move_down(count);
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                self.pending_g = false;
                let count = self.get_count();
                self.move_up(count);
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) | (KeyCode::PageDown, _) => {
                self.pending_g = false;
                self.count_buffer.clear();
                self.page_down();
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) | (KeyCode::PageUp, _) => {
                self.pending_g = false;
                self.count_buffer.clear();
                self.page_up();
            }
            (KeyCode::Char('L'), _) => {
                self.pending_g = false;
                let count = self.get_count();
                self.lock_current(count);
            }
            (KeyCode::Char('U'), _) => {
                self.pending_g = false;
                self.count_buffer.clear();
                self.unlock_at_cursor();
            }
            (KeyCode::Char('G'), _) => {
                self.clear_pending();
                self.count_buffer.clear();
                self.jump_to_end();
            }
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.pending_y = true;
            }
            (KeyCode::Char('o'), KeyModifiers::NONE) => {
                if self.pending_y {
                    self.pending_y = false;
                    self.pending_yo = true;
                } else {
                    self.clear_pending();
                }
            }
            (KeyCode::Char('['), KeyModifiers::NONE) => {
                self.clear_pending();
                self.pending_open_bracket = true;
            }
            (KeyCode::Char(']'), KeyModifiers::NONE) => {
                self.clear_pending();
                self.pending_close_bracket = true;
            }
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                if self.handle_toggle(ToggleTarget::Frequency) {
                    if self.frequency_mode {
                        self.compute_frequencies();
                    }
                } else {
                    self.clear_pending();
                }
            }
            (KeyCode::Char('w'), KeyModifiers::NONE) => {
                if self.handle_toggle(ToggleTarget::Wrap) {
                    self.horizontal_scroll = 0;
                } else {
                    self.clear_pending();
                    self.move_to_next_field();
                }
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if !self.handle_toggle(ToggleTarget::ShowLocks) {
                    self.clear_pending();
                    self.shift_offset_forward();
                }
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                if !self.handle_toggle(ToggleTarget::ShowGutter) {
                    if self.pending_g {
                        self.clear_pending();
                        self.count_buffer.clear();
                        self.jump_to_start();
                    } else {
                        self.clear_pending();
                        self.pending_g = true;
                    }
                }
            }
            _ => {
                self.clear_pending();
                self.count_buffer.clear();
            }
        }
    }
}

pub fn run_interactive(records: Vec<Vec<u8>>, auto_preset: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = InteractiveState::new(records);

    // Auto-load detected preset
    if let Some(preset_name) = auto_preset {
        match state.load_preset(&preset_name) {
            Ok(()) => {
                state.current_preset = Some(preset_name.clone());
                state.message = Some(format!("Auto-loaded preset '{}'", preset_name));
            }
            Err(e) => {
                state.current_preset = Some(preset_name.clone());
                state.message = Some(format!(
                    "Auto-detect found '{}' but failed to load: {}",
                    preset_name, e
                ));
            }
        }
    }

    loop {
        terminal.draw(|f| draw_ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if let Some(should_quit) = state.handle_command_input(key.code) {
                if should_quit {
                    break;
                }
                continue;
            }
            state.handle_key(key.code, key.modifiers);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw_ui(f: &mut Frame, state: &mut InteractiveState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(3),    // Records view
            Constraint::Length(1), // Status bar (or command input)
        ])
        .split(f.area());

    draw_header(f, chunks[0], state);
    draw_records(f, chunks[1], state);
    draw_status_bar(f, chunks[2], state);
}

fn separator() -> Span<'static> {
    Span::styled("│", Style::default().fg(Color::DarkGray))
}

fn header_span(text: String, color: Color) -> Span<'static> {
    Span::styled(text, Style::default().fg(color))
}

fn draw_header(f: &mut Frame, area: Rect, state: &InteractiveState) {
    let mut spans: Vec<Span> = vec![
        header_span(
            format!(" {}/{} ", state.current_record + 1, state.records.len()),
            Color::Cyan,
        ),
        separator(),
        Span::styled(
            format!(" {} ", state.current_type.name()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        separator(),
        header_span(
            format!(" field:{} +{} ", state.current_field, state.field_offset),
            Color::Yellow,
        ),
    ];

    if let Some(ref preset) = state.current_preset {
        spans.push(separator());
        spans.push(header_span(format!(" {} ", preset), Color::Magenta));
    }

    if !state.locked_fields.is_empty() {
        spans.push(separator());
        spans.push(header_span(
            format!(" {}L ", state.locked_fields.len()),
            Color::Green,
        ));
    }

    let modes: Vec<_> = [
        state.frequency_mode.then_some("freq"),
        state.wrap_mode.then_some("wrap"),
        (!state.show_locks).then_some("~lock"),
        (!state.show_gutter).then_some("~gut"),
    ]
    .into_iter()
    .flatten()
    .collect();

    if !modes.is_empty() {
        spans.push(separator());
        spans.push(header_span(format!(" {} ", modes.join(" ")), Color::Blue));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[allow(clippy::too_many_lines)] // TUI rendering with complex field/lock/scroll logic
fn draw_records(f: &mut Frame, area: Rect, state: &mut InteractiveState) {
    state.terminal_width = area.width as usize;
    let type_size = state.current_type.byte_size().unwrap_or(1);

    // Calculate line number width based on total records
    let line_num_width = format!("{}", state.records.len()).len();
    let gutter_width = if state.show_gutter { 2 } else { 0 };
    let prefix_width = line_num_width + gutter_width + 1;

    // Calculate how many fields fit on screen
    let field_width = state.current_type.display_width();
    let visible_fields = (area.width as usize).saturating_sub(prefix_width) / field_width;
    let center_field = visible_fields / 2;

    // Calculate scroll to keep cursor centered
    let scroll_field = state.current_field.saturating_sub(center_field);

    let mut lines: Vec<Line> = Vec::new();
    let mut record_idx = state.scroll_offset;

    while lines.len() < area.height as usize && record_idx < state.records.len() {
        let record = &state.records[record_idx];
        let is_current = record_idx == state.current_record;

        let mut spans: Vec<Span> = Vec::new();

        // Line number
        let line_num_style = if is_current {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(100, 100, 100))
        };
        spans.push(Span::styled(
            format!("{:>width$}", record_idx, width = line_num_width),
            line_num_style,
        ));

        // Gutter (padding between line number and content)
        if state.show_gutter {
            spans.push(Span::raw(" ".repeat(gutter_width)));
        } else {
            spans.push(Span::raw(" "));
        }

        // Start from scrolled field position
        let start_field = scroll_field;
        let mut byte_pos = state.field_offset + (start_field * type_size);
        let mut field_idx = start_field;
        let mut fields_rendered = 0;

        // Render decoded fields - only complete fields that fit
        while byte_pos + type_size <= record.len() && fields_rendered < visible_fields {
            let is_cursor = is_current && field_idx == state.current_field;

            // Check if this field starts inside a locked field (only if show_locks is on)
            let locked_field = if state.show_locks {
                state.locked_fields.iter().find(|lf| {
                    byte_pos >= lf.byte_offset && byte_pos < lf.byte_offset + lf.byte_length
                })
            } else {
                None
            };

            // Check if this field would overflow into a locked section
            let field_end = byte_pos + type_size;
            let overflows_into_lock = if state.show_locks && locked_field.is_none() {
                state.locked_fields.iter().any(|lf| {
                    // Field starts before lock but ends inside or after lock start
                    byte_pos < lf.byte_offset && field_end > lf.byte_offset
                })
            } else {
                false
            };

            let (display_value, display_type, advance_by) = if let Some(lf) = locked_field {
                // Use locked field's type for display
                let val = decode_value(record, lf.byte_offset, lf.data_type);
                (val, lf.data_type, lf.byte_length)
            } else {
                // Use current type
                let val = decode_value(record, byte_pos, state.current_type);
                (val, state.current_type, type_size)
            };

            let byte_val = record.get(byte_pos).copied().unwrap_or(0);
            let style = field_style(
                state,
                is_cursor,
                is_current,
                locked_field.is_some(),
                overflows_into_lock,
                byte_pos,
                byte_val,
            );

            let formatted = format_field_value(&display_value, display_type);
            spans.push(Span::styled(formatted, style));
            spans.push(Span::raw(" "));

            // Advance to next field
            if let Some(lf) = locked_field {
                byte_pos = lf.byte_offset + lf.byte_length;
            } else {
                byte_pos += advance_by;
            }
            field_idx += 1;
            fields_rendered += 1;
        }

        // Handle remaining bytes that don't form a complete field (at end of record)
        if fields_rendered < visible_fields
            && byte_pos < record.len()
            && byte_pos + type_size > record.len()
        {
            let remaining = record.len() - byte_pos;
            let is_cursor = is_current && field_idx == state.current_field;
            let style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Red)
            } else {
                Style::default().fg(Color::White).bg(Color::Red)
            };
            spans.push(Span::styled(format!("?[{}]", remaining), style));
        }

        lines.push(Line::from(spans));
        record_idx += 1;
    }

    state.visible_records = area.height as usize;

    let records_widget = Paragraph::new(lines);
    f.render_widget(records_widget, area);

    // Scrollbar
    if state.records.len() > state.visible_records {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state =
            ScrollbarState::new(state.records.len()).position(state.current_record);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Decode a value from the record at the given byte offset
fn decode_value(record: &[u8], byte_off: usize, dtype: DataType) -> String {
    record
        .get(byte_off..)
        .map(|data| dtype.decode(data))
        .unwrap_or_default()
}

/// Determine the style for a field based on cursor, lock, and frequency state
fn field_style(
    state: &InteractiveState,
    is_cursor: bool,
    is_current_record: bool,
    locked: bool,
    overflows: bool,
    byte_pos: usize,
    byte_val: u8,
) -> Style {
    if overflows {
        Style::default().fg(Color::White).bg(Color::Red)
    } else if is_cursor {
        Style::default().fg(Color::Black).bg(Color::Yellow)
    } else if locked {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else if state.frequency_mode && is_current_record {
        let freq_color = state.get_frequency_color(byte_pos, byte_val);
        Style::default().fg(freq_color).add_modifier(Modifier::BOLD)
    } else if is_current_record {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Rgb(100, 100, 100))
    }
}

/// Format a field value with consistent width for the data type
fn format_field_value(value: &str, dtype: DataType) -> String {
    let width = match dtype {
        DataType::U8 => 3,                       // 0-255
        DataType::Hex => 2,                      // 00-ff
        DataType::Binary => 8,                   // 8 bits
        DataType::U16Le | DataType::U16Be => 5,  // 0-65535
        DataType::U32Le | DataType::U32Be => 10, // 0-4294967295
        DataType::VarInt => 10,                  // variable, but cap display
        DataType::Ascii => 1,                    // single character
    };
    format!("{:>width$}", value, width = width)
}

fn draw_status_bar(f: &mut Frame, area: Rect, state: &InteractiveState) {
    // Command mode overlays the status bar
    if state.command_mode {
        let line = Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::styled(&state.command_buffer, Style::default().fg(Color::White)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]);
        let widget = Paragraph::new(line);
        f.render_widget(widget, area);
        return;
    }

    let mut spans: Vec<Span> = Vec::new();

    // Byte offset of current field
    let byte_off = state.current_field_byte();
    spans.push(Span::styled(
        format!(" byte:{} ", byte_off),
        Style::default().fg(Color::Cyan),
    ));

    // Current record length
    if let Some(record) = state.records.get(state.current_record) {
        spans.push(Span::styled(
            format!("len:{} ", record.len()),
            Style::default().fg(Color::Rgb(150, 150, 150)),
        ));
    }

    // Message if any (right side)
    if let Some(ref msg) = state.message {
        let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
        let msg_len = msg.len();
        let available = area.width as usize;

        if left_len + msg_len + 2 < available {
            let padding = available - left_len - msg_len - 1;
            spans.push(Span::raw(" ".repeat(padding)));
            spans.push(Span::styled(msg, Style::default().fg(Color::Yellow)));
        }
    }

    let widget = Paragraph::new(Line::from(spans));
    f.render_widget(widget, area);
}
