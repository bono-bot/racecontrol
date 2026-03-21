---
phase: 93-community-tribal-identity
plan: "01"
subsystem: discord-bot
tags: [discord, scheduler, cron, community, leaderboard, track-records, time-trial, tournaments]
dependency_graph:
  requires: []
  provides: [COMM-01, COMM-02, COMM-04]
  affects: [racingpoint-discord-bot]
tech_stack:
  added: [node-cron]
  patterns: [scheduled-cron-tasks, persisted-state-json, embed-builder, try-catch-cron-safety]
key_files:
  created:
    - /root/racingpoint-discord-bot/src/services/racecontrolService.js
    - /root/racingpoint-discord-bot/src/services/scheduler.js
    - /root/racingpoint-discord-bot/data/record_state.json
  modified:
    - /root/racingpoint-discord-bot/package.json
    - /root/racingpoint-discord-bot/.env
    - /root/racingpoint-discord-bot/src/config.js
    - /root/racingpoint-discord-bot/src/events/ready.js
decisions:
  - "record_state.json stores per-track/car best lap ms — first-time population does not announce to avoid noise on cold start after PM2 restart"
  - "getActiveTournaments filters to active/registering status at the service layer so scheduler only gets relevant entries"
  - "postTournamentUpdates silently skips if no active tournaments — no empty embed spam"
  - "node-cron v4 auto-starts tasks — no .start() call needed"
metrics:
  duration_seconds: 133
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_changed: 7
requirements: [COMM-01, COMM-02, COMM-04]
---

# Phase 93 Plan 01: Automated Community Posts for Discord Bot Summary

**One-liner:** Discord bot gains 3 automated cron tasks — weekly leaderboard (Mon 09:00 IST), 15-minute track record alerts with JSON-persisted state, and weekly time trial/tournament posts (Mon 09:05 IST).

## What Was Built

Three cron jobs wired into the Discord bot's ready event to transform the server into a living community hub:

1. **COMM-01 — Weekly Leaderboard** (`0 9 * * 1` IST): Fetches top 10 lap records via `GET /bot/leaderboard` and posts a Racing Red embed to `#leaderboard`.

2. **COMM-02 — Track Record Alerts** (`*/15 * * * *`): Polls leaderboard every 15 minutes, compares per-track/car bests against `record_state.json`. Announces new records immediately (within 15 minutes — well inside the 1-hour SLA). First-time population is silent to prevent false alerts after restart.

3. **COMM-04 — Weekly Time Trial + Tournaments** (`5 9 * * 1` IST): Fetches active time trial from public endpoint and active/registering tournaments, posts both to `#leaderboard`.

The `#leaderboard` channel is auto-created by `ready.js` if it does not exist — no manual Discord setup needed.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | `57f8572` | node-cron install + racecontrolService.js + config wiring |
| Task 2 | `b52e9ab` | scheduler.js + ready.js leaderboard channel + startScheduler |

## Verification Results

PM2 logs after restart confirm:
- `#leaderboard channel created` (channelId: 1484840434932125826)
- `Channel IDs resolved` (all 3 channels including leaderboard)
- `Scheduler started: 3 cron tasks active`

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `/root/racingpoint-discord-bot/src/services/racecontrolService.js` exists
- [x] `/root/racingpoint-discord-bot/src/services/scheduler.js` exists
- [x] `/root/racingpoint-discord-bot/data/record_state.json` exists (contains `{}`)
- [x] `57f8572` commit exists in discord-bot repo
- [x] `b52e9ab` commit exists in discord-bot repo
- [x] PM2 logs show "Scheduler started: 3 cron tasks active"
