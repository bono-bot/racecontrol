# Architecture Research

**Domain:** Bash-based automated fleet audit system (v23.0 Audit Protocol v4.0)
**Researched:** 2026-03-25
**Confidence:** HIGH — based on direct analysis of AUDIT-PROTOCOL v3.0, PROJECT.md milestone spec,
existing infrastructure (APIs, comms-link relay, WhatsApp alerter), and CLAUDE.md standing rules.

---

## What We Are Building

AUDIT-PROTOCOL v3.0 is a 1928-line markdown document with 60 phases of copy-paste bash commands.
Running an audit currently means: open the document, read the description, manually copy each curl
block, decide whether the output is PASS/FAIL, record results in a separate summary template. A full
audit takes 45-90 minutes of active operator attention.

v23.0 transforms this into a single command:

```bash
bash audit/audit.sh --mode standard
```

That command runs all appropriate phases non-interactively, scores each phase, emits structured JSON,
generates a Markdown report comparing against the previous audit, applies pre-approved auto-fixes,
notifies Bono via comms-link, and sends a WhatsApp summary to Uday. Total time: under 5 minutes
unattended.

The system is pure bash — no new compiled dependencies, no new services, no new ports.

---

## System Overview

```
+------------------------------------------------------------------+
|  James (.27) — audit runner                                      |
|                                                                  |
|  audit/audit.sh  <-- entry point, mode selector, tier scheduler |
|       |                                                          |
|       +-- audit/lib/core.sh     -- PASS/WARN/FAIL/QUIET helpers |
|       +-- audit/lib/parallel.sh -- background job throttle      |
|       +-- audit/lib/fixes.sh    -- pre-approved auto-fix ops    |
|       +-- audit/lib/delta.sh    -- diff against last results     |
|       +-- audit/lib/notify.sh   -- comms-link + WhatsApp output  |
|       |                                                          |
|       +-- audit/phases/tier1/   -- 10 phases (infra foundation)  |
|       +-- audit/phases/tier2/   -- 6 phases  (core services)     |
|       +-- audit/phases/tier3/   -- 4 phases  (display/UX)        |
|       +-- audit/phases/tier4/   -- 5 phases  (billing/commerce)  |
|       +-- audit/phases/tier5/   -- 4 phases  (games/hardware)    |
|       +-- audit/phases/tier6/   -- 5 phases  (notifications)     |
|       +-- audit/phases/tier7/   -- 5 phases  (cloud/PWA)         |
|       +-- audit/phases/tier8/   -- 5 phases  (security/access)   |
|       +-- audit/phases/tier9/   -- 6 phases  (data/analytics)    |
|       +-- audit/phases/tier10/  -- 5 phases  (AI/feature flags)  |
|       +-- audit/phases/tier11/  -- 5 phases  (marketing/staff)   |
|       +-- audit/phases/tier12/  -- 5 phases  (OTA/deployment)    |
|       |                                                          |
|       +-- audit/results/        -- JSON output per run           |
|       +-- audit/reports/        -- Markdown reports per run      |
|       +-- audit/suppress.json   -- known-issue suppression list  |
+------------------------------------------------------------------+
         |                    |                    |
         | HTTP APIs          | SSH (Tailscale)    | comms-link relay
         v                    v                    v
  Server .23           Bono VPS               WhatsApp
  :8080/:8090         :8080/:8766             (via Evolution API
  8 pods :8090/:8091   comms-link relay        on Bono VPS)
```

---

## Component Boundaries

### Component 1: audit.sh — Entry Point and Tier Scheduler

**Responsibility:** Single entry point. Parses `--mode` flag, selects which tiers to run, detects
venue-open/closed state, initializes shared variables (SESSION token, PODS array, RUN_ID, OUTPUT
paths), schedules tier execution, invokes report and notify steps after tiers complete.

**Does NOT do:** Execute individual phase checks. Parse command output. Apply fixes. Send
notifications directly.

**Inputs:** `--mode quick|standard|full|pre-ship|post-incident`, optional `--pod N` for single-pod
focus, optional `--tier N` for single-tier focus.

**Outputs:** Sets `AUDIT_RUN_ID`, `AUDIT_START_IST`, `RESULTS_DIR`, `REPORT_PATH` for all children.

**Mode → Tier mapping:**
```
quick:         Tier 1 (infra) + Tier 2 (core services) only — ~10 phases
standard:      Tiers 1-6 + skip hardware/display (venue-closed aware) — ~40 phases
full:          All 60 phases
pre-ship:      All 60 phases + strict severity (WARN treated as FAIL)
post-incident: All 60 phases + verbose output + no suppression
```

**Venue-open detection:**
```bash
ACTIVE_SESSIONS=$(curl -s http://192.168.31.23:8080/api/v1/billing/sessions/active \
  -H "x-terminal-session: $SESSION" 2>/dev/null | jq 'length // 0')
# OR: check time-of-day heuristic (09:00–22:00 IST = likely open)
```

When venue-closed, Tier 3 (display/UX) and Tier 5 (games/hardware) phases produce QUIET results,
not FAIL.

---

### Component 2: Phase Scripts — audit/phases/tierN/phaseNN.sh

**Responsibility:** Execute the bash commands for exactly one phase. Emit structured JSON result to
stdout. Apply pre-approved auto-fixes when fix loop triggers are met. Return exit code 0 always
(errors are encoded in JSON, not bash exit status).

**Each phase script:**
1. Runs its verification commands (curl, jq, ssh)
2. Evaluates result against expected values
3. Emits JSON result block (see Data Flow section)
4. If `AUTO_FIX=1` and fix loop triggers are met: applies safe fix, emits a second JSON block
   with `"type": "fix_applied"`
5. Exits 0

**Phase scripts are thin.** Complex logic (throttling, retries, fix application) lives in lib/.
Phase scripts source `../lib/core.sh` and call `emit_result`, `emit_fix`, `apply_safe_fix`.

**Naming convention:** `phaseNN.sh` where NN matches v3.0 phase numbers exactly.
Phase 01 through 60 preserve all v3.0 checks without modification to the commands themselves.
New v4.0 phases (delta tracking, feature flags) start at 61+.

---

### Component 3: lib/core.sh — Shared Primitives

**Responsibility:** Shared bash functions used by every phase script and by audit.sh itself.

**Key functions:**

```bash
# Emit a phase result as JSON to stdout
emit_result() {
  local phase="$1" status="$2" message="$3" severity="$4" detail="$5"
  # status: PASS | WARN | FAIL | QUIET
  # severity: P1 | P2 | P3 (only set for FAIL/WARN)
  printf '{"type":"phase_result","phase":%d,"status":"%s","severity":"%s",
    "message":"%s","detail":%s,"ts_utc":"%s","run_id":"%s"}\n' \
    "$phase" "$status" "$severity" "$message" "$detail" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$AUDIT_RUN_ID"
}

# Emit a fix-applied record
emit_fix() {
  local phase="$1" action="$2" result="$3"
  printf '{"type":"fix_applied","phase":%d,"action":"%s","result":"%s","ts_utc":"%s"}\n' \
    "$phase" "$action" "$result" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}

# Pod loop helper — run command on all 8 pods, collect per-pod results
pod_loop() {
  # Sources lib/parallel.sh for throttled background execution
}

# Auth token fetch — called once at startup, reused by all phases
get_session_token() {
  SESSION=$(curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/auth \
    -H "Content-Type: application/json" -d '{"pin":"261121"}' | jq -r '.session')
  export SESSION
}

# Suppression check — returns 1 if this phase result is suppressed
is_suppressed() {
  local phase="$1" message="$2"
  jq --arg phase "$phase" --arg msg "$message" \
    '.[] | select(.phase == ($phase | tonumber) and (.pattern | test($msg)))' \
    "$SUPPRESS_FILE" | grep -q .
}
```

---

### Component 4: lib/parallel.sh — Background Job Throttle

**Responsibility:** Run phase scripts for a tier in parallel using bash background jobs, with a
maximum concurrency limit of 4 concurrent pod-targeting commands (per project constraint: "Parallel
execution must not overwhelm pods (max 4 concurrent pod queries)").

**Design:**
- Tier-level parallelism: multiple phase scripts within a tier run concurrently in background.
- Pod-level throttle: within any phase that loops over pods, a semaphore limits to 4 simultaneous
  pod requests. This is the critical constraint — 8 pods x unlimited parallel phases could fire 64
  concurrent requests.
- Uses file-based semaphore (lockfiles in `/tmp/audit-sem-$AUDIT_RUN_ID/`).
- All background jobs write JSON to their own temp file; audit.sh collects after tier completes.

**Semaphore pattern:**
```bash
acquire_sem() {
  while true; do
    local count=$(ls /tmp/audit-sem-$AUDIT_RUN_ID/ 2>/dev/null | wc -l)
    if [ "$count" -lt 4 ]; then
      touch "/tmp/audit-sem-$AUDIT_RUN_ID/$$"
      return 0
    fi
    sleep 0.2
  done
}

release_sem() {
  rm -f "/tmp/audit-sem-$AUDIT_RUN_ID/$$"
}
```

---

### Component 5: lib/fixes.sh — Pre-Approved Auto-Fix Operations

**Responsibility:** A curated, conservative set of fix operations that can be applied without
operator approval when a fix loop trigger is met. Every fix here was evaluated for reversibility
and blast radius before being included.

**Design principle:** The fix list is a whitelist. An operation not on the list cannot be applied
automatically — it must be flagged as requiring manual intervention.

**Pre-approved fixes (initial set):**

| Fix ID | Trigger Condition | Action | Reversibility |
|--------|------------------|--------|---------------|
| FIX-01 | MAINTENANCE_MODE file present on pod | `curl -X POST :8091/exec {"cmd":"del C:\\RacingPoint\\MAINTENANCE_MODE"}` then schtasks restart | Reversible: rc-agent restarts cleanly |
| FIX-02 | GRACEFUL_RELAUNCH or restart sentinel present | Delete sentinel via rc-sentry exec | Reversible |
| FIX-03 | Orphan PowerShell count > 2 on pod | `taskkill /F /IM powershell.exe` via rc-agent exec | Reversible |
| FIX-04 | Variable_dump.exe running | `taskkill /F /IM Variable_dump.exe` via rc-agent exec | Reversible |
| FIX-05 | Overlay process detected (Copilot, NVIDIA overlay, GameBar) | Targeted taskkill by process name | Reversible |
| FIX-06 | Duplicate rc-agent instances (count > 1) | `taskkill /F /IM rc-agent.exe` + schtasks restart | Reversible via rc-sentry |
| FIX-07 | Bono comms-link relay health check fails | `curl -X POST /relay/exec/run {"command":"restart_comms_link"}` via relay | Reversible |

**NOT auto-fixable (require manual):** Config file corruption (SSH banner), Tailscale offline,
NVIDIA Surround resolution collapsed, build_id mismatch (requires rebuild), firewall rules
missing, registry key issues.

---

### Component 6: lib/delta.sh — Diff Against Last Results

**Responsibility:** Load the previous audit's JSON result file, compare with current run, emit
a structured diff showing: new failures (regressions), resolved failures (improvements), unchanged
failures (persistent known issues), and unchanged passes.

**Design:**
- Previous result file: `audit/results/latest.json` (symlink to most recent run).
- Current run's per-phase results are collected into `audit/results/$AUDIT_RUN_ID.json`.
- `delta.sh` does a jq-based join on `phase` number between old and new.
- Outputs a `delta.json` summary consumed by the report generator.

**Delta categories:**
```
REGRESSION:   phase was PASS/QUIET in previous run, is FAIL/WARN now
IMPROVEMENT:  phase was FAIL/WARN in previous run, is PASS now
PERSISTENT:   phase was FAIL/WARN in previous run, still FAIL/WARN
NEW_ISSUE:    phase exists in current run but not in previous (newly added phase)
STABLE:       phase was PASS, still PASS
```

**First run:** No previous results exist. All phases produce `NEW_ISSUE` delta. Delta summary
notes "First audit — no baseline available."

---

### Component 7: lib/notify.sh — Comms-Link + WhatsApp Output

**Responsibility:** After all tiers complete and report is generated, send notifications.

**Two notification paths:**

Path A — Bono comms-link (structured):
```bash
# Send full JSON summary to Bono via relay
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d "{\"command\":\"receive_audit_result\",\"reason\":\"audit complete\",
       \"payload\":$(cat $RESULTS_DIR/summary.json)}"

# Also append to INBOX.md (dual-channel rule)
echo "## $(date '+%Y-%m-%d %H:%M IST') — from james" >> \
  /c/Users/bono/racingpoint/comms-link/INBOX.md
echo "Audit $AUDIT_RUN_ID complete: $PASS_COUNT PASS, $FAIL_COUNT FAIL, \
  $WARN_COUNT WARN, $QUIET_COUNT QUIET. Report: $REPORT_PATH" >> \
  /c/Users/bono/racingpoint/comms-link/INBOX.md
```

Path B — WhatsApp to Uday (human-readable summary only):
```bash
# Via comms-link relay to Bono, who has Evolution API access
# (per project_whatsapp_routing.md: marketing/alerts go via Bono VPS)
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d "{\"command\":\"send_whatsapp\",\"reason\":\"audit summary\",
       \"payload\":{\"phone\":\"usingh_number\",
       \"message\":\"$(generate_wa_summary)\"}}"
```

**Notification triggers (configurable):**
- Always: Bono comms-link message (structured JSON).
- Always: Append to INBOX.md (standing rule: dual channel).
- On FAIL or REGRESSION: WhatsApp to Uday.
- On PASS with no regressions: WhatsApp only if `--notify-all` flag set (default: skip).

**Failure handling:** If comms-link relay is down (`:8766` unreachable), fall through to:
1. Append to INBOX.md + git push (will be picked up by Bono on next pull).
2. Write to `audit/results/$AUDIT_RUN_ID-notify-failed.txt` for manual follow-up.
Do NOT fail the audit run because notification failed.

---

### Component 8: Report Generator — audit/generate-report.sh

**Responsibility:** Read `$RESULTS_DIR/$AUDIT_RUN_ID.json` (all phase results) and
`$RESULTS_DIR/$AUDIT_RUN_ID-delta.json`, produce a human-readable Markdown report and a machine-
parseable summary JSON.

**Markdown report structure:**
```
# Fleet Audit — YYYY-MM-DD HH:MM IST
**Mode:** standard | **Run ID:** abc123 | **Duration:** 4m 12s

## Summary
| Status | Count |
|--------|-------|
| PASS   | 47    |
| WARN   | 3     |
| FAIL   | 2     |
| QUIET  | 8     |

## Delta
### Regressions (new failures since last audit)
- Phase 07 (Process Guard): PASS -> FAIL — violation_count_24h: 100 on pods 2,3,5

### Improvements (resolved since last audit)
- Phase 08 (Sentinel Files): FAIL -> PASS — all pods clean

## Failures Requiring Action
### [P1] Phase 07: Process Guard
...

## Warnings
...

## Auto-Fixes Applied
...

## Suppressed (Known Issues)
...

## Full Phase Results
...
```

**Summary JSON (`$RESULTS_DIR/$AUDIT_RUN_ID-summary.json`):**
```json
{
  "run_id": "abc123",
  "mode": "standard",
  "ts_start_utc": "...",
  "ts_end_utc": "...",
  "duration_secs": 252,
  "pass": 47,
  "warn": 3,
  "fail": 2,
  "quiet": 8,
  "fixes_applied": 1,
  "regressions": 1,
  "improvements": 1,
  "report_path": "audit/reports/2026-03-25-abc123.md"
}
```

---

### Component 9: suppress.json — Known-Issue Suppression List

**Responsibility:** A JSON array of known issues that should not clutter results. Suppressed issues
still appear in the report under "Suppressed (Known Issues)" — they are not hidden, just demoted.

**Format:**
```json
[
  {
    "id": "SUP-001",
    "phase": 19,
    "pattern": "Pod 8.*1024x768",
    "reason": "Pod 8 NVIDIA Surround not configured — needs physical setup",
    "added": "2026-03-25",
    "expiry": null
  },
  {
    "id": "SUP-002",
    "phase": 27,
    "pattern": "AC server.*DOWN",
    "reason": "AC server only runs during customer sessions",
    "added": "2026-03-25",
    "expiry": null
  }
]
```

**Suppression scope:** Suppressed results still run. They still emit JSON. They still appear in the
report. They are excluded from the FAIL/WARN counters that trigger WhatsApp notifications. The
`post-incident` mode bypasses all suppression.

---

## Data Flow

### Phase Execution → JSON Results → Report → Notifications

```
audit.sh
  |
  +-- [startup]
  |     get_session_token() -> $SESSION
  |     export PODS="192.168.31.89 192.168.31.33 ..."
  |     export AUDIT_RUN_ID=$(date +%Y%m%d-%H%M%S)-$(openssl rand -hex 3)
  |     mkdir -p audit/results/$AUDIT_RUN_ID/
  |     detect_venue_state() -> $VENUE_OPEN (true/false)
  |
  +-- [tier execution loop]
  |     for TIER in 1 2 3 ... (based on mode):
  |       [parallel job dispatch]
  |       for PHASE_SCRIPT in audit/phases/tierN/phase*.sh:
  |         background: bash $PHASE_SCRIPT > audit/results/$AUDIT_RUN_ID/phase$N.jsonl
  |         (with semaphore throttle from lib/parallel.sh)
  |       wait for all background jobs
  |       [collect tier results]
  |       cat audit/results/$AUDIT_RUN_ID/phase*.jsonl >> \
  |           audit/results/$AUDIT_RUN_ID/all.jsonl
  |
  +-- [delta computation]
  |     lib/delta.sh $AUDIT_RUN_ID -> audit/results/$AUDIT_RUN_ID/delta.json
  |     (compares against audit/results/latest.json)
  |
  +-- [suppression filter]
  |     apply suppress.json rules to all.jsonl
  |     produces: audit/results/$AUDIT_RUN_ID/filtered.jsonl
  |
  +-- [report generation]
  |     bash audit/generate-report.sh $AUDIT_RUN_ID
  |     -> audit/reports/YYYY-MM-DD-$AUDIT_RUN_ID.md
  |     -> audit/results/$AUDIT_RUN_ID-summary.json
  |
  +-- [symlink update]
  |     ln -sf $AUDIT_RUN_ID/all.jsonl audit/results/latest.json
  |
  +-- [notification dispatch]
        lib/notify.sh $AUDIT_RUN_ID
        -> comms-link relay (structured)
        -> INBOX.md (fallback + standing rule)
        -> WhatsApp to Uday (on FAIL or REGRESSION)
```

### Per-Phase JSON Line Format

Every phase script emits one or more newline-delimited JSON objects (JSONL). All objects share:
```json
{
  "type": "phase_result",
  "phase": 7,
  "tier": 1,
  "phase_name": "Process Guard & Allowlist",
  "status": "FAIL",
  "severity": "P1",
  "message": "violation_count_24h: 100 on pods 2,3,5 — likely empty allowlist",
  "detail": {
    "pods_affected": ["192.168.31.33", "192.168.31.28", "192.168.31.86"],
    "pod_2_count": 100,
    "pod_3_count": 100,
    "pod_5_count": 100
  },
  "fix_trigger": true,
  "ts_utc": "2026-03-25T08:30:00Z",
  "run_id": "20260325-083000-a1b2c3"
}
```

Auto-fix records use the same envelope with `"type": "fix_applied"`.

---

## Recommended File/Directory Structure

```
audit/
+-- audit.sh                      # Entry point, mode selector, tier scheduler
+-- generate-report.sh            # Markdown + summary JSON generation
+-- suppress.json                 # Known-issue suppression list
+-- README.md                     # How to run, mode descriptions, output paths
|
+-- lib/
|   +-- core.sh                   # emit_result, emit_fix, get_session_token, is_suppressed
|   +-- parallel.sh               # Background job throttle, semaphore primitives
|   +-- fixes.sh                  # Pre-approved auto-fix operations (FIX-01 through FIX-N)
|   +-- delta.sh                  # Diff against previous run
|   +-- notify.sh                 # comms-link + WhatsApp notification
|
+-- phases/
|   +-- tier1/                    # Infrastructure Foundation (phases 01-10)
|   |   +-- phase01.sh            # Fleet inventory
|   |   +-- phase02.sh            # Config integrity
|   |   ...
|   |   +-- phase10.sh            # AI healer / watchdog
|   |
|   +-- tier2/                    # Core Services (phases 11-16)
|   +-- tier3/                    # Display & UX (phases 17-20) -- QUIET when venue closed
|   +-- tier4/                    # Billing & Commerce (phases 21-25)
|   +-- tier5/                    # Games & Hardware (phases 26-29) -- QUIET when venue closed
|   +-- tier6/                    # Notifications & Marketing (phases 30-34)
|   +-- tier7/                    # Cloud & PWA (phases 35-39)
|   +-- tier8/                    # Security & Access (phases 40-44)
|   +-- tier9/                    # Data & Analytics (phases 45-50)
|   +-- tier10/                   # AI / Feature Flags (phases 51-55)
|   +-- tier11/                   # Marketing & Staff Gamification (phases 56-58)
|   +-- tier12/                   # OTA & Deployment (phases 59-60)
|
+-- results/                      # Runtime output (git-ignored)
|   +-- latest.json               # Symlink to most recent run's all.jsonl
|   +-- 20260325-083000-a1b2c3/
|       +-- phase01.jsonl         # Per-phase raw output
|       +-- phase02.jsonl
|       +-- ...
|       +-- all.jsonl             # Concatenated all phases
|       +-- delta.json            # Diff vs previous run
|       +-- filtered.jsonl        # After suppression applied
|       +-- summary.json          # Machine-readable summary
|
+-- reports/                      # Human-readable output (git-committed for history)
    +-- 2026-03-25-a1b2c3.md      # Full audit report
    +-- 2026-03-25-a1b2c3-delta.md # Delta-only view
```

**Git strategy:**
- `audit/phases/`, `audit/lib/`, `audit/audit.sh`, `audit/suppress.json` — committed, versioned.
- `audit/results/` — git-ignored (raw runtime data, high churn, contains secrets in detail fields).
- `audit/reports/` — committed (human-readable, no secrets in output, valuable as history).

---

## Build Order

The build order is driven by three dependency chains:

**Chain A: Core runtime must exist before phase scripts can be written.**
Chain B: Phase scripts depend on lib/core.sh functions being stable.
Chain C: Notification depends on report generator, which depends on result collector.

### Phase 1: Scaffold (audit.sh + lib/core.sh + one tier-1 phase)

Build the skeleton first. audit.sh with mode parsing and tier dispatch. lib/core.sh with `emit_result`
and `get_session_token`. One working phase (phase01.sh — Fleet Inventory). Verify the pipeline:
run → JSON out → collected in all.jsonl.

**Deliverable:** `bash audit/audit.sh --mode quick` runs Phase 01, emits valid JSON.
**Rationale:** All subsequent phases can be written and tested against a working pipeline.

### Phase 2: lib/parallel.sh + semaphore

Add background job dispatch and the 4-concurrent-pod semaphore. Verify: run all 10 tier-1 phases
in parallel, confirm no more than 4 simultaneous pod requests, confirm all results collected.

**Deliverable:** `bash audit/audit.sh --mode quick` runs all 10 tier-1 phases in parallel within
the throttle constraint.
**Rationale:** Without the semaphore, Phase 3 (writing all 60 phases) would create a flood risk.

### Phase 3: All 60 phase scripts (tiers 1-12)

Convert every v3.0 phase bash block into a phaseNN.sh script using `emit_result`. This is
the bulk work. No new logic — direct translation of existing commands into the script format.

**Deliverable:** All 60 phases runnable. `bash audit/audit.sh --mode full` completes without
errors (some QUIET/FAIL expected on venue-closed checks).
**Rationale:** Phase 4 (fixes, delta, report) requires the full phase set to be useful.

### Phase 4: lib/fixes.sh + auto-fix integration

Add the pre-approved fix operations. Wire `fix_trigger` check into each phase script. Add
`AUTO_FIX=1` flag to audit.sh. Test each fix in isolation before enabling in audit flow.

**Deliverable:** Running with `AUTO_FIX=1` clears MAINTENANCE_MODE on a test pod, kills orphan
PowerShell, deletes sentinel files — and emits `fix_applied` records.
**Rationale:** Fixes depend on correct phase detection (Phase 3 must be correct first).

### Phase 5: generate-report.sh + delta.sh

Build the Markdown report generator and delta comparison. Requires at least two audit runs to
test delta. Run audit twice, verify regression/improvement detection works.

**Deliverable:** `audit/reports/` contains a complete Markdown report with delta section.
**Rationale:** Report depends on all phase results; this cannot be built until Phase 3 complete.

### Phase 6: lib/notify.sh + suppress.json

Add notification dispatch (comms-link relay + INBOX.md + WhatsApp). Populate suppress.json with
the known issues from AUDIT-PROTOCOL v3.0 (Pod 8 display, AC server offline, etc.).

**Deliverable:** End-to-end: `bash audit/audit.sh --mode standard` → Bono receives structured
message → Uday receives WhatsApp summary (on FAIL).

### Phase 7: Venue-closed detection + mode refinement

Implement the venue-open/closed detection and per-mode tier selection. Wire QUIET handling into
tier-3 and tier-5 phases. Add the `--pod N` and `--tier N` targeting flags.

**Deliverable:** `bash audit/audit.sh --mode standard` at 02:00 IST (venue closed) produces QUIET
for all display/hardware phases, no false FAIL alerts to Uday.

---

## Architectural Patterns

### Pattern 1: JSONL as the Audit Bus

**What:** Every component writes and reads newline-delimited JSON (JSONL). Phase scripts write JSONL
to files. delta.sh reads JSONL. The report generator reads JSONL. Notifications read the summary
JSON. Nothing shares state through environment variables or global bash variables except for the
handful set at audit.sh startup (SESSION, PODS, AUDIT_RUN_ID).

**When to use:** All inter-component communication in this system.

**Trade-offs:** JSONL requires `jq` to be installed (it is — `jq` is already used extensively in
v3.0). Slightly more complex than plain text output, but makes the report generator trivially
grep-able and lets Bono's comms-link relay consume structured data.

**Example:**
```bash
# phase07.sh — emit result
VIOLATIONS=$(curl -s http://192.168.31.23:8080/api/v1/fleet/health | \
  jq '[.pods[] | select(.violation_count_24h >= 100)] | length')

if [ "$VIOLATIONS" -gt 0 ]; then
  emit_result 7 "FAIL" "violation_count_24h: 100 on $VIOLATIONS pods" "P1" \
    "$(curl -s http://192.168.31.23:8080/api/v1/fleet/health | \
       jq '.pods[] | {pod_number, violation_count_24h}')"
else
  emit_result 7 "PASS" "Process guard: all pods violation count normal" "" "null"
fi
```

### Pattern 2: Phase Scripts Are Idempotent Read-Only by Default

**What:** Phase scripts never modify system state unless `AUTO_FIX=1` is set. Running the same
phase script twice in a row produces the same JSON result and makes no changes. This means audits
can be safely re-run without side effects.

**When to use:** All 60 v3.0 phases follow this pattern. Auto-fix is opt-in.

**Trade-offs:** Slightly more code in each script (check `AUTO_FIX` before applying). Prevents
accidental state modification during `--mode quick` spot checks.

### Pattern 3: Tier-Scoped Parallelism with Pod-Level Throttle

**What:** Phase scripts within a tier run in parallel (background jobs). Pod-targeting commands
within a single phase are throttled to max 4 concurrent connections. The two levels are independent:
the tier-level parallelism runs phase scripts concurrently; the pod-level throttle controls how
many pods a single phase queries simultaneously.

**When to use:** All tier execution.

**Trade-offs:** File-based semaphores work but have ~200ms poll granularity. Sufficient for this
use case where pod queries take 1-5 seconds each. Not appropriate for sub-millisecond coordination.

### Pattern 4: Fix Whitelist with Reversibility Requirement

**What:** Every entry in `lib/fixes.sh` must document (a) exact trigger condition, (b) exact action,
(c) reversibility proof. Any fix without a clear reversal path is not auto-applicable — it goes into
the report as "manual action required."

**When to use:** Evaluating whether a new fix is safe to add to the auto-fix whitelist.

**Trade-offs:** Conservative — some clearly safe fixes may not be auto-approved on first pass.
Better to have a human approve once and add to the list than to auto-apply something that takes
down a billing session.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Server :8080 | HTTP GET/POST (curl) | Auth via SESSION token; obtained once at startup |
| Server :8090 | HTTP POST exec endpoint | No auth; LAN only; used for pod-targeting remote commands |
| Pod :8090 (rc-agent) | HTTP POST exec endpoint | Direct LAN to each pod IP |
| Pod :8091 (rc-sentry) | HTTP POST exec endpoint | Used when rc-agent down; sentry still reachable |
| Bono comms-link relay :8766 | HTTP POST (relay/exec/run) | Structured exec + INBOX.md dual channel |
| Bono VPS :8080 | HTTP GET (health check) | Verify cloud racecontrol alive |
| Ollama :11434 | Not directly used in audit | AI healer state is read from watchdog-state.json |
| go2rtc :1984 | HTTP GET /api/streams | Verify camera streams accessible (Tier 7) |
| WhatsApp (Evolution API) | Via Bono relay (indirect) | Per project_whatsapp_routing.md: all WA traffic via Bono VPS |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| audit.sh ↔ phase scripts | File (JSONL per phase) | Phase scripts are subprocess children; no shared memory |
| phase scripts ↔ lib/ | Bash source (`source lib/core.sh`) | lib/ functions are imported, not separate processes |
| phase scripts ↔ parallel throttle | File semaphore in /tmp | Per-run directory prevents cross-run interference |
| report generator ↔ results | File (JSONL read by jq) | One-way: generator reads, never writes to results/ |
| notify ↔ comms-link | HTTP (relay endpoint) | Falls back to file append if relay down |

---

## Anti-Patterns

### Anti-Pattern 1: Phase Scripts That Modify State Unconditionally

**What people do:** Write a phase script that kills orphan PowerShell processes as part of the check
itself (not in an explicit AUTO_FIX block).

**Why it's wrong:** Running `--mode quick` to spot-check fleet health would silently kill processes
on pods. Audits should be safe to run at any time without side effects. A phase that unexpectedly
modifies state violates operator trust in the audit tool.

**Do this instead:** All state modification lives in `lib/fixes.sh`. Phase scripts detect and report.
They call `apply_safe_fix` only when `AUTO_FIX=1` and the specific fix trigger condition is met.

### Anti-Pattern 2: Pod Loop Without Throttle

**What people do:** Write a tier-1 phase that loops all 8 pods in parallel background jobs without
a semaphore.

**Why it's wrong:** If 5-6 tier-1 phases each loop 8 pods in parallel, that is 40-48 simultaneous
HTTP connections to pods. Pods run on Windows 11 with rc-agent serving a single exec endpoint. rc-
agent has a 4-slot concurrency cap (`exec_slots_available`). Flooding it causes exec timeout failures
which appear as false FAIL results.

**Do this instead:** Source `lib/parallel.sh` and call `acquire_sem`/`release_sem` around each pod
connection in the loop. The 4-concurrent limit matches rc-agent's capacity.

### Anti-Pattern 3: Hardcoded Phase Numbers in Report Logic

**What people do:** The report generator has `if [ $PHASE -eq 7 ]; then echo "Check process guard"`.

**Why it's wrong:** When phases are reordered, renumbered, or new phases are added, the hardcoded
references become wrong. Phase 7 in v4.0 may not be Process Guard if phases are renumbered.

**Do this instead:** Phase scripts embed their own metadata in the JSON they emit (`phase_name`,
`tier`). The report generator reads metadata from the JSON — it never hardcodes phase-to-name
mappings. The phase number is just an ID; the `phase_name` field is the human-readable label.

### Anti-Pattern 4: Notification Before Report

**What people do:** Send WhatsApp notification immediately when the first FAIL is detected
(mid-audit, before all phases complete).

**Why it's wrong:** (a) Multiple FAIL phases in the same audit would send multiple WhatsApp
messages. (b) A FAIL detected in phase 3 may be resolved by an auto-fix applied in phase 3,
making the notification incorrect. (c) Uday receives an alert at 3 AM and does not know the
full scope.

**Do this instead:** Notification is the last step, after all tiers, all fixes, delta computation,
and report generation complete. The notification message includes full summary counts and the
report path.

### Anti-Pattern 5: Treating QUIET as FAIL

**What people do:** Display/hardware phases return FAIL when venue is closed and Edge kiosk is not
running.

**Why it's wrong:** At 2 AM when venue is closed, pods are powered down or locked. Edge is not
running. This is expected. Treating it as FAIL means every overnight audit triggers a P1 alert
to Uday.

**Do this instead:** Tier 3 (Display/UX) and Tier 5 (Games/Hardware) phase scripts check the
`$VENUE_OPEN` variable. When `VENUE_OPEN=false`, they emit `status: "QUIET"` rather than
`status: "FAIL"`. QUIET results are counted separately, never trigger WhatsApp alerts, and appear
under a separate "Venue-Closed Checks" section in the report.

---

## Scaling Considerations

This system has fixed scale: 1 server, 8 pods, 1 Bono VPS. It does not need to scale beyond this.
The relevant scaling questions are performance (audit duration) and reliability (what happens when
infrastructure is partially down).

| Concern | Current (8 pods) | If pods expand to 16 | Notes |
|---------|------------------|----------------------|-------|
| Audit duration | ~3-5 min (parallel) | ~5-8 min (semaphore bottleneck) | Still well within acceptable range |
| Semaphore limit | 4 concurrent | Increase to 6-8 | Update `MAX_PARALLEL` constant in parallel.sh |
| Result file size | ~50KB per audit | ~100KB per audit | Negligible; JSONL gzips well |
| Report size | ~20KB Markdown | ~40KB Markdown | git-committed history grows slowly |

The more likely scaling pressure is phase count growth (v4.0 adds delta/feature-flag phases beyond
60). The numbering scheme (61+) handles this without renumbering existing phases.

---

## Sources

- Direct analysis: `racecontrol/AUDIT-PROTOCOL.md` v3.0 (1928 lines, 60 phases, 18 tiers)
- Direct analysis: `.planning/PROJECT.md` — v23.0 milestone spec (constraints, target features)
- `CLAUDE.md` standing rules — parallel exec limits, comms-link dual-channel requirement,
  WhatsApp routing (via Bono VPS), JSONL file format, auto-push requirement
- `project_whatsapp_routing.md` (memory) — WA traffic routes via Bono Evolution API, not venue tunnel
- Existing comms-link relay docs (relay/exec/run, relay/chain/run, health endpoint)
- v22.0 gate-check.sh pattern — existing bash-only gate script this audit runner extends

---

*Architecture research for: v23.0 Automated Fleet Audit System*
*Researched: 2026-03-25*
