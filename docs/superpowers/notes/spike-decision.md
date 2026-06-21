# M0.75 Spike — Go / No-Go Decision

**Status:** PENDING — Q1/Q2 validated; Q3 **postponed** (no second X4-capable machine)  
**Last updated:** 2026-06-21 (Q3 tooling built; two-instance run postponed)

## Summary

| Question | Result | Link |
|----------|--------|------|
| Q1 Capture @ 5–10 Hz | PASS (qualitative) — 10 Hz MD capture → pipe → server confirmed; formal metrics note still TBD | [spike-q1-capture.md](./spike-q1-capture.md) |
| Q2 Proxy apply + interpolation | **PASS (revised)** — `<warp>` only; MD-side lerp mandatory | [spike-q2-apply.md](./spike-q2-apply.md) |
| Q3 NPC divergence boundary | _POSTPONED_ — capture cue + `q3_divergence` analyzer built & unit-tested; two-instance run blocked (second machine can't run X4) | [spike-q3-divergence.md](./spike-q3-divergence.md) |

## Decision

- [ ] **GO** — proceed to M1 as designed
- [ ] **GO (revised)** — proceed with documented scope changes:
  - _e.g. interpolation mandatory, NPC sync deferred, rate capped at 3 Hz_
- [ ] **NO-GO** — architecture rethink required

### Rationale

_Fill after measurements._

### Failed assumption (if NO-GO)

_Which API or limit blocked host-authoritative proxy replication?_

### Candidate alternatives (if NO-GO)

- _e.g. sector-instance streaming, server-side sim Phase C, visual-only ghosts_

## Fallback ladder applied

Document which rungs were tried before the final verdict (see M0.75 plan):

1. Lower rate to 2–3 Hz + interpolation — _TBD_
2. Switch MD ↔ Lua tick source — _TBD_
3. Player-piloted ships only — _TBD_

## Next steps after decision

- Update `docs/superpowers/specs/2025-06-21-x4-multiplayer-design.md` § Client simulation stance
- If **GO**: begin M1 ship-sync protocol on `feature/m1-ship-sync`
- If **NO-GO**: schedule architecture review before further replication work
