//! WebAssembly bindings for bl4
//!
//! This module provides JavaScript-friendly bindings for the core bl4 library.

use crate::crypto::{decrypt_sav as rust_decrypt, encrypt_sav as rust_encrypt};
use crate::save::{ChangeSet as RustChangeSet, SaveFile as RustSaveFile};
use wasm_bindgen::prelude::*;

/// Decrypt a Borderlands 4 save file
///
/// # Arguments
/// * `encrypted_data` - The encrypted .sav file as bytes
/// * `steam_id` - Steam ID for decryption
///
/// # Returns
/// Decrypted YAML data as bytes
#[wasm_bindgen(js_name = decryptSav)]
pub fn decrypt_sav(encrypted_data: &[u8], steam_id: &str) -> Result<Vec<u8>, JsValue> {
    rust_decrypt(encrypted_data, steam_id)
        .map_err(|e| JsValue::from_str(&format!("Decryption failed: {}", e)))
}

/// Encrypt YAML data to a Borderlands 4 save file
///
/// # Arguments
/// * `yaml_data` - The YAML data as bytes
/// * `steam_id` - Steam ID for encryption
///
/// # Returns
/// Encrypted .sav file data
#[wasm_bindgen(js_name = encryptSav)]
pub fn encrypt_sav(yaml_data: &[u8], steam_id: &str) -> Result<Vec<u8>, JsValue> {
    rust_encrypt(yaml_data, steam_id)
        .map_err(|e| JsValue::from_str(&format!("Encryption failed: {}", e)))
}

/// JavaScript-friendly SaveFile wrapper
#[wasm_bindgen]
pub struct SaveFile {
    inner: RustSaveFile,
}

#[wasm_bindgen]
impl SaveFile {
    /// Parse a save file from decrypted YAML data
    #[wasm_bindgen(constructor)]
    pub fn new(yaml_data: &[u8]) -> Result<SaveFile, JsValue> {
        let inner = RustSaveFile::from_yaml(yaml_data)
            .map_err(|e| JsValue::from_str(&format!("Parse failed: {}", e)))?;
        Ok(SaveFile { inner })
    }

    /// Serialize back to YAML
    #[wasm_bindgen(js_name = toYaml)]
    pub fn to_yaml(&self) -> Result<Vec<u8>, JsValue> {
        self.inner
            .to_yaml()
            .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
    }

    /// Query a value at a YAML path
    #[wasm_bindgen(js_name = get)]
    pub fn get(&self, path: &str) -> Result<String, JsValue> {
        let value = self
            .inner
            .get(path)
            .map_err(|e| JsValue::from_str(&format!("Query failed: {}", e)))?;
        serde_yaml::to_string(&value)
            .map_err(|e| JsValue::from_str(&format!("Serialize failed: {}", e)))
    }

    /// Set a value at a YAML path (from string, auto-parsed)
    #[wasm_bindgen(js_name = set)]
    pub fn set(&mut self, path: &str, value: &str) -> Result<(), JsValue> {
        let parsed_value = RustSaveFile::parse_value(value);
        self.inner
            .set(path, parsed_value)
            .map_err(|e| JsValue::from_str(&format!("Set failed: {}", e)))
    }

    /// Set raw YAML value from string
    #[wasm_bindgen(js_name = setRaw)]
    pub fn set_raw(&mut self, path: &str, yaml_str: &str) -> Result<(), JsValue> {
        self.inner
            .set_raw(path, yaml_str)
            .map_err(|e| JsValue::from_str(&format!("Set raw failed: {}", e)))
    }

    // Convenience methods

    #[wasm_bindgen(js_name = getCharacterName)]
    pub fn get_character_name(&self) -> Option<String> {
        self.inner.get_character_name().map(String::from)
    }

    #[wasm_bindgen(js_name = setCharacterName)]
    pub fn set_character_name(&mut self, name: &str) -> Result<(), JsValue> {
        self.inner
            .set_character_name(name)
            .map_err(|e| JsValue::from_str(&format!("Set name failed: {}", e)))
    }

    #[wasm_bindgen(js_name = getCharacterClass)]
    pub fn get_character_class(&self) -> Option<String> {
        self.inner.get_character_class().map(String::from)
    }

    #[wasm_bindgen(js_name = getDifficulty)]
    pub fn get_difficulty(&self) -> Option<String> {
        self.inner.get_difficulty().map(String::from)
    }

    #[wasm_bindgen(js_name = getCash)]
    pub fn get_cash(&self) -> Option<f64> {
        self.inner.get_cash().map(|v| v as f64)
    }

    #[wasm_bindgen(js_name = setCash)]
    pub fn set_cash(&mut self, amount: f64) -> Result<(), JsValue> {
        self.inner
            .set_cash(amount as u64)
            .map_err(|e| JsValue::from_str(&format!("Set cash failed: {}", e)))
    }

    #[wasm_bindgen(js_name = getEridium)]
    pub fn get_eridium(&self) -> Option<f64> {
        self.inner.get_eridium().map(|v| v as f64)
    }

    #[wasm_bindgen(js_name = setEridium)]
    pub fn set_eridium(&mut self, amount: f64) -> Result<(), JsValue> {
        self.inner
            .set_eridium(amount as u64)
            .map_err(|e| JsValue::from_str(&format!("Set eridium failed: {}", e)))
    }

    #[wasm_bindgen(js_name = getCharacterLevel)]
    pub fn get_character_level(&self) -> Option<js_sys::Array> {
        self.inner.get_character_level().map(|(level, xp)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from_f64(level as f64));
            arr.push(&JsValue::from_f64(xp as f64));
            arr
        })
    }

    #[wasm_bindgen(js_name = setCharacterXp)]
    pub fn set_character_xp(&mut self, xp: f64) -> Result<(), JsValue> {
        self.inner
            .set_character_xp(xp as u64)
            .map_err(|e| JsValue::from_str(&format!("Set XP failed: {}", e)))
    }

    #[wasm_bindgen(js_name = getSpecializationLevel)]
    pub fn get_specialization_level(&self) -> Option<js_sys::Array> {
        self.inner.get_specialization_level().map(|(level, xp)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from_f64(level as f64));
            arr.push(&JsValue::from_f64(xp as f64));
            arr
        })
    }

    #[wasm_bindgen(js_name = setSpecializationXp)]
    pub fn set_specialization_xp(&mut self, xp: f64) -> Result<(), JsValue> {
        self.inner
            .set_specialization_xp(xp as u64)
            .map_err(|e| JsValue::from_str(&format!("Set spec XP failed: {}", e)))
    }
}

/// JavaScript-friendly ChangeSet wrapper
#[wasm_bindgen]
pub struct ChangeSet {
    inner: RustChangeSet,
}

#[wasm_bindgen]
impl ChangeSet {
    /// Create a new ChangeSet
    #[wasm_bindgen(constructor)]
    pub fn new() -> ChangeSet {
        ChangeSet {
            inner: RustChangeSet::new(),
        }
    }

    /// Add a change (value auto-parsed from string)
    #[wasm_bindgen(js_name = add)]
    pub fn add(&mut self, path: String, value: &str) {
        self.inner.add_parsed(path, value);
    }

    /// Add raw YAML change
    #[wasm_bindgen(js_name = addRaw)]
    pub fn add_raw(&mut self, path: String, yaml_str: &str) -> Result<(), JsValue> {
        self.inner
            .add_raw(path, yaml_str)
            .map_err(|e| JsValue::from_str(&format!("Add raw failed: {}", e)))
    }

    /// Check if a path has a pending change
    #[wasm_bindgen(js_name = hasChange)]
    pub fn has_change(&self, path: &str) -> bool {
        self.inner.has_change(path)
    }

    /// Remove a change
    #[wasm_bindgen(js_name = remove)]
    pub fn remove(&mut self, path: &str) -> bool {
        self.inner.remove(path).is_some()
    }

    /// Clear all changes
    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Get number of changes
    #[wasm_bindgen(js_name = length)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Apply changes to a SaveFile
    #[wasm_bindgen(js_name = apply)]
    pub fn apply(&self, save: &mut SaveFile) -> Result<(), JsValue> {
        self.inner
            .apply(&mut save.inner)
            .map_err(|e| JsValue::from_str(&format!("Apply failed: {}", e)))
    }

    // Convenience methods

    #[wasm_bindgen(js_name = setCharacterName)]
    pub fn set_character_name(&mut self, name: &str) {
        self.inner.set_character_name(name);
    }

    #[wasm_bindgen(js_name = setCash)]
    pub fn set_cash(&mut self, amount: f64) {
        self.inner.set_cash(amount as u64);
    }

    #[wasm_bindgen(js_name = setEridium)]
    pub fn set_eridium(&mut self, amount: f64) {
        self.inner.set_eridium(amount as u64);
    }

    #[wasm_bindgen(js_name = setCharacterXp)]
    pub fn set_character_xp(&mut self, xp: f64) {
        self.inner.set_character_xp(xp as u64);
    }

    #[wasm_bindgen(js_name = setSpecializationXp)]
    pub fn set_specialization_xp(&mut self, xp: f64) {
        self.inner.set_specialization_xp(xp as u64);
    }
}

impl Default for ChangeSet {
    fn default() -> Self {
        Self::new()
    }
}
