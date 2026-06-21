use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event_log::{tail_event_log, EventLog, EVENT_LOG_TAIL_LIMIT};
use crate::session::AppState;

// Human: Why an event or handshake was rejected (debug bundle acl_decisions.json).
// Agent: WRITES stable reason codes; READS by post-mortem tooling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AclDecision {
    pub timestamp_utc_secs: u64,
    pub session_id: Uuid,
    pub client_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub code: String,
    pub message: String,
}

// Human: Host vs reporter hash breakdown when world.hash diverges.
// Agent: WRITES hashes.json; empty host/reporter when not applicable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HashBreakdown {
    pub host_hash: Option<String>,
    pub reporter_hash: Option<String>,
    pub reporter_client_id: Option<Uuid>,
    pub seq_at_report: Option<u64>,
}

// Human: Inputs for a full observability debug bundle export.
// Agent: READS session state + event.log tail; WRITES debug_bundle directory.
pub struct DebugBundleRequest {
    pub session_id: Uuid,
    pub trigger: &'static str,
    pub trace_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub acl_decisions: Vec<AclDecision>,
    pub hash_breakdown: Option<HashBreakdown>,
    pub players: Vec<serde_json::Value>,
    pub versions: serde_json::Value,
}

impl DebugBundleRequest {
    // Human: Build a bundle request from a handshake or ACL rejection.
    // Agent: READS WsErrorFrame fields; WRITES acl_decisions with one entry.
    pub fn from_rejection(
        session_id: Uuid,
        trigger: &'static str,
        trace_id: Option<Uuid>,
        client_id: Option<Uuid>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let now = utc_secs();
        Self {
            session_id,
            trigger,
            trace_id,
            client_id,
            acl_decisions: vec![AclDecision {
                timestamp_utc_secs: now,
                session_id,
                client_id,
                trace_id,
                event_type: None,
                code: code.into(),
                message: message.into(),
            }],
            hash_breakdown: None,
            players: Vec::new(),
            versions: serde_json::json!({}),
        }
    }

    // Human: Build a bundle request for world.hash mismatch (M1+ hook).
    // Agent: READS HashBreakdown; WRITES hashes.json in bundle output.
    pub fn from_hash_mismatch(
        session_id: Uuid,
        trace_id: Option<Uuid>,
        client_id: Option<Uuid>,
        hash_breakdown: HashBreakdown,
        acl_decisions: Vec<AclDecision>,
    ) -> Self {
        Self {
            session_id,
            trigger: "world.hash_mismatch",
            trace_id,
            client_id,
            acl_decisions,
            hash_breakdown: Some(hash_breakdown),
            players: Vec::new(),
            versions: serde_json::json!({}),
        }
    }
}

// Human: Export a full debug bundle per observability-debugging.mdc.
// Agent: WRITES meta.json, last_events.jsonl, acl_decisions.json, hashes.json
//        under data/debug_bundle/<utc>/; RETURNS bundle directory path.
pub fn write_debug_bundle(
    data_root: &Path,
    event_log: &EventLog,
    request: &DebugBundleRequest,
) -> std::io::Result<PathBuf> {
    let ts = utc_secs();
    let dir = data_root.join("debug_bundle").join(ts.to_string());
    std::fs::create_dir_all(&dir)?;

    let meta = serde_json::json!({
        "session_id": request.session_id,
        "trigger": request.trigger,
        "trace_id": request.trace_id,
        "client_id": request.client_id,
        "players": request.players,
        "versions": request.versions,
        "exported_at_utc_secs": ts,
    });
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

    let tail = tail_event_log(event_log.path(), EVENT_LOG_TAIL_LIMIT)?;
    let mut last_events = String::new();
    for env in &tail {
        last_events.push_str(&serde_json::to_string(env).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?);
        last_events.push('\n');
    }
    std::fs::write(dir.join("last_events.jsonl"), last_events)?;

    std::fs::write(
        dir.join("acl_decisions.json"),
        serde_json::to_string_pretty(&request.acl_decisions)?,
    )?;

    let hashes = request.hash_breakdown.clone().unwrap_or(HashBreakdown {
        host_hash: None,
        reporter_hash: None,
        reporter_client_id: request.client_id,
        seq_at_report: None,
    });
    std::fs::write(
        dir.join("hashes.json"),
        serde_json::to_string_pretty(&hashes)?,
    )?;

    Ok(dir)
}

// Human: Convenience wrapper using shared AppState event log.
// Agent: CALLS write_debug_bundle with state.event_log path.
pub fn write_debug_bundle_for_state(
    data_root: &Path,
    state: &AppState,
    request: &DebugBundleRequest,
) -> std::io::Result<PathBuf> {
    write_debug_bundle(data_root, &state.event_log, request)
}

fn utc_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
