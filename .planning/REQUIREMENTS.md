# Requirements: v23.0 Audit Protocol v4.0

**Defined:** 2026-03-25
**Core Value:** One command runs 60 audit phases across the entire fleet and produces actionable, comparable results — no copy-paste, no manual tracking, no missed checks.

## v1 Requirements

### Core Runner

- [ ] **RUN-01**: Operator can execute full audit with `bash audit.sh --mode <mode>` (quick|standard|full|pre-ship|post-incident)
- [ ] **RUN-02**: Each phase produces structured JSON: `{phase, tier, host, status, severity, message, timestamp, mode, venue_state}`
- [ ] **RUN-03**: Every check has a configurable timeout (default 10s) — one offline pod cannot hang the audit
- [ ] **RUN-04**: All 60 phases from AUDIT-PROTOCOL v3.0 are ported as non-interactive bash functions
- [ ] **RUN-05**: Shared library (lib/core.sh) provides `record_result()`, `record_fix()`, `exec_on_pod()`, `exec_on_server()` primitives
- [ ] **RUN-06**: cmd.exe quoting wrapper in exec helpers — prevents the 4+ known quoting pitfalls through rc-agent `/exec`
- [ ] **RUN-07**: curl output sanitization — strips quotes from health endpoint responses (`"200"` → `200`)
- [ ] **RUN-08**: jq is validated at startup — audit aborts with clear error if jq not found
- [ ] **RUN-09**: Auth token obtained automatically at audit start from `/api/v1/terminal/auth` (PIN from env var, not hardcoded)
- [ ] **RUN-10**: Auth token refresh mid-run if full audit exceeds token lifespan

### Execution

- [ ] **EXEC-01**: Venue-open/closed auto-detection via fleet health API (any pod with active billing = open) with time-of-day fallback
- [ ] **EXEC-02**: Display & hardware tiers produce QUIET (not FAIL) when venue is closed
- [ ] **EXEC-03**: Parallel pod queries with 4-concurrent-connection semaphore using file-based locking
- [ ] **EXEC-04**: Background jobs write to per-pod temp files (`$RESULT_DIR/phase_host.json`), assembled after `wait`
- [ ] **EXEC-05**: Mode selects which tiers to run: quick=1-2, standard=1-11, full=1-18, pre-ship=critical subset, post-incident=incident subset
- [ ] **EXEC-06**: `--tier N` and `--phase N` flags for running individual tiers or phases
- [ ] **EXEC-07**: UTC→IST timestamp conversion in all output (standing rule compliance)

### Intelligence

- [ ] **INTL-01**: Delta tracking compares current run against previous run's JSON, highlighting regressions (PASS→FAIL) and improvements (FAIL→PASS)
- [ ] **INTL-02**: Delta is mode-aware and venue-state-aware — PASS→QUIET is NOT flagged as regression
- [ ] **INTL-03**: Known-issue suppression via `suppress.json` with fields: phase, host_pattern, message_pattern, reason, added_date, expires_date, owner
- [ ] **INTL-04**: Suppressed issues appear in report as SUPPRESSED with reason, not silently hidden
- [ ] **INTL-05**: Severity scoring: PASS/WARN/FAIL/QUIET/SUPPRESSED status + P1 (service down) / P2 (degraded) / P3 (cosmetic) severity
- [ ] **INTL-06**: Markdown report with tier summary tables, phase details, delta section, fix actions taken, and overall verdict
- [ ] **INTL-07**: JSON summary file alongside markdown report for machine consumption
- [ ] **INTL-08**: Suppression entries with expired `expires_date` are automatically ignored (stale suppression prevention)

### Auto-Fix

- [ ] **FIX-01**: Auto-fix only executes when `--auto-fix` flag is passed (off by default)
- [ ] **FIX-02**: Every fix function checks `is_pod_idle()` before executing — never touch a pod with active billing
- [ ] **FIX-03**: Every fix function checks for OTA_DEPLOYING and MAINTENANCE_MODE sentinels before executing
- [ ] **FIX-04**: Safe fix: clear stale MAINTENANCE_MODE / GRACEFUL_RELAUNCH / restart sentinel files
- [ ] **FIX-05**: Safe fix: kill orphan PowerShell processes (count > 1) on pods
- [ ] **FIX-06**: Safe fix: restart rc-agent via schtasks on pods where it's down but rc-sentry is up
- [ ] **FIX-07**: All fix actions logged to JSON with before/after state for audit trail
- [ ] **FIX-08**: Explicit approved-fixes whitelist array in lib/fixes.sh — no fix runs unless listed

### Notifications

- [ ] **NOTF-01**: Audit summary sent to Bono via comms-link `send-message.js` on completion
- [ ] **NOTF-02**: Audit summary appended to comms-link INBOX.md with git push
- [ ] **NOTF-03**: WhatsApp summary to Uday via Bono relay Evolution API — P1/P2 counts + overall verdict
- [ ] **NOTF-04**: Notifications only fire on `--notify` flag (off by default for test runs)
- [ ] **NOTF-05**: Notification includes delta summary if previous run exists (regressions highlighted)

### Results Management

- [ ] **RSLT-01**: Results stored in `audit/results/YYYY-MM-DD_HH-MM/` with JSON + Markdown files
- [ ] **RSLT-02**: `audit/results/latest` symlink points to most recent run
- [ ] **RSLT-03**: Results committed to git when `--commit` flag is passed
- [ ] **RSLT-04**: Previous run's JSON automatically found for delta comparison (latest symlink)

## Future Requirements

### Advanced Automation
- **ADV-01**: Scheduled audit via cron/Task Scheduler (daily quick, weekly standard)
- **ADV-02**: Trend dashboard — multi-run severity charts over time
- **ADV-03**: Per-pod health score computed from weighted phase results
- **ADV-04**: Integration with GSD milestone verification (auto-run pre-ship audit on `/gsd:verify-work`)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Continuous monitoring daemon | PROJECT.md constraint — audit is point-in-time, not a service |
| Node.js/Python audit logic | Pure bash constraint — only jq as external dependency |
| Auto-deploy stale binaries | Too destructive — deploy decisions require human confirmation |
| Visual verification automation | Standing rule: display-affecting checks need physical/screenshot verification |
| Pod binary rebuild | Audit detects staleness but doesn't compile — that's deploy tooling |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| RUN-01 | — | Pending |
| RUN-02 | — | Pending |
| RUN-03 | — | Pending |
| RUN-04 | — | Pending |
| RUN-05 | — | Pending |
| RUN-06 | — | Pending |
| RUN-07 | — | Pending |
| RUN-08 | — | Pending |
| RUN-09 | — | Pending |
| RUN-10 | — | Pending |
| EXEC-01 | — | Pending |
| EXEC-02 | — | Pending |
| EXEC-03 | — | Pending |
| EXEC-04 | — | Pending |
| EXEC-05 | — | Pending |
| EXEC-06 | — | Pending |
| EXEC-07 | — | Pending |
| INTL-01 | — | Pending |
| INTL-02 | — | Pending |
| INTL-03 | — | Pending |
| INTL-04 | — | Pending |
| INTL-05 | — | Pending |
| INTL-06 | — | Pending |
| INTL-07 | — | Pending |
| INTL-08 | — | Pending |
| FIX-01 | — | Pending |
| FIX-02 | — | Pending |
| FIX-03 | — | Pending |
| FIX-04 | — | Pending |
| FIX-05 | — | Pending |
| FIX-06 | — | Pending |
| FIX-07 | — | Pending |
| FIX-08 | — | Pending |
| NOTF-01 | — | Pending |
| NOTF-02 | — | Pending |
| NOTF-03 | — | Pending |
| NOTF-04 | — | Pending |
| NOTF-05 | — | Pending |
| RSLT-01 | — | Pending |
| RSLT-02 | — | Pending |
| RSLT-03 | — | Pending |
| RSLT-04 | — | Pending |

**Coverage:**
- v1 requirements: 42 total
- Mapped to phases: 0
- Unmapped: 42 ⚠️

---
*Requirements defined: 2026-03-25*
*Last updated: 2026-03-25 after milestone v23.0 initialization*
