use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use x4mp_proto::{
    Authority, EventEnvelope, HandshakeAckPayload, HandshakePayload, PROTO_VERSION, WsErrorFrame,
};

pub const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MOD_VERSION: &str = "0.1.0";

// Human: Bridge-side config read from environment with M0 defaults.
// Agent: READS X4MP_* env vars; RETURNS URLs and handshake fields.
pub struct BridgeConfig {
    pub server_url: String,
    pub display_name: String,
    pub join_code: String,
    pub game_version: String,
    pub mods_fingerprint: String,
}

impl BridgeConfig {
    pub fn from_env() -> Self {
        Self {
            server_url: std::env::var("X4MP_SERVER_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:7878/ws".into()),
            display_name: std::env::var("X4MP_DISPLAY_NAME").unwrap_or_else(|_| "Player".into()),
            join_code: std::env::var("X4MP_JOIN_CODE").unwrap_or_else(|_| "ABCD-1234".into()),
            game_version: std::env::var("X4MP_GAME_VERSION").unwrap_or_else(|_| "7.5".into()),
            mods_fingerprint: std::env::var("X4MP_MODS_FINGERPRINT")
                .unwrap_or_else(|_| "sha256:0000".into()),
        }
    }
}

// Human: Result of a successful WebSocket handshake with the server.
// Agent: READS handshake.ack; RETURNS assigned ids for pipe forwarding.
pub struct HandshakeResult {
    pub client_id: Uuid,
    pub session_id: Uuid,
    pub display_name: String,
}

// Human: Connect, send handshake, await ack before any game events.
// Agent: EMITS handshake envelope; READS handshake.ack or error frame.
pub async fn perform_handshake(
    config: &BridgeConfig,
) -> Result<(HandshakeResult, tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>), BridgeError> {
    let trace_id = Uuid::new_v4();
    let span = tracing::info_span!("bridge_handshake", trace_id = %trace_id);
    let _guard = span.enter();

    let (mut ws, _) = connect_async(&config.server_url)
        .await
        .map_err(|e| BridgeError::Connect(e.to_string()))?;

    let handshake = EventEnvelope {
        v: 1,
        event_type: "handshake".into(),
        session_id: Uuid::nil(),
        seq: 0,
        timestamp_sim: 0.0,
        sender_id: Uuid::nil(),
        authority: Authority::Client,
        payload: serde_json::to_value(HandshakePayload {
            mod_version: MOD_VERSION.into(),
            proto_version: PROTO_VERSION,
            bridge_version: BRIDGE_VERSION.into(),
            game_version: config.game_version.clone(),
            mods_fingerprint: config.mods_fingerprint.clone(),
            join_code: config.join_code.clone(),
            display_name: config.display_name.clone(),
        })
        .unwrap_or_default(),
    };

    let text = serde_json::to_string(&handshake).map_err(|e| BridgeError::Serialize(e.to_string()))?;
    ws.send(Message::Text(text.into()))
        .await
        .map_err(|e| BridgeError::Send(e.to_string()))?;

    let reply = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .map_err(|_| BridgeError::Timeout)?
        .ok_or(BridgeError::Closed)?
        .map_err(|e| BridgeError::Recv(e.to_string()))?;

    let reply_text = reply.into_text().map_err(|_| BridgeError::BinaryReply)?;
    let value: serde_json::Value =
        serde_json::from_str(&reply_text).map_err(|e| BridgeError::Parse(e.to_string()))?;

    if value.get("type").and_then(|t| t.as_str()) == Some("error") {
        let err: WsErrorFrame = serde_json::from_value(value)
            .map_err(|e| BridgeError::Parse(e.to_string()))?;
        return Err(BridgeError::Server(err.code, err.message));
    }

    let ack: EventEnvelope =
        serde_json::from_str(&reply_text).map_err(|e| BridgeError::Parse(e.to_string()))?;
    if ack.event_type != "handshake.ack" {
        return Err(BridgeError::UnexpectedType(ack.event_type));
    }

    let payload: HandshakeAckPayload =
        serde_json::from_value(ack.payload).map_err(|e| BridgeError::Parse(e.to_string()))?;

    tracing::info!(
        client_id = %payload.client_id,
        session_id = %payload.session_id,
        display_name = %payload.display_name,
        "handshake.ack received"
    );

    Ok((
        HandshakeResult {
            client_id: payload.client_id,
            session_id: payload.session_id,
            display_name: payload.display_name,
        },
        ws,
    ))
}

// Human: Send one NDJSON line to the server and await the echoed reply.
// Agent: WRITES WS text frame; READS echoed EventEnvelope with server seq.
pub async fn send_and_receive(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    line: &str,
) -> Result<EventEnvelope, BridgeError> {
    ws.send(Message::Text(line.to_string().into()))
        .await
        .map_err(|e| BridgeError::Send(e.to_string()))?;

    let reply = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .map_err(|_| BridgeError::Timeout)?
        .ok_or(BridgeError::Closed)?
        .map_err(|e| BridgeError::Recv(e.to_string()))?;

    let text = reply.into_text().map_err(|_| BridgeError::BinaryReply)?;
    serde_json::from_str(&text).map_err(|e| BridgeError::Parse(e.to_string()))
}

#[derive(Debug)]
pub enum BridgeError {
    Connect(String),
    Send(String),
    Recv(String),
    Serialize(String),
    Parse(String),
    Timeout,
    Closed,
    BinaryReply,
    UnexpectedType(String),
    Server(String, String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Connect(m) => write!(f, "connect failed: {m}"),
            BridgeError::Send(m) => write!(f, "send failed: {m}"),
            BridgeError::Recv(m) => write!(f, "recv failed: {m}"),
            BridgeError::Serialize(m) => write!(f, "serialize failed: {m}"),
            BridgeError::Parse(m) => write!(f, "parse failed: {m}"),
            BridgeError::Timeout => write!(f, "timed out waiting for server reply"),
            BridgeError::Closed => write!(f, "connection closed"),
            BridgeError::BinaryReply => write!(f, "expected text frame"),
            BridgeError::UnexpectedType(t) => write!(f, "unexpected frame type: {t}"),
            BridgeError::Server(code, msg) => write!(f, "server error {code}: {msg}"),
        }
    }
}

impl std::error::Error for BridgeError {}
