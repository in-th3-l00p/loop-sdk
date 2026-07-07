/* discovers the `<version>_<name>.sql` files a project keeps under
migrations/, the CLI's on-disk convention for authoring migrations */

use std::fs;
use std::path::{Path, PathBuf};

use lib::database::Migration;

pub struct MigrationFile {
    pub version: i64,
    pub name: String,
    pub path: PathBuf,
}

pub fn dir(project_dir: &Path) -> PathBuf {
    project_dir.join("migrations")
}

pub fn discover(project_dir: &Path) -> Result<Vec<MigrationFile>, String> {
    let dir = dir(project_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("sql") {
            continue;
        }

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let Some((version, name)) = stem.split_once('_') else {
            return Err(format!(
                "{}: migration filenames must look like <version>_<name>.sql",
                path.display()
            ));
        };
        let version: i64 = version
            .parse()
            .map_err(|_| format!("{}: version prefix must be an integer", path.display()))?;

        files.push(MigrationFile {
            version,
            name: name.to_string(),
            path,
        });
    }

    files.sort_by_key(|file| file.version);
    Ok(files)
}

pub fn load(project_dir: &Path) -> Result<Vec<Migration>, String> {
    discover(project_dir)?
        .into_iter()
        .map(|file| {
            let sql = fs::read_to_string(&file.path).map_err(|e| format!("{}: {e}", file.path.display()))?;
            Ok(Migration::new(file.version, file.name, sql))
        })
        .collect()
}

pub fn next_version(project_dir: &Path) -> Result<i64, String> {
    Ok(discover(project_dir)?.last().map_or(1, |file| file.version + 1))
}

pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_separators_and_case() {
        assert_eq!(slugify("Add Users Table"), "add_users_table");
        assert_eq!(slugify("add-bio!!"), "add_bio");
        assert_eq!(slugify("   "), "");
    }

    #[test]
    fn discover_returns_empty_for_a_project_without_a_migrations_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn discover_parses_version_and_name_and_sorts_by_version() {
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("migrations");
        fs::create_dir_all(&migrations).unwrap();
        fs::write(migrations.join("0002_add_bio.sql"), "ALTER TABLE users ADD COLUMN bio TEXT").unwrap();
        fs::write(migrations.join("0001_create_users.sql"), "CREATE TABLE users (id BIGINT)").unwrap();

        let files = discover(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].version, 1);
        assert_eq!(files[0].name, "create_users");
        assert_eq!(files[1].version, 2);
        assert_eq!(files[1].name, "add_bio");
    }

    #[test]
    fn next_version_continues_after_the_highest_existing_version() {
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("migrations");
        fs::create_dir_all(&migrations).unwrap();
        fs::write(migrations.join("0001_create_users.sql"), "").unwrap();
        fs::write(migrations.join("0003_add_email.sql"), "").unwrap();

        assert_eq!(next_version(dir.path()).unwrap(), 4);
    }

    #[test]
    fn next_version_starts_at_one_for_a_fresh_project() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_version(dir.path()).unwrap(), 1);
    }
}
