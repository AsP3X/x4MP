// Human: CLI for the M0.75 Q3 divergence spike. Reads a session event.log that captured
// Human: spike.worldsnap from TWO unsynced X4 instances and prints how far their shared sector
// Human: state drifted over the run.
// Agent: READS --log (direct path) or --session + --data-dir; CALLS divergence::compute_divergence.
// Agent: event.log lives in the server's docker volume; copy it out first, e.g.:
// Agent:   docker cp x4mp-server:/data/sessions/<id>/event.log ./q3.log
// Agent:   cargo run -p x4mp-replay --bin q3_divergence -- --log ./q3.log

use std::path::PathBuf;

use clap::Parser;
use uuid::Uuid;
use x4mp_replay::divergence::{compute_divergence, load_worldsnaps};

#[derive(Parser, Debug)]
#[command(name = "q3-divergence")]
#[command(about = "Diff two instances' spike.worldsnap streams from a session event.log")]
struct Args {
    /// Direct path to an event.log (takes precedence over --session/--data-dir).
    #[arg(long)]
    log: Option<PathBuf>,

    /// Session UUID under <data-dir>/sessions/<id>/event.log.
    #[arg(long)]
    session: Option<Uuid>,

    /// Data root containing sessions/<id>/event.log.
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let log_path = match (&args.log, &args.session) {
        (Some(p), _) => p.clone(),
        (None, Some(id)) => args
            .data_dir
            .join("sessions")
            .join(id.to_string())
            .join("event.log"),
        (None, None) => {
            eprintln!("error: provide --log <path> or --session <uuid>");
            std::process::exit(2);
        }
    };

    let snaps = load_worldsnaps(&log_path)?;
    println!("Loaded {} worldsnap samples from {}", snaps.len(), log_path.display());

    let report = match compute_divergence(&snaps) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("divergence: {e}");
            std::process::exit(1);
        }
    };

    println!("Instance A = {}", report.sender_a);
    println!("Instance B = {}", report.sender_b);
    println!();
    println!(
        "{:>4}  {:>9}  {:>9}  {:>11}  {:>13}  sector (A | B)",
        "seq", "ships A/B", "Δships", "stations A/B", "Δstations"
    );
    for row in &report.rows {
        let sector = if row.sector_mismatch() {
            format!("{} | {}", row.sector_a, row.sector_b)
        } else {
            row.sector_a.clone()
        };
        println!(
            "{:>4}  {:>4}/{:<4}  {:>9}  {:>5}/{:<5}  {:>13}  {}",
            row.snap_seq,
            row.ships_a,
            row.ships_b,
            row.ship_delta(),
            row.stations_a,
            row.stations_b,
            row.station_delta(),
            sector,
        );
    }

    println!();
    println!("Aligned snapshots : {}", report.rows.len());
    println!("Max ship delta    : {}", report.max_ship_delta);
    println!("Max station delta : {}", report.max_station_delta);
    println!("Sector mismatch   : {}", report.any_sector_mismatch);
    println!("Max time skew (s) : {:.1}", report.max_time_skew);
    if report.max_time_skew > 7.5 {
        println!(
            "  note: time skew > one snapshot interval — instances were toggled at different sim \
             times, so seq-aligned rows compare slightly different moments."
        );
    }

    Ok(())
}
