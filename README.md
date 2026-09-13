# The Missing Pieces — backend service

Real-time cooperative EQ puzzle game for the FLAME server. All six phases
are now built: Lobby → Discover → Connect → Reconstruct → Contradiction →
Validate → Decide → Finished.

## Phase summary

| Phase | Duration | Mechanic |
|---|---|---|
| Lobby | until full | Players join with a shared room code |
| Discover | 2 min | Ask teammates for hints, guess your own hidden card |
| Connect | 2 min | Propose links between cards on the shared table |
| Reconstruct | 2 min | Order events correctly, excluding any distractor cards |
| Contradiction | 1 min | Identify the one pair of statements that can't both be true |
| Validate | 1 min | Flag candidate solutions that don't hold up (discussion aid, not scored) |
| Decide | 30 sec | Cast a vote; majority vote determines the team's final answer |

Every phase after Discover is intentionally **not** redacted per player —
by that point in the design, all information is meant to be commonly known
to the team. Only Discover has per-player asymmetric visibility.

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

## Verification

The full backend compiled clean — zero errors, zero warnings — against a
real (if artificially downgraded for my own sandbox's older toolchain)
dependency resolution, including every new phase's logic: sequence
validation, contradiction-pair matching, and vote tallying. `Cargo.toml`
itself uses normal version ranges; only my local `Cargo.lock` was pinned to
satisfy my sandbox's Rust 1.75. Your server's modern toolchain should
resolve current versions on its own `cargo build` without needing any of
that.

All new puzzle content (the event sequence, the contradiction pair, the
three candidate solutions) was added to `puzzles/missing-projector.json`
and validated for structural correctness (exactly one correct solution, no
id collisions, every referenced id resolves to a real card) before being
wired into the Rust side.

## Adding puzzle #2

Every puzzle file now needs four blocks: `players` (with hidden/visible
cards as before), `connections`, `sequence` (cards + correct_order),
`contradiction` (cards + the one true pair), and `solutions` (exactly one
`correct: true`). See `missing-projector.json` for the full shape.
