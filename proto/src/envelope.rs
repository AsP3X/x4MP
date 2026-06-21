use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Human: Who is allowed to originate a given event type.
// Agent: serialized lowercase ("host"|"server"|"client"); checked by server ACL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Authority {
    Host,
    Server,
    Client,
}

// Human: The single message shape carried on every WS frame (incl. handshake).
// Agent: seq is server-assigned on ingest; payload is the per-type body.
//        APPLIES to all proto/server/bridge messages (event-protocol.mdc).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub v: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub session_id: Uuid,
    pub seq: u64,
    pub timestamp_sim: f64,
    pub sender_id: Uuid,
    pub authority: Authority,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trip() {
        let env = EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id: Uuid::nil(),
            seq: 1,
            timestamp_sim: 0.0,
            sender_id: Uuid::nil(),
            authority: Authority::Client,
            payload: serde_json::json!({ "hello": "world" }),
        };
        let json = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);
    }
}
