use serde::{Deserialize, Serialize};

use super::error::ValidationError;
use super::value::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    Min(f64),
    Max(f64),
    MinLen(usize),
    MaxLen(usize),
    Pattern(String),
    OneOf(Vec<Value>),
}

impl Constraint {
    pub(crate) fn check(&self, value: &Value) -> Result<(), ValidationError> {
        match self {
            Constraint::Min(min) => {
                let n = numeric(value)?;
                require(n >= *min, || format!("must be at least {min}"))
            }
            Constraint::Max(max) => {
                let n = numeric(value)?;
                require(n <= *max, || format!("must be at most {max}"))
            }
            Constraint::MinLen(min) => {
                let len = length(value)?;
                require(len >= *min, || format!("length must be at least {min}"))
            }
            Constraint::MaxLen(max) => {
                let len = length(value)?;
                require(len <= *max, || format!("length must be at most {max}"))
            }
            Constraint::Pattern(pattern) => {
                let Value::Str(s) = value else {
                    return fail(format!("pattern applies to str, got {}", value.kind()));
                };
                let regex = regex_lite::Regex::new(pattern).map_err(|_| {
                    ValidationError::Constraint(format!("invalid pattern {pattern:?}"))
                })?;
                require(regex.is_match(s), || {
                    format!("must match pattern {pattern:?}")
                })
            }
            Constraint::OneOf(allowed) => require(allowed.contains(value), || {
                "must be one of the allowed values".to_string()
            }),
        }
    }
}

fn numeric(value: &Value) -> Result<f64, ValidationError> {
    match value {
        Value::I32(n) => Ok(f64::from(*n)),
        Value::U32(n) => Ok(f64::from(*n)),
        Value::I64(n) => Ok(*n as f64),
        Value::U64(n) => Ok(*n as f64),
        Value::F32(n) => Ok(f64::from(*n)),
        Value::F64(n) => Ok(*n),
        other => Err(ValidationError::Constraint(format!(
            "bounds apply to numbers, got {}",
            other.kind()
        ))),
    }
}

fn length(value: &Value) -> Result<usize, ValidationError> {
    match value {
        Value::Str(s) => Ok(s.chars().count()),
        Value::Blob(bytes) => Ok(bytes.len()),
        Value::List(items) => Ok(items.len()),
        Value::Map(entries) => Ok(entries.len()),
        other => Err(ValidationError::Constraint(format!(
            "length applies to str, blob, list or map, got {}",
            other.kind()
        ))),
    }
}

fn require(ok: bool, message: impl FnOnce() -> String) -> Result<(), ValidationError> {
    if ok { Ok(()) } else { fail(message()) }
}

fn fail(message: String) -> Result<(), ValidationError> {
    Err(ValidationError::Constraint(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_check_every_numeric_kind() {
        for value in [
            Value::I32(5),
            Value::U32(5),
            Value::I64(5),
            Value::U64(5),
            Value::F32(5.0),
            Value::F64(5.0),
        ] {
            assert!(Constraint::Min(5.0).check(&value).is_ok());
            assert!(Constraint::Min(6.0).check(&value).is_err());
            assert!(Constraint::Max(5.0).check(&value).is_ok());
            assert!(Constraint::Max(4.0).check(&value).is_err());
        }
    }

    #[test]
    fn bounds_reject_non_numbers() {
        assert!(Constraint::Min(0.0).check(&Value::Str("x".into())).is_err());
    }

    #[test]
    fn lengths_cover_str_blob_list_and_map() {
        for value in [
            Value::Str("ab".into()),
            Value::Blob(vec![1, 2]),
            Value::List(vec![Value::Bool(true), Value::Bool(false)]),
            Value::Map(vec![
                (Value::I64(1), Value::Bool(true)),
                (Value::I64(2), Value::Bool(false)),
            ]),
        ] {
            assert!(Constraint::MinLen(2).check(&value).is_ok());
            assert!(Constraint::MinLen(3).check(&value).is_err());
            assert!(Constraint::MaxLen(2).check(&value).is_ok());
            assert!(Constraint::MaxLen(1).check(&value).is_err());
        }
    }

    #[test]
    fn string_length_counts_characters_not_bytes() {
        assert!(
            Constraint::MaxLen(2)
                .check(&Value::Str("éé".into()))
                .is_ok()
        );
    }

    #[test]
    fn length_rejects_numbers() {
        assert!(Constraint::MinLen(1).check(&Value::I64(1)).is_err());
    }

    #[test]
    fn patterns_match_strings_only() {
        let constraint = Constraint::Pattern("^[a-z]+$".into());
        assert!(constraint.check(&Value::Str("abc".into())).is_ok());
        assert!(constraint.check(&Value::Str("ABC".into())).is_err());
        assert!(constraint.check(&Value::I64(1)).is_err());
    }

    #[test]
    fn invalid_patterns_report_instead_of_panicking() {
        let err = Constraint::Pattern("[".into())
            .check(&Value::Str("x".into()))
            .unwrap_err();
        assert!(err.to_string().contains("invalid pattern"));
    }

    #[test]
    fn one_of_compares_values() {
        let constraint = Constraint::OneOf(vec![Value::Str("a".into()), Value::Str("b".into())]);
        assert!(constraint.check(&Value::Str("a".into())).is_ok());
        assert!(constraint.check(&Value::Str("c".into())).is_err());
    }
}
