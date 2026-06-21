// Human: Version + compatibility payload sent as the first WS frame.
// Agent: READS nothing; serialized into EventEnvelope.payload for type "handshake".
use serde::{Deserialize, Serialize};

pub const PROTO_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandshakePayload {
    pub mod_version: String,
    pub proto_version: u32,
    pub bridge_version: String,
    pub game_version: String,
    pub mods_fingerprint: String,
    pub join_code: String,
    pub display_name: String,
}

// Human: Server reply payload for an accepted handshake.
// Agent: serialized into EventEnvelope.payload for type "handshake.ack".
//        Carries the server-assigned ids; display_name is the confirmed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandshakeAckPayload {
    pub client_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub display_name: String,
    pub proto_version: u32,
}

// Human: Validate a player-chosen display name. Trims, collapses whitespace.
// Agent: RETURNS Ok(normalized) or Err(reason) -> server maps to INVALID_NAME.
pub fn normalize_display_name(raw: &str) -> Result<String, &'static str> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    match name.chars().count() {
        1..=24 => Ok(name),
        0 => Err("empty"),
        _ => Err("too_long"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_display_name_trims_and_collapses() {
        assert_eq!(
            normalize_display_name("  Alice   Bob  ").unwrap(),
            "Alice Bob"
        );
    }

    #[test]
    fn normalize_display_name_rejects_empty() {
        assert_eq!(normalize_display_name("   ").unwrap_err(), "empty");
    }
}
