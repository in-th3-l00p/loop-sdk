#[cfg(feature = "db-postgres")]
mod postgres;
#[cfg(feature = "db-sqlite")]
mod sqlite;

use std::fmt;
use std::pin::Pin;

use super::config::{Config, Driver};
use super::error::DatabaseError;
use super::sql::Dialect;
use crate::schema::{Primitive, Schema, Value};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Backend: Send + Sync {
    fn dialect(&self) -> Dialect;

    fn fetch<'a>(
        &'a self,
        sql: &'a str,
        args: &'a [(Value, Schema)],
        target: &'a Schema,
    ) -> BoxFuture<'a, Result<Vec<Value>, DatabaseError>>;

    fn execute<'a>(
        &'a self,
        sql: &'a str,
        args: &'a [(Value, Schema)],
    ) -> BoxFuture<'a, Result<u64, DatabaseError>>;

    fn raw<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<(), DatabaseError>>;

    fn migrate_step<'a>(
        &'a self,
        migration_sql: &'a str,
        record_sql: &'a str,
        record_args: &'a [(Value, Schema)],
    ) -> BoxFuture<'a, Result<(), DatabaseError>>;
}

pub(crate) async fn connect(config: &Config) -> Result<Box<dyn Backend>, DatabaseError> {
    match config.driver {
        Driver::Sqlite => connect_sqlite(&config.url).await,
        Driver::Postgres => connect_postgres(&config.url).await,
    }
}

async fn connect_sqlite(url: &str) -> Result<Box<dyn Backend>, DatabaseError> {
    #[cfg(feature = "db-sqlite")]
    {
        Ok(Box::new(sqlite::SqliteBackend::connect(url).await?))
    }
    #[cfg(not(feature = "db-sqlite"))]
    {
        let _ = url;
        Err(not_compiled("sqlite", "db-sqlite"))
    }
}

async fn connect_postgres(url: &str) -> Result<Box<dyn Backend>, DatabaseError> {
    #[cfg(feature = "db-postgres")]
    {
        Ok(Box::new(postgres::PostgresBackend::connect(url).await?))
    }
    #[cfg(not(feature = "db-postgres"))]
    {
        let _ = url;
        Err(not_compiled("postgres", "db-postgres"))
    }
}

#[cfg(not(all(feature = "db-sqlite", feature = "db-postgres")))]
fn not_compiled(name: &str, feature: &str) -> DatabaseError {
    DatabaseError::Unavailable(format!(
        "{name} support is not compiled in (enable the {feature} feature)"
    ))
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Column<'a> {
    Index(usize),
    Name(&'a str),
}

impl fmt::Display for Column<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Column::Index(index) => write!(f, "column {index}"),
            Column::Name(name) => write!(f, "column {name}"),
        }
    }
}

pub(crate) fn decode_row<Row>(
    row: &Row,
    target: &Schema,
    decode_column: impl Fn(&Row, Column<'_>, &Schema) -> Result<Value, DatabaseError>,
) -> Result<Value, DatabaseError> {
    match target.base() {
        Schema::Record(fields) => fields
            .iter()
            .map(|(name, schema)| {
                let value = decode_column(row, Column::Name(name), schema)?;
                Ok((Value::Str(name.clone()), value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Map),
        _ => decode_column(row, Column::Index(0), target),
    }
}

pub(crate) fn primitive_of(schema: &Schema) -> Option<&Primitive> {
    let mut base = schema.base();
    while let Schema::Optional(inner) = base {
        base = inner.base();
    }
    match base {
        Schema::Primitive(primitive) => Some(primitive),
        _ => None,
    }
}

pub(crate) fn unsupported_bind(kind: &str) -> DatabaseError {
    DatabaseError::Unsupported(format!(
        "cannot bind {kind} as a query argument (only primitives for now)"
    ))
}

pub(crate) fn unsupported_column(schema: &Schema) -> DatabaseError {
    DatabaseError::Unsupported(format!(
        "cannot decode a column as {:?} (only primitives for now)",
        schema.base()
    ))
}

pub(crate) fn out_of_range(what: &str) -> DatabaseError {
    DatabaseError::Unsupported(format!("{what} does not fit in the column's integer range"))
}

pub(crate) fn decode_failed(column: Column<'_>, error: sqlx::Error) -> DatabaseError {
    DatabaseError::Decode(format!("{column}: {error}"))
}

pub(crate) fn column_out_of_range(column: Column<'_>) -> DatabaseError {
    out_of_range(&column.to_string())
}
