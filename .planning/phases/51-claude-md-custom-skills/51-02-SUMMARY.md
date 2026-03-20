---
phase: 51
plan: "02"
status: complete
started: 2026-03-20
completed: 2026-03-20
duration_minutes: 5
---

# Plan 51-02 Summary: Custom Skills

## What Was Built

Four custom Claude Code skills for Racing Point operations:

1. **`/rp:deploy`** — Build rc-agent release binary, verify size > 8MB, copy to deploy-staging, output pendrive command. `disable-model-invocation: true`.
2. **`/rp:deploy-server`** — Full pipeline: build racecontrol → kill old process → swap binary → verify :8080 → git commit → notify Bono. `disable-model-invocation: true`.
3. **`/rp:pod-status`** — Query fleet/health API, extract specific pod by pod_number, display WS/HTTP status. Model-invocable (read-only).
4. **`/rp:incident`** — 4-tier debug order (deterministic → memory → Ollama → cloud), auto-query pod status, confirm destructive only, auto-log to LOGBOOK. Model-invocable. Fallback guide-only mode when server unreachable.

## Key Files

### Created
- `.claude/skills/rp-deploy/SKILL.md`
- `.claude/skills/rp-deploy-server/SKILL.md`
- `.claude/skills/rp-pod-status/SKILL.md`
- `.claude/skills/rp-incident/SKILL.md`

## Self-Check: PASSED

- All 4 SKILL.md files exist
- Only rp-deploy and rp-deploy-server have `disable-model-invocation: true`
- rp-pod-status has fleet/health endpoint and all 8 pod IPs
- rp-incident has 4-tier debug, LOGBOOK auto-logging, destructive confirmation gate, guide-only fallback

## Commits

| Hash | Message |
|------|---------|
| 2458fc1 | feat(51-02): create /rp:deploy and /rp:deploy-server skills |
| (prior) | feat(51-02): create /rp:pod-status skill |
| 48bae43 | feat(51-02): create /rp:incident skill with 4-tier debug order |

## Tasks: 2/2

| Task | Status | Files |
|------|--------|-------|
| Task 1: /rp:deploy + /rp:deploy-server | Complete | 2 SKILL.md |
| Task 2: /rp:pod-status + /rp:incident | Complete | 2 SKILL.md |

## Deviations

- Task 2 was interrupted mid-execution; /rp:incident was created in a continuation pass rather than by the original executor agent. Content matches plan specification exactly.
