# Requirements: v25.0 Debug-First-Time-Right

**Defined:** 2026-03-26
**Core Value:** Eliminate multi-attempt debugging patterns — every bug fixed right the first time

## v1 Requirements

Requirements for v25.0 milestone. Each maps to roadmap phases.

### Observable State Transitions (OBS)

- [ ] **OBS-01**: When MAINTENANCE_MODE sentinel is written (`C:\RacingPoint\MAINTENANCE_MODE`), system emits WhatsApp alert to Uday via Evolution API within 30s including pod number, reason payload (JSON), and IST timestamp. Uses `eprintln!` immediately (pre-tracing) plus queued WhatsApp via existing `app_health_monitor.rs` alert channel. **Incident reference:** 3 pods dark for 1.5hrs (2026-03-24) — no alert existed.
- [ ] **OBS-02**: When any config field falls back to hardcoded default via `unwrap_or()`, system emits `warn!` log with field name, expected source, and fallback value. Covers the 6+ known sites in `rc-agent/src/main.rs` (`unwrap_or("http://127.0.0.1:8080")`, `unwrap_or("127.0.0.1")`) plus `racecontrol/src/config.rs` `load_or_default()`. Uses `eprintln!` if before tracing init, `tracing::warn!` if after. **Incident reference:** SSH banner corrupted racecontrol.toml → silent fallback to empty defaults → process guard ran with 0 allowed entries for 2+ hours.
- [ ] **OBS-03**: When process guard is enabled (`process_guard.enabled = true` in TOML) but fetched allowlist is empty at boot, system: (a) emits `error!` via `eprintln!` before logging init, (b) writes "EMPTY_ALLOWLIST" to `startup_log`, (c) automatically enters `report_only` mode (not `kill_and_report`), (d) sends fleet alert to racecontrol. Threshold: if first scan produces >50% violations across all processes, assume config error not real violations. **Incident reference:** 28,749 false violations/day for 2 days across all 8 pods (2026-03-24).
- [ ] **OBS-04**: When any sentinel file (`MAINTENANCE_MODE`, `GRACEFUL_RELAUNCH`, `OTA_DEPLOYING`, `rcagent-restart-sentinel.txt`) is created or deleted in `C:\RacingPoint\`, system emits structured `AgentMessage::SentinelChange { file, action: Created|Deleted, timestamp }` over WebSocket to racecontrol, which surfaces it in fleet health dashboard (`/api/v1/fleet/health` response gains `active_sentinels: Vec<String>` field). Uses `notify 8.2.0` crate with `RecommendedWatcher` (`ReadDirectoryChangesW` on Windows) for instant detection — no polling.
- [ ] **OBS-05**: All state transitions across the system follow the pattern: `eprintln!` for pre-tracing-init errors (config parse, DB init, startup crashes) and `tracing::warn!(target: "state", prev = ?prev, next = ?new, "transition")` for post-init transitions. Specifically: `rc-sentry/watchdog.rs` FSM transitions (`Healthy→Suspect(N)→Crashed`) must log to `RecoveryLogger` on ALL transitions (currently only logs on `Crashed`). `rc-agent/self_monitor.rs` background task must log lifecycle (start, first-decision, exit). No silent state change anywhere — every `fs::write()` of a sentinel, every `unwrap_or()` fallback, every FSM advance.

### Chain-of-Verification (COV)

- [ ] **COV-01**: New `rc-common/src/verification.rs` (~100 LOC) containing: `VerifyStep` trait with `Input`/`Output`/`Error` associated types, `VerificationChain` builder that records each step's input/output/verdict with `tracing::info_span!` per step, typed `VerificationError` enum via `thiserror 2` with variants `InputParseError`, `TransformError`, `DecisionError`, `ActionError` each carrying the raw value that failed. Hot-path vs cold-path distinction: billing/game launch/WS handling chains use async fire-and-forget to a ring buffer (never blocking); config load/allowlist fetch/health check chains use synchronous verification.
- [ ] **COV-02**: Pod healer curl→stdout→u32 parse chain in `racecontrol/src/pod_healer.rs` wrapped with VerificationChain. Specifically: when pod healer runs `curl -s -w "%{http_code}" http://<pod>:8090/health` via exec, the chain verifies: (Step 1) raw stdout received is non-empty, (Step 2) stdout trimmed of quotes/whitespace, (Step 3) `u32::parse()` succeeds, (Step 4) HTTP code is 200. Each step logs its input/output. If Step 2 or 3 fails, the actual raw value (e.g., `"200"` with quotes) is logged as `ParseStep::Failed(value="\"200\"")` — making the exact failure visible. **Incident reference:** curl output had surrounding quotes breaking u32::parse, 2 deploy cycles declared PASS because health endpoint returned 200.
- [ ] **COV-03**: Config→URL load chain in `rc-agent/src/main.rs` and `racecontrol/src/config.rs` wrapped with VerificationChain. Chain verifies: (Step 1) TOML file exists and is readable, (Step 2) TOML parses without error (no SSH banner garbage), (Step 3) each critical field (api_url, ws_url, server_ip) has a non-default value, (Step 4) URL is reachable (HEAD request within 2s timeout). If Step 2 fails (parse error), the first 3 lines of the file are logged to help diagnose SSH banner corruption. If Step 3 falls back to default, this is a `VerificationError::TransformError` not a silent success. **Incident reference:** SSH banner prepended to racecontrol.toml, load_or_default() silently fell back, process guard ran unprotected for 2+ hours.
- [ ] **COV-04**: Allowlist→enforcement chain in `rc-agent/src/process_guard.rs` wrapped with VerificationChain. Chain verifies: (Step 1) HTTP fetch from `/api/v1/guard/whitelist/pod-{N}` returns 200, (Step 2) response body deserializes to non-empty `MachineWhitelist`, (Step 3) at least `svchost.exe`, `explorer.exe`, and `rc-agent.exe` are in the allowlist (sanity check), (Step 4) if `enabled=true` and allowlist is empty, produce `VerificationError::InputParseError("empty allowlist with guard enabled")` and auto-switch to `report_only`. **Incident reference:** Server restart with fresh DB left pods table empty, allowlist fetch returned empty, 28K false violations/day.
- [ ] **COV-05**: spawn()→child verification chain in `rc-sentry/src/watchdog.rs` and `rc-sentry/src/session1_spawn.rs`. After any `std::process::Command::spawn()` call that returns `Ok(child)`: (Step 1) log spawn success with PID, (Step 2) wait 500ms then check if PID is still alive via `tasklist /FI "PID eq {pid}"`, (Step 3) wait 10s then poll child's health endpoint (e.g., `http://127.0.0.1:8090/health` for rc-agent), (Step 4) if either check fails, log `VerificationError::ActionError("spawn returned Ok but child not running, pid={pid}")` and retry spawn. Never log "restart successful" based solely on `spawn().is_ok()`. **Incident reference:** rc-sentry restart_service() returned Ok for cmd/PowerShell/schtasks — all silently failed, pods dead for days because "restarted=true" was logged but never verified.

### Pre-Ship Verification Gate (GATE)

- [ ] **GATE-01**: Domain-matched verification checklist added to `gate-check.sh` Suite 0. Script prompts for change type classification: `display` (lock screen, overlay, blanking, Edge kiosk), `network` (WS, HTTP endpoints, fleet exec, cloud sync), `parse` (config loading, curl output, telemetry, TOML), `billing` (session start/stop, rate calculation, wallet), `config` (TOML changes, bat file changes, registry). Each type has a required verification method that cannot be substituted. Output includes verification evidence (screenshot path, curl response, data sample). Script exits non-zero if required verification is missing for the declared change type.
- [ ] **GATE-02**: Visual changes (any commit touching: `lock_screen`, `blanking`, `overlay`, `kiosk`, `Edge`, `browser`, `display`, `screen`, CSS/HTML in Next.js apps) blocked from PASS without explicit `VISUAL_VERIFIED=true` flag in gate-check output. Gate script detects visual-domain changes via `git diff --name-only` pattern matching and requires user confirmation "Are the screens showing correctly on the pods?" before proceeding. Cannot be satisfied by health endpoint, build_id, or cargo test. **Incident reference:** Blanking screen — 4 deploy rounds declared PASS without anyone looking at the actual screens.
- [ ] **GATE-03**: Network changes (any commit touching: `ws_handler`, `fleet_exec`, `cloud_sync`, `http`, `api/v1`, WebSocket, port bindings) blocked from PASS without live connection test. Gate script runs: (a) `curl -s http://192.168.31.23:8080/api/v1/health` for server, (b) sample pod exec via fleet endpoint, (c) WS ping if WebSocket code changed. Timeout = 5s. **Incident reference:** Cloud sync URL missing `:8080` — repo was correct but deployed config hit nginx port 80, 404 every 30s.
- [ ] **GATE-04**: Parse changes (any commit touching: `parse`, `from_str`, `serde`, `toml::from_str`, `u32::parse`, `trim`, config loading) blocked from PASS without end-to-end data flow test using real data sample. Gate script requires a test input file and expected output, runs the parse chain, and compares. **Incident reference:** Pod healer curl quotes — health returned 200 but parse chain failed on the actual value.
- [ ] **GATE-05**: Cause Elimination Process enforced via `scripts/fix_log.sh` bash helper. When invoked, prompts for 5 structured fields: (1) Symptom — exact error/behavior observed, (2) Hypotheses — ALL possible causes listed, (3) Elimination — each hypothesis tested with evidence, (4) Confirmed cause — the one that survived elimination, (5) Verification — how the fix was confirmed to work. Output appended to LOGBOOK.md in structured markdown format. Standing rule: any bug taking >30 min to isolate MUST use this process before declaring fixed. **Incident reference:** ConspitLink had 3 independent root causes discovered separately across 3 fix attempts because hypotheses were never listed upfront.

### Boot Resilience (BOOT)

- [ ] **BOOT-01**: New `rc-common/src/boot_resilience.rs` (~60 LOC) containing generic `spawn_periodic_refetch<T>()` function that: (a) accepts a fetch closure, resource name, and interval duration, (b) spawns a `tokio::spawn` background task with `tokio::time::interval`, (c) logs lifecycle events: "periodic_refetch started" on spawn, "periodic_refetch first_success" on first successful fetch, "periodic_refetch exit" if task panics or is cancelled, (d) on fetch failure, logs `warn!` with resource name, error, and retry count, (e) on successful re-fetch after failure, logs `info!` "self_healed" event with resource name and downtime duration. Pattern extracted from the existing process guard allowlist re-fetch (commit `821c3031`).
- [ ] **BOOT-02**: Feature flags in `rc-agent/src/feature_flags.rs` use `spawn_periodic_refetch()` from BOOT-01 with 5-minute interval. If server was down at boot and feature flags loaded from disk cache, the periodic re-fetch self-heals within 5 minutes when server comes back. Observable event emitted on fallback-to-cache AND on self-heal. Currently feature flags are fetched once at boot via WS FlagSync message and never re-fetched if initial load fails.
- [ ] **BOOT-03**: Architectural rule documented in CLAUDE.md standing rules: "Any data fetched from a remote source at startup MUST have a periodic re-fetch background task using `spawn_periodic_refetch()`. Single-fetch-at-boot without retry is a banned pattern." Include checklist of current startup-fetched resources and their re-fetch status: allowlist (done, 5min), feature flags (BOOT-02), billing rates (check), camera config (check).
- [ ] **BOOT-04**: First-scan validation for any guard/filter system. When `enabled` config field changes from `false` to `true` (detected by comparing previous config on disk): (a) run first scan immediately, (b) log first 10 violations with full details, (c) if violation rate >50%, emit `error!` "possible misconfiguration — {N}/{total} processes flagged" and stay in `report_only` mode, (d) require explicit operator confirmation (via fleet exec command `GUARD_CONFIRMED`) before switching to `kill_and_report` mode. **Incident reference:** Process guard enabled with empty allowlist → every process flagged → 28K false violations/day for 2 days.

### Startup Enforcement (BAT)

- [ ] **BAT-01**: New `scripts/bat-scanner.sh` script that: (a) reads canonical `start-rcagent.bat` from repo (`deploy-staging/start-rcagent.bat` or embedded in `self_heal.rs`), (b) for each of 8 pods, fetches deployed bat via rc-sentry `/files` endpoint (`curl -X POST http://<pod_ip>:8091/files -d '{"path":"C:\\\\RacingPoint\\\\start-rcagent.bat"}'`), (c) computes SHA256 hash of both, (d) if hashes differ, shows specific line differences with `diff`, (e) outputs report: `POD N: MATCH` or `POD N: DRIFT — missing: [lines], extra: [lines]`. Also scans `start-rcsentry.bat` on each pod.
- [ ] **BAT-02**: Bat syntax validator integrated into bat-scanner.sh. Checks: (a) no UTF-8 BOM (first 3 bytes not `EF BB BF`), (b) no parentheses in if/else blocks (`if ... ( ... ) else ( ... )` pattern — must use `goto` labels per standing rule), (c) no `/dev/null` redirection (must be `NUL` on Windows), (d) no `timeout` command (must use `ping -n N 127.0.0.1 >nul` for delays in non-interactive context), (e) no `taskkill` without matching `start` or `schtasks` (killing without restarting = permanent death). Reports each violation with line number and suggested fix.
- [ ] **BAT-03**: Bat scanner integrated into fleet audit (`audit/audit.sh`) as new audit phase in Tier 2 (pod-level checks). Phase runs `bat-scanner.sh` as a subprocess, captures output, and reports PASS (all 8 pods match canonical) or FAIL (drift detected on N pods with details). Drift findings recorded in audit delta tracking for comparison between audit runs.
- [ ] **BAT-04**: Deploy chain (`deploy-pod.sh`, fleet exec deploy JSON) updated to include bat file sync step: after binary download and before RCAGENT_SELF_RESTART, deploy the canonical `start-rcagent.bat` and `start-rcsentry.bat` to the pod via rc-sentry `/exec` + `curl.exe -o`. Standing rule enforced: "binary deploy without bat sync is incomplete." Verification: after deploy, re-run bat scanner for the deployed pod to confirm match.

### Fleet Audit Integration (AUDIT)

- [ ] **AUDIT-01**: All v25.0 verification features validated via automated fleet audit (`audit.sh --mode full`) as post-milestone ship gate. New audit tier/phases cover: (a) sentinel alert wiring — write test sentinel, verify WhatsApp received within 30s, (b) config fallback detection — check pod startup logs for any "fallback" warnings, (c) bat drift — bat-scanner.sh as audit phase (BAT-03), (d) boot resilience — verify periodic re-fetch tasks are running via rc-agent health endpoint new field `periodic_tasks: [{name, last_run, status}]`, (e) verification chain — smoke test each wrapped chain with known-good and known-bad inputs.
- [ ] **AUDIT-02**: 5 new audit phases added to audit.sh: Phase `bat-drift` (Tier 2), Phase `sentinel-alerts` (Tier 3 — requires WhatsApp), Phase `config-fallback` (Tier 2), Phase `boot-resilience` (Tier 2), Phase `verification-chains` (Tier 3). Each phase has PASS/FAIL/QUIET criteria documented. Phases use existing audit infrastructure: parallel execution, delta tracking, suppress.json support, dual Markdown+JSON reports.
- [ ] **AUDIT-03**: Audit report gains new "v25.0 Debug Quality" section showing per-pod: (a) active sentinel files, (b) config fallback warnings in last 24h, (c) bat drift status, (d) periodic re-fetch task health, (e) verification chain last-run status. Summary line: `Debug Quality: N/8 pods fully instrumented`. This section becomes a permanent part of the audit report for all future audits.

## v2 Requirements (Deferred)

### Quality Metrics

- **QUAL-01**: Fix attempt counter per incident in LOGBOOK — track avg attempts-to-fix by category. Requires structured LOGBOOK adoption first.
- **QUAL-02**: Boot resilience scorecard in fleet dashboard — shows which pods fetched vs fell back at boot. Requires startup_log enrichment with fetch outcomes.
- **QUAL-03**: Context snapshot at fix declaration — capture build_ids, sentinel states, config hash, allowlist size at fix commit time.
- **QUAL-04**: Automated parse-chain smoke tests for each critical chain — regression tests added alongside each chain fix.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Universal "super health" endpoint | Creates a new proxy to replace old proxies — the health endpoint IS what failed in 8+ incidents |
| Real-time debugging dashboard | Incidents had enough data in existing logs — failure was process discipline, not data availability |
| LLM-gated fix declarations | External dependency per debug session; structured template achieves same outcome with zero deps |
| Global debug mode flag | Use `RUST_LOG=rc_agent::billing_guard=debug` per-target levels instead |
| Autonomous fix application | Safe-action whitelist is intentionally conservative — never extend to billing/config mutations |
| OpenTelemetry / Prometheus stack | Existing tracing + structured logging is sufficient; OTel adds complexity without solving the actual problems |
| Rewriting existing state machines with `statig` | Regression risk; `statig` is for NEW state machines only (v25.0+ forward) |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| COV-01 | Phase 205 | Pending |
| BOOT-01 | Phase 205 | Pending |
| OBS-01 | Phase 206 | Pending |
| OBS-02 | Phase 206 | Pending |
| OBS-03 | Phase 206 | Pending |
| OBS-04 | Phase 206 | Pending |
| OBS-05 | Phase 206 | Pending |
| BOOT-02 | Phase 207 | Pending |
| BOOT-03 | Phase 207 | Pending |
| BOOT-04 | Phase 207 | Pending |
| COV-02 | Phase 208 | Pending |
| COV-03 | Phase 208 | Pending |
| COV-04 | Phase 208 | Pending |
| COV-05 | Phase 208 | Pending |
| GATE-01 | Phase 209 | Pending |
| GATE-02 | Phase 209 | Pending |
| GATE-03 | Phase 209 | Pending |
| GATE-04 | Phase 209 | Pending |
| GATE-05 | Phase 209 | Pending |
| BAT-01 | Phase 210 | Pending |
| BAT-02 | Phase 210 | Pending |
| BAT-03 | Phase 210 | Pending |
| BAT-04 | Phase 210 | Pending |
| AUDIT-01 | Phase 210 | Pending |
| AUDIT-02 | Phase 210 | Pending |
| AUDIT-03 | Phase 210 | Pending |

**Coverage:**
- v1 requirements: 26 total
- Mapped to phases: 26
- Unmapped: 0

---
*Requirements defined: 2026-03-26*
*Last updated: 2026-03-26 — traceability complete after roadmap creation*
