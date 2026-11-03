# The game service

The Bragd server is a Supabase project and this game service beside it
([ADR 0005](../../docs/adr/0005-bragd-server-on-hosted-supabase.md)). The game service is
`crates/bragd-server`. It referees Games between Bragd players with Bragd's own rules and keeps
them in the project's Postgres. Nothing here is deployed by a workflow: the owner sets it up by hand.

## The database

The tables, their row-level security and their grants are the migrations in
`supabase/migrations/`, in the Supabase CLI's layout. Apply them to the project with
`supabase db push`, or paste them in order into the SQL editor.

The game service connects as a role that may switch to `authenticated` (Supabase's `postgres`
role may). Each request runs as `authenticated` with the caller's token claims set, as Supabase's
own API does, so the row-level security decides what a caller may read and write. Only the
referee's writes, made after the rules have checked a move, step out of that role.

## Running it

```sh
podman build -f packaging/server/Dockerfile --ignorefile packaging/server/Dockerfile.dockerignore -t localhost/bragd-server:dev .
podman run -d --init --name bragd-server -p 127.0.0.1:8080:8080 --env-file /etc/bragd-server.env localhost/bragd-server:dev
```

`--init` lets the container stop at once: the service itself does not wait for a signal. Put a TLS proxy in front of port 8080. Bragd's Settings → Online → *Bragd server address* is that
proxy's `https://` address.

| Variable | What |
|---|---|
| `BRAGD_SERVER_DATABASE_URL` | The project's Postgres, as a connection string with `sslmode=require` |
| `BRAGD_SERVER_SUPABASE_URL` | The project's address, `https://<ref>.supabase.co`. Its signing keys are read from `/auth/v1/.well-known/jwks.json` |
| `BRAGD_SERVER_SUPABASE_KEY` | The project's publishable (anon) key, handed to Bragd for signing in |
| `BRAGD_SERVER_JWT_SECRET` | The project's legacy JWT secret, while its tokens are still signed with HS256. Leave it empty once the project uses signing keys |
| `BRAGD_SERVER_OAUTH` | The sign-in providers turned on in the project, such as `github,google` |
| `BRAGD_SERVER_LISTEN` | Where it listens; `0.0.0.0:8080` by default |
| `BRAGD_SERVER_TURN_URLS` | The STUN and TURN addresses a Call may use, such as `stun:turn.example.org:3478,turn:turn.example.org:3478,turns:turn.example.org:5349` |
| `BRAGD_SERVER_TURN_SECRET` | The secret shared with coturn (`static-auth-secret`). Empty: Calls get STUN only |

## Calls: signalling and TURN

A Call's offers, answers and ICE candidates go over Supabase Realtime, on a private broadcast
topic `call:<a>:<b>` for each pair of players. The policies in
`supabase/migrations/…_signalling.sql` let only those two join, and only while they are Friends.
In the project's Realtime settings, turn off *Allow public access*, so every channel is private.

The media goes directly between the two machines when it can. When it cannot, it goes through
coturn on this host, still encrypted end to end. `turnserver.conf` is its configuration:

1. Install coturn on the host (`apt install coturn`), copy `turnserver.conf` to
   `/etc/turnserver.conf`, and set `static-auth-secret` to a long random value.
2. Give it the TURN name's certificate at the `cert` and `pkey` paths.
3. Open 3478 (UDP and TCP), 5349 (TCP) and 49160–49260 (UDP) in the firewall.
4. Set `BRAGD_SERVER_TURN_SECRET` to the same secret and `BRAGD_SERVER_TURN_URLS` to its
   addresses. `GET /v1/turn` then hands each signed-in player credentials of their own for 12 hours
   (coturn's TURN REST API).

## Tests

`cargo test -p bragd-server` runs the API and the row-level security against a throwaway plain
Postgres: the server named by `BRAGD_TEST_PG`, or a `docker.io/library/postgres` container started
with podman or docker and removed by its id at the end. `tests/pg/supabase-stub.sql` stands in
for what Supabase provides: its roles, `auth.users`, `auth.uid()`, and Realtime's
`realtime.messages` and `realtime.topic()`.
