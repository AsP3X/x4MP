// Human: Local store for events not yet acked by the server (reconnect resilience).
// Agent: M0 = in-memory VecDeque; M2 = disk-backed (SQLite) per design spec § Bridge offline buffer.
use std::collections::VecDeque;

use x4mp_proto::EventEnvelope;

pub trait OfflineBuffer: Send + Sync {
    fn push(&mut self, local_id: u64, envelope: EventEnvelope);
    fn drain(&mut self) -> Vec<EventEnvelope>;
}

// Human: In-memory offline buffer for M0 dev/testing.
// Agent: WRITES VecDeque; drain RETURNS queued envelopes in FIFO order.
pub struct InMemoryOfflineBuffer {
    queue: VecDeque<EventEnvelope>,
}

impl InMemoryOfflineBuffer {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Default for InMemoryOfflineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineBuffer for InMemoryOfflineBuffer {
    fn push(&mut self, _local_id: u64, envelope: EventEnvelope) {
        self.queue.push_back(envelope);
    }

    fn drain(&mut self) -> Vec<EventEnvelope> {
        self.queue.drain(..).collect()
    }
}
