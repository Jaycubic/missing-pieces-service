# The Missing Pieces — backend service

Real-time cooperative EQ puzzle game, built for the FLAME server. Same
philosophy as Step & Share: Rust backend, no frontend framework, Tailwind
compiled once at build time, Postgres for durable records only.

## What's built (MVP scope, deliberately)

**Lobby → Discover → Connect → Finished.** This is a deliberate scope cut,
not an oversight: Discover and Connect contain the genuinely novel hard part
— asymmetric visibility (your hidden card is masked to you, visible to
everyone else) and the clue/token economy. Reconstruct, Contradiction,
Validate, and Decide reuse the exact same plumbing (phase timer, redacted
per-player state, server-authoritative action validation) once this
foundation is proven — that's the natural next milestone, not a rebuild.

## How it's different from Step & Share, architecturally

- **Bidirectional WebSocket.** Step & Share's socket was a read-only feed;
  every write went through REST. Here, player actions (ask for a clue,
  guess, propose a connection) flow *through* the socket, because this is a
  live synchronous session with sub-second turnaround, not an async daily
  check-in.
- **One task owns each active room.** A `tokio::spawn`'d task per room holds
  all live game state, runs the phase clock, and is the *only* thing that
  ever mutates it — everything else just sends it messages and waits for a
  reply. No actor framework, no locks around shared state.
- **Personalized state per connection.** Every state push is built fresh for
  each recipient, masking only that recipient's own hidden card. Postgres is
  never touched mid-game — it only receives a durable, anonymized event log
  once a session actually finishes.

## First-time setup

```bash
sudo -u postgres createdb missing_pieces
cd missing-pieces-service
cp .env.example .env    # set DB_PASSWORD

make build     # compiles Tailwind, then the Rust binary
make install   # one-time systemd setup
make start
make health    # should return {"status":"ok","puzzles_loaded":1}
```

## Day-to-day
```bash
make restart   # rebuilds CSS + binary, restarts the service
make logs
```

## Adding puzzle #2
Drop a new `.json` file into `puzzles/`, matching the shape of
`missing-projector.json` — no recompile needed, it's picked up on the next
restart. `POST /api/rooms` accepts an optional `puzzle_id` to pick a specific
one; omitted, it uses whichever loads first.

## The ethical stance, reflected in the schema
`session_events` never stores a real name or anything resolvable back to a
specific employee — only an anonymous `player_slot` (0–3) scoped to that one
session. Any HR-facing view built on top of this should only ever query
aggregates across many sessions, never a single slot's row in isolation.

## What I could verify this time — a stronger story than last time
Same sandbox limitation as before (apt's Rust 1.75 against a much newer
crate ecosystem), but this time I pushed the pin-and-retry loop all the way
through: **the full project actually compiled clean with zero errors and
zero warnings** in my own sandbox, including the bidirectional `actix-ws`
usage that was the one thing I couldn't fully verify last time. That's much
stronger confidence than the Step & Share backend shipped with. `Cargo.toml`
itself was never touched — only my local `Cargo.lock` was pinned to older
patch versions to satisfy the old toolchain, so your server's modern Rust
will resolve fresh, current dependency versions on its own `cargo build`.
I'd still treat that first real build on your server as the actual final
check, but I'd be surprised if anything comes back this time.
