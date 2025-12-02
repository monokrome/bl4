//! # bl4
//!
//! Borderlands 4 save editor library - encryption, decryption, and parsing.
//!
//! This library provides functionality to:
//! - Decrypt and encrypt Borderlands 4 .sav files
//! - Parse decrypted YAML save data
//! - Decode item serials (weapons, equipment, etc.)
//! - Modify save data (level, currency, inventory, etc.)
//!
//! ## Example
//!
//! ```no_run
//! use std::fs;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let encrypted = fs::read("1.sav")?;
//! let steam_id = "76561197960521364";
//!
//! // Decrypt and parse save file
//! let yaml_data = bl4::decrypt_sav(&encrypted, steam_id)?;
//! let mut save = bl4::SaveFile::from_yaml(&yaml_data)?;
//!
//! // Query and modify save data
//! println!("Character: {:?}", save.get_character_name());
//! println!("Cash: {:?}", save.get_cash());
//!
//! save.set_cash(999999)?;
//! save.set_character_name("NewName")?;
//!
//! // Re-encrypt and save
//! let modified_yaml = save.to_yaml()?;
//! let encrypted = bl4::encrypt_sav(&modified_yaml, steam_id)?;
//! fs::write("1.sav", encrypted)?;
//! # Ok(())
//! # }
//! ```

pub mod backup;
pub mod crypto;
pub mod parts;
pub mod save;
pub mod serial;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export commonly used items
pub use backup::{smart_backup, update_after_edit, BackupError};
pub use crypto::{decrypt_sav, derive_key, encrypt_sav, CryptoError};
pub use parts::{
    category_name, item_type_name, manufacturer_name, CategoryPartInfo, CategoryPartsDatabase,
    PartInfo, PartsDatabase,
};
pub use save::{ChangeSet, SaveError, SaveFile};
pub use serial::{ItemSerial, SerialError};
