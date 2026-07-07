use super::{Config, Database, DatabaseError, Driver, Migration};
use crate::endpoint::HandlerError;
use crate::schema::{AsSchema, FromValue, Primitive, Schema, Value};

#[derive(Debug, PartialEq)]
struct User {
    id: i64,
    name: String,
    nickname: Option<String>,
}

impl AsSchema for User {
    fn schema() -> Schema {
        Schema::Record(vec![
            ("id".into(), Schema::Primitive(Primitive::I64)),
            ("name".into(), Schema::Primitive(Primitive::Str)),
            ("nickname".into(), Option::<String>::schema()),
        ])
    }
}

impl FromValue for User {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        let Value::Map(entries) = value else {
            return Err("expected record".into());
        };
        let [(_, id), (_, name), (_, nickname)] = entries.as_slice() else {
            return Err("bad row shape".into());
        };
        Ok(User {
            id: i64::from_value(id.clone())?,
            name: String::from_value(name.clone())?,
            nickname: Option::<String>::from_value(nickname.clone())?,
        })
    }
}

fn create_users() -> Migration {
    Migration::new(
        1,
        "create_users",
        "CREATE TABLE users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            nickname TEXT
        )",
    )
}

async fn fresh() -> Database {
    let db = Database::connect(&Config::from_url("sqlite::memory:"))
        .await
        .unwrap();
    assert_eq!(db.migrate(&[create_users()]).await.unwrap(), 1);
    db
}

async fn seed(db: &Database) {
    for (name, nickname) in [("ada", Some("countess")), ("alan", None)] {
        let inserted = db
            .query("INSERT INTO users (name, nickname) VALUES (?, ?)")
            .bind(name.to_string())
            .bind(nickname.map(str::to_string))
            .execute_async()
            .await
            .unwrap();
        assert_eq!(inserted, 1);
    }
}

#[tokio::test]
async fn migrates_inserts_and_fetches_records_with_nulls() {
    let db = fresh().await;
    seed(&db).await;

    let users: Vec<User> = db
        .query("SELECT id, name, nickname FROM users ORDER BY id")
        .fetch_all_async()
        .await
        .unwrap();
    assert_eq!(
        users,
        vec![
            User {
                id: 1,
                name: "ada".into(),
                nickname: Some("countess".into())
            },
            User {
                id: 2,
                name: "alan".into(),
                nickname: None
            },
        ]
    );
}

#[tokio::test]
async fn placeholders_bind_and_scalars_fetch() {
    let db = fresh().await;
    seed(&db).await;

    let count: i64 = db
        .query("SELECT count(*) FROM users WHERE nickname IS NOT NULL")
        .fetch_one_async()
        .await
        .unwrap();
    assert_eq!(count, 1);

    let name: Option<String> = db
        .query("SELECT name FROM users WHERE name = ?")
        .bind("alan".to_string())
        .fetch_optional_async()
        .await
        .unwrap();
    assert_eq!(name, Some("alan".into()));

    let missing: Option<String> = db
        .query("SELECT name FROM users WHERE name = ?")
        .bind("grace".to_string())
        .fetch_optional_async()
        .await
        .unwrap();
    assert_eq!(missing, None);
}

#[tokio::test]
async fn fetch_one_reports_empty_results() {
    let db = fresh().await;
    let result: Result<User, _> = db.query("SELECT * FROM users").fetch_one_async().await;
    assert!(matches!(result, Err(DatabaseError::Decode(_))));
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let db = fresh().await;
    assert_eq!(db.migrate(&[create_users()]).await.unwrap(), 0);
}

#[tokio::test]
async fn edited_applied_migrations_are_rejected() {
    let db = fresh().await;
    let edited = Migration::new(1, "create_users", "CREATE TABLE users (id BIGINT)");
    assert!(matches!(
        db.migrate(&[edited]).await,
        Err(DatabaseError::Migration { .. })
    ));
}

#[tokio::test]
async fn duplicate_versions_are_rejected() {
    let db = fresh().await;
    let dup = Migration::new(1, "also_first", "CREATE TABLE other (id BIGINT)");
    assert!(matches!(
        db.migrate(&[create_users(), dup]).await,
        Err(DatabaseError::Migration { .. })
    ));
}

#[tokio::test]
async fn later_migrations_apply_on_top() {
    let db = fresh().await;
    let add_bio = Migration::new(2, "add_bio", "ALTER TABLE users ADD COLUMN bio TEXT");
    assert_eq!(db.migrate(&[create_users(), add_bio]).await.unwrap(), 1);

    db.query("INSERT INTO users (name, bio) VALUES (?, ?)")
        .bind("grace".to_string())
        .bind("pioneer".to_string())
        .execute_async()
        .await
        .unwrap();
    let bio: Option<String> = db
        .query("SELECT bio FROM users WHERE name = ?")
        .bind("grace".to_string())
        .fetch_one_async()
        .await
        .unwrap();
    assert_eq!(bio, Some("pioneer".into()));
}

#[tokio::test]
async fn blocking_api_works_from_handler_threads() {
    let db = fresh().await;
    seed(&db).await;

    let count = tokio::task::spawn_blocking(move || {
        db.query("SELECT count(*) FROM users")
            .fetch_one::<i64>()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn config_infers_driver_from_url() {
    assert_eq!(Config::from_url("sqlite:shop.db").driver, Driver::Sqlite);
    assert_eq!(Config::from_url("shop.db").driver, Driver::Sqlite);
    assert_eq!(
        Config::from_url("postgres://localhost/shop").driver,
        Driver::Postgres
    );
    assert_eq!(
        Config::from_url("postgresql://localhost/shop").driver,
        Driver::Postgres
    );
}
