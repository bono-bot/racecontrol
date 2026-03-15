---
phase: 02-game-crash-recovery
type: research
---

# Phase 2: Game Crash Recovery — Research

## What Already Exists

### Agent-side crash detection (main.rs:746-850)
- 2s polling interval (`game_check_interval.tick()`)
- Detects process exit via `is_running()` → `try_wait()` / PID scan
- On crash during billing: zeroes FFB, sends GameStateUpdate(Error) + GameCrashed + FfbZeroed
- Arms 30s crash_recovery_timer — if core doesn't send SessionEnded, force-resets pod
- AI debugger triggered on crash

### Core-side auto-relaunch (game_launcher.rs:312-403)
- "Race Engineer": on GameState::Error + billing active → relaunch with 5s delay
- Max 2 relaunch attempts per session (auto_relaunch_count on GameTracker)
- Re-checks billing still active + game still Error before relaunching
- After 2 failures: logs "Relaunch Limit Reached", no further action

### Protocol messages (protocol.rs)
- AgentMessage::GameCrashed { pod_id, billing_active } — exists
- AgentMessage::GameStateUpdate(GameLaunchInfo) with GameState::Error — exists
- CoreToAgentMessage::LaunchGame — exists (used for relaunch)

### Kiosk (types.ts + KioskPodCard.tsx)
- GameState type includes "error" — but NO component renders it
- KioskPodCard shows "Launch Game" button only when game_state === "idle"
- No "Game Crashed" badge, no manual relaunch button

### Billing (billing.rs)
- BillingSessionStatus has PausedGamePause (ESC key), PausedDisconnect, PausedManual
- NO PausedGameCrash status — billing continues during crash + relaunch

## Requirement Gaps

| Requirement | Current State | Gap |
|-------------|--------------|-----|
| CRASH-01: detect exit within 5s | 2s polling ✅ | None — already met |
| CRASH-02: auto-pause billing on crash | Billing continues ❌ | Need pause on GameCrashed in ws/mod.rs |
| CRASH-03: "Game Crashed" on dashboard | Not rendered ❌ | Kiosk needs error state badge |
| CRASH-04: re-launch from kiosk | Only auto-relaunch ❌ | Kiosk needs manual relaunch button |

## Implementation Strategy

**CRASH-01 is already done** — 2s polling exceeds the 5s requirement.

**CRASH-02**: In ws/mod.rs where GameCrashed is handled (line 393), add billing pause. Use existing `handle_game_status_update()` with a synthetic `AcStatus::Off` to trigger pause, OR directly transition timer to PausedGamePause. The Race Engineer relaunch (game_launcher.rs:312) already runs on GameState::Error, so billing pause + relaunch happen in parallel — when game successfully relaunches and goes Running → billing resumes via AcStatus::Live.

**CRASH-03 + CRASH-04**: KioskPodCard already receives `gameInfo` with `game_state`. Add conditional rendering for "error" state: red "Game Crashed" badge + "Relaunch" button. Button calls existing `POST /pods/{id}/launch` with the stored launch_args from GameTracker.

## Key Insight

The auto-relaunch (Race Engineer) and manual relaunch (kiosk button) are complementary:
- Race Engineer fires 5s after crash (max 2 attempts) — handles most crashes silently
- If Race Engineer exhausts 2 attempts, game_state stays "error" → kiosk shows crashed badge with manual button
- Manual button resets auto_relaunch_count to allow fresh attempts

## Plan Split

- **Plan 02-01**: rc-core (billing pause on GameCrashed + relaunch endpoint for manual recovery)
- **Plan 02-02**: kiosk ("Game Crashed" badge + manual relaunch button on KioskPodCard and LiveSessionPanel)
