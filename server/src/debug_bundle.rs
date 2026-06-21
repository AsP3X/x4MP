use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use x4mp_proto::EventEnvelope;

// Human: Write debug bundle on rejection (M0 stub; M0.5 expands sampling).
// Agent: WRITES data/debug_bundle/<timestamp>/meta.json + last_events.jsonl.
pub fn write_rejection_bundle(
    session_id: Uuid,
    reason: &str,
    data_root: &Path,
    recent_events: &[EventEnvelope],
) -> std::io::Result<()> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let dir = data_root.join("debug_bundle").join(ts.to_string());
    std::fs::create_dir_all(&dir)?;

    let meta = serde_json::json!({
        "session_id": session_id,
        "reason": reason,
    });
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

    let mut lines = String::new();
    for env in recent_events {
        lines.push_str(&serde_json::to_string(env).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?);
        lines.push('\n');
    }
    std::fs::write(dir.join("last_events.jsonl"), lines)?;
    Ok(())
}
