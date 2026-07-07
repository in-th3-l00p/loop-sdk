use lib::prelude::*;

#[derive(Schema)]
struct NewTodo {
    #[check(min_len = 1)]
    title: String,
}

#[derive(Schema)]
struct TodoUpdate {
    #[check(min_len = 1)]
    title: String,
    done: bool,
    #[check(min = 1, max = 3)]
    priority: i32,
}

#[derive(Schema)]
struct Todo {
    id: i64,
    title: String,
    done: bool,
    priority: i32,
}

#[rest(post, "/todos")]
fn create(todo: NewTodo) -> Result<Todo, HandlerError> {
    lib::database::query(
        "INSERT INTO todos (title, done, priority) VALUES (?, ?, ?) \
         RETURNING id, title, done, priority",
    )
    .bind(todo.title)
    .bind(false)
    .bind(2)
    .fetch_one()
    .map_err(Into::into)
}

#[rest(get, "/todos")]
fn list() -> Result<Vec<Todo>, HandlerError> {
    lib::database::query("SELECT id, title, done, priority FROM todos ORDER BY id")
        .fetch_all()
        .map_err(Into::into)
}

#[rest(get, "/todos/{id}")]
fn get(id: i64) -> Result<Todo, HandlerError> {
    lib::database::query("SELECT id, title, done, priority FROM todos WHERE id = ?")
        .bind(id)
        .fetch_optional()?
        .ok_or_else(|| not_found(id))
}

#[rest(put, "/todos/{id}")]
fn update(id: i64, todo: TodoUpdate) -> Result<Todo, HandlerError> {
    lib::database::query(
        "UPDATE todos SET title = ?, done = ?, priority = ? WHERE id = ? \
         RETURNING id, title, done, priority",
    )
    .bind(todo.title)
    .bind(todo.done)
    .bind(todo.priority)
    .bind(id)
    .fetch_optional()?
    .ok_or_else(|| not_found(id))
}

#[rest(delete, "/todos/{id}")]
fn delete(id: i64) -> Result<bool, HandlerError> {
    let deleted = lib::database::query("DELETE FROM todos WHERE id = ?")
        .bind(id)
        .execute()?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(true)
}

fn not_found(id: i64) -> HandlerError {
    with_status(StatusCode::NOT_FOUND, format!("no todo with id {id}"))
}

fn main() {
    lib::server::run();
}
