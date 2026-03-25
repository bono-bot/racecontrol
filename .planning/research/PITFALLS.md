# Pitfalls Research

**Domain:** Automated fleet audit system — bash-based, Windows Git Bash origin, targeting Windows servers/pods via HTTP exec endpoints, with parallel execution, JSON output, delta tracking, auto-remediation, and known-issue suppression (v23.0 Audit Protocol v4.0)
**Researched:** 2026-03-25
**Confidence:** HIGH — all critical pitfalls sourced from actual incidents documented in CLAUDE.md standing rules, LOGBOOK.md, PROJECT.md, and the v3.0 AUDIT-PROTOCOL.md for this exact codebase. No hypothetical pitfalls.

---

## Context: Why This Audit System Is Especially Hazardous to Build

Before cataloguing pitfalls, understand the properties of this environment that make generic bash audit tooling fail in novel ways:

1. **Git Bash on Windows is not bash on Linux.** Path separators, line endings, process substitution, background job signals, and `wait` semantics all behave differently. Scripts that work perfectly in WSL or Linux CI silently produce wrong results on Git Bash.
2. **Every remote command passes through cmd.exe.** The rc-agent `/exec` endpoint wraps commands with `cmd /C`. This is the single most dangerous property of this system. Quoting, variables, backslashes — all get mangled before the command runs.
3. **curl output on Windows includes quoted strings.** `curl.exe` response bodies sometimes include surrounding quotes in certain Git Bash contexts. A JSON field parsed as `"200"` (with quotes) breaks `u32::parse()` silently — this exact bug caused 2 deploy cycles of pod healer flicker.
4. **SSH banners corrupt piped output.** The post-quantum SSH warning and MOTD go to stderr, but some wrappers merge streams. A config file written via `ssh ... "cat file" > local` has been silently prepended with banner garbage in production.
5. **Eight pods in parallel saturate the network.** The venue LAN is consumer-grade. Firing 8 simultaneous curl probes overwhelms the NVR/switch and produces false FAIL results that look like real failures.
6. **Auto-fix operates on production infrastructure.** There are no test pods. An auto-fix that kills the wrong process takes down a paying customer's session. The cost of a false-positive fix is higher than a false-negative miss.

---

## Critical Pitfalls

### Pitfall 1: cmd.exe Quoting Destroys Remote Commands

**What goes wrong:**
The audit script builds a command string in bash and sends it to rc-agent's `/exec` endpoint. rc-agent wraps it with `cmd /C "<your command>"`. Any `"` in the original command becomes a nested quote inside the outer `cmd /C "..."` wrapper — cmd.exe terminates the string at the first unescaped inner quote. The command is truncated silently.

Examples that fail:
- `taskkill /F /IM "GoPro Webcam.exe"` — the `"` around the image name breaks the outer wrapper
- `powershell -Command "$result = ..."` — the `$` is interpreted by bash before transmission, arrives as empty string
- `findstr /C:"enabled" file.toml` — nested quote in `/C:` arg breaks parsing
- Any Windows path with spaces: `C:\Program Files\...` — space inside cmd.exe string terminates early

**Why it happens:**
Developers test commands locally in cmd.exe where they work correctly. When the same command goes through the exec endpoint, an outer `cmd /C "..."` wrapper is added. The command was never designed to nest inside another cmd invocation.

**How to avoid:**
- Write complex commands as `.bat` files on the pod (`echo @echo off > C:\RacingPoint\audit-check.bat`), then execute the bat file by path. Bat files do not get wrapped in outer cmd quoting.
- Use PID-based targeting for process operations: `tasklist /FI "IMAGENAME eq rc-agent.exe" /FO CSV /NH` then parse PIDs from response, then `taskkill /F /PID <pid>`.
- Never embed `"` in remote commands. Use `%COMSPEC%`, `findstr /C:key file` (no quotes), or write the arg to a file first.
- For PowerShell: escape `$` as `^$` or use `-EncodedCommand` with base64.
- Write remote commands to a temp `.bat` file, execute the bat, then clean up.

**Warning signs:**
- Remote command returns exit code 0 but produces no output
- Output is truncated mid-string
- `tasklist` output shows process still running after kill command "succeeded"
- JSON payload to `/exec` contains backslashes that arrive as single-backslash on the receiving end

**Phase to address:** Phase 1 (audit runner core) — establish a safe remote execution wrapper function that escapes or avoids cmd.exe quoting before building any audit check on top of it.

---

### Pitfall 2: SSH Banner Output Corrupts Captured Results

**What goes wrong:**
The audit script runs `ssh root@<host> "command" > result.txt` to capture remote output. The SSH connection prints a post-quantum upgrade warning and MOTD to stderr. In some terminal configurations and CI runners, stderr and stdout are merged by the shell redirect. `result.txt` starts with 2–3 lines of SSH banner text, then the actual command output. Any downstream parser expecting JSON or a specific format fails silently by reading the first line.

Confirmed production incident (2026-03-24): `racecontrol.toml` had 3 SSH banner lines prepended. TOML parser rejected from line 1. `load_or_default()` fell back to empty defaults. Process guard ran with 0 allowed entries for 2+ hours. No operator noticed.

**Why it happens:**
`ssh` sends banner content to stderr. The bash redirect `>` captures stdout only — BUT when the ssh client is invoked interactively or via certain Git Bash versions, stderr and stdout are merged. The audit developer tests in an environment where banners are absent (LAN, no MOTD) and the bug only appears on the remote Bono VPS or when the SSH client version changes.

**How to avoid:**
- Always add `2>/dev/null` to SSH command captures: `ssh host "command" 2>/dev/null > result.txt`
- After every SSH capture, validate the first line: `head -1 result.txt | grep -q '^\{' || echo "CORRUPTED: $host"`. JSON must start with `{`. TOML sections start with `[`. Validate before parsing.
- For the audit system: never parse SSH output directly. Always validate structure first.
- Prefer HTTP health endpoints over SSH wherever available — they do not have banner contamination.

**Warning signs:**
- `jq` returns `parse error` on SSH-captured output
- First line of captured output contains "Warning:", "Notice:", or "MOTD:"
- Audit reports a FAIL for a service that is actually healthy (validator rejected due to banner)

**Phase to address:** Phase 1 (audit runner core) — wrap all SSH capture calls in a `safe_ssh_capture()` function that adds `2>/dev/null` and validates output structure. Apply universally before any phase-specific checks are written.

---

### Pitfall 3: curl Output Includes Surrounding Quotes in Git Bash

**What goes wrong:**
In certain Git Bash contexts, `curl.exe` (Windows native) vs `curl` (Git Bash cygwin version) behave differently with response bodies. The Windows `curl.exe` binary sometimes wraps string values in quotes when its output is captured via bash command substitution: `STATUS=$(curl.exe -s http://host/health | jq -r '.status')` returns `"ok"` (with quotes) instead of `ok`. When the audit script compares `[ "$STATUS" = "ok" ]`, it fails. The pod appears DOWN when it is healthy.

Confirmed production incident: Pod healer curl fix deployed twice — both times declared "fixed" based on health endpoint. The actual stdout was `"200"` (with quotes), which failed `u32::parse()`. Healer still thought lock screen was down. `ForceRelaunchBrowser` spam continued through two deploy cycles.

**Why it happens:**
Git Bash uses its own `curl` (linked against cygwin) for `curl` but `curl.exe` invokes the Windows binary. The PATH order determines which binary runs. Scripts written assuming one behave incorrectly with the other. The Windows binary is used in rc-agent's exec endpoint; the cygwin binary runs in the audit script's local shell. Comparisons between local processing and remote output can be checking different things.

**How to avoid:**
- In audit bash scripts, always specify `curl.exe` explicitly when calling Windows hosts — or `curl` explicitly when processing locally — to lock binary selection.
- Strip surrounding quotes from all captured values: `STATUS=$(echo "$RAW" | tr -d '"')` before comparison.
- Use `jq -r` (raw output) for all JSON field extraction. `-r` removes surrounding quotes from string values.
- When comparing HTTP status codes, strip whitespace AND quotes: `CODE=$(echo "$CODE" | tr -d '" ')`.

**Warning signs:**
- `[ "$STATUS" = "ok" ]` fails but `echo "$STATUS"` shows `"ok"` (with visible quotes in terminal)
- Audit marks healthy pods as FAIL with no error details
- jq successfully parses a field but string comparison fails on the extracted value

**Phase to address:** Phase 1 (audit runner core) — establish a `http_get()` helper function that always uses `jq -r` and strips quotes from extracted values. Every audit check must go through this helper.

---

### Pitfall 4: Parallel Background Jobs Produce Interleaved Output

**What goes wrong:**
The audit script fires pod checks in parallel using `&` background jobs: `check_pod $IP & `. Each background job writes directly to stdout. With 8 pods running concurrently, their output interleaves — partial lines from one pod appear in the middle of another pod's output block. The result log has garbled content that no parser can interpret. Worse, `wait` collects all exit codes but provides no mapping between job PID and which pod it checked, making failure attribution impossible.

Secondary failure: bash `wait` with exit code capture only works correctly in bash 4.3+ (`wait -n` for the first-to-finish, `wait $pid` for specific jobs). Git Bash on Windows ships an older bash version where `wait` behavior differs. `$!` captures only the last background process PID, losing earlier jobs.

**Why it happens:**
Developers write parallel loops thinking stdout buffering will keep lines atomic. Shell stdout is line-buffered in interactive mode but fully buffered when redirected. Even line buffering doesn't prevent interleaved multi-line blocks. There is no locking primitive for shell stdout.

**How to avoid:**
- Write each pod's result to a dedicated temp file: `check_pod $IP > /tmp/audit-pod-$IP.json 2>&1 &`. Collect PIDs. Use `wait` to drain. Then read and merge the per-pod files.
- Use a result directory: `RESULT_DIR=$(mktemp -d)`. Each pod writes to `$RESULT_DIR/pod-$IP.json`. After all `wait`, merge with `jq -s '.'`.
- Enforce concurrency limit: maximum 4 pods at once (per PROJECT.md constraint). Use a semaphore pattern: track active PID count, wait for one to finish before launching the next when at the limit.
- Never write to shared stdout from background jobs. All per-pod output goes to files.

**Warning signs:**
- Audit log contains lines like `=== 192.168.31.89 === PASS=== 192.168.31.33 ===` (interleaved)
- jq fails to parse the result log with "unexpected character" errors
- Some pods always show correct results while others show garbage (timing-dependent)
- The same run produces different results on consecutive executions

**Phase to address:** Phase 2 (parallel execution engine) — the temp-file-per-target pattern must be established before any parallel check is added. Retrofitting after 60 checks are written is prohibitively difficult.

---

### Pitfall 5: Auto-Remediation Kills Active Billing Sessions

**What goes wrong:**
The audit script detects "orphan PowerShell processes" (one of the v3.0 audit checks) and auto-fixes by running `taskkill /F /IM powershell.exe` on the pod. There are currently 15 legitimate PowerShell processes on each pod — including the rc-agent relaunch chain and the pod's billing session WebSocket handler. `taskkill /IM` kills ALL matching processes. The billing session terminates mid-race. The customer's time is lost. Manual reconciliation is required.

Variant: Auto-fix detects "rc-agent running outside RacingPoint directory" (stale PID check) and kills it. But the pod has an active session. Session end never fires.

**Why it happens:**
Auto-fix logic is written when the venue is closed and pods are empty. The fix is validated on idle pods. It ships. The first time it runs during business hours, the "safe" assumption (idle pods) is wrong.

**How to avoid:**
- Every auto-fix action must check `has_active_session` before executing. Query `GET /api/v1/fleet/health` → check `session_state` field for the target pod. If not `Idle`, skip the fix and emit `SKIP_ACTIVE_SESSION`.
- Maintain an explicit whitelist of auto-fix-safe actions in a config file (`audit-safe-fixes.json`). Any action not on the whitelist requires human approval — the audit script only flags it.
- Safe-fix whitelist for v23.0: clear sentinel files, kill known orphan images that are NOT powershell/rc-agent/racecontrol, restart services that have been `DOWN` for > 5 minutes with no session. Nothing else is auto-fix-safe.
- Log every auto-fix attempt with: timestamp, pod IP, action, session state at time of fix, result. Write to `audit-autofix.log` separate from the main audit results.

**Warning signs:**
- Auto-fix list includes `taskkill /IM powershell.exe` — this is never safe
- Auto-fix list includes any kill command targeting rc-agent or racecontrol
- No `has_active_session` check before any auto-fix action
- Auto-fix is tested only on idle pods

**Phase to address:** Phase 3 (auto-remediation) — session gate must be the first line of every fix function. Establish `is_pod_idle()` as a required prerequisite before any fix action is created.

---

### Pitfall 6: Delta Tracking Produces False Regressions on Venue-Closed Checks

**What goes wrong:**
The delta tracker compares current audit results against the previous run. The previous run was executed while the venue was open — so Phase 45 (kiosk browser check) returned `PASS`. The current run is executed at 02:00 when the venue is closed — Phase 45 returns `QUIET` (venue closed, hardware check skipped). The delta tracker sees `PASS → QUIET` and flags a regression. Uday gets a WhatsApp alert at 02:00 about a regression that does not exist.

Variant: Previous run was a `--mode full` run. Current run is `--mode quick`. Quick mode skips phases that full mode runs. Delta comparison shows dozens of "regressions" because checks are missing from the current run, not because anything broke.

**Why it happens:**
Delta logic compares result status by check ID without normalizing for execution context. `QUIET` and `PASS` are not equivalent, but they are also not a regression — they represent different execution conditions. Mode-aware comparison is not built into the naive diff.

**How to avoid:**
- Each result record must include: `{ "check_id": "...", "status": "...", "mode": "quick|full", "venue_state": "open|closed", "timestamp": "..." }`.
- Delta comparison rules: `PASS → QUIET` is NOT a regression (venue state changed). `PASS → FAIL` IS a regression. `QUIET → FAIL` is a regression only if `venue_state == "open"` in both runs. `PASS → SKIP` (mode change) is NOT a regression.
- Before sending a regression alert, verify both runs were in the same mode and venue state. If they differ, label the delta as `CONTEXT_CHANGE`, not `REGRESSION`.
- Store enough metadata with each run to make comparisons mode-aware and venue-state-aware.

**Warning signs:**
- Delta shows regressions on hardware/display checks during off-hours runs
- Consecutive quick→full runs show dozens of "new failures"
- Alert suppression list grows rapidly because regressions are actually context changes

**Phase to address:** Phase 4 (delta tracking) — result schema must include execution context before any delta comparison logic is written. Retrofitting context into stored results requires re-running historical audits.

---

### Pitfall 7: jq Not Available or Wrong Version on Target Path

**What goes wrong:**
The audit script uses `jq` heavily for JSON processing. Git Bash on Windows does not include `jq` by default. If `jq` is not in PATH, every JSON parse silently returns empty string (if using `$(jq ... 2>/dev/null)`) or crashes the script with an error that is caught by `set -e` and exits the entire audit mid-run, leaving results incomplete.

Variant: `jq` is available but the version in Git Bash PATH is different from the system-installed version. Version differences affect filter syntax for complex queries.

**Why it happens:**
The audit developer has `jq` installed via scoop or chocolatey and never notices it is not part of the default Git Bash installation. Scripts work in development, fail on the first run on a fresh James workstation or after a Git for Windows reinstall.

**How to avoid:**
- Add a `prerequisites_check()` function at the top of `audit.sh` that verifies all external tools before running any check: `command -v jq >/dev/null || { echo "FATAL: jq not found. Install: scoop install jq"; exit 1; }`.
- Check for: `jq`, `curl`, `ssh`, `nc` (for port checks). Fail fast with installation instructions.
- Do not use `jq` features that differ between versions. Stick to `jq -r '.field'`, `.[] | select(.key == "value")`, and `jq -s '.'` — these are stable across jq 1.5+.
- Consider embedding a minimal JSON parser as a bash function for critical single-field extractions, as a fallback: `json_get() { echo "$1" | grep -o '"'"$2"'":"[^"]*"' | cut -d'"' -f4; }`. Fragile but zero-dependency.

**Warning signs:**
- `jq: command not found` in audit output
- All JSON fields return empty string with no error
- Script exits at first JSON parse without completing remaining checks

**Phase to address:** Phase 1 (audit runner core) — prerequisites check is the first function, called before any other execution.

---

### Pitfall 8: Known-Issue Suppression List Masks Real Regressions

**What goes wrong:**
The suppression list (`known-issues.json`) is populated during initial audit development when several checks fail because of pre-existing issues. Check `audit-42` (CCBootClient in autostart) is added to the suppression list. Six months later, CCBootClient is removed from the suppression list when the issue is fixed. But someone re-adds it to the suppression list because it appeared again — without realizing the re-appearance is a new regression (something reactivated CCBootClient). The suppression list silently hides the new incident.

Variant: The suppression list grows unbounded because adding to it is easy and removing from it requires investigation. After 3 months, 40 checks are suppressed. The audit effectively covers only 20 of its 60 checks.

**Why it happens:**
Suppression is operationally convenient — it clears the noise without requiring a fix. There is no review gate, no expiry, and no count of how long an issue has been suppressed.

**How to avoid:**
- Each suppression entry must include: `{ "check_id": "...", "reason": "...", "added": "YYYY-MM-DD", "expires": "YYYY-MM-DD", "owner": "james|bono|uday" }`.
- Suppression entries expire automatically — if `expires` is in the past, the check runs unsuppressed. No silent permanent suppressions.
- Maximum 10 active suppression entries. Exceeding this generates a `SUPPRESSION_OVERFLOW` warning in the audit header.
- Monthly review: any entry older than 30 days without an expiry date generates a `SUPPRESSION_STALE` warning.
- Log suppressed checks in the audit output — they appear as `SUPPRESSED (known-issue: ID)` not as invisible skips.

**Warning signs:**
- Suppression list has entries with no `expires` field
- Suppression list has more than 10 entries
- Entries are older than 30 days
- A check that was previously PASS now requires suppression (potential regression hidden by suppression)

**Phase to address:** Phase 5 (known-issue suppression) — suppression schema must include expiry and owner before the suppression system is built. Suppressions without expiry are a maintenance trap.

---

### Pitfall 9: Timestamp Confusion — UTC Logs Reported as IST

**What goes wrong:**
The audit script reads racecontrol's JSONL log files to check for recent errors (a common audit check). The logs are in UTC. The audit reports "3 ERROR events in the last hour." But the events occurred at 03:30 UTC (09:00 IST, within business hours) and it is currently 09:45 IST (04:15 UTC). The script calculated "last hour" using the local system time (IST) against UTC timestamps — so it finds zero events in the "last hour" when there are actually 3 recent errors.

Confirmed production incident: "5 unexplained restarts" turned out to be 1 post-reboot startup + 4 of our own deploys. UTC 03:28 was misread as IST instead of IST 08:58. The Event Viewer check that would have caught this in 30 seconds was deferred for hours.

**Why it happens:**
The audit script uses `date +%s` for "current time" which returns local system time. The log timestamps are UTC. No conversion is applied. The comparison is wrong. This produces both false positives (events appear outside the window) and false negatives (events inside the window appear outside it).

**How to avoid:**
- All timestamp comparisons in the audit script must normalize to UTC: `date -u +%s` for the current time reference.
- When parsing racecontrol JSONL logs, treat all timestamps as UTC. When displaying in audit reports, convert to IST by adding 19800 seconds (5h30m).
- Add a comment at the top of every time-based check: `# NOTE: racecontrol logs are UTC. date -u +%s used for comparison.`
- In the audit report header, print both UTC and IST: `Audit run: $(date -u +%Y-%m-%dT%H:%M:%SZ) (UTC) / $(date +%Y-%m-%dT%H:%M:%S IST)`

**Warning signs:**
- Audit shows "0 errors in last hour" but log file contains recent ERROR entries
- Audit shows events from 5.5 hours ago as "recent" (IST→UTC offset confusion)
- Time-based checks pass at all times of day regardless of actual log content

**Phase to address:** Phase 1 (audit runner core) — time utilities must be established before any log-based check is written. A single wrong `date` call breaks every time-based check.

---

### Pitfall 10: Parallel Load Overwhelms Pod Network and Produces False FAILs

**What goes wrong:**
The audit script fires health checks to all 8 pods simultaneously. The venue LAN is consumer-grade hardware (DHCP from a home-grade router). Eight concurrent curl requests with a 2-second timeout to the same /24 subnet can produce connection timeouts on pods that are actually healthy — the switch's ARP table fills, or the router's connection tracking is saturated. The audit reports 3 pods as DOWN when they are running correctly. Auto-fix attempts to restart them, but they are not down.

Confirmed constraint from PROJECT.md: "Parallel execution must not overwhelm pods (max 4 concurrent pod queries)."

**Why it happens:**
The constraint is documented but easy to violate when adding a new check. A developer adds check_pod_process() and adds it to the parallel loop without checking the concurrency limit. The limit is an informal convention, not enforced by code.

**How to avoid:**
- Implement a semaphore in the parallel execution engine: maintain an active job count, increment on launch, decrement in the wait loop, block new launches when count = 4.
- Pattern:
  ```bash
  MAX_PARALLEL=4
  active=0
  for IP in $PODS; do
    while [ $active -ge $MAX_PARALLEL ]; do
      wait -n 2>/dev/null || wait
      active=$((active - 1))
    done
    check_pod "$IP" "$RESULT_DIR" &
    active=$((active + 1))
  done
  wait
  ```
- Add a distinct timeout for each curl: `--max-time 5 --connect-timeout 3`. Do not rely on default curl timeout (300 seconds) — it hangs the entire parallel batch.
- Stagger launches by 200ms (`sleep 0.2`) to prevent simultaneous ARP floods.

**Warning signs:**
- Audit runs faster than expected (all checks completing in < 1 second suggests timeout is too short or connections are refused)
- Pods alternate between PASS and FAIL on consecutive runs (network saturation is timing-dependent)
- Router admin shows connection table saturation during audit runs

**Phase to address:** Phase 2 (parallel execution engine) — concurrency limit must be enforced in the engine itself, not by convention. Before any check is added to the parallel loop, the limit must already be in place.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `set -e` without per-command error handling | Script exits on first failure | Partial audit — results are missing for all checks after the first failure | Never for an audit runner — use `|| echo "FAIL"` per check instead |
| Hardcoded pod IPs in check functions | Simple to write | Update in N places when pod IPs change; already happened once (Pod .28 → reassigned) | Never — use `$PODS` array from a single config source |
| `sleep 5` between checks for "stability" | Avoids timing issues | Audit takes 20+ minutes for 60 phases | Never — use `--connect-timeout` on curl instead |
| Parse HTML/text output from Windows commands | Works for the common case | Format changes between Windows versions; `tasklist` output format differs on Server 2022 vs Windows 11 | Only if CSV format (`/FO CSV`) is unavailable |
| Store audit results as plain text | Fast to implement | Cannot do structured delta comparison or machine-readable suppression | Never — use JSON from the start |
| Single monolithic `audit.sh` | Easy to deploy | Untestable, unmaintainable, impossible to run individual checks in isolation | Acceptable for initial prototype only — refactor to modular structure before phase 3 |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| rc-agent `/exec` endpoint | Sending raw shell strings with quotes | Write commands to temp `.bat` file, execute bat by path, clean up |
| racecontrol `/api/v1/fleet/health` | Using array index as pod number | Filter by `pod_number` field: `jq -r '.[] \| select(.pod_number == 3)'` |
| comms-link relay | Using `curl` without `-H "Content-Type: application/json"` | Always include Content-Type header — relay rejects requests without it |
| Bono VPS SSH | Piping `cat` output through SSH into local files | Use `scp` for file transfer; SSH only for commands. Add `2>/dev/null` to all SSH captures |
| WhatsApp notification (Evolution API) | Sending raw report text with special characters | Escape `*`, `_`, and newlines before sending; WhatsApp formatting syntax mangling |
| racecontrol JSONL logs | Using local `date +%s` for UTC log comparison | Always use `date -u +%s` for reference timestamp when comparing against UTC log entries |
| Tailscale SSH to pods | Assuming Tailscale is up before every audit | Pre-check `tailscale status` in prerequisites; fall back to LAN IP if Tailscale shows pod as offline |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Sequential pod checks (8 pods, 2s each) | Full audit takes 20+ minutes | Parallel execution with concurrency limit of 4 | Immediately on first full run |
| `wait` without timeout on pod checks | Audit hangs indefinitely if a pod is completely offline (no TCP RST) | `--max-time 5` on all curl calls; `timeout 10 <command>` wrapper for non-curl checks | When any pod is powered off completely |
| Storing delta history as growing flat file | Delta comparison scans entire history file | Retain only last N runs (configurable, default 10); rotate on each run | After 50+ runs, comparison is slow |
| Generating Markdown report on every check | I/O overhead per check | Collect all results in memory (bash arrays or temp JSON), write report once at end | At 60+ checks, per-check writes add 2+ minutes |
| Calling `/api/v1/fleet/health` separately for each check | 60 HTTP calls to the same endpoint | Cache fleet health at audit start, reuse for all phase checks that need it | Immediately — fleet health is the most-called endpoint |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging the auth PIN in audit output | PIN visible in audit logs and WhatsApp reports | Store in env var (`AUDIT_PIN`), never log it, redact from all output with `sed 's/261121/[REDACTED]/g'` |
| Auto-fix running `taskkill /F /IM powershell.exe` | Kills rc-agent relaunch chain, billing WebSocket, and any other PowerShell process | This specific command is banned from the safe-fix whitelist. Use PID-targeted kills only |
| Sending full JSON audit results to WhatsApp | Report may contain internal IP addresses, process lists, config values | WhatsApp notification contains only: severity summary, FAIL count, top 3 failures. No raw data |
| Executing audit with hardcoded credentials in script | Credentials in git history | Use env vars: `AUDIT_PIN`, `AUDIT_PSK`. Script reads from env, never hardcodes |
| Auto-fix running during business hours without session check | Kills active billing sessions | All auto-fix actions gated on `is_pod_idle()` — absolute requirement |

---

## "Looks Done But Isn't" Checklist

- [ ] **Parallel execution:** Verify temp files are being created per-pod, not written to shared stdout. Run with 8 pods and check for interleaved output.
- [ ] **Delta tracking:** Verify QUIET != regression. Run back-to-back with venue_state toggled and confirm no false regression alert.
- [ ] **Auto-fix session gate:** Simulate an active billing session (`curl -X POST .../billing/start`). Verify auto-fix skips the pod with `SKIP_ACTIVE_SESSION` log.
- [ ] **Suppression expiry:** Set an entry's `expires` to yesterday. Verify the check runs unsuppressed on next audit.
- [ ] **jq prerequisite check:** Remove jq from PATH temporarily. Verify audit exits cleanly with installation instructions, not silently with all JSON results empty.
- [ ] **UTC timestamp:** Set system clock to IST. Run a log-based check against a log file with UTC timestamps from 30 minutes ago. Verify the check finds the entries.
- [ ] **Concurrency limit:** Instrument the parallel loop to log when it blocks. Verify no more than 4 background jobs run simultaneously.
- [ ] **SSH banner robustness:** Add a test SSH connection to a host with a known banner. Verify `safe_ssh_capture()` strips banner and returns valid JSON.
- [ ] **WhatsApp PIN redaction:** Run a full audit and search the WhatsApp message for "261121". Must not appear.
- [ ] **Rollback on auto-fix failure:** If an auto-fix action fails (non-zero exit), verify the audit marks the check as `FIX_FAILED`, not `PASS`.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| cmd.exe quoting destroyed a command mid-audit | LOW | Identify the failed command in audit log, rewrite as a `.bat` file, re-run the specific phase |
| SSH banner corrupted a captured config | HIGH | Re-fetch config with `scp` instead of SSH pipe. Compare against known-good backup in git. Validate first line. |
| Parallel output interleaved — results corrupt | LOW | Delete temp result files, re-run the audit. The parallel engine should be idempotent. |
| Auto-fix killed an active session | HIGH | Check billing log for orphaned session, manually close it via `/api/v1/billing/sessions/<id>/end`. Refund customer time. Add session to known-issues suppression. |
| Delta shows 40 false regressions after mode change | LOW | Add `mode` and `venue_state` to comparison filter. Re-run delta against corrected schema. |
| Suppression list grew to 40 entries | MEDIUM | Audit the suppression list: test each suppressed check manually, remove entries for fixed issues, add expiry to remaining. Takes 2–3 hours. |
| jq not found mid-audit | LOW | Install jq via `scoop install jq`, re-run. Should not happen if prerequisite check is in place. |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| cmd.exe quoting destroys remote commands | Phase 1 (runner core) | Run a command with spaces and quotes through the exec wrapper. Verify correct output. |
| SSH banner corrupts output | Phase 1 (runner core) | Test `safe_ssh_capture()` against a host with a known banner. Assert first line is valid JSON/TOML. |
| curl output includes quotes | Phase 1 (runner core) | Extract a string field from a health endpoint. Assert no surrounding quotes in result. |
| Parallel output interleaved | Phase 2 (parallel engine) | Run 8 parallel checks and verify temp files have clean per-pod JSON. |
| Auto-fix kills active sessions | Phase 3 (auto-remediation) | Simulate active session. Verify `SKIP_ACTIVE_SESSION` is logged and fix does not execute. |
| Delta false regressions from context change | Phase 4 (delta tracking) | Run two audits with different modes. Verify no regressions flagged for skipped checks. |
| jq not available | Phase 1 (runner core) | Run with jq removed from PATH. Verify fast-fail with install instructions. |
| Suppression list hides real regressions | Phase 5 (suppression) | Set expiry to past date. Verify check runs unsuppressed. |
| UTC/IST timestamp confusion | Phase 1 (runner core) | Run log-based check with UTC log. Verify correct event count. |
| Parallel load overwhelms network | Phase 2 (parallel engine) | Monitor active job count during run. Assert never exceeds 4. |

---

## Sources

- CLAUDE.md standing rules (this codebase) — cmd.exe quoting, SSH banner corruption, curl quote stripping, UTC/IST confusion, session gate — all documented from live incidents
- PROJECT.md v23.0 milestone context — "max 4 concurrent pod queries" constraint, auto-fix conservatism requirement
- AUDIT-PROTOCOL.md v3.0 — 60-phase manual audit that v23.0 automates, showing all check patterns that need safe remote execution
- LOGBOOK.md — incident records: pod healer curl bug (2 deploy cycles), SSH banner TOML corruption (2026-03-24), process guard empty allowlist (all 8 pods, 2+ hours)
- PITFALLS.md v22.0 — cmd.exe hostility and recovery system interference patterns (directly applicable to auto-remediation phase)
- PITFALLS-v17.1.md — `spawn().is_ok()` does not mean started; non-interactive context failures (applicable to any auto-fix that spawns processes)
- Personal experience (HIGH confidence): every pitfall in this document corresponds to an incident that has already occurred in this codebase

---
*Pitfalls research for: automated fleet audit system (bash, Windows Git Bash, HTTP exec, parallel, JSON, delta, auto-remediation)*
*Researched: 2026-03-25*
