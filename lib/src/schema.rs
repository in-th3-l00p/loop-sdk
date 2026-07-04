use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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
	Blob
}

#[derive(Serialize, Deserialize)]
pub enum Schema {
	Primitive(Primitive),
	List(Box<Schema>),
	Map(Box<Schema>, Box<Schema>)
}
