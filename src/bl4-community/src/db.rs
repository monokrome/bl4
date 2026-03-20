use bl4_idb::{
    AsyncAttachmentsRepository, AsyncBulkRepository, AsyncItemsRepository, Confidence, ItemFilter,
    SqlxPgDb, SqlxSqliteDb, ValueSource,
};

macro_rules! dispatch {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            Database::Sqlite(db) => db.$method($($arg),*).await,
            Database::Postgres(db) => db.$method($($arg),*).await,
        }
    };
}

pub enum Database {
    Sqlite(SqlxSqliteDb),
    Postgres(SqlxPgDb),
}

impl Database {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Ok(Database::Postgres(SqlxPgDb::connect(url).await?))
        } else {
            Ok(Database::Sqlite(SqlxSqliteDb::connect(url).await?))
        }
    }

    pub async fn init(&self) -> Result<(), bl4_idb::RepoError> {
        dispatch!(self, init)
    }

    pub async fn stats(&self) -> Result<bl4_idb::DbStats, bl4_idb::RepoError> {
        dispatch!(self, stats)
    }

    pub async fn list_items(
        &self,
        filter: &ItemFilter,
    ) -> Result<Vec<bl4_idb::Item>, bl4_idb::RepoError> {
        dispatch!(self, list_items, filter)
    }

    pub async fn count_items(&self, filter: &ItemFilter) -> Result<i64, bl4_idb::RepoError> {
        dispatch!(self, count_items, filter)
    }

    pub async fn get_item(
        &self,
        serial: &str,
    ) -> Result<Option<bl4_idb::Item>, bl4_idb::RepoError> {
        dispatch!(self, get_item, serial)
    }

    pub async fn add_item(&self, serial: &str) -> Result<(), bl4_idb::RepoError> {
        dispatch!(self, add_item, serial)
    }

    pub async fn add_items_bulk(
        &self,
        serials: &[&str],
    ) -> Result<bl4_idb::AsyncBulkResult, bl4_idb::RepoError> {
        dispatch!(self, add_items_bulk, serials)
    }

    pub async fn set_sources_bulk(&self, items: &[(&str, &str)]) -> Result<(), bl4_idb::RepoError> {
        match self {
            Database::Sqlite(_) => Ok(()),
            Database::Postgres(db) => db.set_sources_bulk(items).await,
        }
    }

    pub async fn set_item_types_bulk(
        &self,
        items: &[(&str, &str)],
    ) -> Result<(), bl4_idb::RepoError> {
        match self {
            Database::Sqlite(_) => Ok(()),
            Database::Postgres(db) => db.set_item_types_bulk(items).await,
        }
    }

    pub async fn set_values_bulk(
        &self,
        values: &[(&str, &str, &str, &str, &str)],
    ) -> Result<(), bl4_idb::RepoError> {
        match self {
            Database::Sqlite(_) => Ok(()),
            Database::Postgres(db) => db.set_values_bulk(values).await,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn set_value(
        &self,
        serial: &str,
        field: &str,
        value: &str,
        source: ValueSource,
        source_detail: Option<&str>,
        confidence: Confidence,
    ) -> Result<(), bl4_idb::RepoError> {
        dispatch!(
            self,
            set_value,
            serial,
            field,
            value,
            source,
            source_detail,
            confidence
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_attachment(
        &self,
        serial: &str,
        name: &str,
        mime_type: &str,
        data: &[u8],
        view: &str,
    ) -> Result<i64, bl4_idb::RepoError> {
        dispatch!(self, add_attachment, serial, name, mime_type, data, view)
    }

    pub async fn set_source(&self, serial: &str, source: &str) -> Result<(), bl4_idb::RepoError> {
        dispatch!(self, set_source, serial, source)
    }

    pub async fn set_item_type(
        &self,
        serial: &str,
        item_type: &str,
    ) -> Result<(), bl4_idb::RepoError> {
        dispatch!(self, set_item_type, serial, item_type)
    }
}
