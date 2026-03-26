# Project Research Summary

**Project:** v26.0 Autonomous Bug Detection & Self-Healing
**Domain:** Scheduled autonomous fleet monitoring, config drift detection, log anomaly detection, self-healing escalation
**Researched:** 2026-03-26
**Confidence:** HIGH

## Executive Summary

v26.0 is an operational extension to a mature Bash + jq fleet management system — not a greenfield build. The foundation is already in production: `auto-detect.sh` (6-step pipeline), `bono-auto-detect.sh` (Bono-side failover), the 60-phase audit framework, 8-library audit toolchain, and comms-link relay v18.0. The milestone's job is to convert these manually-triggered scripts into a truly autonomous system by adding scheduling, expanding the detection surface (config drift, log anomalies, cascade verification), hardening the escalation ladder, and ensuring the two AIs do not race when they both act autonomously. The recommended approach is pure Bash + jq throughout — zero new compiled dependencies — using Windows Task Scheduler on James and system cron on Bono. All required tools are already installed on both machines.

The key risk is not implementation complexity — it is safety and coordination. Three failure modes dominate: (1) recovery systems fighting each other (auto-detect, rc-sentry, pod_monitor, WoL, self_monitor all see "pod down" and all act independently, creating restart loops), (2) alert fatigue destroying trust (Uday receiving nightly WhatsApp digests for non-critical drift and eventually ignoring all messages including critical ones), and (3) the dual-AI race condition where James and Bono both initiate fixes on the same pod concurrently and corrupt the binary. All three are documented past incidents, not hypothetical risks.

The recommended build order is safety-first: Phase 1 must ship the sentinel-aware idle gate, escalation cooldown, MAINTENANCE_MODE awareness, and idempotent run guard before any scheduled execution fires on live infrastructure. Phase 2 adds detection depth (config drift, log anomaly, cascade expansion). Phase 3 locks in Bono coordination so autonomous failover is safe. Phase 4 introduces the self-test suite to verify the detectors work. Only after all four phases are verified should the WoL auto-healing tier be enabled. Cascade dry-run must be the permanent default for any fix action that touches pods — apply mode requires an explicit flag.

## Key Findings

### Recommended Stack

The stack constraint is hard and non-negotiable: Bash + jq only, no compiled dependencies. This is already validated across 60 production audit phases. Every new capability needed for v26.0 is expressible with tools already present on James and Bono — jq for JSON parsing, curl for HTTP checks, grep/findstr for log pattern matching, certutil for Windows file checksums, md5sum on Linux, git diff for build drift, and mktemp for safe JSON-to-curl pipelines (required by standing rule).

**Core technologies:**
- Bash 5.x (Git Bash on Windows, native on VPS): all scripts — consistent with existing 60-phase audit framework, `set -euo pipefail` discipline established
- jq 1.6+: JSON parsing for fleet health, log fields, config comparison — `jq -r`, `jq -e`, `--argjson` patterns proven across codebase
- curl 7.x: all HTTP checks — `--max-time` for total timeout, `-d @file` (never inline JSON, standing rule violation)
- Windows Task Scheduler: James-side scheduling — same mechanism as `CommsLink-DaemonWatchdog`, survives reboots without a parent process
- System cron (Bono VPS): Bono-side scheduling — already active, needs schedule correction to 21:00 UTC (= 02:30 IST)
- certutil -hashfile (Windows): bat file checksum on pods without admin — safe for non-interactive exec context

**Version requirements:**
- jq 1.6+ required for `--argjson` flag; bash 4+ for `declare -A` associative arrays (Git Bash ships 5.x)
- No Python, no Node additions, no Go, no new npm packages

### Expected Features

The gap between "already built" and "to be built" defines the entire scope. The foundation scripts exist and are committed but are not yet scheduled — every table stakes feature flows from adding that schedule.

**Must have (table stakes — core autonomy):**
- Scheduled execution (James Task Scheduler daily at 02:30 IST, Bono cron daily at 02:35 IST) — nothing is autonomous without it
- Idempotent run guard (flock on `/tmp/auto-detect.lock`) — prevents concurrent run corruption from cron + manual trigger
- Escalation cooldown tracking (per-pod, per-alert-type) — prevents WhatsApp spam to Uday; required before first scheduled run
- Config drift detection: TOML key validation (ws_connect_timeout, app_health URLs, process_guard.enabled) — these three fields caused documented incidents
- Config drift detection: bat file hash check (certutil on pods vs md5sum on repo canonical) — single biggest regression cause
- Log anomaly scanning: ERROR/PANIC rate on last 200 lines per pod — catches crash storms that health endpoints miss
- Crash loop detection: MAINTENANCE_MODE sentinel existence check — already queryable via fleet health API
- Sentinel-aware idle gate: read OTA_DEPLOYING, MAINTENANCE_MODE, GRACEFUL_RELAUNCH before any fix action

**Should have (detection depth):**
- WoL escalation tier — recovery without human for late-night offline pods (after manual WoL test confirms it works)
- Log anomaly: crash loop pattern in JSONL timestamps (3x "restarting self" in 10 min)
- Process guard violation rate check (>50 violations/24h = empty allowlist investigation)
- Suppression confidence scoring (flag suppress.json entries older than 30 days)

**Defer (v2+):**
- Feature flag sync validation — requires new Rust endpoint on rc-agent, rebuild, fleet deploy
- Schema sync validation (cloud-venue DB column parity) — useful but needs careful output parsing
- Env var validation for Next.js apps — trivial check but file path predictability needs verification
- Pipeline self-test suite — build after detection feature set is stable

**Anti-features (do not build):**
- Autonomous binary deployment — 7-step sequence with rollback dependency; flag drift and page James/Uday instead
- LLM-gated fix decisions — adds latency and external dependency to critical detection path; whitelist is more reliable at 2:30 AM
- Real-time anomaly streaming — over-engineered for a nightly batch job
- Pod-level log streaming to central aggregator — significant infrastructure cost; point-in-time reads are sufficient

### Architecture Approach

The architecture is source-module composition around the existing `auto-detect.sh` pipeline. New detection modules (`cascade.sh`, `standing-rules-check.sh`, `log-anomaly.sh`, `config-drift.sh`) are bash scripts that define functions and are `source`d into `auto-detect.sh` — not spawned as subprocesses. This preserves shared state (STEP_RESULTS, BUGS_FOUND, LOG_FILE, RESULT_DIR) without IPC overhead. The Bono failover pattern is unchanged: check James relay first, delegate if alive, run independent checks if down. Staggering the schedules by 5 minutes (James 02:30, Bono 02:35) eliminates the scheduler race where both fire simultaneously and Bono delegates to a James that has not yet finished.

**Major components:**
1. `scripts/auto-detect.sh` — 6-step pipeline orchestrator (exists, needs scheduling + new sourced modules)
2. `scripts/cascade.sh` — NEW: build drift, pod binary consistency, cloud-venue sync, bat file spot-check
3. `scripts/standing-rules-check.sh` — NEW: unpushed commits, relay health, bat sync canary (pod 8 only)
4. `scripts/log-anomaly.sh` — NEW: fleet exec log tail + ERROR/PANIC pattern count per pod
5. `scripts/config-drift.sh` — NEW: fetch config from pods via API, compare against canonical-config.json
6. `audit/lib/fixes.sh` — EXTEND: add `reset_config_drift`, `git_auto_pull`, `sync_bat_files` to APPROVED_FIXES
7. Task Scheduler entry (James) + cron (Bono) — NEW: register-auto-detect.bat + cron schedule correction
8. `scripts/bono-auto-detect.sh` — UPDATE: add AUTO_DETECT_ACTIVE coordination check

**Key integration points:**
- Config drift requires racecontrol API endpoint (`GET /api/v1/config/health-params`) — the only upstream Rust dependency; if deferred, use SCP with first-line TOML validation as fallback
- WhatsApp alerts route through Bono VPS Evolution API (standing rule: never via venue tunnel)
- All new auto-fix actions require both `_is_approved_fix()` gate AND per-pod `is_pod_idle()` gate — both must pass, neither substitutes for the other

### Critical Pitfalls

1. **Recovery systems fighting each other (infinite restart loop)** — auto-detect joins self_monitor, rc-sentry, pod_monitor, and WoL as the fifth independent recovery actor. Without shared state, all five can act on the same pod simultaneously. Prevention: read OTA_DEPLOYING, MAINTENANCE_MODE, GRACEFUL_RELAUNCH sentinels before any fix; write AUTO_DETECT_ACTIVE lock file that other actors check; RecoveryIntentStore for actor coordination. Address in Phase 1.

2. **Alert fatigue destroying trust** — 6 QUIET findings appear in every v23.0 audit run as baseline. If escalation fires for any unfixed finding, Uday receives nightly messages about venue-closed items and eventually ignores all messages including critical ones. Prevention: WhatsApp only for (a) attempted fix that failed, (b) CRITICAL with no available fix, (c) 3+ pods affected simultaneously; silent on clean runs; digest format; cooldown per pod per alert type. Address in Phase 1.

3. **Dual-AI race condition corrupting binaries** — James (02:30) and Bono (same time without stagger) both detect build drift, both trigger deploy chains, two concurrent writes corrupt `rc-agent-new.exe`. Prevention: stagger schedules by 5 minutes; global mutex via comms-link relay before any fix; AUTO_DETECT_ACTIVE sentinel with agent identity; Bono failover requires confirmed Tailscale offline, not just relay timeout. Address in Phase 3.

4. **Config drift false positives from intentional per-pod differences** — pod_number, ip, mac are legitimately different between pods; OTA canary pods intentionally run a different binary version. Flagging these creates 8 findings per run and trains the team to ignore all drift alerts. Prevention: define shared-key schema vs per-pod-key schema; exclude Pod 8 canary from binary version homogenization checks; OTA_DEPLOYING sentinel skips binary drift for that pod. Address in Phase 2.

5. **SSH output corrupting config files** — documented incident 2026-03-24: SSH banner prepended to racecontrol.toml, TOML parser rejected from line 1, load_or_default() fell back to empty config, process guard ran with 0 entries for 2+ hours with no visible error. Prevention: config fixes use API endpoint only, never SSH exec redirect; validate first line after any config write (`head -1 | grep '^\['`); add `ssh_exec_redirect` to the prohibited fix types list. Address in Phase 2.

## Implications for Roadmap

Based on combined research, the dependency chain is clear: scheduling enables everything, but safety gates must precede scheduling. Suggested 4-phase structure:

### Phase 1: Safe Scheduling Foundation
**Rationale:** The pipeline must be safe before it runs autonomously. Idle gate, sentinel awareness, MAINTENANCE_MODE handling, alert cooldown, and run guard must all be correct from the first scheduled execution — there is no "add safety later" option when the system runs at 2:30 AM without a human present.
**Delivers:** Registered Task Scheduler entry (James, 02:30 IST) + corrected cron (Bono, 02:35 IST); idempotent run guard; per-pod billing session check (not aggregate); sentinel reads (OTA_DEPLOYING, MAINTENANCE_MODE, GRACEFUL_RELAUNCH) gating all fix actions; MAINTENANCE_MODE awareness in auto-fix engine (wait for v17.1 auto-clear before retry); escalation cooldown (per-pod, per-alert-type); WhatsApp silence on clean runs; AUTO_DETECT_ACTIVE lock file; notification wired to audit/lib/notify.sh
**Addresses features:** Scheduled execution (James + Bono), idempotent run guard, escalation cooldown tracking, crash loop detection (MAINTENANCE_MODE existence check)
**Avoids:** Recovery systems fighting (Pitfall 1), alert fatigue (Pitfall 2), scheduled task conflicts (Pitfall 6), MAINTENANCE_MODE permanent blocker (Pitfall 8), process guard empty allowlist window (Pitfall 9)
**Research flag:** Standard patterns — all components exist, work is wiring and registration

### Phase 2: Detection Expansion
**Rationale:** With safe scheduling in place, add the detection surface that covers historical incident patterns. Config drift, log anomaly, and cascade expansion each trace to documented incidents. Cascade dry-run must be the default from first implementation — there is no "add dry-run later."
**Delivers:** `scripts/cascade.sh` (build drift, pod consistency, cloud-venue sync, bat file spot-check on pod 8); `scripts/log-anomaly.sh` (ERROR/PANIC rate per pod via fleet exec); `scripts/config-drift.sh` (TOML key validation via API or SCP fallback); `scripts/standing-rules-check.sh` (unpushed commits, relay health); extended APPROVED_FIXES whitelist (reset_config_drift, git_auto_pull, sync_bat_files); per-run result persistence (timestamped result dirs under audit/results/)
**Uses:** certutil + md5sum for bat file checksums; jq for log field extraction; grep -cE for anomaly rate counts; safe_remote_exec from audit/lib/core.sh
**Implements:** Architecture Pattern 4 (config drift via API, not SSH) and Pattern 5 (log aggregation on-demand)
**Avoids:** Config drift false positives (Pitfall 3, per-pod key schema), log anomaly miscalibration (Pitfall 4, pattern-based triggers), cascade wrong target (Pitfall 5, dry-run default), SSH banner config corruption (Pitfall 10, API-only fix path)
**Research flag:** Needs attention on config-drift.sh — upstream Rust API dependency; plan endpoint or commit to SCP fallback before writing the script

### Phase 3: Bono Coordination Protocol
**Rationale:** Bono failover has been available in detect-only mode since the foundation was built. Enabling autonomous fix actions on Bono requires the coordination protocol to be verified working before the first live failover scenario. This phase formalizes the global mutex and tests simultaneous execution.
**Delivers:** AUTO_DETECT_ACTIVE sentinel coordination (James + Bono both check before starting any fix); global mutex via comms-link relay; Bono failover activation gated on confirmed Tailscale offline (not relay timeout alone); completion status written to INBOX.md so Bono skips its run if James completed within the window; simultaneous dry-run verification test confirming only one agent writes AUTO_DETECT_ACTIVE
**Avoids:** Dual-AI race condition corrupting binaries (Pitfall 7)
**Research flag:** Standard patterns — comms-link relay mutex is a well-established pattern in this codebase

### Phase 4: Pipeline Self-Test Suite
**Rationale:** An autonomous detection system that might have its own bugs is not trustworthy. Self-tests validate that each detection function correctly classifies known-good and known-bad scenarios. Build last — after the detection feature set is stable — so tests reflect real behavior.
**Delivers:** `audit/test/test-auto-detect.sh` (or Suite 5 in comms-link test/run-all.sh); mock server responses; injected anomaly fixtures (known-bad JSONL with ERROR patterns, config with stale values); verification that each detector fires on bad input and stays silent on good input; dry-run verification that fixes are proposed but not applied
**Addresses features:** Pipeline self-test suite (P3 in feature matrix)
**Research flag:** Standard patterns — follows existing comms-link test suite structure (Suites 1-4 provide the pattern)

### Phase Ordering Rationale

- Phase 1 before everything else because scheduling without safety gates exposes live infrastructure to uncoordinated fix actions on the first nightly run
- Phase 2 after Phase 1 because detection modules sourced into auto-detect.sh inherit the safety infrastructure already in place
- Phase 3 after Phase 2 because Bono coordination is only needed when both agents have fix capability — detect-only Bono failover is already safe
- Phase 4 last because tests should validate stable behavior, not planned behavior; building tests before detection functions are finalized means constant test churn
- WoL tier deferred beyond Phase 4 — requires manual test on at least 2 pods before autonomous activation; its absence does not block nightly detection

### Research Flags

Phases needing closer attention during planning:
- **Phase 2 (config-drift.sh):** Upstream dependency on racecontrol `GET /api/v1/config/health-params` endpoint. Requires Rust change, rebuild, and fleet deploy. If deferred, SCP fallback must validate TOML first line to prevent SSH banner corruption pattern. Decide which path before writing config-drift.sh.
- **Phase 2 (log-anomaly.sh thresholds):** Rate-based thresholds require 7 days of silent observation baseline. For Phase 2 launch, use pattern-based triggers only (specific strings from known incidents: "MAINTENANCE_MODE written", "empty allowlist loaded", "violation_count spiking"). Plan when to run the 7-day calibration window.

Phases with standard patterns (skip deeper research):
- **Phase 1:** All components exist (Task Scheduler, cron, flock, sentinel reads, notify.sh) — pure wiring and registration
- **Phase 3:** comms-link relay coordination is a well-established pattern; AUTO_DETECT_ACTIVE sentinel follows existing sentinel patterns throughout the codebase
- **Phase 4:** Follows existing comms-link test suite structure

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All technologies already in production across 60 audit phases; direct codebase inspection; no external research needed |
| Features | HIGH | Every feature traces to a documented past incident; gap analysis based on direct auto-detect.sh code inspection |
| Architecture | HIGH | Direct analysis of all existing scripts and audit framework; source-module composition pattern already established |
| Pitfalls | HIGH | All 10 pitfalls are documented past incidents from CLAUDE.md standing rules, MEMORY.md, and PROJECT.md — no hypothetical risks |

**Overall confidence: HIGH**

### Gaps to Address

- **racecontrol config API endpoint:** `GET /api/v1/config/health-params` does not exist yet. Phase 2 planning must decide: build Rust endpoint first (requires rebuild + deploy to server), or use SCP fallback. SCP is safer to start with given the SSH banner standing rule — validate TOML first line after every SCP copy (`head -1 | grep '^\['`).
- **Bono cron schedule correctness:** Current cron comment shows `0 2 * * *` (2:00 AM UTC = 7:30 AM IST) but target is 02:35 IST (21:05 UTC). Verify live crontab before Phase 1 work: `ssh root@100.70.177.44 "crontab -l | grep auto-detect"`. Correct as part of Phase 1 if wrong.
- **WoL manual verification prerequisite:** WoL escalation tier requires confirming it works on at least 2 pods before autonomous activation. This is a physical test that must happen before the WoL tier is added to the fix whitelist. Not blocking Phases 1-4 but must be scheduled as a prerequisite for post-Phase-4 WoL work.
- **Log anomaly baseline:** Rate-based thresholds require 7 days of silent observation. For Phase 2 launch, use pattern-based triggers only. Plan the calibration window explicitly — do not leave it as "we'll add rate thresholds later" without a date.

## Sources

### Primary (HIGH confidence)
- `scripts/auto-detect.sh` (commit b54e4585) — direct code inspection: 6-step pipeline, all gaps identified
- `scripts/bono-auto-detect.sh` — direct code inspection: Bono failover logic, cron schedule state
- `audit/lib/` (fixes.sh, notify.sh, core.sh, delta.sh, suppress.sh, parallel.sh, results.sh, report.sh) — direct inspection: all existing primitives
- `audit/audit.sh` + `audit/phases/tier*/` — 60-phase framework, parallel engine, result structure
- `CLAUDE.md` (racecontrol) — standing rules: SSH piping hazard, idle gate, OTA sentinel protocol, MAINTENANCE_MODE permanent blocker, process guard empty allowlist incident, certutil pattern, bat parentheses, cmd.exe quoting
- `MEMORY.md` — shipped milestones context, pod MAC addresses, WoL prerequisites, documented incidents
- `.planning/PROJECT.md` — v26.0 milestone spec, foundation already built, constraints
- `comms-link/chains.json` — chain template structure, auto-detect-bono and sync-and-verify templates
- `scripts/AUTONOMOUS-DETECTION.md` — architecture decisions for foundation scripts

### Secondary (MEDIUM confidence)
- STACK.md alternatives analysis — "when to use alternative" column informs anti-feature decisions
- FEATURES.md incident-to-feature mapping — each feature directly traced to a specific past incident with timestamps and impact

---
*Research completed: 2026-03-26 IST*
*Ready for roadmap: yes*
