use std::path::{Path, PathBuf};

use uuid::Uuid;
use x4mp_proto::{Authority, EventEnvelope, HandshakePayload, PROTO_VERSION};
use x4mp_server::read_event_log;

// Q3 spike divergence analysis (reads worldsnap samples from an event.log).
pub mod divergence;

// Human: CLI configuration for replaying a session event log.
// Agent: READS --session, --from-seq, --server, --data-dir from clap/main.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub session_id: Uuid,
    pub from_seq: u64,
    pub server_url: String,
    pub data_root: PathBuf,
    pub display_name: String,
    pub join_code: String,
}

impl ReplayConfig {
    pub fn event_log_path(&self) -> PathBuf {
        self.data_root
            .join("sessions")
            .join(self.session_id.to_string())
            .join("event.log")
    }
}

// Human: Outcome counters after replay completes.
// Agent: RETURNS counts of sent events and last observed server seq.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayStats {
    pub events_sent: usize,
    pub last_seq: Option<u64>,
}

// Human: Load envelopes from disk filtered by --from-seq, skipping handshake types.
// Agent: READS event.log; RETURNS game events only (seq >= from_seq).
pub fn load_events_for_replay(path: &Path, from_seq: u64) -> std::io::Result<Vec<EventEnvelope>> {
    let all = read_event_log(path)?;
    Ok(all
        .into_iter()
        .filter(|env| {
            env.event_type != "handshake"
                && env.event_type != "handshake.ack"
                && env.seq >= from_seq
        })
        .collect())
}

// Human: Replay filtered events against a live server over WebSocket.
// Agent: EMITS handshake then each event; READS server echoes for seq tracking.
pub async fn replay_events(config: &ReplayConfig) -> Result<ReplayStats, ReplayError> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let events = load_events_for_replay(&config.event_log_path(), config.from_seq)
        .map_err(|e| ReplayError::Io(e.to_string()))?;

    let (mut ws, _) = connect_async(&config.server_url)
        .await
        .map_err(|e| ReplayError::Connect(e.to_string()))?;

    let handshake = EventEnvelope {
        v: 1,
        event_type: "handshake".into(),
        session_id: Uuid::nil(),
        seq: 0,
        timestamp_sim: 0.0,
        sender_id: Uuid::nil(),
        authority: Authority::Client,
        payload: serde_json::to_value(HandshakePayload {
            mod_version: "0.1.0".into(),
            proto_version: PROTO_VERSION,
            bridge_version: env!("CARGO_PKG_VERSION").into(),
            game_version: "7.5".into(),
            mods_fingerprint: "sha256:0000".into(),
            join_code: config.join_code.clone(),
            display_name: config.display_name.clone(),
        })
        .map_err(|e| ReplayError::Serialize(e.to_string()))?,
    };

    ws.send(Message::Text(
        serde_json::to_string(&handshake)
            .map_err(|e| ReplayError::Serialize(e.to_string()))?
            .into(),
    ))
    .await
    .map_err(|e| ReplayError::Send(e.to_string()))?;

    let ack = ws
        .next()
        .await
        .ok_or(ReplayError::Closed)?
        .map_err(|e| ReplayError::Recv(e.to_string()))?;
    let ack_text = ack.into_text().map_err(|_| ReplayError::BinaryReply)?;
    let ack_value: serde_json::Value =
        serde_json::from_str(&ack_text).map_err(|e| ReplayError::Parse(e.to_string()))?;
    if ack_value.get("type").and_then(|t| t.as_str()) != Some("handshake.ack") {
        return Err(ReplayError::UnexpectedFrame(ack_text.to_string()));
    }

    let mut last_seq = None;
    let events_sent = events.len();
    for mut env in events {
        env.seq = 0;
        let text = serde_json::to_string(&env).map_err(|e| ReplayError::Serialize(e.to_string()))?;
        ws.send(Message::Text(text.into()))
            .await
            .map_err(|e| ReplayError::Send(e.to_string()))?;

        let reply = ws
            .next()
            .await
            .ok_or(ReplayError::Closed)?
            .map_err(|e| ReplayError::Recv(e.to_string()))?;
        let reply_text = reply.into_text().map_err(|_| ReplayError::BinaryReply)?;
        let echoed: EventEnvelope =
            serde_json::from_str(&reply_text).map_err(|e| ReplayError::Parse(e.to_string()))?;
        last_seq = Some(echoed.seq);
    }

    let _ = ws.close(None).await;

    Ok(ReplayStats {
        events_sent,
        last_seq,
    })
}

#[derive(Debug)]
pub enum ReplayError {
    Io(String),
    Connect(String),
    Send(String),
    Recv(String),
    Serialize(String),
    Parse(String),
    Closed,
    BinaryReply,
    UnexpectedFrame(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Io(m) => write!(f, "io error: {m}"),
            ReplayError::Connect(m) => write!(f, "connect failed: {m}"),
            ReplayError::Send(m) => write!(f, "send failed: {m}"),
            ReplayError::Recv(m) => write!(f, "recv failed: {m}"),
            ReplayError::Serialize(m) => write!(f, "serialize failed: {m}"),
            ReplayError::Parse(m) => write!(f, "parse failed: {m}"),
            ReplayError::Closed => write!(f, "connection closed"),
            ReplayError::BinaryReply => write!(f, "expected text frame"),
            ReplayError::UnexpectedFrame(t) => write!(f, "unexpected frame: {t}"),
        }
    }
}

impl std::error::Error for ReplayError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use x4mp_proto::Authority;

    #[test]
    fn load_events_filters_handshake_and_from_seq() {
        let tmp = std::env::temp_dir().join(format!("x4mp-replay-load-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("event.log");
        let session_id = Uuid::new_v4();

        let lines = [
            EventEnvelope {
                v: 1,
                event_type: "handshake".into(),
                session_id,
                seq: 0,
                timestamp_sim: 0.0,
                sender_id: Uuid::nil(),
                authority: Authority::Client,
                payload: serde_json::json!({}),
            },
            EventEnvelope {
                v: 1,
                event_type: "session.ping".into(),
                session_id,
                seq: 1,
                timestamp_sim: 0.0,
                sender_id: Uuid::new_v4(),
                authority: Authority::Client,
                payload: serde_json::json!({}),
            },
            EventEnvelope {
                v: 1,
                event_type: "session.ping".into(),
                session_id,
                seq: 2,
                timestamp_sim: 0.0,
                sender_id: Uuid::new_v4(),
                authority: Authority::Client,
                payload: serde_json::json!({}),
            },
        ];

        let mut f = std::fs::File::create(&path).unwrap();
        for env in &lines {
            writeln!(f, "{}", serde_json::to_string(env).unwrap()).unwrap();
        }

        let from_2 = load_events_for_replay(&path, 2).unwrap();
        assert_eq!(from_2.len(), 1);
        assert_eq!(from_2[0].seq, 2);

        let from_1 = load_events_for_replay(&path, 1).unwrap();
        assert_eq!(from_1.len(), 2);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
