# Spike Q2 — Apply feasibility

**Status:** PASS (revised) — validated in X4 via single-instance loopback
**Date:** 2026-06-21
**Tester:** project owner (manual in-game)
**Spike mod version:** `0.0.1-spike`

## Question

Can we spawn a proxy ship and continuously update its position/rotation from external data smoothly?

**Pass criteria:** Proxy follows source path with visually acceptable interpolation at 5–10 Hz; no teleport-snapping unless snap mode is intentional.

## What was actually tested (deviation from template)

The original plan assumed *instance 2 + bridge feeder* replaying a captured `capture.ndjson`. To isolate the **apply mechanics** from the network/bridge path, Q2 was validated as a **single-instance MD-only loopback**:

- One X4 instance spawns a player-owned **proxy** ship.
- The proxy replays the **player ship's own path on a ~2 s delay** (a local stand-in for "host snapshots arriving over the wire").
- No server/bridge involved — this answers *"can the engine apply a smooth, correctly-oriented external transform stream?"*, which is the risky unknown. The wire path (server → bridge → pipe → apply) reuses the same MD apply cue and is downstream M1 work.

Code: `extension/spike/md/x4mp_spike_apply.xml` (cues `Spike_ApplyLoop` recorder + `Spike_ApplyWarp` playback).

## Key finding: there is no native transform setter

- **`set_object_position` (MD) and `SetObjectSectorPos` (Lua FFI) do not exist.** Earlier attempts silently no-op'd (proxy stayed frozen). The engine is closed-source, so we cannot add one.
- The **only** working general reposition primitive is the MD action:

  ```xml
  <warp object="$proxy" zone="$zone">
    <position x="$x" y="$y" z="$z"/>   <!-- or value="$position" -->
    <rotation value="$rotation"/>       <!-- sets orientation too -->
  </warp>
  ```

- `<warp>` sets **both position and rotation**, but it **teleports** — there is no engine-side interpolation/tween. Motion therefore steps at the warp rate.
- A piloted proxy with a `<create_order>` (Follow/MoveTo) was rejected: the flight engine fights per-tick warp. Proxy must be **plain** (no pilot, no order) and driven entirely by `<warp>`.
- `create_ship` requires a **valid macro** — arbitrary macro ids fail. Using the source ship's own `$src.macro` always works.

## Approach that passed: record slow, play back fast + interpolate

Because `<warp>` can't tween, smoothness is achieved by **decoupling record rate from warp rate** and interpolating in MD:

| Stage | Cue | Rate | Behavior |
|-------|-----|------|----------|
| Record | `Spike_ApplyLoop` | 10 Hz (`checkinterval="0.1s"`) | Append `{position, zone, rotation}` to a ring buffer |
| Playback | `Spike_ApplyWarp` | ~40 Hz (`checkinterval="0.025s"`) | Walk a fractional cursor ~2 s behind; `<warp>` to a **linearly interpolated** position between the two bracketing samples; **snap** rotation to the forward sample |

- **Position**: component-wise lerp `A + (B − A) * frac` → ~4 interpolated warps per recorded segment → visible steps shrink ~4×.
- **Rotation**: snapped (not slerped) — already looked correct at 10 Hz; angle-wrap slerp was unnecessary complexity for the spike.
- **Cursor self-corrects**: free-runs at `$Step`, clamps to a window behind newest data, so it never extrapolates past real samples and never lags unbounded (robust even if MD doesn't honor 0.025 s exactly).
- **Zone change guard**: if two samples straddle a zone boundary, snap instead of interpolating across frames.

## Results

| Metric | Result |
|--------|--------|
| Visual quality (1–5) | **5** — tester: "works perfectly as expected" |
| Orientation match | Correct (rotation child of `<warp>`) |
| Effective record rate | 10 Hz |
| Effective warp/playback rate | ~40 Hz |
| Replay delay | ~2 s (20 samples) — tunable via `$Delay` |
| Side effects (physics/collision) | None observed (plain proxy, no AI/flight engine) |

Server-side sanity metrics emitted ~1 Hz as `spike.apply_pos` `{seq, proxy_x, proxy_z, base, count}`.

## APIs used (corrected vs template)

| Action | API / cue | Notes |
|--------|-----------|-------|
| Spawn proxy | MD `<create_ship>` | Use source ship's own `$src.macro`; plain (no pilot/order) |
| Set position **and** rotation | MD `<warp object= zone=><position/><rotation/></warp>` | The ONLY working reposition primitive; teleports (no tween) |
| ~~Set position~~ | ~~`set_object_position` / `SetObjectSectorPos`~~ | **Do not exist** — confirmed dead ends |
| Smoothness | MD-side lerp between buffered samples + higher warp rate | Engine provides no interpolation |

## Observations

- Smoothness is entirely **our** responsibility — the engine will not interpolate a teleported object. Any client proxy in M1 must run a client-side interpolation buffer fed by host snapshots (matches `authoring-x4-md-lua-hooks` guidance: "interpolated between host snapshots").
- Tooling caveat surfaced during the spike: SirNuke options registered with `$category` auto-create a submenu keyed by script name that is **not** re-registration-safe → `Submenu id conflicts with prior registered id` on reload drops all rows. Register options **without** `$category` (see `x4mp_spike_menu.xml`).
- Install drift caveat: the X4 extension folder must be a **junction** to the repo (or use `tools/dev-stack/sync-mod.ps1`) or edits silently don't reach the game.

## Verdict

- [ ] **PASS**
- [x] **PASS (revised)** — interpolation is **mandatory** (engine has no transform tween); apply must be host-driven via `<warp>` with position+rotation; record 10 Hz / playback ~40 Hz is smooth. Proxies must be plain (no AI pilot/order).
- [ ] **FAIL**

## Not yet validated (M1 carry-over)

- Driving the proxy from **real wire snapshots** (server → bridge → pipe → MD apply) instead of the local delayed loopback.
- **Multiple** proxies / entity-id mapping under load.
- Interpolation across **sector/zone transitions** (currently snaps).
