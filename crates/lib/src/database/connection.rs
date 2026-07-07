use super::backend::{self, Backend};
use super::config::Config;
use super::error::DatabaseError;
use super::migration::{self, Migration};
use super::sql::Dialect;
use crate::schema::{Schema, Value};

pub struct Database {
    backend: Box<dyn Backend>,
    handle: tokio::runtime::Handle,
}

impl Database {
    pub async fn connect(config: &Config) -> Result<Database, DatabaseError> {
        Ok(Database::with_backend(backend::connect(config).await?))
    }

    pub fn with_backend(backend: Box<dyn Backend>) -> Database {
        Database {
            backend,
            handle: tokio::runtime::Handle::current(),
        }
    }

    pub fn dialect(&self) -> Dialect {
        self.backend.dialect()
    }

    pub async fn migrate(&self, migrations: &[Migration]) -> Result<usize, DatabaseError> {
        migration::apply(self, migrations).await
    }

    pub(crate) async fn fetch_values(
        &self,
        sql: &str,
        args: &[(Value, Schema)],
        target: &Schema,
    ) -> Result<Vec<Value>, DatabaseError> {
        let sql = self.dialect().placeholders(sql);
        self.backend.fetch(&sql, args, target).await
    }

    pub(crate) async fn execute_values(
        &self,
        sql: &str,
        args: &[(Value, Schema)],
    ) -> Result<u64, DatabaseError> {
        let sql = self.dialect().placeholders(sql);
        self.backend.execute(&sql, args).await
    }

    pub(crate) async fn raw(&self, sql: &str) -> Result<(), DatabaseError> {
        self.backend.raw(sql).await
    }

    pub(crate) async fn migrate_step(
        &self,
        migration_sql: &str,
        record_sql: &str,
        record_args: &[(Value, Schema)],
    ) -> Result<(), DatabaseError> {
        self.backend
            .migrate_step(migration_sql, record_sql, record_args)
            .await
    }

    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }
}
