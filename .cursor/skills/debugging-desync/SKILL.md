---
name: debugging-desync
description: Systematic post-mortem workflow for X4 multiplayer desync, event rejection storms, and session crashes using debug bundles, the replay tool, world.hash diffs, and trace_id correlation. Use when investigating a world.hash mismatch, event.rejected, clients showing different state, a session crash, or when the user reports players seeing different things.
---

# Debugging a Desync

Turns a "players see different state" report into a reproducible root cause. Pairs with the built-in systematic-debugging skill (form a hypothesis, find evidence, then fix). Follows `observability-debugging.mdc`.

## First: find the evidence, don't guess

```
- [ ] 1. Locate the debug bundle
- [ ] 2. Read meta.json (versions, players, session)
- [ ] 3. Diff hashes.json (host vs reporter)
- [ ] 4. Scan acl_decisions.json for rejection storms
- [ ] 5. Correlate the failing window by trace_id / seq
- [ ] 6. Reproduce with tools/replay (no X4)
- [ ] 7. Form hypothesis, write failing test, fix
```

## Step 1 — locate the bundle

Bundles auto-write on `world.hash` mismatch, `event.rejected` storm, or crash:

```
data/debug_bundle/<utc-timestamp>/
├── meta.json
├── last_events.jsonl
├── acl_decisions.json
└── hashes.json
```

Never delete bundles without explicit permission (`data-safety.mdc`).

## Step 2 — meta.json

Confirm the basics first — most "desyncs" are mismatched inputs:

- **Version skew** — different `proto_version`, `game_version`, or `mods_fingerprint`? That's the bug; the handshake should have rejected it. Check why it didn't.
- Which players, which session, when.

## Step 3 — hashes.json

Compare host vs reporting client `world.hash`:

- **Diverge from the start** → initial save load mismatch (hash not verified after download). Check `WORLD_READY` / `CLIENT_READY` save hash flow.
- **Diverge at a point in time** → a specific event applied differently. Note the `seq` where hashes split; that's your window.

## Step 4 — acl_decisions.json

A storm of `event.rejected` means a client kept emitting something the server refused:

- Wrong `authority` (client emitting `npc.*`/`host.*`).
- Ownership/role failure (permission matrix).
- `timestamp_sim` outside `SIM_TIME_TOLERANCE` — client stamping its own clock instead of host sim time.

## Step 5 — correlate

Every server/bridge log line carries a `tracing` span with `trace_id`, `session_id`, `client_id`, `seq`. Filter logs to the `seq` window from Step 3 and follow the `trace_id` across server and both bridges to see where the event diverged.

## Step 6 — reproduce without X4

Replay the captured log against a server (or mock consumer) from just before the divergence:

```
cargo run -p x4mp-replay -- --session <id> --from-seq <N> --server ws://127.0.0.1:7878/ws
```

`<N>` = a few events before the hash split. If the divergence reproduces here, X4 is not needed to fix it.

## Step 7 — fix with a test

- Reproduce in the **harness** (`server/tests/two_client_harness.rs`) — add a scenario that drives the failing sequence.
- Make it fail, then fix the validation/apply/ordering bug, then green.
- If the bug is a wire-contract issue, also add/repair the golden fixture for that event type.

## Common root causes

| Symptom | Likely cause |
|---------|--------------|
| Diverge immediately on join | Save hash not verified; clients loaded different saves |
| Diverge after a specific action | Non-idempotent apply cue, or missing server validation |
| Rejection storm | Authority/ownership mismatch, or client clock used for `timestamp_sim` |
| Only one client wrong | That client missed events — check `session.resume` replay from `from_seq` |
| Random per-run divergence | Client simulating shared NPC state instead of using host snapshots (design spec § Client simulation stance) |
