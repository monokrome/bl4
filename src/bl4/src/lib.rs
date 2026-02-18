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
pub mod manifest;
pub mod parts;
pub mod reference;
pub mod save;
pub mod serial;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export commonly used items
#[doc(inline)]
pub use backup::{smart_backup, update_after_edit, BackupError};
#[doc(inline)]
pub use crypto::{decrypt_sav, derive_key, encrypt_sav, CryptoError};
#[doc(inline)]
pub use parts::{
    category_from_varbit, category_name, level_from_code, manufacturer_name,
    serial_id_to_parts_category, varbit_divisor,
};
#[doc(inline)]
pub use save::{ChangeSet, SaveError, SaveFile, StateFlags};
#[doc(inline)]
pub use serial::{ItemSerial, RarityEstimate, SerialError};

// Manifest data lookups
#[doc(inline)]
pub use manifest::{
    all_categories, all_manufacturers, drop_pool, part_name, stats as manifest_stats,
    world_pool_legendary_count, DropPool,
};

// Reference data (rarities, elements, weapon types, manufacturers, gear types)
#[doc(inline)]
pub use reference::{
    element_by_code, gear_type_by_code, legendary_by_name, manufacturer_by_code,
    manufacturer_by_name, manufacturer_name_by_code, rarity_by_code, rarity_by_tier,
    rarity_probability, stat_description, weapon_type_by_code, weapon_type_by_name, ElementType,
    GearType, LegendaryItem, Manufacturer, RarityTier, WeaponType, ELEMENT_TYPES, GEAR_TYPES,
    KNOWN_LEGENDARIES, MANUFACTURERS, RARITY_TIERS, WEAPON_TYPES,
};
