/* sqlx-backed storage behind loop's own Value/Schema types: endpoints talk
to `database::query(...)` (or generated table methods) and never see sqlx.
Queries are written with `?` placeholders in a portable dialect; see
`Dialect` for what diverges per backend. */

#[cfg(not(any(feature = "db-sqlite", feature = "db-postgres")))]
compile_error!(
    "the `database` feature needs a driver: enable `db-sqlite` and/or `db-postgres`"
);

mod bridge;
mod dialect;
mod error;
mod migration;

pub use dialect::Dialect;
pub use error::DatabaseError;
pub use migration::{Migration, checksum};

use std::sync::OnceLock;

use crate::schema::{FromValue, IntoValue, Schema, Value};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Driver {
    Sqlite,
    Postgres,
}

/// Where and how to connect. The driver is inferred from the URL scheme:
/// `postgres://...` / `postgresql://...` is postgres, anything else (a
/// `sqlite:` URL, a bare file path, or `:memory:`) is sqlite.
#[derive(Debug, Clone)]
pub struct Config {
    pub url: String,
    pub driver: Driver,
}

impl Config {
    pub fn from_url(url: impl Into<String>) -> Config {
        let url = url.into();
        let driver = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Driver::Postgres
        } else {
            Driver::Sqlite
        };
        Config { url, driver }
    }

    /// Reads `LOOP_DB_URL`, set by `loop dev` from the project's `loop.toml`.
    pub fn from_env() -> Option<Config> {
        std::env::var("LOOP_DB_URL").ok().map(Config::from_url)
    }
}

enum Pool {
    #[cfg(feature = "db-sqlite")]
    Sqlite(sqlx::sqlite::SqlitePool),
    #[cfg(feature = "db-postgres")]
    Postgres(sqlx::postgres::PgPool),
}

pub struct Database {
    pool: Pool,
    /// Captured at connect time so the synchronous query API can drive sqlx
    /// futures from handler (blocking) threads.
    handle: tokio::runtime::Handle,
}

impl Database {
    pub async fn connect(config: &Config) -> Result<Database, DatabaseError> {
        let pool = match config.driver {
            Driver::Sqlite => {
                #[cfg(feature = "db-sqlite")]
                {
                    use std::str::FromStr as _;
                    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

                    let options = SqliteConnectOptions::from_str(&config.url)
                        .map_err(DatabaseError::Connect)?
                        .create_if_missing(true)
                        .busy_timeout(std::time::Duration::from_secs(5))
                        .foreign_keys(true);
                    let memory = config.url.contains(":memory:");
                    let options = if memory {
                        options
                    } else {
                        options.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    };
                    // a :memory: database exists per connection, so the pool
                    // must not open a second one
                    let pool = SqlitePoolOptions::new()
                        .max_connections(if memory { 1 } else { 5 })
                        .connect_with(options)
                        .await
                        .map_err(DatabaseError::Connect)?;
                    Pool::Sqlite(pool)
                }
                #[cfg(not(feature = "db-sqlite"))]
                {
                    return Err(DatabaseError::Unavailable(
                        "sqlite support is not compiled in (enable the db-sqlite feature)".into(),
                    ));
                }
            }
            Driver::Postgres => {
                #[cfg(feature = "db-postgres")]
                {
                    let pool = sqlx::postgres::PgPoolOptions::new()
                        .max_connections(5)
                        .connect(&config.url)
                        .await
                        .map_err(DatabaseError::Connect)?;
                    Pool::Postgres(pool)
                }
                #[cfg(not(feature = "db-postgres"))]
                {
                    return Err(DatabaseError::Unavailable(
                        "postgres support is not compiled in (enable the db-postgres feature)"
                            .into(),
                    ));
                }
            }
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

    /// Starts a query written with `?` placeholders.
    pub fn query(&self, sql: impl Into<String>) -> Query<'_> {
        Query {
            db: self,
            sql: sql.into(),
            args: Vec::new(),
        }
    }

    /// Applies pending migrations in version order; returns how many ran.
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
            Pool::Sqlite(pool) => bridge::sqlite::fetch(pool, &sql, args, target).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => bridge::postgres::fetch(pool, &sql, args, target).await,
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
            Pool::Sqlite(pool) => bridge::sqlite::execute(pool, &sql, args).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => bridge::postgres::execute(pool, &sql, args).await,
        }
    }

    /// Executes raw (possibly multi-statement) SQL as-is: no placeholder or
    /// dialect translation.
    pub(crate) async fn raw(&self, sql: &str) -> Result<(), DatabaseError> {
        match &self.pool {
            #[cfg(feature = "db-sqlite")]
            Pool::Sqlite(pool) => bridge::sqlite::raw(pool, sql).await,
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => bridge::postgres::raw(pool, sql).await,
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
                bridge::sqlite::migrate_step(pool, migration_sql, record_sql, record_args).await
            }
            #[cfg(feature = "db-postgres")]
            Pool::Postgres(pool) => {
                bridge::postgres::migrate_step(pool, migration_sql, record_sql, record_args).await
            }
        }
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }
}

/// A query in flight. `bind` captures each argument's schema alongside its
/// value so nulls stay typed on strictly-typed backends. The `fetch_*` /
/// `execute` methods are blocking (for use inside handlers, which run on
/// blocking threads); the `*_async` twins serve async contexts.
pub struct Query<'db> {
    db: &'db Database,
    sql: String,
    args: Vec<(Value, Schema)>,
}

impl Query<'_> {
    pub fn bind<T: IntoValue>(mut self, value: T) -> Self {
        self.args.push((value.into_value(), T::schema()));
        self
    }

    pub fn fetch_all<T: FromValue>(self) -> Result<Vec<T>, DatabaseError> {
        self.db.block_on(self.fetch_all_async())
    }

    pub fn fetch_optional<T: FromValue>(self) -> Result<Option<T>, DatabaseError> {
        self.db.block_on(self.fetch_optional_async())
    }

    pub fn fetch_one<T: FromValue>(self) -> Result<T, DatabaseError> {
        self.db.block_on(self.fetch_one_async())
    }

    pub fn execute(self) -> Result<u64, DatabaseError> {
        self.db.block_on(self.execute_async())
    }

    pub async fn fetch_all_async<T: FromValue>(&self) -> Result<Vec<T>, DatabaseError> {
        let rows = self
            .db
            .fetch_values(&self.sql, &self.args, &T::schema())
            .await?;
        rows.into_iter()
            .map(|row| T::from_value(row).map_err(|e| DatabaseError::Decode(e.to_string())))
            .collect()
    }

    pub async fn fetch_optional_async<T: FromValue>(&self) -> Result<Option<T>, DatabaseError> {
        Ok(self.fetch_all_async().await?.into_iter().next())
    }

    pub async fn fetch_one_async<T: FromValue>(&self) -> Result<T, DatabaseError> {
        self.fetch_optional_async()
            .await?
            .ok_or_else(|| DatabaseError::Decode("expected one row, query returned none".into()))
    }

    pub async fn execute_async(&self) -> Result<u64, DatabaseError> {
        self.db.execute_values(&self.sql, &self.args).await
    }
}

static DATABASE: OnceLock<Database> = OnceLock::new();

/// Connects, applies migrations, and installs the process-wide database that
/// `get()`/`query()` serve.
pub async fn init(
    config: &Config,
    migrations: &[Migration],
) -> Result<&'static Database, DatabaseError> {
    let db = Database::connect(config).await?;
    db.migrate(migrations).await?;
    DATABASE
        .set(db)
        .map_err(|_| DatabaseError::Unavailable("database already initialized".into()))?;
    Ok(DATABASE.get().expect("just set"))
}

pub fn try_get() -> Option<&'static Database> {
    DATABASE.get()
}

/// The process-wide database. Panics when nothing connected at startup —
/// set `LOOP_DB_URL` (or `[database]` in loop.toml, which `loop dev` passes
/// through) so the server initializes it.
pub fn get() -> &'static Database {
    try_get().expect(
        "database not initialized: set LOOP_DB_URL or add [database] to loop.toml \
         so the server connects at startup",
    )
}

/// Starts a query on the process-wide database.
pub fn query(sql: impl Into<String>) -> Query<'static> {
    get().query(sql)
}

/// Registers a project's migrations for `server::run` to apply at startup;
/// submitted by the `database!` macro via inventory.
#[cfg(feature = "macros")]
pub struct Migrations(pub fn() -> Vec<Migration>);

#[cfg(feature = "macros")]
inventory::collect!(Migrations);

#[cfg(feature = "macros")]
pub fn registered_migrations() -> Vec<Migration> {
    let mut migrations: Vec<Migration> = inventory::iter::<Migrations>
        .into_iter()
        .flat_map(|set| (set.0)())
        .collect();
    migrations.sort_by_key(|m| m.version);
    migrations
}

/// Connects and migrates from the environment when configured; `Ok(None)`
/// means no database was requested.
#[cfg(feature = "macros")]
pub async fn init_from_env() -> Result<Option<&'static Database>, DatabaseError> {
    match Config::from_env() {
        Some(config) => init(&config, &registered_migrations()).await.map(Some),
        None => Ok(None),
    }
}

#[cfg(all(test, feature = "db-sqlite"))]
mod tests {
    use super::*;
    use crate::endpoint::HandlerError;
    use crate::schema::{AsSchema, Primitive};

    #[derive(Debug, PartialEq)]
    struct User {
        id: i64,
        name: String,
        nickname: Option<String>,
    }

    impl AsSchema for User {
        fn schema() -> Schema {
            Schema::Record(vec![
                ("id".into(), Schema::Primitive(Primitive::I64)),
                ("name".into(), Schema::Primitive(Primitive::Str)),
                ("nickname".into(), Option::<String>::schema()),
            ])
        }
    }

    impl FromValue for User {
        fn from_value(value: Value) -> Result<Self, HandlerError> {
            let Value::Map(entries) = value else {
                return Err("expected record".into());
            };
            let [(_, id), (_, name), (_, nickname)] = entries.as_slice() else {
                return Err("bad row shape".into());
            };
            Ok(User {
                id: i64::from_value(id.clone())?,
                name: String::from_value(name.clone())?,
                nickname: Option::<String>::from_value(nickname.clone())?,
            })
        }
    }

    fn create_users() -> Migration {
        Migration::new(
            1,
            "create_users",
            "CREATE TABLE users (
                id BIGSERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                nickname TEXT
            )",
        )
    }

    async fn fresh() -> Database {
        let db = Database::connect(&Config::from_url("sqlite::memory:"))
            .await
            .unwrap();
        assert_eq!(db.migrate(&[create_users()]).await.unwrap(), 1);
        db
    }

    async fn seed(db: &Database) {
        for (name, nickname) in [("ada", Some("countess")), ("alan", None)] {
            let inserted = db
                .query("INSERT INTO users (name, nickname) VALUES (?, ?)")
                .bind(name.to_string())
                .bind(nickname.map(str::to_string))
                .execute_async()
                .await
                .unwrap();
            assert_eq!(inserted, 1);
        }
    }

    #[tokio::test]
    async fn migrates_inserts_and_fetches_records_with_nulls() {
        let db = fresh().await;
        seed(&db).await;

        let users: Vec<User> = db
            .query("SELECT id, name, nickname FROM users ORDER BY id")
            .fetch_all_async()
            .await
            .unwrap();
        assert_eq!(
            users,
            vec![
                User {
                    id: 1,
                    name: "ada".into(),
                    nickname: Some("countess".into())
                },
                User {
                    id: 2,
                    name: "alan".into(),
                    nickname: None
                },
            ]
        );
    }

    #[tokio::test]
    async fn placeholders_bind_and_scalars_fetch() {
        let db = fresh().await;
        seed(&db).await;

        let count: i64 = db
            .query("SELECT count(*) FROM users WHERE nickname IS NOT NULL")
            .fetch_one_async()
            .await
            .unwrap();
        assert_eq!(count, 1);

        let name: Option<String> = db
            .query("SELECT name FROM users WHERE name = ?")
            .bind("alan".to_string())
            .fetch_optional_async()
            .await
            .unwrap();
        assert_eq!(name, Some("alan".into()));

        let missing: Option<String> = db
            .query("SELECT name FROM users WHERE name = ?")
            .bind("grace".to_string())
            .fetch_optional_async()
            .await
            .unwrap();
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn fetch_one_reports_empty_results() {
        let db = fresh().await;
        let result: Result<User, _> = db.query("SELECT * FROM users").fetch_one_async().await;
        assert!(matches!(result, Err(DatabaseError::Decode(_))));
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let db = fresh().await;
        assert_eq!(db.migrate(&[create_users()]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn edited_applied_migrations_are_rejected() {
        let db = fresh().await;
        let edited = Migration::new(1, "create_users", "CREATE TABLE users (id BIGINT)");
        assert!(matches!(
            db.migrate(&[edited]).await,
            Err(DatabaseError::Migration { .. })
        ));
    }

    #[tokio::test]
    async fn duplicate_versions_are_rejected() {
        let db = fresh().await;
        let dup = Migration::new(1, "also_first", "CREATE TABLE other (id BIGINT)");
        assert!(matches!(
            db.migrate(&[create_users(), dup]).await,
            Err(DatabaseError::Migration { .. })
        ));
    }

    #[tokio::test]
    async fn later_migrations_apply_on_top() {
        let db = fresh().await;
        let add_bio = Migration::new(2, "add_bio", "ALTER TABLE users ADD COLUMN bio TEXT");
        assert_eq!(db.migrate(&[create_users(), add_bio]).await.unwrap(), 1);

        db.query("INSERT INTO users (name, bio) VALUES (?, ?)")
            .bind("grace".to_string())
            .bind("pioneer".to_string())
            .execute_async()
            .await
            .unwrap();
        let bio: Option<String> = db
            .query("SELECT bio FROM users WHERE name = ?")
            .bind("grace".to_string())
            .fetch_one_async()
            .await
            .unwrap();
        assert_eq!(bio, Some("pioneer".into()));
    }

    #[tokio::test]
    async fn blocking_api_works_from_handler_threads() {
        let db = fresh().await;
        seed(&db).await;

        // handlers run on blocking threads (spawn_blocking), where the sync
        // API drives sqlx through the captured runtime handle
        let count = tokio::task::spawn_blocking(move || {
            db.query("SELECT count(*) FROM users")
                .fetch_one::<i64>()
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn config_infers_driver_from_url() {
        assert_eq!(Config::from_url("sqlite:shop.db").driver, Driver::Sqlite);
        assert_eq!(Config::from_url("shop.db").driver, Driver::Sqlite);
        assert_eq!(
            Config::from_url("postgres://localhost/shop").driver,
            Driver::Postgres
        );
        assert_eq!(
            Config::from_url("postgresql://localhost/shop").driver,
            Driver::Postgres
        );
    }
}
