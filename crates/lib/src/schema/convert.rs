/* conversions between Rust types and Schema/Value, so endpoint signatures
can be inferred from ordinary function signatures */

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use super::{Primitive, Schema, Value};
use crate::server::endpoint::HandlerError;

pub trait AsSchema {
    fn schema() -> Schema;
}

pub trait IntoValue: AsSchema {
    fn into_value(self) -> Value;
}

pub trait FromValue: AsSchema + Sized {
    fn from_value(value: Value) -> Result<Self, HandlerError>;
}

/// A binary payload; distinct from `Vec<u8>` so that `Vec<T>` can stay the
/// generic list type.
#[derive(Debug, Clone, PartialEq)]
pub struct Blob(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq)]
pub struct Date(pub String);

fn mismatch<T>(expected: &Primitive, found: &Value) -> Result<T, HandlerError> {
    Err(format!("expected {}, got {}", expected.kind(), found.kind()).into())
}

macro_rules! primitive_conversions {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl AsSchema for $ty {
                fn schema() -> Schema {
                    Schema::Primitive(Primitive::$variant)
                }
            }

            impl IntoValue for $ty {
                fn into_value(self) -> Value {
                    Value::$variant(self)
                }
            }

            impl FromValue for $ty {
                fn from_value(value: Value) -> Result<Self, HandlerError> {
                    match value {
                        Value::$variant(inner) => Ok(inner),
                        other => mismatch(&Primitive::$variant, &other),
                    }
                }
            }
        )*
    };
}

primitive_conversions! {
    bool => Bool,
    i32 => I32,
    u32 => U32,
    i64 => I64,
    u64 => U64,
    f32 => F32,
    f64 => F64,
    String => Str,
}

impl AsSchema for Blob {
    fn schema() -> Schema {
        Schema::Primitive(Primitive::Blob)
    }
}

impl IntoValue for Blob {
    fn into_value(self) -> Value {
        Value::Blob(self.0)
    }
}

impl FromValue for Blob {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::Blob(bytes) => Ok(Blob(bytes)),
            other => mismatch(&Primitive::Blob, &other),
        }
    }
}

impl AsSchema for Date {
    fn schema() -> Schema {
        Schema::Primitive(Primitive::Date)
    }
}

impl IntoValue for Date {
    fn into_value(self) -> Value {
        Value::Date(self.0)
    }
}

impl FromValue for Date {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::Date(date) => Ok(Date(date)),
            other => mismatch(&Primitive::Date, &other),
        }
    }
}

impl<T: AsSchema> AsSchema for Option<T> {
    fn schema() -> Schema {
        Schema::Optional(Box::new(T::schema()))
    }
}

impl<T: IntoValue> IntoValue for Option<T> {
    fn into_value(self) -> Value {
        match self {
            Some(inner) => inner.into_value(),
            None => Value::Null,
        }
    }
}

impl<T: FromValue> FromValue for Option<T> {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::Null => Ok(None),
            present => T::from_value(present).map(Some),
        }
    }
}

impl<T: AsSchema> AsSchema for Vec<T> {
    fn schema() -> Schema {
        Schema::List(Box::new(T::schema()))
    }
}

impl<T: IntoValue> IntoValue for Vec<T> {
    fn into_value(self) -> Value {
        Value::List(self.into_iter().map(IntoValue::into_value).collect())
    }
}

impl<T: FromValue> FromValue for Vec<T> {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::List(items) => items.into_iter().map(T::from_value).collect(),
            other => Err(format!("expected list, got {}", other.kind()).into()),
        }
    }
}

macro_rules! map_conversions {
    ($($map:ident: $($bound:path),+);* $(;)?) => {
        $(
            impl<K: AsSchema, V: AsSchema> AsSchema for $map<K, V> {
                fn schema() -> Schema {
                    Schema::Map(Box::new(K::schema()), Box::new(V::schema()))
                }
            }

            impl<K: IntoValue, V: IntoValue> IntoValue for $map<K, V> {
                fn into_value(self) -> Value {
                    Value::Map(
                        self.into_iter()
                            .map(|(k, v)| (k.into_value(), v.into_value()))
                            .collect(),
                    )
                }
            }

            impl<K: FromValue + $($bound +)+ , V: FromValue> FromValue for $map<K, V> {
                fn from_value(value: Value) -> Result<Self, HandlerError> {
                    match value {
                        Value::Map(entries) => entries
                            .into_iter()
                            .map(|(k, v)| Ok((K::from_value(k)?, V::from_value(v)?)))
                            .collect(),
                        other => Err(format!("expected map, got {}", other.kind()).into()),
                    }
                }
            }
        )*
    };
}

map_conversions! {
    BTreeMap: Ord;
    HashMap: Hash, Eq;
}

/// Hand-written equivalent of what #[derive(Schema)] emits, for record types
/// defined inside this crate — the derive's generated paths only resolve for
/// consumers of `lib`.
#[allow(unused_macros)] // used by the eth and auth features
macro_rules! record_conversions {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        impl crate::schema::AsSchema for $name {
            fn schema() -> crate::schema::Schema {
                crate::schema::Schema::Record(vec![
                    $((
                        stringify!($field).to_string(),
                        <$ty as crate::schema::AsSchema>::schema(),
                    )),*
                ])
            }
        }

        impl crate::schema::IntoValue for $name {
            fn into_value(self) -> crate::schema::Value {
                crate::schema::Value::Map(vec![
                    $((
                        crate::schema::Value::Str(stringify!($field).to_string()),
                        crate::schema::IntoValue::into_value(self.$field),
                    )),*
                ])
            }
        }

        impl crate::schema::FromValue for $name {
            fn from_value(
                value: crate::schema::Value,
            ) -> Result<Self, crate::server::endpoint::HandlerError> {
                let crate::schema::Value::Map(mut entries) = value else {
                    return Err("expected a record".into());
                };
                $(
                    let $field = {
                        let label = stringify!($field);
                        let index = entries
                            .iter()
                            .position(|(key, _)| {
                                matches!(key, crate::schema::Value::Str(name) if name == label)
                            })
                            .ok_or_else(|| {
                                crate::server::endpoint::HandlerError::from(format!(
                                    "missing field {label:?}"
                                ))
                            })?;
                        let (_, value) = entries.swap_remove(index);
                        <$ty as crate::schema::FromValue>::from_value(value)?
                    };
                )*
                Ok($name { $($field),* })
            }
        }

        impl crate::server::endpoint::IntoHandlerOutput for $name {
            type Ok = Self;
            fn into_handler_output(
                self,
            ) -> Result<crate::schema::Value, crate::server::endpoint::HandlerError> {
                Ok(crate::schema::IntoValue::into_value(self))
            }
        }
    };
}

#[allow(unused_imports)]
pub(crate) use record_conversions;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_schemas_from_rust_types() {
        assert_eq!(i64::schema(), Schema::Primitive(Primitive::I64));
        assert_eq!(
            Vec::<String>::schema(),
            Schema::List(Box::new(Schema::Primitive(Primitive::Str)))
        );
        assert_eq!(
            BTreeMap::<String, Vec<f64>>::schema(),
            Schema::Map(
                Box::new(Schema::Primitive(Primitive::Str)),
                Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::F64))))
            )
        );
    }

    #[test]
    fn roundtrips_values_through_conversions() {
        let map = BTreeMap::from([("xs".to_string(), vec![1i64, 2])]);
        let value = map.clone().into_value();
        assert_eq!(
            BTreeMap::<String, Vec<i64>>::from_value(value).unwrap(),
            map
        );

        let blob = Blob(vec![0xde, 0xad]);
        assert_eq!(Blob::from_value(blob.clone().into_value()).unwrap(), blob);
    }

    #[test]
    fn rejects_mismatched_values() {
        assert!(i64::from_value(Value::Str("nope".into())).is_err());
        assert!(Vec::<i64>::from_value(Value::I64(1)).is_err());
    }

    #[test]
    fn options_infer_optional_schemas() {
        assert_eq!(
            Option::<String>::schema(),
            Schema::Optional(Box::new(Schema::Primitive(Primitive::Str)))
        );
        assert_eq!(
            Vec::<Option<i64>>::schema(),
            Schema::List(Box::new(Schema::Optional(Box::new(Schema::Primitive(
                Primitive::I64
            )))))
        );
    }

    #[test]
    fn roundtrips_options_through_null_and_present_values() {
        assert_eq!(Some(7i64).into_value(), Value::I64(7));
        assert_eq!(None::<i64>.into_value(), Value::Null);
        assert_eq!(Option::<i64>::from_value(Value::I64(7)).unwrap(), Some(7));
        assert_eq!(Option::<i64>::from_value(Value::Null).unwrap(), None);
    }

    #[test]
    fn present_options_still_typecheck_their_inner_value() {
        assert!(Option::<i64>::from_value(Value::Str("nope".into())).is_err());
    }
}
