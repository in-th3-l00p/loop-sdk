use sqlx::Row as _;
use sqlx::sqlite::{SqliteArguments, SqlitePool, SqliteRow};

use super::{out_of_range, primitive_of, unsupported_bind, unsupported_column};
use crate::database::error::DatabaseError;
use crate::schema::{Primitive, Schema, Value};

type Query<'q> = sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments<'q>>;

pub async fn fetch(
    pool: &SqlitePool,
    sql: &str,
    args: &[(Value, Schema)],
    target: &Schema,
) -> Result<Vec<Value>, DatabaseError> {
    let rows = bind_all(sqlx::query(sql), args)?
        .fetch_all(pool)
        .await
        .map_err(DatabaseError::Query)?;
    rows.iter().map(|row| decode_row(row, target)).collect()
}

pub async fn execute(
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

pub async fn raw(pool: &SqlitePool, sql: &str) -> Result<(), DatabaseError> {
    sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(())
}

pub async fn migrate_step(
    pool: &SqlitePool,
    migration_sql: &str,
    record_sql: &str,
    record_args: &[(Value, Schema)],
) -> Result<(), DatabaseError> {
    let mut tx = pool.begin().await.map_err(DatabaseError::Query)?;
    sqlx::raw_sql(migration_sql)
        .execute(&mut *tx)
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

fn decode_row(row: &SqliteRow, target: &Schema) -> Result<Value, DatabaseError> {
    match target.base() {
        Schema::Record(fields) => fields
            .iter()
            .map(|(name, schema)| {
                let value = decode_column(row, name.as_str(), schema)?;
                Ok((Value::Str(name.clone()), value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Map),
        _ => decode_column(row, 0, target),
    }
}

fn decode_column<I>(row: &SqliteRow, index: I, schema: &Schema) -> Result<Value, DatabaseError>
where
    I: sqlx::ColumnIndex<SqliteRow> + std::fmt::Display + Copy,
{
    let Some(primitive) = primitive_of(schema) else {
        return Err(unsupported_column(schema));
    };
    let decode_error = |e: sqlx::Error| DatabaseError::Decode(format!("column {index}: {e}"));

    macro_rules! get {
        ($ty:ty, $to:expr) => {
            row.try_get::<Option<$ty>, _>(index)
                .map_err(decode_error)?
                .map($to)
        };
    }

    let value = match primitive {
        Primitive::Bool => get!(bool, Value::Bool),
        Primitive::I32 => get!(i32, Value::I32),
        Primitive::U32 => match get!(i64, |n| n) {
            Some(n) => Some(Value::U32(
                u32::try_from(n).map_err(|_| out_of_range(&format!("column {index}")))?,
            )),
            None => None,
        },
        Primitive::I64 => get!(i64, Value::I64),
        Primitive::U64 => match get!(i64, |n| n) {
            Some(n) => Some(Value::U64(
                u64::try_from(n).map_err(|_| out_of_range(&format!("column {index}")))?,
            )),
            None => None,
        },
        Primitive::F32 => get!(f64, |n| Value::F32(n as f32)),
        Primitive::F64 => get!(f64, Value::F64),
        Primitive::Str => get!(String, Value::Str),
        Primitive::Date => get!(String, Value::Date),
        Primitive::Blob => get!(Vec<u8>, Value::Blob),
    };
    Ok(value.unwrap_or(Value::Null))
}
