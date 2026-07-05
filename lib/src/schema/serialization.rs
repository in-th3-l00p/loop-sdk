use std::fs;
use std::path::Path;

use super::Schema;

impl Schema {
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = bson::serialize_to_vec(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(path)?;
        let schema = bson::deserialize_from_slice(&bytes)?;
        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Primitive;
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn saves_and_loads_primitive_schema() {
        let path = temp_path("loop_sdk_schema_test_primitive.bson");
        let schema = Schema::Primitive(Primitive::Str);

        schema.save(&path).expect("failed to save schema");
        let loaded = Schema::load(&path).expect("failed to load schema");

        assert_eq!(schema, loaded);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn saves_and_loads_nested_schema() {
        let path = temp_path("loop_sdk_schema_test_nested.bson");
        let schema = Schema::Map(
            Box::new(Schema::Primitive(Primitive::Str)),
            Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::I64)))),
        );

        schema.save(&path).expect("failed to save schema");
        let loaded = Schema::load(&path).expect("failed to load schema");

        assert_eq!(schema, loaded);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn load_fails_for_missing_file() {
        let path = temp_path("loop_sdk_schema_test_missing_file_that_does_not_exist.bson");
        assert!(Schema::load(&path).is_err());
    }
}
