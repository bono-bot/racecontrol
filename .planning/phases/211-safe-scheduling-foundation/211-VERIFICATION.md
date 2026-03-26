---
phase: 211-safe-scheduling-foundation
verified: 2026-03-26T07:15:00Z
status: gaps_found
score: 3/5 must-haves verified
gaps:
  - truth: "James auto-detect.sh runs daily at 02:30 IST via Windows Task Scheduler without human trigger"
    status: failed
    reason: "register-auto-detect-task.bat exists and is correct, but has not been executed. schtasks /Query /TN AutoDetect-Daily returns ERROR: The system cannot find the file specified. Task 2 in Plan 02 is a checkpoint:human-verify that was never cleared."
    artifacts:
      - path: "scripts/register-auto-detect-task.bat"
        issue: "File is correct and complete, but the task it registers has not been created in Windows Task Scheduler"
    missing:
      - "Run scripts/register-auto-detect-task.bat as Administrator on James machine to register AutoDetect-Daily in Task Scheduler"
      - "Confirm via schtasks /Query /TN AutoDetect-Daily /FO LIST that task shows Daily at 02:30, runs as SYSTEM"
  - truth: "Bono auto-detect runs daily at 02:35 IST via cron (5 minutes after James)"
    status: failed
    reason: "crontab -l on Bono VPS (root@100.70.177.44) shows no bono-auto-detect entry at all. The SUMMARY claimed SSH correction was applied but it did not persist — the full crontab shows 6 entries, none referencing bono-auto-detect.sh. The script itself exists at /root/racecontrol/scripts/bono-auto-detect.sh."
    artifacts:
      - path: "scripts/bono-auto-detect.sh"
        issue: "Script exists on VPS but no cron entry schedules it"
    missing:
      - "Add cron entry: 5 21 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh >> /root/auto-detect-logs/cron.log 2>&1"
      - "Verify with: ssh root@100.70.177.44 crontab -l | grep bono-auto-detect (must show 5 21)"
human_verification:
  - test: "Run register-auto-detect-task.bat as Administrator"
    expected: "AutoDetect-Daily task appears in Task Scheduler (taskschd.msc) with Daily trigger at 02:30, runs as SYSTEM with Highest privileges"
    why_human: "schtasks /Create requires Administrator elevation; cannot be verified from Git Bash context"
---

# Phase 211: Safe Scheduling Foundation Verification Report

**Phase Goal:** The auto-detect pipeline runs on schedule without human intervention and is safe from its first execution — no fix action fires without checking billing state, sentinel files, and escalation cooldown
**Verified:** 2026-03-26T07:15:00Z (IST 12:45)
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A second auto-detect invocation while one is running exits immediately with 'already running' message | VERIFIED | `_acquire_run_lock()` at line 81-92, "already running (PID $existing_pid). Exiting." at line 86, `trap "rm -f $PID_FILE" EXIT` at line 95 |
| 2 | After a WhatsApp alert for pod+issue, the same combo does not alert again within 6 hours | VERIFIED | `_is_cooldown_active`/`_record_alert` at lines 109-130, `ESCALATION_COOLDOWN_SECS=21600`, wired per `pod_ip:issue_type` at lines 552-567 |
| 3 | When venue is open, auto-detect runs in quick mode regardless of --mode argument | VERIFIED | `venue_state_detect()` called at line 53, override at lines 58-61 with "SCHED-05" label |
| 4 | Sentinel files OTA_DEPLOYING and MAINTENANCE_MODE block fix actions on affected pods | VERIFIED | `check_pod_sentinels()` checks both flags (line 56 in fixes.sh); called via run_auto_fixes() at line 125; wired through audit.sh --auto-fix pipeline |
| 5 | James auto-detect.sh runs daily at 02:30 IST via Windows Task Scheduler without human trigger | FAILED | `register-auto-detect-task.bat` created correctly but never executed. `schtasks /Query /TN AutoDetect-Daily` → "The system cannot find the file specified." Task does not exist. |
| 6 | Bono auto-detect runs daily at 02:35 IST via cron (5 minutes after James) | FAILED | `crontab -l` on VPS shows 6 entries, NONE referencing bono-auto-detect. SUMMARY claimed SSH correction succeeded but it did not persist or was not applied. |

**Score:** 4/6 truths verified (SCHED-03, SCHED-04, SCHED-05, sentinel extension all pass; SCHED-01, SCHED-02 fail)

---

## Required Artifacts

### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `scripts/auto-detect.sh` | PID guard, cooldown, venue-aware mode, sentinel check | VERIFIED | All 9 acceptance criteria patterns present: `_acquire_run_lock`, `PID_FILE`, `_is_cooldown_active`, `_record_alert`, `ESCALATION_COOLDOWN_SECS=21600`, `venue_state_detect`, `SCHED-05`, `COOLDOWN_FILE`, main() banner lines |
| `audit/lib/fixes.sh` | Extended sentinel check (OTA_DEPLOYING + MAINTENANCE_MODE) | VERIFIED | Line 56: checks both `OTA_DEPLOYING` (OTA_ACTIVE) and `MAINTENANCE_MODE` (MM_ACTIVE); backward-compatible return code |
| `.gitignore` | Excludes auto-detect-cooldown.json | VERIFIED | Line 61: `audit/results/auto-detect-cooldown.json` present |

### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `scripts/register-auto-detect-task.bat` | Windows Task Scheduler registration for daily 02:30 trigger | VERIFIED (file) / FAILED (execution) | File exists and is correct: contains AutoDetect-Daily, 02:30 schedule, safety gate check for _acquire_run_lock, AUDIT_PIN baked in, goto labels (no parentheses). However the task is not registered in Windows Task Scheduler. |

---

## Key Link Verification

### Plan 01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `scripts/auto-detect.sh` | `audit/lib/core.sh` | `source + venue_state_detect()` | WIRED | Line 48: `source "$REPO_ROOT/audit/lib/core.sh"`. `venue_state_detect` called at line 53. Function exported via `export -f` in core.sh line 160. |
| `scripts/auto-detect.sh` | `audit/lib/fixes.sh` | `source + check_pod_sentinels()` | WIRED (indirect) | auto-detect.sh calls `bash audit/audit.sh --auto-fix` → audit.sh sources fixes.sh (lines 52-54) → `check_pod_sentinels()` called in `run_auto_fixes()` line 125. Wiring is indirect but live. |
| `scripts/auto-detect.sh` | `audit/results/auto-detect-cooldown.json` | `_is_cooldown_active + _record_alert` | WIRED | `COOLDOWN_FILE` defined at line 106; `_is_cooldown_active` reads it at line 114; `_record_alert` writes it at lines 127-129; both called in notify step lines 559-561. |

### Plan 02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `scripts/register-auto-detect-task.bat` | `scripts/auto-detect.sh` | schtasks /Create with bash.exe path | NOT_WIRED (runtime) | Bat contains correct path and command. Task not registered → link is not live at runtime. |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|------------|------------|-------------|--------|---------|
| SCHED-01 | 211-02 | James auto-detect runs daily at 02:30 IST via Windows Task Scheduler | BLOCKED | `register-auto-detect-task.bat` exists but task not registered. `schtasks /Query /TN AutoDetect-Daily` fails with "file not found". |
| SCHED-02 | 211-02 | Bono auto-detect runs daily at 02:35 IST via cron (5-min offset) | BLOCKED | Verified via SSH: `crontab -l` on root@100.70.177.44 shows no bono-auto-detect entry. Script at `/root/racecontrol/scripts/bono-auto-detect.sh` exists (9822 bytes) but is unscheduled. |
| SCHED-03 | 211-01 | Run guard prevents overlapping auto-detect executions (PID file lock) | SATISFIED | `_acquire_run_lock` at line 81, PID written at line 91, kill -0 liveness check at line 85, EXIT trap at line 95. Commit `090b2b32` verified real. |
| SCHED-04 | 211-01 | Escalation cooldown prevents repeated WhatsApp alerts for same issue within 6 hours | SATISFIED | `ESCALATION_COOLDOWN_SECS=21600`, keyed `pod_ip:issue_type`, atomic write via .tmp swap, wired per-pod in notify step. |
| SCHED-05 | 211-01 | Venue-state-aware timing — full mode during closed hours, quick mode if triggered during open hours | SATISFIED | `venue_state_detect()` from core.sh sourced after arg parsing, override guard at lines 58-61, logged in main() banner at line 582. |

---

## Anti-Patterns Found

### scripts/auto-detect.sh

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `scripts/auto-detect.sh` | 552-567 | WhatsApp escalation block logs cooldown intent but does not actually send. Comment: "WhatsApp send function (from notify.sh) will be wired in Phase 213" | Info | Intentional deferral documented in plan. Cooldown infrastructure is live; send call deferred to Phase 213. Not a blocker for Phase 211 goal. |

### scripts/register-auto-detect-task.bat

No anti-patterns found. File follows .bat standing rules: CRLF, ASCII, goto labels, no parentheses in if/else, AUDIT_PIN baked into command string.

---

## Human Verification Required

### 1. Register AutoDetect-Daily in Windows Task Scheduler

**Test:** Open cmd.exe as Administrator on James machine (.27). Run:
```
cd C:\Users\bono\racingpoint\racecontrol\scripts
register-auto-detect-task.bat
```
Then run: `schtasks /Query /TN AutoDetect-Daily /FO LIST`

**Expected:** Task listed with:
- Task Name: AutoDetect-Daily
- Schedule Type: Daily
- Start Time: 2:30:00 AM
- Run As User: SYSTEM

**Why human:** `schtasks /Create /RU SYSTEM /RL HIGHEST` requires Administrator elevation. Cannot be executed from Git Bash as user bono.

---

## Gaps Summary

Two gaps block full SCHED-01 and SCHED-02 goal achievement:

**SCHED-01 (Task Scheduler):** The bat file is complete and correct, but Windows Task Scheduler does not have the AutoDetect-Daily task registered. Plan 02 Task 2 is a `checkpoint:human-verify` gate that requires the bat to be run as Administrator. This gate was never cleared — the SUMMARY documents "Task 2 requires manual execution as Administrator" and "User Setup Required" but marks the phase completed. The pipeline will NOT run autonomously until this is executed.

**SCHED-02 (Bono cron):** The SUMMARY states the cron was corrected via SSH from `0 21` to `5 21`. Live verification via `ssh root@100.70.177.44 "crontab -l"` shows no bono-auto-detect entry whatsoever — neither the old `0 21` nor the new `5 21`. Either the SSH command was not executed, the cron was removed by another operation, or it was applied to a different user's crontab. The script exists on the VPS but is not scheduled.

**Root cause commonality:** Both failures are infrastructure-registration gaps (Windows Task Scheduler + Linux cron), not code gaps. The safety gate code (SCHED-03, SCHED-04, SCHED-05) is complete and correct. The scheduling infrastructure that makes the pipeline autonomous has not been activated.

---

_Verified: 2026-03-26T07:15:00Z (IST 12:45)_
_Verifier: Claude (gsd-verifier)_
