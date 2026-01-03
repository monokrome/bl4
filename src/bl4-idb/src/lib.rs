//! Items Database Library for Borderlands 4
//!
//! This library provides a trait-based abstraction for item database operations,
//! with implementations for SQLite (sync) and SQLx (async, SQLite + PostgreSQL).
//!
//! # Features
//!
//! - `sqlite-sync` (default) - Synchronous SQLite using rusqlite (for CLI)
//! - `sqlx-sqlite` - Async SQLite using SQLx (for server)
//! - `sqlx-postgres` - Async PostgreSQL using SQLx (for server)
//! - `attachments` - Enable screenshot/image attachment storage
//!
//! # Example (Sync)
//!
//! ```no_run
//! use bl4_idb::{SqliteDb, ItemsRepository, ItemFilter};
//!
//! let db = SqliteDb::open("items.db").unwrap();
//! db.init().unwrap();
//!
//! // List all items
//! let items = db.list_items(&ItemFilter::default()).unwrap();
//! ```
//!
//! # Example (Async with SQLx SQLite)
//!
//! ```ignore
//! // Requires feature "sqlx-sqlite"
//! use bl4_idb::{sqlx_impl::sqlite::SqlxSqliteDb, sqlx_impl::AsyncItemsRepository, ItemFilter};
//!
//! async fn example() {
//!     let db = SqlxSqliteDb::connect("sqlite:items.db").await.unwrap();
//!     db.init().await.unwrap();
//!
//!     // List all items
//!     let items = db.list_items(&ItemFilter::default()).await.unwrap();
//! }
//! ```

pub mod repository;
pub mod shared;
pub mod types;

#[cfg(feature = "sqlite-sync")]
pub mod sqlite;

#[cfg(any(feature = "sqlx-sqlite", feature = "sqlx-postgres"))]
pub mod sqlx_impl;

// Re-export types
pub use types::*;

// Re-export repository traits (sync)
pub use repository::{
    BulkRepository, BulkResult, BulkValueSet, ImportExportRepository, ItemsRepository, RepoError,
    RepoResult,
};

#[cfg(feature = "attachments")]
pub use repository::AttachmentsRepository;

// Re-export async repository traits
#[cfg(any(feature = "sqlx-sqlite", feature = "sqlx-postgres"))]
pub use sqlx_impl::{AsyncItemsRepository, AsyncRepoResult};

#[cfg(all(
    feature = "attachments",
    any(feature = "sqlx-sqlite", feature = "sqlx-postgres")
))]
pub use sqlx_impl::AsyncAttachmentsRepository;

#[cfg(any(feature = "sqlx-sqlite", feature = "sqlx-postgres"))]
pub use sqlx_impl::AsyncBulkRepository;
#[cfg(any(feature = "sqlx-sqlite", feature = "sqlx-postgres"))]
pub use sqlx_impl::BulkResult as AsyncBulkResult;

// Re-export implementations
#[cfg(feature = "sqlite-sync")]
pub use sqlite::{SqliteDb, DEFAULT_DB_PATH};

#[cfg(feature = "sqlx-sqlite")]
pub use sqlx_impl::sqlite::SqlxSqliteDb;

#[cfg(feature = "sqlx-postgres")]
pub use sqlx_impl::postgres::SqlxPgDb;
