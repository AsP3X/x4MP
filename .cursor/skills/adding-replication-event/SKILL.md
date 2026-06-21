---
name: adding-replication-event
description: Step-by-step workflow to add a new replication event type to the X4 multiplayer mod, touching proto, server, host capture, client apply, and tests. Use when adding or changing a wire event type, syncing a new game action (ship order, trade, build, faction change), or whenever the user mentions a new event for the proto/ crate or replication.
---

# Adding a Replication Event

Adds one new event `type` end-to-end. Follows `x4mp-architecture.mdc`, `event-protocol.mdc`, and `websocket-error-shape.mdc`. Every step is required — a missing fixture, ACL rule, or harness scenario is a common source of silent desync.

## Decide first

- **Name**: `category.action` (e.g. `ship.order`, `trade.executed`). Lowercase, dot-separated.
- **Authority**: who may originate it — `host`, `server`, or `client`. Clients must never originate `npc.*` or `host.*`.
- **Direction**: client→server→others (player asset) or host→clients (world state). See design spec § Authority.

## Checklist

Copy and track:

```
- [ ] 1. Define the event type + payload in proto/
- [ ] 2. Add golden fixture JSON
- [ ] 3. Add round-trip + fixture test (run, see it pass)
- [ ] 4. Add server validation + ACL rule
- [ ] 5. Add server test (or harness scenario if session-visible)
- [ ] 6. Host: capture & emit on authority (MD/Lua)
- [ ] 7. Client: apply via MD cue
- [ ] 8. cargo test --workspace + clippy green
- [ ] 9. Document manual X4 smoke steps
```

## Step 1 — proto type

Add a payload struct under `proto/src/events/` and register the type string. Keep payloads flat and serde-friendly. Use session `entity_id` (UUID), **never raw X4 ids**.

```rust
// proto/src/events/ship.rs
// Human: Move order issued for a player-owned ship.
// Agent: type "ship.order"; authority=client; APPLIES on host then rebroadcast.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipOrder {
    pub entity_id: Uuid,
    pub order: String,
    pub target_entity_id: Option<Uuid>,
    pub position: Option<[f64; 3]>,
}
```

## Step 2 — golden fixture

Create `proto/tests/fixtures/<type>.json` — a full envelope with realistic payload. This is the wire contract.

```json
{
  "v": 1,
  "type": "ship.order",
  "session_id": "00000000-0000-0000-0000-000000000001",
  "seq": 0,
  "timestamp_sim": 1234.5,
  "sender_id": "00000000-0000-0000-0000-000000000002",
  "authority": "client",
  "payload": { "entity_id": "00000000-0000-0000-0000-0000000000aa", "order": "moveto", "target_entity_id": null, "position": [1.0, 2.0, 3.0] }
}
```

## Step 3 — fixture test

Add to `proto/tests/fixtures_test.rs`: load file → deserialize envelope → assert `type` → re-serialize → deserialize → equality. Run `cargo test -p x4mp-proto`; confirm PASS before moving on.

## Step 4 — server validation + ACL

In the server's event handler:

1. Reject if sender lacks authority for this `type`.
2. Check ownership/role per the permission matrix (design spec § Permissions).
3. Validate `timestamp_sim` within `SIM_TIME_TOLERANCE` of locked sim time.
4. On reject, send `event.rejected` / `WsErrorFrame` with a stable `code` — never a bare close.

## Step 5 — server/harness test

If the event is session-visible (broadcast to other clients), extend `server/tests/two_client_harness.rs`: client A emits the event, assert client B receives it with a server-assigned `seq`, and assert an unauthorized sender is rejected.

## Step 6 — host capture (MD/Lua)

Emit on the authoritative side only. Use the `authoring-x4-md-lua-hooks` skill for the mechanics. Comment the cue: `<!-- Agent: EMITS ship.order WHEN player issues move order -->`. Map X4 ids through the entity map before emitting.

## Step 7 — client apply

Add an MD apply cue that consumes the incoming event and mutates local state idempotently. Map incoming `entity_id` to the local proxy object. Read-only events (`npc.*`) must not be re-emitted.

## Step 8 — verify

```
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Both green before claiming done.

## Step 9 — document smoke

In the commit/PR body, list the 2-instance LAN steps to verify the event applies on the remote client (see `regression-testing.mdc`).
