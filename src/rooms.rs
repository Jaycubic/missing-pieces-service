use actix_web::{web, HttpResponse};
use serde::Deserialize;
use tokio::sync::oneshot;

use crate::state::{AppState, RoomEvent};

#[derive(Debug, Deserialize)]
pub struct CreateRoomReq {
    pub puzzle_id: Option<String>,
}

pub async fn create_room(state: web::Data<AppState>, body: web::Json<CreateRoomReq>) -> HttpResponse {
    match state.create_room(body.puzzle_id.as_deref()) {
        Ok(code) => HttpResponse::Ok().json(serde_json::json!({ "code": code })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
pub struct JoinReq {
    pub device_token: String,
    pub name: String,
}

pub async fn join_room(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<JoinReq>,
) -> HttpResponse {
    let code = path.into_inner().to_uppercase();
    let Some(tx) = state.room_sender(&code) else {
        return HttpResponse::NotFound().json(serde_json::json!({ "error": "room not found" }));
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    let sent = tx.send(RoomEvent::Join {
        device_token: body.device_token.clone(),
        name: body.name.clone(),
        reply: reply_tx,
    });
    if sent.is_err() {
        return HttpResponse::Gone().json(serde_json::json!({ "error": "room is no longer active" }));
    }

    match reply_rx.await {
        Ok(Ok(slot)) => HttpResponse::Ok().json(serde_json::json!({ "slot": slot, "code": code })),
        Ok(Err(e)) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e })),
        Err(_) => HttpResponse::InternalServerError().json(serde_json::json!({ "error": "room did not respond" })),
    }
}
