use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map as JsonMap, Number, Value as Json};

use crate::schema::{Primitive, Schema, Value};

pub fn decode(schema: &Schema, json: &Json) -> Result<Value, String> {
    match schema {
        Schema::Primitive(primitive) => decode_primitive(primitive, json),
        Schema::Optional(inner) => {
            if json.is_null() {
                Ok(Value::Null)
            } else {
                decode(inner, json)
            }
        }
        Schema::List(item) => {
            let Json::Array(items) = json else {
                return Err(mismatch("array", json));
            };
            items
                .iter()
                .enumerate()
                .map(|(i, item_json)| decode(item, item_json).map_err(|e| format!("item {i}: {e}")))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List)
        }
        Schema::Map(key, value) => decode_map(key, value, json),
        Schema::Record(fields) => decode_record(fields, json),
        Schema::Constrained(inner, _) => decode(inner, json),
    }
}

fn decode_record(fields: &[(String, Schema)], json: &Json) -> Result<Value, String> {
    let Json::Object(object) = json else {
        return Err(mismatch("object", json));
    };

    if let Some(unknown) = object
        .keys()
        .find(|key| !fields.iter().any(|(name, _)| name == key.as_str()))
    {
        return Err(format!("unknown field {unknown:?}"));
    }

    fields
        .iter()
        .map(|(name, schema)| {
            let decoded = match object.get(name) {
                Some(field) => {
                    decode(schema, field).map_err(|e| format!("field {name:?}: {e}"))?
                }
                None if schema.accepts_null() => Value::Null,
                None => return Err(format!("missing field {name:?}")),
            };
            Ok((Value::Str(name.clone()), decoded))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(Value::Map)
}

fn decode_primitive(primitive: &Primitive, json: &Json) -> Result<Value, String> {
    if let Primitive::Blob = primitive {
        return decode_blob(json);
    }

    let decoded = match primitive {
        Primitive::Bool => json.as_bool().map(Value::Bool),
        Primitive::I32 => json
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(Value::I32),
        Primitive::U32 => json
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Value::U32),
        Primitive::I64 => json.as_i64().map(Value::I64),
        Primitive::U64 => json.as_u64().map(Value::U64),
        Primitive::F32 => json.as_f64().map(|n| Value::F32(n as f32)),
        Primitive::F64 => json.as_f64().map(Value::F64),
        Primitive::Str => json.as_str().map(|s| Value::Str(s.into())),
        Primitive::Date => json.as_str().map(|s| Value::Date(s.into())),
        Primitive::Blob => unreachable!(),
    };

    decoded.ok_or_else(|| mismatch(primitive.kind(), json))
}

fn decode_blob(json: &Json) -> Result<Value, String> {
    let encoded = json.as_str().ok_or_else(|| mismatch("base64 blob", json))?;
    BASE64
        .decode(encoded)
        .map(Value::Blob)
        .map_err(|e| format!("invalid base64 blob: {e}"))
}

fn decode_map(key: &Schema, value: &Schema, json: &Json) -> Result<Value, String> {
    if matches!(key, Schema::Primitive(Primitive::Str)) {
        let Json::Object(object) = json else {
            return Err(mismatch("object", json));
        };
        object
            .iter()
            .map(|(k, v)| {
                let decoded = decode(value, v).map_err(|e| format!("key {k:?}: {e}"))?;
                Ok((Value::Str(k.clone()), decoded))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Map)
    } else {
        let Json::Array(entries) = json else {
            return Err(mismatch("array of [key, value] pairs", json));
        };
        entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let Json::Array(pair) = entry else {
                    return Err(format!("entry {i}: expected [key, value] pair"));
                };
                let [k, v] = pair.as_slice() else {
                    return Err(format!("entry {i}: expected exactly 2 elements"));
                };
                Ok((
                    decode(key, k).map_err(|e| format!("entry {i} key: {e}"))?,
                    decode(value, v).map_err(|e| format!("entry {i} value: {e}"))?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Map)
    }
}

pub fn encode(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::I32(n) => Json::Number((*n).into()),
        Value::U32(n) => Json::Number((*n).into()),
        Value::I64(n) => Json::Number((*n).into()),
        Value::U64(n) => Json::Number((*n).into()),
        Value::F32(n) => Number::from_f64(f64::from(*n))
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::F64(n) => Number::from_f64(*n).map(Json::Number).unwrap_or(Json::Null),
        Value::Str(s) | Value::Date(s) => Json::String(s.clone()),
        Value::Blob(bytes) => Json::String(BASE64.encode(bytes)),
        Value::List(items) => Json::Array(items.iter().map(encode).collect()),
        Value::Map(entries) => {
            if entries.iter().all(|(k, _)| matches!(k, Value::Str(_))) {
                let mut object = JsonMap::new();
                for (k, v) in entries {
                    let Value::Str(k) = k else { unreachable!() };
                    object.insert(k.clone(), encode(v));
                }
                Json::Object(object)
            } else {
                Json::Array(
                    entries
                        .iter()
                        .map(|(k, v)| Json::Array(vec![encode(k), encode(v)]))
                        .collect(),
                )
            }
        }
    }
}

fn mismatch(expected: &str, found: &Json) -> String {
    format!("expected {expected}, found {found}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Primitive;
    use serde_json::json;

    #[test]
    fn decodes_primitives_per_schema() {
        assert_eq!(
            decode(&Schema::Primitive(Primitive::I64), &json!(5)).unwrap(),
            Value::I64(5)
        );
        assert_eq!(
            decode(&Schema::Primitive(Primitive::F64), &json!(5)).unwrap(),
            Value::F64(5.0)
        );
        assert_eq!(
            decode(&Schema::Primitive(Primitive::Str), &json!("hi")).unwrap(),
            Value::Str("hi".into())
        );
        assert_eq!(
            decode(&Schema::Primitive(Primitive::Blob), &json!("3q0=")).unwrap(),
            Value::Blob(vec![0xde, 0xad])
        );
    }

    #[test]
    fn rejects_out_of_range_and_mistyped_numbers() {
        assert!(decode(&Schema::Primitive(Primitive::I32), &json!(i64::MAX)).is_err());
        assert!(decode(&Schema::Primitive(Primitive::U64), &json!(-1)).is_err());
        assert!(decode(&Schema::Primitive(Primitive::I64), &json!("5")).is_err());
    }

    #[test]
    fn decodes_string_keyed_map_from_object() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::Primitive(Primitive::I64)),
        );
        let decoded = decode(&schema, &json!({"a": 1})).unwrap();
        assert_eq!(
            decoded,
            Value::Map(vec![(Value::Str("a".into()), Value::I64(1))])
        );
    }

    #[test]
    fn decodes_non_string_keyed_map_from_pairs() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::I64)),
            Box::new(Schema::Primitive(Primitive::Bool)),
        );
        let decoded = decode(&schema, &json!([[1, true], [2, false]])).unwrap();
        assert_eq!(
            decoded,
            Value::Map(vec![
                (Value::I64(1), Value::Bool(true)),
                (Value::I64(2), Value::Bool(false))
            ])
        );
    }

    #[test]
    fn roundtrips_nested_value_through_json() {
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::I64)))),
        );
        let value = Value::Map(vec![(
            Value::Str("xs".into()),
            Value::List(vec![Value::I64(1), Value::I64(2)]),
        )]);

        assert_eq!(decode(&schema, &encode(&value)).unwrap(), value);
    }

    #[test]
    fn decode_error_reports_path_context() {
        let schema = Schema::List(Box::new(Schema::Primitive(Primitive::I64)));
        let err = decode(&schema, &json!([1, "bad"])).unwrap_err();
        assert!(err.contains("item 1"), "unexpected message: {}", err);
    }

    fn person_schema() -> Schema {
        Schema::Record(vec![
            ("name".into(), Schema::Primitive(Primitive::Str)),
            ("age".into(), Schema::Primitive(Primitive::U32)),
        ])
    }

    #[test]
    fn decodes_record_fields_by_their_own_schemas() {
        let decoded = decode(&person_schema(), &json!({"name": "ada", "age": 36})).unwrap();
        assert_eq!(
            decoded,
            Value::Map(vec![
                (Value::Str("name".into()), Value::Str("ada".into())),
                (Value::Str("age".into()), Value::U32(36)),
            ])
        );
    }

    #[test]
    fn record_decode_rejects_missing_unknown_and_mistyped_fields() {
        assert!(
            decode(&person_schema(), &json!({"name": "ada"}))
                .unwrap_err()
                .contains("missing field \"age\"")
        );
        assert!(
            decode(&person_schema(), &json!({"name": "ada", "age": 36, "x": 1}))
                .unwrap_err()
                .contains("unknown field \"x\"")
        );
        assert!(
            decode(&person_schema(), &json!({"name": "ada", "age": "old"}))
                .unwrap_err()
                .contains("field \"age\"")
        );
        assert!(
            decode(&person_schema(), &json!([1]))
                .unwrap_err()
                .contains("expected object")
        );
    }

    fn optional_str() -> Schema {
        Schema::Optional(Box::new(Schema::Primitive(Primitive::Str)))
    }

    #[test]
    fn optional_schemas_decode_null_and_present_values() {
        assert_eq!(decode(&optional_str(), &json!(null)).unwrap(), Value::Null);
        assert_eq!(
            decode(&optional_str(), &json!("hi")).unwrap(),
            Value::Str("hi".into())
        );
        assert!(decode(&optional_str(), &json!(5)).is_err());
    }

    #[test]
    fn optional_record_fields_may_be_omitted_or_null() {
        let schema = Schema::Record(vec![
            ("name".into(), Schema::Primitive(Primitive::Str)),
            ("nickname".into(), optional_str()),
        ]);

        for body in [json!({"name": "ada"}), json!({"name": "ada", "nickname": null})] {
            assert_eq!(
                decode(&schema, &body).unwrap(),
                Value::Map(vec![
                    (Value::Str("name".into()), Value::Str("ada".into())),
                    (Value::Str("nickname".into()), Value::Null),
                ])
            );
        }

        assert!(
            decode(&schema, &json!({"nickname": "lovelace"}))
                .unwrap_err()
                .contains("missing field \"name\"")
        );
    }

    #[test]
    fn null_encodes_to_json_null() {
        assert_eq!(encode(&Value::Null), json!(null));
        assert_eq!(
            decode(&optional_str(), &encode(&Value::Null)).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn constrained_schemas_decode_through_their_inner_schema() {
        let schema = Schema::Constrained(
            Box::new(Schema::Primitive(Primitive::I64)),
            vec![crate::schema::Constraint::Min(0.0)],
        );
        assert_eq!(decode(&schema, &json!(-5)).unwrap(), Value::I64(-5));
    }
}
