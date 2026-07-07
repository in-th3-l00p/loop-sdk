use sqlx::Row as _;
use sqlx::postgres::{PgArguments, PgPool, PgRow};

use super::{out_of_range, primitive_of, unsupported_bind, unsupported_column};
use crate::database::error::DatabaseError;
use crate::schema::{Primitive, Schema, Value};

type Query<'q> = sqlx::query::Query<'q, sqlx::Postgres, PgArguments>;

pub async fn fetch(
    pool: &PgPool,
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

pub async fn raw(pool: &PgPool, sql: &str) -> Result<(), DatabaseError> {
    sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(DatabaseError::Query)?;
    Ok(())
}

pub async fn migrate_step(
    pool: &PgPool,
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

fn decode_row(row: &PgRow, target: &Schema) -> Result<Value, DatabaseError> {
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

fn decode_column<I>(row: &PgRow, index: I, schema: &Schema) -> Result<Value, DatabaseError>
where
    I: sqlx::ColumnIndex<PgRow> + std::fmt::Display + Copy,
{
    let Some(primitive) = primitive_of(schema) else {
        return Err(unsupported_column(schema));
    };
    let decode_error = |e: sqlx::Error| DatabaseError::Decode(format!("column {index}: {e}"));

    macro_rules! get {
        ($ty:ty) => {
            row.try_get::<Option<$ty>, _>(index).map_err(decode_error)
        };
    }

    let wide = |n: i64, primitive: &Primitive| -> Result<Value, DatabaseError> {
        Ok(match primitive {
            Primitive::I32 => Value::I32(
                i32::try_from(n).map_err(|_| out_of_range(&format!("column {index}")))?,
            ),
            Primitive::U32 => Value::U32(
                u32::try_from(n).map_err(|_| out_of_range(&format!("column {index}")))?,
            ),
            Primitive::U64 => Value::U64(
                u64::try_from(n).map_err(|_| out_of_range(&format!("column {index}")))?,
            ),
            _ => Value::I64(n),
        })
    };

    let integer = |primitive: &Primitive| -> Result<Option<Value>, DatabaseError> {
        let n = match get!(i64) {
            Ok(n) => n,
            Err(_) => get!(i32)?.map(i64::from),
        };
        n.map(|n| wide(n, primitive)).transpose()
    };

    let value = match primitive {
        Primitive::Bool => get!(bool)?.map(Value::Bool),
        Primitive::I32 | Primitive::U32 | Primitive::I64 | Primitive::U64 => integer(primitive)?,
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
