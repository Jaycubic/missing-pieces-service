-- Missing Pieces — durable analytics only. Live game state during an active
-- session lives entirely in memory (owned by that room's task); nothing here
-- is read or written until a session actually finishes.
--
-- Deliberately anonymous: player identity is stored only as a session-scoped
-- slot number (0-3), never a name or anything resolvable back to a specific
-- employee. HR-facing views should only ever query aggregates across many
-- sessions, never a single player_slot's row in isolation.

CREATE TABLE IF NOT EXISTS sessions (
    id                 UUID PRIMARY KEY,
    room_code          TEXT NOT NULL,
    puzzle_id          TEXT NOT NULL,
    started_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at        TIMESTAMPTZ,
    connections_found  INTEGER NOT NULL DEFAULT 0,
    connections_total  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS session_events (
    id            UUID PRIMARY KEY,
    session_id    UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    player_slot   INTEGER NOT NULL,
    action_type   TEXT NOT NULL,
    target_slot   INTEGER,
    phase         TEXT NOT NULL,
    successful    BOOLEAN,
    occurred_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id);
