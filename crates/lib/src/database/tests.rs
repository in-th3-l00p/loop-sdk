use super::{Config, Database, DatabaseError, Driver, Migration};
use crate::server::endpoint::HandlerError;
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
async fn multi_statement_migrations_apply_atomically() {
    let db = fresh().await;
    let split = Migration::new(
        2,
        "split_tables",
        "CREATE TABLE tags (id BIGSERIAL PRIMARY KEY, label TEXT NOT NULL);
         CREATE TABLE user_tags (user_id BIGINT NOT NULL, tag_id BIGINT NOT NULL);",
    );
    assert_eq!(db.migrate(&[create_users(), split]).await.unwrap(), 1);

    db.query("INSERT INTO tags (label) VALUES (?)")
        .bind("admin".to_string())
        .execute_async()
        .await
        .unwrap();
    let count: i64 = db
        .query("SELECT count(*) FROM user_tags")
        .fetch_one_async()
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn failed_migrations_are_not_recorded() {
    let db = fresh().await;
    let broken = Migration::new(2, "broken", "CREATE TABLE broken (id NOT A TYPE!!)");
    assert!(matches!(
        db.migrate(&[create_users(), broken]).await,
        Err(DatabaseError::Migration { .. })
    ));

    let fixed = Migration::new(2, "fixed", "CREATE TABLE fixed (id BIGINT)");
    assert_eq!(db.migrate(&[create_users(), fixed]).await.unwrap(), 1);
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

async fn ledger_db() -> Database {
    let db = Database::connect(&Config::from_url("sqlite::memory:"))
        .await
        .unwrap();
    db.raw("CREATE TABLE ledger (id TEXT PRIMARY KEY, balance BIGINT NOT NULL)")
        .await
        .unwrap();
    for (id, balance) in [("a", 100i64), ("b", 0)] {
        db.query("INSERT INTO ledger (id, balance) VALUES (?, ?)")
            .bind(id.to_string())
            .bind(balance)
            .execute_async()
            .await
            .unwrap();
    }
    db
}

async fn balance(db: &Database, id: &str) -> i64 {
    db.query("SELECT balance FROM ledger WHERE id = ?")
        .bind(id.to_string())
        .fetch_one_async()
        .await
        .unwrap()
}

#[tokio::test]
async fn atomic_batches_commit_together() {
    let db = ledger_db().await;

    db.atomic()
        .guard("UPDATE ledger SET balance = balance - ? WHERE id = ? AND balance >= ?")
        .bind(30i64)
        .bind("a".to_string())
        .bind(30i64)
        .query("UPDATE ledger SET balance = balance + ? WHERE id = ?")
        .bind(30i64)
        .bind("b".to_string())
        .execute_async()
        .await
        .unwrap();

    assert_eq!(balance(&db, "a").await, 70);
    assert_eq!(balance(&db, "b").await, 30);
}

#[tokio::test]
async fn failed_guards_roll_back_the_whole_batch() {
    let db = ledger_db().await;

    // debit exceeds the balance: the guard matches no row
    let result = db
        .atomic()
        .guard("UPDATE ledger SET balance = balance - ? WHERE id = ? AND balance >= ?")
        .bind(1000i64)
        .bind("a".to_string())
        .bind(1000i64)
        .query("UPDATE ledger SET balance = balance + ? WHERE id = ?")
        .bind(1000i64)
        .bind("b".to_string())
        .execute_async()
        .await;

    assert!(matches!(result, Err(DatabaseError::Guard(_))), "{result:?}");
    // nothing moved
    assert_eq!(balance(&db, "a").await, 100);
    assert_eq!(balance(&db, "b").await, 0);
}

#[tokio::test]
async fn guards_only_bite_when_their_own_statement_misses() {
    let db = ledger_db().await;

    // a later guard failing must also undo the earlier successful debit
    let result = db
        .atomic()
        .query("UPDATE ledger SET balance = balance - 10 WHERE id = 'a'")
        .guard("UPDATE ledger SET balance = balance + 10 WHERE id = 'missing'")
        .execute_async()
        .await;

    assert!(matches!(result, Err(DatabaseError::Guard(_))));
    assert_eq!(balance(&db, "a").await, 100, "debit must be rolled back");
}
