use chrono::{DateTime, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::env;
use uuid::Uuid;

pub async fn create_pool() -> Result<PgPool, sqlx::Error> {
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| "missing_pieces".into());
    let db_user = env::var("DB_USER").unwrap_or_else(|_| "postgres".into());
    let db_password = env::var("DB_PASSWORD").unwrap_or_default();
    let db_host = env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let db_port: u16 = env::var("DB_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432);

    let url = format!("postgres://{db_user}:{db_password}@{db_host}:{db_port}/{db_name}");

    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// One behavioral event, recorded in memory during a live session and only
/// written to Postgres once, in a batch, when the session ends. The room
/// task never holds a DB connection open mid-game.
#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub player_slot: i32,
    pub action_type: String,
    pub target_slot: Option<i32>,
    pub phase: String,
    pub successful: Option<bool>,
    pub occurred_at: DateTime<Utc>,
}

pub async fn persist_session(
    pool: &PgPool,
    room_code: &str,
    puzzle_id: &str,
    started_at: DateTime<Utc>,
    connections_found: usize,
    connections_total: usize,
    events: &[EventLogEntry],
) -> Result<(), sqlx::Error> {
    let session_id = Uuid::new_v4();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO sessions (id, room_code, puzzle_id, started_at, finished_at, connections_found, connections_total)
         VALUES ($1, $2, $3, $4, now(), $5, $6)",
    )
    .bind(session_id)
    .bind(room_code)
    .bind(puzzle_id)
    .bind(started_at)
    .bind(connections_found as i32)
    .bind(connections_total as i32)
    .execute(&mut *tx)
    .await?;

    for ev in events {
        sqlx::query(
            "INSERT INTO session_events (id, session_id, player_slot, action_type, target_slot, phase, successful, occurred_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(Uuid::new_v4())
        .bind(session_id)
        .bind(ev.player_slot)
        .bind(&ev.action_type)
        .bind(ev.target_slot)
        .bind(&ev.phase)
        .bind(ev.successful)
        .bind(ev.occurred_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}
