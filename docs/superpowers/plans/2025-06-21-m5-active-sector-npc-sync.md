# M5: Active-Sector NPC & Background Economy Sync — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or subagent-driven-development. Every new event type MUST follow the `adding-replication-event` skill end-to-end.

**Goal:** Make NPC ships and background economy **coherent across clients in sectors that contain at least one connected player** ("active sectors"). The host simulates; clients render host-driven **proxies** and **suppress their own local NPC sim** for those sectors. Distant/unattended sectors remain host-save truth (no live sync) per the v1 reconciliation policy.

**This milestone delivers the "NPC ships and economy are synced too" experience for co-located players.** It does NOT attempt galaxy-wide NPC/economy sync — that is Phase B (research).

---

## HARD GATE: M0.75 Q3 decision

**This milestone cannot start until the M0.75 Q3 divergence spike is resolved** — `docs/superpowers/notes/spike-q3-divergence.md` and `docs/superpowers/notes/spike-decision.md`.

| Q3 outcome | Effect on M5 |
|------------|--------------|
| **Suppression feasible** (remove/disable local NPCs, pause local economy ticks in active sectors) | Full M5 as written: NPC ships + background economy synced in active sectors. |
| **GO (revised)** — partial suppression / tolerance boundary | M5 scoped to what the boundary allows (e.g. snapshot only large/combat NPCs; accept civilian traffic divergence). Update Task 2 scope accordingly. |
| **NO-GO** — no usable suppression | **M5 NPC-ship sync is descoped.** Fall back to player-piloted ships only (M1). Economy reduces to discrete events (M4) + checkpoint truth. Record this in the spec before any code. |

> Q3 is currently **PENDING** (`spike-decision.md`). Do not write `npc.*` capture/apply code until the verdict is recorded. The fallback ladder rung 3 ("player-piloted ships only; accept NPC divergence") is the explicit NO-GO landing zone.

**Depends on:** M1 (entity map + proxy apply), M3 (save/hash, locked time), M4 (economy events) all complete.

**Design spec:** `docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md` § Client simulation stance, § Authority, § Core event types (NPC host-only), § MVP Sync Scope (item 6), § Desync detection.

---

## Authority model (binding)

| Event | Authority | Direction |
|-------|-----------|-----------|
| `npc.sector_snapshot` | **host** | host → clients (read-only) |
| `npc.faction_tick` | **host** | host → clients (read-only) |

Clients **cannot** emit `npc.*`. Server ACL rejects any client-originated `npc.*`. Clients apply read-only and never re-emit.

---

## Task 1: Proto types + fixtures

**Files:**
- Create: `proto/src/events/npc.rs`; register in `proto/src/events/mod.rs`
- Fixtures: `proto/tests/fixtures/{npc.sector_snapshot,npc.faction_tick}.json`; schemas under `proto/schema/`
- Modify: `proto/tests/fixtures_test.rs`

- [ ] `npc.sector_snapshot` — full or delta entity list for one active sector:
  ```
  npc.sector_snapshot {
    sector_entity_id, snapshot_kind: "full"|"delta", seq_in_sector,
    entities: [ { entity_id, kind, macro, faction_id?, position:[x,y,z],
                  rotation:[qx,qy,qz,qw], velocity:[vx,vy,vz], hp_pct } ],
    removed: [ entity_id ]   // for delta snapshots
  }
  ```
- [ ] `npc.faction_tick` — coarse periodic background-economy summary (for map/stats UI, not per-ship animation):
  ```
  npc.faction_tick { faction_id, wallet?, relation_changes:[..], sector_production:[{sector_entity_id, ware, rate}] }
  ```
- [ ] All entity references are session `entity_id` UUIDs. Host assigns on first sighting (X4 ids are NOT stable across instances — this is why Q3 worldsnap compared aggregate counts, not per-ship identity).
- [ ] `cargo test -p x4mp-proto` PASS.

## Task 2: Active-sector lifecycle (host)

**Files:** `extension/md/x4mp_capture.xml` (+ new `extension/md/x4mp_active_sector.xml`), `extension/ui/x4mp_pipe.lua`. Use skill `authoring-x4-md-lua-hooks`. Reuse the proven `<warp>` + interpolation apply pattern from the Q2 spike (`spike-q2-apply.md`).

State machine (see also the design spec):

```
Dormant --(player enters sector)--> Activating
Activating --(host emits FULL npc.sector_snapshot)--> Active
Active --(periodic DELTA npc.sector_snapshot @ 1-3 Hz)--> Active
Active --(last player leaves)--> Deactivating
Deactivating --(tear down proxies, stop snapshots)--> Dormant
```

- [ ] Host detects sector activation (a connected player's sector changes) via MD cue.
- [ ] On activation: emit a **full** `npc.sector_snapshot` (catch-up burst) for that sector.
- [ ] Steady state: emit **delta** snapshots at a capped rate (start 1–3 Hz for NPCs; player-piloted ships keep their M1 5–10 Hz path). Include `removed[]` for despawned/destroyed entities.
- [ ] **Bandwidth budget:** cap entities-per-snapshot and rate; if a sector exceeds budget, prioritize (combat > large ships > civilian traffic) and document the cap. This is the main scalability risk — measure it.
- [ ] Emit `npc.faction_tick` on a slow cadence (e.g. every 10–30 s) for background economy summaries.

## Task 3: Client apply + local-sim suppression

**Files:** `extension/md/x4mp_apply.xml`, suppression module per Q3 verdict.

- [ ] On **full** snapshot: suppress/remove local NPCs the host owns in that sector; spawn a plain proxy (no pilot/order) per host entity; map `entity_id` → local proxy id.
- [ ] On **delta** snapshot: update existing proxies via `<warp>` + client-side interpolation buffer; create new, remove `removed[]`.
- [ ] **Suppression** (the Q3-gated core): apply whichever lever Q3 validated — remove locally-spawned NPCs and/or pause local economy/job ticks in active sectors so host snapshots are the only visible truth. If Q3 was GO (revised), apply only within the documented tolerance boundary.
- [ ] On sector **deactivation**: tear down proxies; local sim may resume cosmetically (irrelevant until re-entry; host save is truth).
- [ ] Apply `npc.faction_tick` to map/economy UI only — do not spawn entities from it.

## Task 4: Desync detection for NPC/economy

**Files:** server desync hook (`server/src/debug_bundle.rs` already stubs the bundle), `extension/ui/x4mp_debug_overlay.lua`.

- [ ] Extend `world.hash` components to include cheap active-sector aggregates (entity counts per active sector, key synced-station storage totals, faction relation checksum) — comparable across instances (mirrors the Q3 aggregate approach).
- [ ] On mismatch: request a fresh **full** `npc.sector_snapshot` for the active sector; write debug bundle (`hashes.json` host vs reporter); overlay warns the affected client.
- [ ] Overlay shows active-sector count, last applied snapshot `seq_in_sector`, and NPC proxy count.

## Task 5: Host migration (paired M5 item)

The roadmap pairs host migration with M5. Keep it a **separate tracked sub-task**; do not block NPC sync on it.

- [ ] Document the migration flow: pause sim → checkpoint → promote a client to host → re-establish authority for active sectors → resume. Implement only after NPC sync is stable, or defer to v1.5 per design spec § Host disconnect.

## Task 6: Verification

- [ ] `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` green.
- [ ] Harness: `npc.sector_snapshot` / `npc.faction_tick` broadcast to clients with server `seq`; client-originated `npc.*` rejected.
- [ ] Manual 2-instance LAN smoke (document in PR body): both players in the same NPC-heavy sector see the **same** NPC ships at the same positions (within interpolation tolerance); a destroyed NPC disappears on both; civilian/background divergence stays within the Q3-documented boundary; leaving and re-entering the sector re-syncs via a full snapshot.
- [ ] Re-run the Q3 `q3_divergence` diff with suppression enabled to confirm active-sector deltas stay near zero.

### Done criteria

1. M0.75 Q3 verdict recorded; M5 scope matches it (full / revised / descoped).
2. `npc.sector_snapshot` + `npc.faction_tick` defined with fixtures + passing tests.
3. Host streams full-on-entry + capped-rate delta snapshots for active sectors only; bandwidth cap measured and documented.
4. Clients render host proxies and suppress local NPC sim within the Q3 boundary; sector enter/leave re-syncs cleanly.
5. NPC/economy `world.hash` mismatch triggers snapshot refresh + debug bundle; overlay surfaces it.
6. Workspace tests + clippy green; manual LAN smoke documented; design spec § Client simulation stance updated to reflect the shipped boundary.
