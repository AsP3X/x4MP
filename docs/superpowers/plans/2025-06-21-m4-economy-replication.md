# M4: Economy Replication (Trade, Build, Faction) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or subagent-driven-development. Every new event type MUST follow the `adding-replication-event` skill end-to-end (proto → fixture → ACL → host capture → client apply → harness test).

**Goal:** Replicate **player- and faction-driven economy state** — trades, station construction, storage changes, and faction membership — as host-validated, event-sourced changes so all clients converge on the same shared-asset state.

**Scope boundary:** This milestone covers the **discrete, player-initiated** half of "synced economy." Continuous **background NPC economy** (faction wallets, autonomous NPC trade, production ticks) and **NPC ship motion** are M5 (`npc.faction_tick`, `npc.sector_snapshot`) and are gated on the M0.75 Q3 suppression decision. Do not implement `npc.*` events here.

**Depends on:**
- M1 complete — entity-ID map (`entity_id` UUID ↔ host X4 id) and the MD/Lua apply pipeline exist. Economy events are useless without stable entity ids for stations.
- M2 complete — client→server→host ack path and `session.resume` replay (economy events must replay on reconnect).
- M3 complete — faction ACL, locked sim time, save/hash. ACL roles (`builder`, `treasurer`, `leader`, `pilot`) are defined here.

If any dependency is missing, stop and finish it first — economy events have no consistent target or authority model without M1–M3.

**Design spec:** `docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md` § Authority, § Permissions, § Core event types, § MVP Sync Scope (items 3–5).

---

## Authority model (binding for this milestone)

| Event | Originating authority | Validated by | Applied by |
|-------|----------------------|--------------|------------|
| `trade.offer` | client (acting player) | server ACL + host | host commits, broadcasts result |
| `trade.executed` | **host** | host (truth) | all clients (read-only apply) |
| `station.build_start` | client (faction `builder`) | server ACL + host | all clients |
| `station.build_complete` | **host** | host | all clients |
| `station.storage_delta` | **host** | host | all clients |
| `faction.create` / `faction.join` / `faction.leave` | client | server ACL | server + all clients |
| `faction.relation_change` | client (faction `leader`) | server ACL + host | all clients |

**Rule:** a player **requests** an economy action; the **host's world is the truth** that decides whether it happened and emits the authoritative outcome. Clients never originate `trade.executed`, `station.build_complete`, or `station.storage_delta` — those are host outcomes. Server rejects on authority/ACL mismatch with a stable `event.rejected` code.

---

## Task 1: Proto event types + fixtures

**Files:**
- Create: `proto/src/events/trade.rs`, `proto/src/events/station.rs`, `proto/src/events/faction.rs`
- Modify: `proto/src/events/mod.rs` (register type strings)
- Create fixtures: `proto/tests/fixtures/{trade.offer,trade.executed,station.build_start,station.build_complete,station.storage_delta,faction.create,faction.join,faction.leave,faction.relation_change}.json`
- Create schemas: matching `proto/schema/<type>.schema.json`
- Modify: `proto/tests/fixtures_test.rs` (round-trip each new type)

- [ ] Define flat, serde-friendly payloads. Use session `entity_id` (UUID) for every station/ship/ware reference — **never raw X4 ids**.
- [ ] Suggested payload shapes (refine against X4 getters during host capture):
  - `trade.offer { station_entity_id, ware, amount, price_each, side: "buy"|"sell" }`
  - `trade.executed { buyer_entity_id, seller_entity_id, station_entity_id, ware, amount, price_each, total }`
  - `station.build_start { station_entity_id, sector_entity_id, macro, modules: [..] }`
  - `station.build_complete { station_entity_id, module_entity_id }`
  - `station.storage_delta { station_entity_id, ware, delta, new_total }`
  - `faction.*` per design spec § Faction & Permissions
- [ ] Golden fixture = full `EventEnvelope` with realistic payload and correct `authority`.
- [ ] `cargo test -p x4mp-proto` PASS before proceeding.

## Task 2: Server validation + ACL

**Files:**
- Modify: server event handler + ACL module
- Test: `server/tests/two_client_harness.rs` (extend) and/or a new `server/tests/economy_acl.rs`

- [ ] Enforce the authority table above: reject client-originated host-only types.
- [ ] Permission matrix checks (design spec § Permissions):
  - Build module → faction `builder` or station owner
  - Withdraw/spend faction funds → `treasurer`
  - Diplomacy / `faction.relation_change` → `leader`
- [ ] Validate `timestamp_sim` within `SIM_TIME_TOLERANCE` of locked sim time.
- [ ] Rate limit per player (reuse the 20 events/sec order cap; trades share the budget).
- [ ] On reject → `event.rejected` / `WsErrorFrame` with a stable `code` (never a bare close). Codes logged for debug bundle `acl_decisions.json`.
- [ ] Tests: authorized actor accepted + broadcast with server-assigned `seq`; unauthorized actor rejected with expected code.

## Task 3: Host capture & emit (MD/Lua)

**Files:**
- Modify/Create: `extension/md/x4mp_capture.xml` (economy cues), `extension/ui/x4mp_pipe.lua`
- Use skill: `authoring-x4-md-lua-hooks`

- [ ] Capture cues on the **host** for: a trade completing at a player/shared station, a build start/complete, and storage changes on **synced** stations only (shared-faction or player-owned — not every NPC warehouse).
- [ ] Map X4 ids → `entity_id` via the entity map before emitting. First sighting requests a new `entity_id`.
- [ ] Emit `trade.executed` / `station.*` with `authority="host"`. Comment each cue: `<!-- Agent: EMITS station.storage_delta WHEN synced station ware count changes -->`.
- [ ] **Scope guard:** only stations flagged as session-synced emit `storage_delta`. Do not stream every NPC station's inventory — that is M5/Phase B economy summary territory.

## Task 4: Client apply (MD/Lua)

**Files:**
- Modify/Create: `extension/md/x4mp_apply.xml`

- [ ] Apply cues mutate local shared-station state **idempotently** (re-applying the same `seq` is a no-op). Read-only events are never re-emitted.
- [ ] `station.storage_delta` updates the local proxy/station record; UI reflects new totals.
- [ ] `trade.executed` updates wallet/inventory views for affected shared assets.
- [ ] Map incoming `entity_id` → local object before mutation; if unknown, request/queue until the station entity is known (sector-entry snapshot from M5 may resolve it).

## Task 5: Reconnect & persistence

- [ ] Confirm economy events flow through the M2 `session.resume { from_seq }` replay — a client that missed a trade/build replays it on reconnect and converges.
- [ ] Confirm economy state is captured by the M3 checkpoint (host save) so galaxy-wide economy truth persists even for stations no client currently observes.

## Task 6: Verification

- [ ] `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` green.
- [ ] Harness: two-client trade/build broadcast + ACL rejection scenarios pass.
- [ ] Manual 2-instance LAN smoke (document in PR body): player A buys at a shared station → B sees storage/wallet change; A starts a build → B sees build progress and completion; unauthorized B build attempt is rejected and surfaced.

### Done criteria

1. All Task-1 event types defined with golden fixtures + passing round-trip tests.
2. Server enforces the authority + permission matrix with stable reject codes and harness coverage.
3. Host emits authoritative economy outcomes for **synced** stations only; clients apply idempotently.
4. Economy events replay on reconnect (M2) and persist in checkpoints (M3).
5. Workspace tests + clippy green; manual LAN smoke documented.
6. Design spec § Core event types / § MVP Sync Scope updated if any payload or scope detail changed during implementation.
