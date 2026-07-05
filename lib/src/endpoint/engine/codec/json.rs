use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map as JsonMap, Number, Value as Json};

use super::DecodeError;
use crate::schema::{Primitive, Schema, Value};

pub fn decode(schema: &Schema, json: &Json) -> Result<Value, DecodeError> {
    match schema {
        Schema::Primitive(primitive) => decode_primitive(primitive, json),
        Schema::List(item) => {
            let Json::Array(items) = json else {
                return Err(mismatch("array", json));
            };
            items
                .iter()
                .enumerate()
                .map(|(i, item_json)| {
                    decode(item, item_json).map_err(|e| DecodeError(format!("item {i}: {e}")))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List)
        }
        Schema::Map(key, value) => decode_map(key, value, json),
    }
}

fn decode_primitive(primitive: &Primitive, json: &Json) -> Result<Value, DecodeError> {
    match primitive {
        Primitive::Bool => json
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| mismatch("bool", json)),
        Primitive::I32 => json
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(Value::I32)
            .ok_or_else(|| mismatch("i32", json)),
        Primitive::U32 => json
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Value::U32)
            .ok_or_else(|| mismatch("u32", json)),
        Primitive::I64 => json
            .as_i64()
            .map(Value::I64)
            .ok_or_else(|| mismatch("i64", json)),
        Primitive::U64 => json
            .as_u64()
            .map(Value::U64)
            .ok_or_else(|| mismatch("u64", json)),
        Primitive::F32 => json
            .as_f64()
            .map(|n| Value::F32(n as f32))
            .ok_or_else(|| mismatch("f32", json)),
        Primitive::F64 => json
            .as_f64()
            .map(Value::F64)
            .ok_or_else(|| mismatch("f64", json)),
        Primitive::Str => json
            .as_str()
            .map(|s| Value::Str(s.into()))
            .ok_or_else(|| mismatch("str", json)),
        Primitive::Date => json
            .as_str()
            .map(|s| Value::Date(s.into()))
            .ok_or_else(|| mismatch("date", json)),
        Primitive::Blob => {
            let s = json.as_str().ok_or_else(|| mismatch("base64 blob", json))?;
            BASE64
                .decode(s)
                .map(Value::Blob)
                .map_err(|e| DecodeError(format!("invalid base64 blob: {e}")))
        }
    }
}

fn decode_map(key: &Schema, value: &Schema, json: &Json) -> Result<Value, DecodeError> {
    if matches!(key, Schema::Primitive(Primitive::Str)) {
        let Json::Object(object) = json else {
            return Err(mismatch("object", json));
        };
        object
            .iter()
            .map(|(k, v)| {
                let decoded =
                    decode(value, v).map_err(|e| DecodeError(format!("key {k:?}: {e}")))?;
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
                    return Err(DecodeError(format!(
                        "entry {i}: expected [key, value] pair"
                    )));
                };
                let [k, v] = pair.as_slice() else {
                    return Err(DecodeError(format!(
                        "entry {i}: expected exactly 2 elements"
                    )));
                };
                Ok((
                    decode(key, k).map_err(|e| DecodeError(format!("entry {i} key: {e}")))?,
                    decode(value, v).map_err(|e| DecodeError(format!("entry {i} value: {e}")))?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Map)
    }
}

pub fn encode(value: &Value) -> Json {
    match value {
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

fn mismatch(expected: &str, found: &Json) -> DecodeError {
    DecodeError(format!("expected {expected}, found {found}"))
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
        assert!(err.0.contains("item 1"), "unexpected message: {}", err.0);
    }
}
