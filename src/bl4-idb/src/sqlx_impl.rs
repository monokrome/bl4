//! SQLx implementation for async database operations.
//!
//! Supports both SQLite and PostgreSQL via SQLx.

use crate::repository::RepoError;
use crate::types::*;
use sqlx::Row;
use std::collections::HashMap;

/// Result type for async repository operations
pub type AsyncRepoResult<T> = Result<T, RepoError>;

/// Async trait for items database operations
#[allow(async_fn_in_trait)]
pub trait AsyncItemsRepository {
    /// Initialize the database schema
    async fn init(&self) -> AsyncRepoResult<()>;

    // === Items CRUD ===

    /// Add a new item with just its serial
    async fn add_item(&self, serial: &str) -> AsyncRepoResult<()>;

    /// Get an item by serial
    async fn get_item(&self, serial: &str) -> AsyncRepoResult<Option<Item>>;

    /// Update item metadata
    async fn update_item(&self, serial: &str, update: &ItemUpdate) -> AsyncRepoResult<()>;

    /// List items with optional filters
    async fn list_items(&self, filter: &ItemFilter) -> AsyncRepoResult<Vec<Item>>;

    /// Delete an item
    async fn delete_item(&self, serial: &str) -> AsyncRepoResult<bool>;

    /// Count total items (optionally filtered)
    async fn count_items(&self, filter: &ItemFilter) -> AsyncRepoResult<i64>;

    // === Verification ===

    /// Set verification status for an item
    async fn set_verification_status(
        &self,
        serial: &str,
        status: VerificationStatus,
        notes: Option<&str>,
    ) -> AsyncRepoResult<()>;

    /// Set legal status for an item
    async fn set_legal(&self, serial: &str, legal: bool) -> AsyncRepoResult<()>;

    // === Metadata ===

    /// Set item type
    async fn set_item_type(&self, serial: &str, item_type: &str) -> AsyncRepoResult<()>;

    /// Set source for an item
    async fn set_source(&self, serial: &str, source: &str) -> AsyncRepoResult<()>;

    // === Multi-source values ===

    /// Set a field value with source attribution
    #[allow(clippy::too_many_arguments)] // Trait method with distinct semantic params
    async fn set_value(
        &self,
        serial: &str,
        field: &str,
        value: &str,
        source: ValueSource,
        source_detail: Option<&str>,
        confidence: Confidence,
    ) -> AsyncRepoResult<()>;

    /// Get all values for a field across sources
    async fn get_values(&self, serial: &str, field: &str) -> AsyncRepoResult<Vec<ItemValue>>;

    /// Get the best value for a field
    async fn get_best_value(&self, serial: &str, field: &str)
        -> AsyncRepoResult<Option<ItemValue>>;

    /// Get all values for an item
    async fn get_all_values(&self, serial: &str) -> AsyncRepoResult<Vec<ItemValue>>;

    /// Get best value for each field as a map
    async fn get_best_values(&self, serial: &str) -> AsyncRepoResult<HashMap<String, String>>;

    // === Statistics ===

    /// Get database statistics
    async fn stats(&self) -> AsyncRepoResult<DbStats>;
}

/// Async attachment operations (feature-gated)
#[cfg(feature = "attachments")]
#[allow(async_fn_in_trait)]
pub trait AsyncAttachmentsRepository {
    /// Add an image attachment
    #[allow(clippy::too_many_arguments)] // Trait method with distinct semantic params
    async fn add_attachment(
        &self,
        serial: &str,
        name: &str,
        mime_type: &str,
        data: &[u8],
        view: &str,
    ) -> AsyncRepoResult<i64>;

    /// Get attachments for an item (without data)
    async fn get_attachments(&self, serial: &str) -> AsyncRepoResult<Vec<Attachment>>;

    /// Get attachment data by ID
    async fn get_attachment_data(&self, id: i64) -> AsyncRepoResult<Option<Vec<u8>>>;

    /// Delete an attachment
    async fn delete_attachment(&self, id: i64) -> AsyncRepoResult<bool>;
}

/// Async bulk operations
#[allow(async_fn_in_trait)]
pub trait AsyncBulkRepository {
    /// Add multiple items at once
    async fn add_items_bulk(&self, serials: &[&str]) -> AsyncRepoResult<BulkResult>;
}

/// Bulk operation result
#[derive(Debug, Clone, Default)]
pub struct BulkResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<(String, String)>,
}

// =============================================================================
// SQLite implementation
// =============================================================================

#[cfg(feature = "sqlx-sqlite")]
pub mod sqlite {
    use super::*;
    use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};

    /// SQLite-backed async items database
    pub struct SqlxSqliteDb {
        pool: SqlitePool,
    }

    impl SqlxSqliteDb {
        /// Connect to a SQLite database
        pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
            let pool = SqlitePoolOptions::new()
                .max_connections(5)
                .connect(url)
                .await?;
            Ok(Self { pool })
        }

        /// Connect with an existing pool
        pub fn with_pool(pool: SqlitePool) -> Self {
            Self { pool }
        }

        /// Get the connection pool
        pub fn pool(&self) -> &SqlitePool {
            &self.pool
        }

        fn row_to_item(row: SqliteRow) -> Result<Item, sqlx::Error> {
            use sqlx::Row;
            let status_str: Option<String> = row.try_get("verification_status")?;
            Ok(Item {
                serial: row.try_get("serial")?,
                name: row.try_get("name")?,
                prefix: row.try_get("prefix")?,
                manufacturer: row.try_get("manufacturer")?,
                weapon_type: row.try_get("weapon_type")?,
                item_type: row.try_get("item_type")?,
                rarity: row.try_get("rarity")?,
                level: row.try_get("level")?,
                element: row.try_get("element")?,
                dps: row.try_get("dps")?,
                damage: row.try_get("damage")?,
                accuracy: row.try_get("accuracy")?,
                fire_rate: row.try_get("fire_rate")?,
                reload_time: row.try_get("reload_time")?,
                mag_size: row.try_get("mag_size")?,
                value: row.try_get("value")?,
                red_text: row.try_get("red_text")?,
                notes: row.try_get("notes")?,
                verification_status: status_str
                    .unwrap_or_else(|| "unverified".to_string())
                    .parse()
                    .unwrap_or(VerificationStatus::Unverified),
                verification_notes: row.try_get("verification_notes")?,
                verified_at: row.try_get("verified_at")?,
                legal: row.try_get::<Option<bool>, _>("legal")?.unwrap_or(false),
                source: row.try_get("source")?,
                created_at: row
                    .try_get::<Option<String>, _>("created_at")?
                    .unwrap_or_default(),
            })
        }

        fn row_to_item_value(row: SqliteRow) -> Result<ItemValue, sqlx::Error> {
            use sqlx::Row;
            let source_str: String = row.try_get("source")?;
            let confidence_str: String = row.try_get("confidence")?;
            Ok(ItemValue {
                id: row.try_get("id")?,
                item_serial: row.try_get("item_serial")?,
                field: row.try_get("field")?,
                value: row.try_get("value")?,
                source: source_str.parse().unwrap_or(ValueSource::CommunityTool),
                source_detail: row.try_get("source_detail")?,
                confidence: confidence_str.parse().unwrap_or(Confidence::Uncertain),
                created_at: row
                    .try_get::<Option<String>, _>("created_at")?
                    .unwrap_or_default(),
            })
        }

        /// Get a setting value by key
        pub async fn get_setting(&self, key: &str) -> AsyncRepoResult<Option<String>> {
            let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(row.map(|(v,)| v))
        }

        /// Set a setting value
        pub async fn set_setting(&self, key: &str, value: &str) -> AsyncRepoResult<()> {
            sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
                .bind(key)
                .bind(value)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        /// Get the source salt, generating one if it doesn't exist
        pub async fn get_or_create_salt(&self) -> AsyncRepoResult<String> {
            if let Some(salt) = self.get_setting("source_salt").await? {
                Ok(salt)
            } else {
                let salt = crate::generate_salt();
                self.set_setting("source_salt", &salt).await?;
                Ok(salt)
            }
        }

        /// Get all distinct sources from the database
        pub async fn get_distinct_sources(&self) -> AsyncRepoResult<Vec<String>> {
            let rows: Vec<(String,)> =
                sqlx::query_as("SELECT DISTINCT source FROM items WHERE source IS NOT NULL")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(rows.into_iter().map(|(s,)| s).collect())
        }
    }

    impl AsyncItemsRepository for SqlxSqliteDb {
        #[allow(clippy::too_many_lines)] // SQL schema definition
        async fn init(&self) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS items (
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
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS item_parts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
                    slot TEXT NOT NULL,
                    part_index INTEGER,
                    part_name TEXT,
                    manufacturer TEXT,
                    effect TEXT,
                    verified BOOLEAN DEFAULT FALSE,
                    verification_method TEXT,
                    verification_notes TEXT,
                    verified_at TIMESTAMP
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS attachments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    data BLOB NOT NULL,
                    view TEXT DEFAULT 'OTHER'
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS item_values (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_serial TEXT NOT NULL REFERENCES items(serial) ON DELETE CASCADE,
                    field TEXT NOT NULL,
                    value TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_detail TEXT,
                    confidence TEXT NOT NULL DEFAULT 'inferred',
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(item_serial, field, source)
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Settings table for storing salt and other config
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Create indexes (use new table names - base schema uses old names but migrations rename)
            for sql in [
                "CREATE INDEX IF NOT EXISTS idx_items_name ON items(name)",
                "CREATE INDEX IF NOT EXISTS idx_items_manufacturer ON items(manufacturer)",
                "CREATE INDEX IF NOT EXISTS idx_item_parts_item_serial ON item_parts(item_serial)",
                "CREATE INDEX IF NOT EXISTS idx_item_values_serial ON item_values(item_serial)",
                "CREATE INDEX IF NOT EXISTS idx_item_values_field ON item_values(item_serial, field)",
                "CREATE INDEX IF NOT EXISTS idx_attachments_item_serial ON attachments(item_serial)",
            ] {
                sqlx::query(sql)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            }

            Ok(())
        }

        async fn add_item(&self, serial: &str) -> AsyncRepoResult<()> {
            sqlx::query("INSERT INTO items (serial) VALUES (?)")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn get_item(&self, serial: &str) -> AsyncRepoResult<Option<Item>> {
            let row = sqlx::query(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM items WHERE serial = ?"#,
            )
            .bind(serial)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            match row {
                Some(r) => Ok(Some(
                    Self::row_to_item(r).map_err(|e| RepoError::Database(e.to_string()))?,
                )),
                None => Ok(None),
            }
        }

        async fn update_item(&self, serial: &str, update: &ItemUpdate) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"UPDATE items SET
                    name = COALESCE(?, name),
                    prefix = COALESCE(?, prefix),
                    manufacturer = COALESCE(?, manufacturer),
                    weapon_type = COALESCE(?, weapon_type),
                    rarity = COALESCE(?, rarity),
                    level = COALESCE(?, level),
                    element = COALESCE(?, element),
                    dps = COALESCE(?, dps),
                    damage = COALESCE(?, damage),
                    accuracy = COALESCE(?, accuracy),
                    fire_rate = COALESCE(?, fire_rate),
                    reload_time = COALESCE(?, reload_time),
                    mag_size = COALESCE(?, mag_size),
                    value = COALESCE(?, value),
                    red_text = COALESCE(?, red_text),
                    notes = COALESCE(?, notes)
                WHERE serial = ?"#,
            )
            .bind(&update.name)
            .bind(&update.prefix)
            .bind(&update.manufacturer)
            .bind(&update.weapon_type)
            .bind(&update.rarity)
            .bind(update.level)
            .bind(&update.element)
            .bind(update.dps)
            .bind(update.damage)
            .bind(update.accuracy)
            .bind(update.fire_rate)
            .bind(update.reload_time)
            .bind(update.mag_size)
            .bind(update.value)
            .bind(&update.red_text)
            .bind(&update.notes)
            .bind(serial)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        #[allow(clippy::too_many_lines)] // Item struct has 24 fields to map
        async fn list_items(&self, filter: &ItemFilter) -> AsyncRepoResult<Vec<Item>> {
            let mut sql = String::from(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM items WHERE 1=1"#,
            );

            // Build dynamic query - SQLx doesn't support truly dynamic queries well,
            // so we'll build a string query
            if filter.manufacturer.is_some() {
                sql.push_str(" AND manufacturer = ?");
            }
            if filter.weapon_type.is_some() {
                sql.push_str(" AND weapon_type = ?");
            }
            if filter.element.is_some() {
                sql.push_str(" AND element = ?");
            }
            if filter.rarity.is_some() {
                sql.push_str(" AND rarity = ?");
            }

            sql.push_str(" ORDER BY created_at DESC");

            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }
            if let Some(offset) = filter.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }

            let sql: &'static str = Box::leak(sql.into_boxed_str());
            let mut query = sqlx::query(sql);

            if let Some(m) = &filter.manufacturer {
                query = query.bind(m);
            }
            if let Some(w) = &filter.weapon_type {
                query = query.bind(w);
            }
            if let Some(e) = &filter.element {
                query = query.bind(e);
            }
            if let Some(r) = &filter.rarity {
                query = query.bind(r);
            }

            let rows = query
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn delete_item(&self, serial: &str) -> AsyncRepoResult<bool> {
            let result = sqlx::query("DELETE FROM items WHERE serial = ?")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }

        async fn count_items(&self, filter: &ItemFilter) -> AsyncRepoResult<i64> {
            let mut sql = String::from("SELECT COUNT(*) as count FROM items WHERE 1=1");

            if filter.manufacturer.is_some() {
                sql.push_str(" AND manufacturer = ?");
            }
            if filter.weapon_type.is_some() {
                sql.push_str(" AND weapon_type = ?");
            }
            if filter.element.is_some() {
                sql.push_str(" AND element = ?");
            }
            if filter.rarity.is_some() {
                sql.push_str(" AND rarity = ?");
            }

            let sql: &'static str = Box::leak(sql.into_boxed_str());
            let mut query = sqlx::query(sql);

            if let Some(m) = &filter.manufacturer {
                query = query.bind(m);
            }
            if let Some(w) = &filter.weapon_type {
                query = query.bind(w);
            }
            if let Some(e) = &filter.element {
                query = query.bind(e);
            }
            if let Some(r) = &filter.rarity {
                query = query.bind(r);
            }

            let row = query
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let count: i64 = row
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(count)
        }

        async fn set_verification_status(
            &self,
            serial: &str,
            status: VerificationStatus,
            notes: Option<&str>,
        ) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"UPDATE items SET
                    verification_status = ?,
                    verification_notes = COALESCE(?, verification_notes),
                    verified_at = CASE WHEN ? != 'unverified' THEN CURRENT_TIMESTAMP ELSE verified_at END
                WHERE serial = ?"#,
            )
            .bind(status.to_string())
            .bind(notes)
            .bind(status.to_string())
            .bind(serial)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_legal(&self, serial: &str, legal: bool) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET legal = ? WHERE serial = ?")
                .bind(legal)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_item_type(&self, serial: &str, item_type: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET item_type = ? WHERE serial = ?")
                .bind(item_type)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_source(&self, serial: &str, source: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET source = ? WHERE serial = ?")
                .bind(source)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_value(
            &self,
            serial: &str,
            field: &str,
            value: &str,
            source: ValueSource,
            source_detail: Option<&str>,
            confidence: Confidence,
        ) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"INSERT OR REPLACE INTO item_values
                   (item_serial, field, value, source, source_detail, confidence)
                   VALUES (?, ?, ?, ?, ?, ?)"#,
            )
            .bind(serial)
            .bind(field)
            .bind(value)
            .bind(source.to_string())
            .bind(source_detail)
            .bind(confidence.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn get_values(&self, serial: &str, field: &str) -> AsyncRepoResult<Vec<ItemValue>> {
            let rows = sqlx::query(
                r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
                   FROM item_values
                   WHERE item_serial = ? AND field = ?
                   ORDER BY source DESC, confidence DESC"#,
            )
            .bind(serial)
            .bind(field)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item_value(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn get_best_value(
            &self,
            serial: &str,
            field: &str,
        ) -> AsyncRepoResult<Option<ItemValue>> {
            let values = self.get_values(serial, field).await?;
            Ok(pick_best_value(values))
        }

        async fn get_all_values(&self, serial: &str) -> AsyncRepoResult<Vec<ItemValue>> {
            let rows = sqlx::query(
                r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
                   FROM item_values
                   WHERE item_serial = ?
                   ORDER BY field, source DESC, confidence DESC"#,
            )
            .bind(serial)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item_value(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn get_best_values(&self, serial: &str) -> AsyncRepoResult<HashMap<String, String>> {
            let all_values = self.get_all_values(serial).await?;
            Ok(best_values_by_field(all_values))
        }

        async fn stats(&self) -> AsyncRepoResult<DbStats> {
            let item_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM items")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let part_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM item_parts")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let attachment_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM attachments")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let value_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM item_values")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(DbStats {
                item_count,
                part_count,
                attachment_count,
                value_count,
            })
        }
    }

    #[cfg(feature = "attachments")]
    impl AsyncAttachmentsRepository for SqlxSqliteDb {
        async fn add_attachment(
            &self,
            serial: &str,
            name: &str,
            mime_type: &str,
            data: &[u8],
            view: &str,
        ) -> AsyncRepoResult<i64> {
            let result = sqlx::query(
                "INSERT INTO attachments (item_serial, name, mime_type, data, view) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(serial)
            .bind(name)
            .bind(mime_type)
            .bind(data)
            .bind(view)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(result.last_insert_rowid())
        }

        async fn get_attachments(&self, serial: &str) -> AsyncRepoResult<Vec<Attachment>> {
            let rows = sqlx::query(
                "SELECT id, item_serial, name, mime_type, COALESCE(view, 'OTHER') as view FROM attachments WHERE item_serial = ?",
            )
            .bind(serial)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|row| {
                    use sqlx::Row;
                    Ok(Attachment {
                        id: row
                            .try_get("id")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        item_serial: row
                            .try_get("item_serial")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        name: row
                            .try_get("name")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        mime_type: row
                            .try_get("mime_type")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        view: row
                            .try_get("view")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                    })
                })
                .collect()
        }

        async fn get_attachment_data(&self, id: i64) -> AsyncRepoResult<Option<Vec<u8>>> {
            let row = sqlx::query("SELECT data FROM attachments WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

            match row {
                Some(r) => {
                    use sqlx::Row;
                    let data: Vec<u8> = r
                        .try_get("data")
                        .map_err(|e| RepoError::Database(e.to_string()))?;
                    Ok(Some(data))
                }
                None => Ok(None),
            }
        }

        async fn delete_attachment(&self, id: i64) -> AsyncRepoResult<bool> {
            let result = sqlx::query("DELETE FROM attachments WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }
    }

    impl AsyncBulkRepository for SqlxSqliteDb {
        async fn add_items_bulk(&self, serials: &[&str]) -> AsyncRepoResult<BulkResult> {
            let mut result = BulkResult::default();

            for serial in serials {
                match self.add_item(serial).await {
                    Ok(_) => result.succeeded += 1,
                    Err(e) => {
                        result.failed += 1;
                        result.errors.push((serial.to_string(), e.to_string()));
                    }
                }
            }

            Ok(result)
        }
    }
}

// =============================================================================
// PostgreSQL implementation
// =============================================================================

#[cfg(feature = "sqlx-postgres")]
pub mod postgres {
    use super::*;
    use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};

    /// PostgreSQL-backed async items database
    pub struct SqlxPgDb {
        pool: PgPool,
    }

    impl SqlxPgDb {
        /// Connect to a PostgreSQL database
        pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
            let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;
            Ok(Self { pool })
        }

        /// Connect with an existing pool
        pub fn with_pool(pool: PgPool) -> Self {
            Self { pool }
        }

        /// Get the connection pool
        pub fn pool(&self) -> &PgPool {
            &self.pool
        }

        fn row_to_item(row: PgRow) -> Result<Item, sqlx::Error> {
            use sqlx::Row;
            let status_str: Option<String> = row.try_get("verification_status")?;
            Ok(Item {
                serial: row.try_get("serial")?,
                name: row.try_get("name")?,
                prefix: row.try_get("prefix")?,
                manufacturer: row.try_get("manufacturer")?,
                weapon_type: row.try_get("weapon_type")?,
                item_type: row.try_get("item_type")?,
                rarity: row.try_get("rarity")?,
                level: row.try_get("level")?,
                element: row.try_get("element")?,
                dps: row.try_get("dps")?,
                damage: row.try_get("damage")?,
                accuracy: row.try_get("accuracy")?,
                fire_rate: row.try_get("fire_rate")?,
                reload_time: row.try_get("reload_time")?,
                mag_size: row.try_get("mag_size")?,
                value: row.try_get("value")?,
                red_text: row.try_get("red_text")?,
                notes: row.try_get("notes")?,
                verification_status: status_str
                    .unwrap_or_else(|| "unverified".to_string())
                    .parse()
                    .unwrap_or(VerificationStatus::Unverified),
                verification_notes: row.try_get("verification_notes")?,
                verified_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("verified_at")?
                    .map(|dt| dt.to_rfc3339()),
                legal: row.try_get::<Option<bool>, _>("legal")?.unwrap_or(false),
                source: row.try_get("source")?,
                created_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            })
        }

        fn row_to_item_value(row: PgRow) -> Result<ItemValue, sqlx::Error> {
            use sqlx::Row;
            let source_str: String = row.try_get("source")?;
            let confidence_str: String = row.try_get("confidence")?;
            Ok(ItemValue {
                id: row.try_get("id")?,
                item_serial: row.try_get("item_serial")?,
                field: row.try_get("field")?,
                value: row.try_get("value")?,
                source: source_str.parse().unwrap_or(ValueSource::CommunityTool),
                source_detail: row.try_get("source_detail")?,
                confidence: confidence_str.parse().unwrap_or(Confidence::Uncertain),
                created_at: row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")?
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            })
        }

        /// Get a setting value by key
        pub async fn get_setting(&self, key: &str) -> AsyncRepoResult<Option<String>> {
            let row: Option<(String,)> =
                sqlx::query_as("SELECT value FROM settings WHERE key = $1")
                    .bind(key)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(row.map(|(v,)| v))
        }

        /// Set a setting value
        pub async fn set_setting(&self, key: &str, value: &str) -> AsyncRepoResult<()> {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
            )
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        /// Get the source salt, generating one if it doesn't exist
        pub async fn get_or_create_salt(&self) -> AsyncRepoResult<String> {
            if let Some(salt) = self.get_setting("source_salt").await? {
                Ok(salt)
            } else {
                let salt = crate::generate_salt();
                self.set_setting("source_salt", &salt).await?;
                Ok(salt)
            }
        }

        /// Get all distinct sources from the database
        pub async fn get_distinct_sources(&self) -> AsyncRepoResult<Vec<String>> {
            let rows: Vec<(String,)> =
                sqlx::query_as("SELECT DISTINCT source FROM items WHERE source IS NOT NULL")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(rows.into_iter().map(|(s,)| s).collect())
        }

        /// Run pending database migrations
        #[allow(clippy::too_many_lines)] // SQL migration definitions
        async fn run_migrations(&self) -> AsyncRepoResult<()> {
            // Create migrations tracking table
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS _migrations (
                    id TEXT PRIMARY KEY NOT NULL,
                    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Define migrations (id, sql) - each must be a single statement
            let migrations: &[(&str, &str)] = &[
                (
                    "0001_attachments_bigserial",
                    "ALTER TABLE attachments ALTER COLUMN id TYPE BIGINT",
                ),
                (
                    "0002_attachments_unique",
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_unique ON attachments(item_serial, name, view)",
                ),
                (
                    "0003_drop_old_unique_index",
                    "DROP INDEX IF EXISTS idx_attachments_unique",
                ),
                (
                    "0004_attachments_unique_null_safe",
                    "CREATE UNIQUE INDEX idx_attachments_unique ON attachments(item_serial, name, view) WHERE view IS NOT NULL",
                ),
                (
                    "0005_attachment_blobs_table",
                    r#"CREATE TABLE IF NOT EXISTS attachment_blobs (
                        hash TEXT PRIMARY KEY,
                        data BYTEA NOT NULL,
                        mime_type TEXT NOT NULL,
                        created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
                    )"#,
                ),
                (
                    "0006_attachments_blob_hash",
                    "ALTER TABLE attachments ADD COLUMN IF NOT EXISTS blob_hash TEXT REFERENCES attachment_blobs(hash)",
                ),
                (
                    "0007_weapons_created_at_tz",
                    "ALTER TABLE weapons ALTER COLUMN created_at TYPE TIMESTAMPTZ",
                ),
                (
                    "0008_weapons_verified_at_tz",
                    "ALTER TABLE weapons ALTER COLUMN verified_at TYPE TIMESTAMPTZ",
                ),
                (
                    "0009_item_values_created_at_tz",
                    "ALTER TABLE item_values ALTER COLUMN created_at TYPE TIMESTAMPTZ",
                ),
                (
                    "0010_weapon_parts_verified_at_tz",
                    "ALTER TABLE weapon_parts ALTER COLUMN verified_at TYPE TIMESTAMPTZ",
                ),
                // Rename tables from weapon-centric to generic item names
                (
                    "0011_rename_weapons_to_items",
                    "ALTER TABLE weapons RENAME TO items",
                ),
                (
                    "0012_rename_weapon_parts_to_item_parts",
                    "ALTER TABLE weapon_parts RENAME TO item_parts",
                ),
                // Make data column nullable since we now use blob_hash for content-addressed storage
                (
                    "0013_attachments_data_nullable",
                    "ALTER TABLE attachments ALTER COLUMN data DROP NOT NULL",
                ),
            ];

            for (id, sql) in migrations {
                // Check if already applied
                let applied: Option<(String,)> =
                    sqlx::query_as("SELECT id FROM _migrations WHERE id = $1")
                        .bind(id)
                        .fetch_optional(&self.pool)
                        .await
                        .map_err(|e| RepoError::Database(e.to_string()))?;

                if applied.is_some() {
                    continue;
                }

                // Run migration
                sqlx::query(*sql)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(format!("Migration {} failed: {}", id, e)))?;

                // Mark as applied
                sqlx::query("INSERT INTO _migrations (id) VALUES ($1)")
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            }

            Ok(())
        }

        /// Bulk update sources for multiple items (single query)
        pub async fn set_sources_bulk(
            &self,
            items: &[(&str, &str)], // (serial, source)
        ) -> AsyncRepoResult<()> {
            if items.is_empty() {
                return Ok(());
            }
            let serials: Vec<String> = items.iter().map(|(s, _)| s.to_string()).collect();
            let sources: Vec<String> = items.iter().map(|(_, src)| src.to_string()).collect();

            sqlx::query(
                r#"
                UPDATE items SET source = data.source
                FROM (SELECT UNNEST($1::text[]) as serial, UNNEST($2::text[]) as source) as data
                WHERE items.serial = data.serial
                "#,
            )
            .bind(&serials)
            .bind(&sources)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(())
        }

        /// Bulk update item_types for multiple items (single query)
        pub async fn set_item_types_bulk(
            &self,
            items: &[(&str, &str)], // (serial, item_type)
        ) -> AsyncRepoResult<()> {
            if items.is_empty() {
                return Ok(());
            }
            let serials: Vec<String> = items.iter().map(|(s, _)| s.to_string()).collect();
            let types: Vec<String> = items.iter().map(|(_, t)| t.to_string()).collect();

            sqlx::query(
                r#"
                UPDATE items SET item_type = data.item_type
                FROM (SELECT UNNEST($1::text[]) as serial, UNNEST($2::text[]) as item_type) as data
                WHERE items.serial = data.serial
                "#,
            )
            .bind(&serials)
            .bind(&types)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(())
        }

        /// Bulk insert item values (single query)
        pub async fn set_values_bulk(
            &self,
            values: &[(&str, &str, &str, &str, &str)], // (serial, field, value, source, confidence)
        ) -> AsyncRepoResult<()> {
            if values.is_empty() {
                return Ok(());
            }
            let serials: Vec<String> = values.iter().map(|(s, _, _, _, _)| s.to_string()).collect();
            let fields: Vec<String> = values.iter().map(|(_, f, _, _, _)| f.to_string()).collect();
            let vals: Vec<String> = values.iter().map(|(_, _, v, _, _)| v.to_string()).collect();
            let sources: Vec<String> = values.iter().map(|(_, _, _, s, _)| s.to_string()).collect();
            let confidences: Vec<String> =
                values.iter().map(|(_, _, _, _, c)| c.to_string()).collect();

            sqlx::query(
                r#"
                INSERT INTO item_values (item_serial, field, value, source, confidence)
                SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[])
                ON CONFLICT (item_serial, field, source) DO UPDATE SET
                    value = EXCLUDED.value,
                    confidence = EXCLUDED.confidence
                "#,
            )
            .bind(&serials)
            .bind(&fields)
            .bind(&vals)
            .bind(&sources)
            .bind(&confidences)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(())
        }
    }

    impl AsyncItemsRepository for SqlxPgDb {
        #[allow(clippy::too_many_lines)] // SQL schema definition
        async fn init(&self) -> AsyncRepoResult<()> {
            // PostgreSQL uses SERIAL instead of AUTOINCREMENT, and slightly different syntax
            sqlx::query(
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
                    fire_rate DOUBLE PRECISION,
                    reload_time DOUBLE PRECISION,
                    mag_size INTEGER,
                    value INTEGER,
                    red_text TEXT,
                    notes TEXT,
                    verification_status TEXT DEFAULT 'unverified',
                    verification_notes TEXT,
                    verified_at TIMESTAMPTZ,
                    legal BOOLEAN DEFAULT FALSE,
                    source TEXT,
                    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS weapon_parts (
                    id SERIAL PRIMARY KEY,
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                    slot TEXT NOT NULL,
                    part_index INTEGER,
                    part_name TEXT,
                    manufacturer TEXT,
                    effect TEXT,
                    verified BOOLEAN DEFAULT FALSE,
                    verification_method TEXT,
                    verification_notes TEXT,
                    verified_at TIMESTAMPTZ
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS attachments (
                    id BIGSERIAL PRIMARY KEY,
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    data BYTEA NOT NULL,
                    view TEXT DEFAULT 'OTHER'
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS item_values (
                    id SERIAL PRIMARY KEY,
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
                    field TEXT NOT NULL,
                    value TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_detail TEXT,
                    confidence TEXT NOT NULL DEFAULT 'inferred',
                    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(item_serial, field, source)
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Settings table for storing salt and other config
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Run migrations FIRST (includes table renames)
            self.run_migrations().await?;

            // Create indexes AFTER migrations (use new table names)
            for sql in [
                "CREATE INDEX IF NOT EXISTS idx_items_name ON items(name)",
                "CREATE INDEX IF NOT EXISTS idx_items_manufacturer ON items(manufacturer)",
                "CREATE INDEX IF NOT EXISTS idx_item_parts_item_serial ON item_parts(item_serial)",
                "CREATE INDEX IF NOT EXISTS idx_item_values_serial ON item_values(item_serial)",
                "CREATE INDEX IF NOT EXISTS idx_item_values_field ON item_values(item_serial, field)",
                "CREATE INDEX IF NOT EXISTS idx_attachments_item_serial ON attachments(item_serial)",
            ] {
                sqlx::query(sql)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| RepoError::Database(e.to_string()))?;
            }

            Ok(())
        }

        async fn add_item(&self, serial: &str) -> AsyncRepoResult<()> {
            sqlx::query("INSERT INTO items (serial) VALUES ($1)")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn get_item(&self, serial: &str) -> AsyncRepoResult<Option<Item>> {
            let row = sqlx::query(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM items WHERE serial = $1"#,
            )
            .bind(serial)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            match row {
                Some(r) => Ok(Some(
                    Self::row_to_item(r).map_err(|e| RepoError::Database(e.to_string()))?,
                )),
                None => Ok(None),
            }
        }

        async fn update_item(&self, serial: &str, update: &ItemUpdate) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"UPDATE items SET
                    name = COALESCE($1, name),
                    prefix = COALESCE($2, prefix),
                    manufacturer = COALESCE($3, manufacturer),
                    weapon_type = COALESCE($4, weapon_type),
                    rarity = COALESCE($5, rarity),
                    level = COALESCE($6, level),
                    element = COALESCE($7, element),
                    dps = COALESCE($8, dps),
                    damage = COALESCE($9, damage),
                    accuracy = COALESCE($10, accuracy),
                    fire_rate = COALESCE($11, fire_rate),
                    reload_time = COALESCE($12, reload_time),
                    mag_size = COALESCE($13, mag_size),
                    value = COALESCE($14, value),
                    red_text = COALESCE($15, red_text),
                    notes = COALESCE($16, notes)
                WHERE serial = $17"#,
            )
            .bind(&update.name)
            .bind(&update.prefix)
            .bind(&update.manufacturer)
            .bind(&update.weapon_type)
            .bind(&update.rarity)
            .bind(update.level)
            .bind(&update.element)
            .bind(update.dps)
            .bind(update.damage)
            .bind(update.accuracy)
            .bind(update.fire_rate)
            .bind(update.reload_time)
            .bind(update.mag_size)
            .bind(update.value)
            .bind(&update.red_text)
            .bind(&update.notes)
            .bind(serial)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        #[allow(clippy::too_many_lines)] // Item struct has 24 fields to map
        async fn list_items(&self, filter: &ItemFilter) -> AsyncRepoResult<Vec<Item>> {
            let mut sql = String::from(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM items WHERE 1=1"#,
            );

            let mut param_idx = 1;

            if filter.manufacturer.is_some() {
                sql.push_str(&format!(" AND manufacturer = ${}", param_idx));
                param_idx += 1;
            }
            if filter.weapon_type.is_some() {
                sql.push_str(&format!(" AND weapon_type = ${}", param_idx));
                param_idx += 1;
            }
            if filter.element.is_some() {
                sql.push_str(&format!(" AND element = ${}", param_idx));
                param_idx += 1;
            }
            if filter.rarity.is_some() {
                sql.push_str(&format!(" AND rarity = ${}", param_idx));
            }

            sql.push_str(" ORDER BY created_at DESC");

            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }
            if let Some(offset) = filter.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }

            let sql: &'static str = Box::leak(sql.into_boxed_str());
            let mut query = sqlx::query(sql);

            if let Some(m) = &filter.manufacturer {
                query = query.bind(m);
            }
            if let Some(w) = &filter.weapon_type {
                query = query.bind(w);
            }
            if let Some(e) = &filter.element {
                query = query.bind(e);
            }
            if let Some(r) = &filter.rarity {
                query = query.bind(r);
            }

            let rows = query
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn delete_item(&self, serial: &str) -> AsyncRepoResult<bool> {
            let result = sqlx::query("DELETE FROM items WHERE serial = $1")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }

        async fn count_items(&self, filter: &ItemFilter) -> AsyncRepoResult<i64> {
            let mut sql = String::from("SELECT COUNT(*) as count FROM items WHERE 1=1");
            let mut param_idx = 1;

            if filter.manufacturer.is_some() {
                sql.push_str(&format!(" AND manufacturer = ${}", param_idx));
                param_idx += 1;
            }
            if filter.weapon_type.is_some() {
                sql.push_str(&format!(" AND weapon_type = ${}", param_idx));
                param_idx += 1;
            }
            if filter.element.is_some() {
                sql.push_str(&format!(" AND element = ${}", param_idx));
                param_idx += 1;
            }
            if filter.rarity.is_some() {
                sql.push_str(&format!(" AND rarity = ${}", param_idx));
            }

            let sql: &'static str = Box::leak(sql.into_boxed_str());
            let mut query = sqlx::query(sql);

            if let Some(m) = &filter.manufacturer {
                query = query.bind(m);
            }
            if let Some(w) = &filter.weapon_type {
                query = query.bind(w);
            }
            if let Some(e) = &filter.element {
                query = query.bind(e);
            }
            if let Some(r) = &filter.rarity {
                query = query.bind(r);
            }

            let row = query
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let count: i64 = row
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(count)
        }

        async fn set_verification_status(
            &self,
            serial: &str,
            status: VerificationStatus,
            notes: Option<&str>,
        ) -> AsyncRepoResult<()> {
            sqlx::query(
                r#"UPDATE items SET
                    verification_status = $1,
                    verification_notes = COALESCE($2, verification_notes),
                    verified_at = CASE WHEN $1 != 'unverified' THEN CURRENT_TIMESTAMP ELSE verified_at END
                WHERE serial = $3"#,
            )
            .bind(status.to_string())
            .bind(notes)
            .bind(serial)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_legal(&self, serial: &str, legal: bool) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET legal = $1 WHERE serial = $2")
                .bind(legal)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_item_type(&self, serial: &str, item_type: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET item_type = $1 WHERE serial = $2")
                .bind(item_type)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_source(&self, serial: &str, source: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE items SET source = $1 WHERE serial = $2")
                .bind(source)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_value(
            &self,
            serial: &str,
            field: &str,
            value: &str,
            source: ValueSource,
            source_detail: Option<&str>,
            confidence: Confidence,
        ) -> AsyncRepoResult<()> {
            // PostgreSQL uses ON CONFLICT ... DO UPDATE for upsert
            sqlx::query(
                r#"INSERT INTO item_values
                   (item_serial, field, value, source, source_detail, confidence)
                   VALUES ($1, $2, $3, $4, $5, $6)
                   ON CONFLICT (item_serial, field, source) DO UPDATE SET
                   value = EXCLUDED.value,
                   source_detail = EXCLUDED.source_detail,
                   confidence = EXCLUDED.confidence"#,
            )
            .bind(serial)
            .bind(field)
            .bind(value)
            .bind(source.to_string())
            .bind(source_detail)
            .bind(confidence.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn get_values(&self, serial: &str, field: &str) -> AsyncRepoResult<Vec<ItemValue>> {
            let rows = sqlx::query(
                r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
                   FROM item_values
                   WHERE item_serial = $1 AND field = $2
                   ORDER BY source DESC, confidence DESC"#,
            )
            .bind(serial)
            .bind(field)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item_value(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn get_best_value(
            &self,
            serial: &str,
            field: &str,
        ) -> AsyncRepoResult<Option<ItemValue>> {
            let values = self.get_values(serial, field).await?;
            Ok(pick_best_value(values))
        }

        async fn get_all_values(&self, serial: &str) -> AsyncRepoResult<Vec<ItemValue>> {
            let rows = sqlx::query(
                r#"SELECT id, item_serial, field, value, source, source_detail, confidence, created_at
                   FROM item_values
                   WHERE item_serial = $1
                   ORDER BY field, source DESC, confidence DESC"#,
            )
            .bind(serial)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|r| Self::row_to_item_value(r).map_err(|e| RepoError::Database(e.to_string())))
                .collect()
        }

        async fn get_best_values(&self, serial: &str) -> AsyncRepoResult<HashMap<String, String>> {
            let all_values = self.get_all_values(serial).await?;
            Ok(best_values_by_field(all_values))
        }

        async fn stats(&self) -> AsyncRepoResult<DbStats> {
            let item_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM items")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let part_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM item_parts")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let attachment_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM attachments")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let value_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM item_values")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            Ok(DbStats {
                item_count,
                part_count,
                attachment_count,
                value_count,
            })
        }
    }

    #[cfg(feature = "attachments")]
    impl AsyncAttachmentsRepository for SqlxPgDb {
        #[allow(clippy::too_many_lines)] // conditional logic for content-addressed storage
        async fn add_attachment(
            &self,
            serial: &str,
            name: &str,
            mime_type: &str,
            data: &[u8],
            view: &str,
        ) -> AsyncRepoResult<i64> {
            use sha2::{Digest, Sha256};

            // Compute content hash for deduplication
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash = hex::encode(hasher.finalize());

            // Insert blob if not exists (content-addressed)
            sqlx::query(
                r#"
                INSERT INTO attachment_blobs (hash, data, mime_type)
                VALUES ($1, $2, $3)
                ON CONFLICT (hash) DO NOTHING
                "#,
            )
            .bind(&hash)
            .bind(data)
            .bind(mime_type)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Insert attachment record, handling partial index
            // View is non-NULL: upsert. View is NULL equivalent (empty): just insert.
            let view_val = if view.is_empty() { None } else { Some(view) };

            let row = if view_val.is_some() {
                // Non-NULL view: use ON CONFLICT with partial index (WHERE view IS NOT NULL)
                sqlx::query(
                    r#"
                    INSERT INTO attachments (item_serial, name, mime_type, blob_hash, view)
                    VALUES ($1, $2, $3, $4, $5)
                    ON CONFLICT (item_serial, name, view) WHERE view IS NOT NULL DO UPDATE SET
                        mime_type = EXCLUDED.mime_type,
                        blob_hash = EXCLUDED.blob_hash
                    RETURNING id
                    "#,
                )
                .bind(serial)
                .bind(name)
                .bind(mime_type)
                .bind(&hash)
                .bind(view_val)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
            } else {
                // NULL view: just insert (no uniqueness constraint)
                sqlx::query(
                    r#"
                    INSERT INTO attachments (item_serial, name, mime_type, blob_hash, view)
                    VALUES ($1, $2, $3, $4, NULL)
                    RETURNING id
                    "#,
                )
                .bind(serial)
                .bind(name)
                .bind(mime_type)
                .bind(&hash)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
            };

            use sqlx::Row;
            let id: i64 = row
                .try_get("id")
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(id)
        }

        async fn get_attachments(&self, serial: &str) -> AsyncRepoResult<Vec<Attachment>> {
            let rows = sqlx::query(
                "SELECT id, item_serial, name, mime_type, COALESCE(view, 'OTHER') as view FROM attachments WHERE item_serial = $1",
            )
            .bind(serial)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            rows.into_iter()
                .map(|row| {
                    use sqlx::Row;
                    Ok(Attachment {
                        id: row
                            .try_get("id")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        item_serial: row
                            .try_get("item_serial")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        name: row
                            .try_get("name")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        mime_type: row
                            .try_get("mime_type")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                        view: row
                            .try_get("view")
                            .map_err(|e| RepoError::Database(e.to_string()))?,
                    })
                })
                .collect()
        }

        async fn get_attachment_data(&self, id: i64) -> AsyncRepoResult<Option<Vec<u8>>> {
            // Try content-addressed storage first (blob_hash), fallback to legacy data column
            let row = sqlx::query(
                r#"
                SELECT COALESCE(b.data, a.data) as data
                FROM attachments a
                LEFT JOIN attachment_blobs b ON a.blob_hash = b.hash
                WHERE a.id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            match row {
                Some(r) => {
                    use sqlx::Row;
                    let data: Vec<u8> = r
                        .try_get("data")
                        .map_err(|e| RepoError::Database(e.to_string()))?;
                    Ok(Some(data))
                }
                None => Ok(None),
            }
        }

        async fn delete_attachment(&self, id: i64) -> AsyncRepoResult<bool> {
            let result = sqlx::query("DELETE FROM attachments WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }
    }

    impl AsyncBulkRepository for SqlxPgDb {
        async fn add_items_bulk(&self, serials: &[&str]) -> AsyncRepoResult<BulkResult> {
            if serials.is_empty() {
                return Ok(BulkResult::default());
            }

            // Use UNNEST for true bulk insert with ON CONFLICT DO NOTHING
            let serials_vec: Vec<String> = serials.iter().map(|s| s.to_string()).collect();

            let rows_affected = sqlx::query(
                "INSERT INTO items (serial) SELECT * FROM UNNEST($1::text[]) ON CONFLICT DO NOTHING",
            )
            .bind(&serials_vec)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?
            .rows_affected();

            Ok(BulkResult {
                succeeded: rows_affected as usize,
                failed: serials.len() - rows_affected as usize,
                errors: vec![], // Can't determine which ones failed with bulk insert
            })
        }
    }
}

#[cfg(all(test, feature = "sqlx-postgres"))]
mod tests {
    use super::postgres::SqlxPgDb;
    use super::AsyncItemsRepository;

    /// Test PostgreSQL migrations run successfully
    /// Run with: cargo test -p bl4-idb --features sqlx-postgres test_postgres_migrations -- --ignored
    #[tokio::test]
    #[ignore] // Requires Docker
    async fn test_postgres_migrations() {
        use testcontainers::runners::AsyncRunner;
        use testcontainers_modules::postgres::Postgres;

        // Start PostgreSQL container
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();

        let url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

        // Connect and run init (which runs migrations)
        let db = SqlxPgDb::connect(&url).await.expect("Failed to connect");
        db.init().await.expect("Failed to run migrations");

        // Verify we can do basic operations
        db.add_item("@UgTestSerial123")
            .await
            .expect("Failed to add item");
        let item = db
            .get_item("@UgTestSerial123")
            .await
            .expect("Failed to get item");
        assert!(item.is_some());
        assert_eq!(item.unwrap().serial, "@UgTestSerial123");
    }
}
