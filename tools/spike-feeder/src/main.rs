use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use tokio::io::{AsyncWriteExt, stdout};

// Human: Replay Q1 capture NDJSON into instance-2 pipe via bridge stdin.
// Agent: READS capture file; WRITES lines to stdout at --hz rate for M0 bridge.
#[derive(Parser, Debug)]
#[command(name = "x4mp-spike-feeder")]
#[command(about = "Replay spike.ship_sample NDJSON lines for Q2 apply testing")]
struct Args {
    /// Capture NDJSON file from Q1 (one JSON object per line)
    #[arg(long)]
    input: PathBuf,

    /// Emit rate in Hz (default 10)
    #[arg(long, default_value_t = 10.0)]
    hz: f64,

    /// Loop the file when EOF is reached
    #[arg(long, default_value = "false")]
    r#loop: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let interval = Duration::from_secs_f64(1.0 / args.hz.max(0.1));
    let content = std::fs::read_to_string(&args.input)?;
    let lines: Vec<String> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();

    if lines.is_empty() {
        return Err(format!("no samples in {}", args.input.display()).into());
    }

    eprintln!(
        "feeding {} samples at {:.1} Hz (loop={})",
        lines.len(),
        args.hz,
        args.r#loop
    );

    let mut stdout = stdout();
    loop {
        for line in &lines {
            stdout.write_all(line.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
            tokio::time::sleep(interval).await;
        }
        if !args.r#loop {
            break;
        }
    }

    Ok(())
}
