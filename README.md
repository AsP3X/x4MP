# X4 Multiplayer Mod (x4mp)

A co-op multiplayer mod for **[X4: Foundations](https://www.egosoft.com/games/x4/info_en.php)** that lets 2+ players share a single living universe across separate machines. A central replication server coordinates sessions, permissions, saves, and an append-only event log. In v1 the **host player's X4 instance remains world-authoritative** and clients receive and apply replicated events; the long-term goal is server-authoritative simulation without X4.

> **Project status: design + early scaffolding.**
> The architecture, wire protocol, and milestone plans are written and reviewed.
> The Rust workspace (`proto`, `server`, `bridge`), the X4 extension, and the
> tooling described below are **not implemented yet** — M0 is the first build
> milestone. See [Roadmap](#roadmap) and [docs/superpowers](docs/superpowers).

---

## Table of contents

- [What this is](#what-this-is)
- [How it works](#how-it-works)
- [Architecture](#architecture)
- [Wire protocol](#wire-protocol)
- [Repository layout](#repository-layout)
- [Roadmap](#roadmap)
- [Getting started (developers)](#getting-started-developers)
- [Testing](#testing)
- [Observability & debugging](#observability--debugging)
- [Contributing](#contributing)
- [Documentation](#documentation)
- [License](#license)

---

## What this is

| Decision | Choice |
|----------|--------|
| Co-op model | Shared universe — one galaxy, synced economy/NPCs |
| Time | Locked shared time; SETA disabled or by unanimous consent |
| Factions | Hybrid — players can share a faction or run their own |
| Session | Session-based — the universe runs while players are connected |
| Player identity | Player-chosen, session-unique `display_name` |
| v1 authority | The host player's X4 instance simulates the world |
| Long-term | Server-authoritative sim without X4 (a major future phase) |
| Tech stack | Rust (server + bridge), X4 extension (Mission Director + Lua) |

### Design principles (non-negotiable in v1)

- **Host-authoritative world sim.** Only the host X4 instance creates NPC/economy truth. Clients apply host events; they do not simulate shared NPC state locally.
- **No input lockstep.** X4 is non-deterministic, so deterministic lockstep across clients is impossible by design.
- **Central server owns session state.** Event log, `seq` assignment, ACL, and save storage live on the server — not peer-to-peer.
- **Locked shared time.** One sim speed per session; SETA requires unanimous consent or is disabled.
- **Event-sourced replication.** Append-only events over ad-hoc RPC. New behavior = new event types in `proto/`.

---

## How it works

X4 is non-deterministic: two instances loading the same save diverge immediately. A client's local sim of shared NPC/economy state is therefore **not trustworthy** and must not be presented as truth. The v1 reconciliation policy:

- **Active sectors** (containing at least one connected player) are host-authoritative and continuously reconciled. The host streams entity snapshots; clients render host entities as **proxies** and suppress their own local results for those entities.
- **Distant / unattended sectors are not reconciled in v1.** Local sim there is cosmetic; the host's save remains the single source of truth on checkpoint/reload.
- **Full galaxy-wide NPC/economy sync is out of v1 scope** — moved to a research milestone and gated behind a feasibility spike (M0.75) that must pass before the M1 protocol work begins.

---

## Architecture

```
Host X4 ──pipe──► Host Bridge ──WS──► Server ──WS──► Client Bridge ──pipe──► Client X4
   ▲                                    │                                        │
   └──────────── host snapshots ◄───────┴────────── apply via MD/Lua ◄───────────┘
```

Host captures state changes → bridge serializes → server validates / logs / broadcasts → clients apply via Mission Director (MD) cues and Lua.

### Components

| Component | Responsibility | Must NOT |
|-----------|----------------|----------|
| `extension/` | Capture/apply in-game state via MD/Lua; pipe I/O | Open WebSockets directly |
| `bridge/` | Named Pipes ↔ WebSocket; reconnect + buffer | Implement authoritative game rules |
| `server/` | Sessions, ACL, event log, saves | Run X4 or simulate the full galaxy (v1) |
| `proto/` | Shared Rust types, schema, golden fixtures | Game-specific MD/Lua logic |
| `tools/` | `replay` CLI, dev-echo smoke, debug-bundle export | — |

### Authority (v1)

| Entity | Authority | Data direction |
|--------|-----------|----------------|
| Galaxy, sectors, NPC factions | Host X4 instance | Host → clients (read-only on clients) |
| Player-owned ships | Owning player (server-validated) | Owner → server → others |
| Shared-faction stations | Faction members with role permissions | Acting member → server → others |
| Time speed | Host proposes; all clients must ack | Host → clients |

Clients cannot emit `npc.*` or `host.*` events.

---

## Wire protocol

- **Client ↔ Server:** WebSocket (TLS in non-dev configs; plain WS for local dev).
- **X4 ↔ Bridge:** Named Pipes carrying **newline-delimited JSON (NDJSON)** — one JSON object per line (matches the SirNuke Named Pipes API).

Every message is a single envelope:

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

- `seq` is **server-assigned** on ingest and monotonic per session.
- `timestamp_sim` is in-game time (never wall clock) — clients stamp events with the host's locked sim time.
- `handshake` / `handshake.ack` ride the same envelope; the first WS frame after connect must be a `handshake` carrying `proto_version`, `game_version`, `mods_fingerprint`, `join_code`, and `display_name`. Mismatches return a typed error frame and never proceed to game events.

See the [design spec](docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md) for the full event catalog, handshake reject codes, entity-ID mapping, and reconnect/replay rules.

---

## Repository layout

> Items marked _(planned)_ are described by the milestone plans but not yet created.

```
x4-multiplayer-mod/
├── extension/              # X4 mod: MD cues, Lua UI, content.xml   (planned)
├── bridge/                 # Local pipe ↔ WebSocket agent (Rust)    (planned)
├── server/                 # Central replication server (Rust)      (planned)
│   └── tests/              #   incl. two_client_harness.rs          (planned)
├── proto/                  # Shared event types + JSON schema (Rust)(planned)
│   ├── schema/             #   JSON Schema per event type           (planned)
│   └── tests/fixtures/     #   golden JSON per event type           (planned)
├── tools/
│   ├── replay/             # Session event-log replay CLI           (planned)
│   └── dev-echo.ps1        # Local smoke script                     (planned)
├── .github/workflows/ci.yml#                                        (planned)
├── .cursor/                # Project rules + agent skills           (present)
└── docs/superpowers/       # Design spec + milestone plans          (present)
```

---

## Roadmap

| Milestone | Deliverable |
|-----------|-------------|
| **M0** | Rust workspace, version+compat handshake, tracing spans, server echo, two-client harness, CI, golden fixtures, append-only event log |
| **M0.5** | `tools/replay` CLI; full debug-bundle export |
| **M0.75** | **X4 capture/apply feasibility spike — hard go/no-go gate before M1** |
| **M1** | Ship position sync (piloted ships), entity-ID map, in-game debug overlay |
| **M2** | Client orders → host ack; offline buffer persistence; dev-chaos; `session.resume` replay |
| **M3** | Save/load + hash verify, faction ACL, time lock + SETA vote, TLS & auth hardening |
| **M4** | Trade, build, faction create/join/leave |
| **M5** | Active-sector NPC snapshot sync; host migration |
| **Phase B** | Galaxy-wide NPC/economy reconciliation (research, gated by the M0.75 spike) |
| **Phase C** | Server-authoritative sim without X4 (long-term) |

**M0.75 is a hard gate.** If the spike shows X4 cannot apply external entity updates smoothly, the architecture is revisited before investing in the full M1+ stack.

---

## Getting started (developers)

### Prerequisites

- **Rust** (stable toolchain) with `clippy` — used by `proto/`, `server/`, `bridge/`, and `tools/`.
- **Windows** for the X4 extension and bridge: the SirNuke Named Pipes API is Windows-only, and **Protected UI Mode must be disabled** in X4.
- **X4: Foundations** with the **SirNuke Mod Support APIs** installed (for in-game testing from M0.75 onward).
- Two X4 installs (two machines or two accounts) for multiplayer smoke testing.

### Build & run (once the workspace lands in M0)

```bash
# Build everything
cargo build --workspace

# Run the replication server (defaults to ws://127.0.0.1:7878/ws)
cargo run -p x4mp-server

# In another shell, run the bridge (connects to X4MP_SERVER_URL)
cargo run -p x4mp-bridge
```

The bridge reads `X4MP_SERVER_URL` (default `ws://127.0.0.1:7878/ws`) and `X4MP_DISPLAY_NAME` (default `Player`). The server reads `X4MP_BIND_ADDR` (default `127.0.0.1:7878`).

Local end-to-end smoke (Windows / PowerShell):

```powershell
powershell -File tools/dev-echo.ps1
```

---

## Testing

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

- **Sim harness** (`server/tests/two_client_harness.rs`): fake host + client bridges over a real WebSocket — handshake, seq ordering, broadcast, ACL rejection, reconnect replay — **no X4 required**.
- **Golden fixtures**: every event `type` in `proto/` has `proto/tests/fixtures/<type>.json` validated by a round-trip test.
- **Replay**: `cargo run -p x4mp-replay -- --session <id> --from-seq N` reproduces a session from its `event.log` without launching X4.
- **Extension**: manual two-instance LAN smoke with the debug overlay visible.

CI runs `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` on every push/PR to `dev` and `master`.

---

## Observability & debugging

- **Correlation IDs**: server/bridge logs run inside `tracing` spans carrying `trace_id`, `session_id`, `player_id`/`client_id`, and `seq`.
- **In-game debug overlay** (from M1): connection state, RTT, last sent/applied `seq`, sim time, `world.hash`, and last WS error code.
- **Debug bundles**: on `world.hash` mismatch, an `event.rejected` storm, or a crash, the server writes `data/debug_bundle/<timestamp>/` with recent events, ACL decisions, and hash breakdowns.
- **Chaos mode** (dev only): bridge `dev-chaos` feature + `X4MP_CHAOS_LATENCY_MS`, `X4MP_CHAOS_DROP_RATE`, `X4MP_CHAOS_DISCONNECT_AFTER`.

Session saves are **shared universe state** — treat them as production data. Never overwrite `latest.x4s` without first writing a checkpoint, and never delete anything under `data/sessions/` without explicit confirmation.

---

## Contributing

- **Branch model:** `feature/<name>` → `dev` → `master` (via pull request).
- **Commit prefixes:** `TASK:`, `FIX:`, `BUGFIX:`, `DOCS:`, `CHORE:`.
- **Inline docs:** new/modified Rust, Lua, and MD/XML replication hooks must carry human-readable comments (with `// Agent:` / `-- Agent:` / `<!-- Agent: -->` lines where behavior is non-trivial).
- **Adding a replication event:** define the type + golden fixture in `proto/`, add server validation/ACL, map entity IDs, capture on the host, apply on the client, and extend the harness — in that order.

Project rules live in [`.cursor/rules/`](.cursor/rules) and are treated as binding.

---

## Documentation

- **Design spec:** [`docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md`](docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md)
- **M0 — Workspace scaffold & pipe echo:** [`docs/superpowers/plans/2025-06-21-m0-scaffold-and-pipe-echo.md`](docs/superpowers/plans/2025-06-21-m0-scaffold-and-pipe-echo.md)
- **M0.5 — Replay & debug bundles:** [`docs/superpowers/plans/2025-06-21-m0.5-replay-and-debug-bundles.md`](docs/superpowers/plans/2025-06-21-m0.5-replay-and-debug-bundles.md)
- **M0.75 — X4 feasibility spike:** [`docs/superpowers/plans/2025-06-21-m0.75-x4-feasibility-spike.md`](docs/superpowers/plans/2025-06-21-m0.75-x4-feasibility-spike.md)

---

## License

MIT (see workspace package metadata). X4: Foundations is a trademark of Egosoft GmbH; this is an unofficial, fan-made mod and is not affiliated with or endorsed by Egosoft.
