# Project Research Summary

**Project:** v23.0 Audit Protocol v4.0 -- Automated Fleet Audit Runner
**Domain:** Bash-based automated infrastructure audit system targeting 8 gaming pods, a Windows server, and Bono VPS via HTTP exec endpoints, SSH, and comms-link relay
**Researched:** 2026-03-25 IST
**Confidence:** HIGH

## Executive Summary

v23.0 transforms AUDIT-PROTOCOL v3.0 -- a 1928-line markdown document of 60 phases of copy-paste bash commands requiring 45-90 minutes of active operator attention -- into a single command: `bash audit/audit.sh --mode standard`. The automation gap is entirely in execution mechanics, not in check coverage: the 60 phases and their expected outputs are already defined. The task is to make them run non-interactively, produce structured output, compare against previous runs, and close the loop via comms-link and WhatsApp. The recommended approach is pure bash with jq (the only missing tool, installable via winget), using a modular phase-per-file architecture with a shared lib/ for primitives, parallel execution capped at 4 concurrent pod connections, and JSON output as the foundation for all downstream features.

The key architectural decision is to treat every phase as a thin bash function that writes a JSON result record and returns exit 0 always -- errors are encoded in the JSON, not as bash exit codes. This design allows the main runner to collect all results without aborting on first failure, which is the fundamental requirement of an audit tool. Phase scripts source shared primitives (lib/core.sh) and write to per-run temp files; the report generator assembles everything after all tiers complete. Parallelism is contained within tiers: pod-targeting loops use a file-based semaphore to enforce the 4-connection cap, preventing false FAILs from saturating the venue LAN.

The critical risks are all Windows-specific: cmd.exe quoting destroys remote commands passed through rc-agent's /exec endpoint, curl output in Git Bash can include surrounding quotes that break string comparisons, SSH banners corrupt captured output when stderr is merged with stdout, and parallel background jobs interleave stdout without coordination. All four have documented production incidents in this codebase. The mitigation is to establish safe wrapper functions in Phase 1 (lib/core.sh) before any phase checks are written -- `safe_remote_exec` (writes commands to bat files), `http_get` (jq -r plus quote stripping), and `safe_ssh_capture` (2>/dev/null plus structure validation). Getting these primitives right before building 60 checks on top of them is the single most important implementation decision.

---

## Key Findings

### Recommended Stack

The stack is intentionally minimal. Every tool except jq is already installed on James's machine and already used in AUDIT-PROTOCOL v3.0. The constraint (pure bash, no new compiled dependencies) is workable and appropriate -- adding Node.js or Python for audit logic would create dependency management overhead without simplifying the bash control flow.

**Core technologies:**
- **bash 5.2.37** (existing, Git Bash): audit runner language; `wait -n` (bash 4.3+) enables safe bounded parallelism; `declare -A` associative arrays available
- **jq 1.8.1** (needs `winget install jqlang.jq`): the only missing tool; required for all JSON assembly, parsing, and delta comparison; without it, JSON output requires fragile string concatenation that breaks on every consumer
- **curl 8.18.0** (existing): all HTTP health checks and API queries; always use `--max-time 10 --connect-timeout 5`; never use the `timeout` command (Git Bash semantics differ from Linux)
- **ssh/scp** (existing, OpenSSH via Git Bash): fallback exec when HTTP endpoints are down; always add `2>/dev/null` to SSH captures; validate output structure before parsing
- **diff** (existing, GNU diff): delta tracking between audit runs; `jq -S` normalization before diff for structural JSON comparison

**Supporting patterns:**
- `wait -n` semaphore for bounded parallelism (max 4 concurrent pod connections per standing rule)
- `mktemp -d` temp dir per run for per-pod result files (avoids stdout interleaving from background jobs)
- `trap ... EXIT INT TERM` cleanup for background job management
- `set -euo pipefail` in lib files only; main runner collects FAILs without aborting
- Node.js v22 used only for comms-link `send-message.js` and WhatsApp relay -- NOT for audit logic

**Not to use:** `set -e` in the main runner, inline JSON string concatenation, `xargs -P`, `eval`, unbounded background jobs, `timeout` command for HTTP calls, `.bat` files as the audit format.

---

### Expected Features

The existing v3.0 protocol defines all 60 checks. v23.0 adds the execution layer. Features fall into three groups based on dependency order.

**Must have (table stakes -- without these, a human still manually operates the tool):**
- Single-command invocation with `--mode quick|standard|full|pre-ship|post-incident`
- All 60 v3.0 phases ported as non-interactive bash functions
- Per-check PASS/WARN/FAIL/QUIET status (QUIET = skipped because venue closed or target deliberately offline -- not a failure)
- Venue-open/closed detection -- hardware checks must be QUIET when venue is closed, not FAIL
- Timeout enforcement -- 10s per curl call, 30s per phase; offline pods do not hang the audit
- JSON output file plus Markdown report on every run (both formats from the same execution pass)
- Structured exit codes: 0 = clean, 1 = WARN, 2 = FAIL
- Auth token acquisition at startup, reused across all phases
- Prerequisites check (jq, curl, ssh) as first function before any check runs

**Should have (give automation capability unavailable to copy-paste execution):**
- Delta tracking against previous run -- "Phase 07 added 47 new violations since last audit" is actionable; raw counts are not
- Parallel phase execution within tiers capped at 4 concurrent pod connections (reduces ~24 min sequential to ~6 min)
- Known-issue suppression list with mandatory expiry dates -- suppressed checks appear as KNOWN-ISSUE, not invisible skips
- Severity scoring per phase (P1 = immediate action, P2 = fix before next deploy, P3 = fix before next milestone)
- Audit run history index (timestamp, mode, P1/P2/P3 counts per run)
- Safe auto-fix for pre-approved low-blast-radius operations: clear sentinel files, kill orphan Variable_dump.exe, kill duplicate rc-agent instances; all gated on `is_pod_idle()` before executing

**Defer to after validation (require stable core to be safe to enable):**
- Comms-link notification to Bono on audit completion
- WhatsApp summary to Uday (P1 issues only by default)
- Phase-level retry for transient curl failures (1 retry, 5s delay)
- Continuous monitoring daemon -- explicitly out of scope; existing watchdog infrastructure handles this

**Anti-features (explicitly excluded per PROJECT.md):**
- Daemon/always-on monitoring (watchdog infrastructure already covers it)
- Auto-fix for anything touching billing sessions (active session = customer revenue at risk on false positive)
- Fleet-wide parallel execution (all 8 pods simultaneously) -- exceeds the 4-connection standing rule cap
- Auto-deploy for stale binaries -- requires human confirmation of the 7-step deploy sequence
- Alerting on every WARN -- creates alert fatigue; notify on P1 FAILs and regressions only
- Web dashboard for audit results -- coupling output to admin panel creates runtime dependency

---

### Architecture Approach

The system uses a modular file structure where each of the 60 phases is a thin bash script (one file per phase, 12 tier directories) that emits JSON to stdout and exits 0. A shared lib/ provides all primitives. The main entry point (audit.sh) is a scheduler -- it selects tiers based on mode, initializes shared state, and orchestrates execution; it does not contain check logic.

**Major components:**
1. **audit.sh** -- entry point, mode selector, tier scheduler; parses `--mode`, detects venue state, initializes SESSION token and output paths; does NOT execute check logic
2. **audit/phases/tierN/phaseNN.sh** -- one file per phase; sources lib/core.sh; calls `emit_result`; exits 0 always; preserves v3.0 phase numbering exactly; new v4.0 phases start at 61+
3. **lib/core.sh** -- shared primitives: `emit_result`, `emit_fix`, `http_get` (jq -r plus quote stripping), `safe_remote_exec` (bat-file wrapper for cmd.exe safety), `safe_ssh_capture` (2>/dev/null + structure validation), `get_session_token`, `is_suppressed`, `pod_loop`
4. **lib/parallel.sh** -- file-based semaphore (`/tmp/audit-sem-$RUN_ID/`); enforces 4-concurrent-pod cap; all background jobs write to per-pod temp files, never to shared stdout
5. **lib/fixes.sh** -- pre-approved auto-fix whitelist (FIX-01 through FIX-07: sentinel clearing, orphan process kills, duplicate instance kills); every fix gates on `is_pod_idle()` before executing
6. **lib/delta.sh** -- jq-based join on phase number between previous and current JSON; REGRESSION/IMPROVEMENT/PERSISTENT/NEW_ISSUE/STABLE categories; mode-aware and venue-state-aware to avoid false regressions from context changes
7. **lib/notify.sh** -- comms-link relay (structured JSON) + INBOX.md append (dual-channel standing rule) + WhatsApp via Bono relay; notification failure does NOT abort audit run
8. **generate-report.sh** -- reads all phase result JSON and delta JSON; produces Markdown report and summary JSON; runs once after all tiers complete
9. **suppress.json** -- known-issue suppression list; each entry requires: check_id, reason, added date, expires date, owner; 10-entry cap; expired entries auto-unsuppressed

**Data flow:** phase scripts emit NDJSON to temp files (per pod/check) -> tier completes -> audit.sh assembles per-tier results with jq -> all tiers complete -> delta.sh compares against latest.json -> generate-report.sh produces .md and -summary.json -> notify.sh dispatches -> audit.sh updates latest.json symlink.

---

### Critical Pitfalls

Research identified 10 pitfalls, all sourced from confirmed production incidents in this codebase. The top 5 by impact:

1. **cmd.exe quoting destroys remote commands** -- rc-agent `/exec` wraps commands with `cmd /C "..."`; any inner `"` truncates the command silently. Prevention: write complex commands as `.bat` files via rc-sentry `/files`, execute the bat by path. Establish `safe_remote_exec()` in lib/core.sh before any check uses the exec endpoint. Address in Phase 1.

2. **curl output includes surrounding quotes in Git Bash** -- `curl.exe` (Windows binary) sometimes returns `"ok"` instead of `ok` from command substitution; health check comparisons silently fail; confirmed production incident (pod healer flicker, 2 deploy cycles). Prevention: `http_get()` helper always uses `jq -r` and strips surrounding quotes. Address in Phase 1.

3. **SSH banner output corrupts captured results** -- post-quantum SSH warning prepends to stdout when stderr is merged; racecontrol.toml had 3 banner lines prepended, TOML parser rejected it, process guard ran with empty allowlist for 2+ hours (confirmed 2026-03-24). Prevention: `safe_ssh_capture()` adds `2>/dev/null`, validates structure before parse. Prefer HTTP endpoints over SSH wherever available. Address in Phase 1.

4. **Parallel background jobs produce interleaved output** -- 8 pods writing to shared stdout produce garbled output that no parser can interpret. Prevention: each pod writes to a dedicated temp file; file-based semaphore controls concurrency. Must be established before Phase 2 -- retrofitting after 60 checks are written is prohibitively difficult. Address in Phase 2.

5. **Auto-fix kills active billing sessions** -- `taskkill /IM powershell.exe` kills the billing session WebSocket handler along with orphan processes; an active session represents paying customer time. Prevention: `is_pod_idle()` check is the first line of every fix function; PID-targeted kills only; `taskkill /F /IM powershell.exe` banned from the safe-fix whitelist. Address in Phase 3.

Additional pitfalls by phase: delta false regressions from mode/venue-state context changes (Phase 4, include `mode` and `venue_state` in every result record), jq not found on fresh Git Bash install (Phase 1 prerequisites check), UTC/IST timestamp confusion in log-based checks (Phase 1, always `date -u +%s`), parallel load overwhelming venue LAN (Phase 2, semaphore plus 200ms stagger), suppression list masking real regressions (Phase 4, mandatory `expires` field and 10-entry cap).

---

## Implications for Roadmap

Based on combined research, the phase structure follows strict dependency ordering. Retrofitting parallel output coordination, suppression schema, or delta metadata after 60 checks are written is prohibitively expensive.

### Phase 1: Core Runner and Shared Primitives

**Rationale:** All 60 phase scripts depend on lib/core.sh. Every Windows-specific failure mode must be addressed here before any check is built on top of it. This is the highest-leverage work in the milestone -- getting these primitives right makes every subsequent phase correct by default.

**Delivers:**
- `audit/audit.sh` entry point: mode parsing, venue-open detection, auth token acquisition, prerequisites check (jq/curl/ssh), output directory initialization
- `audit/lib/core.sh`: `emit_result`, `emit_fix`, `http_get`, `safe_remote_exec`, `safe_ssh_capture`, `get_session_token`, `is_suppressed`, `pod_loop` stubs
- JSON schema for phase result records including `mode` and `venue_state` fields (required by delta tracking in Phase 4)
- Structured exit codes (0/1/2)

**Addresses:** Single-command invocation, auth token reuse, timeout enforcement, PASS/WARN/FAIL/QUIET per check, venue-open/closed detection, prerequisites validation

**Avoids:** cmd.exe quoting (Pitfall 1), curl quote artifacts (Pitfall 2), SSH banner corruption (Pitfall 3), UTC/IST timestamp confusion (Pitfall 9), jq not found (Pitfall 7)

**Research flag:** Standard patterns -- no phase research needed.

---

### Phase 2: Phase Script Migration (Tiers 1-6, Core Checks)

**Rationale:** Port the 60 v3.0 phases as non-interactive bash functions before adding parallelism. Validate each tier against a live fleet in sequential mode first. Tiers 1-6 cover the daily operations critical path (infrastructure, core services, display/UX, billing, games/hardware, notifications).

**Delivers:**
- `audit/phases/tier1/` through `audit/phases/tier6/` with all phase scripts
- Each script: sources lib/core.sh, calls `emit_result`, exits 0 always, preserves v3.0 phase numbering
- Sequential execution baseline confirming correctness before parallelism is introduced

**Addresses:** All 60 v3.0 phases non-interactive, JSON output file, Markdown report generation

**Avoids:** Parallel output interleaving (Pitfall 4) -- sequential execution first

**Research flag:** Tier 3 (display/UX) and Tier 5 (games/hardware) QUIET detection needs live venue testing. Display checks are invisible to API health endpoints -- verify QUIET logic works correctly when venue is closed.

---

### Phase 3: Parallel Execution Engine and Tiers 7-12

**Rationale:** Parallelism added after all phase scripts exist and work sequentially. Adding the engine to validated scripts is straightforward; debugging parallel scripts from scratch is not. Tiers 7-12 cover cloud/PWA, security, data/analytics, AI/feature flags, marketing/staff, and OTA/deployment.

**Delivers:**
- `audit/lib/parallel.sh` with file-based semaphore enforcing 4-connection cap
- Pod-loop helper in lib/core.sh using semaphore
- `audit/phases/tier7/` through `audit/phases/tier12/` with remaining phase scripts
- Audit runtime reduced from ~24 minutes to ~6 minutes
- 200ms stagger between parallel launches to prevent ARP flood on venue LAN

**Addresses:** Parallel phase execution capped at 4 concurrent, full 60-phase coverage

**Avoids:** Parallel output interleaving (Pitfall 4), parallel load overwhelming network (Pitfall 10)

**Research flag:** Tier 12 (OTA/deployment) -- review AUDIT-PROTOCOL.md Phases 56-60 before porting to confirm check commands are fully specified.

---

### Phase 4: Intelligence Layer (Delta, Suppression, Severity, Reports)

**Rationale:** Delta tracking requires JSON output from prior runs. Suppression requires stable check IDs. Both require the `mode` and `venue_state` fields established in Phase 1. Build all three together since they share the same data dependencies and the report generator consumes all three.

**Delivers:**
- `audit/lib/delta.sh`: jq-based join on phase number; REGRESSION/IMPROVEMENT/PERSISTENT/NEW_ISSUE/STABLE categories; mode-aware and venue-state-aware comparison
- `audit/suppress.json` schema: check_id, reason, added, expires, owner; 10-entry cap; stale-entry (>30 days) warning; expired entries auto-unsuppressed
- `audit/generate-report.sh`: Markdown report with delta section, failures section, auto-fix log, suppressed section; summary JSON with counts and regression flag
- `audit/results/index.json`: audit run history (timestamp, mode, P1/P2/P3 counts)

**Addresses:** Delta tracking, known-issue suppression with expiry enforcement, severity scoring, Markdown report generation, audit run history

**Avoids:** Delta false regressions from context changes (Pitfall 6), suppression list masking regressions (Pitfall 8)

**Research flag:** Run at least 3 consecutive audits with mode and venue state variations to verify false-regression suppression before enabling WhatsApp alerting in Phase 5.

---

### Phase 5: Auto-Fix and Notifications

**Rationale:** Auto-fix must not be deployed until at least 2 clean audit runs confirm the PASS/FAIL results it acts on are accurate. Notifications depend on delta tracking from Phase 4 to send meaningful summaries rather than raw report dumps.

**Delivers:**
- `audit/lib/fixes.sh`: FIX-01 through FIX-07 (sentinel clearing, orphan process kills, duplicate instance kills, overlay kills)
- `is_pod_idle()` as mandatory pre-fix gate (queries fleet health `session_state` field)
- Per-fix audit log (`audit/results/autofix.log`): timestamp, pod IP, action, session state at time of fix, result
- `audit/lib/notify.sh`: comms-link relay + INBOX.md dual-channel (standing rule compliance); WhatsApp via Bono relay (P1 FAILs or regressions by default); notification failure does NOT abort audit run
- Phase-level retry (1 retry, 5s delay) for transient curl failures

**Addresses:** Safe auto-fix for pre-approved operations, comms-link Bono notification, WhatsApp Uday summary, phase-level retry

**Avoids:** Auto-fix kills active billing sessions (Pitfall 5)

**Research flag:** Simulate an active billing session before enabling auto-fix in production. Verify `is_pod_idle()` blocks all 7 fix operations with SKIP_ACTIVE_SESSION. Verify WhatsApp message does not include the auth PIN.

---

### Phase Ordering Rationale

- Phase 1 before everything: shared primitives contain all Windows pitfall mitigations; they propagate to all 60 checks automatically
- Phase 2 sequential before Phase 3 parallel: debugging sequential bash is dramatically simpler; correctness must precede performance
- Phase 4 after Phase 2: delta tracking and suppression require at least 2 completed audit runs; the JSON schema with `mode`/`venue_state` fields must be established in Phase 1 before any runs happen
- Phase 5 after Phase 4 is validated: auto-fix must not be enabled until PASS/FAIL signals are known to be accurate; notifications send meaningful content only when delta tracking is working

### Research Flags

Phases needing deeper research during planning:
- **Phase 2:** Tier 3 (display/UX) and Tier 5 (games/hardware) QUIET detection -- venue-closed heuristic needs testing against actual operating hours
- **Phase 3:** Tier 12 (OTA/deployment) -- review AUDIT-PROTOCOL.md Phases 56-60 to confirm check commands are fully specified before porting
- **Phase 4:** Delta comparison edge cases -- first run (no baseline), mode switches, venue state switches; need 3+ consecutive test runs before enabling regression alerting
- **Phase 5:** Auto-fix blast radius review -- each FIX-01 through FIX-07 needs explicit `is_pod_idle()` gate testing against a live pod with an active session

Phases with standard patterns (no phase research needed):
- **Phase 1:** bash primitives, JSON schema design, curl/jq wrappers -- well-documented patterns; direct knowledge of exec endpoints
- **Phase 3 (parallel engine):** bash background jobs with file-based semaphore -- established pattern

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All tools verified live on James's machine; jq is the only install needed; bash 5.2 `wait -n` and `declare -A` confirmed; curl/ssh versions confirmed |
| Features | HIGH | v3.0 protocol is the authoritative source for all 60 check definitions; feature list derived directly from PROJECT.md constraints and production incident history |
| Architecture | HIGH | Component boundaries derived from direct analysis of AUDIT-PROTOCOL v3.0 and existing integration points (rc-agent :8090, rc-sentry :8091, fleet health :8080); no new ports or services required |
| Pitfalls | HIGH | Every pitfall documented from a confirmed production incident in this codebase; no hypothetical risks; all have specific CLAUDE.md standing rules and LOGBOOK.md incident records |

**Overall confidence:** HIGH

### Gaps to Address

- **jq install step:** Must happen before Phase 1 begins. `winget install jqlang.jq` then verify `which jq` in Git Bash specifically (winget installs to Windows PATH, not always Git Bash PATH). Fallback: `scoop install jq`.
- **Operating hours for venue-closed detection:** Time-of-day heuristic (09:00-22:00 IST) needs confirmation against actual venue schedule. Active billing session check is more reliable but requires working SESSION token. Consider supporting an explicit `--venue-closed` flag as an override.
- **Tier 12 phase coverage:** Verify AUDIT-PROTOCOL.md Phases 56-60 (OTA/deployment) have fully specified check commands before the Phase 3 porting pass.
- **WhatsApp recipient for Uday:** project_whatsapp_phone_mapping.md assigns staff phone as 7075778180. Confirm whether Uday's audit summary goes to the staff number or a separate number before wiring notify.sh.

---

## Sources

### Primary (HIGH confidence)
- `racecontrol/AUDIT-PROTOCOL.md` (1928 lines, v3.0) -- authoritative source for all 60 phase check definitions, result recording template, and execution mode definitions
- `racecontrol/.planning/PROJECT.md` -- v23.0 milestone specification, constraints (pure bash, no daemon, max 4 concurrent pod connections, auto-fix conservatism)
- `racecontrol/CLAUDE.md` -- standing rules for cmd.exe quoting, SSH banner corruption, curl quote stripping, UTC/IST confusion, session gate, OTA sentinel awareness, JSON in Git Bash -- all derived from production incidents
- `racecontrol/LOGBOOK.md` -- incident records: pod healer curl bug (2 deploy cycles), SSH banner TOML corruption (2026-03-24), process guard empty allowlist (fleet-wide, 2+ hours)
- Live tool verification on James's machine: bash 5.2.37, curl 8.18.0, Node v22.22.0, jq NOT installed
- `comms-link/send-message.js` path verified: `C:/Users/bono/racingpoint/comms-link/send-message.js`

### Secondary (MEDIUM confidence)
- jq 1.8.1 release notes (github.com/jqlang/jq/releases) -- version features and CVE-2025-49014 fix confirmed
- bash 5.2 `wait -n` documentation -- parallel job semaphore patterns
- WebSearch: parallel bash patterns, jq on Windows, delta tracking approaches -- corroborated by live tool verification

### Tertiary (informational)
- `PITFALLS-v22.0.md` and `PITFALLS-v17.1.md` -- cmd.exe hostility and spawn/recovery patterns applicable to auto-fix phase design
- Comparison to generic monitoring tools (Nagios/Zabbix) -- confirms HTTP-exec-based approach is correct for this infrastructure; no new agents needed

---
*Research completed: 2026-03-25 IST*
*Ready for roadmap: yes*
