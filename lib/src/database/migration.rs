use std::time::{SystemTime, UNIX_EPOCH};

use super::error::DatabaseError;
use super::{Database, Value};
use crate::schema::{Primitive, Schema};

/// One migration: portable DDL (see `Dialect::migration_sql`) applied once
/// and recorded in `_loop_migrations`.
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub sql: String,
}

impl Migration {
    pub fn new(version: i64, name: impl Into<String>, sql: impl Into<String>) -> Self {
        Migration {
            version,
            name: name.into(),
            sql: sql.into(),
        }
    }
}

/// Content fingerprint stored alongside each applied migration, so editing an
/// already-applied file is caught instead of silently ignored. FNV-1a; this
/// is an integrity check, not a security boundary.
pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sql.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

const TRACKING_TABLE: &str = "CREATE TABLE IF NOT EXISTS _loop_migrations (
    version BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at BIGINT NOT NULL
)";

const RECORD: &str =
    "INSERT INTO _loop_migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)";

fn applied_schema() -> Schema {
    Schema::Record(vec![
        ("version".into(), Schema::Primitive(Primitive::I64)),
        ("checksum".into(), Schema::Primitive(Primitive::Str)),
    ])
}

/// Applies pending migrations in version order; returns how many ran.
pub(super) async fn apply(
    db: &Database,
    migrations: &[Migration],
) -> Result<usize, DatabaseError> {
    let mut ordered: Vec<&Migration> = migrations.iter().collect();
    ordered.sort_by_key(|m| m.version);
    if let Some(pair) = ordered.windows(2).find(|w| w[0].version == w[1].version) {
        return Err(DatabaseError::Migration {
            name: pair[1].name.clone(),
            message: format!("duplicate version {}", pair[1].version),
        });
    }

    db.raw(TRACKING_TABLE).await?;
    let applied = applied_versions(db).await?;
    let head = applied.last().map(|(version, _)| *version);

    let mut ran = 0;
    for migration in ordered {
        let expected = checksum(&migration.sql);

        if let Some((_, stored)) = applied.iter().find(|(v, _)| *v == migration.version) {
            if *stored != expected {
                return Err(DatabaseError::Migration {
                    name: migration.name.clone(),
                    message: "already applied but its contents changed \
                              (edit a new migration instead)"
                        .into(),
                });
            }
            continue;
        }

        if head.is_some_and(|head| migration.version < head) {
            return Err(DatabaseError::Migration {
                name: migration.name.clone(),
                message: format!(
                    "version {} is older than the newest applied migration ({})",
                    migration.version,
                    head.unwrap()
                ),
            });
        }

        let sql = db.dialect().migration_sql(&migration.sql);
        let record_sql = db.dialect().placeholders(RECORD);
        let args = [
            (Value::I64(migration.version), Schema::Primitive(Primitive::I64)),
            (Value::Str(migration.name.clone()), Schema::Primitive(Primitive::Str)),
            (Value::Str(expected), Schema::Primitive(Primitive::Str)),
            (Value::I64(epoch_seconds()), Schema::Primitive(Primitive::I64)),
        ];
        db.migrate_step(&sql, &record_sql, &args)
            .await
            .map_err(|e| DatabaseError::Migration {
                name: migration.name.clone(),
                message: e.to_string(),
            })?;
        ran += 1;
    }

    Ok(ran)
}

async fn applied_versions(db: &Database) -> Result<Vec<(i64, String)>, DatabaseError> {
    let rows = db
        .fetch_values(
            "SELECT version, checksum FROM _loop_migrations ORDER BY version",
            &[],
            &applied_schema(),
        )
        .await?;

    rows.into_iter()
        .map(|row| match row {
            Value::Map(entries) => match entries.as_slice() {
                [(_, Value::I64(version)), (_, Value::Str(checksum))] => {
                    Ok((*version, checksum.clone()))
                }
                _ => Err(DatabaseError::Decode(
                    "malformed _loop_migrations row".into(),
                )),
            },
            _ => Err(DatabaseError::Decode(
                "malformed _loop_migrations row".into(),
            )),
        })
        .collect()
}

fn epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_are_stable_and_content_sensitive() {
        assert_eq!(checksum("CREATE TABLE t (id BIGINT)"), checksum("CREATE TABLE t (id BIGINT)"));
        assert_ne!(checksum("CREATE TABLE t (id BIGINT)"), checksum("CREATE TABLE t (id TEXT)"));
        assert_eq!(checksum(""), "cbf29ce484222325");
    }
}
