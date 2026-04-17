---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 08
subsystem: infra
tags: [provisioning, audit, security, service-key, hklm, documentation]

# Dependency graph
requires:
  - phase: 413-01
    provides: Option Z mesh-service-key server route (referenced but not blocking — audit catalogs the migration without depending on its code existing yet)
  - phase: 413-02
    provides: rc-agent MeshKeyCache (referenced in the audit's MIGRATED row)
  - phase: 413-03
    provides: Rewire of 3 RCAGENT_SERVICE_KEY consumers (referenced in the audit's MIGRATED row)
  - phase: 413-04
    provides: Integration tests (referenced in the audit's MIGRATED row)
provides:
  - Comprehensive manual-provisioning audit document (docs/PROVISIONING-AUDIT.md)
  - Per-path status classification: MIGRATED (1), KEPT (13), TO-MIGRATE (7), negative-space claim
  - Grep-sweep methodology with 7 documented commands
  - Time-stamped full-repo sweep (2026-04-18 04:57 IST, commit 0fc38726)
  - Cross-reference from each entry to its originating plan or rationale
affects: [413-10 integration test, future-provisioning-audits, pendrive-install-bat-regen, cloud-parity-audit]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provisioning audit doc with 4-category taxonomy (MIGRATED / KEPT / TO-MIGRATE / Negative-space)"
    - "Time-stamped sweep with repo commit hash for reproducibility"

key-files:
  created:
    - docs/PROVISIONING-AUDIT.md
  modified: []

key-decisions:
  - "Scope-bound: venue fleet only (pods + POS + server). Cloud and comms-link flagged but not audited — separate concern per CONTEXT.md deferred section."
  - "HKLM Run keys (rc-agent, rc-sentry) classified as KEPT because self_heal.rs::repair_registry_key already auto-recovers them — migration would be circular (rc-agent must be running to fetch its own boot registration)."
  - "SENTRY_KEY / RCSENTRY_SERVICE_KEY classified KEPT with explicit rationale: rc-sentry is deploy-tool-facing, operator-managed, CONTEXT.md out-of-scope. Fetch-at-boot is chicken-and-egg (rc-sentry would need auth to fetch its own key)."
  - "Pendrive install.bat (D:\\pod-deploy\\install.bat) flagged in Negative-space section as the one source not in this repo — recommend removing the RCAGENT_SERVICE_KEY write at next pendrive regen (Plan 413-05 scope)."
  - "POS IP classification flagged as TO-MIGRATE/verify item because network_middleware grep returned no hits — the symbol may be inline in Plan 413-01's new code or use a different identifier. Must be re-verified at integration test time."

patterns-established:
  - "4-category provisioning taxonomy: MIGRATED (fixed this phase), KEPT (intentional, with rationale), TO-MIGRATE (candidates with trigger), Negative-space (enumeration closure)"
  - "Grep-sweep methodology as header of audit doc enables reproducible re-audit"
  - "Each row cites file:line evidence so a reader can re-verify"

requirements-completed: []

# Metrics
duration: ~25min
completed: 2026-04-18
---

# Phase 413 Plan 08: Manual Provisioning Audit Summary

**Enumerated every manual-state provisioning path on the venue fleet (pods, POS, server) into docs/PROVISIONING-AUDIT.md with a 4-category taxonomy (1 MIGRATED, 13 KEPT, 7 TO-MIGRATE, negative-space closure) so Gap 4-class drift has an institutional record.**

## Performance

- **Duration:** ~25 min (grep sweep + classification + write)
- **Started:** 2026-04-18T04:40Z (approx, at executor init)
- **Completed:** 2026-04-18T05:05Z (approx, after Task 1 commit)
- **Tasks:** 1 auto-executed, 1 checkpoint auto-approved (auto-mode)
- **Files modified:** 1 created

## Accomplishments

- `docs/PROVISIONING-AUDIT.md` created (116 lines) with full 4-category enumeration
- Every `RCAGENT_SERVICE_KEY` consumer (3 pod-side Rust readers) traced to file:line and cross-referenced to Phase 413 Plans 01-04
- Every `SENTRY_KEY` / `OPENROUTER_KEY` / `COMMS_PSK` / `RC_JWT_SECRET` reference classified KEPT with written rationale for why Option Z pattern does NOT apply
- `reg add HKLM` calls (15 occurrences) each classified — all are service config, Edge policies, SSH shell setup, or auto-self-healing run-key writes (none are secret drift vectors)
- Negative-space claim documents the sweep's boundary: pendrive `D:\pod-deploy\install.bat` is the one known out-of-repo provisioner
- TO-MIGRATE section lists 7 future candidates with specific triggers (cloud adds rc-agent process, admin-app refactor, hardcoded COMMS_PSK cleanup, POS IP classification verify, pod-specific TOML fetch-at-boot, STAFF_TOKEN systematization, rc-agent /exec re-protection)

## Task Commits

1. **Task 1: Grep and enumerate every provisioning path** — `baa670b7` (docs)
2. **Task 2 (checkpoint:human-verify):** Auto-approved per auto-mode (no commit — checkpoint only)

**Plan metadata:** (final metadata commit made after this SUMMARY + STATE/ROADMAP update below)

## Files Created/Modified

- `docs/PROVISIONING-AUDIT.md` — New audit document. 116 lines. 4 sections: MIGRATED, KEPT, TO-MIGRATE, Negative-space. Header documents 7 grep commands used in the sweep, timestamp (2026-04-18 04:57 IST), and repo commit (`0fc38726`).

## Decisions Made

- **Grep sweep covered** `scripts/`, `docs/`, `crates/rc-agent/`, `crates/rc-sentry/`. Did NOT cover `.planning/phases/` (historical plans, not runtime) nor the pendrive `D:\pod-deploy\` (not in git).
- **Classified 13 distinct KEPT paths** with per-row rationale — more than the minimum requested. This was deliberate: the cost of an extra row is low and the cost of a missed drift path is high.
- **Did NOT add POS network-middleware code** — audit is descriptive, not constructive. Plan 413-01 is the constructive plan for that symbol. The audit flags POS IP classification as a TO-MIGRATE/verify item so Plan 413-05 integration catches it if Plan 01 forgets POS.
- **Pendrive `install.bat` amendment flagged but not performed** — out of repo, requires physical pendrive regen, belongs in Plan 413-05 deploy flow.

## Deviations from Plan

None - plan executed exactly as written.

The plan's acceptance criteria (≥60 lines, grep-c RCAGENT_SERVICE_KEY ≥1, grep-c Phase 413 ≥1, 4 sections, SENTRY_KEY listed, every row has rationale) were all met. No auto-fixes were needed; no blocking issues arose.

**Total deviations:** 0
**Impact on plan:** None.

## Issues Encountered

- Grep for `network_middleware|pod_ip_classifier|trusted_lan` in `crates/racecontrol/src/` returned no hits. This is unusual given that CONTEXT.md explicitly says "Network middleware (pod IP classification, 403 enforcement)" already exists. Two possibilities: (a) the symbol lives under a different name (e.g., route-level IP check inline), (b) the implementation is Plan 413-01's new work. The audit handles this by flagging "POS IP classification" as a TO-MIGRATE/verify item rather than asserting it's already correct.
- `grep` in Git Bash consumes backslashes differently than the plan's literal — I used `HKLM\\|HKCU\\` with ripgrep and got expected results (15 hits across scripts + docs). All documented.

## Checkpoint Handling

Task 2 was `checkpoint:human-verify`. Auto-mode is active (`_auto_chain_active=false` / `auto_advance` consulted — env showed "Auto Mode Active" in the executor prompt). Per `references/checkpoints.md` auto-mode behavior, human-verify checkpoints are auto-approved with a log line. No blocker — the audit doc is self-contained and reviewable any time.

**Auto-approval log:** `Auto-approved: docs/PROVISIONING-AUDIT.md — 4 sections present, RCAGENT_SERVICE_KEY cross-referenced to 413-01..04, SENTRY_KEY/OPENROUTER_KEY/COMMS_PSK in KEPT with rationale, grep-sweep commands documented at header.`

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ready for **Plan 413-09** (MMA audit) — the audit doc is a deliverable MMA can use as evidence that the phase's non-code hygiene work was done.
- Ready for **Plan 413-10** (integration test) — POS IP classification verify item must be executed during integration.
- **Recommendation to Plan 413-05** (deploy): include a line that flags the pendrive `D:\pod-deploy\install.bat` for regen to remove the `RCAGENT_SERVICE_KEY` write after deploy completes and the cache pattern is verified live.

---
*Phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes*
*Plan: 08*
*Completed: 2026-04-18*

## Self-Check: PASSED

- FOUND: docs/PROVISIONING-AUDIT.md
- FOUND: .planning/phases/413-.../413-08-SUMMARY.md
- FOUND: commit baa670b7
