# X4 Multiplayer Mod — Design Spec

**Status:** Approved  
**Date:** 2025-06-21  
**Project:** x4-multiplayer-mod (x4mp)

## Summary

A co-op multiplayer mod for **X4: Foundations** that lets 2+ players share one universe across separate machines. A **central replication server** coordinates sessions, permissions, saves, and an append-only event log. In v1, the **host player's X4 instance** remains world-authoritative; clients receive and apply replicated events. Long-term goal: server-authoritative simulation without X4 (Phase C).

## Requirements

| Decision | Choice |
|----------|--------|
| Co-op model | Shared universe — one galaxy, synced economy/NPCs |
| Time | Locked shared time; SETA disabled or unanimous consent |
| Factions | Hybrid — players can share a faction or run their own |
| Session | Session-based — universe runs while players are connected |
| Player identity | Player-chosen, session-unique `display_name` for easy identification |
| v1 authority | Host player's X4 instance simulates the world |
| Long-term | Server-authoritative sim without X4 (major future phase) |
| Tech stack | Rust (server + bridge), X4 extension (MD + Lua) |

## Architecture

### Components

**1. X4 Extension Mod (`extension/`)**

- MD scripts: capture game events, apply remote events
- Lua UI: host/join menu, player list, connection status, time-lock controls
- SirNuke **Named Pipes API** → local bridge (Windows)

**2. Local Bridge (`bridge/`)**

- Rust process spawned alongside X4
- Named pipes ↔ WebSocket to central server
- Reconnect, heartbeat, local event buffer (disk-backed from M2; interface stubbed in M0)

**3. Central Server (`server/`)**

- Session lifecycle (create, join, leave, kick)
- Append-only **event log** (source of truth per session)
- Shared save storage (checkpoints + session end)
- ACL: ship/station/faction permissions
- Validation and rate limiting

**4. Shared Protocol (`proto/`)**

- Event types, envelope schema, shared Rust types crate
- Golden JSON fixtures per event type
- Version constants for handshake (`PROTO_VERSION`)

**5. Dev & debug tooling (`tools/`)**

- `replay` — replay session event log from arbitrary `seq`
- `dev-echo` — local smoke script
- Debug bundle export on desync (server-side)

### Data flow

```
Host X4 ←pipe→ Host Bridge ←WS→ Server ←WS→ Client Bridge ←pipe→ Client X4
```

Host captures state changes → bridge serializes → server validates/logs/broadcasts → clients apply via MD/Lua.

### Authority (v1)

| Entity | Authority | Data direction |
|--------|-----------|----------------|
| Galaxy, sectors, NPC factions | Host X4 instance | Host → clients (broadcast, read-only on clients) |
| Player-owned ships | Owning player (server-validated) | Owner → server → others |
| Shared-faction stations | Faction members with role permissions | Acting member → server → others |
| Time speed | Host proposes; all clients must ack | Host → clients |

**Data direction is explicit and hybrid:** a client is authoritative for **its own player ship** and broadcasts that state outward; the host is authoritative for **NPC/shared world state** and broadcasts that outward. Neither side overwrites the other's authoritative entities.

Clients cannot emit `npc.*` or `host.*` events.

### Client simulation stance (critical)

X4 is **non-deterministic**: two instances loading the same save and running will diverge immediately. The client's local X4 sim of shared NPC/economy state is therefore **not trustworthy** and must not be presented as truth.

**v1 reconciliation policy (active-sector authority):**

- Only **sectors that contain at least one connected player** are host-authoritative and continuously reconciled. The host streams entity snapshots for these sectors; clients render host entities as **proxies** and suppress/ignore their own local sim results for those entities.
- **Distant/unattended sectors are not reconciled in v1.** The client's local sim there is cosmetic and explicitly out of scope; the host's save remains the single source of truth, restored on checkpoint/reload.
- **Full galaxy-wide NPC/economy sync is out of v1 scope** and moved to a research milestone (see Milestones). It is likely not achievable through MD/Lua at full scale and must be proven by a spike before it is promised.

This policy is the single biggest determinant of whether co-op feels coherent. It is validated by the **M0.75 feasibility spike** before any protocol work for M1 ship sync begins.

## Wire Protocol

### Transport

- Client ↔ Server: WebSocket over TLS (plain WS for dev)
- X4 ↔ Bridge: Named Pipes, **newline-delimited JSON (NDJSON)** — one JSON object per line. This matches SirNuke's message-oriented pipe API and the M0 stdin stub. (MessagePack/length-prefixed binary may be evaluated later behind a version bump.)
- v1 server message format: JSON; MessagePack optional later

### Connection lifecycle

1. Bridge opens WebSocket
2. **Handshake** — client sends `handshake` (versions, compatibility fingerprint, `join_code`, chosen `display_name`)
3. Server validates: `proto_version`, `game_version`, `mods_fingerprint`, `join_code`, and `display_name` (format + uniqueness). On any mismatch → error frame (hard stop, see codes below)
4. Server responds `handshake.ack` + `client_id` + assigned `session_id` + confirmed `display_name`
5. Host loads shared save → `WORLD_READY` + save hash
6. Clients download save → load locally → verify hash → `CLIENT_READY`
7. Server broadcasts `SESSION_START`
8. Heartbeat every 2s; 3 missed → offline
9. On reconnect: client sends `session.resume { from_seq }`; server replays log from `from_seq`

### Version & compatibility handshake

```json
{
  "v": 1,
  "type": "handshake",
  "payload": {
    "mod_version": "0.1.0",
    "proto_version": 1,
    "bridge_version": "0.1.0",
    "game_version": "7.5",
    "mods_fingerprint": "sha256:<hash of active extension ids+versions>",
    "join_code": "ABCD-1234",
    "display_name": "Alice"
  }
}
```

Rejection codes (hard stop, never proceed to game events):

| Code | Cause |
|------|-------|
| `INCOMPATIBLE_VERSION` | `proto_version` differs from server |
| `INCOMPATIBLE_GAME_VERSION` | `game_version` mismatch between players |
| `INCOMPATIBLE_MODS` | `mods_fingerprint` mismatch (different installed extension set) |
| `INVALID_JOIN_CODE` | Wrong or missing `join_code` for the session |
| `INVALID_NAME` | `display_name` fails format rules (see § Player names) |
| `NAME_TAKEN` | `display_name` already used by another connected player in the session |

`mod_version` mismatch may warn but proceed only if explicitly allowed in session config. The `mods_fingerprint` check is the primary defense against silent desync from differing mod sets. On `NAME_TAKEN`/`INVALID_NAME` the client is prompted to choose a different name and re-handshake — these are recoverable, not fatal.

### Event envelope

```json
{
  "v": 1,
  "type": "event.ship.order",
  "session_id": "uuid",
  "seq": 1042,
  "timestamp_sim": 123456.789,
  "sender_id": "player-uuid",
  "authority": "host",
  "payload": {}
}
```

- `seq`: monotonic per session (server-assigned on ingest)
- `timestamp_sim`: in-game time for ordering/replay. **Clients stamp events with the host's locked sim time** (received via the latest snapshot), not their own wall/sim clock.
- Server rejects events whose `timestamp_sim` exceeds current locked sim time by more than `SIM_TIME_TOLERANCE` (default 1.0s) to absorb jitter; smaller drift is accepted and ordered by `seq`.
- Optional dev field: `payload._debug.trace_id` for log correlation (see § Observability)

### Core event types (v1)

| Category | Events |
|----------|--------|
| Handshake | `handshake`, `handshake.ack` |
| Session | `session.create`, `session.join`, `session.leave`, `session.resume`, `session.start`, `session.end`, `session.save`, `session.ping` |
| Time | `time.set_speed`, `time.seta_request`, `time.seta_vote` |
| Player | `player.spawn`, `player.despawn`, `player.rename`, `player.control_claim`, `player.control_release` |
| Ship | `ship.state_snapshot`, `ship.order`, `ship.position_delta`, `ship.damaged`, `ship.destroyed` |
| Station | `station.build_start`, `station.build_complete`, `station.storage_delta` |
| Trade | `trade.executed`, `trade.offer` |
| Faction | `faction.create`, `faction.join`, `faction.leave`, `faction.relation_change` |
| NPC (host-only) | `npc.sector_snapshot`, `npc.faction_tick` |

### Sync strategy

- Host emits `ship.state_snapshot` at 5–10 Hz for player-controlled ships
- Clients emit `ship.order`; server forwards to host for validation
- Host emits authoritative ack or `event.rejected`
- Checkpoint every 5 min: save + event seq uploaded to server

**Scope guard (v1):** Only **directly player-piloted ships** are position-synced at 5–10 Hz. Fleets, subordinates, and AI-tasked owned ships are **not** per-ship synced in v1 — they follow host snapshots for their sector like any other entity. This prevents bandwidth blowup from large owned fleets.

### Resume & log retention

- The server **retains the per-session append-only event log** for the life of the session (and in checkpoints) so a reconnecting client can replay missed events.
- On `session.resume { from_seq }`, the server streams all events with `seq > from_seq` in order, then resumes live broadcast.
- If `from_seq` is older than the oldest retained event (log truncated to a checkpoint), the server instead sends a fresh snapshot + checkpoint save reference rather than partial replay.

## Faction & Permissions

### Player identity

- `player_id` (persistent account UUID)
- `session_player_id` (this session)
- `display_name` (player-chosen, human-readable — see § Player names)
- `faction_id` (nullable — independent if null)
- `controlled_entities[]`

### Player names

Every player picks their own `display_name` so they are easily identified across all UI (player list, ship nameplates, chat, debug overlay, logs).

- **Chosen at connect** in the handshake; confirmed in `handshake.ack`.
- **Format:** 1–24 characters after trimming; printable Unicode; collapsed internal whitespace. Fails → `INVALID_NAME`.
- **Unique per session** (case-insensitive). Collision → `NAME_TAKEN`; client re-prompts and re-handshakes. Uniqueness is what makes names a reliable identifier.
- **Changeable mid-session** via `player.rename { session_player_id, display_name }`; server re-validates (format + uniqueness) and broadcasts the new name, or rejects with the same codes.
- **Identity vs. display:** all wire references use `session_player_id`/`player_id` (stable). `display_name` is presentation only — never used as a key, so renames never break entity ownership or ACL.
- **Persistence:** the last-used name is remembered per `player_id` as a default for future sessions; the session value still wins on conflict.

### Faction types

| Type | Description |
|------|-------------|
| Independent | Own faction; session-configurable ally rules |
| Shared | Multiple players; shared stations/funds per roles |
| Joined NPC faction | Mirrors X4 rank system in server ACL |

### Permission matrix

| Action | Rule |
|--------|------|
| Pilot ship / give orders | Owner or faction `pilot` role |
| Build station module | Faction `builder` or station owner |
| Withdraw faction funds | Faction `treasurer` |
| Change diplomacy | Faction `leader` |
| SETA / time speed | Unanimous vote |
| Kick player | Host only |

### Proxy rendering

Remote ships render as **proxy entities** (model + nameplate showing the owner's `display_name`, interpolated position). Host snapshots drive loadout/AI state. Nameplates update live on `player.rename`.

## Entity ID mapping

X4 internal entity identifiers are **not stable across clients or save loads**. All wire payloads use session-scoped `entity_id` (UUID).

| Layer | Responsibility |
|-------|----------------|
| Host extension | On first sighting, request `entity_id` from bridge for each X4 object |
| Bridge / server | Maintain `session_entity_map`: `entity_id` ↔ host X4 id |
| Clients | Map incoming `entity_id` to local proxy object id on apply |

Mapping persisted in session meta and checkpoints. Never emit raw X4 ids in event payloads.

## Observability & debugging

### Correlation IDs

All server and bridge logs use `tracing` spans with `trace_id`, `session_id`, `client_id`, and `seq` when applicable. Same `trace_id` attached to envelope debug metadata during development.

### In-game debug overlay

Lua dev HUD (required from M1): connection state, RTT, last sent/applied `seq`, sim time, `world.hash`, last error code. Toggle via menu or hotkey.

### Session replay

`tools/replay` reads `data/sessions/<id>/event.log` and replays from `--from-seq N` for post-mortems without launching X4.

### Debug bundles

On desync (`world.hash` mismatch), `event.rejected` storm, or crash, server writes `data/debug_bundle/<timestamp>/`:

- `meta.json` — session, players, versions
- `last_events.jsonl` — last 200 events
- `acl_decisions.json` — recent rejections
- `hashes.json` — hash components when available

### Sim harness (CI)

`server/tests/harness/` runs fake host + client bridges against the real server — handshake, seq order, broadcast, ACL reject, reconnect — **without X4**. (Integration test file: `server/tests/two_client_harness.rs`.)

### Chaos mode (dev only)

Bridge feature flag `dev-chaos` + env vars: `X4MP_CHAOS_LATENCY_MS`, `X4MP_CHAOS_DROP_RATE`, `X4MP_CHAOS_DISCONNECT_AFTER`. Release builds exclude this code.

## Error Handling & Edge Cases

### Host disconnect

| Scenario | Behavior |
|----------|------------|
| Brief drop (<30s) | Pause sim (`time.speed = 0`), wait |
| Drop >30s | Session paused; 5 min rejoin window |
| Permanent | Session ends; last checkpoint preserved |

Host migration: v1.5+ (not MVP).

### Client disconnect

- Ship enters autopilot hold
- After 10 min: NPC-guarded until return
- Rejoin: replay events since last `seq`

### Desync detection

- Host sends periodic `world.hash`
- Mismatch → request `sector_snapshot` for active sector
- Log all mismatches; **write debug bundle** automatically
- Overlay shows hash mismatch warning to affected client

### Bridge offline buffer (M2+)

Unsent events buffered to disk (SQLite or append-only file). On reconnect, replay with client-local dedup id. Interface defined in M0; full persistence in M2.

### Cheat resistance (v1)

- Server validates ownership, structural validity, sim time
- Host trusted for world truth
- Rate limit: max 20 order events/sec per player

## MVP Sync Scope (phased)

1. Player presence (connect, names, map markers)
2. Controlled ship state (position, rotation, velocity, order) — piloted ships only
3. Direct player actions (move, attack, dock, NPC trade)
4. Shared build/trade events
5. Faction membership changes
6. **Active-sector** NPC snapshot sync (host broadcast, read-only on clients) — sectors containing players only

**Out of v1 scope (research):** Full galaxy-wide NPC/economy reconciliation across unattended sectors. See Phase B research milestone — feasibility unproven via MD/Lua and must be spiked before promising.

## Testing Plan

| Layer | Tests |
|-------|-------|
| CI | GitHub Actions: `cargo test --workspace`, `clippy -D warnings` on PR |
| Proto | Golden fixture per event type; round-trip + validation |
| Server | ACL, event ordering, seq assignment, save store, debug bundle write |
| Harness | Fake host/client WS tests (no X4) |
| Bridge | Pipe I/O, WS reconnect, buffer interface, chaos mode (dev) |
| Replay | `tools/replay` against sample log files |
| Mod | MD cue on ship order; remote order applies; debug overlay |
| Integration | 2-machine LAN, 30 min session, ship sync |
| Soak | 4-hour session, checkpoint/restore, reconnect, chaos injection |

## Repository Layout

```
x4-multiplayer-mod/
├── extension/              # X4 mod (MD, Lua, content.xml)
├── bridge/                 # Local pipe ↔ WebSocket agent
├── server/                 # Central replication server
│   └── tests/              # Integration tests incl. two_client_harness.rs
├── proto/                  # Shared event types + schema
│   └── tests/fixtures/     # Golden JSON per event type
├── tools/
│   ├── replay/             # Session event replay CLI
│   └── dev-echo.ps1        # Local smoke script
├── .github/workflows/ci.yml
└── docs/superpowers/
```

## Delivery Milestones

| Milestone | Deliverable |
|-----------|-------------|
| M0 | Workspace, handshake (versions + compat + join code), tracing spans, server echo, harness two-client test, CI, golden fixtures for `handshake` + `session.ping` |
| M0.5 | `tools/replay` CLI; debug bundle writer (full) |
| **M0.75** | **X4 capture/apply feasibility spike (gate before M1)** — prove read ship state at 5–10 Hz, spawn+update a proxy ship in a 2nd instance, and assess suppressing local sim in active sectors. **Go/no-go decision documented.** |
| M1 | Ship position sync (piloted ships), entity ID map, in-game debug overlay |
| M2 | Client orders → host ack; offline buffer persistence; dev-chaos feature; `session.resume` replay |
| M3 | Session save/load + hash verify, faction ACL, time lock + SETA vote |
| M4 | Trade, build, faction create/join/leave |
| M5 | Active-sector NPC snapshot sync; host migration |
| **Phase B (research)** | Galaxy-wide NPC/economy reconciliation — only after spike proves feasibility |
| **Phase C (long-term)** | Server-authoritative sim without X4 |

**M0.75 is a hard gate.** If the spike shows X4 cannot apply external entity updates smoothly, the architecture is revisited before investing in the full M1+ protocol stack.

## Path to Server-Authoritative Sim (Phase C)

The event log and schema are designed so each event type can eventually be handled by server-side simulation logic without wire-format changes. Phase C requires gradually reimplementing X4 universe systems on the server — a multi-year effort, not a refactor.
