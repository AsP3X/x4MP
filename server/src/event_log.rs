// Human: Append-only, newline-delimited JSON event log, one file per session.
// Agent: WRITES data/sessions/<session_id>/event.log; one EventEnvelope per line
//        AFTER the server assigns seq. RETURNS io::Result. Source of truth for
//        replay (tools/replay) and debug bundles (last_events.jsonl).
use std::io::Write;
use std::path::{Path, PathBuf};

use x4mp_proto::EventEnvelope;

pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    // Human: Open or create the session event log under the configured data root.
    // Agent: READS data_root env/default; WRITES event.log append-only.
    pub fn for_session(data_root: impl AsRef<Path>, session_id: uuid::Uuid) -> std::io::Result<Self> {
        let dir = data_root
            .as_ref()
            .join("sessions")
            .join(session_id.to_string());
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            path: dir.join("event.log"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    // Append one seq-stamped envelope as a single NDJSON line.
    // Note: serde_json::Error does not auto-convert into io::Error, so map it.
    pub fn append(&self, env: &EventEnvelope) -> std::io::Result<()> {
        let line = serde_json::to_string(env)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{line}")
    }

    // Human: Read the last NDJSON line for test assertions.
    // Agent: RETURNS None when empty; parses last line into EventEnvelope.
    pub fn read_last(&self) -> std::io::Result<Option<EventEnvelope>> {
        let content = std::fs::read_to_string(&self.path)?;
        let line = content.lines().last().filter(|l| !l.is_empty());
        match line {
            Some(l) => {
                let env: EventEnvelope = serde_json::from_str(l).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })?;
                Ok(Some(env))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use x4mp_proto::{Authority, EventEnvelope};

    #[test]
    fn append_and_read_last() {
        let tmp = std::env::temp_dir().join(format!("x4mp-eventlog-{}", Uuid::new_v4()));
        let session_id = Uuid::new_v4();
        let log = EventLog::for_session(&tmp, session_id).unwrap();
        let env = EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id,
            seq: 1,
            timestamp_sim: 0.0,
            sender_id: Uuid::new_v4(),
            authority: Authority::Client,
            payload: serde_json::json!({}),
        };
        log.append(&env).unwrap();
        let last = log.read_last().unwrap().unwrap();
        assert_eq!(last.seq, 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
