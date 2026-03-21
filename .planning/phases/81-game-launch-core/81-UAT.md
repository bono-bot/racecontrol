---
status: complete
phase: 81-game-launch-core
source: 81-01-SUMMARY.md, 81-02-SUMMARY.md, 81-03-SUMMARY.md
started: 2026-03-21T12:00:00+05:30
updated: 2026-03-21T12:00:00+05:30
---

## Current Test

[testing complete]

## Tests

### 1. Kiosk Game Picker Panel
expected: On the staff dashboard, click "Launch Game" on any idle pod card. GamePickerPanel appears with installed games list, logos/chips, and Launch buttons. Non-AC games launch directly. AC redirects to wizard.
result: skipped
reason: Requires server .23 with PIN configured and pods online. Deferred to deployment.

### 2. Pod Card Game Display
expected: When a game is running on a pod, the pod card shows a 40x40px game logo (or abbreviation chip fallback) and the game name. Visible in both "selecting" and "on_track" states.
result: skipped
reason: Requires live pod with game running. Deferred to deployment.

### 3. PWA Game Request Banner
expected: When a customer requests a game from PWA, staff sees a GameLaunchRequestBanner at the top of the staff dashboard with game name, pod number, driver name, "Confirm Launch" and "Dismiss" buttons. Banner auto-expires after 60 seconds.
result: skipped
reason: Requires PWA + server + WebSocket. Deferred to deployment.

### 4. Non-AC Crash Recovery
expected: If a non-AC game (e.g., F1 25) crashes or exits unexpectedly, rc-agent detects it within 30 seconds, cleans up stale processes, and auto-relaunches the same game with the same config. Staff is alerted if relaunch fails.
result: skipped
reason: Requires live pod with game installed. Deferred to deployment.

### 5. TOML Game Profiles
expected: The deployment TOML template (deploy/rc-agent.template.toml) contains game stanzas for all 6 games with correct Steam app IDs (F1 25=3059520, iRacing=266410, AC EVO=3058630, EA WRC=3917090, LMU=1564310, AC=use_steam false). The example TOML is consistent.
result: pass

### 6. Cargo Build Clean
expected: Running `cargo build --release --bin rc-agent && cargo build --release --bin racecontrol` compiles without errors. All new code integrates cleanly with existing codebase.
result: pass

### 7. Kiosk Build Clean
expected: Running `cd kiosk && npx next build` completes without errors. All new components (GamePickerPanel, GameLaunchRequestBanner, gameDisplayInfo) compile and bundle correctly.
result: pass

## Summary

total: 7
passed: 3
issues: 0
pending: 0
skipped: 4

## Gaps

[none yet]
