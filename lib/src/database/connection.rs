use super::config::{Config, Driver};
use super::error::DatabaseError;
use super::migration::{self, Migration};
use super::sql::Dialect;
use crate::schema::{Schema, Value};

pub(crate) enum Pool {
    #[cfg(feature = "db-sqlite")]
    Sqlite(sqlx::sqlite::SqlitePool),
    #[cfg(feature = "db-postgres")]
    Postgres(sqlx::postgres::PgPool),
}

pub struct Database {
    pool: Pool,
    handle: tokio::runtime::Handle,
}

impl Database {
    pub async fn connect(config: &Config) -> Result<Database, DatabaseError> {
        let pool = match config.driver {
            Driver::Sqlite => connect_sqlite(&config.url).await?,
            Driver::Postgres => connect_postgres(&config.url).await?,
        };
        Ok(Database {
            pool,
            handle: tokio::runtime::Handle::current(),
        })
    }

    pub fn dialect(&self) -> Dialect {
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(_) => Dialect::Sqlite,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(_) => Dialect::Postgres,
        }
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
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(pool) => super::driver::sqlite::fetch(pool, &sql, args, target).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => super::driver::postgres::fetch(pool, &sql, args, target).await,
        }
    }

    pub(crate) async fn execute_values(
        &self,
        sql: &str,
        args: &[(Value, Schema)],
    ) -> Result<u64, DatabaseError> {
        let sql = self.dialect().placeholders(sql);
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(pool) => super::driver::sqlite::execute(pool, &sql, args).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => super::driver::postgres::execute(pool, &sql, args).await,
        }
    }

    pub(crate) async fn raw(&self, sql: &str) -> Result<(), DatabaseError> {
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(pool) => super::driver::sqlite::raw(pool, sql).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => super::driver::postgres::raw(pool, sql).await,
        }
    }

    pub(crate) async fn migrate_step(
        &self,
        migration_sql: &str,
        record_sql: &str,
        record_args: &[(Value, Schema)],
    ) -> Result<(), DatabaseError> {
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(pool) => {
                super::driver::sqlite::migrate_step(pool, migration_sql, record_sql, record_args)
                    .await
            }
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => {
                super::driver::postgres::migrate_step(pool, migration_sql, record_sql, record_args)
                    .await
            }
        }
    }

    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }
}

async fn connect_sqlite(url: &str) -> Result<Pool, DatabaseError> {
    #[cfg(feature = "db-sqlite")]
    {
        use std::str::FromStr as _;
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

        let options = SqliteConnectOptions::from_str(url)
            .map_err(DatabaseError::Connect)?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .foreign_keys(true);
        let memory = url.contains(":memory:");
        let options = if memory {
            options
        } else {
            options.journal_mode(SqliteJournalMode::Wal)
        };
        let pool = SqlitePoolOptions::new()
            .max_connections(if memory { 1 } else { 5 })
            .connect_with(options)
            .await
            .map_err(DatabaseError::Connect)?;
        Ok(Pool::Sqlite(pool))
    }
    #[cfg(not(feature = "db-sqlite"))]
    {
        let _ = url;
        Err(DatabaseError::Unavailable(
            "sqlite support is not compiled in (enable the db-sqlite feature)".into(),
        ))
    }
}

async fn connect_postgres(url: &str) -> Result<Pool, DatabaseError> {
    #[cfg(feature = "db-postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(DatabaseError::Connect)?;
        Ok(Pool::Postgres(pool))
    }
    #[cfg(not(feature = "db-postgres"))]
    {
        let _ = url;
        Err(DatabaseError::Unavailable(
            "postgres support is not compiled in (enable the db-postgres feature)".into(),
        ))
    }
}
