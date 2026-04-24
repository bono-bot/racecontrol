# Milestone v53.0 — Fleet Drift Graphs

**Status:** Defining
**Started:** 2026-04-24
**Owner:** James (primary) + Bono (cloud-side probes, INBOX reporting)
**Scope:** cross-repo infrastructure (racecontrol + racingpoint-admin + comms-link + bono-bot + 11 fleet targets)
**Phase range:** 447–455 (9 phases; scope locked 2026-04-24 after gap + MI + synergy analysis)

## Goal

Detect drift between "what's in git" and "what's actually running" by building two knowledge graphs — a **build graph** (source repos + docs + memory) and a **deploy graph** (fleet-wide runtime manifest probed from every deployable surface) — and diffing them.

## Core Non-Negotiable Success Criterion

**The diff tool must, on first run after this milestone ships, correctly flag the four known-drift items below as drift — without being tuned to them after the fact.** This is the binary gate between "milestone done" and "milestone not done." If any of the four is not surfaced by the diff output, the tool is broken.

## Known-Drift Ground Truth (validation targets)

These items exist in memory as confirmed code-complete-not-deployed. The diff tool must catch all four:

1. **HUD v1 / AC ephemeral SHM** (`project_racing_hud_v1_and_ac_ephemeral_shm_20260423.md`) — PR #38, Pod 8 only, Pods 1-7 + POS NOT deployed
2. **Freedom mode focus contract** (`project_freedom_mode_focus_contract_20260423.md`) — PR #33 MMA-approved 3/3, NOT deployed fleet-wide
3. **FH5 phantom haptic XInput skip** (`project_fh5_haptic_xinput_skip_20260423.md`) — commit `0f3cb05e`, NOT deployed
4. **Q5 `D:/racecontrol.toml` drift** (`project_q5_racecontrol_toml_drift_20260423.md`) — 5 categories of divergence between `D:/` proxy and git-tracked config; actual Server .23 TOML still unaudited

## Fixed Constraints

1. Probes must not require installing agents on targets — use existing SSH, rp-bono-exec relay, HTTP/JWT, Tailscale only
2. Manifest format must be human-readable JSON (not binary, not database)
3. Cannot assume Server .23 SSH is pre-audited — access may itself be part of the work (see PROBE-01)
4. No automated REMEDIATION — v53.0 detects and reports only
5. Must work when venue is offline (probes gracefully fail → "probe_failed" manifest row, not missing row)

## Requirements

### Manifest Schema (SCHEMA)

- [ ] **SCHEMA-01**: System defines a normalized per-target manifest schema with fields: `target_id`, `host`, `ip`, `role`, `probed_at_ist`, `probe_status`, `binary_sha256`, `build_id`, `config_hash`, `running_procs`, `scheduled_tasks`, `autostart_entries`, `env_vars_hash`, `last_deploy_ts`
- [ ] **SCHEMA-02**: Schema has a `schema_version` field with forward-compat unknown-field handling (schema can evolve without breaking prior manifests)
- [ ] **SCHEMA-03**: Manifest persisted as pretty-printed JSON under `state/fleet-manifest/<iso-ts>/<target_id>.json` with a `state/fleet-manifest/<iso-ts>/_meta.json` summary index

### Probe Infrastructure (PROBE)

- [ ] **PROBE-01**: Staff can probe Server .23 via SSH and capture a normalized manifest. Access audit + any gaps fixed (or documented) as part of this requirement
- [ ] **PROBE-02**: Staff can probe Pods 1-8 via existing rp-bono-exec relay + HTTP `/debug` endpoints and capture manifest per pod
- [ ] **PROBE-03**: Staff can probe POS .130 via Tailscale SSH and capture manifest (including kiosk Chrome + BillingDashboard state)
- [ ] **PROBE-04**: Staff can probe James .27 localhost and capture manifest (comms-link relay, hooks, MCP servers)
- [ ] **PROBE-05**: Staff can probe Bono VPS via comms-link relay `/relay/exec/run` and capture manifest
- [ ] **PROBE-06**: Staff can probe cloud admin (`admin.racingpoint.cloud`) via HTTP + staff JWT and capture manifest (build_id, env, gate status)
- [ ] **PROBE-07**: Staff can probe cloud racecontrol (Bono VPS :8080) and capture manifest
- [ ] **PROBE-08**: Staff can probe comms-link relay (James .27:8766 + VPS :8765) and capture manifest (uptime, queue depth, last-sync)
- [ ] **PROBE-09**: `scripts/fleet-probe/probe-all.sh` orchestrates all 8 probes in one invocation, emits one manifest directory

### Build Graph (GRAPH-B)

- [ ] **GRAPH-B-01**: Staff can build a graphify graph of source repos (racecontrol + racingpoint-admin + comms-link) → `graphify-out/graph_build.html` + `graph_build.json`
- [ ] **GRAPH-B-02**: Build graph includes memory + planning docs as nodes (inherits the existing memory graph's cross-repo communities)
- [ ] **GRAPH-B-03**: Build graph is incrementally refreshable (`--update` mode) so re-runs after code changes take < 60s

### Deploy Graph (GRAPH-D)

- [ ] **GRAPH-D-01**: Staff can build a graphify graph from the fleet manifest → `graphify-out/graph_deploy.html` + `graph_deploy.json`
- [ ] **GRAPH-D-02**: Deploy graph nodes carry target-device metadata (host, IP, role) as node attributes

### Diff Tool (DIFF)

- [ ] **DIFF-01**: Staff can diff build-graph against deploy-graph → drift report with three categories: `built_not_deployed` (in git, absent on ≥1 target), `deployed_not_in_build` (on target, not in git), `shape_mismatch` (build_id/sha256 divergence)
- [ ] **DIFF-02**: Diff tool produces both human-readable Markdown (`DRIFT-REPORT.md`) and machine-readable JSON (`drift.json`)
- [ ] **DIFF-03**: Diff tool categorizes each drift entry by severity (P0/P1/P2) based on target type + file class — e.g. racecontrol binary mismatch on Server .23 = P0; hook drift on James = P2
- [ ] **DIFF-04**: Drift severity incorporates a canary window — drift on a canary target within N hours of its last deploy timestamp is reported at reduced severity (P3/info) with explicit `canary_grace` tag. N configurable per target class; default 4h. Prevents false-positive on intentional Pod 8 canary pattern.
- [ ] **DIFF-05**: `shape_mismatch` entries include sha→commit mapping via `SWAPLOG.md` (Server .23 binary) or `state/deploy-ledger/<target_id>.jsonl` (Pods, POS, cloud) — operator sees "target has commit X, git HEAD has commit Y" not just "sha mismatch." Load-bearing for Phase 453 validation: without this, drift entries have no actionable fix pointer.

### Ground-Truth Validation (VALIDATE)

- [ ] **VALIDATE-01**: Tool flags HUD v1 (PR #38) as `built_not_deployed` on Pods 1-7 + POS
- [ ] **VALIDATE-02**: Tool flags freedom mode (PR #33) as `built_not_deployed` fleet-wide
- [ ] **VALIDATE-03**: Tool flags FH5 haptic fix (commit `0f3cb05e`) as `built_not_deployed` fleet-wide
- [ ] **VALIDATE-04**: Tool flags Q5 TOML drift — 5 config categories diverging between `D:/racecontrol.toml` and git-tracked `racecontrol.toml`

### Scheduling + Reporting (REPORT)

- [ ] **REPORT-01**: Staff can run drift audit on-demand via `bash scripts/fleet-drift/run.sh`
- [ ] **REPORT-02**: Staff can schedule daily drift audit (cron on James + schtasks on Server .23 as backup)
- [ ] **REPORT-03**: Drift report auto-posts to `comms-link/INBOX.md` + optional WhatsApp alert on P0 drift via existing comms-link relay

### Lifecycle Integration (LIFECYCLE)

- [ ] **LIFECYCLE-01**: Post-deploy re-probe hook — every `deploy-*.sh` success emits a `{commit, targets, timestamp}` event that triggers `probe-target.sh` on the affected targets and refreshes both graphs within 90s. Probe failure does not block deploy success; logs to `deploy-events.log` for retry. Solves the "deploy lands, map stale until tomorrow's cron" class.
- [ ] **LIFECYCLE-02**: Non-server deploy-ledger — `state/deploy-ledger/<target_id>.jsonl` appended on every pod/POS/cloud deploy with `{timestamp, commit, sha256, deploy_script, triggered_by, reason}`. Mirrors `SWAPLOG.md`'s pattern for Server .23. Consumed by DIFF-05.

### Mesh Intelligence Integration (MI)

- [ ] **MI-01**: Diff tool emits `drift-seeds.json` and auto-POSTs every entry to `/api/v1/mesh/audit-seed` with schema `{problem_key, severity, symptom_patterns[], root_cause, fix_action, fix_status, affects[]}`. MI Tier 0 short-circuits known drift instead of running expensive Ollama diagnosis cycles.
- [ ] **MI-02**: MI context-builder includes `state/fleet-manifest/<latest>/<target_id>.json` and the relevant drift fragment when diagnosing symptoms on a specific target. Richer Tier 3 prompts; no new MI infrastructure required.
- [ ] **MI-03**: MI resolution feedback loop — when MI marks an `audit_known_issues` entry as `fix_applied_at`, the next drift probe either confirms resolution (state now matches git → entry auto-closes) or flags the fix as ineffective. MI becomes accountable to the drift detector.

### Ecosystem Synergy (SYN)

- [ ] **SYN-01**: `backlog-enforce.js` hook replaced (or rewritten) to read `audit_known_issues` table instead of text-scanning memory prose. Eliminates false-positives and duplicate entries (current hook emits duplicates like `project_freedom_mode_focus_contract_20260423.md: NOT YET DEPLOYED, NOT DEPLOYED` — two matches on one item).
- [ ] **SYN-02**: Content Drift Detector (Phase 366, polls pod `GET /debug/content-dirs` every 60min, writes `content_drift_events`) subsumed or composed into unified schema — v53.0 drift report includes `delta_type` field covering game_added/removed, car_added/removed, track_added/removed. No parallel drift systems.
- [ ] **SYN-03**: Drift penalty contributes to Phase 366 Fleet Intelligence composite health score — pods with P0/P1/P2 drift subtract up to 20pts from `METRIC_POD_HEALTH_SCORE`. Existing Uday dashboards reflect drift without new UI work.

## Out of Scope

- **Automated remediation** — v53.0 detects and reports only. Each drift entry gets a manual fix action (deploy via existing script, commit to git, etc.). Auto-apply is v54.0+ if ever.
- **Historical drift timeline** — initial manifest is a point-in-time snapshot. Time-series retention (diff vs N days ago) is deferred to v53.1.
- **Manifest signing / tamper-evidence** — probes capture state, they don't sign it. If an attacker tampers with a pod, probes will report the tampered state. Defence-in-depth is separate scope.
- **Kiosk + PWA state probing** — browser runtime state (localStorage, session JWT) is not captured in v1; only server/backend state.
- **Reverse-engineering unlabeled binaries** — probe captures `binary_sha256` only; mapping unknown SHA → source commit is a v53.1 enhancement.

## Future Requirements (deferred)

- Time-series drift retention (90 days) — lets "when did X diverge?" queries work
- Auto-remediation hooks — on detection, auto-deploy binaries that are confirmed-safe (MMA-approved commits)
- Integration with existing Phase 445 typed-API-contract probe — confirm TS types match deployed server schema
- Manifest diff between two arbitrary timestamps (not just "live vs git")

## Open Questions

All resolved in milestone kickoff (2026-04-24):
1. Milestone version → v53.0
2. Location → `racecontrol/.planning/` (graphify-informed: unified cross-repo precedent)
3. Starting phase → 447 (446 taken by OPENROUTER_KEY canonicalization)
4. Backlog ordering → 3 undeployed items ship AFTER milestone (user directive)
5. Cloud scope → included in v1 (admin.racingpoint.cloud + cloud racecontrol)
6. Server .23 SSH audit gap → part of PROBE-01, not a prereq

## Traceability

Filled by roadmap — each requirement maps to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SCHEMA-01 | Phase 447 | Pending |
| SCHEMA-02 | Phase 447 | Pending |
| SCHEMA-03 | Phase 447 | Pending |
| PROBE-01 | Phase 448 | Pending |
| PROBE-02 | Phase 448 | Pending |
| PROBE-03 | Phase 448 | Pending |
| PROBE-04 | Phase 448 | Pending |
| PROBE-05 | Phase 448 | Pending |
| PROBE-06 | Phase 448 | Pending |
| PROBE-07 | Phase 448 | Pending |
| PROBE-08 | Phase 448 | Pending |
| PROBE-09 | Phase 448 | Pending |
| GRAPH-B-01 | Phase 450 | Pending |
| GRAPH-B-02 | Phase 450 | Pending |
| GRAPH-B-03 | Phase 450 | Pending |
| GRAPH-D-01 | Phase 451 | Pending |
| GRAPH-D-02 | Phase 451 | Pending |
| DIFF-01 | Phase 452 | Pending |
| DIFF-02 | Phase 452 | Pending |
| DIFF-03 | Phase 452 | Pending |
| DIFF-04 | Phase 452 | Pending |
| DIFF-05 | Phase 452 | Pending |
| VALIDATE-01 | Phase 453 | Pending |
| VALIDATE-02 | Phase 453 | Pending |
| VALIDATE-03 | Phase 453 | Pending |
| VALIDATE-04 | Phase 453 | Pending |
| REPORT-01 | Phase 454 | Pending |
| REPORT-02 | Phase 454 | Pending |
| REPORT-03 | Phase 454 | Pending |
| LIFECYCLE-01 | Phase 455 | Pending |
| LIFECYCLE-02 | Phase 455 | Pending |
| MI-01 | Phase 455 | Pending |
| MI-02 | Phase 455 | Pending |
| MI-03 | Phase 455 | Pending |
| SYN-01 | Phase 455 | Pending |
| SYN-02 | Phase 455 | Pending |
| SYN-03 | Phase 455 | Pending |

**Coverage:** 37 / 37 requirements mapped to phases (100%) — 27 original + 2 added to Phase 452 (DIFF-04/05, gap-fix from lifecycle review) + 8 added via Phase 455 (Lifecycle+MI+Synergy omnibus, scope locked 2026-04-24).
**Phase 449** (First Full-Fleet Probe Run) is an execution gate that exercises PROBE-01..09 against the live fleet; it carries no new REQ mappings but produces the evidence that PROBE-01..09 are complete in practice, not just in code.
**Phase 455** (Lifecycle & Ecosystem Integration) wires v53.0 into the deploy lifecycle, MI diagnostic pipeline, and ecosystem surfaces (backlog gate, content drift detector, fleet health score) so the tool stays truthful after every deploy AND force-multiplies existing systems instead of running parallel to them.

---

*Last updated: 2026-04-24 — v53.0 milestone definition. Kickoff commit `e073bdb6` on `docs/v53-milestone-kickoff-20260424`.*
