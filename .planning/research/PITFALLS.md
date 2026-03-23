# Pitfalls Research

**Domain:** OTA update pipeline, feature flag registry, and config push for Windows sim racing fleet — v22.0 Feature Management & OTA Pipeline
**Researched:** 2026-03-23
**Confidence:** HIGH — pitfalls derived directly from live incidents documented in CLAUDE.md and PROJECT.md for this codebase, supplemented by OTA fleet management literature (MEDIUM confidence for general patterns)

---

## Context: What Makes This System Uniquely Dangerous to Update

Before cataloguing pitfalls, understand the properties of this environment that make standard OTA/feature-flag playbooks fail:

1. **Pods can go offline at any time.** Power loss, network drop, Windows Update reboot — any pod can vanish mid-deploy. The system must handle partial fleet state without human intervention.
2. **Active billing sessions must never be interrupted.** A restart that kills a billing session is a financial loss and a customer experience failure. No OTA action is worth interrupting a paying customer.
3. **Four independent recovery systems exist.** self_monitor, rc-sentry, pod_monitor, and WoL all respond to "pod offline" independently. A deploy that makes a pod appear offline for 15 seconds will trigger all four simultaneously, potentially fighting the deploy.
4. **cmd.exe is hostile to quoting.** Every remote command goes through `cmd /C`. JSON payloads, file paths with spaces, environment variables — all get mangled. This has caused four separate production incidents already.
5. **Static CRT.** Binaries are self-contained. The update system cannot assume shared runtime libraries. Every build must be validated for static linkage before deploy.
6. **Manual TOML files are the current source of truth.** Staff has edited these during incidents. Any config push system that silently overwrites a manual emergency fix creates a secondary incident.

Every pitfall below exploits one or more of these properties.

---

## Critical Pitfalls

### Pitfall 1: Update Interrupts Active Billing Session

**What goes wrong:**
OTA pipeline triggers a binary swap or rc-agent restart on a pod mid-session. The billing timer keeps running on the server but rc-agent restarts, losing in-memory session state. Session end never fires cleanly. Customer is charged for dead time, or the session remains "active" in the DB permanently. In the worst case, the pod reboots during a game save.

The pipeline checks `pod.ws_connected == true` and assumes the pod is idle. Connected does not mean idle. A pod running a 45-minute session is fully connected and fully billing.

**Why it happens:**
OTA logic is designed to find "available" pods. The natural availability proxy is connectivity. Developers add the billing session check as a post-launch hardening item — and it never gets added. The billing state lives in rc-agent's memory, not in a field that fleet health polls return by default.

**How to avoid:**
- Add `session_state: Idle | Active | Ending` to the `PodFleetStatus` struct (returned by `/api/v1/fleet/health`). The OTA coordinator must read this field before queuing a pod for any destructive step.
- Gate all destructive OTA actions (binary swap, service restart, pod reboot) on `session_state == Idle`. If a session starts mid-rollout on a pod already in the queue, remove that pod from the current wave immediately.
- Hot-reload config changes and feature flag updates over the existing WebSocket without restarting rc-agent. Binary restarts are only needed for binary-only changes. Treat them as the heavyweight option of last resort.
- Do not re-queue a pod after its session ends during a deploy wave. Re-queue it for the next scheduled deployment window. A deploy that started while a session was active is a signal to try again later, not to pounce the moment the session ends.

**Warning signs:**
- OTA system logs "deploy started" and "deploy complete" without querying `billing_state`
- Pipeline only checks `ws_connected`, not `session_active`
- `PodFleetStatus` struct has no `session_state` field

**Phase to address:** OTA pipeline — session gate must be the first gate, enforced before canary selection, not added later as a safety net

---

### Pitfall 2: Recovery Systems Fight the OTA Restarter

**What goes wrong:**
The OTA pipeline sends `RCAGENT_SELF_RESTART` to begin a binary swap. The swap takes 8–15 seconds. rc-sentry's health poller (polling `localhost:8090/health` every 5s) sees rc-agent go offline during the swap window and restarts rc-agent using the OLD binary. The pod is now running the old binary. The OTA system gets a successful ack from the restart command and thinks deploy succeeded. `build_id` on `/health` is wrong. The deploy is silently rolled back by the watchdog.

Variant 2: The server-side `pod_monitor` triggers a WoL packet because the pod appeared offline during the swap. WoL wakes a pod that was deliberately taken offline for pre-deploy maintenance, creating an offline-restart-offline loop.

Variant 3: All four recovery systems (self_monitor, rc-sentry, pod_monitor, WoL) simultaneously try to recover the same pod during a 30-second binary swap. The pod restarts 3 times in 30 seconds. rc-agent comes back on an inconsistent state.

**Why it happens:**
rc-sentry, pod_monitor, and the OTA pipeline are independent systems with no shared state. Each acts on "pod offline" without knowing why the pod is offline. This is the exact failure mode documented in CLAUDE.md standing rule "Cross-Process Recovery Awareness" — but that rule was written for crash recovery, not OTA.

**How to avoid:**
- Before any OTA deploy step, write a sentinel file to the pod at a known path: `C:\RacingPoint\ota-in-progress.flag` with content `{ "started_at": "<timestamp>", "expected_offline_seconds": 30 }`. rc-sentry must check for this file before triggering a restart — if the flag exists and is fresh (< 60s old), rc-sentry backs off.
- The OTA coordinator must push a `DeployInProgress { pod_id, expected_duration_secs }` message to the server's `AppState` before beginning each pod's deploy. `pod_monitor` must check this state before sending WoL.
- After deploy completes, delete the sentinel file AND verify `build_id` matches the manifest. If they disagree, the old binary is running (the watchdog won). Alert and retry.
- The sentinel file approach is the minimum viable coordination. It requires only that rc-sentry reads a local file before restarting — this is already within rc-sentry's capability.

**Warning signs:**
- rc-sentry has no concept of "expected restart" vs "crash restart"
- `pod_monitor` has no suppression input from the OTA coordinator
- Deploy logs show `build_id` correct but rc-sentry logs show an additional restart within the same deploy window
- WoL triggers fire during a deploy wave

**Past incident (direct precedent):** Pod 5 was offline 2+ minutes during v17.0 deploy because taskkill killed rc-agent before the restart command ran. rc-sentry eventually recovered it — but rc-sentry recovering with the OLD binary is the silent failure mode of this pitfall.

**Phase to address:** OTA coordination phase — must be built before canary rollout phase, not after

---

### Pitfall 3: Build ID Mismatch Triggers False Fleet-Wide Redeploy

**What goes wrong:**
The OTA system compares `fleet_health.build_id` against the manifest's expected build ID. A docs-only commit — LOGBOOK.md, CLAUDE.md, a comment fix — changes the git hash. The OTA system builds a new binary, sees all 8 pods on the old hash, and redeploys the entire fleet. The new binary is byte-for-byte identical to the old one. 8 pods restart unnecessarily, including those with active sessions.

Variant: The reverse. A real bugfix commit ships a new binary. The git hash advances. But the binary was already manually deployed to all pods via `scp` before the OTA system ran. The OTA system sees "old hash" and tries to redeploy an already-correct fleet.

**Why it happens:**
`build_id = git rev-parse --short HEAD`. Any commit advances the hash. The OTA system uses hash equality as a proxy for binary identity. CLAUDE.md explicitly flags this: "git log before calling builds 'old'" — but this standing rule is for human debugging, not automated pipeline design.

**How to avoid:**
- Use a content-addressed binary identity: `sha256sum rc-agent.exe | cut -c1-16`. This only changes when the binary actually changes. A docs commit produces the same binary and the same SHA256.
- Store both in the manifest: `{ "git_hash": "abc123", "binary_sha256": "d4e5f6..." }`. Deploy gates use `binary_sha256` for equality. `git_hash` is for auditability and rollback navigation only.
- Gate CI build triggers on path-filtered changes: only rebuild when files under `crates/rc-agent/` or `crates/rc-common/` changed. Commits that only touch `docs/`, `LOGBOOK.md`, `CLAUDE.md`, or `.planning/` never trigger a binary build or deploy.

**Warning signs:**
- CI builds a new binary on every push regardless of changed paths
- OTA manifest uses only `git_hash` for version identity
- Fleet redeploy runs after a LOGBOOK.md commit
- OTA system logs "all pods outdated" after a docs-only commit

**Past incident (direct precedent):** All 8 pods on `82bea1eb` were called "old build" — git log showed zero functional rc-agent code changes since that commit. Pods were on the correct build. (Documented in CLAUDE.md standing rules.)

**Phase to address:** CI build triggers and binary identity — must be established before any canary or staged rollout is built on top of it

---

### Pitfall 4: Config Push Overwrites Manual TOML During Outage Recovery

**What goes wrong:**
Staff manually edits `C:\RacingPoint\rc-agent.toml` on a pod to work around an incident while the server is offline. Server comes back online. The config push system sees the pod's config hash differs from the canonical server-side config and pushes the server version — silently overwriting the manual emergency fix. The incident recurs. The manual fix is gone with no log entry.

Variant: The server's stored config is itself broken (the incident was caused by a bad server-side config change). Staff fixed the pod manually. Config push reapplies the broken server config.

**Why it happens:**
Config push systems treat the server as the single source of truth. Manual edits are "drift" to be corrected. This is correct in steady state but wrong during recovery — the manual edit IS the correct state and the server config is the broken one.

**How to avoid:**
- Track config version with a monotonic integer counter, not just a hash. Any write to `rc-agent.toml` — whether from config push or manual edit — must increment the counter. If a pod's counter is greater than the server's stored version for that pod, config push must NOT overwrite. It must alert: "Pod 3 config is newer than server config — manual edit detected, review before pushing."
- Config push must log at WARN level whenever it overwrites an on-pod config, including the old hash and new hash. Silently overwriting is not acceptable.
- On reconnect, the push system compares the pod's current config hash against the last-pushed hash stored server-side. If they differ and the server config hasn't changed since last push, the difference is a manual edit. This triggers a review prompt, not an automatic push.
- The standing rule "smallest reversible fix first" applies here: the config push system should default to read-only (report drift) during the first week of operation, then enable write mode only after the version tracking is validated.

**Warning signs:**
- Config push system has no concept of "who made the last change" or config version counter
- Server outage + manual pod edit + server recovery produces no alert
- Config push completion is logged as success without noting what was overwritten

**Phase to address:** Config push protocol design — the version tracking model must be specified before implementation

---

### Pitfall 5: Partial Rollout Leaves Fleet in Permanent Mixed Feature State

**What goes wrong:**
Canary rollout deploys feature flag `telemetry_v2_format = true` to Pod 8. Pod 8 sends a new telemetry wire format. The server handles both formats. Pods 1–7 send the old format. Three weeks later, "canary succeeded, rollout is done" is the assumed state. A new phase removes the old format handler from the server because "we're on telemetry_v2 now." Pods 1–7 silently drop all telemetry.

The trap: canary succeeded, rollout paused because pods were busy, nobody completed it, the mixed state became the de facto baseline, and code evolved on top of the false assumption.

**Why it happens:**
Staged rollouts have clear start gates but no completion obligation. The canary pod is the exciting milestone. The remaining 7 pods are "just a rollout, we'll do it when convenient." Under any time pressure, "when convenient" becomes never.

**How to avoid:**
- Every staged rollout manifest must include a `complete_by: <timestamp>` field. After that timestamp, the pipeline either: (a) auto-completes the rollout to remaining idle pods (if health metrics are green), or (b) fires an alert requiring an explicit operator decision: "complete rollout" or "roll back." No action is not an option.
- Server-side dual-path handlers (old format + new format) must be coupled to the rollout completion state. The old handler can only be removed in a subsequent release where the manifest marks `rollout_complete: true`.
- Feature flags that affect wire protocol or data format are a different class than UI toggles. They must be explicitly tagged as `breaking_protocol_change: true` in the manifest. These flags require all pods to converge before any old code path can be removed.

**Warning signs:**
- A canary pod has been on a different feature flag value than the rest of the fleet for more than 7 days
- Server has dual-path handlers for the same data format with no scheduled removal date
- Rollout percentage is stuck at 12.5% (1/8 pods) with no pipeline progress and no alert

**Phase to address:** Staged rollout design — completion policy must be designed alongside the rollout mechanism, not left implicit

---

### Pitfall 6: Rollback Loses Active Billing Session State

**What goes wrong:**
A one-command fleet rollback reverts all 8 pods to the previous binary. Three pods have active sessions. rc-agent restarts with the old binary. Billing guard's in-memory session state is gone. The server's `BillingManager` still has the sessions as active. When rc-agent reconnects, the server sends a `SessionSync` message. The old binary's `SessionSync` handler was written before the new session struct was added. It drops unknown fields. Session end never fires cleanly. Billing is unsound.

**Why it happens:**
Rollback is designed for speed under pressure: "get to known-good NOW." Session state preservation feels like an edge case. It only matters when pods are actively billing during a rollback — exactly the scenario where you're already under maximum pressure and least likely to be careful.

**How to avoid:**
- Before any rollback: query all pods for `session_state`. Any pod with `session_state == Active` must first have its session ended cleanly via the `BillingStop` server API. Only after a confirmed clean session end does rollback proceed on that pod.
- The server must persist session state to DB (not just in-memory BillingManager) before rollback begins. The rollback coordinator verifies DB persistence of active sessions before sending `RCAGENT_SELF_RESTART`.
- Rollback must follow the same session-gated sequence as the forward deploy. It must be per-pod with session checks, not a simultaneous fleet-wide blast.
- Keep the session wire protocol backward-compatible across at least two consecutive versions. An rc-agent N-1 binary must be able to receive a `SessionSync` message from a server running N without data loss. Design session structs with `#[serde(default)]` on all new fields.

**Warning signs:**
- Rollback command is a single script that hits all pods simultaneously without checking session state
- Session data lives only in rc-agent memory, not persisted to server DB before rollback
- Session protocol structs use `#[serde(deny_unknown_fields)]` — incompatible with rollback across versions

**Phase to address:** Rollback system design — session-gated rollback must be a first-class requirement, not a post-launch hardening

---

### Pitfall 7: cmd.exe Quoting Breaks Config Push Commands

**What goes wrong:**
The config push system sends a command to pods via the fleet exec endpoint `POST /api/v1/fleet/exec`. The payload contains feature flag values as a JSON string: `{"flags":{"process_guard":true,"pod_label":"Pod 3"}}`. The value `Pod 3` has a space. rc-agent passes this through `cmd /C`. cmd.exe mangles the quoting. The update silently fails — rc-agent returns exit code 0 because cmd.exe didn't error, it just misinterpreted the string. The pod continues running with the old config. The push system sees a 200 and logs "success."

Variant: The config value contains a Windows path `C:\RacingPoint\` with a backslash. cmd.exe interprets `\R` as an escape. The path is corrupted. rc-agent writes a broken TOML file and crashes on next startup.

**Why it happens:**
This is the most-documented class of bug in this codebase. CLAUDE.md has an explicit standing rule for it. But new code written by a developer unfamiliar with the constraint will naturally reach for the fleet exec endpoint for config delivery — it already exists, it already works for commands, why not use it for config values?

**How to avoid:**
- Config push must NEVER route through the fleet exec endpoint. Config push must go over the existing WebSocket as a dedicated typed message: a `ConfigPush { version: u32, flags: HashMap<String, FlagValue> }` enum variant. This path never touches cmd.exe.
- If a fallback shell path is ever needed (it should not be), config values must be written to a temp file first, and the command references only the file path — never inline config values in any string passed through cmd.exe.
- Integration test before any config push phase ships: push a config value containing a space (`Pod 3`), a backslash (`C:\RacingPoint\`), and a dollar sign (`$100`). Read back the value from the pod's TOML. Verify byte-for-byte match.

**Warning signs:**
- Config push implementation uses `fleet/exec` with the config JSON as the command field
- No integration test for config values containing spaces, backslashes, or special characters
- Pod acknowledges config push with HTTP 200 but the on-disk TOML value differs from what was sent

**Past incidents (direct precedent):**
- PowerShell `$r` variable stripped by cmd.exe caused the original pod healer flicker bug — 4 deploy rounds declared "fixed"
- `taskkill /F /IM "GoPro Webcam.exe"` failed because the space in the exe name broke cmd.exe quote parsing
- All documented in CLAUDE.md: "cmd.exe is hostile to quoting"

**Phase to address:** Config push protocol design — WebSocket typed message must be the primary and only path for config data; fleet exec must be explicitly prohibited for this use

---

### Pitfall 8: Standing Rules That Cannot Be Automated Still Get Automated

**What goes wrong:**
The v22.0 goal is to codify all 41+ standing rules as automated pipeline gates. Some rules are machine-checkable (build_id match, test pass/fail, binary SHA256). Some rules are only detectable by a human at the venue (screen rendering, physical hardware state, customer experience). If a human-observable rule is auto-passed by a terminal check, the gate is green and the rule is violated.

The specific failure: the rule "visual verification for display-affecting deploys" is implemented as a health endpoint check. `health_status == ok` passes the gate. The screens are flickering. The pipeline reports PASS. This is the exact failure mode from v17.0 — four rounds of "PASS" while the flicker was visible to anyone in the venue.

**Why it happens:**
All rules look the same in a checklist. Under schedule pressure, every rule gets mapped to an automated check to keep the pipeline moving. The distinction between "machine-checkable" and "human-observable" is not documented, so it doesn't get enforced.

**How to avoid:**
Before writing any automation, classify every standing rule into exactly one of three categories:

| Category | Definition | Pipeline handling |
|----------|-----------|-------------------|
| AUTO | Fully machine-verifiable | Block pipeline on failure; auto-pass on success |
| HUMAN-CONFIRM | Observable only at the venue or requires judgment | Pipeline PAUSES; issues named checklist to operator; cannot proceed without explicit `CONFIRM <rule-id>` |
| INFORMATIONAL | Context that must be acknowledged, no pass/fail | Logged to release notes; no gate |

Known HUMAN-CONFIRM rules from this codebase (these must remain human gates, never auto-passed):
- "Visual verification for display-affecting deploys" — someone must look at the screens
- "Verify what the CUSTOMER sees, not what the API returns" — window titles, overlay rendering
- "Investigate anomalies, don't dismiss them" — requires judgment about whether a spike is expected
- Any standing rule that uses the word "verify visually" or "check the screen"

**Warning signs:**
- All 41 rules map to automated checks with no HUMAN-CONFIRM category
- Visual verification rule implemented as `health_endpoint == 200`
- Pipeline has no PAUSE state — it either blocks or proceeds

**Past incident (direct precedent):** v17.0 browser watchdog caused screen flicker on all pods. Four deploy rounds declared "fixed" without anyone looking at the screens. The flicker was obvious to anyone in the venue. Build IDs, fleet health, and cargo tests all passed. (CLAUDE.md: "Visual verification for display-affecting deploys.")

**Phase to address:** Standing rules codification — classification phase must precede any automation implementation

---

### Pitfall 9: Process Guard Empty Allowlist After Config Push During Server Startup

**What goes wrong:**
Config push queues a config update for offline pods. The server restarts (or comes online after an outage). Before the server's feature flag registry is fully initialized, a pod reconnects and receives the queued config update — which was generated from a partially-initialized registry with an empty or incomplete allowlist. Process guard now enforces an empty allowlist on that pod. Every process is flagged as a violation. `violation_count_24h` hits 100 across all 8 pods. This was dismissed as "expected behavior in report_only mode" without investigating why whitelisted processes (svchost.exe) were being flagged.

**Why it happens:**
Config push queuing is a good pattern — offline pods should receive updates when they reconnect. But the queue does not validate whether the enqueued config was generated from a fully initialized, healthy server state. A server in the middle of startup pushes a broken config, and the pod faithfully applies it.

**How to avoid:**
- Config push messages must include a server-state field: `{ "config_version": 42, "server_healthy": true, "registry_initialized": true, ... }`. Pods must reject any push where `registry_initialized == false`.
- The config push queue must only accept messages after the server passes its own startup health check gate. Messages generated during startup or recovery are quarantined until the server reports `registry_initialized`.
- Alert on `violation_count_24h > 20` across more than 4 pods simultaneously. This pattern is a fleet-wide signal (server-side config push bug), not per-pod noise. It must never be silently dismissed.

**Warning signs:**
- All pods simultaneously showing elevated violation counts after a server restart
- Config push queue has no "was the server healthy when this was enqueued?" field
- `violation_count_24h: 100` on all pods is dismissed as "expected report_only behavior"

**Past incident (direct precedent):** Process guard had empty whitelist on all pods. Fetched config when server was down. Svchost.exe was being flagged. Dismissed as expected without checking WHY whitelisted processes were flagged. (CLAUDE.md: "Investigate anomalies, don't dismiss them.")

**Phase to address:** Config push + process guard integration

---

### Pitfall 10: Cargo Feature Gates Create Deployment Topology Debt

**What goes wrong:**
`rc-agent` is compiled with `--features telemetry,ai_debugger` for pods 1–7 and `--features telemetry,ai_debugger,process_guard` for Pod 8 (canary). The fleet now has two distinct binaries. When a bug is found in shared telemetry code, it must be fixed in both builds, released as both binaries, and deployed in two passes. The "single fleet binary" assumption — built into fleet health comparison, rollback, and canary analysis — breaks everywhere.

Over time: 3 features × per-pod runtime variance = up to 8 binary variants in steady state. The OTA system must track which pod gets which binary. Rollback requires knowing which variant each pod was on. The pipeline complexity grows combinatorially.

**Why it happens:**
Cargo feature gates are elegant for compile-time inclusion. The trap is using them for per-pod runtime configuration rather than for broad capability tiers (debug vs. production). Per-pod configuration belongs in runtime config, not compile-time flags.

**How to avoid:**
- Use Cargo feature gates ONLY for broad capability tiers that are fleet-wide: `debug` (AI debugger symbols, verbose logging, extra diagnostics) vs. `production` (minimal footprint, stripped symbols). All pods get the same tier binary.
- Use the runtime feature flag registry for per-pod enablement: `{ "pod_id": 3, "flags": { "process_guard": false, "ai_debugger": true } }`. The binary compiles all code; the flag registry gates execution at runtime.
- The only legitimate compile-time per-pod variation: code that is physically unsafe to include (e.g., anti-cheat-triggering debug APIs). This is a rare exception, not the general pattern.
- Document the binary matrix policy explicitly before the Cargo feature gate phase ships: "How many distinct production binaries does the fleet support?" The answer must be 1 in steady state, 2 temporarily during canary.

**Warning signs:**
- Deploy system has a `build_config` field with per-pod Cargo feature flag combinations
- Different pods have different binary SHA256 hashes in steady state (not just during canary)
- A bugfix requires building N > 2 binaries
- The OTA manifest tracks "which feature set goes to which pod" rather than "which binary version"

**Phase to address:** Cargo feature gates design — this decision locks in deployment topology for all subsequent phases; it cannot be changed after canary infrastructure is built on top of it

---

### Pitfall 11: Feature Flag Registry Is an Auth-Unprotected Write Endpoint

**What goes wrong:**
The feature flag registry endpoint `POST /api/v1/flags` allows toggling features per-pod. Process guard can be disabled. Billing can be paused. The AI debugger can be enabled. If this endpoint does not require auth, any device on the 192.168.31.x LAN can call it. A pod or a kiosk with a compromised browser can hit the endpoint from within the network. Process guard gets disabled fleet-wide.

The existing `/api/v1/config/kiosk-allowlist` endpoint already has this bug: it requires auth but rc-agent calls it without auth, resulting in 401 responses that silently fall back to the hardcoded local allowlist. The flag registry endpoint will be built by the same team and will likely repeat the same oversight.

**Why it happens:**
Internal LAN endpoints get lower security scrutiny. "It's not exposed to the internet" is treated as sufficient protection. The existing allowlist auth bug demonstrates this pattern is already present in the codebase.

**How to avoid:**
- Feature flag registry write endpoints must require admin authentication (same auth as the admin dashboard).
- rc-agent, if it reads from the flag registry, must do so with a pre-shared API key specific to pod-to-server reads (read-only token, not admin token).
- Gate flag changes that disable safety systems (process guard, billing guard) behind a second confirmation step: the admin must explicitly confirm "disable safety system on Pod N" rather than a single API call.
- Document the auth requirement in the OpenAPI spec before implementation begins. Auth is easier to add at design time than to retrofit.

**Warning signs:**
- Flag registry endpoint is in the same router group as unauthenticated fleet health endpoints
- rc-agent fetches flags using no auth header
- No distinction between "read flags" (pod, authenticated with read token) and "write flags" (admin, authenticated with admin token)

**Phase to address:** Feature flag registry design — auth model must be specified before the endpoint is implemented

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Deploy OTA binary via fleet exec endpoint (existing path) | No new protocol | cmd.exe quoting mangling; exit codes unreliable for binary writes | Never — binary deploy must use RCAGENT_SELF_RESTART + HTTP download pattern |
| Feature flags stored only in racecontrol.toml | Simple, no new infrastructure | Manual TOML editing survives config push; drift undetectable | Never for v22.0 — defeats the purpose of config push |
| Single fleet-wide rollback command (all pods simultaneously) | Fast, simple DX | Hits pods with active billing sessions simultaneously | Never without pre-rollback session drain per pod |
| Skip canary for "small" flag changes | Saves 10 minutes | Feature flag with side effects (enabling process guard, changing billing rate logic) can silently break pods | Never — canary cost is ~10 minutes; incident cost is 45+ minutes |
| Auto-pass visual verification rules | Green pipeline faster | Screen flicker ships undetected; exact failure mode of v17.0 | Never |
| Use git hash as binary identity for deploy gating | One field, simple | Docs-only commits trigger unnecessary 8-pod redeploy | Never for deploy gating — use binary SHA256 |
| Feature flags with no expiry or cleanup date | Flags accumulate easily | Server accumulates permanent dual-path handlers that can never be removed | Acceptable in MVP; must have a cleanup story before the flag count exceeds ~20 |
| Per-pod Cargo feature builds | Enables per-pod capability differences | Binary topology debt; rollback complexity grows combinatorially | Never for runtime configuration — use runtime flag registry instead |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Fleet exec → config push | Sending JSON config as a command string through `cmd /C` | WebSocket `ConfigPush` typed message with Rust enum variant — never through exec endpoint |
| OTA coordinator → rc-sentry | Not suppressing rc-sentry restart during expected binary swap offline window | Write `ota-in-progress.flag` on pod before swap; rc-sentry checks flag before restarting |
| Config push → process guard | Pushing partial config during server startup or recovery | Gate config push on `server_health.registry_initialized == true` |
| Feature flag changes → billing guard | Changing billing-related flags mid-session | Billing flags apply at next session start only; current session is immune to flag changes |
| Rollback coordinator → BillingManager | Rolling back binary with active sessions in memory | Persist session to DB, send BillingStop, verify DB ack, then rollback — never rollback a pod with `session_state != Idle` |
| Staged rollout → server dual-path handlers | Removing old handler after canary without completing fleet rollout | Couple handler removal to `rollout_complete` flag in manifest — removal ships only when `rollout_complete == true` |
| Standing rules enforcement → human-observable rules | Implementing a HUMAN-CONFIRM rule as a health endpoint check | Classify rules before automating — HUMAN-CONFIRM rules must pause the pipeline; they cannot auto-pass |
| OTA binary download → LAN saturation | All 8 pods download new binary simultaneously | Stage-gate download: Pod 8 canary downloads first, verify, then 2-pod waves with 30-second gaps |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Config push on every pod reconnect | 8 pods reconnect after server restart → 8 simultaneous config pushes → server spike | Rate-limit config push to 1 pod per 2 seconds on reconnect burst | On any server restart when all pods reconnect simultaneously |
| Feature flag polling (pull model) | 8 pods polling `/api/v1/flags` every 5s = 96 req/min for flag reads alone | Push-only model over existing WebSocket — server pushes on change; pods do not poll | Immediately visible at 8 pods; catastrophic at scale |
| Manifest + binary download during peak hours | 8 pods downloading simultaneously saturates the 192.168.31.x LAN | Stage downloads: canary first, then 2-pod waves; add inter-wave delay | Any time a release coincides with a busy session period |
| Post-deploy health check hammering pods | OTA system polling all 8 pods every 1s for 30s = 240 requests | Use the existing `/api/v1/fleet/health` aggregate endpoint; poll once per 5s | Immediately on first OTA with health verification enabled |
| OTA pipeline holding fleet health lock during deploy | Other systems (dashboard, alerting) time out waiting for fleet health response | OTA state is a separate struct from fleet health; deploy does not block health queries | Any deploy that takes longer than the health endpoint timeout |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Feature flag registry write endpoint lacks auth | Any LAN device can disable process guard, billing guard, or enable unsafe features | Require admin auth on all flag write endpoints; read endpoints use pod-specific read-only token |
| OTA manifest served over plain HTTP from James (:9998) | MITM on LAN can serve a malicious binary | Sign the manifest with a pre-shared key; rc-agent verifies signature before executing the swap |
| Rollback manifest contains rollback command as a composed string | Injection via malformed version string in manifest | Rollback is a typed Rust command, not a string composed from manifest fields |
| Config push broadcasts to all pods with no pod-specific verification | Server compromise → push malicious config to all 8 pods at once | Config push includes pod-specific HMAC; each pod verifies using its pre-shared key before applying |
| Feature flag enabling process guard bypass is ungated | Single API call can disable a safety system across the fleet | Safety-system flags (process_guard, billing_guard) require a second admin confirmation before applying |

---

## "Looks Done But Isn't" Checklist

- [ ] **OTA Session Gate:** Build ID is updated on all pods — verify `session_state` was checked BEFORE each pod's restart, not just fleet health after the full deploy completes.
- [ ] **Recovery System Coordination:** OTA deploys cleanly on idle pods — verify the same deploy works when rc-sentry is actively running and polling. Check rc-sentry logs during deploy to confirm it did NOT trigger a restart.
- [ ] **Config Push Delivery:** Server shows config pushed successfully — verify the pod's actual on-disk `rc-agent.toml` (or feature flag cache) matches the pushed values byte-for-byte, not just that the HTTP endpoint returned 200.
- [ ] **Rollback Billing Safety:** Rollback completes cleanly on idle pods — verify rollback behavior when exactly one pod has an active session. It must gate on that pod, not abort the entire rollback.
- [ ] **Standing Rules Gate:** Pipeline shows "all rules PASS" — verify at least the four known HUMAN-CONFIRM rules show as PAUSE (requiring operator confirmation), not AUTO-PASS.
- [ ] **Binary Identity:** Fleet shows all pods on the latest `build_id` — verify using binary SHA256 against the manifest, not just git hash equality.
- [ ] **Process Guard After Config Push:** Config push delivers feature flags — immediately check `violation_count_24h` across all pods. If it spiked, the allowlist was empty or partial.
- [ ] **Partial Rollout Completion:** Canary (Pod 8) shows the new feature flag active — verify a `complete_by` timestamp exists in the manifest and the pipeline has a scheduled next wave for pods 1–7.
- [ ] **Cargo Feature Gate Topology:** Feature gate design is finalized — verify all pods in steady state produce the same binary SHA256 (only one production binary variant exists).
- [ ] **Visual Verification:** All automated OTA checks pass — verify someone physically in the venue confirmed screens are showing correctly after the first deploy to a billing-capable pod.
- [ ] **cmd.exe Config Integrity:** Config push with a value containing a space, a backslash, and a dollar sign has been integration-tested — verify the pod's TOML shows the exact bytes sent, not a cmd.exe-mangled version.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Update interrupted active billing session | MEDIUM | Call `BillingStop` via server API for the affected pod → reconcile session end time manually with staff → restart rc-agent → verify session closed in DB → credit customer if session was short |
| Recovery systems fight OTA (rc-sentry restarts old binary) | MEDIUM | Write `ota-in-progress.flag` manually via Tailscale SSH to the affected pod → redeploy that pod → verify `build_id` matches manifest → delete flag |
| Build ID mismatch false redeploy triggered | LOW | Binary SHA256 is correct even if git hash differs — verify SHA256 first before accepting "all pods outdated" → add SHA256 check to verification, no redeploy needed |
| Config push overwrote manual TOML fix | HIGH | Restore manual fix from git history → push corrected config from server → immediately add version counter to prevent recurrence → inform staff what changed |
| Partial rollout stuck in mixed feature state | LOW | Push feature flag to remaining pods via admin UI → verify all pods show the same flag value → mark rollout complete in manifest |
| Rollback lost billing session | HIGH | Query billing DB for sessions with no `end_time` → manually reconcile with venue staff → issue credit if applicable → add rollback session-drain to pipeline before any future rollback |
| Process guard empty allowlist after config push | MEDIUM | Push correct config with full allowlist immediately → verify `violation_count_24h` drops toward zero within 5 minutes → audit server startup sequence for empty-registry config push |
| Cargo feature gate binary proliferation discovered | HIGH | Freeze all new feature gate additions → audit which pods have which binary variant → consolidate to a single binary with runtime flags → this is a design rewrite, not a hotfix |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Update interrupts billing session | OTA pipeline — session gate | Integration test: start billing session on Pod 8, trigger OTA, verify pod is skipped; session completes cleanly |
| Recovery systems fight OTA restarter | OTA coordination — sentinel file design | Test: deploy to Pod 8 while rc-sentry is running; verify rc-sentry logs show it backed off during the swap window |
| Build ID mismatch false redeploy | CI build triggers — path-gated builds + binary SHA256 identity | Commit docs-only change; verify CI does NOT trigger a binary build or fleet deploy |
| Config push overwrites manual TOML | Config push protocol — version counter | Manually edit pod TOML, disconnect/reconnect pod, verify server does NOT overwrite without operator alert |
| Partial rollout permanent mixed state | Staged rollout — completion policy | Verify manifest has `complete_by` field; verify pipeline fires alert after it expires with no completion |
| Rollback loses billing session | Rollback system design — session-gated drain | Integration test: active session on one pod, trigger rollback, verify `BillingStop` fires before binary swap |
| cmd.exe quoting breaks config push | Config push protocol — WebSocket typed message | Integration test: push config with space + backslash + dollar sign; verify pod TOML exact match |
| Standing rules resist automation | Rules classification — HUMAN-CONFIRM category | Audit: 4 known HUMAN-CONFIRM rules exist as PAUSE gates, not AUTO gates |
| Process guard empty allowlist from partial config | Config push + process guard integration | After server restart, push config; immediately check `violation_count_24h < 5` on all pods |
| Cargo features deployment topology debt | Feature gates design | Verify: all pods in steady state have the same binary SHA256 |
| Feature flag registry auth gap | Feature flag registry design | Verify: unauthenticated call to `POST /api/v1/flags` returns 401, not 200 |

---

## Sources

- `CLAUDE.md` standing rules — live incident register for this codebase (HIGH confidence — all cited incidents are real and documented)
- `PROJECT.md` milestone context — v17.0 incident triggers, v17.1 recovery system conflicts, v12.1 process guard empty whitelist (HIGH confidence)
- `MEMORY.md` — Pod 5 offline 2+ minutes v17.0, Pod 6 down 4 times self-restart, screen flicker 4 rounds, build_id false positive, process guard violation_count_24h 100 (HIGH confidence)
- [OTA Update Checklist for Embedded Devices — Memfault](https://memfault.com/blog/ota-update-checklist-for-embedded-devices/) (MEDIUM — general embedded OTA; staged rollout patterns)
- [Firmware OTA Design Patterns and Pitfalls — Arshon](https://arshon.com/blog/firmware-over-the-air-ota-updates-design-patterns-pitfalls-and-a-playbook-you-can-ship/) (MEDIUM — canary progression and kill switch patterns)
- [OTA Best Practices for Industrial IoT — Mender](https://mender.io/resources/reports-and-guides/ota-updates-best-practices) (MEDIUM — offline device handling, partial fleet state)
- [self-replace crate — crates.io](https://crates.io/crates/self-replace/1.3.6) (HIGH — Windows binary replacement: file locking, rename-before-replace requirement)
- [Feature Toggles — Martin Fowler](https://martinfowler.com/articles/feature-toggles.html) (HIGH — flag lifecycle, permanent partial rollout trap, protocol-change flag classification)
- [WebSocket Reconnection State Sync — WebSocket.org](https://websocket.org/guides/reconnection/) (MEDIUM — config push queue on reconnect, sequence number for replay)
- [Challenges With Device OTA Updates — SoftServe](https://www.softserveinc.com/en-us/blog/challenges-with-device-ota-updates) (MEDIUM — maintenance window scheduling, version support windows)

---
*Pitfalls research for: v22.0 Feature Management & OTA Pipeline — Windows sim racing fleet (8 pods)*
*Researched: 2026-03-23 IST*
