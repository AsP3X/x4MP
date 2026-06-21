use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::StreamExt;
use tokio::net::TcpListener;
use uuid::Uuid;

use x4mp_proto::{
    normalize_display_name, Authority, EventEnvelope, HandshakeAckPayload, HandshakePayload,
    PROTO_VERSION, WsErrorFrame,
};

use crate::debug_bundle::write_rejection_bundle;
use crate::session::{AppState, DEV_JOIN_CODE};
use crate::tracing_util::ws_message_span;

// Human: Resolve the data directory from env or default.
// Agent: READS X4MP_DATA_DIR; RETURNS PathBuf for event.log and debug bundles.
pub fn data_root() -> std::path::PathBuf {
    std::env::var("X4MP_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("data"))
}

// Human: Build the axum router wired to shared session state.
// Agent: CALLS ws_handler on /ws upgrade; READS AppState for seq + event log.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(state)
}

async fn ws_upgrade(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// Human: Run the HTTP/WS server on an existing listener (for tests + main).
// Agent: CALLS axum::serve until listener closes or error.
pub async fn run_on_listener(listener: TcpListener) -> Result<(), std::io::Error> {
    let session_id = crate::session::DEV_SESSION_ID;
    let state = AppState::new(data_root(), session_id)?;
    let app = router(state);
    axum::serve(listener, app).await
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let trace_id = Uuid::new_v4();
    let mut handshaken = false;
    let mut client_id = Uuid::nil();
    let mut recent_events: Vec<EventEnvelope> = Vec::new();

    while let Some(msg) = socket.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(%trace_id, error = %e, "websocket read error");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let span = ws_message_span(trace_id, Some(state.session_id), Some(client_id), None);
        let _guard = span.enter();

        let parsed: EventEnvelope = match serde_json::from_str(&text) {
            Ok(env) => env,
            Err(e) => {
                tracing::warn!(error = %e, "invalid json envelope");
                let err = WsErrorFrame::new("INVALID_JSON", "Message is not valid JSON.");
                let _ = send_json(&mut socket, &err).await;
                break;
            }
        };

        if !handshaken {
            if parsed.event_type != "handshake" {
                let err = WsErrorFrame::new(
                    "HANDSHAKE_REQUIRED",
                    "First message must be a handshake envelope.",
                );
                let _ = send_json(&mut socket, &err).await;
                break;
            }

            match process_handshake(&parsed, &state) {
                HandshakeResult::Accepted { ack, assigned_client_id } => {
                    client_id = assigned_client_id;
                    handshaken = true;
                    if send_json(&mut socket, &ack).await.is_err() {
                        break;
                    }
                }
                HandshakeResult::Rejected { err } => {
                    let _ = send_json(&mut socket, &err).await;
                    let _ = write_rejection_bundle(
                        state.session_id,
                        &err.code,
                        &data_root(),
                        &recent_events,
                    );
                    break;
                }
            }
            continue;
        }

        // Post-handshake: assign seq, log, echo.
        let mut stamped = parsed;
        stamped.session_id = state.session_id;
        stamped.sender_id = client_id;
        stamped.seq = state.next_seq();

        if let Err(e) = state.event_log.append(&stamped) {
            tracing::error!(error = %e, "failed to append event log");
        }

        recent_events.push(stamped.clone());
        if recent_events.len() > 200 {
            recent_events.remove(0);
        }

        tracing::debug!(
            event_type = %stamped.event_type,
            seq = stamped.seq,
            "echoing envelope"
        );

        if send_json(&mut socket, &stamped).await.is_err() {
            break;
        }
    }
}

enum HandshakeResult {
    Accepted {
        ack: EventEnvelope,
        assigned_client_id: Uuid,
    },
    Rejected {
        err: WsErrorFrame,
    },
}

// Human: Validate handshake payload and build handshake.ack or error frame.
// Agent: READS HandshakePayload; WRITES EventEnvelope with HandshakeAckPayload.
fn process_handshake(envelope: &EventEnvelope, state: &AppState) -> HandshakeResult {
    let payload: HandshakePayload = match serde_json::from_value(envelope.payload.clone()) {
        Ok(p) => p,
        Err(_) => {
            return HandshakeResult::Rejected {
                err: WsErrorFrame::new("INVALID_HANDSHAKE", "Handshake payload is malformed."),
            };
        }
    };

    if payload.proto_version != PROTO_VERSION {
        return HandshakeResult::Rejected {
            err: WsErrorFrame::new(
                "INCOMPATIBLE_VERSION",
                "Protocol version mismatch.",
            ),
        };
    }

    if payload.join_code != DEV_JOIN_CODE {
        return HandshakeResult::Rejected {
            err: WsErrorFrame::new("INVALID_JOIN_CODE", "Join code is not valid."),
        };
    }

    // M0: lenient game/mods checks — strict validation lands in M3.
    if payload.game_version.is_empty() {
        return HandshakeResult::Rejected {
            err: WsErrorFrame::new(
                "INCOMPATIBLE_GAME_VERSION",
                "Game version is required.",
            ),
        };
    }

    let display_name = match normalize_display_name(&payload.display_name) {
        Ok(name) => name,
        Err(_) => {
            return HandshakeResult::Rejected {
                err: WsErrorFrame::new("INVALID_NAME", "Display name is not valid."),
            };
        }
    };

    let client_id = Uuid::new_v4();
    let ack_payload = HandshakeAckPayload {
        client_id,
        session_id: state.session_id,
        display_name,
        proto_version: PROTO_VERSION,
    };

    let ack = EventEnvelope {
        v: 1,
        event_type: "handshake.ack".into(),
        session_id: state.session_id,
        seq: 0,
        timestamp_sim: 0.0,
        sender_id: Uuid::nil(),
        authority: Authority::Server,
        payload: serde_json::to_value(ack_payload).unwrap_or_default(),
    };

    HandshakeResult::Accepted {
        ack,
        assigned_client_id: client_id,
    }
}

async fn send_json<T: serde::Serialize>(socket: &mut WebSocket, value: &T) -> Result<(), ()> {
    let text = serde_json::to_string(value).map_err(|_| ())?;
    socket.send(Message::Text(text.into())).await.map_err(|_| ())
}
