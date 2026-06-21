use tracing_subscriber::EnvFilter;

// Human: Bridge binary — handshake then NDJSON stdin/stdout pipe loop.
// Agent: CALLS run_stdin_pipe; exits cleanly on stdin EOF.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("x4mp_bridge=info".parse()?))
        .init();

    if let Err(e) = x4mp_bridge::run_stdin_pipe().await {
        tracing::error!(error = %e, "bridge pipe loop failed");
        std::process::exit(1);
    }
    Ok(())
}
