# siwe-vault

The eth and auth pillars together. Two doors into the same account system:

- **sign-in-with-ethereum** — the user proves ownership of their own wallet
  (EIP-4361 + EIP-191). Self-custodial: the server never sees the key, and
  server-side signing with a linked wallet refuses with a clear error.
- **email one-time code** — the user gets an **embedded** wallet, created and
  custodied by the app (key encrypted at rest under `LOOP_AUTH_SECRET`),
  which signs server-side through the same `Signer` trait as the treasury.

Either way, `user.wallet()` drives personalized on-chain reads
(`/portfolio`), live event feeds (`/incoming`), and transfers (`/tip`).

## run

```sh
export ETH_RPC_URL=https://ethereum-rpc.publicnode.com
export LOOP_AUTH_SECRET=$(loop auth secret new | head -1 | cut -d' ' -f2)
loop dev
```

## door 1: email + embedded wallet

```sh
curl localhost:3000/auth/otp/send -d '{"email": "ada@example.com"}'
# the dev mailer prints the code on the server console
TOKEN=$(curl -s localhost:3000/auth/otp/verify \
  -d '{"email": "ada@example.com", "code": "<from console>"}' | jq -r .token)

curl -H "Authorization: Bearer $TOKEN" localhost:3000/me
# → [{"address": "0x…", "kind": "embedded"}]
curl -H "Authorization: Bearer $TOKEN" localhost:3000/portfolio
```

A funded embedded wallet can `/tip` — the server signs with the decrypted
key: `curl -H "Authorization: Bearer $TOKEN" localhost:3000/tip -d '{"to":
"0x…", "amount": "0xf4240"}'`.

## door 2: sign-in-with-ethereum (self-custodial)

In production the browser wallet signs; for the terminal, this example ships
a `sign` helper that plays that role with a local key:

```sh
# a throwaway keypair standing in for the user's wallet
loop eth wallet new    # note the address + private key

# 1. fetch the SIWE message for that address
curl -s "localhost:3000/auth/wallet/nonce?address=<ADDRESS>" > nonce.json
jq -r .message nonce.json > message.txt

# 2. sign it (in real life: metamask's personal_sign prompt)
KEY=<PRIVATE_KEY> MESSAGE_FILE=message.txt cargo run --bin sign

# 3. exchange proof for a session — registers on first use
TOKEN=$(curl -s localhost:3000/auth/wallet/verify \
  -d "{\"address\": \"<ADDRESS>\", \"signature\": \"<SIGNATURE>\", \"nonce\": $(jq .nonce nonce.json)}" \
  | jq -r .token)

curl -H "Authorization: Bearer $TOKEN" localhost:3000/me
# → [{"address": "0x…", "kind": "linked"}]

# self-custody is enforced: the server cannot spend from a linked wallet
curl -H "Authorization: Bearer $TOKEN" localhost:3000/tip \
  -d '{"to": "0x0000000000000000000000000000000000000001", "amount": "0x1"}'
# → 500 "this wallet is self-custodial: … signed client-side"
```

Verifying while already logged in (send the bearer token with
`/auth/wallet/verify`) **links** the wallet to the existing account instead
of creating a new one.
