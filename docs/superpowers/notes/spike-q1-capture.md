# Spike Q1 — Capture feasibility

**Status:** PENDING manual X4 measurement  
**Date:** _fill after test_  
**Tester:** _name_  
**X4 version:** _e.g. 7.5_  
**Spike mod version:** `0.0.1-spike`

## Question

Can we read a player ship's position/rotation/velocity at 5–10 Hz from Lua/MD and push it over the pipe?

**Pass criteria:** Stable 5–10 Hz sample stream, <100 ms capture latency, no UI stutter.

## Setup

- [ ] SirNuke Mod Support APIs installed
- [ ] Protected UI Mode **disabled**
- [ ] `extension/spike/` loaded as `x4mp_spike`
- [ ] Bridge running: `cargo run -p x4mp-bridge` (stdout redirected to `capture.ndjson`)
- [ ] Signalled `md.X4MP_Spike_Capture.SpikeCapture_Start`

## APIs used

| Field | API | Notes |
|-------|-----|-------|
| Ship id | `GetPlayerPrimaryShipID()` | |
| Position | `GetComponentData(ship, "position")` | _confirm returns x,y,z_ |
| Rotation | `GetComponentData(ship, "rotation")` | _confirm yaw/pitch/roll_ |
| Sector | `GetComponentData(ship, "sector")` | |
| Speed | `GetComponentData(ship, "speed")` | |
| Tick source | MD `checkinterval="0.1s"` | _compare Lua frame hook if MD too slow_ |

## Measurements (2 minute flight)

| Metric | Target | Measured |
|--------|--------|----------|
| Sample count | 600–1200 | _TBD_ |
| Mean interval (ms) | 100–200 | _TBD_ |
| Jitter p95 (ms) | <50 | _TBD_ |
| Capture latency (ms) | <100 | _TBD_ |
| Frame hitch observed? | No | _TBD_ |

## Observations

_Describe stutter, log errors, pipe fallback vs SirNuke pipe, MD vs Lua tick quality._

## Verdict

- [ ] **PASS**
- [ ] **PASS (revised)** — note required changes: _
- [ ] **FAIL** — blocker: _
