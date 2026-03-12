# Roadmap

## Phase 1: FFB Safety — Eliminate Wrist Injury Risk
**Goal:** Ensure the wheelbase always reaches a safe zero-force state when a session ends, a game crashes, or rc-agent starts after an unclean exit.
**Requirements:** FFB-01, FFB-02, FFB-03, FFB-04
**Plans:** P1.1 — FFB Controller and Session Lifecycle Integration

### Success Criteria
1. When billing ends, the wheelbase stops producing torque BEFORE the game process is killed (observable: wheel goes limp, then game closes)
2. When the game crashes unexpectedly, the wheelbase reaches zero-force within 200ms of crash detection
3. When rc-agent starts on a pod where the previous session ended uncleanly, the wheelbase is disarmed immediately on startup (observable: spinning wheel stops when rc-agent launches)
4. When the wheelbase USB is disconnected or the device is not found, rc-agent logs a warning and continues normally without panic or blocking

---

## Phase 2: HUD Infrastructure — GDI Resource Cache and Component System
**Goal:** Eliminate the GDI font leak and establish the component-based paint architecture that all subsequent HUD work builds on.
**Requirements:** INFRA-01, INFRA-02
**Plans:** P2.1 — GDI Resources Cache and Component Paint System

### Success Criteria
1. GDI font handle count remains constant after overlay initialization (observable: no font handle growth in Task Manager over a 30-minute session)
2. Adding a new HUD section requires implementing one trait and registering it in a dispatcher — no modifications to `paint_hud()` internals
3. Existing overlay functionality (speed, gear, RPM bar) continues to render correctly after the refactor (observable: visual parity with current HUD on Pod 8)

---

## Phase 3: HUD Layout and Display — Essentials-Style Redesign
**Goal:** Redesign the overlay to the AC Essentials centered layout with all visual elements at glanceable size and position.
**Requirements:** HUD-01, HUD-02, HUD-03, HUD-04, HUD-05, HUD-06, HUD-07, HUD-08, HUD-09
**Plans:** 2 plans

Plans:
- [ ] 03-01-PLAN.md — Core Layout: Essentials geometry, Consolas fonts, centered gear, RPM bar, speed
- [ ] 03-02-PLAN.md — Timing Display: lap times, sectors, session timer, lap counter, invalid indicator + Pod 8 verification

### Success Criteria
1. Gear indicator is centered and rendered at 60-80pt equivalent size (observable: readable from 1.5 meters away in driver seating position)
2. RPM bar spans the full overlay width (8-12px), fills left-to-right with green/yellow/amber/red color zones based on dynamic `max_rpm` from shared memory
3. Lap times (current, previous, best), sector times (S1/S2/S3 with purple/green/yellow coloring), session timer, and lap counter are all visible simultaneously without overlapping
4. All numeric values use Consolas monospace font — no layout jitter when digits change (observable: speed, lap time digits do not shift horizontally during driving)
5. Speed display shows KM/H repositioned to the Essentials layout location

---

## Phase 4: HUD Data Accuracy — Fix Timing and State Bugs
**Goal:** Ensure all displayed timing data is correct, complete, and never stale — first lap records, sector bests are independent, lap boundary transitions are smooth, and the HUD clears when the game exits.
**Requirements:** DATA-01, DATA-02, DATA-03, DATA-04
**Plans:** P4.1 — Timing Data Fixes and State Validation

### Success Criteria
1. The first lap of every session is recorded and displayed (observable: lap 1 time appears in "previous lap" after crossing the line, not just dashes)
2. Best sector times are tracked independently from best lap — a purple S1 can appear even if the rest of that lap was slow (observable: sector colors reflect per-sector bests, not best-lap-sector bests)
3. Current lap timer does not flash to 0:00.000 at the lap boundary — it holds the previous value for at least 2 poll cycles before resetting (observable: smooth transition, no visible flicker)
4. After the game exits, the HUD does not display stale telemetry data — all values clear or the overlay hides (observable: no phantom lap times lingering after AC closes)

---

## Execution Order

```
Phase 1 (FFB Safety)  ──>  Phase 2 (HUD Infra)  ──>  Phase 3 (HUD Layout)  ──>  Phase 4 (Data Accuracy)
     [URGENT]                 [Foundation]              [Visible Value]           [Polish]
```

**Rationale:**
- Phase 1 is first because it addresses a real injury risk (8Nm uncontrolled torque). Safety is non-negotiable.
- Phase 2 is second because the GDI font leak must be fixed before adding more HUD complexity, and the component system is a prerequisite for the layout redesign.
- Phase 3 is third because it delivers the visible customer-facing value (the Essentials layout).
- Phase 4 is last because the data accuracy bugs are edge cases that matter for correctness but do not block the layout work — they refine what Phase 3 displays.

---
*Created: 2026-03-11 | 19 requirements across 4 phases | Sequential execution*
