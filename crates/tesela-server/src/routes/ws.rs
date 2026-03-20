use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};

use crate::state::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(s): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut rx = s.ws_tx.subscribe();
    ws.on_upgrade(move |mut socket| async move {
        while let Ok(event) = rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(msg) => {
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to serialize WsEvent: {}", e);
                }
            }
        }
    })
}

