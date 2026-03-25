# Pitfalls Research

**Domain:** Debug-First-Time-Right verification framework — retrofitting chain-of-verification, observable state transitions, boot resilience, startup bat auditing, pre-ship verification gates, and silent failure elimination into an existing Rust/Axum fleet management system on Windows 11.
**Researched:** 2026-03-26
**Confidence:** HIGH — every pitfall in this document is sourced from documented incidents in this exact codebase (CLAUDE.md standing rules, PROJECT.md incident log, MEMORY.md audit records). No hypothetical pitfalls.

---

## Context: Why Verification Frameworks Are Hard to Retrofit

Before cataloguing pitfalls, understand what makes this specific retrofit difficult:

1. **Verification wraps existing code paths — it cannot break them.** rc-agent handles active billing sessions, lock screens, and game launches. A verification wrapper that adds latency to the billing start path or panics in the health-check loop causes customer-visible harm. The existing code is correct (mostly); the problem is observability, not logic.

2. **The failure modes already documented are the exact ones to prevent repeating.** This project has an extraordinary record of the SAME bug type recurring: proxy verification passing while the actual behavior remains broken. 8+ incidents of "build_id matches" declared as PASS. 4 deploy cycles of "health endpoint OK" while flicker continued on every screen. The verification framework must prevent this specific failure mode.

3. **Windows sentinel files and registry keys are the coordination primitives.** Every multi-attempt debug incident in this system involved either a missing sentinel check (MAINTENANCE_MODE blocks restart silently) or a missing observable transition (no alert when sentinel was written). The framework must instrument these primitives.

4. **Bat files are the first line of boot enforcement — and the most brittle.** Every manual fix that regressed (power plan, USB suspend, ConspitLink singleton) was not encoded in the bat file. Bat files themselves have Windows-specific syntax traps that silently break enforcement.

5. **Rust logging is not available at config-load time.** The most dangerous silent failures in this system (SSH banner corrupted racecontrol.toml, empty process guard allowlist) occurred before `tracing` was initialized. Verification of early-boot conditions must use a different output path.

---

## Critical Pitfalls

### Pitfall 1: Chain-of-Verification That Only Checks the Endpoint, Not the Chain

**What goes wrong:**
A verification framework is built with health endpoint probes at the leaves (HTTP 200 from `/api/v1/health`). Every step in the chain from input to action is verified by checking whether the final endpoint returns success. The original failures — curl output including surrounding quotes (`"200"` vs `200`), `spawn().is_ok()` != child started, empty allowlist with no error — all returned "healthy" from top-level probes while the internal chain was broken.

Concrete example from this codebase: Pod healer was deployed and declared fixed twice, based on `/health` returning 200 and `build_id` matching expected. The actual bug was `u32::parse()` failing on `"200"` with quotes — visible only by probing the intermediate parse step, not the final endpoint. Four deploy cycles. All declared PASS from terminal output.

**Why it happens:**
End-to-end health endpoints are easy to check (one curl). They feel authoritative. Intermediate parse steps and decision logic are invisible from the outside. The natural impulse is to instrument the most accessible point in the chain, not every link.

**How to avoid:**
Chain-of-verification must be implemented as a structured step sequence at the code level — not a post-hoc health endpoint. Each step produces a typed `VerificationResult` with its own status:
```
Input received → Transform applied → Value parsed → Decision made → Action taken
```
The verification API exposes each step's result independently. A failing parse is visible as `ParseStep::Failed("value='\"200\"' -- expected u32")` without having to reproduce the full chain from outside.

Key rule: for every existing code path that has had a multi-attempt debug incident, identify which link in the chain was the actual failure point. The verification framework must instrument THAT link, not the endpoint.

**Warning signs:**
- Verification only checks HTTP response codes, not response content
- No test exists that specifically breaks the intermediate transformation and confirms the verification catches it
- Verification declarations reference build_id or health endpoint as confirmation of behavioral fix

**Phase to address:** Phase 1 (chain-of-verification framework core) — the framework's data model must represent each step as a distinct trackable unit before any existing code path is wrapped. A framework that only supports leaf-node checking will require a complete rewrite to add intermediate visibility.

---

### Pitfall 2: Observable State Transitions That Log But Don't Alert

**What goes wrong:**
MAINTENANCE_MODE was written to `C:\RacingPoint\MAINTENANCE_MODE` and blocked all pod restarts permanently with no alert to staff. It silently blocked pods 5, 6, and 7 for 1.5+ hours while fleet health showed them as offline. The sentinel existed and was detectable — but no code sent an alert when it was written.

The same pattern: process guard loaded with empty allowlist (server was down at boot), logged the empty count at `DEBUG` level, then continued running for 2+ hours flagging 28,749 false violations per day. The transition "allowlist count changed from healthy to 0" was never instrumented.

Adding observability that writes to the existing `tracing` log at DEBUG level does not prevent the problem. By definition, the operator isn't watching DEBUG logs. Observability must alert on sentinel-class state transitions.

**Why it happens:**
"Add a log line" is the path of least resistance for making state transitions visible. But log lines are passive — they require someone to be watching the right log stream at the right level. The failure mode is not "we couldn't see it if we looked" — it's "nobody was alerted, nobody looked."

**How to avoid:**
Observable state transitions require two things, not one:
1. **A structured event** when the transition happens (not a log line — a typed event emitted to a persistent ring buffer or channel)
2. **An alert** when the transition is to a degraded state, delivered to a surface the operator monitors (fleet health dashboard, WhatsApp via Evolution API, or kiosk badge)

The alert channel already exists: WhatsApp via Evolution API at `racingpoint.in`. The fleet health dashboard already exists at `:8080`. State transitions that produce silent degradation MUST write to one of these.

Specific sentinels requiring alert on write:
- `C:\RacingPoint\MAINTENANCE_MODE` → immediate WhatsApp alert with pod number and restart count
- `C:\RacingPoint\GRACEFUL_RELAUNCH` → no alert needed (healthy transition)
- `C:\RacingPoint\OTA_DEPLOYING` → info-level dashboard update
- Config fallback to defaults → WhatsApp alert ("racecontrol.toml parse failed, running on defaults")
- Process guard allowlist empty at boot → WARN-level alert before any guard enforcement begins
- Feature flag DB unreachable at startup → alert ("feature flags unavailable, using defaults for all pods")

**Warning signs:**
- A sentinel file can be written to disk with no corresponding fleet dashboard update
- `grep -r "MAINTENANCE_MODE" crates/` returns writes but no WhatsApp/alert calls adjacent to those writes
- State transition test only checks that the file exists, not that an alert was sent

**Phase to address:** Phase 2 (observable state transitions) — alert channels must be wired before any sentinel-writing code path is considered instrumented. "Writes a log line at INFO" does not count as observable.

---

### Pitfall 3: Boot Resilience That Fetches Once and Assumes Success

**What goes wrong:**
Process guard fetched the allowlist once at startup from `/api/v1/guard/whitelist/pod-{N}`. The server was briefly down during pod boot. The fetch failed silently, fell back to `MachineWhitelist::default()` (empty list), and process guard ran for 2+ hours flagging every process as a violation. 28,749 false violations. No periodic retry.

The same pattern: feature flags fetched once at startup. If the feature flag DB is unavailable, pods run on compile-time defaults for the entire session. There is no observable transition when this happens and no retry.

**Why it happens:**
"Fetch at startup" is the simplest implementation of initialization. The developer tests on a running system where the server is healthy. The fetch succeeds in every test. The error path (server down at boot) only triggers in production under specific timing conditions.

**How to avoid:**
Any value fetched from a remote source at startup that affects ongoing behavior MUST have a periodic re-fetch loop. The pattern from `821c3031` (process guard fix) is canonical for this codebase:

```rust
// At startup: attempt fetch, fall back to default on failure
// In tokio background task: re-fetch every N seconds, update shared state
// On re-fetch success after previous failure: emit observable state transition
```

Specific values requiring periodic re-fetch in v25.0 scope:
- Process guard allowlist: already fixed in `821c3031` (every 300s)
- Feature flags: re-fetch every 60s (already in v22.0 flag cache)
- Billing rates: re-fetch every 60s (already implemented)
- Config from `racecontrol.toml`: hot-reload trigger on file change (not yet implemented)
- Game launch profiles from TOML: re-fetch on `SIGHUP` equivalent (file watcher)

New requirement: the first re-fetch after a failed boot fetch must emit an observable state transition ("allowlist recovered from server after boot failure — 47 entries loaded").

**Warning signs:**
- A `load_or_default()` call at startup with no background refresh task nearby
- Test coverage for remote fetch failure returns empty default but no test verifies the system self-heals when server comes back
- Re-fetch interval is hard-coded with no observable event on recovery

**Phase to address:** Phase 3 (boot resilience patterns) — the re-fetch pattern must be formalized as a reusable async primitive before the verification framework wraps existing code. Otherwise, each code path independently re-implements the same retry logic (or doesn't).

---

### Pitfall 4: Pre-Ship Verification That Checks the Wrong Domain

**What goes wrong:**
The blanking screen was deployed four times. Each time, the verification was "fleet health shows pods connected, build_id matches." The blanking screen was broken on every deploy — visually obvious to anyone in the venue, invisible to every API probe. The verification checked connectivity (network domain), not screen rendering (visual domain).

The same pattern: kiosk static files returning 404 declared healthy because the health endpoint (which checks page load, not `_next/static/` delivery) returned 200. The deploy was declared complete. The CSS and JavaScript were absent from the UI for an unknown duration.

This is the most recurring failure mode in this codebase's history. The problem is structural: the verification methodology does not match the change domain.

**Why it happens:**
Terminal-accessible verification (curl, health endpoints, build_id) is fast, automatable, and feels authoritative. Visual/behavioral verification requires physical presence or screenshot tools, is slower, and feels subjective. The bias is always toward the terminal-accessible probe, even when the change is visual.

**How to avoid:**
The pre-ship verification gate must enforce domain-matched verification at the process level — not as a guideline but as a checklist that explicitly names the required verification type per change category:

| Change Category | Required Verification | Tools |
|----------------|----------------------|-------|
| Binary deploy (no visual change) | build_id match + health endpoint | curl |
| Lock screen / overlay / blanking | Visual check from venue | User confirmation or `verify-pod-screen.js` |
| Next.js deploy (frontend) | `_next/static/` URL returns 200 + open in browser NOT on server | curl to static asset from POS/James browser |
| Bat file change | `cmd /c` syntax check + `tasklist` verification | Pre-deploy test step |
| Sentinel/config change | Observable alert received + state visible in dashboard | Check WhatsApp or fleet health |
| Network/WS change | Round-trip test from actual pod client to server | WS connection test, not just HTTP |

The gate must fail if the required verification for the change category was not performed. This is a human-process gate, not a code gate — but it must be explicit, not left to judgment.

**Warning signs:**
- Verification summary mentions only build_id and health endpoints for a change that touched lock screen or overlay code
- "Verified" declared from SSH terminal without asking whether the physical screens look correct
- Next.js deploy verified with `curl http://server/` without a `curl http://server/_next/static/` check from a remote client

**Phase to address:** Phase 4 (pre-ship verification gate) — the domain mapping table must be the deliverable for this phase. Every phase in the v25.0 roadmap must reference the gate before being declared complete.

---

### Pitfall 5: Startup Bat Auditing That Finds Issues Without Enforcing Fixes

**What goes wrong:**
Manual fix enforcement is the deepest recurring failure pattern in this codebase. ConspitLink flickering was fixed three times in the same day. Each time: (1) fix applied manually, (2) deploy cycle occurred, (3) fix regressed because `start-rcagent.bat` did not enforce it. The fourth fix — adding enforcement to `start-rcagent.bat` — was permanent.

The same pattern: power plan settings, USB suspend disable, process kill list, firewall rules — all fixed manually, all regressed on the next pod reboot or deploy cycle.

A bat file auditing system that generates a report ("Pod 1 bat is missing 8 enforcement lines") without also deploying the corrected bat file leaves the problem one step from resolved. Manual steps after audit findings are the exact failure mode being addressed.

**Why it happens:**
Audit → report → manual fix is the intuitive flow. The assumption is that "now that we know, we'll fix it." But each deployment cycle brings a fresh opportunity to regress. The only permanent fix is code enforcement that survives deploy cycles automatically.

**How to avoid:**
The bat auditing phase must deliver both detection AND deployment:
1. **Detection:** Compare deployed bat on each pod against canonical bat in git (`start-rcagent.bat`, `start-rcsentry.bat`)
2. **Diff:** Identify which enforcement lines are missing (not just "bat is different" — specifically which category: process kills, power settings, singleton guards, sentinel clears)
3. **Auto-deploy:** For pods where canonical bat exists in git and diff is non-empty, deploy the canonical bat automatically. Do not generate a report and stop.
4. **Verify:** After bat deploy, verify the specific enforcement lines are present on the pod via `/exec` + `findstr`

The auditing system for bats must be closed-loop, not report-only.

**Warning signs:**
- Bat audit generates a diff report without proceeding to deploy the canonical bat
- Canonical bat is not version-controlled (can't determine what "correct" looks like)
- Bat audit treats all bat differences as equivalent (missing process kill line == missing comment == missing enforcement are different severities)

**Phase to address:** Phase 5 (startup bat auditing) — deploy step must be part of the phase scope, not a follow-on action. A bat audit that reports without fixing is not a reliable enforcement mechanism.

---

### Pitfall 6: Logging Initialization Gap Swallows Critical Early-Boot Errors

**What goes wrong:**
SSH banner contaminated `racecontrol.toml`. TOML parse failed. `load_or_default()` silently returned empty defaults. `tracing` was not initialized yet at config-load time. The error was emitted to nowhere. Process guard ran with 0 allowlist entries for 2+ hours. No operator saw anything.

This is the canonical early-boot silent failure mode for this codebase. The Rust `tracing` subscriber requires initialization, which requires a valid configuration, which requires the config to be loaded first. The bootstrap order means the window between "binary started" and "tracing initialized" is where the most dangerous silent failures live.

**Why it happens:**
Developers add observability to production failures using `tracing::error!()`. For 95% of failures, this is correct. For the 5% that happen before `tracing` is initialized, the call is a no-op. There is no compiler warning for a `tracing::error!()` call that will silently do nothing at runtime.

**How to avoid:**
Every error that can occur before tracing initialization MUST use `eprintln!()`, not `tracing::error!()`. This is a standing rule in this codebase — but it is violated in config loading code.

Pattern to audit during v25.0:
```
Search: tracing::error! / tracing::warn! calls in:
- config loading (load_or_default, load_from_path)
- toml::from_str / serde_json::from_str error handlers
- database initialization (SQLite open/migrate)
- feature flag initialization
- process guard allowlist initial fetch
```

For each hit: verify that `tracing` is initialized BEFORE this call is reached in the startup sequence. If not, replace with `eprintln!()` + structured startup log that gets flushed after tracing init.

Additionally: the startup log should be a dedicated `startup.log` file (not JSONL, plaintext) that captures all pre-tracing events. rc-sentry already reads `startup_log` for post-crash diagnosis — this file is the pre-crash equivalent.

**Warning signs:**
- Config parse failure produces no visible output in any log stream
- Error in `fn main()` before `init_logging()` call uses `tracing::error!()`
- `startup.log` is empty when a startup failure occurred (file not opened early enough)

**Phase to address:** Phase 1 (chain-of-verification framework) addresses the verification side; Phase 3 (boot resilience) must address the startup log side. Both phases need to audit for pre-tracing error paths.

---

### Pitfall 7: Verification Framework That Adds Latency to the Billing Hot Path

**What goes wrong:**
The chain-of-verification framework wraps existing code paths to make intermediate steps observable. The billing start path (`BillingStarted` → session creation → rate fetch → WebSocket broadcast) is a latency-sensitive path. Adding synchronous verification steps — even logging steps — to this path can delay billing start and cause the UI to show a stale "starting" state beyond the customer's expectation window.

The PlayableSignal billing fix (v24.0) specifically gates billing on car-controllable state detection. Any added latency in the billing decision path changes when billing starts relative to when the customer is on-track.

**Why it happens:**
Verification is developed in isolation ("instrument everything"). Latency impact is not checked because the test environment does not simulate real pod timing. The billing path appears to work correctly in tests. The latency only becomes visible at runtime on actual hardware with UDP telemetry streams active.

**How to avoid:**
Chain-of-verification wrappers must be added to diagnostic code paths, not hot paths. The rule:
- **Hot paths (billing start, game launch, session end, WS message handling):** Verification via async logging only — fire-and-forget, never blocking. No `await` on verification steps.
- **Cold paths (startup initialization, config load, allowlist fetch, periodic health checks):** Synchronous verification is acceptable — these paths already have latency tolerance.
- **Background paths (pod healer cycle, audit runner, health poller):** Full chain verification with structured step results — this is where the framework adds the most value.

For billing specifically: verification wrapping must emit structured events to a ring buffer (already exists in recovery.rs for pod events) and return immediately. Post-hoc analysis of the ring buffer proves the chain was correct without adding latency to the hot path.

**Warning signs:**
- Verification step in billing path uses `await`
- Billing start duration increases after verification framework is merged
- Test for verification does not measure latency impact on billing path

**Phase to address:** Phase 1 (framework core) — the async fire-and-forget pattern for hot paths must be specified in the framework design before any wrapper is written. A framework design that doesn't distinguish hot/cold/background paths will be applied uniformly and break billing latency.

---

### Pitfall 8: Silent Failure Elimination That Only Covers New Code

**What goes wrong:**
The v25.0 framework adds `eprintln!()` for pre-tracing errors and observable transitions for new sentinels and new config paths. Existing silent failures — the ones already documented in CLAUDE.md — remain silent because they are in existing code paths that are not touched by v25.0. The framework works for everything written after it ships but does not retroactively fix the 6+ known silent failure categories already in production.

Specifically, the known-silent failures in existing code (from PROJECT.md audit evidence):
1. `spawn().is_ok()` without health poll verification (rc-sentry `restart_service()`)
2. Empty allowlist from server-down boot (fixed in `821c3031`, but the error path still logs at DEBUG)
3. Config parse failure before tracing init (racecontrol.toml load path)
4. `MAINTENANCE_MODE` written with no alert (v17.1 added auto-clear + WhatsApp, but 30-min gap remains)
5. Feature flag DB unreachable at startup (no alert)
6. SSL/TOML corruption from SSH pipe (config re-load path, no corruption detection)

**Why it happens:**
Frameworks are built for new code. Retrofitting requires explicitly identifying each existing silent failure and touching existing code — which is slower and more risky than building new abstractions. The path of least resistance is "we'll migrate existing code later."

**How to avoid:**
v25.0 must include an explicit audit-and-fix pass for the 6 known silent failure categories, not just a framework for future code. The framework deliverable is incomplete until the known failures are fixed.

The correct scope for v25.0 is:
1. Build the framework abstractions (chain-of-verification, observable transitions, boot resilience primitives)
2. Apply them to the known failure categories (the 6 above + any discovered during implementation)
3. Add a regression test for each known failure that proves it is now observable

Without step 2, the framework is an improvement for future code only, and the next audit will still find the same silent failure categories.

**Warning signs:**
- v25.0 phases address framework building but no phase explicitly addresses retroactive application to known failures
- `spawn().is_ok()` pattern still exists in rc-sentry `restart_service()` after v25.0 ships
- MAINTENANCE_MODE write in rc-agent still has no same-cycle WhatsApp alert

**Phase to address:** Phase 2 (observable state transitions) must include a sweep of all existing sentinel write sites. Phase 3 (boot resilience) must include a sweep of all existing `load_or_default()` callsites.

---

### Pitfall 9: Bat File Syntax Traps That Break Enforcement Silently

**What goes wrong:**
Every bat file deployment in this codebase that needed multiple attempts failed due to one of four known Windows-specific traps:
1. **UTF-8 BOM** from Claude Code's Write tool — cmd.exe interprets the BOM as a character, causing the first command to fail silently
2. **Parentheses in if/else blocks** — cmd.exe if/else with parentheses has undocumented parsing behaviors; commands inside parenthesized blocks fail silently in certain contexts
3. **`/dev/null` instead of `nul`** — `/dev/null` does not exist on Windows; redirections silently fail, sometimes producing unexpected output
4. **`timeout` command in non-interactive context** — `timeout /T N` requires keyboard input; in a non-interactive SSH session it hangs indefinitely. Use `ping -n N 127.0.0.1 >nul`

Each of these produces silent failure — the bat file "runs" without error but the enforcement line does not execute. The most dangerous case: a singleton guard (`taskkill /F /IM ConspitLink.exe 2>nul`) fails silently due to parentheses syntax, and ConspitLink accumulates to 11 instances.

**Why it happens:**
These are Windows-specific traps that do not exist in bash. Developers writing bat files from a bash-first background apply bash patterns that compile without error but fail at runtime. The traps are not caught by syntax checkers — `cmd /C "type file.bat"` succeeds even on a broken bat.

**How to avoid:**
Every bat file must pass a three-step validation before deploy:
1. **BOM check:** `file start-rcagent.bat | grep -q "BOM" && echo "FAIL: BOM present"`
2. **Syntax test:** `cmd /c "start-rcagent.bat" 2>&1` in a local non-interactive context
3. **Canary verification:** Deploy to Pod 8, wait 10 seconds, verify target process state with `tasklist | findstr <target>`

The canonical bat file creation method for this codebase is bash heredoc + `sed 's/$/\r/'` (adds CRLF). The Write tool must NEVER be used directly for bat files.

Bat file rules (never violate):
- No parentheses in `if`/`else` blocks — use `goto` labels
- No `/dev/null` — use `nul`
- No `timeout /T N` — use `ping -n N 127.0.0.1 >nul`
- No UTF-8 characters in bat files — ASCII only

**Warning signs:**
- Bat file created with the Write tool directly (bypasses CRLF/BOM handling)
- Bat file contains `(` on a line with an `if` or `else` statement
- Bat file contains `/dev/null`
- Enforcement line (process kill, power setting) appears to run but target behavior is unchanged

**Phase to address:** Phase 5 (startup bat auditing) — the auditor must specifically check for these four patterns in addition to comparing against canonical bat. The audit report must distinguish "missing enforcement line" from "present but broken enforcement line" — the latter is worse because it gives false confidence.

---

### Pitfall 10: Cause Elimination Process That Stops at "Found a Crash Dump"

**What goes wrong:**
Pod 6 game crash investigation found crash dumps from `Variable_dump.exe`. The investigation stopped. Fix: "kill Variable_dump.exe on boot." The fix was never verified. Real cause was untested (RAM pressure from 15 orphan PowerShell processes, USB hub fault, FFB driver crash). The crash pattern has appeared again since (per PROJECT.md open issues).

This pattern recurs: a plausible artifact is found, a fix is applied to the artifact, the fix is logged as resolving the issue. Other hypotheses are never tested. If the plausible artifact was correlative rather than causative, the real cause remains unfixed.

The structured Cause Elimination Process (5 steps: document symptom → list ALL hypotheses → test one by one → fix confirmed cause → log) is documented in CLAUDE.md. But it is a guideline. Without a structured template that must be filled in before a fix is declared, the guideline is bypassed under time pressure.

**Why it happens:**
The first plausible explanation breaks the momentum of hypothesis generation. Once a "likely cause" is identified, cognitive closure sets in. The other hypotheses feel like extra work. The incident log shows "found Variable_dump.exe crash dumps" without listing what other hypotheses were considered and eliminated.

**How to avoid:**
The Cause Elimination Process must be enforced by a template, not a guideline. Before any non-trivial bug fix can be committed, the commit description must include:

```
## Cause Elimination
Symptom: [exact observed behavior]
Hypotheses tested:
- [H1]: [test result] → ELIMINATED / CONFIRMED
- [H2]: [test result] → ELIMINATED / CONFIRMED
Confirmed cause: [H?]
Verification: [how the fix was verified to actually fix it]
```

For v25.0, this is a process enforcement deliverable — a commit hook or PR template that requires the section for any commit touching rc-agent, racecontrol, or rc-sentry.

**Warning signs:**
- Fix commit description says "fix X" without mentioning how it was verified
- Fix commit references a single artifact (crash dump, log line) as the sole evidence
- Multiple fixes for the same symptom within 7 days (indicates confirmed-cause was wrong)

**Phase to address:** Phase 6 (Cause Elimination Process enforcement) — commit template must be the deliverable. The standing rule in CLAUDE.md is not sufficient — it is bypassed under time pressure.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `health.is_ok()` as chain verification | Zero new code | Same proxy check problem; next multi-attempt bug will pass health and fail the actual behavior | Never for behavioral fixes |
| Observable state via `tracing::info!()` only | Fast to add | Passive — operator must be watching the right stream; doesn't create fleet health dashboard update | Only for transitions that are never degraded-state (e.g. successful recovery) |
| Boot resilience via "restart the service" | Users work around it | Root cause remains; same issue surfaces every time server reboots | Never as a permanent solution |
| Domain-matched verification as optional checklist | Faster to ship | Same proxy verification pattern; visual bugs declared PASS from terminal | Never — must be enforced per change category |
| Bat audit as report-only | Faster to implement | Audit findings accumulate, manual fix steps are skipped, enforcement regresses on next deploy | Acceptable for first-pass discovery; not for production enforcement |
| Silent failure elimination only for new code | Lower scope | Known failures stay silent; same categories recur; audit finds the same issues | Never — known failures must be fixed in scope |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Chain-of-verification + billing hot path | Wrapping billing start with synchronous verification steps | Async fire-and-forget to ring buffer; verification is post-hoc, never blocking |
| Observable transitions + WhatsApp Evolution API | Sending full structured state on every transition | Send summary only (pod number, sentinel name, timestamp, current state); full detail stays in fleet health JSON |
| Boot resilience + `load_or_default()` | Treating default return as success ("no error") | `load_or_default()` must emit `eprintln!()` for the parse failure AND queue a deferred re-fetch |
| Bat auditing + rc-agent `/exec` endpoint | Sending bat diff commands with quotes through exec | Write audit commands to temp bat, execute bat by path; never send `findstr /C:"..."` through exec (nested quoting breaks) |
| Pre-ship gate + visual changes | Automating visual verification with `verify-pod-screen.js` | Playwright screenshot is a proxy; must ask user "are the screens showing correctly?" as a blocking gate |
| Cause Elimination template + parallel session commits | Other sessions bypass the template | Commit hook must check ALL commits to rc-agent/racecontrol/rc-sentry, not just human commits |
| Silent failure fix + tracing init order | Adding `tracing::error!()` to config load path after verification determines tracing is initialized at that point | Config load must be split: early (pre-tracing, uses `eprintln!`) and late (post-tracing, uses tracing macros) |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Verification logging on every WS message | Log volume overwhelms JSONL appender; latency on WS handler | Verification only on state transitions, not on every message; sample at 1% for high-frequency paths | Immediately at first load test with real pods |
| Synchronous verification step in billing path | Billing start delay observable to customers | All verification in billing path must be async fire-and-forget; measure `billing_start_latency_ms` before and after | Any synchronous await in billing path |
| Bat auditing on all 8 pods in parallel | Same network saturation as audit runner (Pitfall 10 of v23 PITFALLS) | Max 4 concurrent pod queries; stagger bat deploy by 500ms | When all 8 pods queried simultaneously |
| Cause Elimination template on trivial fixes | Template overhead slows down minor fixes | Scope: only required for commits touching core binaries (rc-agent, racecontrol, rc-sentry); not for TOML updates, bat files, Next.js pages | Never — trivial fix scope is clearly defined |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Observable state alerts include config values | `racecontrol.toml` values (IPs, PSK) visible in WhatsApp notifications | Alerts include only: what changed, when, which pod — no config values, no paths, no secrets |
| Cause Elimination template stored as plaintext in repo | Debug artifacts (crash dumps, process lists) may contain PII or credentials | Template is a commit message format — commit message is public; no credential values, no customer data in template fields |
| Bat audit deploying unverified canonical bats | Canonical bat could introduce malicious enforcement line | Canonical bats must come from the main git branch only; hash-verified before deploy to pods |
| Chain-of-verification exposing intermediate state via API | Internal parse failures reveal implementation details to API callers | Verification results are internal telemetry only — not exposed via public API endpoints; fleet health shows aggregate status |

---

## "Looks Done But Isn't" Checklist

- [ ] **Chain-of-verification:** Verify a known broken intermediate step (wrong curl output format) is caught by the framework — not just that the health endpoint is probed
- [ ] **Observable state transitions:** Manually write `MAINTENANCE_MODE` sentinel. Verify WhatsApp alert arrives within 30 seconds. Not just that a log line appears.
- [ ] **Boot resilience:** Kill racecontrol server. Boot a pod. Wait 60 seconds. Restart server. Verify pod self-heals (allowlist loaded, feature flags updated) within next re-fetch interval — without pod restart.
- [ ] **Pre-ship gate:** Apply a visual change to lock screen. Verify the gate explicitly requires visual confirmation from user before marking shipped — not just health endpoint probe.
- [ ] **Bat auditing:** Introduce a known-broken enforcement line (parentheses syntax error in kill command). Verify audit detects it as "present but broken" not "present and correct."
- [ ] **Startup log:** Force a config parse failure before tracing init. Verify error appears in `startup.log` with `eprintln!()` output — not just silently discarded.
- [ ] **Cause Elimination:** Attempt to commit a fix to rc-agent without the Cause Elimination template section. Verify commit hook blocks or warns.
- [ ] **Silent failure sweep:** Run `grep -r "spawn().is_ok()" crates/rc-sentry/` after v25.0 ships. Zero results expected.
- [ ] **Verification latency:** Measure `billing_start_latency_ms` with verification framework active. Must be within 5ms of baseline (framework adds no blocking operations to hot path).

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Verification framework adds latency to billing path | HIGH | Remove synchronous verification steps; replace with async fire-and-forget ring buffer writes; re-measure billing latency |
| Observable alert storms (transition fires repeatedly) | MEDIUM | Add rate limiting to alert channel: max 1 alert per sentinel type per pod per 5 minutes; same rate limiting that exists for email alerts (ALERT-02) |
| Boot resilience re-fetch causes allowlist flip-flop | MEDIUM | Add hysteresis: apply new allowlist only if it has ≥ 80% of previous entry count (prevents accidentally wiping 47-entry list with empty default on transient server error) |
| Bat audit deploys wrong canonical bat version | HIGH | Roll back via Tailscale SSH: `scp canonical.bat ADMIN@<pod>:C:\RacingPoint\start-rcagent.bat`; `schtasks /Run /TN StartRCAgent`; verify via `tasklist` |
| Cause Elimination template blocks urgent hotfix | LOW | Template has an emergency bypass field: `emergency: true` with reason; bypass is logged and reviewed post-incident |
| Silent failure sweep breaks existing behavior | MEDIUM | Each existing silent failure must be fixed with the Smallest Reversible Fix First rule — add `eprintln!()` before the failing call, verify behavior unchanged, then add alert channel |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Chain checks endpoint not chain | Phase 1 (chain-of-verification core) | Break intermediate parse step; verify framework catches it without health endpoint probe |
| Observable transitions log but don't alert | Phase 2 (observable state transitions) | Write MAINTENANCE_MODE sentinel manually; verify WhatsApp alert arrives |
| Boot fetch once, no retry | Phase 3 (boot resilience) | Server down at pod boot; verify self-heal within re-fetch interval |
| Pre-ship gate checks wrong domain | Phase 4 (pre-ship verification gate) | Visual change committed without visual check; verify gate blocks declaration of PASS |
| Bat audit reports without fixing | Phase 5 (startup bat auditing) | Known-broken bat file on pod; verify audit deploys canonical and confirms enforcement |
| Pre-tracing logging gap | Phase 1 + Phase 3 | Config parse failure before tracing init; verify error in startup.log |
| Verification latency on billing path | Phase 1 | Billing latency baseline before and after; verify within 5ms |
| Silent failure sweep incomplete | Phase 2 + Phase 3 | grep for known silent failure patterns in existing code; verify zero hits after sweep |
| Bat file syntax traps | Phase 5 | BOM check + cmd /c test + canary verify; all three must pass before fleet deploy |
| Cause Elimination process bypass | Phase 6 | Attempt commit without template; verify hook fires |

---

## Sources

- CLAUDE.md standing rules (this codebase) — every pitfall in this document is sourced from documented production incidents:
  - Proxy verification (build_id/health): blanking screen flicker (4 deploy rounds), pod healer curl quotes (2 deploy cycles)
  - Silent failures: SSH banner TOML corruption, MAINTENANCE_MODE 1.5h silent block, empty allowlist 2+ hours
  - Boot resilience: process guard empty allowlist at boot (`821c3031` fix)
  - Bat file traps: ConspitLink flicker 3 manual fixes same day, 4 bat deploy attempts for rc-sentry
  - Domain-matched verification: visual changes declared PASS from terminal (standing rule added after 4 incidents)
  - Pre-tracing logging: `racecontrol.toml` parse failure before tracing init (2026-03-24 audit)
  - `spawn().is_ok()`: rc-sentry restart_service() tested 3 methods, all returned Ok, all silently failed
- PROJECT.md v25.0 milestone context — 7 root cause categories from retrospective audit, 11 multi-attempt bugs (avg 2.4 attempts each)
- MEMORY.md — 2026-03-24 audit: Pods 5/6/7 MAINTENANCE_MODE simultaneous; 2026-03-25 audit: process guard empty allowlist; Variable_dump.exe crash investigation incomplete
- PITFALLS-v17.1-watchdog-ai.md — `spawn().is_ok()` false confirmation; non-interactive context spawn failure; MAINTENANCE_MODE silent block; recovery authority conflicts (all directly applicable)
- PITFALLS.md (v23.0 audit runner) — cmd.exe quoting, SSH banner corruption, curl quote stripping (applicable to bat auditing and verification tooling)

---
*Pitfalls research for: Debug-First-Time-Right verification framework — chain-of-verification, observability, boot resilience, bat auditing, pre-ship gates (v25.0)*
*Researched: 2026-03-26*
