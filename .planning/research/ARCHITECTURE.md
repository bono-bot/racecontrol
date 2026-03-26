# Architecture Research

**Domain:** Autonomous bug detection & self-healing integration — v26.0 (adds scheduling, cascade engine, expanded fixes, chain templates)
**Researched:** 2026-03-26
**Confidence:** HIGH (based on direct analysis of existing scripts, audit framework, comms-link, and all deployed infrastructure)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                   JAMES (On-Site Primary, .27)                            │
│                                                                            │
│  Task Scheduler (daily 02:30 IST)                                         │
│       │                                                                    │
│       ▼                                                                    │
│  scripts/auto-detect.sh  ←── on-demand: AUDIT_PIN=... bash scripts/...   │
│       │                                                                    │
│  Step 1: audit/audit.sh (60 phases, parallel, auto-fix)                  │
│  Step 2: comms-link/test/run-all.sh (quality gate)                       │
│  Step 3: E2E health (server + relay + Next.js apps)                       │
│  Step 4: cascade.sh (build drift / pod consistency / cloud-venue sync)   │  ← NEW
│  Step 5: standing-rules-check.sh (unpushed commits, relay health, bats)  │  ← NEW
│  Step 6: report + notify (Bono WS + INBOX.md + WhatsApp if critical)     │
│                                                                            │
│  ┌─────────────────────────────────────────────────────────┐              │
│  │ audit/lib/fixes.sh  (extended APPROVED_FIXES whitelist) │  ← EXTEND   │
│  │ audit/lib/notify.sh (already works — no change)         │              │
│  └─────────────────────────────────────────────────────────┘              │
│                                                                            │
│  comms-link relay :8766                                                   │
│       ▲ (chain templates: auto-detect-bono, sync-and-verify)             │
└──────────────────────────┬───────────────────────────────────────────────┘
                           │ WS :8765 / INBOX.md / git push
┌──────────────────────────┴───────────────────────────────────────────────┐
│                   BONO VPS (Failover)                                     │
│                                                                            │
│  cron (0 21 * * * UTC = 02:30 IST)                                       │
│       │                                                                    │
│       ▼                                                                    │
│  scripts/bono-auto-detect.sh                                              │
│       ├── Check James relay alive?                                        │
│       │       YES → delegate to James via relay exec, exit                │
│       │       NO  → run independent checks:                               │
│       │             venue server (Tailscale) / cloud racecontrol         │
│       │             fleet health / Next.js apps / git sync               │
│       └── Notify Uday via WhatsApp if critical                           │
└──────────────────────────────────────────────────────────────────────────┘
                           │
              Both feed into:
                           ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                   Venue Server .23 :8080                                  │
│  /api/v1/fleet/health       /api/v1/health    /api/v1/app-health         │
│                                                                            │
│  8 pods (rc-agent :8090, rc-sentry :8091 each)                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Status |
|-----------|---------------|--------|
| `scripts/auto-detect.sh` | 6-step autonomous pipeline: audit → quality gate → E2E → cascade → standing rules → notify | EXISTS (committed b54e4585) |
| `scripts/bono-auto-detect.sh` | Independent Bono-side detection + James failover | EXISTS (deployed to VPS) |
| `audit/audit.sh` | 60-phase parallel fleet audit with auto-fix engine | EXISTS (v4.0, shipped v23.0) |
| `audit/lib/fixes.sh` | Whitelist-only auto-fix engine (3 approved fixes currently) | EXISTS — needs expansion |
| `audit/lib/notify.sh` | 3-channel notifications: Bono WS + INBOX.md + WhatsApp | EXISTS — reuse as-is |
| `scripts/cascade.sh` | Build drift, pod binary consistency, cloud-venue sync delta, comms-link sync | NEW module |
| `scripts/standing-rules-check.sh` | Unpushed commits, relay health, bat file sync state | NEW module |
| `scripts/log-anomaly.sh` | JSONL log scanning for WARN/ERROR patterns above threshold | NEW module |
| `scripts/config-drift.sh` | Fetch running config from pods/server, compare to repo baseline | NEW module |
| Task Scheduler entry | Daily 2:30 AM IST trigger for auto-detect.sh | NEW (Windows Task Scheduler, not cron) |
| `comms-link/chains.json` | `auto-detect-bono` + `sync-and-verify` templates | EXISTS (updated during v26.0 foundation) |

## Recommended Project Structure

```
scripts/
├── auto-detect.sh          # Main 6-step pipeline (EXISTS)
├── bono-auto-detect.sh     # Bono failover script (EXISTS, on VPS)
├── AUTONOMOUS-DETECTION.md # Architecture doc (EXISTS)
├── cascade.sh              # NEW: cascade engine module
├── standing-rules-check.sh # NEW: standing rules enforcement
├── log-anomaly.sh          # NEW: log pattern scanner
├── config-drift.sh         # NEW: config drift detector
├── register-james-watchdog.bat  # EXISTS: Task Scheduler registration
└── deploy/                 # existing deploy scripts

audit/
├── audit.sh                # Entry point (EXISTS)
├── lib/
│   ├── core.sh             # Primitives (EXISTS)
│   ├── fixes.sh            # Auto-fix engine (EXISTS — extend APPROVED_FIXES)
│   ├── notify.sh           # Notifications (EXISTS — no change)
│   ├── parallel.sh         # Parallel engine (EXISTS)
│   ├── results.sh          # Result aggregation (EXISTS)
│   ├── delta.sh            # Delta tracking (EXISTS)
│   ├── suppress.sh         # Suppression (EXISTS)
│   └── report.sh           # Report generation (EXISTS)
└── phases/
    └── tier*/phaseNN.sh    # 60 phase scripts (EXISTS — v23.1 extends some)

audit/results/
└── auto-detect-YYYY-MM-DD_HH-MM/   # Per-run result dirs
    ├── auto-detect.log             # Full run log
    ├── steps.jsonl                 # Per-step PASS/FAIL/WARN records
    ├── cascade.json                # Cascade check results
    ├── standing-rules.json         # Standing rules check results
    ├── audit-summary.json          # Copied from audit run
    └── auto-detect-summary.json    # Final summary for notifications
```

### Structure Rationale

- **scripts/ for pipeline:** auto-detect.sh is not an audit phase — it is an orchestrator that calls audit.sh plus additional checks. Separate dir avoids polluting the audit framework.
- **Extend audit/lib/fixes.sh, do not fork it:** The APPROVED_FIXES whitelist and is_pod_idle() billing gate are already correct. New fixes (config drift reset, git auto-pull) are appended to the same whitelist — same safety properties.
- **New modules as sourced scripts:** cascade.sh, standing-rules-check.sh, log-anomaly.sh, config-drift.sh are sourced by auto-detect.sh (not spawned as subprocesses). This preserves variable sharing (BUGS_FOUND, BUGS_FIXED, STEP_RESULTS) without IPC.
- **Result dir per run:** Each auto-detect run gets its own timestamped result dir under audit/results/. Allows delta comparison between runs and post-run debugging.

## Architectural Patterns

### Pattern 1: Source-Based Module Composition

**What:** New feature modules (cascade.sh, standing-rules-check.sh) are bash scripts that define functions and are `source`d into auto-detect.sh — not spawned as separate processes.

**When to use:** When modules need to share the STEP_RESULTS associative array, BUGS_FOUND counter, and LOG_FILE path without IPC overhead.

**Trade-offs:** Cannot run modules in parallel (they modify shared state). Acceptable because cascade and standing-rules checks are fast (< 30s each) and run after the parallel audit phase completes.

**Example:**
```bash
# In auto-detect.sh
source "$SCRIPT_DIR/cascade.sh"
source "$SCRIPT_DIR/standing-rules-check.sh"

run_cascade_check  # defined in cascade.sh, uses shared STEP_RESULTS
run_standing_rules # defined in standing-rules-check.sh
```

### Pattern 2: Extend APPROVED_FIXES, Never Bypass

**What:** Every new auto-fix action must be added to the `APPROVED_FIXES` array in `audit/lib/fixes.sh`. The `_is_approved_fix()` gate runs before any fix executes.

**When to use:** Every time a new self-healing action is added (config reset, git pull, bat deploy).

**Trade-offs:** Requires a code change to add new fix types. This is intentional — it prevents runaway automation by requiring explicit human approval for new fix categories.

**Example:**
```bash
# audit/lib/fixes.sh
APPROVED_FIXES=(
  "clear_stale_sentinels"
  "kill_orphan_powershell"
  "restart_rc_agent"
  "reset_config_drift"      # NEW: rewrite config file to repo baseline
  "git_auto_pull_server"    # NEW: git pull on server when behind remote
  "sync_bat_files"          # NEW: deploy bat files from repo to pod
)
```

### Pattern 3: Bono Delegate-or-Act Failover

**What:** bono-auto-detect.sh checks James relay health first. If James is alive, it delegates via relay exec and exits. If James is down, it runs independent checks directly.

**When to use:** Any Bono-side autonomous task that overlaps with James's capabilities.

**Trade-offs:** If the James relay is alive but James is overloaded (audit running), delegation still exits — acceptable because both can't audit simultaneously anyway. Bono's independent checks cover only what's reachable via Tailscale (server .23, cloud :8080) — not direct pod exec.

### Pattern 4: Config Drift via API, Not SSH

**What:** To detect config drift, fetch the running config from the server's `/api/v1/config/...` endpoints and compare to the repo baseline. Do NOT SSH into the server to read `racecontrol.toml` directly.

**When to use:** Any check that needs the running config value vs the expected value.

**Trade-offs:** Only works for config values exposed via API. Config values not surfaced by an endpoint cannot be drift-checked without SSH (standing rule: avoid SSH piping — use scp or dedicated endpoint).

**Implication for build order:** If config drift check requires new API endpoints on racecontrol (e.g. `/api/v1/config/ws_timeout`), those endpoints must ship first (requires Rust rebuild + deploy). This is a dependency for the config-drift.sh phase.

### Pattern 5: Log Aggregation — Query On-Demand, Not Centralized

**What:** Log anomaly detection queries logs on-demand from each pod via fleet exec, rather than streaming all logs to a central store.

**When to use:** For the autonomous detection pipeline (runs nightly, not real-time). Centralized log streaming would require new infrastructure.

**Trade-offs:** On-demand query means the pipeline must exec a command on each pod to fetch recent log lines. This is slow (8 pods x 1 exec each) but fits within the 8-minute budget. The exec goes through rc-agent :8090 fleet exec endpoint, not direct SSH. Billing session check (is_pod_idle) is NOT required for read-only exec.

**Example flow:**
```
auto-detect.sh → log-anomaly.sh
  → POST /api/v1/fleet/exec {pod_ip, command: "tail -n 200 C:\RacingPoint\rc-agent-*.jsonl | findstr ERROR"}
  → parse response
  → count ERROR/WARN lines above threshold
  → record_step "log_anomaly_pod_N" "FAIL" "errors=42"
```

## Data Flow

### Detection Run Flow

```
Task Scheduler (02:30 IST)
    │
    ▼
auto-detect.sh
    │
    ├─── Step 1: audit.sh --mode standard --auto-fix
    │         │
    │         └── 60 phases (parallel, 4 concurrent)
    │             → emit_result() per phase → phase-NNN-host.json
    │             → auto-fix via fixes.sh (whitelist gate + idle gate)
    │             → emit_fix() to fixes.jsonl
    │             → audit-summary.json + audit-report.md
    │
    ├─── Step 2: comms-link test/run-all.sh
    │         → 4 suites (contract + integration + syntax + security)
    │         → exit code 0=PASS, 1=FAIL
    │
    ├─── Step 3: E2E health (curl-based)
    │         → server :8080/health, relay :8766/health, Next.js apps
    │         → record per-service status
    │
    ├─── Step 4: cascade.sh (sourced)
    │         → git log: HEAD vs deployed build_id (drift check)
    │         → fleet/health: all pods same build_id (consistency)
    │         → compare venue build_id vs cloud build_id (cloud-venue sync)
    │         → comms-link: git log HEAD vs deployed (comms drift)
    │         → record cascade.json
    │
    ├─── Step 5: standing-rules-check.sh (sourced)
    │         → git status: any unpushed commits?
    │         → relay health: comms-link daemon alive?
    │         → bat sync: deployed bats match repo? (spot-check pod 8)
    │         → record standing-rules.json
    │
    └─── Step 6: report + notify
              → auto-detect-summary.json (all step results + counts)
              → send_notifications() (Bono WS + INBOX.md + WhatsApp if FAIL)
```

### Cascade Check Data Flow

```
cascade.sh
    │
    ├── git rev-parse --short HEAD              (James local)
    ├── GET /api/v1/health → .build_id          (server .23)
    ├── GET /api/v1/fleet/health → .build_id[N] (all 8 pods via server)
    ├── GET cloud-racecontrol/health → .build_id (Bono VPS via cloud URL)
    └── git log in comms-link dir               (James local)
    │
    → compare each pair → emit drift event if mismatch
    → write cascade.json: {drift_events: [...], all_consistent: bool}
```

### Bono Failover Data Flow

```
Bono cron (02:30 IST)
    │
    ▼
bono-auto-detect.sh
    │
    ├── curl JAMES_RELAY/relay/health (Tailscale :8766)
    │       OK → POST /relay/exec/run {command: shell, args: "bash auto-detect.sh"}
    │            exit 0 (James handles it)
    │       FAIL ↓
    │
    ├── curl SERVER_URL/api/v1/health (Tailscale .23)
    ├── curl CLOUD_URL/api/v1/health (localhost)
    ├── curl SERVER_URL/api/v1/fleet/health (pod summary)
    ├── curl Next.js apps via server
    └── git -C /root/racecontrol log --oneline -5 (sync state)
    │
    └── if critical: notify_uday() via WhatsApp (Evolution API)
```

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Current (8 pods) | On-demand log query via fleet exec is fine. 8 x 1 exec = ~8s within budget. |
| 16+ pods | Parallel log query (background subshells + wait, like audit parallel engine). |
| Real-time anomaly detection | Would require streaming logs to a central store (Redis/Loki). Out of scope for v26.0 — nightly batch is sufficient. |

### Scaling Priorities

1. **First bottleneck:** Auto-detect runtime exceeding 8-minute budget. Mitigation: log-anomaly and config-drift checks use --mode=quick audit internally (Tiers 1-2 only), or run as separate cron job at a different time.
2. **Second bottleneck:** Cascade check blocked on server being down. Mitigation: cascade.sh uses a 10s timeout per check and skips non-reachable targets without failing the entire run.

## Anti-Patterns

### Anti-Pattern 1: SSH Piping for Config Fetch

**What people do:** `ssh ADMIN@server "cat C:\RacingPoint\racecontrol.toml" > local-copy.toml` to check running config.

**Why it's wrong:** SSH banner lines (post-quantum warnings, MOTD) corrupt the file silently. This exact pattern caused a 2-hour process guard outage (racecontrol.toml had banner lines prepended, TOML parser rejected it, empty config loaded).

**Do this instead:** Expose config values via a dedicated `/api/v1/config/...` endpoint in racecontrol. Fetch with curl. If endpoint doesn't exist yet, use SCP (`scp ADMIN@server:C:/RacingPoint/racecontrol.toml /tmp/`) and validate first line.

### Anti-Pattern 2: Adding New Steps Without Runtime Budget Check

**What people do:** Add config drift, log anomaly, and standing rules checks as sequential steps inside auto-detect.sh without measuring cumulative runtime.

**Why it's wrong:** audit.sh (full mode) already takes ~8 minutes. Adding 3 more sequential steps risks blowing past the 8-minute ceiling constraint (v26.0 requirement: total runtime under 8 minutes in full mode).

**Do this instead:** Run new steps only in `standard` mode (not `full`) by default, or run them only when audit PASS count is above threshold (skip deep checks if audit already found serious failures). Measure runtime with `time` and adjust mode selection.

### Anti-Pattern 3: Scheduler Race with Existing Bono Monitors

**What people do:** Set James Task Scheduler and Bono cron to the same time (02:30 IST).

**Why it's wrong:** If both fire simultaneously, Bono's cron checks James alive → James is running auto-detect (relay alive) → Bono delegates → James now has two parallel auto-detect runs (original cron + delegated via relay exec). Double audit load.

**Do this instead:** Bono cron runs 5 minutes later (02:35 IST = `5 21 * * * UTC`). By then, James's Task Scheduler run is underway and the relay health check succeeds → Bono delegates and exits. No race condition.

### Anti-Pattern 4: Blocking on Pod Exec for Standing Rules Check

**What people do:** Include `bat file sync check` in standing-rules-check.sh by exec-ing into all 8 pods to fetch the bat file content.

**Why it's wrong:** 8 x exec calls for a standing rules check turns a 5-second check into a 40-second blocking step. Standing rules checks should be local-first.

**Do this instead:** Spot-check pod 8 only (canary pattern), or check only the server and James bat files (which are in the repo). Pod bat files are deployed as part of the binary deploy cycle — if the deploy standing rule is followed, they're in sync.

### Anti-Pattern 5: Autonomous Fix Without is_pod_idle Gate

**What people do:** Add a new auto-fix action that restarts a service on a pod, but forget to call `is_pod_idle()` first.

**Why it's wrong:** The idle gate check is the only thing preventing a fix from interrupting a live billing session. This is the most critical safety invariant in the entire auto-fix system.

**Do this instead:** Every new fix function in fixes.sh that affects a pod MUST call `is_pod_idle "$pod_ip"` before executing. The existing `_is_approved_fix()` gate does not substitute for this — both must pass.

## Integration Points

### Where New Components Hook Into Existing Architecture

| New Component | Hooks Into | Integration Method |
|---------------|-----------|-------------------|
| `scripts/cascade.sh` | auto-detect.sh Step 4 | `source "$SCRIPT_DIR/cascade.sh"` then call `run_cascade_check` |
| `scripts/standing-rules-check.sh` | auto-detect.sh Step 5 | `source "$SCRIPT_DIR/standing-rules-check.sh"` then call `run_standing_rules_check` |
| `scripts/log-anomaly.sh` | auto-detect.sh (optional Step 3b) OR as audit phase (tier-N) | Either sourced in auto-detect, or as a new phase in audit/phases/ (preferred for parallel execution) |
| `scripts/config-drift.sh` | Requires new racecontrol API endpoints OR fetches via SCP | Dependency: racecontrol changes needed FIRST if API-based |
| Extended `APPROVED_FIXES` | audit/lib/fixes.sh | Append new fix names to array; implement `apply_fix_<name>()` function |
| Task Scheduler entry | Windows Task Scheduler on James .27 | register-james-watchdog.bat already exists — add new entry for auto-detect.sh |
| Bono cron | Bono VPS crontab | Already deployed at `0 21 * * *` UTC |
| `chains.json` templates | comms-link relay :8766 | Already updated with `auto-detect-bono` and `sync-and-verify` templates |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| auto-detect.sh ↔ audit.sh | subprocess (bash call + exit code + stdout) | audit.sh writes to its own result dir; auto-detect.sh finds latest result dir by timestamp |
| auto-detect.sh ↔ cascade.sh | sourced (shared bash env) | Shares STEP_RESULTS, BUGS_FOUND, LOG_FILE, RESULT_DIR |
| auto-detect.sh ↔ comms-link test suite | subprocess (cd + bash call + exit code) | Must cd to comms-link dir; COMMS_PSK env var required |
| auto-detect.sh ↔ notify.sh | sourced (reuse audit notify.sh) | OR replicate inline; notify.sh requires RESULT_DIR, AUDIT_MODE, COMMS_PSK, COMMS_URL |
| James auto-detect ↔ Bono VPS | WS (comms-link relay) + INBOX.md + git | Results sent as WS message on completion; INBOX.md for persistent record |
| Bono auto-detect ↔ James relay | HTTP GET /relay/health + POST /relay/exec/run | Tailscale IP 100.82.33.94:8766 for James relay |
| cascade.sh ↔ racecontrol | HTTP GET fleet/health + health endpoints | No auth required for health endpoints (public_routes) |
| config-drift.sh ↔ racecontrol | HTTP GET config endpoints (need to be added) OR SCP | Config endpoints don't exist yet — new racecontrol routes needed |
| log-anomaly.sh ↔ pods | HTTP POST /api/v1/fleet/exec (read-only log fetch) | No billing idle gate needed for read-only exec |

## Recommended Build Order

This ordering respects all dependencies identified above:

### Phase 1 — Extend Existing Modules (no new files, no new infrastructure)

1. Extend `audit/lib/fixes.sh` — add new entries to APPROVED_FIXES + implement fix functions (`reset_config_drift`, `git_auto_pull`, `sync_bat_files`). These are bash function additions to an existing file. Zero risk to existing behavior (whitelist gate unchanged).

2. Add `--notify` integration to auto-detect.sh notification step — currently auto-detect.sh has a stub. Wire it to audit/lib/notify.sh's `send_notifications` function or replicate the 3-channel pattern inline. Low risk.

### Phase 2 — New Bash Modules (no compiled deps, no new infrastructure)

3. Create `scripts/cascade.sh` — build drift check, pod consistency, cloud-venue sync. Pure bash + curl + jq. Source it in auto-detect.sh as Step 4.

4. Create `scripts/standing-rules-check.sh` — git status, relay health, bat spot-check on pod 8. Source it in auto-detect.sh as Step 5.

5. Create `scripts/log-anomaly.sh` — fleet exec log tail + pattern count. Add as Step 3b in auto-detect.sh (after E2E health, before cascade).

### Phase 3 — Scheduler (Windows-only concern)

6. Register Task Scheduler entry for auto-detect.sh on James. Use `register-james-watchdog.bat` as a model. Run at 02:30 IST = 21:00 UTC. Bono cron already active.

### Phase 4 — Config Drift (has upstream dependency)

7. If config drift requires racecontrol API changes: add `GET /api/v1/config/health-params` endpoint to racecontrol (Rust — requires rebuild + deploy). This is a hard dependency that cannot be faked.
8. Create `scripts/config-drift.sh` after endpoint is available.

### Phase 5 — Integration Tests for the Pipeline

9. Add test suite for auto-detect.sh itself — dry-run mode + mock audit results + verify each step records correctly. Fits in comms-link test/run-all.sh as Suite 5, or as standalone `scripts/test-auto-detect.sh`.

### Phase 6 — WhatsApp Escalation for Critical Unfixed Issues

10. Integrate WhatsApp alert into auto-detect.sh final report: if BUGS_UNFIXED > 0 after all steps, call `notify_uday()` (same pattern as bono-auto-detect.sh already has). Uses Bono VPS Evolution API (not James — per standing rule: promotions/alerts go via Bono VPS).

## Sources

- `scripts/auto-detect.sh` (committed b54e4585) — 6-step pipeline, actual implementation
- `scripts/bono-auto-detect.sh` — Bono failover, actual implementation
- `scripts/AUTONOMOUS-DETECTION.md` — architecture decisions, chain templates
- `audit/lib/fixes.sh` — APPROVED_FIXES whitelist, is_pod_idle() billing gate
- `audit/lib/notify.sh` — 3-channel notification pattern
- `audit/lib/core.sh` — shared primitives, IST timestamps, emit_result/emit_fix
- `audit/audit.sh` — full audit entry point, mode flags, parallel engine
- `CLAUDE.md` — standing rules: SSH piping hazard, cmd.exe quoting, bat file CWD, idle gate
- `MEMORY.md` — comms-link relay endpoints, Bono exec pattern, WhatsApp routing (Bono VPS only)

---
*Architecture research for: v26.0 Autonomous Bug Detection & Self-Healing integration*
*Researched: 2026-03-26*
