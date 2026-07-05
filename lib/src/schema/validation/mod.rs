mod constraint;
mod error;
mod validator;
mod value;

pub use constraint::Constraint;
pub use error::ValidationError;
pub use value::Value;

use super::Schema;

impl Schema {
    pub fn validate(&self, value: &Value) -> Result<(), ValidationError> {
        validator::validate(self, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Primitive;

    #[test]
    fn schema_validate_accepts_matching_value() {
        let schema = Schema::Primitive(Primitive::Bool);
        assert!(schema.validate(&Value::Bool(true)).is_ok());
    }

    #[test]
    fn schema_validate_rejects_mismatched_value() {
        let schema = Schema::Primitive(Primitive::Bool);
        assert!(schema.validate(&Value::Str("x".into())).is_err());
    }
}
