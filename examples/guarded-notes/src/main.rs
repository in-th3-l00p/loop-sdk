/* demonstrates authentication: a User parameter guards an endpoint (401
before the body runs), Option<User> makes the session optional, and the
auth routes come from the [auth] section in loop.toml — no auth code in
this file at all */

use lib::prelude::*;

#[derive(Schema)]
struct Profile {
    id: String,
    email: String,
}

#[derive(Schema)]
struct Note {
    id: i64,
    text: String,
}

/// Who am I? Requires a session: register or log in first.
#[rest(get, "/me")]
fn me(user: User) -> Profile {
    Profile {
        id: user.id().to_string(),
        email: user.email().unwrap_or_default(),
    }
}

#[rest(post, "/notes")]
fn create(user: User, #[check(min_len = 1, max_len = 500)] text: String) -> Result<Note, HandlerError> {
    let author = user.id().to_string();
    lib::database::query("INSERT INTO notes (author, text) VALUES (?, ?)")
        .bind(author.clone())
        .bind(text)
        .execute()?;
    Ok(
        lib::database::query("SELECT id, text FROM notes WHERE author = ? ORDER BY id DESC")
            .bind(author)
            .fetch_one()?,
    )
}

/// Only your own notes — the session scopes the query.
#[rest(get, "/notes")]
fn list(user: User) -> Result<Vec<Note>, HandlerError> {
    Ok(
        lib::database::query("SELECT id, text FROM notes WHERE author = ? ORDER BY id")
            .bind(user.id().to_string())
            .fetch_all()?,
    )
}

#[rest(delete, "/notes/{id}")]
fn remove(user: User, id: i64) -> Result<bool, HandlerError> {
    let deleted = lib::database::query("DELETE FROM notes WHERE id = ? AND author = ?")
        .bind(id)
        .bind(user.id().to_string())
        .execute()?;
    if deleted == 0 {
        return Err(with_status(
            StatusCode::NOT_FOUND,
            format!("no note {id} of yours"),
        ));
    }
    Ok(true)
}

/// Works with or without a session — Option<User> never 401s.
#[rest(get, "/lobby")]
fn lobby(viewer: Option<User>) -> Result<String, HandlerError> {
    let notes: Vec<Note> = lib::database::query("SELECT id, text FROM notes").fetch_all()?;
    let name = viewer
        .and_then(|user| user.email())
        .unwrap_or_else(|| "guest".to_string());
    Ok(format!(
        "hello {name} — this server holds {} note(s)",
        notes.len()
    ))
}

fn main() {
    lib::server::run();
}
