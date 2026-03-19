# Roadmap: RaceControl

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

<details>
<summary>v2.0 Kiosk URL Reliability — 6 phases, 12 plans (Shipped 2026-03-14)</summary>

Phases: Diagnosis → Server-Side Pinning → Pod Lock Screen Hardening → Edge Browser Hardening → Staff Dashboard Controls → Customer Experience Polish

</details>

<details>
<summary>v3.0 Leaderboards, Telemetry & Competitive — Phases 12–13.1 complete, 14–15 paused (2026-03-15)</summary>

Phases complete: Data Foundation → Leaderboard Core → Pod Fleet Reliability (inserted)
Phases paused: Events and Championships (Phase 14), Telemetry and Driver Rating (Phase 15) — deferred until v4.0 completes.

</details>

<details>
<summary>v4.0 Pod Fleet Self-Healing — Phases 16–22 (Shipped 2026-03-16)</summary>

Phases: Firewall Auto-Config → WebSocket Exec → Startup Self-Healing → Watchdog Service → Deploy Resilience → Fleet Health Dashboard → Pod 6/7/8 Recovery and Remote Restart Reliability

</details>

<details>
<summary>v4.5 AC Launch Reliability — Phases 28–32 (Shipped 2026-03-16)</summary>

Phases: Billing-Game Lifecycle → Game Crash Recovery → Launch Resilience → Multiplayer Server Lifecycle → Synchronized Group Play

Key: billing↔game lifecycle wired end-to-end; CM fallback diagnostics; acServer.exe auto-start/stop on booking/billing; kiosk self-serve multiplayer with per-pod PINs; coordinated group launch + continuous race mode + join failure recovery.

</details>

<details>
<summary>v5.0 RC Bot Expansion — Phases 23–26 (Shipped 2026-03-16)</summary>

Phases: Protocol Contract + Concurrency Safety → Crash, Hang, Launch + USB Bot Patterns → Billing Guard + Server Bot Coordinator → Lap Filter, PIN Security, Telemetry + Multiplayer

</details>

<details>
<summary>v5.5 Billing Credits — Phases 33–35 (Shipped 2026-03-17)</summary>

Phases: DB Schema + Billing Engine → Admin Rates API → Credits UI

Key: billing_rates DB table + non-retroactive additive algorithm + in-memory rate cache; four CRUD endpoints for staff rate management; every user-facing screen replaced rupees with credits.

</details>

## Current Milestone

### v6.0 Salt Fleet Management (Phases 36–40)

**Milestone Goal:** Replace the custom pod-agent/remote_ops HTTP endpoint (port 8090) with SaltStack — salt-master on WSL2 James (.27), salt-minion on all 8 pods + server (.23), salt_exec.rs as the server-side integration seam, remote_ops.rs deleted from rc-agent, and deploy workflow fully migrated to Salt.

### v7.0 E2E Test Suite (Phases 41–44)

**Milestone Goal:** Comprehensive end-to-end test coverage for the full kiosk→server→agent→game launch pipeline — Playwright browser tests for all 5 sim wizard flows, curl-based API pipeline tests for billing/launch/game-state lifecycle, deploy verification for binary swap and port conflict detection, and a single master `run-all.sh` entry point reusable for future services (POS, Admin Dashboard).

### v8.0 RC Bot Autonomy (Phases 45–49)

**Milestone Goal:** Raise rc-agent autonomy from 6/10 to 8/10 — fix the CLOSE_WAIT socket leak causing 5/8 pods to self-relaunch every 5 minutes, install panic hooks for FFB safety on crash, deploy local Ollama (qwen3:0.6b + rp-debug model) to all 8 pods so AI diagnosis is instant and offline-capable, add dynamic server-fetched kiosk allowlist to eliminate the #1 manual intervention, auto-end orphaned billing sessions, and auto-reset pods after session end.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 36: WSL2 Infrastructure** - WSL2 Ubuntu 24.04 with mirrored networking, salt-master 3008 LTS, salt-api, and Hyper-V firewall rules running on James (.27) and verified reachable from the pod subnet
- [ ] **Phase 37: Pod 8 Minion Bootstrap** - Salt minion 3008 installed on Pod 8 canary with explicit minion ID, Defender exclusions pre-applied, sc failure recovery configured, key accepted, and install.bat rewritten without pod-agent sections
- [ ] **Phase 38: salt_exec.rs + Server Module Migration** - New salt_exec.rs Rust module wrapping salt-api REST calls, all four server-side modules (deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs) migrated from pod-agent HTTP to Salt
- [ ] **Phase 39: remote_ops.rs Removal** - Characterization tests written covering the WebSocket path, remote_ops.rs deleted from rc-agent, all port 8090 references purged from Rust source and deploy scripts, cargo build clean, Pod 8 canary billing lifecycle verified
- [ ] **Phase 40: Fleet Rollout** - Salt minion deployed to all 8 pods + server via updated install.bat, all keys accepted, salt '*' test.ping returns 9 True, deploy workflow fully migrated to Salt
- [x] **Phase 41: Test Foundation** - Shared shell library, pod IP map, Playwright config, and cargo-nextest configured — the skeleton every other test script sources (completed 2026-03-18)
- [x] **Phase 42: Kiosk Source Prep + Browser Smoke** - data-testid attributes added to kiosk wizard components, pre-test cleanup fixture built, page smoke tests confirm all routes load in a real browser with no SSR/JS errors (completed 2026-03-18)
- [x] **Phase 43: Wizard Flows + API Pipeline Tests** - All 5 sim wizard flows tested per-step in Playwright, API pipeline tests for billing lifecycle and game state, per-game launch validation with PID check, Steam dialog dismissal (completed 2026-03-18)
- [x] **Phase 44: Deploy Verification + Master Script** - Deploy verify script (binary swap, port conflict, agent reconnect), fleet health validation, run-all.sh phase-gated orchestrator, AI debugger error routing (completed 2026-03-18)
- [x] **Phase 45: CLOSE_WAIT Fix + Connection Hygiene** - Fix remote_ops HTTP server socket leak causing 100-134 CLOSE_WAIT sockets on 5/8 pods, fix fleet_health.rs client connection reuse, add SO_REUSEADDR to UDP sockets, mark all sockets non-inheritable, separate health endpoint from exec slot pool. **E2E (v7.0):** Add `tests/e2e/fleet/close-wait.sh` sourcing lib/common.sh + lib/pod-map.sh — verify CLOSE_WAIT count <5 on all pods after 30min soak, verify no 429 slot exhaustion on /health (completed 2026-03-19)
- [x] **Phase 46: Crash Safety + Panic Hook** - Install std::panic::set_hook() to zero FFB + show error lock screen + log crash before exit, check all server port bindings at startup (remote_ops :8090, lock screen :18923, overlay :18925), FFB zero retry logic (3x attempts with escalation), startup health verification message to server. **E2E (v7.0):** Add `tests/e2e/fleet/startup-verify.sh` — after agent restart, verify BootVerification message received by server within 30s, all ports bound, correct build_id (completed 2026-03-19)
- [x] **Phase 47: Local LLM Fleet Deployment** - Ollama + qwen3:0.6b + rp-debug model installed and verified on all 8 pods, rc-agent TOML pointing to localhost:11434, ai_debugger feeds Windows Event Viewer + rc-bot-events.log to LLM (PodErrorContext), Ollama timeout 120s→30s. **E2E (v7.0):** Add `tests/e2e/fleet/ollama-health.sh` — verify `curl localhost:11434/api/tags` returns rp-debug on all 8 pods, verify `ollama generate` returns valid response <5s on each pod (completed 2026-03-19)
- [ ] **Phase 48: Dynamic Kiosk Allowlist** - Server endpoint GET /api/v1/config/kiosk-allowlist, admin panel UI for adding/removing allowed processes, rc-agent fetches allowlist on startup + every 5 min, merges with hardcoded baseline, LLM-based process classifier for unknown processes (ALLOW/BLOCK/ASK). **E2E (v7.0):** Add `tests/e2e/api/kiosk-allowlist.sh` — curl CRUD on allowlist API, verify rc-agent picks up new process within 5min, Playwright test for admin panel UI
- [ ] **Phase 49: Session Lifecycle Autonomy** - Auto-end orphaned billing sessions after configurable threshold (TOML: auto_end_orphan_session_secs), auto-reset pod to idle 30s after session end, game crash pauses billing with auto-resume on relaunch (max 2 retries before auto-end), fast WS reconnect path (skip relaunch if reconnect succeeds within 30s). **E2E (v7.0):** Add `tests/e2e/api/session-lifecycle.sh` — create billing session, verify auto-end after timeout, verify pod reset to idle, verify billing pause on simulated crash
- [ ] **Phase 50: LLM Self-Test + Fleet Health** - self_test.rs with 18 deterministic probes (WS, lock screen, remote ops, overlay, debug server, 5 UDP ports, HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam), local LLM verdict (HEALTHY/DEGRADED/CRITICAL) with correlation and auto-fix recommendations, server /api/v1/pods/{id}/self-test endpoint, expanded auto-fix patterns 8-14 (DirectX, shader cache, memory, DLL, Steam, performance, network). **E2E (v7.0):** Add `tests/e2e/fleet/pod-health.sh` — trigger self-test on all 8 pods via API, assert all HEALTHY, wire into run-all.sh as final phase gate

## Phase Details

### Phase 36: WSL2 Infrastructure
**Goal**: James's machine (.27) runs a reachable salt-master — WSL2 Ubuntu 24.04 with mirrored networking so pods on 192.168.31.x can reach the master directly, both firewall layers open (Windows Defender + Hyper-V), salt-api running for racecontrol server integration, and the full stack auto-starts on Windows boot
**Depends on**: Phase 35 (v5.5 Credits UI — last completed phase)
**Requirements**: INFRA-01, INFRA-02, INFRA-03, INFRA-04, INFRA-05
**Success Criteria** (what must be TRUE):
  1. `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 returns `TcpTestSucceeded: True` — WSL2 mirrored mode is active and the Hyper-V firewall layer is open
  2. `salt-call --local test.ping` inside WSL2 Ubuntu returns True — salt-master process is running and responding
  3. A curl request to `http://192.168.31.27:8000/login` from the racecontrol server (.23) returns a 200 with a token — salt-api is reachable from the server subnet
  4. After a full reboot of James's machine, salt-master and salt-api are running within 60 seconds without manual intervention — Windows Task Scheduler autostart is working
**Plans**: 2 plans

Plans:
- [ ] 36-01-PLAN.md — WSL2 mirrored networking + salt-master 3008 install + Hyper-V firewall rule (INFRA-01, INFRA-02, INFRA-03)
- [ ] 36-02-PLAN.md — salt-api rest_cherrypy config + Windows Task Scheduler autostart (INFRA-04, INFRA-05)

### Phase 37: Pod 8 Minion Bootstrap
**Goal**: Pod 8 is a verified salt minion — silently installed with explicit ID `pod8`, Defender exclusions applied before the installer runs so binaries are not quarantined, Windows Service recovery configured so the minion restarts itself after a stop, key accepted on master, and `salt 'pod8' cmd.run 'whoami'` succeeds; install.bat is rewritten to bootstrap salt-minion instead of pod-agent
**Depends on**: Phase 36
**Requirements**: MINION-01, MINION-02, MINION-03, MINION-04
**Success Criteria** (what must be TRUE):
  1. `salt 'pod8' test.ping` returns True from James's WSL2 terminal — Pod 8 minion is connected and key is accepted
  2. `salt 'pod8' cmd.run 'whoami'` returns the pod's Windows user — remote execution works end-to-end through the WSL2 master
  3. `sc qfailure salt-minion` on Pod 8 shows restart actions at 5s, 10s, 30s — the minion self-restarts after a stop (working around the confirmed Salt Windows service restart bug)
  4. `salt 'pod8' test.ping` still returns True 30 seconds after `sc stop salt-minion` — the sc failure recovery kicked in and restarted the minion service
  5. The rewritten install.bat contains no pod-agent kill, no :8090 firewall rule, and no pod-agent binary reference — only Defender exclusions + rc-agent copy + salt-minion MSI bootstrap
**Plans**: TBD

Plans:
- [ ] 37-01-PLAN.md — Pod 8 minion install: Defender exclusions + silent EXE install with id:pod8 + sc failure config + key accept (MINION-01, MINION-02, MINION-04)
- [ ] 37-02-PLAN.md — Rewrite install.bat: strip pod-agent sections, add salt-minion bootstrap, verify on Pod 8 (MINION-03)

### Phase 38: salt_exec.rs + Server Module Migration
**Goal**: racecontrol has a new `salt_exec.rs` module that wraps salt-api REST calls via the existing reqwest client, and all four modules that currently call port 8090 (deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs) are rewritten to use salt_exec — verified end-to-end against Pod 8 with Pod 8 canary deploy succeeding
**Depends on**: Phase 37
**Requirements**: SALT-01, SALT-02, SALT-03, SALT-04, SALT-05
**Success Criteria** (what must be TRUE):
  1. `cargo test -p racecontrol-crate` passes with salt_exec.rs compiled — the `[salt]` section in racecontrol.toml and `SaltClient` in AppState are wired without breaking existing tests
  2. `fleet_health.rs` reports Pod 8 as `minion_reachable: true` in the staff dashboard — `salt_exec.ping()` replaces the old HTTP health check and the field name is updated
  3. A deploy triggered from racecontrol to Pod 8 via `salt_exec.cp_get_file()` + `salt_exec.cmd_run()` completes with the new rc-agent binary running on the pod — the Python HTTP server + curl pipeline is no longer needed for this operation
  4. `pod_monitor.rs` restarts the rc-agent Windows service on Pod 8 via `salt_exec.service_restart()` — confirmed by checking pod agent reconnect after the restart
  5. `pod_healer.rs` runs a healing command on Pod 8 via `salt_exec.cmd_run()` and the result is logged — all diagnostic parse logic in pod_healer is unchanged, only the transport layer changed
**Plans**: TBD

Plans:
- [ ] 38-01-PLAN.md — salt_exec.rs module: SaltClient, cmd_run, cp_get_file, ping, ping_all, service_restart; [salt] config section; AppState wiring (SALT-01)
- [ ] 38-02-PLAN.md — fleet_health.rs + pod_monitor.rs migration to salt_exec; minion_reachable rename (SALT-03, SALT-04)
- [ ] 38-03-PLAN.md — pod_healer.rs + deploy.rs migration to salt_exec; cp.get_file vs curl decision applied to deploy (SALT-02, SALT-05)

### Phase 39: remote_ops.rs Removal
**Goal**: remote_ops.rs is permanently deleted from rc-agent — but only after characterization tests cover the billing lifecycle WebSocket path, every caller is confirmed migrated, and Pod 8 runs a full billing session without panics; all port 8090 references are purged from Rust source, deploy scripts, training data, and docs
**Depends on**: Phase 38
**Requirements**: PURGE-01, PURGE-02, PURGE-03, PURGE-04, PURGE-05, FLEET-01
**Success Criteria** (what must be TRUE):
  1. Characterization tests for the billing lifecycle WebSocket path (session start, game launch, billing tick, session end, lock screen) are green before any file is deleted — Refactor Second rule satisfied
  2. `grep -r "remote_ops\|8090\|pod.agent" crates/rc-agent/src/` returns no matches — all references purged from rc-agent Rust source including firewall.rs port 8090 rule and main.rs startup call
  3. `cargo build --release -p rc-agent-crate` succeeds and `cargo test` passes — rc-agent compiles cleanly without the remote_ops module
  4. No references to pod-agent or port 8090 remain in deploy scripts, training data pairs, or operational docs — confirmed by grep across the full repo
  5. Pod 8 completes a full billing session (start → game launch → billing ticks → session end → lock screen) with the new rc-agent binary that has no remote_ops module — no panics, no blank screens, billing amounts correct
**Plans**: TBD

Plans:
- [ ] 39-01-PLAN.md — Characterization tests: WebSocket billing lifecycle path covering AppState fields touched by remote_ops.rs (PURGE-01 prerequisite, FLEET-01 prerequisite)
- [ ] 39-02-PLAN.md — Delete remote_ops.rs + purge all Rust source references (firewall.rs, main.rs, constants) + cargo build clean (PURGE-01, PURGE-02, PURGE-05)
- [ ] 39-03-PLAN.md — Purge pod-agent references from scripts/docs/training data + Port 8090 firewall rule removal from install.bat and netsh configs + Pod 8 canary billing lifecycle verify (PURGE-03, PURGE-04, FLEET-01)

### Phase 40: Fleet Rollout
**Goal**: All 8 pods and the server (.23) are running salt-minion 3008 with accepted keys, `salt '*' test.ping` returns 9 True responses, every pod runs rc-agent without remote_ops, and staff can deploy a new rc-agent binary to any pod via Salt from James's machine — the pod-agent era is over
**Depends on**: Phase 39
**Requirements**: MINION-05, FLEET-02, FLEET-03
**Success Criteria** (what must be TRUE):
  1. `salt '*' test.ping` from James's WSL2 terminal returns 9 True responses (pod1–pod8 + server) — all minion keys are accepted and all nodes are reachable
  2. The staff fleet health dashboard shows all 8 pods as `minion_reachable: true` — fleet_health.rs is pulling live Salt ping results
  3. Staff deploys a new rc-agent.exe to Pod 3 via Salt (as a rollout verification step) and the pod reconnects to racecontrol within 30 seconds — the full deploy workflow via Salt works end-to-end without the Python HTTP server
  4. No active billing sessions are interrupted during the rolling minion installation across pods 1–7 + server — install.bat canary discipline preserved (Pod 8 already done, remaining pods installed one at a time)
**Plans**: TBD

Plans:
- [ ] 40-01-PLAN.md — Install salt-minion on pods 1–7 + server via updated install.bat; accept all keys; fleet-wide test.ping (MINION-05, FLEET-02)
- [ ] 40-02-PLAN.md — Verify full deploy workflow via Salt to all pods; confirm staff dashboard shows all minion_reachable; close port 8090 on all pods (FLEET-03)

### Phase 41: Test Foundation
**Goal**: Every test script has a shared library to source — `lib/common.sh` with pass/fail/skip/info helpers, `lib/pod-map.sh` with all 8 pod IPs, Playwright installed with bundled Chromium and `playwright.config.ts` configured for sequential single-worker runs against the live venue server, and cargo-nextest configured for Rust crate tests with per-process isolation
**Depends on**: Phase 40 (v6.0 Fleet Rollout — last v6.0 phase; may also start independently as v7.0 infrastructure)
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-05
**Success Criteria** (what must be TRUE):
  1. Any shell script that sources `lib/common.sh` can call `pass "message"`, `fail "message"`, and `skip "message"` and the output is consistently color-coded with correct exit code tracking — the shared library works
  2. `lib/pod-map.sh` is sourced once and all 8 pod IPs (192.168.31.x) are available as variables to any script in the suite — no more hardcoded IPs scattered across scripts
  3. `npx playwright test --list` from `tests/e2e/` shows discovered spec files and the Playwright config reports `workers: 1`, `fullyParallel: false`, and `baseURL` set from `RC_BASE_URL` — Playwright is installed and configured correctly
  4. `cargo nextest run -p racecontrol-crate` exits 0 with per-process test isolation active — cargo-nextest is configured and Rust crate tests pass under it
**Plans**: 2 plans

Plans:
- [ ] 41-01-PLAN.md — Shared shell library (lib/common.sh, lib/pod-map.sh) + refactor existing scripts (FOUND-01, FOUND-02)
- [ ] 41-02-PLAN.md — Playwright install + config + cargo-nextest install + config (FOUND-03, FOUND-05)

### Phase 42: Kiosk Source Prep + Browser Smoke
**Goal**: The kiosk wizard components have `data-testid` attributes on every interactive element (game selector, track selector, car selector, wizard step indicators, next/back buttons), a pre-test cleanup fixture stops stale games and ends stale billing before each run, and the browser smoke spec confirms every kiosk route returns 200 in a real Chromium instance with no SSR errors, no React error boundaries, and no uncaught JS exceptions
**Depends on**: Phase 41
**Requirements**: FOUND-04, FOUND-06, FOUND-07, BROW-01, BROW-07
**Success Criteria** (what must be TRUE):
  1. `npx playwright test smoke.spec.ts` passes — all kiosk routes (`/`, `/kiosk`, `/kiosk/book`, `/kiosk/pods`) return HTTP 200 in a real Chromium browser with no `pageerror` events and no React error boundary text visible in the DOM
  2. A Playwright spec that selects the game picker by `[data-testid="sim-select"]` and clicks the AC option by `[data-testid="game-option-ac"]` successfully opens the AC wizard — data-testid attributes are present and functional in the live kiosk
  3. Running the pre-test cleanup fixture against a pod with a stale billing session results in that session being ended and the pod returning to Idle state before any test assertion runs — cleanup is idempotent and safe to run on a clean pod
  4. A Playwright test that fails captures a PNG screenshot and a DOM snapshot in `tests/e2e/results/screenshots/` automatically — the screenshot-on-failure hook is wired
  5. Tab, Enter, and Escape key navigation through the wizard reaches the expected step — keyboard navigation simulation works against the live kiosk
**Plans**: 2 plans

Plans:
- [ ] 42-01-PLAN.md — Add data-testid attributes to kiosk wizard source files (FOUND-06)
- [ ] 42-02-PLAN.md — Pre-test cleanup fixture + browser smoke spec + keyboard nav (FOUND-04, FOUND-07, BROW-01, BROW-07)

### Phase 43: Wizard Flows + API Pipeline Tests
**Goal**: All 5 sim wizard flows are exercised step-by-step in Playwright (AC: 13-step full flow; F1 25/EVO/Rally/iRacing: 5-step simplified flow), experience filtering and staff mode bypass are validated in the browser, and curl-based API tests confirm the full billing lifecycle, per-game launch with PID verification, game state machine transitions, and Steam dialog auto-dismissal on Pod 8
**Depends on**: Phase 42
**Requirements**: BROW-02, BROW-03, BROW-04, BROW-05, BROW-06, API-01, API-02, API-03, API-04, API-05
**Success Criteria** (what must be TRUE):
  1. `npx playwright test wizard.spec.ts` passes for all 5 sim types — AC wizard reaches the review step via all 13 steps with track and car selections confirmed; non-AC wizard reaches review via exactly 5 steps with no `select_track` or `select_car` steps present in the DOM
  2. Staff mode test (`?staff=true&pod=pod-8`) navigates the full booking flow without the phone/OTP step appearing — the staff bypass path is exercised end-to-end
  3. The experience filtering spec confirms that selecting F1 25 shows only F1 25 experiences and the Custom button is absent from the DOM — per-game filtering works correctly
  4. `api/billing.sh` exits 0 — a billing session is created, the launch gate rejects a launch request without an active session, the session timer ticks, and the session is ended cleanly
  5. `api/launch.sh` exits 0 for each installed sim — each game reaches `Launching` state with a PID returned or a confirmed `Launching` state within 60s; game state cycles through Idle→Launching→Running→Idle; Steam dialog dismissal via WM_CLOSE is attempted and logged
**Plans**: 2 plans

Plans:
- [ ] 43-01-PLAN.md — Playwright wizard spec: AC flow, non-AC flow, staff mode, experience filtering, UI navigation (BROW-02, BROW-03, BROW-04, BROW-05, BROW-06)
- [ ] 43-02-PLAN.md — Shell API scripts: billing lifecycle + per-game launch with state polling, Steam dismiss, error screenshot (API-01, API-02, API-03, API-04, API-05)

### Phase 44: Deploy Verification + Master Script
**Goal**: A single `run-all.sh` entry point runs all test phases in sequence, aborts on preflight failure, collects exit codes from each phase, writes a `results/summary.json`, and exits with the total failure count — making it usable as a pre-deploy gate; deploy verification confirms binary swap, detects EADDRINUSE after kiosk restart, and validates all 8 pods reconnect after a rolling restart
**Depends on**: Phase 43
**Requirements**: DEPL-01, DEPL-02, DEPL-03, DEPL-04
**Success Criteria** (what must be TRUE):
  1. `bash tests/e2e/run-all.sh` runs all four phases in sequence, prints a summary table with pass/fail counts per phase, exits 0 when all tests pass, and exits with the failure count when any test fails — the master entry point works as a pre-deploy gate
  2. `deploy/verify.sh` detects an EADDRINUSE condition after kiosk restart, polls until port 3300 is free (up to 30s), and only then starts the new kiosk process — the port-free poll loop prevents the documented bind failure
  3. `deploy/verify.sh` verifies binary size changed after a swap, confirms racecontrol process is running on port 8080, and checks `/api/v1/fleet/health` shows all 8 agents reconnected — the full deploy verification sequence completes against Pod 8 as canary
  4. Test failures and error screenshots captured during the run are passed to the AI debugger error log — the `DEPL-04` routing is wired and a test failure produces an entry in the AI debugger input
**Plans**: 2 plans

Plans:
- [ ] 44-01-PLAN.md — Deploy verification script: binary swap, port conflict, fleet health, AI debugger routing (DEPL-01, DEPL-02, DEPL-04)
- [ ] 44-02-PLAN.md — Master run-all.sh orchestrator: phase-gated sequential runner with summary.json (DEPL-03)

### Phase 45: CLOSE_WAIT Fix + Connection Hygiene
**Goal**: Eliminate the CLOSE_WAIT socket leak on port 8090 that causes 5/8 pods to accumulate 100-134 stuck sockets and trigger unnecessary self-relaunches every ~5 minutes — fix the remote_ops axum server to properly close HTTP connections, fix fleet_health.rs to reuse a shared reqwest client, add SO_REUSEADDR to all UDP game telemetry sockets, mark UDP sockets non-inheritable (matching ea30ca3 treatment for :8090), and increase exec slots from 4→8 or separate health checks from exec pool
**Depends on**: None (can proceed independently). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh, run-all.sh
**Requirements**: CONN-HYG-01 through CONN-HYG-05
**Success Criteria** (what must be TRUE):
  1. After 30 minutes of normal fleet_health polling, no pod has >5 CLOSE_WAIT sockets on :8090 — leak is eliminated
  2. Pod self-relaunches from CLOSE_WAIT strike counter drop to zero across 8-hour monitoring window
  3. After rc-agent self-relaunch, all 5 UDP ports bind successfully (no error 10048) — SO_REUSEADDR applied
  4. fleet_health.rs uses a single shared reqwest::Client with connection pooling — no per-request clients
  5. Health endpoint requests never return 429 (slot exhaustion) — separated from exec pool or pool expanded
  6. `bash tests/e2e/fleet/close-wait.sh` passes — E2E verification of CLOSE_WAIT count <5 on all 8 pods after 30min soak test
**Plans**: 2 plans

Plans:
- [ ] 45-01-PLAN.md — rc-agent socket hygiene: Connection:close middleware, UDP SO_REUSEADDR + non-inherit, OnceLock Ollama client, MAX_CONCURRENT_EXECS 4->8 (CONN-HYG-01, CONN-HYG-02, CONN-HYG-03, CONN-HYG-04, CONN-HYG-05)
- [ ] 45-02-PLAN.md — fleet_health.rs pool_max_idle_per_host(0) + close-wait.sh E2E test (CONN-HYG-01)

### Phase 46: Crash Safety + Panic Hook
**Goal**: rc-agent never leaves a pod in an unsafe state after a crash — custom panic hook zeroes FFB and shows error lock screen, all server port bindings are checked at startup with clear error messages on failure, FFB zero retries 3 times before escalating, and a BootVerification message is sent to the server after all subsystems initialize
**Depends on**: None (can proceed independently). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh
**Requirements**: SAFETY-01 through SAFETY-05
**Success Criteria** (what must be TRUE):
  1. A simulated panic in rc-agent (test mode) results in FFB zeroed, "System Error" shown on lock screen, crash logged to rc-bot-events.log, and clean process exit — no orphaned game processes
  2. If port 18923 (lock screen) or 8090 (remote ops) is already in use, rc-agent logs a clear error and exits within 5s — no silent failure
  3. FFB zero failure on first attempt triggers 2 retries at 100ms intervals and logs the final result — verified by test
  4. Server receives BootVerification message within 30s of rc-agent startup showing: WS connected, lock screen port bound, remote ops port bound, HID status, UDP port status
  5. `cargo test -p rc-agent-crate` passes with all new safety tests green
  6. `bash tests/e2e/fleet/startup-verify.sh` passes — E2E verification that BootVerification arrives on all pods after rolling restart
**Plans**: 2 plans

Plans:
- [ ] 46-01-PLAN.md — FFB zero retry + StartupReport protocol extension + server-side fleet health update (SAFETY-03, SAFETY-04, SAFETY-05)
- [ ] 46-02-PLAN.md — Panic hook + port-bind signaling + BootVerification wiring + startup-verify.sh E2E (SAFETY-01, SAFETY-02, SAFETY-04, SAFETY-05)

### Phase 47: Local LLM Fleet Deployment
**Goal**: Every pod runs Ollama locally with the rp-debug model (qwen3:0.6b base, Racing Point system prompt), rc-agent queries localhost:11434 for AI diagnosis with Windows Event Viewer + rc-bot-events.log fed as context (PodErrorContext), Ollama timeout reduced to 30s, and pattern memory pre-seeded with 7 deterministic fix patterns
**Depends on**: Phase 45 (CLOSE_WAIT fix — so local Ollama diagnosis is meaningful). Uses v7.0 E2E: lib/common.sh, lib/pod-map.sh
**Requirements**: LLM-01 through LLM-04
**Success Criteria** (what must be TRUE):
  1. `ollama list` on all 8 pods returns `rp-debug:latest` (522 MB, qwen3:0.6b base) — DONE (deployed 2026-03-19)
  2. rc-agent TOML on all pods has `ollama_url = "http://127.0.0.1:11434"` and `ollama_model = "rp-debug"`
  3. ai_debugger.rs includes PodErrorContext (rc-bot-events.log + Windows Event Viewer + CLOSE_WAIT count + known patterns) in every LLM prompt — DONE (coded 2026-03-19)
  4. debug-memory.json on each pod is pre-seeded with the 7 deterministic fix patterns (success_count=1) — instant replay from first boot
  5. `bash tests/e2e/fleet/ollama-health.sh` passes — E2E verification that rp-debug model responds <5s on all 8 pods
**Plans**: 2 plans

Plans:
- [ ] 47-01-PLAN.md — Modelfile expansion (14 keywords) + seed-debug-memory.sh (7 patterns) (LLM-03, LLM-04)
- [ ] 47-02-PLAN.md — ollama-health.sh E2E test: model presence + response time <5s (LLM-01, LLM-02)

### Phase 48: Dynamic Kiosk Allowlist
**Goal**: Staff can add allowed processes via the admin panel instead of requiring code changes + rebuild + redeploy to all 8 pods — server stores allowlist in DB, serves it via API, rc-agent fetches it on startup and every 5 minutes, merges with hardcoded baseline, and local LLM classifies unknown processes as ALLOW/BLOCK/ASK
**Depends on**: Phase 47 (local LLM needed for process classification). Uses v7.0 E2E: lib/common.sh, Playwright for admin panel
**Requirements**: ALLOW-01 through ALLOW-05
**Success Criteria** (what must be TRUE):
  1. `GET /api/v1/config/kiosk-allowlist` returns the merged allowlist (hardcoded + DB additions)
  2. Admin panel has a "Kiosk Allowlist" section where staff can add/remove process names
  3. rc-agent picks up a newly added process within 5 minutes without restart or redeploy
  4. Unknown process triggers local LLM classification (ALLOW/BLOCK/ASK) — no kill without classification
  5. No false lockdowns occur when a Windows system process runs on any pod
  6. `bash tests/e2e/api/kiosk-allowlist.sh` passes — CRUD test on allowlist API + `npx playwright test allowlist.spec.ts` for admin panel
**Plans**: TBD

### Phase 49: Session Lifecycle Autonomy
**Goal**: rc-agent autonomously handles session end-of-life — auto-ends orphaned billing after configurable timeout, resets pod to idle after session, pauses billing on game crash with auto-resume, and fast-reconnects WebSocket without full relaunch when server blips
**Depends on**: Phase 46 (crash safety must be in place before autonomous billing actions). Uses v7.0 E2E: lib/common.sh, api pipeline tests
**Requirements**: SESSION-01 through SESSION-04
**Success Criteria** (what must be TRUE):
  1. After billing_active=true with no game_pid for 5 minutes (configurable via `auto_end_orphan_session_secs`), rc-agent auto-ends session via server API — no human intervention needed
  2. 30 seconds after session end, pod automatically returns to PinEntry/ScreenBlanked state — no "Session Complete!" stuck forever
  3. On game crash (CRASH-01), billing is paused within 5s. If game relaunches successfully, billing resumes. After 2 failed relaunches, session auto-ends.
  4. When WebSocket drops, if reconnect succeeds within 30s, no self-relaunch occurs — existing state preserved
  5. Orphaned session auto-end triggers a notification to the server for staff visibility
  6. `bash tests/e2e/api/session-lifecycle.sh` passes — billing create → orphan timeout → auto-end → pod reset verified via API
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 36 → 37 → 38 → 39 → 40 → 41 → 42 → 43 → 44 → 45 → 46 → 47 → 48 → 49 → 50

Note: v8.0 phases 45–50 build on v7.0's shipped E2E infrastructure (lib/common.sh, lib/pod-map.sh, Playwright, run-all.sh). Every phase includes E2E test scripts wired into run-all.sh as new fleet/ and api/ test phases. Phases 45 (CLOSE_WAIT) and 46 (Panic Hook) have no dependencies. Phase 47 (LLM Fleet) depends on 45. Phase 48 (Dynamic Allowlist) depends on 47 (needs local LLM for process classifier). Phase 49 (Session Lifecycle) depends on 46. Phase 50 (Self-Test) depends on 46+47 and is the capstone — its pod-health.sh becomes the final gate in run-all.sh.

Note: Phase 36 (WSL2 Infrastructure) is the non-negotiable critical path — the mirrored networking and Hyper-V firewall must be verified from an actual pod before any minion is installed or any Rust code is written. Phase 37 (Pod 8 Canary) validates the networking with a real minion and rewrites install.bat — this template is reused in Phase 40. Phase 38 (salt_exec.rs) must compile and be tested against live Pod 8 before any module is considered migrated. Phase 39 (remote_ops.rs Removal) requires characterization tests before any deletion — Refactor Second standing rule. Phase 40 (Fleet Rollout) is the irreversible step; no billing session should be interrupted.

For v7.0: Phase 41 (Foundation) must complete before any script can source the shared library. Phase 42 (Kiosk Source Prep) must add data-testid attributes before Phase 43 wizard specs can select wizard elements. Phase 43 (Wizard + API) must complete before Phase 44 can wire run-all.sh around phase scripts that do not yet exist.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 2/2 | Complete | 2026-03-14 |
| 8. Pod Lock Screen Hardening | v2.0 | 3/3 | Complete | 2026-03-14 |
| 9. Edge Browser Hardening | v2.0 | 1/1 | Complete | 2026-03-14 |
| 10. Staff Dashboard Controls | v2.0 | 2/2 | Complete | 2026-03-14 |
| 11. Customer Experience Polish | v2.0 | 2/2 | Complete | 2026-03-14 |
| 12. Data Foundation | v3.0 | 2/2 | Complete | 2026-03-14 |
| 13. Leaderboard Core | v3.0 | 5/5 | Complete | 2026-03-15 |
| 13.1. Pod Fleet Reliability | v3.0 | 3/3 | Complete | 2026-03-15 |
| 14. Events and Championships | v3.0 | 5/5 | Complete | 2026-03-16 |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Deferred | - |
| 16. Firewall Auto-Config | v4.0 | 1/1 | Complete | 2026-03-15 |
| 17. WebSocket Exec | v4.0 | 3/3 | Complete | 2026-03-15 |
| 18. Startup Self-Healing | v4.0 | 2/2 | Complete | 2026-03-15 |
| 19. Watchdog Service | v4.0 | 2/2 | Complete | 2026-03-15 |
| 20. Deploy Resilience | v4.0 | 2/2 | Complete | 2026-03-15 |
| 21. Fleet Health Dashboard | v4.0 | 2/2 | Complete | 2026-03-15 |
| 22. Pod 6/7/8 Recovery + Remote Restart Reliability | v4.0 | 2/2 | Complete | 2026-03-16 |
| 28. Billing-Game Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 29. Game Crash Recovery | v4.5 | 2/2 | Complete | 2026-03-16 |
| 30. Launch Resilience | v4.5 | 2/2 | Complete | 2026-03-16 |
| 31. Multiplayer Server Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 32. Synchronized Group Play | v4.5 | 2/2 | Complete | 2026-03-16 |
| 23. Protocol Contract + Concurrency Safety | v5.0 | 2/2 | Complete | 2026-03-16 |
| 24. Crash, Hang, Launch + USB Bot Patterns | v5.0 | 4/4 | Complete | 2026-03-16 |
| 25. Billing Guard + Server Bot Coordinator | v5.0 | 4/4 | Complete | 2026-03-16 |
| 26. Lap Filter, PIN Security, Telemetry + Multiplayer | v5.0 | 4/4 | Complete | 2026-03-16 |
| 27. Tailscale Mesh + Internet Fallback | v5.0 | 5/5 | Complete | 2026-03-16 |
| 33. DB Schema + Billing Engine | v5.5 | 1/1 | Complete | 2026-03-17 |
| 34. Admin Rates API | v5.5 | 1/1 | Complete | 2026-03-17 |
| 35. Credits UI | v5.5 | 1/1 | Complete | 2026-03-17 |
| 36. WSL2 Infrastructure | v6.0 | 0/2 | Not started | - |
| 37. Pod 8 Minion Bootstrap | v6.0 | 0/2 | Not started | - |
| 38. salt_exec.rs + Server Module Migration | v6.0 | 0/3 | Not started | - |
| 39. remote_ops.rs Removal | v6.0 | 0/3 | Not started | - |
| 40. Fleet Rollout | v6.0 | 0/2 | Not started | - |
| 41. Test Foundation | v7.0 | Complete    | 2026-03-18 | 2026-03-19 |
| 42. Kiosk Source Prep + Browser Smoke | 2/2 | Complete    | 2026-03-18 | - |
| 43. Wizard Flows + API Pipeline Tests | 2/2 | Complete    | 2026-03-18 | - |
| 44. Deploy Verification + Master Script | 2/2 | Complete   | 2026-03-18 | - |
| 45. CLOSE_WAIT Fix + Connection Hygiene | 2/2 | Complete   | 2026-03-19 | - |
| 46. Crash Safety + Panic Hook | 2/2 | Complete   | 2026-03-19 | - |
| 47. Local LLM Fleet Deployment | 2/2 | Complete   | 2026-03-19 | - |
| 48. Dynamic Kiosk Allowlist | v8.0 | 0/? | Not started | - |
| 49. Session Lifecycle Autonomy | v8.0 | 0/? | Not started | - |
| 50. LLM Self-Test + E2E Integration | v8.0 | 0/? | Not started | - |
