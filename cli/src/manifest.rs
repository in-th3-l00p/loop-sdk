/* interpretation of a loop project's manifest (loop.toml) */

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub dev: Dev,
}

#[derive(Deserialize, Default)]
pub struct Dev {
    pub port: Option<u16>,
}

#[derive(Debug)]
pub enum ManifestError {
    Io(PathBuf, std::io::Error),
    Invalid(toml::de::Error),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io(path, e) => {
                write!(f, "{}: {e} (is this a loop project?)", path.display())
            }
            ManifestError::Invalid(e) => write!(f, "invalid loop.toml: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

pub fn parse(dir: impl AsRef<Path>) -> Result<Manifest, ManifestError> {
    let path = dir.as_ref().join("loop.toml");
    let text = fs::read_to_string(&path).map_err(|e| ManifestError::Io(path, e))?;
    toml::from_str(&text).map_err(ManifestError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_dev_port() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[dev]\nport = 4000\n",
        )
        .unwrap();

        let manifest = parse(dir.path()).unwrap();
        assert_eq!(manifest.name, "my-api");
        assert_eq!(manifest.dev.port, Some(4000));
    }

    #[test]
    fn dev_section_is_optional() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), "name = \"my-api\"\n").unwrap();

        let manifest = parse(dir.path()).unwrap();
        assert_eq!(manifest.dev.port, None);
    }

    #[test]
    fn fails_for_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(parse(dir.path()), Err(ManifestError::Io(_, _))));
    }
}
