# Spike Q3 — Local sim divergence

**Status:** POSTPONED — tooling built (capture cue + analyzer + tests); two-instance run not yet performed  
**Date:** _fill after test_

> **Postponed 2026-06-21:** the planned two-instance run is blocked because the **second machine cannot run X4**, so there is no way to produce two unsynced `spike.worldsnap` streams yet. The capture cue and the `q3_divergence` analyzer are committed and unit-tested; only the manual in-game measurement remains. Resume when a second X4-capable machine (or a viable one-machine two-instance setup, see Topology) is available.
>
> **In-game toggle not yet wired:** the menu (`extension/spike/md/x4mp_spike_menu.xml`) is mid-refactor and currently exposes **only** the Q1 capture toggle. Before the run, the **"World snapshot (Q3)"** toggle (and the Q2 apply toggle) must be re-added so `WorldSnap.$SnapActive` can be flipped in-game. Until then the worldsnap cue never fires.

## Question

Can we suppress or tolerate the client's local NPC sim so host snapshots are authoritative?

## Automated capture & diff (built in M0.75)

Instead of eyeballing, both instances emit **aggregate sector snapshots** that land in one server `event.log`, then a tool diffs them.

- **Capture** (in-game): `extension/spike/md/x4mp_spike_worldsnap.xml` — every ~15 s while toggled on, counts ships + stations in the **player's current sector** and emits `spike.worldsnap {seq, t, sector, ships, stations}` over the pipe. Toggle via Extension Options → **"X4MP Spike: World snapshot (Q3)"**.
- **Why aggregates**: NPC component ids are not stable across two separate X4 instances, so individual ships can't be matched 1:1. Counts/sums are directly comparable and answer the authority-boundary question.
- **Transport**: each instance's bridge connects to the same server, so its snapshots get a distinct `sender_id` → two labeled streams in one `event.log`.
- **Diff**: `cargo run -p x4mp-replay --bin q3_divergence -- --log <event.log>` groups by `sender_id`, aligns by snapshot `seq`, and prints per-snapshot `Δships`/`Δstations`, sector match, and max deltas + time skew.

### Topology requirement

Each instance needs its own bridge hosting pipe `x4mp_bridge`. Two bridges can't host the same pipe name on **one** machine, so:
- **Two machines (recommended)**: each runs X4 + its own bridge (pipe `x4mp_bridge`) → the **same** server.
  - **LAN**: machine 2 bridge `X4MP_SERVER_URL=ws://<host-lan-ip>:7878/ws`.
  - **Internet**: expose the host's port 7878 (router port-forward, or a TCP tunnel like `ngrok tcp 7878` / `cloudflared`) and point machine 2 at `ws://<public-host>:<port>/ws`. The dev server has only a join-code gate and no TLS — fine for a short spike, do not leave it open.
- **One machine**: requires distinct pipe names per instance (second mod copy emitting to e.g. `x4mp_bridge_b` + a second bridge) — extra setup.

## Procedure

1. Load **identical save** on two instances (no sync), both parked in the same NPC-heavy sector.
2. Toggle **"World snapshot (Q3)"** on **both** instances (ideally within a few seconds of each other so `seq` aligns).
3. Run **5 minutes**; toggle both off.
4. Copy the log out and diff:
   ```
   docker cp x4mp-server:/data/sessions/<session-id>/event.log ./q3.log
   cargo run -p x4mp-replay --bin q3_divergence -- --log ./q3.log
   ```
5. Record the timeline below; also note suppression levers tried:

| Lever | Feasible? | Notes |
|-------|-----------|-------|
| Remove local NPCs, host-driven proxies only | _TBD_ | |
| Pause local economy/job ticks | _TBD_ | |
| Accept NPC divergence; sync player ships only | _TBD_ | |

## Divergence timeline

| Elapsed | Observation |
|---------|-------------|
| 0 min | _TBD_ |
| 1 min | _TBD_ |
| 5 min | _TBD_ |

## v1 authority boundary (draft)

**Can be host-authoritative in v1:**

- _TBD after test_

**Must accept as divergent in v1:**

- _TBD after test_

## Verdict

- [ ] Suppression feasible for active sectors
- [ ] Tolerance boundary documented (player ships only)
- [ ] **Blocker** — requires architecture change: _
