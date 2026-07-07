# todo-list

A REST CRUD API backed by a real SQLite database, showcasing the loop CLI's
database commands and file-based migrations instead of in-memory state.

```sh
loop db create
loop db migrate
loop dev
```

`migrations/` holds two `.sql` files applied in order — `0001_create_todos.sql`
creates the table, `0002_add_done_and_priority.sql` evolves it. Check progress
any time with:

```sh
loop migration status
```

```sh
# create
curl -X POST localhost:3000/todos \
  -H 'content-type: application/json' \
  -d '{"title": "write the README"}'

# list
curl localhost:3000/todos

# read one
curl localhost:3000/todos/1

# update (also flips done/priority)
curl -X PUT localhost:3000/todos/1 \
  -H 'content-type: application/json' \
  -d '{"title": "write the README", "done": true, "priority": 1}'

# delete
curl -X DELETE localhost:3000/todos/1
```

Each handler is a plain synchronous function that calls
`lib::database::query(...)` — no manual `async`/`await`, connection pooling,
or row-mapping boilerplate. `Todo`/`NewTodo`/`TodoUpdate` derive `Schema`,
so both the HTTP request bodies and the database rows decode through the same
typed path, and `#[check(...)]` constraints (`min_len`, `min`/`max`) validate
input before it ever reaches SQL.

To start over: `loop db reset --yes`.
