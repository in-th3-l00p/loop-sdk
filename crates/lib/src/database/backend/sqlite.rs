use sqlx::{Executor as _, Row as _};
use sqlx::sqlite::{
    SqliteArguments, SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions,
    SqliteRow,
};

use super::{
    Backend, BoxFuture, Column, column_out_of_range, decode_failed, decode_row, out_of_range,
    primitive_of, unsupported_bind, unsupported_column,
};
use crate::database::error::DatabaseError;
use crate::database::sql::Dialect;
use crate::schema::{Primitive, Schema, Value};

type Query<'q> = sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments<'q>>;

pub(super) struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    pub(super) async fn connect(url: &str) -> Result<SqliteBackend, DatabaseError> {
        use std::str::FromStr as _;

        let memory = url.contains(":memory:");
        let mut options = SqliteConnectOptions::from_str(url)
            .map_err(DatabaseError::Connect)?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5))
            .foreign_keys(true);
        if !memory {
            options = options.journal_mode(SqliteJournalMode::Wal);
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(if memory { 1 } else { 5 })
            .connect_with(options)
            .await
            .map_err(DatabaseError::Connect)?;
        Ok(SqliteBackend { pool })
    }
}

impl Backend for SqliteBackend {
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    fn fetch<'a>(
        &'a self,
        sql: &'a str,
        args: &'a [(Value, Schema)],
        target: &'a Schema,
    ) -> BoxFuture<'a, Result<Vec<Value>, DatabaseError>> {
        Box::pin(fetch(&self.pool, sql, args, target))
    }

    fn execute<'a>(
        &'a self,
        sql: &'a str,
        args: &'a [(Value, Schema)],
    ) -> BoxFuture<'a, Result<u64, DatabaseError>> {
        Box::pin(execute(&self.pool, sql, args))
    }

    fn raw<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<(), DatabaseError>> {
        Box::pin(raw(&self.pool, sql))
    }

    fn migrate_step<'a>(
        &'a self,
        migration_sql: &'a str,
        record_sql: &'a str,
        record_args: &'a [(Value, Schema)],
    ) -> BoxFuture<'a, Result<(), DatabaseError>> {
        Box::pin(migrate_step(&self.pool, migration_sql, record_sql, record_args))
    }
}

async fn fetch(
    pool: &SqlitePool,
    sql: &str,
    args: &[(Value, Schema)],
    target: &Schema,
) -> Result<Vec<Value>, DatabaseError> {
    let rows = bind_all(sqlx::query(sql), args)?
        .fetch_all(pool)
        .await
        .map_err(DatabaseError::Query)?;
    rows.iter()
        .map(|row| decode_row(row, target, decode_column))
        .collect()
}

async fn execute(
    pool: &SqlitePool,
    sql: &str,
    args: &[(Value, Schema)],
) -> Result<u64, DatabaseError> {
    let result = bind_all(sqlx::query(sql), args)?
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(result.rows_affected())
}

async fn raw(pool: &SqlitePool, sql: &str) -> Result<(), DatabaseError> {
    sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(())
}

async fn migrate_step(
    pool: &SqlitePool,
    migration_sql: &str,
    record_sql: &str,
    record_args: &[(Value, Schema)],
) -> Result<(), DatabaseError> {
    let mut tx = pool.begin().await.map_err(DatabaseError::Query)?;
    (&mut *tx)
        .execute(migration_sql)
        .await
        .map_err(DatabaseError::Query)?;
    bind_all(sqlx::query(record_sql), record_args)?
        .execute(&mut *tx)
        .await
        .map_err(DatabaseError::Query)?;
    tx.commit().await.map_err(DatabaseError::Query)
}

fn bind_all<'q>(
    mut query: Query<'q>,
    args: &[(Value, Schema)],
) -> Result<Query<'q>, DatabaseError> {
    for (value, _schema) in args {
        query = match value {
            Value::Null => query.bind(None::<i64>),
            Value::Bool(b) => query.bind(*b),
            Value::I32(n) => query.bind(*n),
            Value::U32(n) => query.bind(i64::from(*n)),
            Value::I64(n) => query.bind(*n),
            Value::U64(n) => {
                query.bind(i64::try_from(*n).map_err(|_| out_of_range("u64 value"))?)
            }
            Value::F32(n) => query.bind(f64::from(*n)),
            Value::F64(n) => query.bind(*n),
            Value::Str(s) | Value::Date(s) => query.bind(s.clone()),
            Value::Blob(bytes) => query.bind(bytes.clone()),
            Value::List(_) => return Err(unsupported_bind("a list")),
            Value::Map(_) => return Err(unsupported_bind("a map")),
        };
    }
    Ok(query)
}

fn decode_column(
    row: &SqliteRow,
    column: Column<'_>,
    schema: &Schema,
) -> Result<Value, DatabaseError> {
    let Some(primitive) = primitive_of(schema) else {
        return Err(unsupported_column(schema));
    };

    macro_rules! get {
        ($ty:ty) => {
            match column {
                Column::Index(index) => row.try_get::<Option<$ty>, _>(index),
                Column::Name(name) => row.try_get::<Option<$ty>, _>(name),
            }
            .map_err(|e| decode_failed(column, e))
        };
    }

    let value = match primitive {
        Primitive::Bool => get!(bool)?.map(Value::Bool),
        Primitive::I32 => get!(i32)?.map(Value::I32),
        Primitive::U32 => get!(i64)?
            .map(|n| {
                u32::try_from(n)
                    .map(Value::U32)
                    .map_err(|_| column_out_of_range(column))
            })
            .transpose()?,
        Primitive::I64 => get!(i64)?.map(Value::I64),
        Primitive::U64 => get!(i64)?
            .map(|n| {
                u64::try_from(n)
                    .map(Value::U64)
                    .map_err(|_| column_out_of_range(column))
            })
            .transpose()?,
        Primitive::F32 => get!(f64)?.map(|n| Value::F32(n as f32)),
        Primitive::F64 => get!(f64)?.map(Value::F64),
        Primitive::Str => get!(String)?.map(Value::Str),
        Primitive::Date => get!(String)?.map(Value::Date),
        Primitive::Blob => get!(Vec<u8>)?.map(Value::Blob),
    };
    Ok(value.unwrap_or(Value::Null))
}
