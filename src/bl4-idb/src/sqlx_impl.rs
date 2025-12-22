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
    }

    impl AsyncItemsRepository for SqlxSqliteDb {
        async fn init(&self) -> AsyncRepoResult<()> {
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
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
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
                    item_serial TEXT NOT NULL REFERENCES weapons(serial) ON DELETE CASCADE,
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

            // Create indexes
            for sql in [
                "CREATE INDEX IF NOT EXISTS idx_weapons_name ON weapons(name)",
                "CREATE INDEX IF NOT EXISTS idx_weapons_manufacturer ON weapons(manufacturer)",
                "CREATE INDEX IF NOT EXISTS idx_weapon_parts_item_serial ON weapon_parts(item_serial)",
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
            sqlx::query("INSERT INTO weapons (serial) VALUES (?)")
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
                   FROM weapons WHERE serial = ?"#,
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
                r#"UPDATE weapons SET
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

        async fn list_items(&self, filter: &ItemFilter) -> AsyncRepoResult<Vec<Item>> {
            let mut sql = String::from(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM weapons WHERE 1=1"#,
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
            let result = sqlx::query("DELETE FROM weapons WHERE serial = ?")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }

        async fn count_items(&self, filter: &ItemFilter) -> AsyncRepoResult<i64> {
            let mut sql = String::from("SELECT COUNT(*) as count FROM weapons WHERE 1=1");

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
                r#"UPDATE weapons SET
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
            sqlx::query("UPDATE weapons SET legal = ? WHERE serial = ?")
                .bind(legal)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_item_type(&self, serial: &str, item_type: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE weapons SET item_type = ? WHERE serial = ?")
                .bind(item_type)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_source(&self, serial: &str, source: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE weapons SET source = ? WHERE serial = ?")
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
            let item_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM weapons")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let part_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM weapon_parts")
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
                verified_at: row.try_get("verified_at")?,
                legal: row.try_get::<Option<bool>, _>("legal")?.unwrap_or(false),
                source: row.try_get("source")?,
                created_at: row
                    .try_get::<Option<String>, _>("created_at")?
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
                    .try_get::<Option<String>, _>("created_at")?
                    .unwrap_or_default(),
            })
        }
    }

    impl AsyncItemsRepository for SqlxPgDb {
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
                    id SERIAL PRIMARY KEY,
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
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(item_serial, field, source)
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

            // Create indexes (IF NOT EXISTS for PostgreSQL)
            for sql in [
                "CREATE INDEX IF NOT EXISTS idx_weapons_name ON weapons(name)",
                "CREATE INDEX IF NOT EXISTS idx_weapons_manufacturer ON weapons(manufacturer)",
                "CREATE INDEX IF NOT EXISTS idx_weapon_parts_item_serial ON weapon_parts(item_serial)",
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
            sqlx::query("INSERT INTO weapons (serial) VALUES ($1)")
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
                   FROM weapons WHERE serial = $1"#,
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
                r#"UPDATE weapons SET
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

        async fn list_items(&self, filter: &ItemFilter) -> AsyncRepoResult<Vec<Item>> {
            let mut sql = String::from(
                r#"SELECT serial, name, prefix, manufacturer, weapon_type, item_type, rarity, level, element,
                        dps, damage, accuracy, fire_rate, reload_time, mag_size, value, red_text,
                        notes, verification_status, verification_notes, verified_at, legal, source, created_at
                   FROM weapons WHERE 1=1"#,
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

            let mut query = sqlx::query(&sql);

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
            let result = sqlx::query("DELETE FROM weapons WHERE serial = $1")
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(result.rows_affected() > 0)
        }

        async fn count_items(&self, filter: &ItemFilter) -> AsyncRepoResult<i64> {
            let mut sql = String::from("SELECT COUNT(*) as count FROM weapons WHERE 1=1");
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

            let mut query = sqlx::query(&sql);

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
                r#"UPDATE weapons SET
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
            sqlx::query("UPDATE weapons SET legal = $1 WHERE serial = $2")
                .bind(legal)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_item_type(&self, serial: &str, item_type: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE weapons SET item_type = $1 WHERE serial = $2")
                .bind(item_type)
                .bind(serial)
                .execute(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?;
            Ok(())
        }

        async fn set_source(&self, serial: &str, source: &str) -> AsyncRepoResult<()> {
            sqlx::query("UPDATE weapons SET source = $1 WHERE serial = $2")
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
            let item_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM weapons")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepoError::Database(e.to_string()))?
                .try_get("count")
                .map_err(|e| RepoError::Database(e.to_string()))?;

            let part_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM weapon_parts")
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
        async fn add_attachment(
            &self,
            serial: &str,
            name: &str,
            mime_type: &str,
            data: &[u8],
            view: &str,
        ) -> AsyncRepoResult<i64> {
            let row = sqlx::query(
                "INSERT INTO attachments (item_serial, name, mime_type, data, view) VALUES ($1, $2, $3, $4, $5) RETURNING id",
            )
            .bind(serial)
            .bind(name)
            .bind(mime_type)
            .bind(data)
            .bind(view)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

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
            let row = sqlx::query("SELECT data FROM attachments WHERE id = $1")
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
