# M0.75 Feasibility Spike — Experimental Code

**Not shipped in production X4MP.** Throwaway code to answer Q1–Q3 before M1.

## Prerequisites

- Two X4 installs (or two accounts) with **SirNuke Mod Support APIs**
- **Protected UI Mode disabled** (Extensions menu)
- M0 bridge running (`cargo run -p x4mp-bridge`) on each instance (stdin NDJSON until Named Pipe lands in M1)

## Install

Copy or symlink `extension/spike/` into each X4 extensions folder as a separate mod (`x4mp_spike`).

## Q1 — Capture (instance 1)

1. Start bridge: `cargo run -p x4mp-server` then `cargo run -p x4mp-bridge > capture.ndjson`
2. Load game with spike mod; signal `SpikeCapture_Start` (MD cue — bind via debug menu or `md.X4MP_Spike_Capture.SpikeCapture_Start`)
3. Fly for **2 minutes**; stop capture
4. Record sample rate / jitter in `docs/superpowers/notes/spike-q1-capture.md`

**Pass:** sustained 5–10 Hz, no UI stutter.

## Q2 — Apply (instance 2)

1. Run Q1 capture to produce `capture.ndjson`
2. Start bridge on instance 2
3. Feed samples: `cargo run --manifest-path tools/spike-feeder/Cargo.toml -- --input capture.ndjson --hz 10 | cargo run -p x4mp-bridge`
4. Signal `SpikeApply_SpawnProxy` then `SpikeApply_Start`
5. Compare **snap** vs **lerp** modes; document in `spike-q2-apply.md`

**Pass:** proxy visually tracks source at 5–10 Hz with acceptable interpolation.

## Q3 — Divergence

Run both instances on the same save for 5 minutes without sync. Document NPC divergence in `spike-q3-divergence.md`.

## Decision

Record GO / GO (revised) / NO-GO in `docs/superpowers/notes/spike-decision.md` after all measurements.
