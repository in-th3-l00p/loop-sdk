# loopboard

The whole framework in one app: a social tip board with a real frontend,
served by the backend itself.

| pillar | where |
| --- | --- |
| auth, both doors | email one-time codes (embedded wallet) and MetaMask via SIWE (self-custodial) |
| guards | `User` parameters 401 before handlers run; the board itself is `Option<User>` public |
| database | file migrations, per-user rows |
| **transactions** | tipping credits is `database::atomic()` — guarded debit → credit → record, all or nothing |
| SSE | `/feed` — tips on your posts, live |
| live WebSocket | `/board` — posts + leaderboard pushed on every change |
| eth reads | `/wallet` — live ETH/USDC balances via `#[contract]` bindings |
| eth writes, both rails | `/tip-onchain` signs server-side for embedded wallets; `/tip-calldata` hands linked wallets the calldata to sign in MetaMask |
| static serving | the `public/` frontend rides the same origin as the api |

## run

```sh
export ETH_RPC_URL=https://ethereum-rpc.publicnode.com
export LOOP_AUTH_SECRET=$(loop auth secret new | head -1 | cut -d' ' -f2)
loop db create && loop db migrate
loop dev
```

Open <http://localhost:3000>, sign in with an email (the code prints on the
`loop dev` console) or with MetaMask, post something, and tip posts from a
second browser profile — the board updates live over the websocket, and the
author gets an activity toast over SSE.

Credits are ledger rows; every tip is one atomic transaction with a balance
guard, so over-tipping cleanly answers "not enough credits" and moves
nothing. The "tip usdc" button moves real USDC: embedded wallets sign on the
server (fund the address shown in the wallet panel), MetaMask users sign the
prepared calldata themselves — the server never touches their key.
