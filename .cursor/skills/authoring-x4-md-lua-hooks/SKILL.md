---
name: authoring-x4-md-lua-hooks
description: Domain knowledge and patterns for writing X4 Foundations extension hooks — Mission Director (MD) cues, Lua UI, SirNuke Named Pipes (NDJSON), entity mapping, and Protected UI Mode. Use when editing files under extension/, writing MD/XML cues or Lua, wiring pipe I/O between X4 and the bridge, or capturing/applying game state for replication.
---

# Authoring X4 MD/Lua Hooks

X4's modding surface is esoteric. This captures the project-specific mechanics so capture/apply hooks work the first time. Follows `x4-extension-mod.mdc` and `inline-documentation.mdc`.

## Environment facts

- **Target Windows.** SirNuke Named Pipes API is Windows-only (v1).
- **Protected UI Mode must be disabled** in the Extensions menu, or Lua/pipe hooks won't load.
- **X4 7.5+ loads custom Lua via `ui/ui.xml`** — the old `Lua_Loader.Load` MD event is legacy. Use `ui.xml`.
- **Pipe framing is NDJSON** — one JSON object per line, matching the bridge.

## File layout

```
extension/
├── content.xml                 # mod metadata + dependency on SirNuke APIs
├── md/                         # Mission Director cues (capture + apply)
│   ├── x4mp_capture.xml
│   ├── x4mp_apply.xml
│   └── x4mp_entity_map.xml
└── ui/
    ├── ui.xml                  # registers Lua files (7.5+)
    ├── x4mp_pipe.lua           # the ONLY pipe I/O module
    └── x4mp_debug_overlay.lua  # dev HUD (M1+)
```

## MD cue patterns

Prefix all cues `md.X4MP.*` to avoid collisions. Init cues must survive savegame loads:

```xml
<!-- Human: Setup runs on new game AND on save load. -->
<!-- Agent: waits for md.Setup.Start so newly-added cues still fire. -->
<cue name="X4MP_Init">
  <conditions>
    <event_cue_signalled cue="md.Setup.Start"/>
  </conditions>
  <actions>
    <!-- init globals, open pipe via Lua -->
  </actions>
</cue>
```

Capture cues fire on game events and hand data to Lua to serialize. Comment every cue that EMITS or APPLIES a wire event:

```xml
<!-- Agent: EMITS ship.order WHEN player issues a move order. -->
```

## Lua pipe I/O

Keep **all** pipe access in `x4mp_pipe.lua`. MD raises a UI event; Lua serializes to NDJSON and writes the pipe. Lua must **not** open network sockets — only the Named Pipe to the bridge.

```lua
-- Human: Serialize an event table to one NDJSON line and write to the bridge pipe.
-- Agent: CALLS SirNuke pipe write; RETURNS nothing; failure mode = pipe closed -> buffer + retry.
local function send_event(evt)
  local line = json.encode(evt) .. "\n"
  Pipe.write(PIPE_NAME, line)  -- SirNuke Named Pipe API
end
```

Reading ship state for capture (rates and exact getters are validated in the M0.75 spike):

```lua
-- Agent: READS player ship live state for ship.state_snapshot at 5-10 Hz.
local ship = GetPlayerPrimaryShipID()
local x, y, z = GetComponentData(ship, "position")
```

## Entity mapping (mandatory)

Never put raw X4 ids on the wire. On first sighting, get a stable session `entity_id` (UUID) from the bridge/server and store the mapping in `md/x4mp_entity_map.xml`. On apply, map incoming `entity_id` → local proxy object id.

## Authority discipline

- Emit an event **only on the side that owns it** (host for `npc.*`/world, client for its own ship).
- Apply cues mutate state **idempotently** where possible and never re-emit a received event.
- Clients render remote ships as **proxies** (model + nameplate), interpolated between host snapshots.

## Debug overlay (M1+)

`x4mp_debug_overlay.lua` shows connection state, RTT, last sent/applied `seq`, sim time, `world.hash`, last error code. Toggle via hotkey/menu. Required before M1 sign-off (`observability-debugging.mdc`).

## Testing

No automated X4 CI. After changes, run the 2-instance LAN smoke with the debug overlay visible and document steps in the commit body (`regression-testing.mdc`). Protocol-level behavior is covered by the Rust harness instead.
