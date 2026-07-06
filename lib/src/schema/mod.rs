pub mod convert;
mod serialization;
mod validation;

pub use convert::{AsSchema, Blob, Date, FromValue, IntoValue};
pub use validation::{Constraint, ValidationError, Value};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Primitive {
    Bool,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Str,
    Date,
    Blob,
}

impl Primitive {
    pub fn kind(&self) -> &'static str {
        match self {
            Primitive::Bool => "bool",
            Primitive::I32 => "i32",
            Primitive::U32 => "u32",
            Primitive::I64 => "i64",
            Primitive::U64 => "u64",
            Primitive::F32 => "f32",
            Primitive::F64 => "f64",
            Primitive::Str => "str",
            Primitive::Date => "date",
            Primitive::Blob => "blob",
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Schema {
    Primitive(Primitive),
    Optional(Box<Schema>),
    List(Box<Schema>),
    Map(Box<Schema>, Box<Schema>),
    Record(Vec<(String, Schema)>),
    Constrained(Box<Schema>, Vec<Constraint>),
}

impl Schema {
    pub fn base(&self) -> &Schema {
        match self {
            Schema::Constrained(inner, _) => inner.base(),
            other => other,
        }
    }

    /// Whether `Value::Null` satisfies this schema (i.e. it is optional,
    /// possibly under constraints).
    pub fn accepts_null(&self) -> bool {
        matches!(self.base(), Schema::Optional(_))
    }
}
