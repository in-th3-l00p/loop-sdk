use super::error::ValidationError;
use super::value::Value;
use crate::schema::{Primitive, Schema};

pub(crate) fn validate(schema: &Schema, value: &Value) -> Result<(), ValidationError> {
	match schema {
		Schema::Primitive(primitive) => validate_primitive(primitive, value),
		Schema::List(item_schema) => validate_list(item_schema, value),
		Schema::Map(key_schema, value_schema) => validate_map(key_schema, value_schema, value)
	}
}

fn validate_primitive(primitive: &Primitive, value: &Value) -> Result<(), ValidationError> {
	let matches = matches!(
		(primitive, value),
		(Primitive::Bool, Value::Bool(_))
			| (Primitive::I32, Value::I32(_))
			| (Primitive::U32, Value::U32(_))
			| (Primitive::I64, Value::I64(_))
			| (Primitive::U64, Value::U64(_))
			| (Primitive::F32, Value::F32(_))
			| (Primitive::F64, Value::F64(_))
			| (Primitive::Str, Value::Str(_))
			| (Primitive::Date, Value::Date(_))
			| (Primitive::Blob, Value::Blob(_))
	);

	if matches {
		Ok(())
	} else {
		Err(ValidationError::TypeMismatch { expected: primitive_kind(primitive), found: value.kind() })
	}
}

fn validate_list(item_schema: &Schema, value: &Value) -> Result<(), ValidationError> {
	let Value::List(items) = value else {
		return Err(ValidationError::TypeMismatch { expected: "list", found: value.kind() });
	};

	for (index, item) in items.iter().enumerate() {
		validate(item_schema, item).map_err(|source| ValidationError::ListItem { index, source: Box::new(source) })?;
	}

	Ok(())
}

fn validate_map(key_schema: &Schema, value_schema: &Schema, value: &Value) -> Result<(), ValidationError> {
	let Value::Map(entries) = value else {
		return Err(ValidationError::TypeMismatch { expected: "map", found: value.kind() });
	};

	for (index, (key, val)) in entries.iter().enumerate() {
		validate(key_schema, key).map_err(|source| ValidationError::MapKey { index, source: Box::new(source) })?;
		validate(value_schema, val).map_err(|source| ValidationError::MapValue { index, source: Box::new(source) })?;
	}

	Ok(())
}

fn primitive_kind(primitive: &Primitive) -> &'static str {
	match primitive {
		Primitive::Bool => "bool",
		Primitive::I32 => "i32",
		Primitive::U32 => "u32",
		Primitive::I64 => "i64",
		Primitive::U64 => "u64",
		Primitive::F32 => "f32",
		Primitive::F64 => "f64",
		Primitive::Str => "str",
		Primitive::Date => "date",
		Primitive::Blob => "blob"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn validates_matching_primitive() {
		assert!(validate(&Schema::Primitive(Primitive::Bool), &Value::Bool(true)).is_ok());
	}

	#[test]
	fn rejects_mismatched_primitive() {
		let err = validate(&Schema::Primitive(Primitive::Bool), &Value::Str("nope".into())).unwrap_err();
		assert_eq!(err, ValidationError::TypeMismatch { expected: "bool", found: "str" });
	}

	#[test]
	fn validates_list_of_primitives() {
		let schema = Schema::List(Box::new(Schema::Primitive(Primitive::I64)));
		let value = Value::List(vec![Value::I64(1), Value::I64(2)]);
		assert!(validate(&schema, &value).is_ok());
	}

	#[test]
	fn rejects_non_list_value_against_list_schema() {
		let schema = Schema::List(Box::new(Schema::Primitive(Primitive::I64)));
		let err = validate(&schema, &Value::I64(1)).unwrap_err();
		assert_eq!(err, ValidationError::TypeMismatch { expected: "list", found: "i64" });
	}

	#[test]
	fn rejects_list_with_bad_item() {
		let schema = Schema::List(Box::new(Schema::Primitive(Primitive::I64)));
		let value = Value::List(vec![Value::I64(1), Value::Str("bad".into())]);
		let err = validate(&schema, &value).unwrap_err();
		assert_eq!(
			err,
			ValidationError::ListItem {
				index: 1,
				source: Box::new(ValidationError::TypeMismatch { expected: "i64", found: "str" })
			}
		);
	}

	#[test]
	fn validates_nested_map() {
		let schema = Schema::Map(
			Box::new(Schema::Primitive(Primitive::Str)),
			Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::I64))))
		);
		let value = Value::Map(vec![(Value::Str("a".into()), Value::List(vec![Value::I64(1), Value::I64(2)]))]);
		assert!(validate(&schema, &value).is_ok());
	}

	#[test]
	fn rejects_map_with_bad_key() {
		let schema =
			Schema::Map(Box::new(Schema::Primitive(Primitive::Str)), Box::new(Schema::Primitive(Primitive::I64)));
		let value = Value::Map(vec![(Value::I64(1), Value::I64(2))]);
		let err = validate(&schema, &value).unwrap_err();
		assert_eq!(
			err,
			ValidationError::MapKey {
				index: 0,
				source: Box::new(ValidationError::TypeMismatch { expected: "str", found: "i64" })
			}
		);
	}

	#[test]
	fn rejects_map_with_bad_value() {
		let schema =
			Schema::Map(Box::new(Schema::Primitive(Primitive::Str)), Box::new(Schema::Primitive(Primitive::I64)));
		let value = Value::Map(vec![(Value::Str("a".into()), Value::Str("bad".into()))]);
		let err = validate(&schema, &value).unwrap_err();
		assert_eq!(
			err,
			ValidationError::MapValue {
				index: 0,
				source: Box::new(ValidationError::TypeMismatch { expected: "i64", found: "str" })
			}
		);
	}
}
