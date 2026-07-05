# examples

Three loop projects showcasing the SDK's initial feature set. Each is a
standalone project managed by the loop CLI — `cd` into one and run `loop dev`
(they all bind port 3000, so run one at a time).

| Example                              | Showcases                                                          |
| ------------------------------------ | ------------------------------------------------------------------ |
| [motorcycle-shop](motorcycle-shop/)  | REST CRUD: typed signatures, path/body params, shared state        |
| [circle-game](circle-game/)          | `Live` (WebSocket) endpoints: multiplayer board pushed at ~20 fps  |
| [ollama-stream](ollama-stream/)      | `Sse` endpoints: forwarding LLM tokens as they generate            |
