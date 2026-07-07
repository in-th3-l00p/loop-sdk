use std::time::{SystemTime, UNIX_EPOCH};

use super::{Migration, checksum};
use crate::database::connection::Database;
use crate::database::error::DatabaseError;
use crate::schema::{Primitive, Schema, Value};

const TRACKING_TABLE: &str = "CREATE TABLE IF NOT EXISTS _loop_migrations (
    version BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at BIGINT NOT NULL
)";

const RECORD: &str =
    "INSERT INTO _loop_migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)";

pub(crate) async fn apply(
    db: &Database,
    migrations: &[Migration],
) -> Result<usize, DatabaseError> {
    let ordered = ordered(migrations)?;

    let applied = applied_versions(db).await?;
    let head = applied.last().map(|(version, _)| *version);

    let mut ran = 0;
    for migration in ordered {
        if verify_or_pending(migration, &applied)? {
            continue;
        }
        reject_out_of_order(migration, head)?;
        run_one(db, migration).await?;
        ran += 1;
    }

    Ok(ran)
}

fn ordered(migrations: &[Migration]) -> Result<Vec<&Migration>, DatabaseError> {
    let mut ordered: Vec<&Migration> = migrations.iter().collect();
    ordered.sort_by_key(|m| m.version);
    if let Some(pair) = ordered.windows(2).find(|w| w[0].version == w[1].version) {
        return Err(DatabaseError::Migration {
            name: pair[1].name.clone(),
            message: format!("duplicate version {}", pair[1].version),
        });
    }
    Ok(ordered)
}

fn verify_or_pending(
    migration: &Migration,
    applied: &[(i64, String)],
) -> Result<bool, DatabaseError> {
    let Some((_, stored)) = applied.iter().find(|(v, _)| *v == migration.version) else {
        return Ok(false);
    };
    if *stored != checksum(&migration.sql) {
        return Err(DatabaseError::Migration {
            name: migration.name.clone(),
            message: "already applied but its contents changed (edit a new migration instead)"
                .into(),
        });
    }
    Ok(true)
}

fn reject_out_of_order(migration: &Migration, head: Option<i64>) -> Result<(), DatabaseError> {
    match head {
        Some(head) if migration.version < head => Err(DatabaseError::Migration {
            name: migration.name.clone(),
            message: format!(
                "version {} is older than the newest applied migration ({head})",
                migration.version
            ),
        }),
        _ => Ok(()),
    }
}

async fn run_one(db: &Database, migration: &Migration) -> Result<(), DatabaseError> {
    let sql = db.dialect().migration_sql(&migration.sql);
    let record_sql = db.dialect().placeholders(RECORD);
    let args = [
        (
            Value::I64(migration.version),
            Schema::Primitive(Primitive::I64),
        ),
        (
            Value::Str(migration.name.clone()),
            Schema::Primitive(Primitive::Str),
        ),
        (
            Value::Str(checksum(&migration.sql)),
            Schema::Primitive(Primitive::Str),
        ),
        (
            Value::I64(epoch_seconds()),
            Schema::Primitive(Primitive::I64),
        ),
    ];
    db.migrate_step(&sql, &record_sql, &args)
        .await
        .map_err(|e| DatabaseError::Migration {
            name: migration.name.clone(),
            message: e.to_string(),
        })
}

pub async fn applied_versions(db: &Database) -> Result<Vec<(i64, String)>, DatabaseError> {
    db.raw(TRACKING_TABLE).await?;

    let schema = Schema::Record(vec![
        ("version".into(), Schema::Primitive(Primitive::I64)),
        ("checksum".into(), Schema::Primitive(Primitive::Str)),
    ]);
    let rows = db
        .fetch_values(
            "SELECT version, checksum FROM _loop_migrations ORDER BY version",
            &[],
            &schema,
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
