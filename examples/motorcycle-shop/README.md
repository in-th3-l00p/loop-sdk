# motorcycle-shop

A complete REST CRUD API built on the loop SDK: five natively-bound endpoints
sharing one in-memory inventory.

```sh
loop dev
```

```sh
# create
curl -X POST localhost:3000/motorcycles \
  -H 'content-type: application/json' \
  -d '{"brand": "Ducati", "model": "Panigale V4", "year": 2025, "price": 24995.0}'

# list
curl localhost:3000/motorcycles

# read one
curl localhost:3000/motorcycles/1

# update
curl -X PUT localhost:3000/motorcycles/1 \
  -H 'content-type: application/json' \
  -d '{"brand": "Ducati", "model": "Panigale V4 S", "year": 2026, "price": 31495.0}'

# delete
curl -X DELETE localhost:3000/motorcycles/1
```

Typed inputs are enforced by each endpoint's signature — try sending
`"year": "new"` and you'll get a 400 with the schema mismatch.
