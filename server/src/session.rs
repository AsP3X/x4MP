use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use uuid::Uuid;

use crate::event_log::EventLog;

// Human: M0 dev join code accepted by the server during handshake.
// Agent: READS join_code from handshake payload; mismatch -> INVALID_JOIN_CODE.
pub const DEV_JOIN_CODE: &str = "ABCD-1234";

// Human: Fixed session id for the single M0 dev session.
// Agent: WRITES all event.log entries under this session directory.
pub const DEV_SESSION_ID: Uuid = uuid::uuid!("11111111-1111-1111-1111-111111111111");

/// Shared listener state: one session, monotonic seq, append-only log.
pub struct AppState {
    pub session_id: Uuid,
    pub seq: AtomicU64,
    pub event_log: EventLog,
}

impl AppState {
    // Human: Construct shared state for a new listener binding.
    // Agent: READS data_root; WRITES event.log path for session_id.
    pub fn new(data_root: impl AsRef<std::path::Path>, session_id: Uuid) -> std::io::Result<Arc<Self>> {
        let event_log = EventLog::for_session(data_root, session_id)?;
        Ok(Arc::new(Self {
            session_id,
            seq: AtomicU64::new(0),
            event_log,
        }))
    }

    // Human: Assign the next server seq for an inbound event.
    // Agent: RETURNS strictly increasing u64 shared across all M0 clients.
    pub fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }
}
