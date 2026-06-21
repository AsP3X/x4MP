# Spike Q3 — Local sim divergence

**Status:** PENDING manual X4 measurement  
**Date:** _fill after test_

## Question

Can we suppress or tolerate the client's local NPC sim so host snapshots are authoritative?

## Procedure

1. Load **identical save** on two instances (no sync)
2. Run **5 minutes** in an NPC-heavy sector (e.g. busy trade hub)
3. Compare: ship counts, station storage, faction rep ticks if visible
4. Test suppression levers (document what was tried):

| Lever | Feasible? | Notes |
|-------|-----------|-------|
| Remove local NPCs, host-driven proxies only | _TBD_ | |
| Pause local economy/job ticks | _TBD_ | |
| Accept NPC divergence; sync player ships only | _TBD_ | |

## Divergence timeline

| Elapsed | Observation |
|---------|-------------|
| 0 min | _TBD_ |
| 1 min | _TBD_ |
| 5 min | _TBD_ |

## v1 authority boundary (draft)

**Can be host-authoritative in v1:**

- _TBD after test_

**Must accept as divergent in v1:**

- _TBD after test_

## Verdict

- [ ] Suppression feasible for active sectors
- [ ] Tolerance boundary documented (player ships only)
- [ ] **Blocker** — requires architecture change: _
