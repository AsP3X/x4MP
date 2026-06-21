// Human: Q3 divergence analyzer — reads a session event.log, splits the two instances'
// Human: spike.worldsnap streams, aligns them by snapshot seq, and reports how far the shared
// Human: world state (sector ship/station counts) drifted between two unsynced X4 instances.
// Agent: READS event.log via x4mp_server::read_event_log; groups by EventEnvelope.sender_id
// Agent: (each instance = distinct server-assigned id). THROWAWAY M0.75 spike tooling; aggregate
// Agent: counts only (NPC component ids are not stable across instances, so no per-entity match).

use std::collections::BTreeMap;
use std::path::Path;

use uuid::Uuid;
use x4mp_server::read_event_log;

// Human: One decoded spike.worldsnap sample from one instance.
// Agent: snap_seq = payload.$seq (per-instance counter from MD); t = player.age seconds (align key).
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub sender: Uuid,
    pub snap_seq: u64,
    pub t: f64,
    pub sector: String,
    pub ships: i64,
    pub stations: i64,
}

// Human: Read all spike.worldsnap samples (ignores started/stopped control messages).
// Agent: RETURNS samples in log order; a line is a sample only if it has a numeric "ships" field.
pub fn load_worldsnaps(path: &Path) -> std::io::Result<Vec<Snapshot>> {
    let envelopes = read_event_log(path)?;
    Ok(envelopes
        .into_iter()
        .filter(|env| env.event_type == "spike.worldsnap")
        .filter_map(|env| {
            let p = &env.payload;
            // started/stopped carry no "ships"; skip them.
            let ships = p.get("ships").and_then(|v| v.as_i64())?;
            Some(Snapshot {
                sender: env.sender_id,
                snap_seq: p.get("seq").and_then(|v| v.as_u64()).unwrap_or(0),
                t: p.get("t").and_then(|v| v.as_f64()).unwrap_or(0.0),
                sector: p
                    .get("sector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                ships,
                stations: p.get("stations").and_then(|v| v.as_i64()).unwrap_or(-1),
            })
        })
        .collect())
}

// Human: One aligned comparison between the two instances at the same snapshot seq.
#[derive(Debug, Clone, PartialEq)]
pub struct DivergenceRow {
    pub snap_seq: u64,
    pub t_a: f64,
    pub t_b: f64,
    pub sector_a: String,
    pub sector_b: String,
    pub ships_a: i64,
    pub ships_b: i64,
    pub stations_a: i64,
    pub stations_b: i64,
}

impl DivergenceRow {
    pub fn ship_delta(&self) -> i64 {
        (self.ships_a - self.ships_b).abs()
    }
    pub fn station_delta(&self) -> i64 {
        (self.stations_a - self.stations_b).abs()
    }
    pub fn sector_mismatch(&self) -> bool {
        self.sector_a != self.sector_b
    }
}

// Human: Full divergence report for a two-instance Q3 run.
#[derive(Debug, Clone, PartialEq)]
pub struct DivergenceReport {
    pub sender_a: Uuid,
    pub sender_b: Uuid,
    pub rows: Vec<DivergenceRow>,
    pub max_ship_delta: i64,
    pub max_station_delta: i64,
    pub any_sector_mismatch: bool,
    /// Largest |t_a - t_b| across aligned rows — large values mean the two instances were toggled
    /// at noticeably different sim times, so seq-aligned rows compare different moments.
    pub max_time_skew: f64,
}

#[derive(Debug, PartialEq)]
pub enum DivergenceError {
    /// Need exactly two instances; found this many distinct sender ids with worldsnap samples.
    NotTwoSenders(usize),
    /// No overlapping snapshot seq between the two instances.
    NoOverlap,
}

impl std::fmt::Display for DivergenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DivergenceError::NotTwoSenders(n) => write!(
                f,
                "expected worldsnap streams from exactly 2 instances, found {n} (run the snapshot toggle on both instances against the same server)"
            ),
            DivergenceError::NoOverlap => {
                write!(f, "the two instances share no snapshot seq to compare")
            }
        }
    }
}

impl std::error::Error for DivergenceError {}

// Human: Group samples by instance, align by snapshot seq, and summarize drift.
// Agent: Picks the two senders with the most samples (ignores stray third connections like replay
// Agent: bots). Aligns on snap_seq (both instances start the counter at toggle); reports t skew so
// Agent: a misaligned toggle is visible. RETURNS rows sorted by snap_seq plus max deltas.
pub fn compute_divergence(snaps: &[Snapshot]) -> Result<DivergenceReport, DivergenceError> {
    let mut by_sender: BTreeMap<Uuid, Vec<&Snapshot>> = BTreeMap::new();
    for s in snaps {
        by_sender.entry(s.sender).or_default().push(s);
    }
    if by_sender.len() < 2 {
        return Err(DivergenceError::NotTwoSenders(by_sender.len()));
    }

    // Choose the two streams with the most samples (robust against a stray extra connection).
    let mut senders: Vec<(Uuid, usize)> =
        by_sender.iter().map(|(id, v)| (*id, v.len())).collect();
    senders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let sender_a = senders[0].0;
    let sender_b = senders[1].0;

    let index = |id: &Uuid| -> BTreeMap<u64, &Snapshot> {
        by_sender[id].iter().map(|s| (s.snap_seq, *s)).collect()
    };
    let map_a = index(&sender_a);
    let map_b = index(&sender_b);

    let mut rows = Vec::new();
    let mut max_ship_delta = 0;
    let mut max_station_delta = 0;
    let mut any_sector_mismatch = false;
    let mut max_time_skew = 0.0_f64;

    for (seq, a) in &map_a {
        if let Some(b) = map_b.get(seq) {
            let row = DivergenceRow {
                snap_seq: *seq,
                t_a: a.t,
                t_b: b.t,
                sector_a: a.sector.clone(),
                sector_b: b.sector.clone(),
                ships_a: a.ships,
                ships_b: b.ships,
                stations_a: a.stations,
                stations_b: b.stations,
            };
            max_ship_delta = max_ship_delta.max(row.ship_delta());
            max_station_delta = max_station_delta.max(row.station_delta());
            any_sector_mismatch |= row.sector_mismatch();
            max_time_skew = max_time_skew.max((a.t - b.t).abs());
            rows.push(row);
        }
    }

    if rows.is_empty() {
        return Err(DivergenceError::NoOverlap);
    }
    rows.sort_by_key(|r| r.snap_seq);

    Ok(DivergenceReport {
        sender_a,
        sender_b,
        rows,
        max_ship_delta,
        max_station_delta,
        any_sector_mismatch,
        max_time_skew,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(sender: Uuid, seq: u64, t: f64, ships: i64, stations: i64, sector: &str) -> Snapshot {
        Snapshot {
            sender,
            snap_seq: seq,
            t,
            sector: sector.to_string(),
            ships,
            stations,
        }
    }

    #[test]
    fn aligns_by_seq_and_tracks_max_deltas() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let snaps = vec![
            snap(a, 1, 10.0, 100, 13, "Black Hole Sun IV"),
            snap(b, 1, 10.0, 100, 13, "Black Hole Sun IV"),
            snap(a, 2, 25.0, 104, 13, "Black Hole Sun IV"),
            snap(b, 2, 25.0, 101, 13, "Black Hole Sun IV"),
            snap(a, 3, 40.0, 110, 13, "Black Hole Sun IV"),
            snap(b, 3, 40.0, 102, 12, "Black Hole Sun IV"),
        ];

        let report = compute_divergence(&snaps).unwrap();
        assert_eq!(report.rows.len(), 3);
        // seq 3: |110-102| = 8 ships, |13-12| = 1 station.
        assert_eq!(report.max_ship_delta, 8);
        assert_eq!(report.max_station_delta, 1);
        assert!(!report.any_sector_mismatch);
        assert_eq!(report.max_time_skew, 0.0);
    }

    #[test]
    fn detects_sector_mismatch_and_time_skew() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let snaps = vec![
            snap(a, 1, 10.0, 50, 5, "Argon Prime"),
            snap(b, 1, 13.0, 50, 5, "Hatikvah's Choice I"),
        ];
        let report = compute_divergence(&snaps).unwrap();
        assert!(report.any_sector_mismatch);
        assert!((report.max_time_skew - 3.0).abs() < 1e-9);
    }

    #[test]
    fn errors_without_two_senders() {
        let a = Uuid::from_u128(1);
        let snaps = vec![snap(a, 1, 10.0, 50, 5, "Argon Prime")];
        assert_eq!(
            compute_divergence(&snaps),
            Err(DivergenceError::NotTwoSenders(1))
        );
    }

    #[test]
    fn errors_when_no_overlapping_seq() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let snaps = vec![
            snap(a, 1, 10.0, 50, 5, "Argon Prime"),
            snap(b, 2, 25.0, 50, 5, "Argon Prime"),
        ];
        assert_eq!(compute_divergence(&snaps), Err(DivergenceError::NoOverlap));
    }
}
