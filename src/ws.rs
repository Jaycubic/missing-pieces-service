use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::Message;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{PlayerAction, ServerMessage};
use crate::state::{AppState, RoomEvent};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub device_token: String,
}

/// Unlike Step & Share's read-only feed, this socket is genuinely two-way:
/// every player action (ask for a clue, guess, propose a connection) arrives
/// here as a text frame and is forwarded straight into the room's own
/// channel. The room task is still the only thing that ever mutates state —
/// this handler never touches game logic itself.
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
    query: web::Query<WsQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let code = path.into_inner().to_uppercase();
    let Some(room_tx) = state.room_sender(&code) else {
        return Ok(HttpResponse::NotFound().body("unknown room code"));
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    if room_tx
        .send(RoomEvent::Reconnect { device_token: query.device_token.clone(), reply: reply_tx })
        .is_err()
    {
        return Ok(HttpResponse::Gone().body("room is no longer active"));
    }
    let slot = match reply_rx.await {
        Ok(Some(slot)) => slot,
        _ => return Ok(HttpResponse::Unauthorized().body("join the room before connecting")),
    };

    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMessage>();
    let _ = room_tx.send(RoomEvent::PlayerConnected { slot, sender: out_tx });

    let room_tx2 = room_tx.clone();
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                out = out_rx.recv() => {
                    match out {
                        Some(msg) => {
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if session.text(json).await.is_err() {
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
                msg = msg_stream.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(action) = serde_json::from_str::<PlayerAction>(&text) {
                                let _ = room_tx2.send(RoomEvent::PlayerAction { slot, action });
                            }
                            // Malformed frames are silently dropped — the
                            // client only ever sends what its own UI allows.
                        }
                        Some(Ok(Message::Ping(bytes))) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(_)) => break,
                        _ => {}
                    }
                }
            }
        }
        let _ = room_tx2.send(RoomEvent::PlayerDisconnected { slot });
        let _ = session.close(None).await;
    });

    Ok(response)
}
