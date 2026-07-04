/// A runtime data instance to be checked against a [`super::Schema`].
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
	Bool(bool),
	I32(i32),
	U32(u32),
	I64(i64),
	U64(u64),
	F32(f32),
	F64(f64),
	Str(String),
	Date(String),
	Blob(Vec<u8>),
	List(Vec<Value>),
	Map(Vec<(Value, Value)>)
}

impl Value {
	pub(crate) fn kind(&self) -> &'static str {
		match self {
			Value::Bool(_) => "bool",
			Value::I32(_) => "i32",
			Value::U32(_) => "u32",
			Value::I64(_) => "i64",
			Value::U64(_) => "u64",
			Value::F32(_) => "f32",
			Value::F64(_) => "f64",
			Value::Str(_) => "str",
			Value::Date(_) => "date",
			Value::Blob(_) => "blob",
			Value::List(_) => "list",
			Value::Map(_) => "map"
		}
	}
}
