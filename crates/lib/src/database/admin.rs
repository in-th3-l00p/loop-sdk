use super::config::{Config, Driver};
use super::error::DatabaseError;

pub async fn create_database(config: &Config) -> Result<(), DatabaseError> {
    match config.driver {
        Driver::Sqlite => create_sqlite(&config.url),
        Driver::Postgres => create_postgres(&config.url).await,
    }
}

pub async fn drop_database(config: &Config) -> Result<(), DatabaseError> {
    match config.driver {
        Driver::Sqlite => drop_sqlite(&config.url),
        Driver::Postgres => drop_postgres(&config.url).await,
    }
}

fn create_sqlite(url: &str) -> Result<(), DatabaseError> {
    #[cfg(feature = "db-sqlite")]
    {
        let Some(path) = sqlite_path(url) else {
            return Ok(());
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(DatabaseError::Io)?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&path)
            .map(|_| ())
            .map_err(DatabaseError::Io)
    }
    #[cfg(not(feature = "db-sqlite"))]
    {
        let _ = url;
        Err(not_compiled("sqlite", "db-sqlite"))
    }
}

fn drop_sqlite(url: &str) -> Result<(), DatabaseError> {
    #[cfg(feature = "db-sqlite")]
    {
        let Some(path) = sqlite_path(url) else {
            return Ok(());
        };
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            let candidate = std::path::PathBuf::from(name);
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(DatabaseError::Io(e)),
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "db-sqlite"))]
    {
        let _ = url;
        Err(not_compiled("sqlite", "db-sqlite"))
    }
}

#[cfg(feature = "db-sqlite")]
fn sqlite_path(url: &str) -> Option<std::path::PathBuf> {
    if url.contains(":memory:") {
        return None;
    }
    let rest = url.strip_prefix("sqlite:")?;
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    let rest = rest.split('?').next().unwrap_or(rest);
    Some(std::path::PathBuf::from(rest))
}

async fn create_postgres(url: &str) -> Result<(), DatabaseError> {
    #[cfg(feature = "db-postgres")]
    {
        postgres_admin(url, |name| format!("CREATE DATABASE \"{name}\"")).await
    }
    #[cfg(not(feature = "db-postgres"))]
    {
        let _ = url;
        Err(not_compiled("postgres", "db-postgres"))
    }
}

async fn drop_postgres(url: &str) -> Result<(), DatabaseError> {
    #[cfg(feature = "db-postgres")]
    {
        postgres_admin(url, |name| format!("DROP DATABASE IF EXISTS \"{name}\"")).await
    }
    #[cfg(not(feature = "db-postgres"))]
    {
        let _ = url;
        Err(not_compiled("postgres", "db-postgres"))
    }
}

#[cfg(feature = "db-postgres")]
async fn postgres_admin(
    url: &str,
    statement: impl FnOnce(&str) -> String,
) -> Result<(), DatabaseError> {
    use std::str::FromStr as _;

    use sqlx::Executor as _;
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

    let options = PgConnectOptions::from_str(url).map_err(DatabaseError::Connect)?;
    let name = options
        .get_database()
        .ok_or_else(|| DatabaseError::Unsupported("postgres url has no database name".into()))?
        .to_string();

    let admin_options = options.database("postgres");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_options)
        .await
        .map_err(DatabaseError::Connect)?;

    pool.execute(statement(&name).as_str())
        .await
        .map_err(DatabaseError::Query)?;
    Ok(())
}

#[cfg(not(all(feature = "db-sqlite", feature = "db-postgres")))]
fn not_compiled(name: &str, feature: &str) -> DatabaseError {
    DatabaseError::Unavailable(format!(
        "{name} support is not compiled in (enable the {feature} feature)"
    ))
}

#[cfg(all(test, feature = "db-sqlite"))]
mod tests {
    use super::*;

    fn unique_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("loop_admin_test_{}_{nanos}_{name}", std::process::id()))
    }

    #[test]
    fn create_sqlite_makes_the_file_and_parent_dirs() {
        let nested = unique_path("nested").join("create.db");
        let url = format!("sqlite:{}", nested.display());

        create_sqlite(&url).unwrap();

        assert!(nested.exists());
        std::fs::remove_dir_all(nested.parent().unwrap()).unwrap();
    }

    #[test]
    fn drop_sqlite_removes_the_file_and_sidecars() {
        let path = unique_path("drop.db");
        std::fs::write(&path, b"").unwrap();
        for suffix in ["-wal", "-shm"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            std::fs::write(std::path::PathBuf::from(name), b"").unwrap();
        }
        let url = format!("sqlite:{}", path.display());

        drop_sqlite(&url).unwrap();

        assert!(!path.exists());
        for suffix in ["-wal", "-shm"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            assert!(!std::path::PathBuf::from(name).exists());
        }
    }

    #[test]
    fn memory_url_is_a_no_op() {
        create_sqlite("sqlite::memory:").unwrap();
        drop_sqlite("sqlite::memory:").unwrap();
    }
}
