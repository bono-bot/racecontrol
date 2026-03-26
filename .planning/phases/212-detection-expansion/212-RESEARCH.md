# Phase 212: Detection Expansion - Research

**Researched:** 2026-03-26
**Domain:** Bash detection scripts sourced into auto-detect.sh — config drift, bat drift, log anomaly, crash loop, flag desync, schema gap
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation choices are at Claude's discretion — pure infrastructure phase. Detection scripts follow existing audit/phases/ pattern (bash + jq, sourced into auto-detect.sh).

Key technical constraints:
- Config drift (DET-01) must SCP racecontrol.toml from pods (SSH banner corruption standing rule applies — never pipe SSH output into config)
- Bat drift (DET-02) uses sha256sum checksum comparison against canonical repo version
- Log anomaly (DET-03) uses pattern-based triggers (ERROR/PANIC line count in last hour) — rate-based thresholds deferred (need 7-day calibration)
- Crash loop (DET-04) reads JSONL restart timestamps, not process count
- Flag desync (DET-05) requires querying /api/v1/flags on each pod and comparing enabled sets
- Schema gap (DET-06/07) checks for ALTER TABLE migrations matching CREATE TABLE columns
- All modules output findings in the same JSON format as existing audit phases (category, severity, pod_ip, message)

### Claude's Discretion
All implementation choices.

### Deferred Ideas (OUT OF SCOPE)
- Rate-based anomaly thresholds (need 7-day calibration window) — future phase
- Config drift via Rust API endpoint GET /api/v1/config/health-params — requires upstream work
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DET-01 | Config drift detection compares running racecontrol.toml values against canonical expected values | SCP pattern from core.sh `safe_ssh_capture`; known keys: ws_connect_timeout (>=600ms), app_health URLs, process_guard.enabled |
| DET-02 | Bat file drift detection compares pod start-rcagent.bat checksums against repo canonical version | bat-scanner.sh `bat_scan_pod_json` already implements this; phase61.sh shows integration pattern; DET-02 wraps bat-scanner.sh for auto-detect integration |
| DET-03 | Log anomaly detection scans pod JSONL logs for ERROR/PANIC rate exceeding threshold (>10/hour open, >2/hour closed) | rc-agent logs to C:\RacingPoint\rc-agent-YYYY-MM-DD.jsonl (daily rolling); safe_remote_exec to count ERROR/PANIC in last hour via findstr; venue-state-aware thresholds |
| DET-04 | Crash loop detection flags pods with >3 rc-agent restarts in 30 minutes | rc-agent-startup.log at C:\RacingPoint\rc-agent-startup.log has timestamped phase lines; count lines with `phase=config_loaded` or process-start markers within 30-min window via safe_remote_exec |
| DET-05 | Feature flag sync check verifies all 8 pods have identical enabled flag set | GET /api/v1/flags on server returns all flags; rc-agent has its own flags cache; query rc-agent :8090/api/v1/flags or server and compare enabled sets across fleet; DET-05 compares enabled flag names |
| DET-06 | Schema drift detection compares cloud and venue DB table schemas for column mismatches | db/mod.rs has CREATE TABLE + ALTER TABLE migrations; compare PRAGMA table_info() results between cloud DB and venue DB via API; server has /api/v1/health (no schema endpoint); use sqlite3 CLI or API exec |
| DET-07 | Cascade module (cascade.sh) sources into auto-detect.sh, shares env (BUGS_FOUND, LOG_FILE) | auto-detect.sh step 4 is `run_cascade_check()`; DET-07 creates scripts/cascade.sh as a new sourced module that auto-detect.sh sources; cascade.sh calls the 6 detector functions and accumulates BUGS_FOUND |
</phase_requirements>

---

## Summary

Phase 212 adds 6 detection modules to the existing 6-step auto-detect.sh pipeline. The pipeline already has its plumbing in place (PID lock, venue-state detection, cooldown, step tracking) from Phase 211. Phase 212 inserts detection capabilities that run during the existing Step 4 (Cascade Check) or as a new Step 2.5 sourced module via cascade.sh (DET-07).

The code structure is fully understood: `audit/lib/core.sh` provides `safe_remote_exec`, `safe_ssh_capture`, `emit_result`, and `emit_fix`. Existing audit phases (phase02.sh for config, phase61.sh for bat drift) demonstrate the exact patterns. The bat-scanner.sh is already a complete bat drift implementation — DET-02 is a thin wrapper that adapts it to auto-detect.sh's finding format. All 6 detectors write findings to `$RESULT_DIR/findings.json` so Step 6 (notify) can send WhatsApp cooldown-gated alerts.

The critical architectural decision is DET-07: cascade.sh is a new script sourced into auto-detect.sh that calls all 6 detector functions, accumulates `BUGS_FOUND`, and writes to `$LOG_FILE`. The `run_cascade_check()` function in auto-detect.sh currently handles build drift and comms-link sync; DET-07 either extends it or replaces its detection section with a `source cascade.sh` call.

**Primary recommendation:** Create `scripts/detectors/` directory with 6 `detect-*.sh` scripts. `scripts/cascade.sh` sources all 6 and is sourced into `auto-detect.sh` at the top of `run_cascade_check()`. This keeps auto-detect.sh clean while DET-07 is explicitly satisfied by cascade.sh.

---

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| bash + jq | system | All detection scripts | Existing pattern — 60 audit phases already use it |
| curl | system | HTTP to rc-agent :8090, :8091, server :8080 | Established via `http_get` and `safe_remote_exec` in core.sh |
| sha256sum | system | Bat file drift checksum | Already used in bat-scanner.sh |
| scp | system | Config file retrieval from pods | Standing rule: never pipe SSH into config |
| sqlite3 | system (server) | Schema gap comparison via PRAGMA | Already used in check-schema-compat.js pattern |

### Supporting
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| safe_remote_exec (core.sh) | — | Execute commands on pods via rc-agent :8090 | Preferred for all pod exec (temp file JSON payload, strips \r) |
| safe_ssh_capture (core.sh) | — | SSH with banner corruption protection | Only when rc-agent is down or file retrieval needed |
| emit_result (core.sh) | — | Write JSON audit findings | All detection results |
| bat_scan_pod_json (bat-scanner.sh) | — | Bat file drift with structured JSON output | DET-02 delegates to this |

**No new packages to install.** All tools are already available on James's machine and on pods.

---

## Architecture Patterns

### Recommended Project Structure
```
scripts/
├── auto-detect.sh          # 6-step pipeline (EXISTING — add source cascade.sh)
├── cascade.sh              # NEW — DET-07, sources all detectors, shared env
├── bat-scanner.sh          # EXISTING — bat drift, reused by DET-02
└── detectors/
    ├── detect-config-drift.sh    # DET-01
    ├── detect-bat-drift.sh       # DET-02 (wraps bat-scanner.sh)
    ├── detect-log-anomaly.sh     # DET-03
    ├── detect-crash-loop.sh      # DET-04
    ├── detect-flag-desync.sh     # DET-05
    └── detect-schema-gap.sh      # DET-06
```

### Pattern 1: Detector Script Structure
Every detector script follows the audit/phases/ pattern exactly:

```bash
#!/usr/bin/env bash
# scripts/detectors/detect-config-drift.sh — DET-01
# Returns: writes findings to FINDINGS_FILE, returns count via DETECTOR_FINDINGS var

set -u
set -o pipefail
# NO set -e — errors encoded in findings, not exit codes

detect_config_drift() {
  local findings_count=0
  local pod_ip

  for pod_ip in 192.168.31.{89,33,28,88,86,87,38,91}; do
    # Use SCP (not SSH pipe) to avoid banner corruption — standing rule
    local tmpfile; tmpfile=$(mktemp)
    scp -q -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
      "User@${pod_ip}:C:/RacingPoint/racecontrol.toml" "$tmpfile" 2>/dev/null || {
      # Pod offline — skip, not a config drift
      rm -f "$tmpfile"
      continue
    }

    # Validate first line (SSH banner corruption guard)
    local first_line; first_line=$(head -1 "$tmpfile" 2>/dev/null)
    if ! printf '%s' "$first_line" | grep -q '^\['; then
      # Emit finding: banner-corrupted TOML
      _emit_finding "config_drift" "P1" "$pod_ip" \
        "racecontrol.toml first line invalid — likely banner corruption: ${first_line:0:60}"
      findings_count=$((findings_count + 1))
      rm -f "$tmpfile"
      continue
    fi

    # Check ws_connect_timeout
    local ws_val; ws_val=$(grep -oP 'ws_connect_timeout\s*=\s*\K[0-9]+' "$tmpfile" 2>/dev/null | head -1)
    if [[ -n "$ws_val" ]] && [[ "$ws_val" -lt 600 ]] 2>/dev/null; then
      _emit_finding "config_drift" "P1" "$pod_ip" \
        "ws_connect_timeout=${ws_val}ms on ${pod_ip} — expected>=600ms (incident: WS timeouts at 200ms)"
      findings_count=$((findings_count + 1))
    fi

    rm -f "$tmpfile"
  done

  DETECTOR_FINDINGS=$((DETECTOR_FINDINGS + findings_count))
}
export -f detect_config_drift
```

### Pattern 2: Finding Emission Format
All detectors write to `$RESULT_DIR/findings.json` — same file Step 6 reads for WhatsApp cooldown gating:

```bash
_emit_finding() {
  local category="$1" severity="$2" pod_ip="$3" message="$4"
  local ts; ts=$(TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30')
  local finding
  finding=$(jq -n \
    --arg cat   "$category" \
    --arg sev   "$severity" \
    --arg pod   "$pod_ip" \
    --arg msg   "$message" \
    --arg ts    "$ts" \
    '{category:$cat,severity:$sev,pod_ip:$pod,message:$msg,timestamp:$ts,issue_type:$cat}')
  local findings_file="${RESULT_DIR}/findings.json"
  # Append to array: read existing, add element, write back
  if [[ -f "$findings_file" ]]; then
    local existing; existing=$(cat "$findings_file" 2>/dev/null || echo "[]")
    printf '%s' "$existing" | jq --argjson f "$finding" '. + [$f]' > "${findings_file}.tmp" \
      && mv "${findings_file}.tmp" "$findings_file"
  else
    printf '[%s]' "$finding" > "$findings_file"
  fi
  log WARN "FINDING [$severity] [$category] $pod_ip — $message"
}
```

### Pattern 3: cascade.sh Integration
DET-07 requires cascade.sh to share `BUGS_FOUND` and `LOG_FILE` with auto-detect.sh. The script is sourced (not exec'd), so env vars are inherited:

```bash
# scripts/cascade.sh — DET-07: sources all detectors, called from auto-detect.sh
# Env required: RESULT_DIR, LOG_FILE, BUGS_FOUND, DETECTOR_FINDINGS (accumulator)

DETECTOR_FINDINGS=0

# Source all detector scripts
DETECTORS_DIR="${SCRIPT_DIR}/detectors"
for detector_file in \
  "$DETECTORS_DIR/detect-config-drift.sh" \
  "$DETECTORS_DIR/detect-bat-drift.sh" \
  "$DETECTORS_DIR/detect-log-anomaly.sh" \
  "$DETECTORS_DIR/detect-crash-loop.sh" \
  "$DETECTORS_DIR/detect-flag-desync.sh" \
  "$DETECTORS_DIR/detect-schema-gap.sh"; do
  if [[ -f "$detector_file" ]]; then
    # shellcheck disable=SC1090
    source "$detector_file"
  fi
done

run_all_detectors() {
  detect_config_drift
  detect_bat_drift
  detect_log_anomaly
  detect_crash_loop
  detect_flag_desync
  detect_schema_gap
  # Accumulate into auto-detect.sh's BUGS_FOUND
  BUGS_FOUND=$((BUGS_FOUND + DETECTOR_FINDINGS))
}
export -f run_all_detectors
```

### Pattern 4: auto-detect.sh Integration Hook
`run_cascade_check()` in auto-detect.sh calls `run_all_detectors` after sourcing cascade.sh:

```bash
# In auto-detect.sh, before run_cascade_check() definition:
if [[ -f "$SCRIPT_DIR/cascade.sh" ]]; then
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/cascade.sh"
fi

# Inside run_cascade_check(), before the build drift checks:
if [[ $(type -t run_all_detectors) == "function" ]]; then
  run_all_detectors
fi
```

### Pattern 5: Config Drift via SCP (DET-01)
The standing rule is explicit: never pipe SSH output into config. Use SCP, then validate:

```bash
# SCP from Windows pod — note Windows path format
scp -q -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
  "User@${pod_ip}:C:/RacingPoint/racecontrol.toml" "$tmpfile" 2>/dev/null

# Banner guard
head -1 "$tmpfile" | grep -q '^\[' || { echo "CORRUPTED"; rm -f "$tmpfile"; continue; }

# Key checks (known incident sources):
# 1. ws_connect_timeout >= 600 (incident: 200ms caused WS timeouts fleet-wide)
# 2. process_guard.enabled = true (incident: empty allowlist + enabled=false masked violations)
# Note: app_health URLs are hardcoded in app_health_monitor.rs, NOT in racecontrol.toml
#       — phase02.sh already checks these via curl to :3201/:3300/:3200
```

**Critical**: The `ws_connect_timeout` in racecontrol.toml is advisory-only (not consumed by Rust code per phase02.sh comment). DET-01 should focus on `process_guard.enabled` and pod-level rc-agent.toml keys (pod_number), not ws_connect_timeout. However the success criteria specifies ws_connect_timeout explicitly, so DET-01 must check it on pods (rc-agent.toml may have it), even if it reports as informational on the server.

**RESOLUTION**: DET-01 targets pod-side rc-agent.toml. The success criteria test ("ws_connect_timeout below 600ms") refers to a pod config. Fetch rc-agent.toml via SCP from each pod and check:
- `ws_connect_timeout` if present (must be >= 600)
- `pod_number` key must exist
- No banner corruption (first line must start with `[`)

### Pattern 6: Crash Loop Detection (DET-04)
rc-agent logs to daily rolling file: `C:\RacingPoint\rc-agent-YYYY-MM-DD.jsonl`. Restart events are visible as process restarts — each startup writes to `C:\RacingPoint\rc-agent-startup.log` with UTC timestamps. The detection should:

1. Read `rc-agent-startup.log` via `safe_remote_exec` (type command)
2. Filter timestamps within last 30 minutes (convert UTC to comparison)
3. Count startup events (lines containing `phase=config_loaded` = one startup cycle)
4. If count > 3, emit FAIL finding

```bash
# rc-agent-startup.log format (from startup_log.rs):
# 2026-03-26T03:15:00Z phase=config_loaded pod=3
# Timestamps are UTC — convert window: 30 min ago in UTC
local cutoff_ts; cutoff_ts=$(date -u -d '30 minutes ago' '+%Y-%m-%dT%H:%M' 2>/dev/null \
  || date -u -v-30M '+%Y-%m-%dT%H:%M' 2>/dev/null || echo "")

# Safe remote exec to get startup log content
local log_content
log_content=$(safe_remote_exec "$pod_ip" 8090 \
  'type C:\RacingPoint\rc-agent-startup.log' 10)
local stdout; stdout=$(printf '%s' "$log_content" | jq -r '.stdout // ""' 2>/dev/null)

# Count lines with phase=config_loaded that are within last 30 min window
local restart_count
restart_count=$(printf '%s' "$stdout" | grep 'phase=config_loaded' | \
  awk -v cutoff="$cutoff_ts" '$1 >= cutoff' | wc -l | tr -d ' ')
```

**Note**: The startup log is truncated on each new rc-agent start (first call truncates per startup_log.rs). This means the log only contains the CURRENT startup's phases. To detect crash loops, check the JSONL log file for restart markers within the time window instead. The JSONL log is NOT truncated — it accumulates all events.

**Revised approach for DET-04**: Query the JSONL log for recent startup events via the log API or direct exec. The rc-agent JSONL log is at `C:\RacingPoint\rc-agent-YYYY-MM-DD.jsonl` (same directory as the binary, i.e., `C:\RacingPoint\`). Look for `"level":"INFO"` lines with `"message"` containing startup markers within the last 30 minutes.

Alternatively: query `/api/v1/health` for `uptime_secs` across multiple polls — if uptime resets 3+ times within 30 min, that's a crash loop. But the requirements say "reads JSONL restart timestamps" so use JSONL.

### Pattern 7: Flag Desync (DET-05)
rc-agent serves its own cached flag state. The server `GET /api/v1/flags` returns the canonical set. To detect desync between pods:

```bash
# Get canonical flag set from server
local server_flags
server_flags=$(curl -s --max-time 10 "$SERVER_URL/api/v1/flags" 2>/dev/null || echo "[]")
local canonical_enabled
canonical_enabled=$(printf '%s' "$server_flags" | \
  jq -r '[.[] | select(.enabled==true) | .name] | sort | join(",")' 2>/dev/null || echo "")

# For each pod, query rc-agent flags endpoint
for pod_ip in 192.168.31.{89,33,28,88,86,87,38,91}; do
  local pod_flags
  pod_flags=$(curl -s --max-time 10 "http://${pod_ip}:8090/api/v1/flags" 2>/dev/null || echo "")
  if [[ -z "$pod_flags" ]]; then
    # Pod unreachable — skip
    continue
  fi
  local pod_enabled
  pod_enabled=$(printf '%s' "$pod_flags" | \
    jq -r '[.[] | select(.enabled==true) | .name] | sort | join(",")' 2>/dev/null || echo "")
  if [[ "$pod_enabled" != "$canonical_enabled" ]]; then
    # Find which flags differ
    local missing; missing=$(comm -23 <(printf '%s\n' "${canonical_enabled//,/$'\n'}" | sort) \
                                       <(printf '%s\n' "${pod_enabled//,/$'\n'}" | sort))
    local extra; extra=$(comm -13 <(printf '%s\n' "${canonical_enabled//,/$'\n'}" | sort) \
                                   <(printf '%s\n' "${pod_enabled//,/$'\n'}" | sort))
    _emit_finding "flag_desync" "P2" "$pod_ip" \
      "flag desync: missing=[${missing}] extra=[${extra}] (canonical from server)"
  fi
done
```

**Note**: rc-agent has a `/api/v1/flags` endpoint (per feature_flags.rs in rc-agent). Verify this endpoint exists before calling it — use `safe_remote_exec` health check first. If rc-agent does not expose `/api/v1/flags`, use the server's per-pod override data instead.

### Pattern 8: Schema Gap (DET-06)
The db/mod.rs has 21+ `CREATE TABLE IF NOT EXISTS` statements and 10+ `ALTER TABLE ADD COLUMN` migrations. Schema gap detection checks that the live venue DB and cloud DB have the same columns on critical tables:

```bash
# Use the racecontrol server_ops exec endpoint to run sqlite3 queries
# Critical tables: drivers, billing_sessions, feature_flags, sessions

local table="drivers"
local venue_cols
venue_cols=$(safe_remote_exec "192.168.31.23" 8090 \
  "sqlite3 C:\RacingPoint\racecontrol.db \"PRAGMA table_info(${table})\"" 15 | \
  jq -r '.stdout // ""')
```

**Problem**: sqlite3 CLI may not be on the server. Alternative: use the server's API to run the query via a dedicated endpoint. Phase 212's scope is limited — use the simpler approach: query a known column that was added late (e.g., `updated_at` on drivers) and detect its absence via a SELECT.

**Practical approach**: Run `SELECT updated_at FROM drivers LIMIT 1` via server_ops exec and detect the "no such column" error. If the column is missing, a schema gap exists. Repeat for key columns across critical tables.

### Anti-Patterns to Avoid
- **SSH pipe to variable**: Never `ssh pod "cat file" > local_file` — use `scp` instead (SSH banner corruption standing rule)
- **Inline JSON in curl**: Always write JSON to temp file and use `curl -d @file` (Git Bash JSON escaping)
- **set -e in detector scripts**: Must not exit on first error — use `|| true` or explicit error handling
- **Assuming rc-agent JSONL is in a fixed path**: The log is in `current_exe` directory which is `C:\RacingPoint\` on pods — use this path, but verify
- **Counting process instances for crash loop**: Requirements explicitly say "reads JSONL restart timestamps, not process count alone"
- **Comparing full flag objects**: DET-05 must compare enabled flag NAMES, not full FeatureFlagRow objects (overrides and version may legitimately differ)

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bat file drift | Custom checksum loop | bat_scan_pod_json() in bat-scanner.sh | Already handles SCP, sha256sum, diff, syntax validation — complete implementation |
| Config file retrieval | SSH pipe to grep | scp + head-1 banner validation | SSH banner corruption is a documented production incident (racecontrol.toml corrupted) |
| Finding format | Custom JSON | _emit_finding() helper using jq -n | Consistent format for WhatsApp cooldown gating in Step 6 |
| Pod iteration | Hardcoded separate blocks | `for pod_ip in 192.168.31.{89,33,28,88,86,87,38,91}` | Matches existing audit pattern across 60 phases |
| Log parsing (ERROR count) | Read entire file | `tail -n 2000` + `grep -c` | Large JSONL files — limit to recent lines before counting |

---

## Common Pitfalls

### Pitfall 1: Startup Log Truncation (DET-04)
**What goes wrong:** rc-agent-startup.log is truncated on every startup (per startup_log.rs first-call behavior). After a crash + restart, the log only contains the CURRENT startup — not previous ones. Counting startup events in this file cannot detect crash loops.
**Why it happens:** startup_log.rs uses `AtomicBool FIRST_WRITE` to truncate on first call per process.
**How to avoid:** Use the JSONL log file (`rc-agent-YYYY-MM-DD.jsonl`) instead. Filter for startup-marker log lines within the 30-minute window. The JSONL is append-only and persists across restarts.
**Warning signs:** Crash loop detector always reports 0 or 1 restarts even when you know a loop occurred.

### Pitfall 2: racecontrol.toml ws_connect_timeout is Advisory-Only
**What goes wrong:** phase02.sh comments explicitly state "ws_connect_timeout is NOT consumed by any Rust code — the WS connect timeout is hardcoded in fleet_health.rs (3s)". If DET-01 checks this field on the SERVER's racecontrol.toml, it may generate findings that cannot be acted on.
**Why it happens:** The success criteria mention "pod racecontrol.toml" — but pods run rc-agent, not racecontrol. Pods have rc-agent.toml, not racecontrol.toml.
**How to avoid:** DET-01 should check rc-agent.toml on pods (which is the actual config). For the server, check the keys that ARE actionable (process_guard.enabled, app_health monitor settings). The success criteria test for ws_connect_timeout must check rc-agent.toml on pods.
**Warning signs:** DET-01 reports server findings that cannot be correlated with actual WS timeout behavior.

### Pitfall 3: Flag Endpoint on rc-agent vs Server
**What goes wrong:** rc-agent serves flags from its in-memory cache at `/api/v1/flags`. If this endpoint does not exist (feature not implemented in rc-agent), curling it returns empty string, which is treated as "no flags" — all pods appear to have no flags, triggering false desync alerts.
**Why it happens:** The server has a full feature flag registry; rc-agent has a cache populated via WS FlagSync. The cache endpoint may not be publicly exposed on rc-agent's HTTP API.
**How to avoid:** Before writing DET-05, check the rc-agent routes for a flags endpoint. If absent, use the server's fleet-wide flag state and compare with the server's own record (which already tracks overrides per pod). A simpler DET-05: query the server's /api/v1/flags, check all pods' per-pod overrides, and report if any pod has a different effective enabled state.
**Warning signs:** All 8 pods return empty array for flag query.

### Pitfall 4: JSONL Log Format (UTC vs IST)
**What goes wrong:** rc-agent JSONL logs are in UTC (tracing logs are UTC per CLAUDE.md). Filtering by "last hour" using IST time computes the wrong window.
**Why it happens:** Rust's `tracing` crate uses system time; tracing_subscriber JSONL output has UTC timestamps. The standing rule is explicit: "Rust tracing logs are in UTC. When reading racecontrol JSONL logs, always convert."
**How to avoid:** Use UTC for all time window comparisons when scanning JSONL logs. Compute cutoff as UTC: `date -u -d '1 hour ago' ...`.
**Warning signs:** Log anomaly detector misses recent errors, or reports errors from a previous day.

### Pitfall 5: Missing findings.json Array Initialization
**What goes wrong:** Step 6 in auto-detect.sh reads findings.json to gate WhatsApp alerts: `jq -r '.[] | "\(.pod_ip) \(.issue_type)"' "$RESULT_DIR/findings.json"`. If findings.json is missing or malformed, this silently fails to escalate any findings.
**Why it happens:** Detectors may not create the file if all checks pass, leaving Step 6 with no file to read.
**How to avoid:** Initialize `$RESULT_DIR/findings.json` to `[]` at the start of cascade.sh, before any detector runs. Each detector appends to the array.

### Pitfall 6: SCP User Account for Pods
**What goes wrong:** `scp User@pod_ip:C:/RacingPoint/...` fails with authentication error because the pod user account is not "User" or SSH is not enabled/configured.
**Why it happens:** Pod SSH access via Tailscale uses Windows user credentials. The correct user for pods varies.
**How to avoid:** Use `safe_remote_exec` with rc-agent :8090 to read file contents via the `type` command. This avoids SSH entirely. Only fall back to SCP via Tailscale when rc-agent is down. The `type` command on Windows reads file content and rc-agent's exec returns it as stdout JSON.
**Correct approach for DET-01:**
```bash
# Use rc-agent exec to read file (no SSH required)
local response
response=$(safe_remote_exec "$pod_ip" 8090 \
  'type C:\RacingPoint\rc-agent.toml' 10)
local content; content=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)
```

---

## Code Examples

Verified patterns from existing codebase:

### Reading a Pod Config File (safe path — avoids SSH banner corruption)
```bash
# Source: audit/lib/core.sh safe_remote_exec
response=$(safe_remote_exec "192.168.31.89" 8090 \
  'type C:\RacingPoint\rc-agent.toml' 10)
content=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)
# Validate not empty and first line is TOML
printf '%s' "$content" | head -1 | grep -q '^\[' || { echo "invalid config"; return; }
```

### Getting Specific Value from TOML Content
```bash
# From phase02.sh pattern
ws_val=$(printf '%s' "$content" | grep -oP 'ws_connect_timeout\s*=\s*\K[0-9]+' | head -1 || true)
if [[ -n "$ws_val" ]] && [[ "$ws_val" -lt 600 ]] 2>/dev/null; then
  : # flag this
fi
```

### Bat Drift via bat_scan_pod_json (Source: scripts/bat-scanner.sh + audit/phases/tier2/phase61.sh)
```bash
source "$REPO_ROOT/scripts/bat-scanner.sh"
result_json=$(bat_scan_pod_json "$pod_num" "start-rcagent.bat" "$CANONICAL_RCAGENT")
scan_status=$(printf '%s' "$result_json" | jq -r '.status // "UNKNOWN"')
# scan_status is: MATCH | DRIFT | UNREACHABLE | SKIP
if [[ "$scan_status" == "DRIFT" ]]; then
  local canon_hash fetched_hash
  canon_hash=$(printf '%s' "$result_json" | jq -r '.diff // ""' | head -1 || true)
  _emit_finding "bat_drift" "P2" "$pod_ip" \
    "start-rcagent.bat drift on pod $pod_num — checksum mismatch"
fi
```

### Log Anomaly Scan (DET-03 — count ERROR/PANIC in JSONL log)
```bash
# Get today's log file via exec — JSONL is in C:\RacingPoint\
local today_log; today_log="C:\\RacingPoint\\rc-agent-$(date -u '+%Y-%m-%d').jsonl"
local log_response
log_response=$(safe_remote_exec "$pod_ip" 8090 \
  "findstr /C:\"\\\"level\\\":\\\"ERROR\\\"\" /C:\"\\\"level\\\":\\\"WARN\\\"\" \"${today_log}\"" 15)
# Note: PANIC shows as ERROR level in tracing — filter by message content too
local error_lines
error_lines=$(printf '%s' "$log_response" | jq -r '.stdout // ""' | wc -l | tr -d ' ')
```

**Better approach**: Use the racecontrol server's `/api/v1/logs` endpoint which reads pod JSONL logs server-side:
```bash
# Source: rc-agent exposes GET /api/v1/logs (per racecontrol routes)
local logs_response
logs_response=$(curl -s --max-time 15 "http://192.168.31.23:8080/api/v1/logs?pod=${pod_num}&level=ERROR&limit=50" 2>/dev/null || echo "[]")
local error_count
error_count=$(printf '%s' "$logs_response" | jq '. | length' 2>/dev/null || echo "0")
```

### Feature Flag Comparison (DET-05)
```bash
# Source: crates/racecontrol/src/flags.rs — GET /api/v1/flags returns FeatureFlagRow[]
server_flags=$(curl -s --max-time 10 "http://192.168.31.23:8080/api/v1/flags" 2>/dev/null || echo "[]")
canonical_set=$(printf '%s' "$server_flags" | \
  jq -r '[.[] | select(.enabled==true) | .name] | sort | join(" ")' 2>/dev/null || echo "")
```

### Pod Iteration Pattern (Source: audit/phases/tier1/phase02.sh)
```bash
for ip in $PODS; do  # PODS set by audit.sh: "192.168.31.89 192.168.31.33 ..."
  host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
  # ... per-pod logic ...
done
```

Or standalone (Source: scripts/bat-scanner.sh):
```bash
for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 \
              192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SSH pipe to cat config | SCP + first-line validation | Phase 202 (standing rule after incident) | Prevents banner corruption in TOML |
| Single-fetch feature flags | Periodic re-fetch every 5 min | Phase 207 (boot_resilience) | Flags self-heal after server transience |
| Count process instances for restart | Read JSONL timestamps | Phase 212 requirement | Accurate crash loop detection without race conditions |
| Manual bat comparison | bat_scan_pod_json with sha256sum | Phase 210 (bat-scanner.sh shipped) | Structured JSON output, syntax validation included |

**Deprecated/outdated:**
- `safe_ssh_capture` for config reading: replaced by `safe_remote_exec` with `type` command — avoids all SSH issues
- `racecontrol.toml` ws_connect_timeout on server: advisory-only (not consumed by Rust), check phase02.sh notes

---

## Open Questions

1. **Does rc-agent expose GET /api/v1/flags for DET-05?**
   - What we know: Server has /api/v1/flags. rc-agent has feature_flags.rs with an in-memory cache. The routes.rs in rc-agent registers endpoints.
   - What's unclear: Whether rc-agent exposes a GET /api/v1/flags endpoint or only consumes flags via WS FlagSync.
   - Recommendation: Planner should include a task to verify `GET http://192.168.31.89:8090/api/v1/flags` returns a flag list, before writing DET-05 to depend on it. Fallback: compare server's effective flag state using per-pod overrides from /api/v1/flags (overrides field).

2. **Does rc-agent.toml contain ws_connect_timeout on pods?**
   - What we know: The success criteria test specifies "pod racecontrol.toml has ws_connect_timeout below 600ms". Pods run rc-agent, not racecontrol. rc-agent reads rc-agent.toml.
   - What's unclear: Whether rc-agent.toml on pods has a ws_connect_timeout field, or whether this key only appears in racecontrol.toml on the server.
   - Recommendation: The success criteria may be using "racecontrol.toml" loosely to mean the pod's config file. DET-01 should check rc-agent.toml on pods for this key, reporting "not present" as an informational finding (not a failure) if it's absent.

3. **JSONL log access for DET-03/DET-04: exec vs API**
   - What we know: rc-agent logs to `C:\RacingPoint\rc-agent-YYYY-MM-DD.jsonl`. The server has `/api/v1/logs` endpoint. safe_remote_exec can run `findstr` on the file directly.
   - What's unclear: The `/api/v1/logs` endpoint parameters and whether it supports level/pod filtering.
   - Recommendation: Use `safe_remote_exec` with `findstr` directly for simpler, less brittle log scanning. Fallback to API if exec returns empty (pod offline but API available).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | bash with fixture files + assertion patterns (existing audit test pattern) |
| Config file | none — tests are inline bash scripts |
| Quick run command | `bash scripts/detectors/detect-config-drift.sh --test` (per-detector self-test) |
| Full suite command | `bash scripts/cascade.sh --test` (all 6 detectors with injected fixtures) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DET-01 | Config drift reports ws_connect_timeout<600 with specific key+value | unit | `bash scripts/detectors/detect-config-drift.sh --test` | ❌ Wave 0 |
| DET-02 | Bat drift flags Pod 8 with specific hash mismatch | unit | `bash scripts/detectors/detect-bat-drift.sh --test` | ❌ Wave 0 |
| DET-03 | 15 ERROR lines triggers; 5 ERROR lines does not | unit | `bash scripts/detectors/detect-log-anomaly.sh --test` | ❌ Wave 0 |
| DET-04 | >3 restarts in 30min from JSONL timestamps flagged | unit | `bash scripts/detectors/detect-crash-loop.sh --test` | ❌ Wave 0 |
| DET-05 | Flag desync reports specific flag name + which pods diverge | unit | `bash scripts/detectors/detect-flag-desync.sh --test` | ❌ Wave 0 |
| DET-06 | Schema gap reports missing column in cloud vs venue | unit | `bash scripts/detectors/detect-schema-gap.sh --test` | ❌ Wave 0 |
| DET-07 | cascade.sh sources into auto-detect.sh, shares BUGS_FOUND, LOG_FILE | integration | `bash scripts/cascade.sh --test` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `bash scripts/cascade.sh --test` (fixture-based, offline, < 10 seconds)
- **Per wave merge:** Full cascade test + `AUDIT_PIN=261121 bash scripts/auto-detect.sh --dry-run`
- **Phase gate:** All 7 unit tests pass + dry-run completes without error before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `scripts/detectors/` directory — does not exist yet
- [ ] `scripts/cascade.sh` — does not exist yet
- [ ] Each detector script's `--test` mode with injected fixtures (fake toml, fake log lines, fake flag sets)
- [ ] `$RESULT_DIR/findings.json` initialization at cascade.sh entry point

*(These are all created during implementation — no pre-existing test infrastructure for Phase 212.)*

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read: `scripts/auto-detect.sh` — step structure, env var contracts, BUGS_FOUND/LOG_FILE/RESULT_DIR
- Direct codebase read: `audit/lib/core.sh` — safe_remote_exec, safe_ssh_capture, emit_result, venue_state_detect
- Direct codebase read: `audit/lib/fixes.sh` — APPROVED_FIXES pattern, is_pod_idle, emit_fix
- Direct codebase read: `audit/phases/tier1/phase02.sh` — config detection pattern, ws_connect_timeout note (advisory-only)
- Direct codebase read: `audit/phases/tier2/phase61.sh` — bat drift audit integration pattern
- Direct codebase read: `scripts/bat-scanner.sh` — bat_scan_pod_json(), complete bat drift implementation
- Direct codebase read: `crates/racecontrol/src/flags.rs` — FeatureFlagRow structure, GET /api/v1/flags
- Direct codebase read: `crates/racecontrol/src/db/mod.rs` — feature_flags CREATE TABLE, ALTER TABLE migrations
- Direct codebase read: `crates/rc-agent/src/startup_log.rs` — startup log format, truncation behavior
- Direct codebase read: `crates/rc-agent/src/main.rs` — JSONL log path (C:\RacingPoint\ + daily rolling)
- Direct codebase read: `212-CONTEXT.md` — locked decisions, deferred items
- Direct codebase read: `REQUIREMENTS.md` — DET-01 through DET-07 exact specifications
- Direct codebase read: `STATE.md` — Phase 211 decisions, config-drift decision pending

### Secondary (MEDIUM confidence)
- CLAUDE.md standing rule: "Rust tracing logs are in UTC" — UTC conversion needed for JSONL log scanning
- CLAUDE.md standing rule: "Never pipe SSH output into config files" — SCP approach for DET-01
- CLAUDE.md standing rule: ".bat files: clean ASCII + CRLF, never parentheses in if/else" — bat-scanner.sh implements all checks

### Tertiary (LOW confidence)
- rc-agent `/api/v1/flags` endpoint existence: inferred from feature_flags.rs being present in rc-agent/src, not verified via grep of routes

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all tools are already in use across 60 audit phases
- Architecture: HIGH — pattern directly derived from phase61.sh and auto-detect.sh source code
- Pitfalls: HIGH — startup log truncation verified in startup_log.rs; ws_connect_timeout advisory-only verified in phase02.sh comments; UTC/IST split documented in CLAUDE.md
- Open Questions: MEDIUM — rc-agent flags endpoint existence not verified

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable infrastructure, 30-day window)
