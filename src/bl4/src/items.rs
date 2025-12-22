//! Items Database
//!
//! SQLite-based storage for verified item data including serials,
//! metadata, parts, and image attachments.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Default database location
pub const DEFAULT_DB_PATH: &str = "share/items.db";

/// Verification status for items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Decoded,
    Screenshot,
    Verified,
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => write!(f, "unverified"),
            Self::Decoded => write!(f, "decoded"),
            Self::Screenshot => write!(f, "screenshot"),
            Self::Verified => write!(f, "verified"),
        }
    }
}

impl std::str::FromStr for VerificationStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "unverified" => Ok(Self::Unverified),
            "decoded" => Ok(Self::Decoded),
            "screenshot" => Ok(Self::Screenshot),
            "verified" => Ok(Self::Verified),
            _ => anyhow::bail!("Unknown verification status: {}", s),
        }
    }
}

/// Item entry in the database (serial is the primary key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub serial: String,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: Option<String>,
    pub item_type: Option<String>, // Serial type char: 'r', 'e', '!', 'd', etc.
    pub rarity: Option<String>,
    pub level: Option<i32>,
    pub element: Option<String>,
    pub dps: Option<i32>,
    pub damage: Option<i32>,
    pub accuracy: Option<i32>,
    pub fire_rate: Option<f64>,
    pub reload_time: Option<f64>,
    pub mag_size: Option<i32>,
    pub value: Option<i32>,
    pub red_text: Option<String>,
    pub notes: Option<String>,
    pub verification_status: VerificationStatus,
    pub verification_notes: Option<String>,
    pub verified_at: Option<String>,
    pub legal: bool,            // Whether item is verified legal (not modded)
    pub source: Option<String>, // Import source: monokrome, ryechews, community, etc.
    pub created_at: String,
}

/// Weapon part entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPart {
    pub id: i64,
    pub item_serial: String,
    pub slot: String,
    pub part_index: Option<i32>,
    pub part_name: Option<String>,
    pub manufacturer: Option<String>,
    pub effect: Option<String>,
    pub verified: bool,
    pub verification_method: Option<String>,
    pub verification_notes: Option<String>,
    pub verified_at: Option<String>,
}

/// Image attachment entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: i64,
    pub item_serial: String,
    pub name: String,
    pub mime_type: String,
    /// View type: POPUP (item card), DETAIL (3D inspect), or OTHER
    pub view: String,
}

/// Items database manager
pub struct ItemsDb {
    conn: Connection,
}

impl ItemsDb {
    /// Open or create the items database
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open database: {}", path.as_ref().display()))?;
        Ok(Self { conn })
    }

    /// Initialize the database schema
    pub fn init(&self) -> Result<()> {
        // Check if we need to migrate from old schema (id-based) to new (serial-based)
        let needs_migration = self.conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='weapons'
                 AND sql LIKE '%id INTEGER PRIMARY KEY%'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if needs_migration {
            self.migrate_to_serial_pk()?;
        }

        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS weapons (
                serial TEXT PRIMARY KEY NOT NULL,
                name TEXT,
                prefix TEXT,
                manufacturer TEXT,
                weapon_type TEXT,
                item_type TEXT,                  -- Serial type char: r, e, !, d, etc.
                rarity TEXT,
                level INTEGER,
                element TEXT,
                dps INTEGER,
                damage INTEGER,
                accuracy INTEGER,
                fire_rate REAL,
                reload_time REAL,
                mag_size INTEGER,
                value INTEGER,
                red_text TEXT,
                notes TEXT,
                -- Verification tracking
                verification_status TEXT DEFAULT 'unverified',  -- unverified, decoded, screenshot, verified
                verification_notes TEXT,
                verified_at TIMESTAMP,
                legal BOOLEAN DEFAULT FALSE,     -- Whether item is verified legal (not modded)
                source TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS weapon_parts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                slot TEXT NOT NULL,              -- grip, barrel, body, scope, accessory, element, etc.
                part_index INTEGER,              -- decoded index from serial
                part_name TEXT,                  -- resolved part name (e.g., "JAK_PS.part_grip_04")
                manufacturer TEXT,
                effect TEXT,
                -- Part verification
                verified BOOLEAN DEFAULT FALSE,
                verification_method TEXT,        -- inspect_screen, visual_compare, inferred, etc.
                verification_notes TEXT,
                verified_at TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS attachments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                name TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                data BLOB NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_weapons_name ON weapons(name);
            CREATE INDEX IF NOT EXISTS idx_weapons_manufacturer ON weapons(manufacturer);
            CREATE INDEX IF NOT EXISTS idx_weapon_parts_item_serial ON weapon_parts(item_serial);
            CREATE INDEX IF NOT EXISTS idx_attachments_item_serial ON attachments(item_serial);
            "#,
        )?;
        Ok(())
    }

    /// Migrate old id-based schema to serial-based schema
    fn migrate_to_serial_pk(&self) -> Result<()> {
        println!("Migrating database to use serial as primary key...");

        self.conn.execute_batch(
            r#"
            -- Rename old tables
            ALTER TABLE weapons RENAME TO weapons_old;
            ALTER TABLE weapon_parts RENAME TO weapon_parts_old;
            ALTER TABLE attachments RENAME TO attachments_old;

            -- Create new tables with serial as PK
            CREATE TABLE weapons (
                serial TEXT PRIMARY KEY NOT NULL,
                name TEXT,
                prefix TEXT,
                manufacturer TEXT,
                weapon_type TEXT,
                item_type TEXT,
                rarity TEXT,
                level INTEGER,
                element TEXT,
                dps INTEGER,
                damage INTEGER,
                accuracy INTEGER,
                fire_rate REAL,
                reload_time REAL,
                mag_size INTEGER,
                value INTEGER,
                red_text TEXT,
                notes TEXT,
                verification_status TEXT DEFAULT 'unverified',
                verification_notes TEXT,
                verified_at TIMESTAMP,
                legal BOOLEAN DEFAULT FALSE,
                source TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE weapon_parts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                slot TEXT NOT NULL,
                part_index INTEGER,
                part_name TEXT,
                manufacturer TEXT,
                effect TEXT,
                verified BOOLEAN DEFAULT FALSE,
                verification_method TEXT,
                verification_notes TEXT,
                verified_at TIMESTAMP
            );

            CREATE TABLE attachments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                name TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                data BLOB NOT NULL,
                view TEXT DEFAULT 'OTHER'
            );

            -- Copy data from old tables
            INSERT INTO weapons (serial, name, prefix, manufacturer, weapon_type, item_type, rarity,
                level, element, dps, damage, accuracy, fire_rate, reload_time, mag_size, value,
                red_text, notes, verification_status, verification_notes, verified_at, legal,
                source, created_at)
            SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity,
                level, element, dps, damage, accuracy, fire_rate, reload_time, mag_size, value,
                red_text, notes, verification_status, verification_notes, verified_at, legal,
                source, created_at
            FROM weapons_old;

            INSERT INTO weapon_parts (item_serial, slot, part_index, part_name, manufacturer,
                effect, verified, verification_method, verification_notes, verified_at)
            SELECT w.serial, wp.slot, wp.part_index, wp.part_name, wp.manufacturer,
                wp.effect, wp.verified, wp.verification_method, wp.verification_notes, wp.verified_at
            FROM weapon_parts_old wp
            JOIN weapons_old w ON wp.weapon_id = w.id;

            INSERT INTO attachments (item_serial, name, mime_type, data)
            SELECT w.serial, a.name, a.mime_type, a.data
            FROM attachments_old a
            JOIN weapons_old w ON a.weapon_id = w.id;

            -- Drop old tables
            DROP TABLE attachments_old;
            DROP TABLE weapon_parts_old;
            DROP TABLE weapons_old;

            -- Drop old indexes (they reference old tables)
            DROP INDEX IF EXISTS idx_weapons_serial;
            DROP INDEX IF EXISTS idx_weapon_parts_weapon_id;
            DROP INDEX IF EXISTS idx_attachments_weapon_id;
            "#,
        )?;

        println!("Migration complete.");
        Ok(())
    }

    /// Add a new item to the database
    pub fn add_item(&self, serial: &str) -> Result<()> {
        self.conn
            .execute("INSERT INTO weapons (serial) VALUES (?1)", params![serial])?;
        Ok(())
    }

    /// Update item metadata
    #[allow(clippy::too_many_arguments)]
    pub fn update_item(
        &self,
        serial: &str,
        name: Option<&str>,
        prefix: Option<&str>,
        manufacturer: Option<&str>,
        weapon_type: Option<&str>,
        rarity: Option<&str>,
        level: Option<i32>,
        element: Option<&str>,
        dps: Option<i32>,
        damage: Option<i32>,
        accuracy: Option<i32>,
        fire_rate: Option<f64>,
        reload_time: Option<f64>,
        mag_size: Option<i32>,
        value: Option<i32>,
        red_text: Option<&str>,
        notes: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE weapons SET
                name = COALESCE(?2, name),
                prefix = COALESCE(?3, prefix),
                manufacturer = COALESCE(?4, manufacturer),
                weapon_type = COALESCE(?5, weapon_type),
                rarity = COALESCE(?6, rarity),
                level = COALESCE(?7, level),
                element = COALESCE(?8, element),
                dps = COALESCE(?9, dps),
                damage = COALESCE(?10, damage),
                accuracy = COALESCE(?11, accuracy),
                fire_rate = COALESCE(?12, fire_rate),
                reload_time = COALESCE(?13, reload_time),
                mag_size = COALESCE(?14, mag_size),
                value = COALESCE(?15, value),
                red_text = COALESCE(?16, red_text),
                notes = COALESCE(?17, notes)
            WHERE serial = ?1"#,
            params![
                serial,
                name,
                prefix,
                manufacturer,
                weapon_type,
                rarity,
                level,
                element,
                dps,
                damage,
                accuracy,
                fire_rate,
                reload_time,
                mag_size,
                value,
                red_text,
                notes
            ],
        )?;
        Ok(())
    }

    /// Get an item by serial
    pub fn get_item(&self, serial: &str) -> Result<Option<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                    dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                    notes, verification_status, verification_notes, verified_at, legal, source, created_at
             FROM weapons WHERE serial = ?1",
        )?;

        let weapon = stmt
            .query_row(params![serial], |row| {
                let status_str: String = row
                    .get::<_, Option<String>>(18)?
                    .unwrap_or_else(|| "unverified".to_string());
                Ok(Item {
                    serial: row.get(0)?,
                    name: row.get(1)?,
                    prefix: row.get(2)?,
                    manufacturer: row.get(3)?,
                    weapon_type: row.get(4)?,
                    item_type: row.get(5)?,
                    rarity: row.get(6)?,
                    level: row.get(7)?,
                    element: row.get(8)?,
                    dps: row.get(9)?,
                    damage: row.get(10)?,
                    accuracy: row.get(11)?,
                    fire_rate: row.get(12)?,
                    reload_time: row.get(13)?,
                    mag_size: row.get(14)?,
                    value: row.get(15)?,
                    red_text: row.get(16)?,
                    notes: row.get(17)?,
                    verification_status: status_str
                        .parse()
                        .unwrap_or(VerificationStatus::Unverified),
                    verification_notes: row.get(19)?,
                    verified_at: row.get(20)?,
                    legal: row.get::<_, Option<bool>>(21)?.unwrap_or(false),
                    source: row.get(22)?,
                    created_at: row.get(23)?,
                })
            })
            .optional()?;

        Ok(weapon)
    }

    /// List all items with optional filters
    pub fn list_items(
        &self,
        manufacturer: Option<&str>,
        weapon_type: Option<&str>,
        element: Option<&str>,
        rarity: Option<&str>,
    ) -> Result<Vec<Item>> {
        let mut sql = String::from(
            "SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                    dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                    notes, verification_status, verification_notes, verified_at, legal, source, created_at
             FROM weapons WHERE 1=1",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(m) = manufacturer {
            sql.push_str(" AND manufacturer = ?");
            params_vec.push(Box::new(m.to_string()));
        }
        if let Some(w) = weapon_type {
            sql.push_str(" AND weapon_type = ?");
            params_vec.push(Box::new(w.to_string()));
        }
        if let Some(e) = element {
            sql.push_str(" AND element = ?");
            params_vec.push(Box::new(e.to_string()));
        }
        if let Some(r) = rarity {
            sql.push_str(" AND rarity = ?");
            params_vec.push(Box::new(r.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let weapons = stmt
            .query_map(params_refs.as_slice(), |row| {
                let status_str: String = row
                    .get::<_, Option<String>>(18)?
                    .unwrap_or_else(|| "unverified".to_string());
                Ok(Item {
                    serial: row.get(0)?,
                    name: row.get(1)?,
                    prefix: row.get(2)?,
                    manufacturer: row.get(3)?,
                    weapon_type: row.get(4)?,
                    item_type: row.get(5)?,
                    rarity: row.get(6)?,
                    level: row.get(7)?,
                    element: row.get(8)?,
                    dps: row.get(9)?,
                    damage: row.get(10)?,
                    accuracy: row.get(11)?,
                    fire_rate: row.get(12)?,
                    reload_time: row.get(13)?,
                    mag_size: row.get(14)?,
                    value: row.get(15)?,
                    red_text: row.get(16)?,
                    notes: row.get(17)?,
                    verification_status: status_str
                        .parse()
                        .unwrap_or(VerificationStatus::Unverified),
                    verification_notes: row.get(19)?,
                    verified_at: row.get(20)?,
                    legal: row.get::<_, Option<bool>>(21)?.unwrap_or(false),
                    source: row.get(22)?,
                    created_at: row.get(23)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(weapons)
    }

    /// Update item verification status
    pub fn set_verification_status(
        &self,
        serial: &str,
        status: VerificationStatus,
        notes: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE weapons SET
                verification_status = ?2,
                verification_notes = COALESCE(?3, verification_notes),
                verified_at = CASE WHEN ?2 != 'unverified' THEN CURRENT_TIMESTAMP ELSE verified_at END
            WHERE serial = ?1"#,
            params![serial, status.to_string(), notes],
        )?;
        Ok(())
    }

    /// Set legal status for an item
    pub fn set_legal(&self, serial: &str, legal: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE weapons SET legal = ?2 WHERE serial = ?1",
            params![serial, legal],
        )?;
        Ok(())
    }

    /// Set legal status for all items
    pub fn set_all_legal(&self, legal: bool) -> Result<usize> {
        let rows = self
            .conn
            .execute("UPDATE weapons SET legal = ?1", params![legal])?;
        Ok(rows)
    }

    /// Set item type for an item
    pub fn set_item_type(&self, serial: &str, item_type: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE weapons SET item_type = ?2 WHERE serial = ?1",
            params![serial, item_type],
        )?;
        Ok(())
    }

    /// Set source for an item
    pub fn set_source(&self, serial: &str, source: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE weapons SET source = ?2 WHERE serial = ?1",
            params![serial, source],
        )?;
        Ok(())
    }

    /// Set source for items matching a condition
    pub fn set_source_where(&self, source: &str, condition: &str) -> Result<usize> {
        let sql = format!("UPDATE weapons SET source = ?1 WHERE {}", condition);
        let rows = self.conn.execute(&sql, params![source])?;
        Ok(rows)
    }

    /// Set source for items without a source
    pub fn set_source_for_null(&self, source: &str) -> Result<usize> {
        let rows = self.conn.execute(
            "UPDATE weapons SET source = ?1 WHERE source IS NULL",
            params![source],
        )?;
        Ok(rows)
    }

    /// Get parts for an item
    pub fn get_parts(&self, item_serial: &str) -> Result<Vec<ItemPart>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_serial, slot, part_index, part_name, manufacturer, effect,
                    verified, verification_method, verification_notes, verified_at
             FROM weapon_parts WHERE item_serial = ?1",
        )?;

        let parts = stmt
            .query_map(params![item_serial], |row| {
                Ok(ItemPart {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    slot: row.get(2)?,
                    part_index: row.get(3)?,
                    part_name: row.get(4)?,
                    manufacturer: row.get(5)?,
                    effect: row.get(6)?,
                    verified: row.get::<_, Option<bool>>(7)?.unwrap_or(false),
                    verification_method: row.get(8)?,
                    verification_notes: row.get(9)?,
                    verified_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(parts)
    }

    /// Add an image attachment
    pub fn add_attachment(
        &self,
        item_serial: &str,
        name: &str,
        mime_type: &str,
        data: &[u8],
        view: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO attachments (item_serial, name, mime_type, data, view) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item_serial, name, mime_type, data, view],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get attachments for an item (without data)
    pub fn get_attachments(&self, item_serial: &str) -> Result<Vec<Attachment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_serial, name, mime_type, COALESCE(view, 'OTHER') FROM attachments WHERE item_serial = ?1",
        )?;

        let attachments = stmt
            .query_map(params![item_serial], |row| {
                Ok(Attachment {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    name: row.get(2)?,
                    mime_type: row.get(3)?,
                    view: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(attachments)
    }

    /// Get attachment data
    pub fn get_attachment_data(&self, id: i64) -> Result<Option<Vec<u8>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM attachments WHERE id = ?1")?;
        let data = stmt.query_row(params![id], |row| row.get(0)).optional()?;
        Ok(data)
    }

    /// Import an item from a directory (share/items format)
    pub fn import_from_dir<P: AsRef<Path>>(&self, dir: P) -> Result<String> {
        let dir = dir.as_ref();

        // Read serial
        let serial_path = dir.join("serial.txt");
        let serial = std::fs::read_to_string(&serial_path)
            .with_context(|| format!("Failed to read serial from {}", serial_path.display()))?
            .trim()
            .to_string();

        // Check if already exists
        if let Some(existing) = self.get_item(&serial)? {
            println!("Item already exists: {}", existing.serial);
            return Ok(existing.serial);
        }

        // Add item
        self.add_item(&serial)?;

        // Parse directory name for metadata hints
        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let parts: Vec<&str> = dir_name.split('_').collect();

        // Try to extract manufacturer and type from directory name (e.g., JAK_PS_seventh_sense_cryo)
        if parts.len() >= 2 {
            let manufacturer = Some(parts[0]);
            let weapon_type = Some(parts[1]);
            let name = if parts.len() > 2 {
                Some(parts[2..].join("_").replace('_', " "))
            } else {
                None
            };

            self.update_item(
                &serial,
                name.as_deref(),
                None,
                manufacturer,
                weapon_type,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )?;
        }

        // Import images
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "png").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                // Infer view type from filename
                let view = match name {
                    "inventory" | "stats" => "POPUP",
                    "inspect" => "DETAIL",
                    _ => "OTHER",
                };
                let data = std::fs::read(&path)?;
                self.add_attachment(&serial, name, "image/png", &data, view)?;
            }
        }

        Ok(serial)
    }

    /// Export an item to a directory
    pub fn export_to_dir<P: AsRef<Path>>(&self, serial: &str, dir: P) -> Result<()> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;

        let item = self
            .get_item(serial)?
            .with_context(|| format!("Item {} not found", serial))?;

        // Write serial
        std::fs::write(dir.join("serial.txt"), &item.serial)?;

        // Write metadata as JSON
        let metadata = serde_json::to_string_pretty(&item)?;
        std::fs::write(dir.join("metadata.json"), metadata)?;

        // Export attachments
        let attachments = self.get_attachments(serial)?;
        for attachment in attachments {
            if let Some(data) = self.get_attachment_data(attachment.id)? {
                let ext = match attachment.mime_type.as_str() {
                    "image/png" => "png",
                    "image/jpeg" => "jpg",
                    _ => "bin",
                };
                let filename = format!("{}.{}", attachment.name, ext);
                std::fs::write(dir.join(filename), data)?;
            }
        }

        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DbStats> {
        let item_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM weapons", [], |row| row.get(0))?;
        let part_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM weapon_parts", [], |row| row.get(0))?;
        let attachment_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM attachments", [], |row| row.get(0))?;

        Ok(DbStats {
            item_count,
            part_count,
            attachment_count,
        })
    }
}

/// Database statistics
#[derive(Debug, Serialize)]
pub struct DbStats {
    pub item_count: i64,
    pub part_count: i64,
    pub attachment_count: i64,
}
