#[cfg(feature = "db-postgres")]
pub(crate) mod postgres;
#[cfg(feature = "db-sqlite")]
pub(crate) mod sqlite;

use super::error::DatabaseError;
use crate::schema::{Primitive, Schema};

fn primitive_of(schema: &Schema) -> Option<&Primitive> {
    let mut base = schema.base();
    while let Schema::Optional(inner) = base {
        base = inner.base();
    }
    match base {
        Schema::Primitive(primitive) => Some(primitive),
        _ => None,
    }
}

fn unsupported_bind(kind: &str) -> DatabaseError {
    DatabaseError::Unsupported(format!(
        "cannot bind {kind} as a query argument (only primitives for now)"
    ))
}

fn unsupported_column(schema: &Schema) -> DatabaseError {
    DatabaseError::Unsupported(format!(
        "cannot decode a column as {:?} (only primitives for now)",
        schema.base()
    ))
}

fn out_of_range(what: &str) -> DatabaseError {
    DatabaseError::Unsupported(format!("{what} does not fit in the column's integer range"))
}
