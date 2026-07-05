use super::error::ValidationError;
use super::value::Value;
use crate::schema::{Primitive, Schema};

pub(crate) fn validate(schema: &Schema, value: &Value) -> Result<(), ValidationError> {
    match schema {
        Schema::Primitive(primitive) => validate_primitive(primitive, value),
        Schema::List(item_schema) => validate_list(item_schema, value),
        Schema::Map(key_schema, value_schema) => validate_map(key_schema, value_schema, value),
        Schema::Record(fields) => validate_record(fields, value),
        Schema::Constrained(inner, constraints) => {
            validate(inner, value)?;
            for constraint in constraints {
                constraint.check(value)?;
            }
            Ok(())
        }
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
        Err(ValidationError::TypeMismatch {
            expected: primitive.kind(),
            found: value.kind(),
        })
    }
}

fn validate_list(item_schema: &Schema, value: &Value) -> Result<(), ValidationError> {
    let Value::List(items) = value else {
        return Err(ValidationError::TypeMismatch {
            expected: "list",
            found: value.kind(),
        });
    };

    for (index, item) in items.iter().enumerate() {
        validate(item_schema, item).map_err(|source| ValidationError::ListItem {
            index,
            source: Box::new(source),
        })?;
    }

    Ok(())
}

fn validate_record(fields: &[(String, Schema)], value: &Value) -> Result<(), ValidationError> {
    let Value::Map(entries) = value else {
        return Err(ValidationError::TypeMismatch {
            expected: "record",
            found: value.kind(),
        });
    };

    for (key, _) in entries {
        let Value::Str(name) = key else {
            return Err(ValidationError::TypeMismatch {
                expected: "str field name",
                found: key.kind(),
            });
        };
        if !fields.iter().any(|(field, _)| field == name) {
            return Err(ValidationError::UnknownField(name.clone()));
        }
    }

    for (name, schema) in fields {
        let entry = entries
            .iter()
            .find(|(key, _)| matches!(key, Value::Str(field) if field == name));
        let Some((_, field_value)) = entry else {
            return Err(ValidationError::MissingField(name.clone()));
        };
        validate(schema, field_value).map_err(|source| ValidationError::Field {
            name: name.clone(),
            source: Box::new(source),
        })?;
    }

    Ok(())
}

fn validate_map(
    key_schema: &Schema,
    value_schema: &Schema,
    value: &Value,
) -> Result<(), ValidationError> {
    let Value::Map(entries) = value else {
        return Err(ValidationError::TypeMismatch {
            expected: "map",
            found: value.kind(),
        });
    };

    for (index, (key, val)) in entries.iter().enumerate() {
        validate(key_schema, key).map_err(|source| ValidationError::MapKey {
            index,
            source: Box::new(source),
        })?;
        validate(value_schema, val).map_err(|source| ValidationError::MapValue {
            index,
            source: Box::new(source),
        })?;
    }

    Ok(())
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
        let err = validate(
            &Schema::Primitive(Primitive::Bool),
            &Value::Str("nope".into()),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ValidationError::TypeMismatch {
                expected: "bool",
                found: "str"
            }
        );
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
        assert_eq!(
            err,
            ValidationError::TypeMismatch {
                expected: "list",
                found: "i64"
            }
        );
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
                source: Box::new(ValidationError::TypeMismatch {
                    expected: "i64",
                    found: "str"
                })
            }
        );
    }

    #[test]
    fn validates_nested_map() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::I64)))),
        );
        let value = Value::Map(vec![(
            Value::Str("a".into()),
            Value::List(vec![Value::I64(1), Value::I64(2)]),
        )]);
        assert!(validate(&schema, &value).is_ok());
    }

    #[test]
    fn rejects_map_with_bad_key() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::Primitive(Primitive::I64)),
        );
        let value = Value::Map(vec![(Value::I64(1), Value::I64(2))]);
        let err = validate(&schema, &value).unwrap_err();
        assert_eq!(
            err,
            ValidationError::MapKey {
                index: 0,
                source: Box::new(ValidationError::TypeMismatch {
                    expected: "str",
                    found: "i64"
                })
            }
        );
    }

    #[test]
    fn rejects_map_with_bad_value() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::Primitive(Primitive::I64)),
        );
        let value = Value::Map(vec![(Value::Str("a".into()), Value::Str("bad".into()))]);
        let err = validate(&schema, &value).unwrap_err();
        assert_eq!(
            err,
            ValidationError::MapValue {
                index: 0,
                source: Box::new(ValidationError::TypeMismatch {
                    expected: "i64",
                    found: "str"
                })
            }
        );
    }

    fn record_schema() -> Schema {
        Schema::Record(vec![
            ("name".into(), Schema::Primitive(Primitive::Str)),
            ("age".into(), Schema::Primitive(Primitive::U32)),
        ])
    }

    fn record_value(name: &str, age: u32) -> Value {
        Value::Map(vec![
            (Value::Str("name".into()), Value::Str(name.into())),
            (Value::Str("age".into()), Value::U32(age)),
        ])
    }

    #[test]
    fn validates_record_with_heterogeneous_fields() {
        assert!(validate(&record_schema(), &record_value("ada", 36)).is_ok());
    }

    #[test]
    fn rejects_non_map_value_against_record_schema() {
        let err = validate(&record_schema(), &Value::I64(1)).unwrap_err();
        assert_eq!(
            err,
            ValidationError::TypeMismatch {
                expected: "record",
                found: "i64"
            }
        );
    }

    #[test]
    fn rejects_record_with_missing_field() {
        let value = Value::Map(vec![(Value::Str("name".into()), Value::Str("ada".into()))]);
        assert_eq!(
            validate(&record_schema(), &value).unwrap_err(),
            ValidationError::MissingField("age".into())
        );
    }

    #[test]
    fn rejects_record_with_unknown_field() {
        let value = Value::Map(vec![
            (Value::Str("name".into()), Value::Str("ada".into())),
            (Value::Str("age".into()), Value::U32(36)),
            (Value::Str("extra".into()), Value::Bool(true)),
        ]);
        assert_eq!(
            validate(&record_schema(), &value).unwrap_err(),
            ValidationError::UnknownField("extra".into())
        );
    }

    #[test]
    fn rejects_record_with_non_string_key() {
        let value = Value::Map(vec![(Value::I64(1), Value::Str("ada".into()))]);
        assert_eq!(
            validate(&record_schema(), &value).unwrap_err(),
            ValidationError::TypeMismatch {
                expected: "str field name",
                found: "i64"
            }
        );
    }

    #[test]
    fn reports_record_field_errors_with_the_field_name() {
        let value = Value::Map(vec![
            (Value::Str("name".into()), Value::Str("ada".into())),
            (Value::Str("age".into()), Value::Str("old".into())),
        ]);
        assert_eq!(
            validate(&record_schema(), &value).unwrap_err(),
            ValidationError::Field {
                name: "age".into(),
                source: Box::new(ValidationError::TypeMismatch {
                    expected: "u32",
                    found: "str"
                })
            }
        );
    }

    #[test]
    fn constrained_schema_checks_type_then_constraints() {
        let schema = Schema::Constrained(
            Box::new(Schema::Primitive(Primitive::U32)),
            vec![crate::schema::Constraint::Min(18.0)],
        );

        assert!(validate(&schema, &Value::U32(21)).is_ok());
        assert_eq!(
            validate(&schema, &Value::U32(3)).unwrap_err(),
            ValidationError::Constraint("must be at least 18".into())
        );
        assert!(matches!(
            validate(&schema, &Value::Str("x".into())).unwrap_err(),
            ValidationError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn constrained_schema_applies_every_constraint() {
        let schema = Schema::Constrained(
            Box::new(Schema::Primitive(Primitive::U32)),
            vec![
                crate::schema::Constraint::Min(1.0),
                crate::schema::Constraint::Max(10.0),
            ],
        );

        assert!(validate(&schema, &Value::U32(5)).is_ok());
        assert!(validate(&schema, &Value::U32(11)).is_err());
    }
}
