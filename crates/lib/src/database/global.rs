use std::sync::OnceLock;

use super::config::Config;
use super::connection::Database;
use super::error::DatabaseError;
use super::migration::Migration;
use super::query::Query;

static DATABASE: OnceLock<Database> = OnceLock::new();

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

pub fn get() -> &'static Database {
    try_get().expect(
        "database not initialized: set LOOP_DB_URL or add [database] to loop.toml \
         so the server connects at startup",
    )
}

pub fn query(sql: impl Into<String>) -> Query<'static> {
    get().query(sql)
}

/// Starts an atomic multi-statement batch on the global database.
pub fn atomic() -> super::Atomic<'static> {
    get().atomic()
}

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

#[cfg(feature = "macros")]
pub async fn init_from_env() -> Result<Option<&'static Database>, DatabaseError> {
    match Config::from_env() {
        Some(config) => init(&config, &registered_migrations()).await.map(Some),
        None => Ok(None),
    }
}
