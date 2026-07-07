use sqlx::{Executor as _, Row as _};
use sqlx::postgres::{PgArguments, PgPool, PgPoolOptions, PgRow};

use super::{
    Backend, BoxFuture, Column, column_out_of_range, decode_failed, decode_row, out_of_range,
    primitive_of, unsupported_bind, unsupported_column,
};
use crate::database::error::DatabaseError;
use crate::database::sql::Dialect;
use crate::schema::{Primitive, Schema, Value};

type Query<'q> = sqlx::query::Query<'q, sqlx::Postgres, PgArguments>;

pub(super) struct PostgresBackend {
    pool: PgPool,
}

impl PostgresBackend {
    pub(super) async fn connect(url: &str) -> Result<PostgresBackend, DatabaseError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(DatabaseError::Connect)?;
        Ok(PostgresBackend { pool })
    }
}

impl Backend for PostgresBackend {
    fn dialect(&self) -> Dialect {
        Dialect::Postgres
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
    pool: &PgPool,
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
    pool: &PgPool,
    sql: &str,
    args: &[(Value, Schema)],
) -> Result<u64, DatabaseError> {
    let result = bind_all(sqlx::query(sql), args)?
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(result.rows_affected())
}

async fn raw(pool: &PgPool, sql: &str) -> Result<(), DatabaseError> {
    sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(())
}

async fn migrate_step(
    pool: &PgPool,
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
    for (value, schema) in args {
        query = match value {
            Value::Null => match primitive_of(schema) {
                Some(Primitive::Bool) => query.bind(None::<bool>),
                Some(Primitive::I32) => query.bind(None::<i32>),
                Some(Primitive::U32 | Primitive::I64 | Primitive::U64) => query.bind(None::<i64>),
                Some(Primitive::F32) => query.bind(None::<f32>),
                Some(Primitive::F64) => query.bind(None::<f64>),
                Some(Primitive::Str | Primitive::Date) => query.bind(None::<String>),
                Some(Primitive::Blob) => query.bind(None::<Vec<u8>>),
                None => return Err(unsupported_bind("a null of non-primitive type")),
            },
            Value::Bool(b) => query.bind(*b),
            Value::I32(n) => query.bind(*n),
            Value::U32(n) => query.bind(i64::from(*n)),
            Value::I64(n) => query.bind(*n),
            Value::U64(n) => {
                query.bind(i64::try_from(*n).map_err(|_| out_of_range("u64 value"))?)
            }
            Value::F32(n) => query.bind(*n),
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
    row: &PgRow,
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

    let narrow = |n: i64| -> Result<Value, DatabaseError> {
        Ok(match primitive {
            Primitive::I32 => {
                Value::I32(i32::try_from(n).map_err(|_| column_out_of_range(column))?)
            }
            Primitive::U32 => {
                Value::U32(u32::try_from(n).map_err(|_| column_out_of_range(column))?)
            }
            Primitive::U64 => {
                Value::U64(u64::try_from(n).map_err(|_| column_out_of_range(column))?)
            }
            _ => Value::I64(n),
        })
    };

    let integer = || -> Result<Option<Value>, DatabaseError> {
        let n = match get!(i64) {
            Ok(n) => n,
            Err(_) => get!(i32)?.map(i64::from),
        };
        n.map(narrow).transpose()
    };

    let value = match primitive {
        Primitive::Bool => get!(bool)?.map(Value::Bool),
        Primitive::I32 | Primitive::U32 | Primitive::I64 | Primitive::U64 => integer()?,
        Primitive::F32 => match get!(f32) {
            Ok(n) => n.map(Value::F32),
            Err(_) => get!(f64)?.map(|n| Value::F32(n as f32)),
        },
        Primitive::F64 => match get!(f64) {
            Ok(n) => n.map(Value::F64),
            Err(_) => get!(f32)?.map(|n| Value::F64(f64::from(n))),
        },
        Primitive::Str => get!(String)?.map(Value::Str),
        Primitive::Date => get!(String)?.map(Value::Date),
        Primitive::Blob => get!(Vec<u8>)?.map(Value::Blob),
    };
    Ok(value.unwrap_or(Value::Null))
}
