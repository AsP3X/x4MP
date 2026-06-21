// Human: Append-only, newline-delimited JSON event log, one file per session.
// Agent: WRITES data/sessions/<session_id>/event.log; one EventEnvelope per line
//        AFTER the server assigns seq. RETURNS io::Result. Source of truth for
//        replay (tools/replay) and debug bundles (last_events.jsonl).
use std::io::Write;
use std::path::{Path, PathBuf};

use x4mp_proto::EventEnvelope;

pub const EVENT_LOG_TAIL_LIMIT: usize = 200;

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
    // Agent: Builds "line\n" then does ONE write_all. With O_APPEND/FILE_APPEND_DATA the OS
    // Agent: appends each (small) write atomically at EOF, so concurrent writers (e.g. two
    // Agent: clients in the Q3 two-instance run) never interleave a line with its newline.
    // Agent: A `writeln!` here could emit the body and the "\n" as two appends and corrupt NDJSON.
    pub fn append(&self, env: &EventEnvelope) -> std::io::Result<()> {
        let mut line = serde_json::to_string(env)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        line.push('\n');
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        f.write_all(line.as_bytes())
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

    // Human: Tail the last N NDJSON lines from this session log.
    // Agent: READS event.log; RETURNS parsed envelopes (skips blank lines).
    pub fn tail(&self, limit: usize) -> std::io::Result<Vec<EventEnvelope>> {
        tail_event_log(&self.path, limit)
    }
}

// Human: Tail NDJSON lines from any event.log path (used by debug bundles + replay).
// Agent: READS path; RETURNS up to `limit` trailing EventEnvelope rows.
pub fn tail_event_log(path: &Path, limit: usize) -> std::io::Result<Vec<EventEnvelope>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let lines: Vec<&str> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    let start = lines.len().saturating_sub(limit);
    let mut events = Vec::with_capacity(lines.len().saturating_sub(start));
    for line in &lines[start..] {
        let env: EventEnvelope = serde_json::from_str(line).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        events.push(env);
    }
    Ok(events)
}

// Human: Load all envelopes from a session log (replay CLI).
// Agent: READS event.log; RETURNS every parsed line in file order.
pub fn read_event_log(path: &Path) -> std::io::Result<Vec<EventEnvelope>> {
    tail_event_log(path, usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use x4mp_proto::{Authority, EventEnvelope};

    fn sample_env(session_id: Uuid, seq: u64) -> EventEnvelope {
        EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id,
            seq,
            timestamp_sim: 0.0,
            sender_id: Uuid::new_v4(),
            authority: Authority::Client,
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn append_and_read_last() {
        let tmp = std::env::temp_dir().join(format!("x4mp-eventlog-{}", Uuid::new_v4()));
        let session_id = Uuid::new_v4();
        let log = EventLog::for_session(&tmp, session_id).unwrap();
        let env = sample_env(session_id, 1);
        log.append(&env).unwrap();
        let last = log.read_last().unwrap().unwrap();
        assert_eq!(last.seq, 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn tail_returns_last_n_lines() {
        let tmp = std::env::temp_dir().join(format!("x4mp-eventlog-tail-{}", Uuid::new_v4()));
        let session_id = Uuid::new_v4();
        let log = EventLog::for_session(&tmp, session_id).unwrap();
        for seq in 1..=5 {
            log.append(&sample_env(session_id, seq)).unwrap();
        }
        let tail = log.tail(2).unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].seq, 4);
        assert_eq!(tail[1].seq, 5);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
