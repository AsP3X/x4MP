# M0: Workspace Scaffold & Pipe Echo — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bootstrap the Rust workspace (`proto`, `server`, `bridge`), prove WebSocket session echo with **version handshake**, **tracing correlation**, **golden fixtures**, **sim harness**, and **CI** — so X4 mod integration (M1) builds on a debuggable foundation.

**Architecture:** Cargo workspace with shared `x4mp-proto` crate. Server accepts WebSocket connections, validates handshake, assigns session/client ids, echoes validated envelopes with server `seq` and tracing spans. Bridge connects to server and exposes a local pipe stub. Harness tests simulate two clients without X4.

**Tech Stack:** Rust 2021, tokio, axum + axum WS (server), tokio-tungstenite (bridge), serde_json, tracing, uuid

**Design spec:** `docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md`

**Cursor rules:** `observability-debugging.mdc`, `ci-and-harness.mdc`, `event-protocol.mdc`, `inline-documentation.mdc`

---

### Task 1: Cargo workspace root

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["proto", "server", "bridge"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["X4MP Contributors"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Create `.gitignore`**

```
/target/
/data/
.env
*.x4s
.DS_Store
debug_bundle/
```

- [ ] **Step 3: Defer workspace check**

Do **not** run `cargo check --workspace` yet — member crates (`proto`, `server`, `bridge`) do not exist, so it will fail. The first real workspace check happens at the end of Task 4. (Alternatively scaffold empty `src/lib.rs` per member first.)

---

### Task 2: Protocol crate (`x4mp-proto`)

**Files:**
- Create: `proto/Cargo.toml`
- Create: `proto/src/lib.rs`
- Create: `proto/src/envelope.rs`
- Create: `proto/src/error.rs`
- Create: `proto/src/events/mod.rs`

- Create: `proto/src/handshake.rs`
- Create: `proto/tests/fixtures/handshake.json`
- Create: `proto/tests/fixtures/handshake.ack.json`
- Create: `proto/tests/fixtures/session.ping.json`
- Create: `proto/tests/fixtures_test.rs`
- Create: `proto/schema/handshake.schema.json`
- Create: `proto/schema/handshake.ack.schema.json`
- Create: `proto/schema/session.ping.schema.json`

- [ ] **Step 1: Add version constants and handshake types**

`proto/src/handshake.rs`:

```rust
// Human: Version + compatibility payload sent as the first WS frame.
// Agent: READS nothing; serialized into EventEnvelope.payload for type "handshake".
use serde::{Deserialize, Serialize};

pub const PROTO_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandshakePayload {
    pub mod_version: String,
    pub proto_version: u32,
    pub bridge_version: String,
    pub game_version: String,
    pub mods_fingerprint: String,
    pub join_code: String,
    pub display_name: String,
}

// Human: Server reply payload for an accepted handshake.
// Agent: serialized into EventEnvelope.payload for type "handshake.ack".
//        Carries the server-assigned ids; display_name is the confirmed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandshakeAckPayload {
    pub client_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub display_name: String,
    pub proto_version: u32,
}
```

Add a name-validation helper (format only in M0; uniqueness enforced server-side in M3):

```rust
// Human: Validate a player-chosen display name. Trims, collapses whitespace.
// Agent: RETURNS Ok(normalized) or Err(reason) -> server maps to INVALID_NAME.
pub fn normalize_display_name(raw: &str) -> Result<String, &'static str> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    match name.chars().count() {
        1..=24 => Ok(name),
        0 => Err("empty"),
        _ => Err("too_long"),
    }
}
```

- [ ] **Step 2: Write failing envelope round-trip test**

Create `proto/Cargo.toml`:

```toml
[package]
name = "x4mp-proto"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true

[dev-dependencies]
# Integration tests under tests/ are separate crates and only see the lib +
# dev-dependencies, so re-declare what the fixture tests use directly.
serde_json.workspace = true
uuid.workspace = true
```

Create `proto/src/envelope.rs` with test first:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Human: Who is allowed to originate a given event type.
// Agent: serialized lowercase ("host"|"server"|"client"); checked by server ACL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Authority {
    Host,
    Server,
    Client,
}

// Human: The single message shape carried on every WS frame (incl. handshake).
// Agent: seq is server-assigned on ingest; payload is the per-type body.
//        APPLIES to all proto/server/bridge messages (event-protocol.mdc).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub v: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub session_id: Uuid,
    pub seq: u64,
    pub timestamp_sim: f64,
    pub sender_id: Uuid,
    pub authority: Authority,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trip() {
        let env = EventEnvelope {
            v: 1,
            event_type: "session.ping".into(),
            session_id: Uuid::nil(),
            seq: 1,
            timestamp_sim: 0.0,
            sender_id: Uuid::nil(),
            authority: Authority::Client,
            payload: serde_json::json!({ "hello": "world" }),
        };
        let json = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);
    }
}
```

- [ ] **Step 3: Add golden fixture tests**

Create `proto/tests/fixtures/handshake.json`:

```json
{
  "v": 1,
  "type": "handshake",
  "session_id": "00000000-0000-0000-0000-000000000000",
  "seq": 0,
  "timestamp_sim": 0.0,
  "sender_id": "00000000-0000-0000-0000-000000000002",
  "authority": "client",
  "payload": {
    "mod_version": "0.1.0",
    "proto_version": 1,
    "bridge_version": "0.1.0",
    "game_version": "7.5",
    "mods_fingerprint": "sha256:0000",
    "join_code": "ABCD-1234",
    "display_name": "Alice"
  }
}
```

Create `proto/tests/fixtures/handshake.ack.json` — a `handshake.ack` envelope (authority `server`) whose `payload` deserializes into `HandshakeAckPayload` (`client_id`, `session_id`, `display_name`, `proto_version`).

Create `proto/tests/fixtures/session.ping.json` (same shape as echo test envelope).

`proto/tests/fixtures_test.rs` — one round-trip test **per fixture** so the
`event-protocol.mdc` "fixture + round-trip per type" rule is met for all three
M0 types (`handshake`, `handshake.ack`, `session.ping`). Also assert the
payloads parse into their typed structs:

```rust
use std::fs;

fn round_trip(path: &str, expected_type: &str) -> x4mp_proto::EventEnvelope {
    let raw = fs::read_to_string(path).unwrap();
    let env: x4mp_proto::EventEnvelope = serde_json::from_str(&raw).unwrap();
    assert_eq!(env.event_type, expected_type);
    let again = serde_json::to_string(&env).unwrap();
    let back: x4mp_proto::EventEnvelope = serde_json::from_str(&again).unwrap();
    assert_eq!(env, back);
    env
}

#[test]
fn fixture_handshake_round_trip() {
    let env = round_trip("tests/fixtures/handshake.json", "handshake");
    let _p: x4mp_proto::HandshakePayload =
        serde_json::from_value(env.payload).unwrap();
}

#[test]
fn fixture_handshake_ack_round_trip() {
    let env = round_trip("tests/fixtures/handshake.ack.json", "handshake.ack");
    let _p: x4mp_proto::HandshakeAckPayload =
        serde_json::from_value(env.payload).unwrap();
}

#[test]
fn fixture_session_ping_round_trip() {
    round_trip("tests/fixtures/session.ping.json", "session.ping");
}
```

- [ ] **Step 3b: Add JSON Schemas (`proto/schema/`)**

`event-protocol.mdc` requires a JSON Schema per event type alongside the golden
fixture. Add `proto/schema/handshake.schema.json`,
`proto/schema/handshake.ack.schema.json`, and
`proto/schema/session.ping.schema.json` describing the envelope + payload shape
(draft 2020-12). Keep them minimal but accurate (required envelope fields +
payload `required`). A follow-up milestone may add a schema-validation test; for
M0 the fixtures are validated by the round-trip tests above and the schemas are
committed as the canonical contract.

- [ ] **Step 4: Run tests**

Run: `cargo test -p x4mp-proto`  
Expected: PASS

- [ ] **Step 5: Add error codes**

`proto/src/error.rs`:

```rust
use serde::{Deserialize, Serialize};

// Human: Canonical server→client error frame (websocket-error-shape.mdc).
// Agent: `code` is SCREAMING_SNAKE + stable; `message` is client-safe (no
//        internal paths/secrets). Shared by server and bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsErrorFrame {
    pub v: u32,
    #[serde(rename = "type")]
    pub frame_type: String,
    pub code: String,
    pub message: String,
}

impl WsErrorFrame {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            v: 1,
            frame_type: "error".into(),
            code: code.into(),
            message: message.into(),
        }
    }
}
```

Export from `proto/src/lib.rs` (single block — `events/mod.rs` may be empty for M0):

```rust
pub mod envelope;
pub mod error;
pub mod events;
pub mod handshake;

pub use envelope::{Authority, EventEnvelope};
pub use error::WsErrorFrame;
pub use handshake::{HandshakeAckPayload, HandshakePayload, PROTO_VERSION};
```

---

### Task 3: Replication server (handshake + echo + tracing)

**Files:**
- Create: `server/Cargo.toml`
- Create: `server/src/lib.rs` (library target — testable API surface)
- Create: `server/src/main.rs` (thin binary that calls into the lib)
- Create: `server/src/session.rs`
- Create: `server/src/ws.rs`
- Create: `server/src/tracing_util.rs`
- Create: `server/src/event_log.rs`
- Test: `server/tests/ws_echo.rs`
- Test: `server/tests/ws_handshake.rs`

> **Crate layout (important):** Integration tests under `server/tests/` are
> separate crates that can only import the crate as a **library**
> (`x4mp_server::...`). A binary-only crate (`[[bin]]` + `src/main.rs`) does
> **not** expose its items to `tests/`. Therefore the server is a **lib + bin**:
> `src/lib.rs` holds `run_on_listener`, `ws`, `session`, `tracing_util`,
> `event_log`; `src/main.rs` is a thin wrapper that parses env/args and calls
> `x4mp_server::run_on_listener(...)`. Cargo auto-detects `src/lib.rs` even with
> an explicit `[[bin]]`, so the two coexist.

- [ ] **Step 1: Write failing handshake test**

`server/tests/ws_handshake.rs` — connect, send fixture `handshake.json`, expect `handshake.ack` with assigned `client_id`. Send wrong `proto_version`, expect `INCOMPATIBLE_VERSION` error frame.

- [ ] **Step 2: Write failing echo integration test**

`server/tests/ws_echo.rs`:

```rust
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const HANDSHAKE: &str = r#"{"v":1,"type":"handshake","session_id":"00000000-0000-0000-0000-000000000000","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{"mod_version":"0.1.0","proto_version":1,"bridge_version":"0.1.0","game_version":"7.5","mods_fingerprint":"sha256:0000","join_code":"ABCD-1234","display_name":"Alice"}}"#;

#[tokio::test]
async fn ws_echoes_envelope_with_seq() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        x4mp_server::run_on_listener(listener).await.unwrap();
    });

    let (mut ws, _) = connect_async(format!("ws://{addr}/ws"))
        .await
        .expect("connect");

    // Handshake first — server requires it before any game event.
    ws.send(Message::Text(HANDSHAKE.into())).await.unwrap();
    let ack = ws.next().await.unwrap().unwrap().into_text().unwrap();
    let ack: serde_json::Value = serde_json::from_str(&ack).unwrap();
    assert_eq!(ack["type"], "handshake.ack");

    // Now a normal event is echoed with a server-assigned seq.
    ws.send(Message::Text(
        r#"{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}"#.into(),
    ))
    .await
    .unwrap();

    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["type"], "session.ping");
    assert!(parsed["seq"].as_u64().unwrap() >= 1);
}
```

Put `run_on_listener` in `server/src/lib.rs` (not `main.rs`) so the integration tests can call `x4mp_server::run_on_listener`. `main.rs` is a thin binary that binds the default address `127.0.0.1:7878` (override via `X4MP_BIND_ADDR`), initializes tracing, and awaits `run_on_listener`. The default port `7878` must match the bridge's default `X4MP_SERVER_URL` (`ws://127.0.0.1:7878/ws`). For M0 the server accepts a fixed dev `join_code` (e.g. `ABCD-1234`) and a wildcard `mods_fingerprint`; real validation lands in M3.

- [ ] **Step 3: Run tests — expect FAIL**

Run: `cargo test -p x4mp-server`  
Expected: FAIL (crate not implemented)

- [ ] **Step 4: Implement server with tracing spans**

Create `server/Cargo.toml`:

```toml
[package]
name = "x4mp-server"
version.workspace = true
edition.workspace = true

# Lib target exposes run_on_listener (+ modules) to integration tests in tests/.
[lib]
name = "x4mp_server"
path = "src/lib.rs"

[[bin]]
name = "x4mp-server"
path = "src/main.rs"

[dependencies]
x4mp-proto = { path = "../proto" }
axum = { version = "0.8", features = ["ws"] }
futures-util = "0.3"
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
uuid.workspace = true

[dev-dependencies]
tokio-tungstenite = "0.26"
```

`server/src/tracing_util.rs` — helper to create span with `trace_id`, `session_id`, `client_id`, `seq`.

WS flow:

1. On connect, generate `trace_id`
2. Require first message `handshake`; validate in order:
   - `proto_version == PROTO_VERSION` else `INCOMPATIBLE_VERSION`
   - `join_code` matches session (M0: fixed dev code) else `INVALID_JOIN_CODE`
   - `game_version` / `mods_fingerprint` (M0: accepted leniently; strict check in M3) else `INCOMPATIBLE_GAME_VERSION` / `INCOMPATIBLE_MODS`
   - `display_name` via `normalize_display_name` else `INVALID_NAME` (M0: format only; per-session uniqueness → `NAME_TAKEN` lands with the session registry in M3)
3. Respond `handshake.ack` (with assigned `client_id`, assigned `session_id`, and confirmed `display_name`) or the matching `WsErrorFrame` then close
4. Subsequent messages: parse `EventEnvelope`, assign `seq`, **append the seq-stamped envelope to the session event log** (see `event_log.rs` below), then echo with span fields logged

Invalid JSON → `WsErrorFrame`. A game event received before handshake → `WsErrorFrame::new("HANDSHAKE_REQUIRED", ...)`. Use `tracing_subscriber` with env filter in `main`.

- [ ] **Step 4b: Implement the append-only event log (`event_log.rs`)**

The event log is the architectural source of truth (`x4mp-architecture.mdc`) and is the input that **M0.5 replay and debug bundles depend on** — so it must exist in M0, not later.

```rust
// Human: Append-only, newline-delimited JSON event log, one file per session.
// Agent: WRITES data/sessions/<session_id>/event.log; one EventEnvelope per line
//        AFTER the server assigns seq. RETURNS io::Result. Source of truth for
//        replay (tools/replay) and debug bundles (last_events.jsonl).
use std::io::Write;
use std::path::PathBuf;
use x4mp_proto::EventEnvelope;

pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    pub fn for_session(session_id: uuid::Uuid) -> std::io::Result<Self> {
        let dir = PathBuf::from("data/sessions").join(session_id.to_string());
        std::fs::create_dir_all(&dir)?;
        Ok(Self { path: dir.join("event.log") })
    }

    // Append one seq-stamped envelope as a single NDJSON line.
    // Note: serde_json::Error does not auto-convert into io::Error, so map it.
    pub fn append(&self, env: &EventEnvelope) -> std::io::Result<()> {
        let line = serde_json::to_string(env)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{line}")
    }
}
```

For M0 the server has one logical session; open/create its `event.log` on first handshake and append every seq-stamped envelope. This produces the `data/sessions/<id>/event.log` format M0.5 reads, and the line format the debug bundle samples for `last_events.jsonl`.

- [ ] **Step 4c: Test the event log is written**

Add an assertion (in `ws_echo.rs` or a small `event_log.rs` unit test) that after an echoed `session.ping`, the session's `event.log` exists and its last line parses back into an `EventEnvelope` whose `seq` matches the echoed `seq`. Use a temp/working dir so the test does not collide with real session data.

- [ ] **Step 5: Run tests — expect PASS**

Run: `cargo test -p x4mp-server`  
Expected: PASS

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p x4mp-server -- -D warnings`  
Expected: no warnings

---

### Task 4: Bridge (WebSocket client + pipe stub + trace propagation)

**Files:**
- Create: `bridge/Cargo.toml`
- Create: `bridge/src/lib.rs` (library target — exposes `ws_client`, `pipe`, `offline_buffer` to tests)
- Create: `bridge/src/main.rs` (thin binary that calls into the lib)
- Create: `bridge/src/pipe.rs`
- Create: `bridge/src/ws_client.rs`
- Create: `bridge/src/offline_buffer.rs` (trait + in-memory stub for M0)
- Test: `bridge/tests/ws_roundtrip.rs`

> **Crate layout (important):** Like the server, the bridge is a **lib + bin**
> so `bridge/tests/ws_roundtrip.rs` can exercise `ws_client`/`pipe` directly.
> The roundtrip test also starts a real server in-process, so it needs
> `x4mp-server` as a **dev-dependency** (see Cargo.toml below).

- [ ] **Step 1: Create `bridge/Cargo.toml`**

```toml
[package]
name = "x4mp-bridge"
version.workspace = true
edition.workspace = true

# Lib target exposes ws_client/pipe/offline_buffer to integration tests.
[lib]
name = "x4mp_bridge"
path = "src/lib.rs"

[[bin]]
name = "x4mp-bridge"
path = "src/main.rs"

[dependencies]
x4mp-proto = { path = "../proto" }
futures-util = "0.3"
tokio-tungstenite = "0.26"
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
uuid.workspace = true

[dev-dependencies]
# Roundtrip test starts the real server in-process, so it must link the
# server lib (which is why the server is a lib + bin — see Task 3).
x4mp-server = { path = "../server" }
```

- [ ] **Step 2: Write failing bridge integration test**

Test spawns server, bridge performs handshake then sends `session.ping`, asserts echoed `seq >= 1`.

- [ ] **Step 3: Implement `ws_client` module**

Connect to `X4MP_SERVER_URL` (default `ws://127.0.0.1:7878/ws`). Send handshake first with crate version constants; read `game_version`/`mods_fingerprint`/`join_code`/`display_name` from env vars (`X4MP_DISPLAY_NAME`, default `Player`) in M0. Propagate `trace_id` in spans matching server.

**Ordering requirement (must hold):** the bridge MUST send the `handshake` frame and **await `handshake.ack`** before it forwards any stdin/pipe game event. The server rejects pre-handshake game events with `HANDSHAKE_REQUIRED`, so the pipe pump must not start until the ack arrives. If an `error` frame (e.g. `INVALID_NAME`, `NAME_TAKEN`) is received instead of the ack, surface `code`+`message` to logs and do not start the pump.

- [ ] **Step 4: Implement `offline_buffer` trait (stub)**

```rust
// Human: Local store for events not yet acked by the server (reconnect resilience).
// Agent: M0 = in-memory VecDeque; M2 = disk-backed (SQLite) per design spec § Bridge offline buffer.
use x4mp_proto::EventEnvelope;

pub trait OfflineBuffer: Send + Sync {
    fn push(&mut self, local_id: u64, envelope: EventEnvelope);
    fn drain(&mut self) -> Vec<EventEnvelope>;
}
```

In-memory `VecDeque` impl for M0; SQLite impl deferred to M2 per design spec.

- [ ] **Step 5: Implement `pipe` stub**

For M0: **after** `handshake.ack` (see Step 3), read **newline-delimited JSON** (NDJSON) from stdin (dev stand-in for the Named Pipe), forward each line to WS, print reply to stdout. On stdin EOF, drain any in-flight reply, then shut down the WS connection and **exit cleanly** (so `dev-echo.ps1`'s `finally` block can stop the server without a hung process). Document that the Named Pipe replaces stdin/stdout in M1, keeping the same NDJSON framing chosen in the design spec.

- [ ] **Step 6: Run tests**

Run: `cargo test -p x4mp-bridge`  
Expected: PASS

---

### Task 5: Dev runner script

**Files:**
- Create: `tools/dev-echo.ps1`

- [ ] **Step 1: PowerShell script starts server + bridge stdin test**

PowerShell does not background with `&`; use `Start-Process`. Provide complete JSON, not `...`.

```powershell
# tools/dev-echo.ps1
# Human: Local smoke — start server, feed bridge one handshake + ping via stdin.
$ErrorActionPreference = "Stop"

# Start the server in a separate process; capture it so we can stop it later.
$server = Start-Process -FilePath "cargo" -ArgumentList "run","-p","x4mp-server" -PassThru
Start-Sleep -Seconds 2

# The bridge performs the handshake itself on connect; stdin carries game events only.
$ping = '{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}'

try {
    @($ping) | cargo run -p x4mp-bridge
}
finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
```

- [ ] **Step 2: Manual smoke**

Run: `powershell -File tools/dev-echo.ps1`  
Expected: bridge logs `handshake.ack`, then prints echoed `session.ping` with `seq >= 1`. Server process is stopped on exit.

---

### Task 6: Sim harness (two fake clients, no X4)

**Files:**
- Create: `server/tests/two_client_harness.rs`

- [ ] **Step 1: Write failing harness test**

`server/tests/two_client_harness.rs`:

- Start server on ephemeral port
- Connect Client A and Client B; both complete handshake (await `handshake.ack`)
- **Sequence the sends to avoid races:** A sends `session.ping` and awaits its echo (`seq_a`); *then* B sends `session.ping` and awaits its echo (`seq_b`)
- Assert: `seq_a >= 1`, `seq_b > seq_a` (distinct and strictly increasing across clients)

Do **not** assert exact literal values (`1`, `2`) — assert the ordering invariant only, so the test is not flaky under concurrent connection setup.

- [ ] **Step 2: Implement any missing server session seq sharing**

Single shared atomic seq counter per listener for M0 (refine to per-session in M3).

- [ ] **Step 3: Run harness**

Run: `cargo test -p x4mp-server --test two_client_harness`  
Expected: PASS

---

### Task 7: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Add GitHub Actions workflow**

```yaml
name: CI
on:
  push:
    branches: [master, dev]
  pull_request:
    branches: [master, dev]
jobs:
  rust:
    # ubuntu is faster/cheaper and catches non-Windows portability for the
    # cross-platform crates. Windows-only pipe tests get their own job once
    # the Named Pipe code lands (M1).
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings
```

- [ ] **Step 2: Verify locally**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`  
Expected: PASS (after Tasks 2–6 complete)

---

### Task 8: Debug bundle stub + replay placeholder

**Files:**
- Create: `server/src/debug_bundle.rs`
- Create: `tools/replay/README.md`

- [ ] **Step 1: Implement debug bundle writer stub**

On test hook or `event.rejected`, write `data/debug_bundle/<timestamp>/meta.json` + empty `last_events.jsonl`. Full implementation in M0.5.

- [ ] **Step 2: Document replay CLI scope**

`tools/replay/README.md` — M0.5 deliverable; document expected CLI: `cargo run -p x4mp-replay -- --session <id> --from-seq N`

---

### Task 9: Extension placeholder

**Files:**
- Create: `extension/content.xml`
- Create: `extension/ui/ui.xml`
- Create: `extension/ui/x4mp_init.lua`
- Create: `extension/ui/x4mp_debug_overlay.lua` (empty frame stub; implemented in M1)

- [ ] **Step 1: Minimal extension skeleton**

`content.xml` with mod metadata; `ui.xml` loading init Lua; init Lua logs `"X4MP loaded"` only. Stub `ui/x4mp_debug_overlay.lua` (empty frame, implemented in M1). No pipe yet — documented as M1.

---

## Self-review checklist

- [x] Spec M0 (bridge pipe loop + server echo) covered
- [x] Handshake with compat fields (proto/game/mods/join_code) + reject codes
- [x] Player `display_name` in handshake (format-validated; uniqueness in M3)
- [x] Echo test handshakes first (matches server requirement)
- [x] proto dev-deps declared for integration tests (serde_json, uuid)
- [x] handshake.rs imports serde
- [x] `HandshakeAckPayload` typed struct + `handshake.ack` fixture + round-trip test
- [x] server + bridge are **lib + bin** so `tests/` can import `x4mp_server`/`x4mp_bridge`
- [x] bridge dev-dep on `x4mp-server` (roundtrip test spawns server in-process)
- [x] Append-only `event.log` writer in M0 (source of truth for M0.5 replay/bundles)
- [x] JSON Schemas in `proto/schema/` for all three M0 types (event-protocol.mdc)
- [x] Bridge awaits `handshake.ack` before stdin pump; exits cleanly on EOF
- [x] Server default bind `127.0.0.1:7878` matches bridge default URL
- [x] Single `lib.rs` export block (no duplicate)
- [x] server + bridge Cargo.toml content provided
- [x] Tracing correlation IDs (`observability-debugging.mdc`)
- [x] Golden fixtures for handshake + session.ping
- [x] Sim harness two-client test, non-flaky ordering assertions (`ci-and-harness.mdc`)
- [x] CI workflow (ubuntu + clippy component)
- [x] Offline buffer trait stub with import (M2 persistence)
- [x] Debug bundle stub (M0.5 full)
- [x] dev-echo.ps1 is valid PowerShell (Start-Process, complete JSON)
- [x] Event envelope matches design doc
- [x] WsErrorFrame matches `websocket-error-shape.mdc`
- [x] No placeholders in task steps
- [x] All new Rust code subject to `inline-documentation.mdc` and `no-allow-dead-code.mdc`

## Done criteria

- `cargo test --workspace` green (including harness + fixtures + event-log test)
- `cargo clippy --workspace -- -D warnings` green
- Manual dev-echo smoke passes (handshake.ack + ping echo with seq ≥ 1)
- `data/sessions/<id>/event.log` is written with seq-stamped envelopes (verified by test)
- Extension skeleton present (load-only + debug overlay stub)
- `.github/workflows/ci.yml` present and matches local commands

## Next gate

**M0.75 feasibility spike is required before M1.** See `docs/superpowers/plans/2025-06-21-m0.75-x4-feasibility-spike.md`. Do not begin M1 ship-sync protocol work until the spike's go/no-go is documented.
