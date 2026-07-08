# guarded-notes

Authentication as configuration: the `[auth]` section in `loop.toml` mounts
the `/auth/*` routes, and a `User` parameter in a handler signature is the
guard — absent or invalid sessions answer 401 before your code runs.
`Option<User>` makes the session optional.

## run

```sh
loop db migrate
loop dev
```

## try it

```sh
# no session → 401
curl -i localhost:3000/me

# register (returns { token, user })
TOKEN=$(curl -s localhost:3000/auth/register \
  -H 'content-type: application/json' \
  -d '{"email": "ada@example.com", "password": "hunter2222"}' | jq -r .token)

# authenticated calls carry the bearer token
curl -H "Authorization: Bearer $TOKEN" localhost:3000/me
curl -H "Authorization: Bearer $TOKEN" localhost:3000/notes \
  -d '{"text": "loops all the way down"}'
curl -H "Authorization: Bearer $TOKEN" localhost:3000/notes

# Option<User>: works logged out and logged in
curl localhost:3000/lobby
curl -H "Authorization: Bearer $TOKEN" localhost:3000/lobby

# one-time codes: the dev mailer prints the code on the server console
curl localhost:3000/auth/otp/send -d '{"email": "grace@example.com"}'
curl localhost:3000/auth/otp/verify -d '{"email": "grace@example.com", "code": "<from console>"}'

# sessions end at logout
curl -X POST -H "Authorization: Bearer $TOKEN" localhost:3000/auth/logout
curl -i -H "Authorization: Bearer $TOKEN" localhost:3000/auth/session   # 401
```
