use std::collections::HashMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::Value as Json;

use super::json;
use crate::endpoint::Signature;
use crate::endpoint::engine::EngineError;
use crate::schema::{Primitive, Schema, Value};

pub fn collect_args(
    signature: &Signature,
    path: &HashMap<String, String>,
    query: &HashMap<String, String>,
    body: Option<&Json>,
) -> Result<Vec<Value>, EngineError> {
    let body_param = single_record_param(signature);

    signature
        .params
        .iter()
        .map(|param| {
            let whole_body = body_param == Some(param.name.as_str());
            let value = if let Some(raw) = path.get(&param.name) {
                decode_scalar(&param.name, &param.schema, raw)?
            } else if whole_body {
                let body = body.ok_or_else(|| EngineError::MissingParam(param.name.clone()))?;
                json::decode(&param.schema, body)
                    .map_err(|e| EngineError::Decode(format!("parameter {:?}: {e}", param.name)))?
            } else if let Some(value) = body.and_then(|b| b.get(&param.name)) {
                json::decode(&param.schema, value)
                    .map_err(|e| EngineError::Decode(format!("parameter {:?}: {e}", param.name)))?
            } else if let Some(raw) = query.get(&param.name) {
                decode_scalar(&param.name, &param.schema, raw)?
            } else if param.schema.accepts_null() {
                Value::Null
            } else {
                return Err(EngineError::MissingParam(param.name.clone()));
            };

            Ok(value)
        })
        .collect()
}

fn single_record_param(signature: &Signature) -> Option<&str> {
    let mut records = signature
        .params
        .iter()
        .filter(|param| matches!(param.schema.base(), Schema::Record(_)));
    match (records.next(), records.next()) {
        (Some(param), None) => Some(&param.name),
        _ => None,
    }
}

fn decode_scalar(name: &str, schema: &Schema, raw: &str) -> Result<Value, EngineError> {
    let error = |expected: &str| {
        EngineError::Decode(format!(
            "parameter {name:?}: expected {expected}, found {raw:?}"
        ))
    };

    // a present optional parameter parses as its inner type
    let mut base = schema.base();
    while let Schema::Optional(inner) = base {
        base = inner.base();
    }
    let Schema::Primitive(primitive) = base else {
        return Err(error(
            "a primitive (path/query parameters cannot be lists, maps or records)",
        ));
    };

    let parsed = match primitive {
        Primitive::Bool => raw.parse().ok().map(Value::Bool),
        Primitive::I32 => raw.parse().ok().map(Value::I32),
        Primitive::U32 => raw.parse().ok().map(Value::U32),
        Primitive::I64 => raw.parse().ok().map(Value::I64),
        Primitive::U64 => raw.parse().ok().map(Value::U64),
        Primitive::F32 => raw.parse().ok().map(Value::F32),
        Primitive::F64 => raw.parse().ok().map(Value::F64),
        Primitive::Str => Some(Value::Str(raw.into())),
        Primitive::Date => Some(Value::Date(raw.into())),
        Primitive::Blob => BASE64.decode(raw).ok().map(Value::Blob),
    };

    parsed.ok_or_else(|| error(primitive.kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::Parameter;
    use serde_json::json;

    fn signature(params: Vec<(&str, Schema)>) -> Signature {
        Signature {
            params: params
                .into_iter()
                .map(|(name, schema)| Parameter {
                    name: name.into(),
                    schema,
                })
                .collect(),
            output: Schema::Primitive(Primitive::I64),
        }
    }

    #[test]
    fn resolves_args_with_path_over_body_over_query_precedence() {
        let signature = signature(vec![
            ("a", Schema::Primitive(Primitive::I64)),
            ("b", Schema::Primitive(Primitive::I64)),
            ("c", Schema::Primitive(Primitive::I64)),
        ]);
        let path = HashMap::from([("a".to_string(), "1".to_string())]);
        let query = HashMap::from([
            ("a".to_string(), "10".to_string()),
            ("b".to_string(), "20".to_string()),
            ("c".to_string(), "30".to_string()),
        ]);
        let body = json!({"a": 100, "b": 200});

        let args = collect_args(&signature, &path, &query, Some(&body)).unwrap();
        assert_eq!(args, vec![Value::I64(1), Value::I64(200), Value::I64(30)]);
    }

    #[test]
    fn reports_missing_parameter_by_name() {
        let signature = signature(vec![("a", Schema::Primitive(Primitive::I64))]);
        let err = collect_args(&signature, &HashMap::new(), &HashMap::new(), None).unwrap_err();
        assert!(matches!(err, EngineError::MissingParam(name) if name == "a"));
    }

    #[test]
    fn rejects_non_primitive_query_parameter() {
        let signature = signature(vec![(
            "xs",
            Schema::List(Box::new(Schema::Primitive(Primitive::I64))),
        )]);
        let query = HashMap::from([("xs".to_string(), "[1,2]".to_string())]);
        assert!(matches!(
            collect_args(&signature, &HashMap::new(), &query, None),
            Err(EngineError::Decode(_))
        ));
    }

    #[test]
    fn decodes_structured_body_parameter() {
        let signature = signature(vec![(
            "xs",
            Schema::List(Box::new(Schema::Primitive(Primitive::I64))),
        )]);
        let body = json!({"xs": [1, 2, 3]});
        let args = collect_args(&signature, &HashMap::new(), &HashMap::new(), Some(&body)).unwrap();
        assert_eq!(
            args,
            vec![Value::List(vec![
                Value::I64(1),
                Value::I64(2),
                Value::I64(3)
            ])]
        );
    }

    fn record() -> Schema {
        Schema::Record(vec![("name".into(), Schema::Primitive(Primitive::Str))])
    }

    #[test]
    fn single_record_param_takes_the_whole_body() {
        let signature = signature(vec![
            ("id", Schema::Primitive(Primitive::U64)),
            ("person", record()),
        ]);
        let path = HashMap::from([("id".to_string(), "7".to_string())]);
        let body = json!({"name": "ada"});

        let args = collect_args(&signature, &path, &HashMap::new(), Some(&body)).unwrap();
        assert_eq!(
            args,
            vec![
                Value::U64(7),
                Value::Map(vec![(Value::Str("name".into()), Value::Str("ada".into()))]),
            ]
        );
    }

    #[test]
    fn single_record_param_requires_a_body() {
        let signature = signature(vec![("person", record())]);
        assert!(matches!(
            collect_args(&signature, &HashMap::new(), &HashMap::new(), None),
            Err(EngineError::MissingParam(name)) if name == "person"
        ));
    }

    #[test]
    fn multiple_record_params_stay_keyed_by_name() {
        let signature = signature(vec![("a", record()), ("b", record())]);
        let body = json!({"a": {"name": "x"}, "b": {"name": "y"}});

        let args = collect_args(&signature, &HashMap::new(), &HashMap::new(), Some(&body)).unwrap();
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn absent_optional_parameters_resolve_to_null() {
        let signature = signature(vec![
            ("a", Schema::Primitive(Primitive::I64)),
            (
                "b",
                Schema::Optional(Box::new(Schema::Primitive(Primitive::I64))),
            ),
        ]);
        let query = HashMap::from([("a".to_string(), "1".to_string())]);

        let args = collect_args(&signature, &HashMap::new(), &query, None).unwrap();
        assert_eq!(args, vec![Value::I64(1), Value::Null]);
    }

    #[test]
    fn present_optional_parameters_decode_as_their_inner_type() {
        let signature = signature(vec![(
            "b",
            Schema::Optional(Box::new(Schema::Primitive(Primitive::I64))),
        )]);
        let query = HashMap::from([("b".to_string(), "5".to_string())]);

        let args = collect_args(&signature, &HashMap::new(), &query, None).unwrap();
        assert_eq!(args, vec![Value::I64(5)]);

        let bad = HashMap::from([("b".to_string(), "x".to_string())]);
        assert!(matches!(
            collect_args(&signature, &HashMap::new(), &bad, None),
            Err(EngineError::Decode(_))
        ));
    }

    #[test]
    fn constrained_scalars_decode_in_path_and_query() {
        let signature = signature(vec![(
            "age",
            Schema::Constrained(
                Box::new(Schema::Primitive(Primitive::U32)),
                vec![crate::schema::Constraint::Min(18.0)],
            ),
        )]);
        let query = HashMap::from([("age".to_string(), "21".to_string())]);

        let args = collect_args(&signature, &HashMap::new(), &query, None).unwrap();
        assert_eq!(args, vec![Value::U32(21)]);
    }

    #[test]
    fn record_params_cannot_come_from_the_query() {
        let signature = signature(vec![("person", record()), ("also", record())]);
        let query = HashMap::from([
            ("person".to_string(), "x".to_string()),
            ("also".to_string(), "y".to_string()),
        ]);
        assert!(matches!(
            collect_args(&signature, &HashMap::new(), &query, None),
            Err(EngineError::Decode(_))
        ));
    }
}
