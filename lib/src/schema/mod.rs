mod serialization;
mod validation;

pub use validation::{ValidationError, Value};

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
    List(Box<Schema>),
    Map(Box<Schema>, Box<Schema>),
}
