# circle-game

A minimal multiplayer demo on the loop SDK: two circles on a shared board.
Moves arrive over REST; every connected client watches the board through a
`Live` (WebSocket) endpoint pushing ~20 state frames per second.

```sh
cargo run -p circle-game
```

Then open `examples/circle-game/index.html` in a browser (two windows for two
players). Player 1 moves with WASD, player 2 with the arrow keys.

Without a browser:

```sh
# nudge player 1 to the right
curl "localhost:3000/move/1?dx=12&dy=0"

# watch the board over the websocket (requires websocat)
websocat ws://localhost:3000/watch
```
