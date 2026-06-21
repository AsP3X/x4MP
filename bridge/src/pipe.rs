use tokio::io::{AsyncBufReadExt, BufReader};

use crate::ws_client::{perform_handshake, send_and_receive, BridgeConfig, BridgeError};

// Human: M0 stdin/stdout NDJSON stand-in for the Named Pipe (M1 replaces this).
// Agent: READS stdin lines AFTER handshake.ack; WRITES echoed envelopes to stdout.
pub async fn run_stdin_pipe() -> Result<(), BridgeError> {
    let config = BridgeConfig::from_env();
    let (_handshake, mut ws) = perform_handshake(&config).await?;

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await.map_err(|e| BridgeError::Recv(e.to_string()))? {
        if line.trim().is_empty() {
            continue;
        }
        let echoed = send_and_receive(&mut ws, &line).await?;
        println!("{}", serde_json::to_string(&echoed).unwrap_or_default());
    }

    // Stdin EOF: drain in-flight reply if any, then close cleanly.
    let _ = ws.close(None).await;
    Ok(())
}
