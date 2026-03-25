# Requirements: v23.0 Audit Protocol v4.0

**Defined:** 2026-03-25
**Core Value:** One command runs 60 audit phases across the entire fleet and produces actionable, comparable results — no copy-paste, no manual tracking, no missed checks.

## v1 Requirements

### Core Runner

- [x] **RUN-01**: Operator can execute full audit with `bash audit.sh --mode <mode>` (quick|standard|full|pre-ship|post-incident)
- [x] **RUN-02**: Each phase produces structured JSON: `{phase, tier, host, status, severity, message, timestamp, mode, venue_state}`
- [x] **RUN-03**: Every check has a configurable timeout (default 10s) — one offline pod cannot hang the audit
- [x] **RUN-04**: All 60 phases from AUDIT-PROTOCOL v3.0 are ported as non-interactive bash functions
- [x] **RUN-05**: Shared library (lib/core.sh) provides `record_result()`, `record_fix()`, `exec_on_pod()`, `exec_on_server()` primitives
- [x] **RUN-06**: cmd.exe quoting wrapper in exec helpers — prevents the 4+ known quoting pitfalls through rc-agent `/exec`
- [x] **RUN-07**: curl output sanitization — strips quotes from health endpoint responses (`"200"` → `200`)
- [x] **RUN-08**: jq is validated at startup — audit aborts with clear error if jq not found
- [x] **RUN-09**: Auth token obtained automatically at audit start from `/api/v1/terminal/auth` (PIN from env var, not hardcoded)
- [x] **RUN-10**: Auth token refresh mid-run if full audit exceeds token lifespan

### Execution

- [x] **EXEC-01**: Venue-open/closed auto-detection via fleet health API (any pod with active billing = open) with time-of-day fallback
- [x] **EXEC-02**: Display & hardware tiers produce QUIET (not FAIL) when venue is closed
- [x] **EXEC-03**: Parallel pod queries with 4-concurrent-connection semaphore using file-based locking
- [x] **EXEC-04**: Background jobs write to per-pod temp files (`$RESULT_DIR/phase_host.json`), assembled after `wait`
- [x] **EXEC-05**: Mode selects which tiers to run: quick=1-2, standard=1-11, full=1-18, pre-ship=critical subset, post-incident=incident subset
- [x] **EXEC-06**: `--tier N` and `--phase N` flags for running individual tiers or phases
- [x] **EXEC-07**: UTC→IST timestamp conversion in all output (standing rule compliance)

### Intelligence

- [x] **INTL-01**: Delta tracking compares current run against previous run's JSON, highlighting regressions (PASS→FAIL) and improvements (FAIL→PASS)
- [x] **INTL-02**: Delta is mode-aware and venue-state-aware — PASS→QUIET is NOT flagged as regression
- [x] **INTL-03**: Known-issue suppression via `suppress.json` with fields: phase, host_pattern, message_pattern, reason, added_date, expires_date, owner
- [x] **INTL-04**: Suppressed issues appear in report as SUPPRESSED with reason, not silently hidden
- [x] **INTL-05**: Severity scoring: PASS/WARN/FAIL/QUIET/SUPPRESSED status + P1 (service down) / P2 (degraded) / P3 (cosmetic) severity
- [x] **INTL-06**: Markdown report with tier summary tables, phase details, delta section, fix actions taken, and overall verdict
- [x] **INTL-07**: JSON summary file alongside markdown report for machine consumption
- [x] **INTL-08**: Suppression entries with expired `expires_date` are automatically ignored (stale suppression prevention)

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

- [x] **RSLT-01**: Results stored in `audit/results/YYYY-MM-DD_HH-MM/` with JSON + Markdown files
- [x] **RSLT-02**: `audit/results/latest` symlink points to most recent run
- [ ] **RSLT-03**: Results committed to git when `--commit` flag is passed
- [x] **RSLT-04**: Previous run's JSON automatically found for delta comparison (latest symlink)

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
| RUN-01 | Phase 189 | Complete |
| RUN-02 | Phase 189 | Complete |
| RUN-03 | Phase 189 | Complete |
| RUN-04 | Phase 190 | Complete |
| RUN-05 | Phase 189 | Complete |
| RUN-06 | Phase 189 | Complete |
| RUN-07 | Phase 189 | Complete |
| RUN-08 | Phase 189 | Complete |
| RUN-09 | Phase 189 | Complete |
| RUN-10 | Phase 189 | Complete |
| EXEC-01 | Phase 189 | Complete |
| EXEC-02 | Phase 189 | Complete |
| EXEC-03 | Phase 191 | Complete |
| EXEC-04 | Phase 191 | Complete |
| EXEC-05 | Phase 190 | Complete |
| EXEC-06 | Phase 190 | Complete |
| EXEC-07 | Phase 189 | Complete |
| INTL-01 | Phase 192 | Complete |
| INTL-02 | Phase 192 | Complete |
| INTL-03 | Phase 192 | Complete |
| INTL-04 | Phase 192 | Complete |
| INTL-05 | Phase 192 | Complete |
| INTL-06 | Phase 192 | Complete |
| INTL-07 | Phase 192 | Complete |
| INTL-08 | Phase 192 | Complete |
| FIX-01 | Phase 193 | Pending |
| FIX-02 | Phase 193 | Pending |
| FIX-03 | Phase 193 | Pending |
| FIX-04 | Phase 193 | Pending |
| FIX-05 | Phase 193 | Pending |
| FIX-06 | Phase 193 | Pending |
| FIX-07 | Phase 193 | Pending |
| FIX-08 | Phase 193 | Pending |
| NOTF-01 | Phase 193 | Pending |
| NOTF-02 | Phase 193 | Pending |
| NOTF-03 | Phase 193 | Pending |
| NOTF-04 | Phase 193 | Pending |
| NOTF-05 | Phase 193 | Pending |
| RSLT-01 | Phase 192 | Complete |
| RSLT-02 | Phase 192 | Complete |
| RSLT-03 | Phase 193 | Pending |
| RSLT-04 | Phase 192 | Complete |

**Coverage:**
- v1 requirements: 42 total
- Mapped to phases: 42
- Unmapped: 0 ✓

| Phase | Requirements |
|-------|-------------|
| Phase 189 | RUN-01, RUN-02, RUN-03, RUN-05, RUN-06, RUN-07, RUN-08, RUN-09, RUN-10, EXEC-01, EXEC-02, EXEC-07 (12 requirements) |
| Phase 190 | RUN-04, EXEC-05, EXEC-06 (3 requirements) |
| Phase 191 | EXEC-03, EXEC-04 (2 requirements) |
| Phase 192 | INTL-01, INTL-02, INTL-03, INTL-04, INTL-05, INTL-06, INTL-07, INTL-08, RSLT-01, RSLT-02, RSLT-04 (11 requirements) |
| Phase 193 | FIX-01, FIX-02, FIX-03, FIX-04, FIX-05, FIX-06, FIX-07, FIX-08, NOTF-01, NOTF-02, NOTF-03, NOTF-04, NOTF-05, RSLT-03 (14 requirements) |

---
*Requirements defined: 2026-03-25*
*Last updated: 2026-03-25 after roadmap created for v23.0*
