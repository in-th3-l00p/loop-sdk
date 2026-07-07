use super::connection::Database;
use super::error::DatabaseError;
use crate::schema::{FromValue, IntoValue, Schema, Value};

pub struct Query<'db> {
    db: &'db Database,
    sql: String,
    args: Vec<(Value, Schema)>,
}

impl Database {
    pub fn query(&self, sql: impl Into<String>) -> Query<'_> {
        Query {
            db: self,
            sql: sql.into(),
            args: Vec::new(),
        }
    }
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
