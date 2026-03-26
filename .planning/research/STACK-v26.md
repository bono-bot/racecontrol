# Stack Research

**Domain:** Autonomous bug detection, config drift detection, log anomaly detection, self-healing escalation — v26.0
**Researched:** 2026-03-26
**Confidence:** HIGH — all recommendations drawn from existing codebase, no external library additions needed

---

## Context: What Already Exists

This milestone adds to a mature bash + jq operational stack. The foundation is already validated in production:

| Existing Component | Location | Status |
|-------------------|----------|--------|
| 60-phase audit runner | `audit/audit.sh` + `audit/phases/` | Production, parallel engine active |
| 8-lib audit framework | `audit/lib/` (core, parallel, results, delta, suppress, report, fixes, notify) | Production |
| Auto-fix engine | `audit/lib/fixes.sh` — whitelist-only, billing gate | Production |
| Comms-link relay v18.0 | `comms-link/` — exec/chain, WS + HTTP | Production |
| Standing rules registry | JSON, 76+ rules classified | Production |
| `auto-detect.sh` | `scripts/auto-detect.sh` — 6-step pipeline | Committed b54e4585, tested |
| `bono-auto-detect.sh` | `scripts/bono-auto-detect.sh` — James failover | Deployed to VPS, cron active |
| Chain templates | `comms-link/chains.json` — auto-detect-bono, sync-and-verify | Active |

**The stack constraint is hard:** Bash + jq only. No compiled dependencies. Consistent with audit framework.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Bash | 5.x (Git Bash on Windows, native on VPS) | All new scripts | Existing constraint — consistent with 60-phase audit, `set -euo pipefail` discipline already established |
| jq | 1.6+ | JSON parsing for API responses, config comparison, log field extraction | Already used throughout; `jq -r`, `jq -e`, `--argjson` patterns proven across 60 phases |
| curl | 7.x | HTTP health checks, relay calls, WhatsApp Evolution API | Already used; `--max-time`, `-d @file` (JSON-to-file pattern required by standing rule) |
| cron (Linux/VPS) | System cron | Scheduled execution on Bono VPS | Simpler than alternatives; already active for `bono-auto-detect.sh`; `0 21 * * *` (UTC) = 2:30 AM IST pattern |
| Windows Task Scheduler | Windows built-in | Scheduled execution on James | Already used for comms-link watchdog, kiosk, web dashboard — consistent pattern via `schtasks` |
| node (send-message.js) | v22+ (James), v24+ (Server) | WS notifications to Bono | Only JS needed is already in comms-link; no new Node modules required |

### Supporting Libraries (all already installed, zero new dependencies)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| jq filters for config comparison | Built-in | Config drift detection — compare expected key-value pairs vs actual API response | Parsing JSON from `/api/v1/health`, `/api/v1/config` endpoints; NOT parsing raw TOML (use grep/findstr for that) |
| `git diff` / `git log` | System git | Build drift detection, unpushed commit detection | `git log --oneline origin/main..HEAD` — already used in auto-detect.sh Step 5; `git log "${server_build}..HEAD" -- crates/` for code-only diff |
| `grep` / `findstr` (Windows) | System | Log anomaly pattern matching on JSONL files | `grep -cE "ERROR\|PANIC\|CRITICAL"` on log files; `findstr /C:"pattern"` via remote exec on pods |
| `mktemp` | System | Temporary files for JSON payloads to curl | Required by standing rule: curl -d @file, never inline JSON with Windows paths |
| `wc -l` / `awk` | System | Count-based threshold checks for anomaly detection | Line counts for log anomaly frequency thresholds; `awk '{print $1}'` for field extraction |
| `sort -u` / `uniq -c` | System | Build consistency checks across fleet | `sort -u` on build_ids from fleet health API — already used in auto-detect.sh Step 4b |
| `certutil -hashfile` (Windows) | Windows built-in | Checksum computation for bat file drift detection on pods | No admin required; works in non-interactive exec context via rc-agent :8090 |
| `md5sum` (Linux) | System | Checksum computation for bat files in canonical repo on James | Compare against `certutil` output from pods after normalizing case |
| `ssh` (fallback only) | OpenSSH | Bono → Server restart when James relay down | Only when comms-link relay is down; established pattern in bono-auto-detect.sh |
| `pm2` | VPS system | Cloud racecontrol failover activation | Already on VPS; `pm2 start racecontrol` in bono-auto-detect.sh — no changes needed |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `shellcheck` | Static analysis for new bash scripts | Run before committing; catches unquoted vars, unset references; `shellcheck scripts/auto-detect.sh` |
| `bash -n` | Syntax check | Already in comms-link test suite (Suite 3); add new scripts to the syntax check list |
| `audit/test/test-audit-sh.sh` | Integration test for audit pipeline | Extend this with auto-detect pipeline integration tests |

---

## New Capability Stack (What to Add for v26.0)

### 1. Scheduled Autonomous Execution

**James side (Windows Task Scheduler):**

No new tools. Use `schtasks` — same mechanism as `CommsLink-DaemonWatchdog`. Create a task that runs daily at 2:30 AM IST = 21:00 UTC (previous calendar day).

```batch
rem register-auto-detect.bat — mirrors register-james-watchdog.bat pattern
schtasks /Create /TN "RacingPoint-AutoDetect" /TR "bash C:/Users/bono/racingpoint/racecontrol/scripts/auto-detect.sh --mode standard" /SC DAILY /ST 21:00 /F /RU bono
```

Why Task Scheduler over a loop: The comms-link daemon watchdog already uses this pattern. Task Scheduler handles retries on failure, logs to Windows Event Viewer, and survives system restarts without a parent process to keep it alive.

**Bono side (cron):**

Cron is already active. Verify the existing cron entry is at `0 21 * * *` (UTC) for 2:30 AM IST — PROJECT.md specifies "daily 2:30 AM IST" which is 21:00 UTC. The entry in bono-auto-detect.sh comments shows `0 2 * * *` which is 2:00 AM UTC = 7:30 AM IST — this may need correction to target 2:30 AM IST (21:00 UTC previous day).

```cron
# Correct entry for 2:30 AM IST
30 21 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh --mode standard >> /root/auto-detect.log 2>&1
```

### 2. Config Drift Detection

No new libraries. Pure bash + jq + curl + grep:

**Layer 1 — TOML structural integrity** (already in `audit/phases/tier1/phase02.sh`):
- First line must start with `[` (SSH banner corruption guard)
- Duplicate `enabled =` keys (sed conflict from 2026-03-25 incident)

**Layer 2 — Canonical value comparison** (new, extend phase02.sh or new phase script):
Compare expected config values against a `scripts/canonical-config.json` source-of-truth file. For each key in canonical, fetch the actual value from the running API and compare.

```bash
# Pattern: compare canonical vs actual
canonical_ws_timeout=$(jq -r '.ws_connect_timeout_ms // 600' "$REPO_ROOT/scripts/canonical-config.json")
actual_ws_timeout=$(curl -s --max-time 5 "$SERVER_URL/api/v1/health" | jq -r '.config.ws_connect_timeout // 0' 2>/dev/null || echo "0")
if [[ "$actual_ws_timeout" -lt "$canonical_ws_timeout" ]]; then
  log WARN "CONFIG DRIFT: ws_connect_timeout actual=$actual_ws_timeout expected>=$canonical_ws_timeout"
fi
```

**Layer 3 — Bat file drift detection** (new, add to cascade check):
Compare MD5 checksums of `start-rcagent.bat` and `start-rcsentry.bat` on each pod against the canonical repo version. Catches regressions where manual changes weren't deployed via the deploy pipeline.

```bash
# On James (canonical source)
canonical_hash=$(md5sum "$REPO_ROOT/deploy-staging/start-rcagent.bat" | awk '{print $1}')

# On each pod via safe_remote_exec (note: certutil outputs MD5 hash on line 2)
pod_hash_raw=$(safe_remote_exec "$pod_ip" 8090 "certutil -hashfile C:\\RacingPoint\\start-rcagent.bat MD5" 10)
pod_hash=$(echo "$pod_hash_raw" | jq -r '.stdout // ""' 2>/dev/null | sed -n '2p' | tr -d ' \r\n' | tr '[:upper:]' '[:lower:]')
if [[ "$canonical_hash" != "$pod_hash" ]]; then
  log WARN "BAT DRIFT: pod $pod_ip start-rcagent.bat hash mismatch (canonical=$canonical_hash, pod=$pod_hash)"
fi
```

Why this approach: No external tooling. `safe_remote_exec` in `audit/lib/core.sh` handles remote execution with error handling. Checksum comparison is O(1) per pod. Standing rule already requires bat file sync on every deploy — this automates the verification.

### 3. Log Anomaly Detection

No new libraries. Pattern: `grep -cE` on the JSONL log files via the existing `/api/v1/logs` endpoint with configurable thresholds.

**Log format already known:** JSONL with `level`, `message`, `timestamp` (UTC — convert before counting per standing rule). The `/api/v1/logs?lines=N` endpoint already exists and returns structured JSON.

**Three detection patterns to implement as a new `run_log_anomaly_check()` function:**

```bash
# Pattern 1: Error rate threshold (in last N lines)
LOG_ERROR_THRESHOLD=10
server_logs=$(curl -s --max-time 10 "http://192.168.31.23:8080/api/v1/logs?lines=500" 2>/dev/null || echo "[]")
error_count=$(echo "$server_logs" | jq '[.[] | select(.level == "ERROR" or .level == "error")] | length' 2>/dev/null || echo "0")
if [[ "$error_count" -gt "$LOG_ERROR_THRESHOLD" ]]; then
  log WARN "LOG ANOMALY: $error_count ERROR entries in last 500 lines (threshold=$LOG_ERROR_THRESHOLD)"
fi

# Pattern 2: Panic/crash detection (any occurrence = FAIL)
panic_entries=$(echo "$server_logs" | jq -r '.[] | select(.message | test("panic|thread panicked|unwrap.*failed|index out of bounds"; "i")) | .message' 2>/dev/null || echo "")
if [[ -n "$panic_entries" ]]; then
  log ERROR "LOG ANOMALY: Rust panic detected in recent logs"
fi

# Pattern 3: Silence detection — no log activity for N minutes
SILENCE_THRESHOLD_MINUTES=10
last_log_ts=$(echo "$server_logs" | jq -r '.[-1].timestamp // empty' 2>/dev/null || echo "")
if [[ -n "$last_log_ts" ]]; then
  # Convert UTC log timestamp to epoch, compare against now
  last_epoch=$(TZ=UTC date -d "$last_log_ts" +%s 2>/dev/null || echo "0")
  now_epoch=$(date +%s)
  age_minutes=$(( (now_epoch - last_epoch) / 60 ))
  if [[ "$age_minutes" -gt "$SILENCE_THRESHOLD_MINUTES" ]]; then
    log WARN "LOG ANOMALY: No log activity for $age_minutes minutes (threshold=$SILENCE_THRESHOLD_MINUTES)"
  fi
fi
```

**On-pod log anomalies:** Same pattern via `safe_remote_exec` — scan `C:\RacingPoint\rc-agent.jsonl` for PANIC/ERROR patterns. Already done partially in `audit/phases/tier18/phase60.sh` for feature flag evidence.

Why grep/jq over external log tooling (ELK, Loki, Grafana): The logs are JSONL, which `jq` handles natively. The `/api/v1/logs` endpoint returns structured JSON. Adding a log aggregator requires compiled dependencies (Go/Java/Python) and persistent infrastructure — violates the bash+jq constraint and creates new failure modes to monitor.

### 4. Cross-System Cascade Verification (Extending Existing Step 4)

Already implemented in `auto-detect.sh` Steps 4a-4e. Extend with:

**4f — Sync timestamp delta** (covers audit phases 35+36):
Verify `updated_at` delta between venue DB and cloud DB is within acceptable window. The cloud sync runs every 30s, so a delta > 5 minutes indicates sync failure.

```bash
venue_sync_ts=$(curl -s --max-time 5 "$SERVER_URL/api/v1/health" | jq -r '.last_cloud_sync // empty' 2>/dev/null || echo "")
# Parse and compare against now; flag if > 5 minutes old
```

**4g — Pod bat file sync** (new):
Check bat file checksums on all 8 pods vs repo canonical (implementation above in Layer 3). Flag pods with stale bat files as cascade issues.

**4h — Relay E2E with timing** (covers phases 38+46):
Existing chain round-trip check (Step 3e). Move timing tracking into Step 4 and flag if round-trip > 30 seconds.

All new checks fit into the existing `run_cascade_check()` structure. Follow the established pattern: check → `log WARN/INFO` → increment `cascade_issues` → optional auto-fix → log fix result.

### 5. Self-Healing Escalation

The escalation tiers exist in the codebase but are scattered. Formalize as a reusable `escalate()` function:

```bash
escalate() {
  local issue_type="$1"   # e.g. "server_down", "pod_offline", "build_drift"
  local severity="$2"     # "CRIT", "HIGH", "WARN"
  local context="$3"      # JSON blob with issue details

  # Tier 1: Auto-fix (whitelist only, billing gate)
  # Already in audit/lib/fixes.sh — called by individual phase scripts

  # Tier 2: Relay-based retry
  # Already in chain orchestrator retries field in chains.json

  # Tier 3: Service restart
  # schtasks /Run /TN StartRCDirect via Tailscale SSH — already in bono-auto-detect.sh Check 1

  # Tier 4: Cloud failover
  # pm2 start racecontrol — already in bono-auto-detect.sh

  # Tier 5: WhatsApp alert to Uday
  # notify_uday() already in bono-auto-detect.sh; notify_whatsapp() in audit/lib/notify.sh
  if [[ "$severity" == "CRIT" ]]; then
    local wa_msg="RACING POINT ALERT ($TIMESTAMP): $issue_type — $severity. Context: $(echo "$context" | jq -r '.summary // "see logs"' 2>/dev/null)"
    # Call appropriate notify function
  fi
}
```

No new tools — just orchestration of existing pieces into one auditable function.

---

## Integration Points

| New Capability | Integrates With | How |
|---------------|-----------------|-----|
| Scheduled execution | Task Scheduler (James) | New `register-auto-detect.bat` following `register-james-watchdog.bat` pattern |
| Scheduled execution | System cron (Bono) | Verify/update existing cron entry for correct IST time |
| Config drift detection | `audit/phases/tier1/phase02.sh` | Extend existing phase OR add new `run_config_drift()` function to `scripts/auto-detect.sh` |
| Bat file drift | `audit/lib/core.sh:safe_remote_exec` | New sub-check in `run_cascade_check()` Step 4g |
| Log anomaly detection | `/api/v1/logs` endpoint + `jq` | New `run_log_anomaly_check()` function as Step 4.5 in auto-detect pipeline |
| Cascade sync verification | `auto-detect.sh:run_cascade_check()` | New sub-checks 4f, 4g, 4h |
| Self-healing escalation | `audit/lib/notify.sh` + `bono-auto-detect.sh:notify_uday()` | New `escalate()` function in `scripts/auto-detect.sh` |
| Integration test suite | `audit/test/test-audit-sh.sh` | Extend with auto-detect pipeline mock tests |

---

## Installation

Nothing to install. All tools are already present on both machines.

```bash
# Verify prerequisites on James (already checked in auto-detect.sh)
for cmd in jq curl node git bash; do command -v "$cmd" && echo "OK: $cmd" || echo "MISSING: $cmd"; done

# Task Scheduler registration (James — new for v26.0)
bash scripts/register-auto-detect.bat
schtasks /Query /TN "RacingPoint-AutoDetect"   # verify

# Cron entry (Bono VPS — verify/update existing)
ssh root@100.70.177.44 "crontab -l | grep auto-detect"
# Expected: 30 21 * * * AUDIT_PIN=... bash /root/racecontrol/scripts/bono-auto-detect.sh --mode standard
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Bash + jq pattern matching | Python log analysis (structlog, loguru) | If log volume exceeds 10MB/day and grep becomes too slow — not the case here (500-line cap on API response) |
| Windows Task Scheduler | Loop-based watchdog bash script | Never for scheduled tasks — Task Scheduler is more resilient to crashes, survives reboots without a parent process |
| System cron (Bono) | systemd timer | systemd timers are better for complex dependencies, but VPS uses cron and it's already working — consistency wins |
| `certutil -hashfile` (Windows bat drift) | SHA256 via PowerShell `Get-FileHash` | certutil is available without admin in non-interactive exec context; PowerShell may fail in CREATE_NO_WINDOW context (per standing rule) |
| jq for JSON log parsing | Python json module | jq is already everywhere; Python adds a dependency and version management concern |
| `/api/v1/logs` endpoint for log access | Direct file read via safe_remote_exec | Log files may be locked by the rolling appender; API endpoint is safer and provides pre-structured JSON |
| grep -cE for pattern threshold counts | awk for log analysis | grep is simpler and already used throughout; awk adds cognitive overhead without benefit for count-based thresholds |
| Single `escalate()` function | Per-issue-type escalation logic scattered in check functions | Centralized escalation ensures consistent tier progression and makes escalation paths auditable in one place |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| ELK Stack / Grafana Loki | Requires compiled Go/Java + persistent services; violates bash+jq constraint; adds infra to monitor the monitor | `/api/v1/logs` endpoint + jq + grep patterns |
| Python scripts for new detection logic | Breaks homogeneity with audit framework; Python version drift between James/Bono; pip deps | Bash + jq — battle-tested across 60 phases |
| `curl` with inline JSON strings | Standing rule violation — bash escaping mangles backslashes in Windows paths | Write JSON to `mktemp` file, `curl -d @file` |
| Prometheus / Alertmanager | External service requiring compiled Go binary; over-engineered for 8-pod venue | WhatsApp via Evolution API (already in production), WS notifications via send-message.js |
| `set -e` in audit phase scripts | Causes unexpected exits on probe failures; audit phases must encode errors in `emit_result`, not bash exit codes | `set -u` + `set -o pipefail` only; handle errors in `status="FAIL"` emit_result calls |
| `local` keyword for variables outside functions | Causes `set -u` failures in some bash versions; `local` is function-scoped | Declare variables at script top scope without `local` when outside functions |
| Systemd units for scheduled tasks on James | James is Windows 11 — systemd is Linux-only | `schtasks` via `register-auto-detect.bat` |
| Hardcoded PSK/credentials in new scripts | Pre-commit hook blocks this; security gate SEC-01 scans for it | Environment variables: `COMMS_PSK`, `AUDIT_PIN`, `EVOLUTION_API_KEY` — same pattern as existing scripts |
| `sleep` loops for retry logic | Blocks the pipeline; wastes 8-minute audit time budget | `--max-time` on curl + exponential backoff via chain orchestrator retries (already implemented in chains.json) |
| Checking cloud build against HEAD for equality | Cloud may deliberately run a stable older build — cloud vs HEAD equality is not the right check | Check cloud vs venue for equality: `cloud_build == venue_build` (already in auto-detect.sh Step 4c) |
| Full audit mode in scheduled runs | `--mode full` takes up to 8 minutes, includes venue-closed QUIET phases — wasteful at 2:30 AM | `--mode standard` for scheduled; `--mode full` reserved for manual investigation or post-incident |

---

## Stack Patterns by Variant

**If adding a new auto-fix to the whitelist:**
- Add the fix name to `APPROVED_FIXES` array in `audit/lib/fixes.sh`
- Implement the fix function following the `clear_stale_sentinels()` pattern: check → `is_pod_idle()` gate → exec → verify → `emit_fix()`
- Because: whitelist enforcement is the single gate preventing unsafe automated operations on billing-active pods

**If adding a new cascade check:**
- Add to `run_cascade_check()` in `scripts/auto-detect.sh` as sub-check 4f/4g/...
- Follow pattern: check → `log WARN/INFO` → increment `cascade_issues` → optional auto-fix → log fix result
- Because: monolithic function keeps all cascade logic in one auditable place

**If adding a new log anomaly pattern:**
- Add to `run_log_anomaly_check()` (new function to create in v26.0)
- Define threshold as a named constant at top of function (not magic numbers)
- Because: thresholds will need tuning as normal log volumes become known; named constants make that obvious

**If the comms-link relay is down when auto-detect runs:**
- James side: log the relay failure, skip Steps that require relay, mark e2e_health as WARN not FAIL, continue with audit + cascade checks that use direct curl
- Bono side: already handles this via `james_alive` check — delegates to James if up, runs independently if down
- Because: the relay being down is itself a detectable bug — don't let it prevent the rest of the pipeline from running

**If a new check is needed on Bono side only:**
- Add to `bono-auto-detect.sh` Check N pattern (Check 1 through 5 already present)
- Keep it independent of James-specific paths (no references to `C:/Users/bono/...`)
- Because: bono-auto-detect.sh must work entirely without James connectivity

**If a scheduled run finds unfixable bugs:**
- Auto-detect.sh already returns exit code 1 and includes bug count in the Bono notification
- Escalation to Uday via WhatsApp should only fire for CRIT/HIGH severity issues with BUGS_UNFIXED > 0
- Because: Uday's goal is automation — alert fatigue defeats the purpose

---

## Version Compatibility

| Component | Compatible With | Notes |
|-----------|-----------------|-------|
| jq 1.6 | All bash scripts | `--argjson` flag required (available since 1.5); `jq -e` for boolean exit codes; `jq -r` for raw string output |
| curl 7.x | All scripts | `--max-time` (not `--connect-timeout` alone) for total timeout; `-d @file` for JSON; `-s` for silent mode |
| Git Bash bash 5.x | Windows scripts | `declare -A` associative arrays (used in auto-detect.sh) require bash 4+; Git Bash ships bash 5.x |
| Task Scheduler + bash | James watchdog | Use `bash C:/path/script.sh` as the task action — no `cmd /c` wrapper needed for bash scripts |
| cron + bash (Bono) | bono-auto-detect.sh | Full path required in crontab: `bash /root/racecontrol/scripts/bono-auto-detect.sh` — no $PATH expansion in cron |
| Evolution API v2 | WhatsApp notifications | Endpoint: `/message/sendText/{instance}`, apikey header — already validated in notify.sh and bono-auto-detect.sh |
| `certutil -hashfile` | Windows pods (all 8) | Available on all Windows versions without admin — safe for non-interactive exec context |

---

## Sources

- `scripts/auto-detect.sh` (b54e4585) — existing 6-step pipeline implementation reviewed directly — HIGH confidence
- `scripts/bono-auto-detect.sh` — existing Bono failover implementation reviewed directly — HIGH confidence
- `audit/lib/fixes.sh` — whitelist-only auto-fix engine, `is_pod_idle()` billing gate pattern — HIGH confidence
- `audit/lib/core.sh` — `safe_remote_exec`, `emit_result`, `http_get` primitives — HIGH confidence
- `audit/phases/tier1/phase02.sh` — config drift detection pattern (TOML integrity, value validation) — HIGH confidence
- `audit/phases/tier18/phase60.sh` — cross-system chain E2E pattern, log scanning via API endpoint — HIGH confidence
- `comms-link/chains.json` — chain template structure (auto-detect-bono, sync-and-verify) — HIGH confidence
- `CLAUDE.md` (racecontrol) — standing rules: bash+jq constraint, `curl -d @file`, no hardcoded PSK, Task Scheduler pattern, bat parentheses — HIGH confidence (read directly)
- `CLAUDE.md` (comms-link) — available exec commands, chain orchestrator retry field, shell relay allowlist — HIGH confidence (read directly)
- `.planning/PROJECT.md` — v26.0 milestone spec: target features, constraints, foundation already built — HIGH confidence

---

*Stack research for: v26.0 Autonomous Bug Detection & Self-Healing — Bash + jq operational stack extensions*
*Researched: 2026-03-26 IST*
