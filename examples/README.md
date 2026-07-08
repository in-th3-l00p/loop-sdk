# examples

Seven loop projects showcasing the SDK's initial feature set. Each is a
standalone project managed by the loop CLI — `cd` into one and run `loop dev`
(they all bind port 3000, so run one at a time).

| Example                              | Showcases                                                            |
| ------------------------------------ | -------------------------------------------------------------------- |
| [motorcycle-shop](motorcycle-shop/)  | REST CRUD: typed signatures, path/body params, shared state          |
| [todo-list](todo-list/)              | REST CRUD backed by SQLite: file-based migrations, `loop db`         |
| [circle-game](circle-game/)          | `Live` (WebSocket) endpoints: multiplayer board pushed at ~20 fps    |
| [ollama-stream](ollama-stream/)      | `Sse` endpoints: forwarding LLM tokens as they generate              |
| [erc20-dashboard](erc20-dashboard/)  | ethereum SDK: chain primitives, `#[contract]` bindings, event feeds  |
| [guarded-notes](guarded-notes/)      | auth: `User` guards, `Option<User>`, email/password + one-time codes |
| [siwe-vault](siwe-vault/)            | auth + eth: SIWE self-custodial login, embedded wallets, `user.wallet()` |
