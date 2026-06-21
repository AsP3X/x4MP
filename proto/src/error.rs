use serde::{Deserialize, Serialize};

// Human: Canonical server→client error frame (websocket-error-shape.mdc).
// Agent: `code` is SCREAMING_SNAKE + stable; `message` is client-safe (no
//        internal paths/secrets). Shared by server and bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsErrorFrame {
    pub v: u32,
    #[serde(rename = "type")]
    pub frame_type: String,
    pub code: String,
    pub message: String,
}

impl WsErrorFrame {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            v: 1,
            frame_type: "error".into(),
            code: code.into(),
            message: message.into(),
        }
    }
}
