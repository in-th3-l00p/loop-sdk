# loop sdk

development kit for building apps in the modern age of AI, focused on
decentralization. write statically typed Rust handlers; the framework turns
their signatures into validated REST, SSE and WebSocket endpoints — with
authentication, an ethereum SDK, a database layer and a served frontend as
configuration, not code.

```rust
use lib::prelude::*;

#[contract("abi/erc20.json")]
struct Erc20;

// a User parameter IS the auth guard: 401 before this body runs.
// Address validates at the wire: a malformed one is a 400.
#[rest(post, "/tip")]
fn tip(user: User, to: Address, amount: U256) -> Result<TxHandle, HandlerError> {
    Ok(Erc20::at("0xa0b8…eb48").transfer(to, amount).from(user.wallet()).send()?)
}

// streams are plain iterators — this is a live SSE feed
#[sse("/transfers/{token}")]
fn transfers(token: Address) -> Result<impl Iterator<Item = Transfer>, HandlerError> {
    Ok(Erc20::at(token).events::<Transfer>()?)
}

fn main() {
    lib::server::run();
}
```

## what it offers

1. **endpoints & schemas** — one attribute per handler (`#[rest]`, `#[sse]`,
   `#[live]`); parameters and return types become validated wire schemas,
   with `#[check(...)]` constraints enforced before your code runs
2. **database** — sqlite/postgres behind `database::query()`, file
   migrations via the CLI, and guarded multi-statement transactions via
   `database::atomic()`
3. **authentication** — privy-inspired: email & password, email one-time
   codes, and sign-in-with-ethereum, all mounting `/auth/*` routes from
   `loop.toml`; every user can hold keys via the embedded wallet manager
4. **ethereum sdk** — typed contract bindings from ABI json, schema-carrying
   chain primitives, event streams, and one `Signer` trait across the app
   treasury, embedded wallets (server-signed) and linked wallets
   (self-custodial)
5. **frontend hosting** — a `public/` directory is served on the same origin
   as the api, so a plain-js app talks to it with zero CORS setup
6. payments & subscriptions, ai engineering — *planned (blueprints in the docs)*

## prerequisites

- **Rust** (stable, edition 2024) — <https://rustup.rs>
- **foundry** *(optional)* — powers `loop devnet` local testnets:
  `curl -L https://foundry.paradigm.xyz | bash && foundryup` (or `brew install foundry`)
- **node 20+** *(optional)* — only for the docs/landing site under `web/`

## install the CLI

```sh
git clone <this repo> && cd loop-sdk
./bin/install.sh        # builds loop-cli in release mode, installs it as `loop`
loop --help
```

## start a project

```sh
mkdir my-api && cd my-api
loop init               # scaffolds loop.toml, Cargo.toml, src/main.rs
loop dev                # runs it on http://localhost:3000
```

add features by editing `loop.toml` — a `[database]` section gives you
`loop db create / migrate` and `database::query()`; an `[auth]` section
mounts the login routes; an `[eth]` section connects the chain client at
startup. `loop dev` turns the manifest into environment variables and runs
your crate; a compiled binary reads the same `LOOP_*` variables directly.

## start the full demo (loopboard)

`examples/loopboard` exercises every pillar at once — auth (both doors),
atomic credit tipping, a live WebSocket board, an SSE activity feed, on-chain
USDC reads and writes, and a served browser frontend. run it on a local
devnet forked from mainnet:

```sh
# terminal 1: a persistent local chain with funded accounts
loop devnet create board --fork https://ethereum-rpc.publicnode.com
loop devnet serve board

# terminal 2: the app
cd examples/loopboard
export ETH_RPC_URL=http://127.0.0.1:8545
export LOOP_AUTH_SECRET=$(loop auth secret new | head -1 | cut -d' ' -f2)
loop db create && loop db migrate
loop dev
```

open <http://localhost:3000>, sign in with an email (the one-time code
prints on the `loop dev` console) or with MetaMask, and tip posts from two
browser profiles — the board moves live. fund your embedded wallet for gas
with `loop devnet fund 0x<address> --eth 10 --name board`. chain state
persists across devnet restarts; see [examples/](examples/README.md) for
six smaller, single-pillar examples.

## repository layout

| path | what |
| --- | --- |
| [crates/lib](crates/lib/) | the SDK: `schema`, `server`, `database`, `auth`, `eth` modules behind cargo features |
| [crates/macros](crates/macros/) | `#[rest]` / `#[sse]` / `#[live]` / `#[derive(Schema)]` / `#[contract]` |
| [crates/cli](crates/cli/) | the `loop` binary: `init`, `dev`, `build`, `db`, `migration`, `eth`, `auth`, `devnet` |
| [examples/](examples/) | eight runnable projects, from a todo list to the full stack |
| [web/](web/) | docs & landing site (next.js) |

## the docs site

```sh
cd web
npm install
npm run dev             # http://localhost:3000 — manual, guides, blueprints
```

## developing the SDK itself

```sh
cd crates
cargo test -p lib                                # core (server, database, macros)
cargo test -p lib --features auth                # + authentication
cargo test -p lib --features eth                 # + ethereum (mock json-rpc suite)
cargo test -p lib --features "auth eth"          # + wallets and SIWE
cargo test -p loop-cli                           # manifest + CLI
```

the library defaults to `server`, `database`, `db-sqlite` and `macros`;
`auth` and `eth` are opt-in features, so apps only compile what they use.
