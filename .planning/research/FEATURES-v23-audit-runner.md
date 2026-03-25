# Feature Research

**Domain:** Automated infrastructure audit runner / fleet health check system
**Project:** v23.0 — Audit Protocol v4.0
**Researched:** 2026-03-25
**Confidence:** HIGH (based on existing v3.0 manual protocol, PROJECT.md constraints, and direct infrastructure knowledge)

---

## Context: What Already Exists (v3.0 Manual Protocol)

The existing AUDIT-PROTOCOL.md is a 1928-line document of copy-paste bash commands covering 60 phases across 11 tiers. It already defines:

- Check logic for every domain: fleet inventory, config integrity, network, firewall, processes, sentinels, API integrity, WebSocket flows, display/UX, billing, games/hardware, notifications, cloud sync, DB schema, code quality, E2E journeys, cross-system chains
- 5 execution modes: Quick (Tiers 1-2), Standard (Tiers 1-11), Full (all 60), Pre-Ship, Post-Incident
- Result recording template with PASS/WARN/FAIL/QUIET statuses and P1-P3 severity levels
- Known-issue patterns documented in standing rules

The gap is entirely in **execution mechanics**, not in check coverage. The audit runner does not need to invent new checks — it needs to execute the existing 60 phases non-interactively and make results structured, comparable, and actionable.

---

## Feature Landscape

### Table Stakes (Execution Without These = Still Manual)

Features that make automation meaningful. Without each one, a human still has to manually operate the tool.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Single-command invocation | The entire value proposition — `bash audit.sh --mode quick` replaces copy-paste. If it requires any interactive input, it is not automated. | LOW | Must accept `--mode quick\|standard\|full\|pre-ship\|post-incident` |
| Structured exit codes | Caller (CI, cron, chain) must know if the audit found failures without parsing output. Exit 0 = clean, 1 = WARN, 2 = FAIL. | LOW | Without this, callers cannot gate on results |
| Per-check PASS/WARN/FAIL/QUIET status | The existing protocol already uses these four statuses. The runner must emit them in machine-readable form (JSON). QUIET means the check was skipped because the venue is closed or the target is offline for a known reason — not a failure. | LOW | QUIET is distinct from FAIL — critical for venue-closed state |
| JSON output file | Human summary plus machine-parseable results in one pass. Both formats from the same run, not separate invocations. | LOW | Output path: `audit-results/YYYY-MM-DD-HH-MM-<mode>.json` |
| Markdown report generation | Uday reads reports. The JSON is for machines; the markdown is for humans. Must be generated automatically alongside JSON, not as a separate step. | LOW | Output path: `audit-results/YYYY-MM-DD-HH-MM-<mode>.md` |
| Venue-open/closed detection | Hardware checks (display, lock screen, Edge kiosk, FFB wheelbase, AC server) must be skipped with QUIET status when venue is closed. A closed-venue FAIL on "AC server not running" is noise that trains operators to ignore results. | MEDIUM | Detect via: active billing sessions OR time-of-day OR explicit `--venue-closed` flag |
| Non-interactive pod queries | All 60 phases contain for-loops over $PODS. The runner must execute these in sequence or parallel without any interactive input, timeout gracefully per pod, and not hang on an offline pod. | MEDIUM | Per-pod timeout: 10s. Offline pods = QUIET, not hanging |
| Timeout enforcement | Every curl/ssh/exec call in the existing protocol can hang indefinitely. The runner must wrap all network calls with a timeout. A single unresponsive pod must not block the entire audit. | MEDIUM | Global: 30s per phase. Per-call: 10s. Parallel pod queries. |
| Auth token acquisition and reuse | The existing protocol obtains a SESSION token once and reuses it across all phases. The runner must do this automatically at startup, fail cleanly if auth fails, and pass the token to all phases that require it. | LOW | PIN stored in audit config, not hardcoded in script |

### Differentiators (What Manual Audit Cannot Do)

Features that give the automated runner capability that is fundamentally unavailable to copy-paste execution.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Delta tracking against previous run | The single most useful feature for ongoing operations. "Pod 3 added 47 new violations since last audit" is actionable. "Pod 3 has 100 violations" requires context to interpret. Store last-N results and diff on every run. | MEDIUM | Compare same check IDs across runs. Flag: new FAILs (regressions), resolved FAILs (improvements), new WARNs. |
| Parallel phase execution within a tier | The existing protocol is purely sequential. 8 pods x 60 phases x ~3s per call is approximately 24 minutes. Parallelizing pod queries within each phase (max 4 concurrent per standing rule) cuts this to approximately 6 minutes. | MEDIUM | Use bash background jobs with `wait`. Cap at 4 concurrent pod connections. |
| Known-issue suppression list | The audit will always find the same known-open issues (blanking screen on Pod 8, VT-x blocker, etc.). If these are always FAIL, operators stop reading the report. A JSON suppression file allows tagging known issues so they render as KNOWN-ISSUE rather than FAIL, without hiding them. | LOW | File: `audit-known-issues.json`. Each entry: check_id, pattern, reason, expiry date. |
| Safe auto-fix for pre-approved actions | A small set of fixes are safe enough to apply automatically during audit: clear stale sentinel files, kill orphan Variable_dump.exe processes, kill excess PowerShell processes (more than 1). These currently require a human to read the result and manually execute the fix. | HIGH | Auto-fix whitelist must be conservative and version-controlled. Each fix must log what it did. Never auto-fix anything that affects billing sessions. |
| Severity scoring per phase | Not all failures are equal. P1 failures (billing broken, pods offline, MAINTENANCE_MODE active) require immediate action. P3 failures (Tailscale node stale, log rotation needed) can wait. The runner must emit a severity score per check and an aggregate severity for the entire audit run. | LOW | P1 = immediate action required. P2 = fix before next deploy. P3 = fix before next milestone. |
| Comms-link notification to Bono | After every audit, push results summary to Bono via comms-link WS + INBOX.md. Bono can then act on cloud-side findings (Bono VPS health, cloud sync, pm2 services) without James having to manually relay. | LOW | Existing comms-link relay handles this. Message format: audit mode, run timestamp, P1/P2/P3 counts, new FAILs since last run. |
| WhatsApp summary to Uday on completion | Uday wants operational visibility without reading technical reports. A single WhatsApp message after each audit: "Audit complete. 2 P1 issues found: [summary]. 0 regressions since last audit." | LOW | Use existing WhatsApp channel (staff phone 7075778180). Trigger only when P1 issues found OR when `--notify` flag passed. |
| Phase-level retry on transient failure | Pod queries fail transiently (pod briefly unresponsive, network blip). A single retry with 5s delay prevents false FAILs from transient connectivity. The manual protocol has no retry — operators just re-run the phase. | LOW | Retry policy: 1 retry, 5s delay. Only for curl/HTTP failures. Not for logic failures (empty allowlist = real FAIL). |
| Audit run history index | A lightweight JSON index of all past runs (timestamp, mode, P1/P2/P3 counts, FAIL count) enables trend analysis. "How many P1 issues did we have last week?" is answerable without opening individual reports. | LOW | File: `audit-results/index.json`. Append on each run. |

### Anti-Features (Explicitly Out of Scope)

Features that seem like natural extensions but conflict with the stated constraints or create more problems than they solve.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Continuous monitoring daemon | "Just leave it running and alert on failures" seems like the logical endpoint of automation. | PROJECT.md explicitly excludes this. A daemon creates a persistent process that can fail silently, consumes resources on James's machine, requires its own restart logic, and turns an on-demand audit into an always-on system that needs its own auditing. The existing watchdog infrastructure already handles continuous monitoring. | On-demand execution via cron (daily) plus triggered execution before milestones and post-incidents. |
| Node.js or Python runtime for audit logic | Richer templating, better JSON manipulation, async/await. | PROJECT.md explicitly constrains to pure bash. No new compiled or interpreted runtime dependencies. James's machine has bash via Git Bash. Adding a Node.js dependency means the audit tool needs its own dependency management. | bash plus jq (already present on James's machine for existing protocol work). |
| New check categories beyond v3.0 | "While we are here, add checks for X" scope creep. | Every new check category that is not in the existing 60-phase protocol is a new feature, not an automation of an existing one. v23.0 scope is automation of v3.0, not expansion. | Add to v3.0 protocol first, then auto-run next audit cycle. |
| Auto-fix for anything touching billing sessions | "If billing is stuck, auto-fix it" seems helpful. | A billing session represents a customer actively paying. Any auto-termination of a billing session loses revenue and requires manual reconciliation. The OTA pipeline standing rule already covers this: "Billing sessions must drain before binary swap." | Auto-fix only for process/sentinel operations. Billing issues = FAIL plus alert, no auto-fix. |
| Fleet-wide parallel execution (all pods simultaneously) | "Just run everything in parallel, it will be faster." | The standing rule explicitly caps pod connections at 4 concurrent. Hammering all 8 pods simultaneously can trigger rate limiting, overwhelm the router, and cause false connectivity failures. | Parallel within phases, capped at 4 concurrent pod connections. Sequential phase ordering preserved. |
| Auto-deploy fixes found during audit | "If a binary is stale, just auto-deploy the new one." | Binary deployment requires the full 7-step deploy sequence (build, stage, HTTP serve, download, swap, verify, confirm). Auto-deploying from within an audit script collapses this process and removes human confirmation from a destructive operation. | Audit reports stale builds as WARN/FAIL. Human initiates deploy separately. |
| Alerting on every WARN | "Send Uday every warning found." | The existing protocol generates dozens of WARNs per run from normal operational state. Alerting on all of them creates alert fatigue — Uday learns to ignore all messages. | Alert only on: P1 FAILs, new regressions (FAIL not present in last run), explicit `--notify` flag. |
| Web dashboard for audit results | "Show results in the admin panel." | The admin panel (Next.js, :3201) is a separate service. Coupling audit output to a specific web UI creates a runtime dependency — audit is useless if admin is down. Audit results need to be readable without any running service. | JSON plus Markdown files in `audit-results/` directory. Human-readable without tooling. |

---

## Feature Dependencies

```
Single-command invocation
    |
    +--requires--> Timeout enforcement
    +--requires--> Auth token acquisition and reuse
    +--requires--> Non-interactive pod queries
    +--requires--> Per-check PASS/WARN/FAIL/QUIET status
                       |
                       +--requires--> Venue-open/closed detection (for QUIET)
                       +--requires--> Structured exit codes (for callers)
                       |
                       +--enables--> JSON output file
                       |                 |
                       |                 +--enables--> Markdown report generation
                       |                 +--enables--> Delta tracking
                       |                                   |
                       |                                   +--enables--> Audit run history index
                       |                                   +--enables--> WhatsApp summary (regressions)
                       |                                   +--requires--> JSON output file
                       |
                       +--enables--> Severity scoring per phase
                                         +--enables--> Comms-link Bono notification
                                         +--enables--> WhatsApp Uday summary

Known-issue suppression list
    +--enhances--> Per-check status (FAIL -> KNOWN-ISSUE in output)
    +--requires--> JSON output file (suppression applied before rendering)

Safe auto-fix for pre-approved actions
    +--requires--> Per-check PASS/WARN/FAIL/QUIET status (know what to fix)
    +--requires--> Severity scoring (only auto-fix P1/P2)
    +--conflicts--> Anything touching billing sessions

Parallel phase execution within tiers
    +--requires--> Timeout enforcement (parallel jobs can hang)
    +--requires--> Non-interactive pod queries
```

### Dependency Notes

- **Delta tracking requires JSON output:** Cannot diff against previous run without a machine-readable previous run. JSON output is the prerequisite for every comparison feature.
- **Venue-open/closed detection enables QUIET:** Without this, hardware checks (Edge kiosk, AC server, FFB) always FAIL when the venue is closed. This makes pre-ship and daily audits unusable during non-operating hours.
- **Auto-fix conflicts with billing session touches:** The billing-session boundary is a hard constraint from PROJECT.md and standing rules. Auto-fix logic must check for active sessions before any process kill that could affect billing.
- **Parallel execution requires timeouts:** Without per-call timeouts, a single hung pod blocks the parallel job group indefinitely. The cap at 4 concurrent connections requires a counting mechanism (semaphore via bash FIFOs or simple counter).
- **Comms-link and WhatsApp require delta tracking:** Sending the full audit report over WhatsApp or comms-link is too verbose. Delta (regressions since last run) is the correct granularity for notifications.

---

## MVP Definition

### Launch With (v23.0 Phase 1 — Core Runner)

The minimum that makes the tool usable as a drop-in replacement for copy-paste execution.

- [ ] Single-command invocation with `--mode` flag — the tool runs end-to-end without human input
- [ ] All 60 phases from v3.0 ported as non-interactive bash functions
- [ ] Per-check PASS/WARN/FAIL/QUIET status with severity (P1/P2/P3)
- [ ] Venue-open/closed detection (QUIET for hardware checks when closed)
- [ ] Timeout enforcement (10s per call, 30s per phase, bail on hung pods)
- [ ] JSON output file plus Markdown report (generated on every run)
- [ ] Structured exit codes (0/1/2)
- [ ] Auth token acquisition at startup, reused across all phases

### Add After Core Works (v23.0 Phase 2 — Intelligence Layer)

Features that require the core to be working first.

- [ ] Delta tracking against previous run (requires JSON output from Phase 1)
- [ ] Known-issue suppression list (requires stable check IDs from Phase 1)
- [ ] Parallel phase execution within tiers, capped at 4 concurrent (requires per-phase functions from Phase 1)
- [ ] Audit run history index (requires JSON output from Phase 1)
- [ ] Severity scoring and aggregate audit score

### Add After Validation (v23.0 Phase 3 — Notifications and Auto-Fix)

Features that require human validation of the core results before enabling automated actions.

- [ ] Safe auto-fix for pre-approved operations (clear sentinels, kill orphan Variable_dump, kill excess PowerShell) — ONLY after at least 2 clean audit runs confirm the auto-fix logic is correct
- [ ] Comms-link notification to Bono on audit completion
- [ ] WhatsApp summary to Uday for P1 issues
- [ ] Phase-level retry (1 retry, 5s delay) for transient curl failures

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Single-command invocation | HIGH | LOW | P1 |
| All 60 phases non-interactive | HIGH | MEDIUM | P1 |
| PASS/WARN/FAIL/QUIET per check | HIGH | LOW | P1 |
| Venue-open/closed detection | HIGH | LOW | P1 |
| Timeout enforcement | HIGH | LOW | P1 |
| JSON + Markdown output | HIGH | LOW | P1 |
| Structured exit codes | HIGH | LOW | P1 |
| Auth token auto-acquisition | HIGH | LOW | P1 |
| Delta tracking | HIGH | MEDIUM | P2 |
| Parallel execution (capped 4) | MEDIUM | MEDIUM | P2 |
| Known-issue suppression | MEDIUM | LOW | P2 |
| Severity scoring | MEDIUM | LOW | P2 |
| Audit run history index | LOW | LOW | P2 |
| Safe auto-fix (sentinels, orphans) | HIGH | HIGH | P2 |
| Comms-link Bono notification | MEDIUM | LOW | P3 |
| WhatsApp Uday summary | MEDIUM | LOW | P3 |
| Phase-level retry | LOW | LOW | P3 |

**Priority key:**
- P1: Must have for launch — without these, the runner is not useful as an automation
- P2: Should have — these are what make automation better than manual execution
- P3: Nice to have — enhancements once core is stable

---

## Comparison to Generic Infrastructure Audit Tools

This is a bespoke system, not a competing product. Relevant comparisons are to existing infrastructure audit tools in the broader ecosystem, mapped to the specific constraints (pure bash, Windows targets via curl/exec, no daemon, 8-node fleet).

| Feature | Generic Tools (Nagios/Zabbix) | Shell-based Audit Scripts | Our Approach |
|---------|-------------------------------|---------------------------|--------------|
| Check execution | Daemon plus plugin model | Inline bash, no structure | bash functions per phase, invoked by runner |
| State tracking | Time-series DB (heavy) | None | JSON file per run, delta on invocation |
| Alert routing | Complex rule engine | stdout only | WhatsApp plus comms-link, P1-only threshold |
| Auto-fix | Not standard | Manual action after alert | Pre-approved whitelist, conservative scope |
| Windows targets | Agent-based (Nagios NRPE) | SSH plus WMI | curl to existing HTTP endpoints (rc-agent :8090, rc-sentry :8091) — no new agents |
| Venue-aware context | Not applicable | Not applicable | Explicit QUIET status for hardware-offline-by-design |
| Known-issue suppression | Maintenance windows (complex) | None | JSON suppression file with expiry dates |

The key differentiator from generic tools is that the existing infrastructure (rc-agent :8090 exec endpoints, rc-sentry :8091 exec endpoints, fleet health API on :8080) provides a rich HTTP-based remote execution layer. The audit runner does not need to install any agents — it queries HTTP endpoints that already exist on every machine.

---

## Sources

- `racecontrol/AUDIT-PROTOCOL.md` — existing 60-phase manual protocol, authoritative source for all check definitions (HIGH confidence — written by James, covers live infrastructure)
- `racecontrol/.planning/PROJECT.md` — v23.0 target features and constraints (HIGH confidence — authoritative project context)
- `racecontrol/CLAUDE.md` — standing rules for deploy, verification, process guard, MAINTENANCE_MODE, OTA pipeline (HIGH confidence — operational constraints derived from production incidents)
- Standing rules on auto-fix safety, billing session protection, parallel connection caps — all derived from production incident history (HIGH confidence)

---

*Feature research for: v23.0 Automated Fleet Audit System*
*Researched: 2026-03-25 IST*
