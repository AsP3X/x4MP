# Spike Q2 — Apply feasibility

**Status:** PENDING manual X4 measurement  
**Date:** _fill after test_

## Question

Can we spawn a proxy ship in a second instance and continuously update its position/rotation from external data smoothly?

**Pass criteria:** Proxy follows source path with visually acceptable interpolation at 5–10 Hz; no teleport-snapping unless snap mode is intentional.

## Setup

- [ ] Q1 `capture.ndjson` available
- [ ] Instance 2: spike mod + bridge
- [ ] Feeder: `cargo run --manifest-path tools/spike-feeder/Cargo.toml -- --input capture.ndjson --hz 10`
- [ ] Signalled `SpikeApply_SpawnProxy` then `SpikeApply_Start`

## Approaches tested

### (a) Snap — direct position set each sample

| Metric | Measured |
|--------|----------|
| Visual quality (1–5) | _TBD_ |
| Max stable rate (Hz) | _TBD_ |
| Side effects (physics/collision) | _TBD_ |

### (b) Lerp — interpolate between samples

| Metric | Measured |
|--------|----------|
| Visual quality (1–5) | _TBD_ |
| Required lerp window (ms) | _TBD_ |
| Max stable rate (Hz) | _TBD_ |

## APIs used

| Action | API / cue | Notes |
|--------|-----------|-------|
| Spawn proxy | MD `<create_ship>` | macro `ship_xs_xperimental_01_a_macro` — adjust if invalid |
| Set position | MD `set_object_position` | _confirm works for player-visible movement_ |
| Inbound samples | SirNuke `Pipe.read` / bridge stdin | |

## Observations

_Describe judder, snapping, sector mismatch, proxy despawn, etc._

## Verdict

- [ ] **PASS**
- [ ] **PASS (revised)** — interpolation mandatory / rate capped at _ Hz
- [ ] **FAIL** — blocker: _
