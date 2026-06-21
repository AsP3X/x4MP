use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use x4mp_replay::ReplayConfig;

// Human: CLI entry for replaying session event logs against a running server.
// Agent: READS clap args; CALLS replay_events; logs stats on success.
#[derive(Parser, Debug)]
#[command(name = "x4mp-replay")]
#[command(about = "Replay session event.log from a given seq against a WebSocket server")]
struct Args {
    /// Session UUID whose event.log to replay
    #[arg(long)]
    session: Uuid,

    /// Minimum seq to replay (inclusive)
    #[arg(long, default_value_t = 1)]
    from_seq: u64,

    /// WebSocket server URL
    #[arg(long, default_value = "ws://127.0.0.1:7878/ws")]
    server: String,

    /// Data root containing sessions/<id>/event.log
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,

    /// Display name for replay handshake
    #[arg(long, default_value = "ReplayBot")]
    display_name: String,

    /// Join code for replay handshake
    #[arg(long, default_value = "ABCD-1234")]
    join_code: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("x4mp_replay=info".parse()?))
        .init();

    let args = Args::parse();
    let config = ReplayConfig {
        session_id: args.session,
        from_seq: args.from_seq,
        server_url: args.server,
        data_root: args.data_dir,
        display_name: args.display_name,
        join_code: args.join_code,
    };

    let stats = x4mp_replay::replay_events(&config).await?;
    tracing::info!(
        events_sent = stats.events_sent,
        last_seq = ?stats.last_seq,
        "replay complete"
    );
    Ok(())
}
