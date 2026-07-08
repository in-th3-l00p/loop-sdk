/* multi-statement transactions as a builder: every statement runs in one
BEGIN…COMMIT, and a `guard` statement that affects no rows rolls the whole
batch back. the write-side sibling of Query */

use super::backend::AtomicStatement;
use super::connection::Database;
use super::error::DatabaseError;
use crate::schema::IntoValue;

pub struct Atomic<'db> {
    db: &'db Database,
    statements: Vec<AtomicStatement>,
}

impl Database {
    pub fn atomic(&self) -> Atomic<'_> {
        Atomic {
            db: self,
            statements: Vec::new(),
        }
    }
}

impl Atomic<'_> {
    /// Adds a statement to the batch.
    pub fn query(mut self, sql: impl Into<String>) -> Self {
        self.statements.push(AtomicStatement {
            sql: sql.into(),
            args: Vec::new(),
            guard: false,
        });
        self
    }

    /// Adds a statement that must affect at least one row — otherwise the
    /// whole transaction rolls back with [`DatabaseError::Guard`]. The
    /// natural place for balance checks and optimistic conditions:
    /// `.guard("UPDATE ledger SET balance = balance - ? WHERE id = ? AND balance >= ?")`.
    pub fn guard(mut self, sql: impl Into<String>) -> Self {
        self.statements.push(AtomicStatement {
            sql: sql.into(),
            args: Vec::new(),
            guard: true,
        });
        self
    }

    /// Binds an argument to the most recently added statement.
    pub fn bind<T: IntoValue>(mut self, value: T) -> Self {
        let statement = self
            .statements
            .last_mut()
            .expect("bind() needs a preceding query() or guard()");
        statement.args.push((value.into_value(), T::schema()));
        self
    }

    pub fn execute(self) -> Result<(), DatabaseError> {
        let Atomic { db, statements } = self;
        db.block_on(db.atomic_values(statements))
    }

    pub async fn execute_async(self) -> Result<(), DatabaseError> {
        let Atomic { db, statements } = self;
        db.atomic_values(statements).await
    }
}
