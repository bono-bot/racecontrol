# Stack Research

**Domain:** Bash-based automated fleet audit runner — v23.0 Audit Protocol v4.0
**Researched:** 2026-03-25 IST
**Confidence:** HIGH (tools verified live on James's machine; integration points traced through existing codebase; parallel execution patterns confirmed against bash 5.2 docs)

---

## Context: The Constraint

Pure bash. No new compiled dependencies. No new Node packages for audit logic. Runs on James's machine (Windows 11, Git Bash) targeting server (.23), 8 pods, Bono VPS via HTTP APIs and SSH. The constraint is real and workable — this stack leans into what bash already does well.

**What's already available on James's machine (verified live):**

| Tool | Version | Available |
|------|---------|-----------|
| bash | 5.2.37 (x86_64-pc-msys) | YES — Git Bash |
| curl | 8.18.0 (mingw32 / Schannel) | YES |
| ssh | OpenSSH (via Git Bash) | YES |
| scp | OpenSSH (via Git Bash) | YES |
| diff | GNU diff | YES |
| awk | GNU awk | YES |
| sed | GNU sed | YES |
| mktemp | GNU coreutils | YES |
| date | GNU coreutils | YES |
| node | v22.22.0 | YES — for helpers only |
| python3 | 3.13.12 | YES — for helpers only |
| jq | NOT INSTALLED | NEEDS INSTALL |

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| bash | 5.2 (existing) | Audit runner language | Already installed, all 60 phases already written in bash, zero new dependency. `wait -n` (bash 4.3+) enables safe bounded parallelism without external tools. |
| jq | 1.8.1 | JSON assembly, parsing, delta comparison | The only missing tool. Single static binary, no runtime deps, winget-installable. Without jq, JSON output requires fragile string concatenation — every consumer breaks. With jq, structured output is safe and diff-able. `jq --arg` handles escaping so bash never touches JSON strings directly. |
| curl | 8.18.0 (existing) | HTTP health checks, API queries, comms-link relay calls | Already used throughout AUDIT-PROTOCOL v3.0. Schannel TLS on Windows — no openssl dep. Use `-s --max-time N --connect-timeout N` on every call to prevent hangs from offline pods. |
| ssh | OpenSSH (existing) | Fallback exec when HTTP endpoints are down | Already in Git Bash. Tailscale mesh gives Tailscale IPs as fallback path. Use only when HTTP unreachable — matches existing standing rule. |
| diff | GNU diff (existing) | Delta tracking between audit runs | `diff --unified=0` on previous/current JSON produces machine-readable change lines. `diff -q` for pass/fail. No new tool needed for delta tracking. |

### Supporting Libraries / Patterns

| Library / Pattern | Version | Purpose | When to Use |
|-------------------|---------|---------|-------------|
| `wait -n` (bash builtin) | bash 4.3+ | Bounded parallel execution — wait for any one job to finish before launching next | Use in all tier loops that target multiple pods: `(check_pod "$IP") & pids+=($!); [[ ${#pids[@]} -ge 4 ]] && wait -n` |
| `mktemp -d` (coreutils) | existing | Temp directory for per-run JSON output fragments before assembly | Each parallel phase writes to `$TMPDIR/<phase>_<host>.json`; assembler merges with jq. Avoids stdout interleaving from background jobs. |
| `trap ... EXIT` (bash builtin) | existing | Cleanup temp files and kill stray background jobs on script exit/signal | Required for any script with background jobs — `trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$TMPDIR"' EXIT INT TERM` |
| `tee` (coreutils) | existing | Dual output: JSON to file + human-readable to stdout simultaneously | Use for report generation: `generate_report | tee audit-report.md` so operator sees progress while file is written |
| Node.js (v22, existing) | v22.22.0 | One-off helpers: comms-link WS notify, WhatsApp summary dispatch | NOT for audit logic. Only for the two integration points that require it: comms-link send-message.js and WhatsApp. Never replace bash control flow with Node. |
| `set -euo pipefail` (bash) | bash 5.x | Fail-fast: exit on unset variable, propagate pipe failures | Use in every sourced library file. The main runner MUST NOT use `set -e` — it needs to collect FAIL results without aborting. Child subshells use `set -e` so individual check failures are captured, not propagated up. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `winget install jqlang.jq` | Install jq 1.8.1 on James's machine | One-time setup. After install, `jq --version` should show `jq-1.8.1`. Verify jq is on PATH in Git Bash: `which jq`. |
| `bash -n script.sh` | Syntax check audit scripts without running them | Run in CI (comms-link test/run-all.sh gate) to prevent deploying broken audit scripts. |
| `shellcheck` (optional) | Static analysis for bash scripts | Not required, but catches `[[ ]]` vs `[ ]` misuse, unquoted variables, and SC2086/SC2046 glob expansion bugs. Available via winget or scoop if needed. |

---

## Installation

```bash
# One-time setup — jq (the only missing tool)
winget install jqlang.jq

# Verify in Git Bash after install
jq --version   # expect: jq-1.8.1

# All other tools already present — no installs needed
bash --version | head -1
curl --version | head -1
ssh -V 2>&1
diff --version | head -1
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Pure bash + jq | Python script (python3 available) | When you need complex data structures (dicts, sets, sorting) that bash+jq cannot express cleanly. For this audit, jq handles all JSON needs; Python would add a different interpreter context with different error handling. |
| `wait -n` bounded parallelism | GNU Parallel (`parallel`) | GNU Parallel is more ergonomic for complex fan-out patterns, but it is NOT installed on James's machine and requires a separate download. `wait -n` + PID arrays achieve the same 4-concurrent-pod limit with zero new deps. |
| `diff` for delta tracking | `jd` (JSON diff tool) | `jd` (github.com/josephburnett/jd) provides semantic JSON diff — better for nested structure changes. Use it if pure field-level diff proves insufficient. Requires Go binary download. Start with `diff` + jq field extraction; upgrade to jd only if regression detection needs path-level granularity. |
| curl for all HTTP | Node fetch / axios | Node is available but adding Node to the audit control path means Node errors look like audit errors. Keep curl as the HTTP tool — it returns exit codes that bash can test directly. |
| comms-link relay for Bono notifications | Direct SSH to Bono VPS | comms-link relay is the standing rule default. SSH is fallback only. Audit completion notification MUST go through relay so there is an audit trail. |
| Temp dir + jq merge for parallel output | Named pipes / mkfifo | Named pipes are elegant but fragile on Windows Git Bash — FIFO semantics differ from Linux. Temp files are reliable, predictable, and debuggable (you can inspect them after a failed run). |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `set -e` in the main audit runner | The runner MUST collect FAIL results without aborting. `set -e` would exit on the first pod that returns non-200, discarding all subsequent results. | Explicit exit code capture: `result=$(curl ... 2>&1); exit_code=$?; [[ $exit_code -ne 0 ]] && record_fail ...` |
| Inline JSON string concatenation in bash | Bash string escaping is hostile to JSON. Double quotes, backslashes, newlines in log messages will corrupt the output silently. Already a standing rule: "Git Bash JSON: write JSON payloads to a file". | `jq -n --arg key "$value"` always. Never `echo '{"key":"'$value'"}'`. |
| `xargs -P` for pod parallelism | xargs mangles arguments with spaces and special characters. Pod IPs are safe, but xargs passes all args as one string to the command — the callback function can't receive structured results. | `for IP in $PODS; do (check_pod "$IP") & pids+=($!); done; wait` with temp file output collection. |
| `eval` for dynamic command construction | Audit scripts will compose commands from pod IPs, phase names, and fix commands. `eval` with any external input is a security hole and makes debugging impossible. | Arrays: `cmd=("curl" "-s" "--max-time" "5" "http://$IP:8090/health"); "${cmd[@]}"` |
| Unbounded background jobs | Running all 60 phases × 8 pods simultaneously = 480 background processes. Git Bash has a ~256 process limit; Windows kernel has overhead per process. Exceeds constraint: "max 4 concurrent pod queries". | `wait -n` semaphore: check `jobs | wc -l` or track PID count before launching next job. |
| `timeout` command for curl timeouts | `timeout` behavior differs between GNU coreutils (Linux) and Git Bash on Windows. It exists but can behave unexpectedly with subshells. | curl native timeout flags: `--max-time 10 --connect-timeout 5`. Always set both. Never rely on process-level timeout for HTTP calls. |
| SaltStack / Ansible / Puppet | v6.0 was blocked by BIOS AMD-V. Fleet management via HTTP APIs + SSH is the proven, working path. These tools add new compiled deps contradicting the constraint. | Existing rc-agent exec endpoint (:8090) + SSH via Tailscale. |
| Writing `.bat` files as the audit format | .bat files are CRLF-sensitive, cmd.exe hostile to quoting, and cannot produce structured JSON output. All standing rules about .bat file pitfalls apply. | Pure bash scripts that call the rc-agent HTTP exec endpoint for anything that needs to run on the pod. |

---

## Stack Patterns by Variant

**For quick mode (Phases 1-16, daily health check):**
- Run all 8 pods in parallel (8 concurrent — safe for quick health polls)
- Use `wait` (not `wait -n`) — simpler, all 8 complete before summary
- Target: <5 minutes wall clock

**For standard/full mode (all 50-60 phases):**
- Max 4 concurrent pod queries at any time (`wait -n` semaphore)
- Tier ordering preserved: complete tier N before starting tier N+1
- Target: <20 minutes automated vs 90-120 minutes manual

**For pre-ship mode (critical subset only):**
- Phases 1, 51, 53, 57, 46, 48-50, 58 (as defined in AUDIT-PROTOCOL v3.0)
- Run sequentially within each phase — pre-ship needs deterministic ordering for audit trail
- Emit pass/fail summary to comms-link + WhatsApp on completion

**For post-incident mode:**
- Skip QUIET phases (venue-closed hardware checks)
- Run Tier 1 (infrastructure) + the tier most relevant to the incident
- Emit delta: compare against last pre-incident audit JSON

**For venue-closed state (QUIET detection):**
- Check `GET /api/v1/fleet/health` — if all pods `ws_connected: false` AND `http_reachable: false`, venue is closed
- Mark hardware/display/kiosk-browser phases as QUIET rather than FAIL
- Still run all server, cloud, comms-link, and static analysis phases

---

## Integration Points: Existing Infrastructure

| Integration | How | Notes |
|-------------|-----|-------|
| Fleet health check | `curl -s --max-time 10 http://192.168.31.23:8080/api/v1/fleet/health` | Returns array of PodFleetStatus. Parse with jq: `.[] | select(.pod_number == N)` |
| Pod exec | `curl -s -X POST http://<POD_IP>:8090/exec -d '{"cmd":"..."}'` | Write cmd to temp file, use `-d @file` per standing rule on JSON in Git Bash |
| Pod exec via rc-sentry | `curl -s -X POST http://<POD_IP>:8091/exec -d @file.json` | Fallback when rc-agent is down. rc-sentry :8091 has 6 endpoints including /files |
| Server exec | `curl -s -X POST http://192.168.31.23:8090/exec -d @file.json` | Server :8090 is server_ops (part of racecontrol binary) |
| Auth token | `SESSION=$(curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/auth -H "Content-Type: application/json" -d '{"pin":"261121"}' \| jq -r '.session')` | Reuse single token across all authenticated phases |
| Comms-link Bono notify | `cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." COMMS_URL="ws://..." node send-message.js "audit complete: N pass, M fail"` | Node-based — call from audit runner at completion. Not bash, called as subprocess. |
| WhatsApp Uday summary | `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d @whatsapp-payload.json` | comms-link relay exec. The relay handles WhatsApp dispatch. |
| SSH fallback | `ssh -o StrictHostKeyChecking=no User@<tailscale_ip> "command"` | Only when HTTP unreachable. Per standing rule: SSH only as fallback. |
| OTA sentinel awareness | Check `C:\RacingPoint\OTA_DEPLOYING` via rc-sentry /files before any fix action | Standing rule: never restart during OTA. Audit auto-fix MUST check this sentinel. |
| MAINTENANCE_MODE awareness | Check `C:\RacingPoint\MAINTENANCE_MODE` before recording pod as FAIL | Pod with MAINTENANCE_MODE sentinel is deliberately stopped — report as QUIET/WARN, not FAIL |

---

## Output Format Strategy

Two files per audit run — human + machine:

```
audit-results/
  YYYY-MM-DD_HHMMIST_quick.json      # Structured: all results, severity, timestamps
  YYYY-MM-DD_HHMMIST_quick.md        # Human: tier summaries, delta from previous, auto-fix log
  latest-quick.json                  # Symlink / copy to latest — used for delta comparison
  latest-standard.json
  latest-full.json
  suppression.json                   # Known issues: {phase, host, pattern} → suppress if matches
```

Delta logic: `diff <(jq -S '.' latest-full.json) <(jq -S '.' current-full.json)` produces a unified diff. Extract regressions (new FAILs) with jq: `.results[] | select(.status == "FAIL")` compared against previous run's same field.

---

## Version Compatibility

| Package | Version | Notes |
|---------|---------|-------|
| bash | 5.2.37 | `wait -n` requires bash 4.3+. `declare -A` associative arrays require bash 4.0+. Both confirmed available. |
| jq | 1.8.1 | Latest stable as of 2026-03. Fixes CVE-2025-49014 heap use-after-free. Use `jq -e` for exit-code-based failure detection (exits non-zero if result is false/null). |
| curl | 8.18.0 | Schannel TLS (Windows native). No `--cacert` needed for HTTPS to known hosts. `--max-time` and `--connect-timeout` both supported. `-w "%{http_code}"` for status code extraction. |
| ssh | OpenSSH (Git Bash) | `StrictHostKeyChecking=no` required for pod Tailscale IPs (host keys not pre-registered). Already used in standing rules. |
| Node.js | v22.22.0 LTS | Used ONLY for send-message.js (comms-link WS) and WhatsApp relay calls. Not imported into audit logic. |

---

## Sources

- bash 5.2 `wait -n` docs — HIGH confidence (builtin, verified on James's machine)
- jq 1.8.1 release notes (github.com/jqlang/jq/releases) — HIGH confidence (latest stable June 2025, CVE fix in 1.8.1)
- jq official docs (jqlang.org) — HIGH confidence
- curl 8.18.0 (verified live: `curl --version`) — HIGH confidence
- AUDIT-PROTOCOL.md v3.0 (1928 lines, read directly) — HIGH confidence
- CLAUDE.md standing rules (JSON in Git Bash, cmd.exe quoting, OTA sentinel, SSH fallback) — HIGH confidence
- PROJECT.md v23.0 milestone spec (read directly) — HIGH confidence
- comms-link send-message.js (verified path: `C:/Users/bono/racingpoint/comms-link/send-message.js`) — HIGH confidence
- WebSearch: parallel bash patterns, jq on Windows, delta tracking approaches — MEDIUM confidence (corroborated by live tool verification)

---

*Stack research for: v23.0 Audit Protocol v4.0 — bash automated fleet audit runner*
*Researched: 2026-03-25 IST*
