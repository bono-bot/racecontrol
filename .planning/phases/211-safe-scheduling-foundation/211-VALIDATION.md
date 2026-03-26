# Phase 211: Safe Scheduling Foundation - Validation

**Extracted from:** 211-RESEARCH.md Validation Architecture section
**Created:** 2026-03-26

## Test Framework

| Property | Value |
|----------|-------|
| Framework | bash manual assertions (no formal test runner; comms-link test/run-all.sh covers quality gate) |
| Config file | none for phase 211 (shell scripts only) |
| Quick run command | bash scripts/auto-detect.sh --dry-run --no-notify |
| Full suite command | COMMS_PSK=... bash comms-link/test/run-all.sh |

## Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SCHED-01 | Task registered at 02:30 daily | smoke | schtasks /Query /TN AutoDetect-Daily /FO LIST | No -- Wave 0: register-auto-detect-task.bat |
| SCHED-02 | Bono cron at 02:35 IST | smoke | relay exec crontab -l, check for '5 21' | Yes -- verify post-edit |
| SCHED-03 | Double-trigger exits with already running | unit | Two concurrent auto-detect.sh --dry-run; second exits 0 | No -- Wave 0: test-sched-03-pid-guard.sh |
| SCHED-04 | 6-hour cooldown suppresses repeated alerts | unit | Seed cooldown.json now-3600 verify suppressed; now-25200 verify fires | No -- Wave 0: test-sched-04-cooldown.sh |
| SCHED-05 | Quick mode when venue open | unit | Mock fleet with active billing; --dry-run; verify MODE=quick | No -- Wave 0: verify in dry-run output |

## Sampling Rate

- **Per task commit:** bash scripts/auto-detect.sh --dry-run --no-notify
- **Per wave merge:** COMMS_PSK=... bash comms-link/test/run-all.sh
- **Phase gate:** All 5 requirements verified before /gsd:verify-work

## Wave 0 Gaps

- [ ] scripts/register-auto-detect-task.bat -- new file, covers SCHED-01
- [ ] test/test-sched-03-pid-guard.sh -- SCHED-03 double-trigger test
- [ ] test/test-sched-04-cooldown.sh -- SCHED-04 cooldown unit test
- [ ] Verify audit/results/ in .gitignore -- covers cooldown file hygiene
