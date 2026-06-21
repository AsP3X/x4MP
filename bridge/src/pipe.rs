use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ws_client::{perform_handshake, send_and_receive, BridgeConfig, BridgeError, HandshakeResult};

#[cfg(windows)]
use crate::named_pipe_win;

// Human: Normalize spike NDJSON or pass through full EventEnvelope lines for the server.
// Agent: WRAPS bare {"type":"..."} objects into envelope shape; RETURNS JSON string for WS send.
fn normalize_inbound_line(
    line: &str,
    session_id: Uuid,
    client_id: Uuid,
) -> Result<String, BridgeError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::Parse("empty pipe line".into()));
    }

    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| BridgeError::Parse(e.to_string()))?;

    if value.get("v").is_some() && value.get("type").is_some() {
        return Ok(trimmed.to_string());
    }

    let event_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| BridgeError::Parse("pipe line missing type field".into()))?
        .to_string();

    let envelope = serde_json::json!({
        "v": 1,
        "type": event_type,
        "session_id": session_id,
        "seq": 0,
        "timestamp_sim": 0.0,
        "sender_id": client_id,
        "authority": "host",
        "payload": value,
    });

    serde_json::to_string(&envelope).map_err(|e| BridgeError::Serialize(e.to_string()))
}

// Human: Forward one normalized line to the server and print the echoed envelope.
// Agent: CALLS send_and_receive; WRITES JSON line to stdout for dev visibility.
async fn forward_line(
    handshake: &HandshakeResult,
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    line: &str,
) -> Result<(), BridgeError> {
    let normalized = normalize_inbound_line(line, handshake.session_id, handshake.client_id)?;
    tracing::info!(
        event_type = %serde_json::from_str::<serde_json::Value>(&normalized)
            .ok()
            .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
            .unwrap_or_else(|| "unknown".into()),
        "forwarding pipe line to server"
    );
    let echoed = send_and_receive(ws, &normalized).await?;
    println!("{}", serde_json::to_string(&echoed).unwrap_or_default());
    Ok(())
}

// Human: M0 pipe loop — stdin or Windows named pipe (SirNuke) after handshake.
// Agent: READS X4MP_PIPE_NAME on Windows; WRITES echoed envelopes to stdout.
pub async fn run_stdin_pipe() -> Result<(), BridgeError> {
    let config = BridgeConfig::from_env();
    let (handshake, mut ws) = perform_handshake(&config).await?;

    let pipe_name = std::env::var("X4MP_PIPE_NAME").ok();

    #[cfg(windows)]
    if let Some(name) = pipe_name.as_ref() {
        if !name.trim().is_empty() {
            return run_named_pipe_bridge(handshake, ws, name.trim()).await;
        }
    }

    #[cfg(not(windows))]
    if pipe_name.is_some() {
        tracing::warn!("X4MP_PIPE_NAME is set but named pipes are Windows-only; using stdin");
    }

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| BridgeError::Recv(e.to_string()))?
    {
        if line.trim().is_empty() {
            continue;
        }
        forward_line(&handshake, &mut ws, &line).await?;
    }

    let _ = ws.close(None).await;
    Ok(())
}

// Human: Host \\.\pipe\<name> on a blocking thread; async task forwards to WebSocket.
// Agent: READS SirNuke pipe messages; CALLS forward_line until channel closes.
#[cfg(windows)]
async fn run_named_pipe_bridge(
    handshake: HandshakeResult,
    mut ws: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    pipe_name: &str,
) -> Result<(), BridgeError> {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let pipe_name_owned = pipe_name.to_string();
    let reader = std::thread::spawn(move || named_pipe_win::serve_named_pipe(&pipe_name_owned, tx));

    while let Some(line) = rx.recv().await {
        if let Err(err) = forward_line(&handshake, &mut ws, &line).await {
            tracing::warn!(error = %err, "failed to forward pipe line; continuing");
        }
    }

    let _ = ws.close(None).await;
    match reader.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::warn!(error = %err, "named pipe server exited with error"),
        Err(_) => tracing::warn!("named pipe server thread panicked"),
    }
    Ok(())
}
