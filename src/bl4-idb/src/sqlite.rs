//! SQLite implementation using rusqlite (synchronous).
//!
//! This implementation is used by the CLI tool.

use crate::repository::*;
use crate::shared::{self, ITEM_SELECT_COLUMNS};
use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

/// Default database location
pub const DEFAULT_DB_PATH: &str = "share/items.db";

/// SQLite-backed items database
pub struct SqliteDb {
    conn: Connection,
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
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
        verification_status: status_str.parse().unwrap_or(VerificationStatus::Unverified),
        verification_notes: row.get(19)?,
        verified_at: row.get(20)?,
        legal: row.get::<_, Option<bool>>(21)?.unwrap_or(false),
        source: row.get(22)?,
        created_at: row.get::<_, Option<String>>(23)?.unwrap_or_default(),
    })
}

/// Build parameter vector from filter for rusqlite queries
fn build_filter_params(filter: &ItemFilter) -> Vec<Box<dyn rusqlite::ToSql>> {
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(m) = &filter.manufacturer {
        params.push(Box::new(m.clone()));
    }
    if let Some(w) = &filter.weapon_type {
        params.push(Box::new(w.clone()));
    }
    if let Some(e) = &filter.element {
        params.push(Box::new(e.clone()));
    }
    if let Some(r) = &filter.rarity {
        params.push(Box::new(r.clone()));
    }
    params
}

impl SqliteDb {
    /// Open or create the database
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path.as_ref())?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Check if migration from old schema is needed and perform it
    #[allow(clippy::too_many_lines)] // SQL schema definition
    fn migrate_to_serial_pk(&self) -> RepoResult<()> {
        println!("Migrating database to use serial as primary key...");

        self.conn
            .execute_batch(
                r#"
            ALTER TABLE weapons RENAME TO weapons_old;
            ALTER TABLE weapon_parts RENAME TO weapon_parts_old;
            ALTER TABLE attachments RENAME TO attachments_old;

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

            DROP TABLE attachments_old;
            DROP TABLE weapon_parts_old;
            DROP TABLE weapons_old;

            DROP INDEX IF EXISTS idx_weapons_serial;
            DROP INDEX IF EXISTS idx_weapon_parts_weapon_id;
            DROP INDEX IF EXISTS idx_attachments_weapon_id;
            "#,
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        println!("Migration complete.");
        Ok(())
    }

    /// Get a setting value
    pub fn get_setting(&self, key: &str) -> RepoResult<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(RepoError::Database(e.to_string())),
        }
    }

    /// Set a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> RepoResult<()> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    /// Get the source salt, generating one if it doesn't exist
    pub fn get_or_create_salt(&self) -> RepoResult<String> {
        if let Some(salt) = self.get_setting("source_salt")? {
            Ok(salt)
        } else {
            let salt = crate::generate_salt();
            self.set_setting("source_salt", &salt)?;
            Ok(salt)
        }
    }

    /// Get all distinct sources from the database
    pub fn get_distinct_sources(&self) -> RepoResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT source FROM items WHERE source IS NOT NULL")
            .map_err(|e| RepoError::Database(e.to_string()))?;
        let sources = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(sources)
    }
}

impl SqliteDb {
    /// Check if a migration has been applied
    fn is_migration_applied(&self, version: &str) -> RepoResult<bool> {
        let result: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                params![version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(result.is_some())
    }

    /// Mark a migration as applied
    fn mark_migration_applied(&self, version: &str) -> RepoResult<()> {
        self.conn
            .execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                params![version],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    /// Run pending migrations
    #[allow(clippy::too_many_lines)] // SQL schema definition
    fn run_migrations(&self) -> RepoResult<()> {
        // Check if tables already exist (for existing databases)
        let tables_exist = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='weapons'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        // Migration 0001: Base schema (weapons, weapon_parts, attachments, item_values, settings)
        if !self.is_migration_applied("0001_base_schema")? {
            self.conn
                .execute_batch(
                    r#"
                CREATE TABLE IF NOT EXISTS weapons (
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

                CREATE TABLE IF NOT EXISTS weapon_parts (
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

                CREATE TABLE IF NOT EXISTS attachments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    data BLOB NOT NULL,
                    view TEXT DEFAULT 'OTHER'
                );

                CREATE TABLE IF NOT EXISTS item_values (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                    field TEXT NOT NULL,
                    value TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_detail TEXT,
                    confidence TEXT NOT NULL DEFAULT 'inferred',
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(item_serial, field, source)
                );

                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                );
                "#,
                )
                .map_err(|e| RepoError::Database(e.to_string()))?;

            self.mark_migration_applied("0001_base_schema")?;

            if tables_exist {
                println!("SQLite: Marked existing schema as migrated (0001_base_schema)");
            } else {
                println!("SQLite: Applied migration 0001_base_schema");
            }
        }

        // Migration 0002: Rename weapons -> items, weapon_parts -> item_parts
        if !self.is_migration_applied("0002_rename_tables")? {
            self.conn
                .execute_batch(
                    r#"
                    ALTER TABLE weapons RENAME TO items;
                    ALTER TABLE weapon_parts RENAME TO item_parts;
                    "#,
                )
                .map_err(|e| RepoError::Database(e.to_string()))?;

            self.mark_migration_applied("0002_rename_tables")?;
            println!("SQLite: Applied migration 0002_rename_tables");
        }

        // Create indexes AFTER all migrations (on new table names)
        self.conn
            .execute_batch(
                r#"
                CREATE INDEX IF NOT EXISTS idx_items_name ON items(name);
                CREATE INDEX IF NOT EXISTS idx_items_manufacturer ON items(manufacturer);
                CREATE INDEX IF NOT EXISTS idx_item_parts_item_serial ON item_parts(item_serial);
                CREATE INDEX IF NOT EXISTS idx_item_values_serial ON item_values(item_serial);
                CREATE INDEX IF NOT EXISTS idx_item_values_field ON item_values(item_serial, field);
                CREATE INDEX IF NOT EXISTS idx_attachments_item_serial ON attachments(item_serial);
                "#,
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(())
    }
}

impl ItemsRepository for SqliteDb {
    fn init(&self) -> RepoResult<()> {
        // Check if we need to migrate from old schema (legacy id-based PK)
        let needs_legacy_migration = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='weapons'
                 AND sql LIKE '%id INTEGER PRIMARY KEY%'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if needs_legacy_migration {
            self.migrate_to_serial_pk()?;
        }

        // Create schema_migrations table first
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version TEXT PRIMARY KEY NOT NULL,
                    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
                [],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        // Run incremental migrations
        self.run_migrations()?;

        Ok(())
    }

    fn add_item(&self, serial: &str) -> RepoResult<()> {
        self.conn
            .execute("INSERT INTO items (serial) VALUES (?1)", params![serial])
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_item(&self, serial: &str) -> RepoResult<Option<Item>> {
        let sql = format!(
            "SELECT {} FROM items WHERE serial = ?1",
            ITEM_SELECT_COLUMNS
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| RepoError::Database(e.to_string()))?;
        let item = stmt
            .query_row(params![serial], row_to_item)
            .optional()
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(item)
    }

    fn update_item(&self, serial: &str, update: &ItemUpdate) -> RepoResult<()> {
        self.conn
            .execute(
                r#"UPDATE items SET
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
                    update.name,
                    update.prefix,
                    update.manufacturer,
                    update.weapon_type,
                    update.rarity,
                    update.level,
                    update.element,
                    update.dps,
                    update.damage,
                    update.accuracy,
                    update.fire_rate,
                    update.reload_time,
                    update.mag_size,
                    update.value,
                    update.red_text,
                    update.notes
                ],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn list_items(&self, filter: &ItemFilter) -> RepoResult<Vec<Item>> {
        let (sql, _) = shared::build_list_query(filter, false);
        let params_vec = build_filter_params(filter);
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| RepoError::Database(e.to_string()))?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let items = stmt
            .query_map(params_refs.as_slice(), row_to_item)
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(items)
    }

    fn delete_item(&self, serial: &str) -> RepoResult<bool> {
        let rows = self
            .conn
            .execute("DELETE FROM items WHERE serial = ?1", params![serial])
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(rows > 0)
    }

    fn set_verification_status(
        &self,
        serial: &str,
        status: VerificationStatus,
        notes: Option<&str>,
    ) -> RepoResult<()> {
        self.conn
            .execute(
                r#"UPDATE items SET
                verification_status = ?2,
                verification_notes = COALESCE(?3, verification_notes),
                verified_at = CASE WHEN ?2 != 'unverified' THEN CURRENT_TIMESTAMP ELSE verified_at END
            WHERE serial = ?1"#,
                params![serial, status.to_string(), notes],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn set_legal(&self, serial: &str, legal: bool) -> RepoResult<()> {
        self.conn
            .execute(
                "UPDATE items SET legal = ?2 WHERE serial = ?1",
                params![serial, legal],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn set_all_legal(&self, legal: bool) -> RepoResult<usize> {
        let rows = self
            .conn
            .execute("UPDATE items SET legal = ?1", params![legal])
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(rows)
    }

    fn set_item_type(&self, serial: &str, item_type: &str) -> RepoResult<()> {
        self.conn
            .execute(
                "UPDATE items SET item_type = ?2 WHERE serial = ?1",
                params![serial, item_type],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn set_source(&self, serial: &str, source: &str) -> RepoResult<()> {
        self.conn
            .execute(
                "UPDATE items SET source = ?2 WHERE serial = ?1",
                params![serial, source],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn set_source_for_null(&self, source: &str) -> RepoResult<usize> {
        let rows = self
            .conn
            .execute(
                "UPDATE items SET source = ?1 WHERE source IS NULL",
                params![source],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(rows)
    }

    fn set_source_where(&self, source: &str, condition: &str) -> RepoResult<usize> {
        let sql = format!("UPDATE items SET source = ?1 WHERE {}", condition);
        let rows = self
            .conn
            .execute(&sql, params![source])
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(rows)
    }

    fn get_parts(&self, serial: &str) -> RepoResult<Vec<ItemPart>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_serial, slot, part_index, part_name, manufacturer, effect,
                    verified, verification_method, verification_notes, verified_at
             FROM item_parts WHERE item_serial = ?1",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let parts = stmt
            .query_map(params![serial], |row| {
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
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(parts)
    }

    fn set_value(
        &self,
        serial: &str,
        field: &str,
        value: &str,
        source: ValueSource,
        source_detail: Option<&str>,
        confidence: Confidence,
    ) -> RepoResult<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO item_values
               (item_serial, field, value, source, source_detail, confidence)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                params![
                    serial,
                    field,
                    value,
                    source.to_string(),
                    source_detail,
                    confidence.to_string()
                ],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_values(&self, serial: &str, field: &str) -> RepoResult<Vec<ItemValue>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
             FROM item_values
             WHERE item_serial = ?1 AND field = ?2
             ORDER BY source DESC, confidence DESC",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let values = stmt
            .query_map(params![serial, field], |row| {
                let source_str: String = row.get(4)?;
                let confidence_str: String = row.get(6)?;
                Ok(ItemValue {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    field: row.get(2)?,
                    value: row.get(3)?,
                    source: source_str.parse().unwrap_or(ValueSource::CommunityTool),
                    source_detail: row.get(5)?,
                    confidence: confidence_str.parse().unwrap_or(Confidence::Uncertain),
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(values)
    }

    fn get_best_value(&self, serial: &str, field: &str) -> RepoResult<Option<ItemValue>> {
        let values = self.get_values(serial, field)?;
        Ok(pick_best_value(values))
    }

    fn get_all_values(&self, serial: &str) -> RepoResult<Vec<ItemValue>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
             FROM item_values
             WHERE item_serial = ?1
             ORDER BY field, source DESC, confidence DESC",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let values = stmt
            .query_map(params![serial], |row| {
                let source_str: String = row.get(4)?;
                let confidence_str: String = row.get(6)?;
                Ok(ItemValue {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    field: row.get(2)?,
                    value: row.get(3)?,
                    source: source_str.parse().unwrap_or(ValueSource::CommunityTool),
                    source_detail: row.get(5)?,
                    confidence: confidence_str.parse().unwrap_or(Confidence::Uncertain),
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(values)
    }

    fn get_best_values(&self, serial: &str) -> RepoResult<HashMap<String, String>> {
        let all_values = self.get_all_values(serial)?;
        Ok(best_values_by_field(all_values))
    }

    fn get_all_items_best_values(&self) -> RepoResult<HashMap<String, HashMap<String, String>>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT item_serial, field, value, source, confidence
             FROM item_values
             ORDER BY item_serial, field, source DESC, confidence DESC",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let values: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let mut result: HashMap<String, HashMap<String, (String, u8, u8)>> = HashMap::new();

        for (serial, field, value, source_str, confidence_str) in values {
            let source: ValueSource = source_str.parse().unwrap_or(ValueSource::CommunityTool);
            let confidence: Confidence = confidence_str.parse().unwrap_or(Confidence::Uncertain);

            let entry = result.entry(serial).or_default();
            let current = entry.get(&field);

            let should_replace = current
                .map(
                    |(_, src_prio, conf_prio)| match source.priority().cmp(src_prio) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Equal => confidence.priority() > *conf_prio,
                        std::cmp::Ordering::Less => false,
                    },
                )
                .unwrap_or(true);

            if should_replace {
                entry.insert(field, (value, source.priority(), confidence.priority()));
            }
        }

        Ok(result
            .into_iter()
            .map(|(serial, fields)| {
                (
                    serial,
                    fields.into_iter().map(|(f, (v, _, _))| (f, v)).collect(),
                )
            })
            .collect())
    }

    fn stats(&self) -> RepoResult<DbStats> {
        let item_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let part_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM item_parts", [], |row| row.get(0))
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let attachment_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM attachments", [], |row| row.get(0))
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let value_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM item_values", [], |row| row.get(0))
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(DbStats {
            item_count,
            part_count,
            attachment_count,
            value_count,
        })
    }

    #[allow(clippy::too_many_lines)] // column-by-column data migration
    fn migrate_column_values(&self, dry_run: bool) -> RepoResult<MigrationStats> {
        let mut stats = MigrationStats::default();

        let mut stmt = self
            .conn
            .prepare(
                "SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity,
                    level, element, dps, damage, accuracy, fire_rate, reload_time,
                    mag_size, value, red_text
             FROM items",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let items: Vec<(String, Vec<Option<String>>)> = stmt
            .query_map([], |row| {
                let serial: String = row.get(0)?;
                let values: Vec<Option<String>> = (1..=16)
                    .map(|i| {
                        row.get::<_, Option<String>>(i)
                            .or_else(|_| {
                                row.get::<_, Option<i32>>(i)
                                    .map(|v| v.map(|n| n.to_string()))
                            })
                            .or_else(|_| {
                                row.get::<_, Option<f64>>(i)
                                    .map(|v| v.map(|n| n.to_string()))
                            })
                            .unwrap_or(None)
                    })
                    .collect();
                Ok((serial, values))
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        for (serial, values) in items {
            stats.items_processed += 1;

            for (i, (_, field_name)) in shared::FIELDS_TO_MIGRATE.iter().enumerate() {
                if let Some(value) = &values[i] {
                    if value.is_empty() {
                        continue;
                    }

                    let existing: Option<i64> = self
                        .conn
                        .query_row(
                            "SELECT 1 FROM item_values WHERE item_serial = ?1 AND field = ?2",
                            params![&serial, field_name],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| RepoError::Database(e.to_string()))?;

                    if existing.is_some() {
                        stats.values_skipped += 1;
                        continue;
                    }

                    if dry_run {
                        println!("Would migrate: {}.{} = {}", serial, field_name, value);
                    } else {
                        self.set_value(
                            &serial,
                            field_name,
                            value,
                            ValueSource::Decoder,
                            None,
                            Confidence::Inferred,
                        )?;
                    }
                    stats.values_migrated += 1;
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(feature = "attachments")]
impl AttachmentsRepository for SqliteDb {
    fn add_attachment(
        &self,
        serial: &str,
        name: &str,
        mime_type: &str,
        data: &[u8],
        view: &str,
    ) -> RepoResult<i64> {
        self.conn
            .execute(
                "INSERT INTO attachments (item_serial, name, mime_type, data, view) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![serial, name, mime_type, data, view],
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    fn get_attachments(&self, serial: &str) -> RepoResult<Vec<Attachment>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, item_serial, name, mime_type, COALESCE(view, 'OTHER') FROM attachments WHERE item_serial = ?1",
            )
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let attachments = stmt
            .query_map(params![serial], |row| {
                Ok(Attachment {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    name: row.get(2)?,
                    mime_type: row.get(3)?,
                    view: row.get(4)?,
                })
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(attachments)
    }

    fn get_attachment_data(&self, id: i64) -> RepoResult<Option<Vec<u8>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM attachments WHERE id = ?1")
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let data = stmt
            .query_row(params![id], |row| row.get(0))
            .optional()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(data)
    }

    fn delete_attachment(&self, id: i64) -> RepoResult<bool> {
        let rows = self
            .conn
            .execute("DELETE FROM attachments WHERE id = ?1", params![id])
            .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(rows > 0)
    }
}

// Bulk attachment methods (not part of trait - SqliteDb specific)
#[cfg(feature = "attachments")]
impl SqliteDb {
    /// Get all attachments for multiple serials (bulk fetch)
    pub fn get_attachments_bulk(&self, serials: &[&str]) -> RepoResult<Vec<Attachment>> {
        if serials.is_empty() {
            return Ok(vec![]);
        }

        // Build placeholders for IN clause
        let placeholders: Vec<String> = (1..=serials.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT id, item_serial, name, mime_type, COALESCE(view, 'OTHER') FROM attachments WHERE item_serial IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let attachments = stmt
            .query_map(rusqlite::params_from_iter(serials.iter()), |row| {
                Ok(Attachment {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    name: row.get(2)?,
                    mime_type: row.get(3)?,
                    view: row.get(4)?,
                })
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(attachments)
    }

    /// Get attachment data for multiple IDs (bulk fetch)
    pub fn get_attachment_data_bulk(&self, ids: &[i64]) -> RepoResult<Vec<(i64, Vec<u8>)>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT id, data FROM attachments WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let data = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(data)
    }
}

// Bulk values method (not part of trait - SqliteDb specific)
impl SqliteDb {
    /// Get all item_values for multiple serials (bulk fetch)
    pub fn get_all_values_bulk(&self, serials: &[&str]) -> RepoResult<Vec<ItemValue>> {
        if serials.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = (1..=serials.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
             FROM item_values WHERE item_serial IN ({})
             ORDER BY item_serial, field, source DESC, confidence DESC",
            placeholders.join(", ")
        );

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| RepoError::Database(e.to_string()))?;

        let values = stmt
            .query_map(rusqlite::params_from_iter(serials.iter()), |row| {
                Ok(ItemValue {
                    id: row.get(0)?,
                    item_serial: row.get(1)?,
                    field: row.get(2)?,
                    value: row.get(3)?,
                    source: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(ValueSource::Decoder),
                    source_detail: row.get(5)?,
                    confidence: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or(Confidence::Uncertain),
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| RepoError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(values)
    }
}

impl ImportExportRepository for SqliteDb {
    fn import_from_dir(&self, dir: &Path) -> RepoResult<String> {
        let serial_path = dir.join("serial.txt");
        let serial = std::fs::read_to_string(&serial_path)?.trim().to_string();

        if self.get_item(&serial)?.is_some() {
            println!("Item already exists: {}", serial);
            return Ok(serial);
        }

        self.add_item(&serial)?;

        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let parts: Vec<&str> = dir_name.split('_').collect();

        if parts.len() >= 2 {
            let update = ItemUpdate {
                manufacturer: Some(parts[0].to_string()),
                weapon_type: Some(parts[1].to_string()),
                name: if parts.len() > 2 {
                    Some(parts[2..].join("_").replace('_', " "))
                } else {
                    None
                },
                ..Default::default()
            };
            self.update_item(&serial, &update)?;
        }

        #[cfg(feature = "attachments")]
        {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map(|e| e == "png").unwrap_or(false) {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    let view = match name {
                        "inventory" | "stats" => "POPUP",
                        "inspect" => "DETAIL",
                        _ => "OTHER",
                    };
                    let data = std::fs::read(&path)?;
                    self.add_attachment(&serial, name, "image/png", &data, view)?;
                }
            }
        }

        Ok(serial)
    }

    fn export_to_dir(&self, serial: &str, dir: &Path) -> RepoResult<()> {
        std::fs::create_dir_all(dir)?;

        let item = self
            .get_item(serial)?
            .ok_or_else(|| RepoError::NotFound(serial.to_string()))?;

        std::fs::write(dir.join("serial.txt"), &item.serial)?;

        let metadata =
            serde_json::to_string_pretty(&item).map_err(|e| RepoError::Database(e.to_string()))?;
        std::fs::write(dir.join("metadata.json"), metadata)?;

        #[cfg(feature = "attachments")]
        {
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
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> SqliteDb {
        let db = SqliteDb::open_in_memory().unwrap();
        db.init().unwrap();
        db
    }

    #[test]
    fn test_init_creates_tables() {
        let db = setup_db();
        // Should be able to query the tables
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_add_and_get_item() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";

        db.add_item(serial).unwrap();

        let item = db.get_item(serial).unwrap().unwrap();
        assert_eq!(item.serial, serial);
    }

    #[test]
    fn test_add_duplicate_item_errors() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";

        db.add_item(serial).unwrap();
        // Adding again should error (UNIQUE constraint)
        let result = db.add_item(serial);
        assert!(result.is_err());

        // Should still only have one item
        let filter = ItemFilter::default();
        let items = db.list_items(&filter).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_update_item() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";
        db.add_item(serial).unwrap();

        let update = ItemUpdate {
            name: Some("Test Weapon".to_string()),
            level: Some(50),
            rarity: Some("legendary".to_string()),
            ..Default::default()
        };
        db.update_item(serial, &update).unwrap();

        let item = db.get_item(serial).unwrap().unwrap();
        assert_eq!(item.name, Some("Test Weapon".to_string()));
        assert_eq!(item.level, Some(50));
        assert_eq!(item.rarity, Some("legendary".to_string()));
    }

    #[test]
    fn test_set_and_get_value() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";
        db.add_item(serial).unwrap();

        db.set_value(
            serial,
            "name",
            "Test Name",
            ValueSource::Decoder,
            None,
            Confidence::Inferred,
        )
        .unwrap();

        let values = db.get_values(serial, "name").unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value, "Test Name");
        assert_eq!(values[0].source, ValueSource::Decoder);
        assert_eq!(values[0].confidence, Confidence::Inferred);
    }

    #[test]
    fn test_get_best_values() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";
        db.add_item(serial).unwrap();

        // Add community value first
        db.set_value(
            serial,
            "name",
            "Community Name",
            ValueSource::CommunityTool,
            Some("test-tool"),
            Confidence::Uncertain,
        )
        .unwrap();

        // Add decoder value
        db.set_value(
            serial,
            "name",
            "Decoder Name",
            ValueSource::Decoder,
            None,
            Confidence::Inferred,
        )
        .unwrap();

        let best = db.get_best_values(serial).unwrap();
        // Decoder should win over CommunityTool
        assert_eq!(best.get("name"), Some(&"Decoder Name".to_string()));
    }

    #[test]
    fn test_list_items_with_filter() {
        let db = setup_db();

        db.add_item("BL4(item1)").unwrap();
        db.add_item("BL4(item2)").unwrap();
        db.add_item("BL4(item3)").unwrap();

        db.update_item(
            "BL4(item1)",
            &ItemUpdate {
                rarity: Some("legendary".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        db.update_item(
            "BL4(item2)",
            &ItemUpdate {
                rarity: Some("epic".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        db.update_item(
            "BL4(item3)",
            &ItemUpdate {
                rarity: Some("legendary".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let filter = ItemFilter {
            rarity: Some("legendary".to_string()),
            ..Default::default()
        };
        let items = db.list_items(&filter).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_list_items_count() {
        let db = setup_db();

        db.add_item("BL4(item1)").unwrap();
        db.add_item("BL4(item2)").unwrap();
        db.add_item("BL4(item3)").unwrap();

        let items = db.list_items(&ItemFilter::default()).unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_stats() {
        let db = setup_db();

        db.add_item("BL4(item1)").unwrap();
        db.add_item("BL4(item2)").unwrap();

        db.set_value(
            "BL4(item1)",
            "name",
            "Test",
            ValueSource::Decoder,
            None,
            Confidence::Inferred,
        )
        .unwrap();

        let stats = db.stats().unwrap();
        assert_eq!(stats.item_count, 2);
        assert_eq!(stats.value_count, 1);
    }

    #[test]
    fn test_set_item_type() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";
        db.add_item(serial).unwrap();

        db.set_item_type(serial, "weapon").unwrap();

        let item = db.get_item(serial).unwrap().unwrap();
        assert_eq!(item.item_type, Some("weapon".to_string()));
    }

    #[test]
    fn test_set_source() {
        let db = setup_db();
        let serial = "BL4(AwAAAACo4A==)";
        db.add_item(serial).unwrap();

        db.set_source(serial, "community-pull").unwrap();

        let item = db.get_item(serial).unwrap().unwrap();
        assert_eq!(item.source, Some("community-pull".to_string()));
    }
}
