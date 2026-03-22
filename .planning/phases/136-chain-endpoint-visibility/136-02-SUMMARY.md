---
phase: 136-chain-endpoint-visibility
plan: 02
subsystem: infra
tags: [comms-link, relay, websocket, health, degradation, visibility]

# Dependency graph
requires:
  - phase: 136-chain-endpoint-visibility-01
    provides: /relay/chain/run endpoint with chain_result WS handler

provides:
  - GET /relay/health returns connectionMode (REALTIME/EMAIL_FALLBACK/OFFLINE_QUEUE) and lastHeartbeat timestamp
  - POST /relay/exec/run returns HTTP 503 immediately when connectionMode is not REALTIME
  - rp-bono-exec SKILL.md health probe guidance for callers

affects:
  - rp-bono-exec skill users
  - any Claude session sending exec or chain requests

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fail-fast HTTP 503 before send() when relay is not in REALTIME mode"
    - "Module-level timestamp tracker updated on every incoming WS message"
    - "Health endpoint exposes degradation mode for callers to decide before sending"

key-files:
  created: []
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/index.js
    - C:/Users/bono/racingpoint/comms-link/.claude/skills/rp-bono-exec/SKILL.md

key-decisions:
  - "lastBonoMessageAt set at top of client.on('message') handler -- captures all incoming WS messages including heartbeats"
  - "REALTIME guard placed AFTER execId assignment (so 503 response includes execId for tracing) but BEFORE client.send()"
  - "Existing if (!sent) guard preserved as secondary safety net -- defense in depth"
  - "SKILL.md health section inserted between Common commands and SSH fallback -- logical flow: check health, then decide to send or fall back"

patterns-established:
  - "Health endpoint pattern: always expose connectionMode.mode alongside raw connected boolean"
  - "Fail-fast pattern: check connection mode before buffering messages, return 503 with mode field for self-diagnosis"

requirements-completed: [VIS-01, VIS-02, VIS-03]

# Metrics
duration: 15min
completed: 2026-03-22
---

# Phase 136 Plan 02: Chain Endpoint Visibility Summary

**Relay health endpoint now exposes connectionMode + lastHeartbeat, /relay/exec/run fails fast with 503 when not REALTIME, and SKILL.md guides callers to probe health before sending**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-22T04:10:00Z
- **Completed:** 2026-03-22T04:25:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- GET /relay/health now returns `{ connected, connectionMode, lastHeartbeat }` — callers can see degradation state without guessing
- POST /relay/exec/run returns HTTP 503 immediately (not after 120s timeout) when connectionMode is not REALTIME, with clear message pointing to /relay/health
- rp-bono-exec SKILL.md has a "Check relay health first" section with curl example, mode explanations, and guidance to not retry in a loop

## Task Commits

Each task was committed atomically:

1. **Task 1: Enhance /relay/health and guard /relay/exec/run** - `0b8e35d` (feat)
2. **Task 2: Update rp-bono-exec SKILL.md with health probe guidance** - `3f1fa9e` (feat)

**Plan metadata:** `42cbe0a` (chore: LOGBOOK update)

## Files Created/Modified
- `C:/Users/bono/racingpoint/comms-link/james/index.js` - Added lastBonoMessageAt tracker, enriched /relay/health response, added REALTIME guard in /relay/exec/run
- `C:/Users/bono/racingpoint/comms-link/.claude/skills/rp-bono-exec/SKILL.md` - Added "Check relay health first" section with connectionMode explanations

## Decisions Made
- `lastBonoMessageAt` set at the top of `client.on('message', ...)` before any type checks — captures every incoming WS message, including control messages and heartbeats
- REALTIME guard placed after `execId` is assigned so the 503 response includes `execId` for tracing, but before `client.send()` so no message is sent when degraded
- Existing `if (!sent)` check preserved after `client.send()` as secondary defense
- New SKILL.md section placed between "Common commands" and "SSH fallback" so callers read it before reaching the fallback guidance

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Relay degradation is now fully visible to callers (health endpoint) and self-enforcing (exec/run fails fast)
- Callers using rp-bono-exec skill will see health probe guidance on first load
- Bono needs to pull and restart comms-link to pick up the new health fields

---
*Phase: 136-chain-endpoint-visibility*
*Completed: 2026-03-22*
