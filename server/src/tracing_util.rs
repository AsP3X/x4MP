use tracing::Span;
use uuid::Uuid;

// Human: Build tracing spans with correlation ids for observability-debugging.mdc.
// Agent: EMITS trace_id, session_id, client_id, seq on every inbound WS message.
pub fn ws_message_span(
    trace_id: Uuid,
    session_id: Option<Uuid>,
    client_id: Option<Uuid>,
    seq: Option<u64>,
) -> Span {
    tracing::info_span!(
        "ws_message",
        trace_id = %trace_id,
        session_id = session_id.map(|id| id.to_string()).unwrap_or_default(),
        client_id = client_id.map(|id| id.to_string()).unwrap_or_default(),
        seq = seq.unwrap_or(0),
    )
}
