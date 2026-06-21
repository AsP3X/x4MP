use tracing_subscriber::EnvFilter;

// Human: Server binary entry — bind default addr and serve WebSocket echo.
// Agent: READS X4MP_BIND_ADDR; CALLS run_on_listener until shutdown.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("x4mp_server=info".parse()?))
        .init();

    let bind_addr = std::env::var("X4MP_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(%bind_addr, "x4mp-server listening");
    x4mp_server::run_on_listener(listener).await?;
    Ok(())
}
