mod db;
mod protocol;
mod puzzle;
mod rooms;
mod state;
mod ws;

use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use state::AppState;

async fn health(state: web::Data<AppState>) -> HttpResponse {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "puzzles_loaded": state.puzzles.len()
        })),
        Err(e) => {
            tracing::error!(error = %e, "health check DB ping failed");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({ "status": "db_error" }))
        }
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let pool = db::create_pool().await?;
    db::run_migrations(&pool).await?;
    tracing::info!("migrations applied");

    let puzzles_dir = std::env::var("PUZZLES_DIR").unwrap_or_else(|_| "./puzzles".to_string());
    let puzzles = puzzle::load_puzzles(&puzzles_dir);
    if puzzles.is_empty() {
        tracing::warn!("no puzzles loaded — rooms cannot be created until at least one puzzle file exists");
    }

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8091);

    let app_state = web::Data::new(AppState::new(puzzles, pool));

    tracing::info!(bind_addr = %bind_addr, port, "missing-pieces-service starting");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/health", web::get().to(health))
            .route("/api/rooms", web::post().to(rooms::create_room))
            .route("/api/rooms/{code}/join", web::post().to(rooms::join_room))
            .route("/ws/{code}", web::get().to(ws::ws_route))
            .service(Files::new("/", "./public").index_file("index.html"))
    })
    .bind((bind_addr.as_str(), port))?
    .run()
    .await?;

    Ok(())
}
