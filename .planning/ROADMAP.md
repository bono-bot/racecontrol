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

## Current Milestone

### v4.0 Pod Fleet Self-Healing (Phases 16–21)

**Milestone Goal:** Every pod survives any failure — crashes, reboots, firewall resets, missing files — without physical intervention. Pods self-heal and remain remotely manageable at all times.

**Motivated by:** 4-hour debugging session on Mar 15, 2026 — Pods 1/3/4 offline due to exec exhaustion, CRLF-damaged batch files, missing firewall rules, rc-agent crash with no restart, no remote diagnostics when HTTP blocked.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 16: Firewall Auto-Config** - rc-agent configures ICMP + TCP 8090 rules in Rust on every startup — eliminates CRLF-damaged batch file failures permanently (completed 2026-03-15)
- [x] **Phase 17: WebSocket Exec** - rc-core can send shell commands to any pod over the existing WebSocket — pods remain manageable even when HTTP port 8090 is firewall-blocked (completed 2026-03-15)
- [x] **Phase 18: Startup Self-Healing** - rc-agent verifies and repairs its own config, start script, and registry key on every boot — pods recover from corrupted config without physical intervention (completed 2026-03-15)
- [x] **Phase 19: Watchdog Service** - rc-watchdog.exe runs as a Windows SYSTEM service and auto-restarts rc-agent in Session 1 after any crash — no more permanent agent death on unhandled panic (completed 2026-03-15)
- [x] **Phase 20: Deploy Resilience** - Deploys verify pod health post-swap, auto-rollback on failure, and fleet summary reports per-pod outcomes — bad deploys can never leave pods permanently offline (completed 2026-03-15)
- [ ] **Phase 21: Fleet Health Dashboard** - Uday can see real-time status for all 8 pods (WS connected, HTTP reachable, version, uptime) from his phone via the kiosk /fleet page

## Phase Details

### Phase 16: Firewall Auto-Config
**Goal**: rc-agent ensures its own firewall rules are correct on every startup — ICMP echo and TCP 8090 open with profile=any — so pods are always reachable from the server after any reboot or firewall reset
**Depends on**: Phase 13.1 (v3.0 complete)
**Requirements**: FW-01, FW-02, FW-03
**Success Criteria** (what must be TRUE):
  1. After a fresh reboot of any pod, port 8090 is reachable from the server without running any batch file or manual netsh command
  2. rc-agent can be started 10 times in a row and the firewall rule list does not accumulate duplicate entries — idempotency verified by running `netsh advfirewall show` before and after
  3. The firewall rules apply to all network profiles (domain, private, public) — verified by checking `profile=any` in the rule output
  4. rc-agent startup log shows "Firewall configured" before the HTTP server bind line — confirming rules are applied before the port opens
**Plans:** 1/1 plans complete
Plans:
- [ ] 16-01-PLAN.md — Create firewall.rs module and wire into rc-agent startup

### Phase 17: WebSocket Exec
**Goal**: rc-core can send any shell command to any connected pod over the existing WebSocket connection and receive stdout, stderr, and exit code — so pods remain manageable even when HTTP port 8090 is firewall-blocked
**Depends on**: Phase 16
**Requirements**: WSEX-01, WSEX-02, WSEX-03, WSEX-04
**Success Criteria** (what must be TRUE):
  1. From rc-core (or a test harness), a shell command sent via WebSocket to a pod returns the correct stdout/stderr/exit code within 30 seconds — verified with a simple `whoami` or `dir` command
  2. WebSocket exec works correctly even when a simultaneous HTTP exec request fills all 4 HTTP exec slots — the two paths do not compete for the same semaphore
  3. When HTTP port 8090 is blocked on a pod (firewall rule manually deleted), deploy.rs falls back to WebSocket exec and the deploy completes successfully
  4. Each WebSocket exec response includes the same request_id that was sent — confirmed by sending two concurrent commands and verifying responses are correctly correlated
**Plans**: TBD

### Phase 18: Startup Self-Healing
**Goal**: rc-agent detects and repairs its own broken state on every startup — missing config file, CRLF-damaged start script, missing registry key — so a pod recovers from corruption automatically the next time it reboots
**Depends on**: Phase 16
**Requirements**: HEAL-01, HEAL-02, HEAL-03
**Success Criteria** (what must be TRUE):
  1. If the rc-agent.toml config file is deleted from a pod, the next rc-agent startup recreates it from an embedded template and continues running — no manual file copy needed
  2. If the HKLM Run key for rc-agent is deleted, the next rc-agent startup recreates it — verified by checking the registry after a run with the key manually removed
  3. rc-core logs a startup report from each pod within 10 seconds of the pod's WebSocket connecting — the report includes agent version, uptime, config hash, and a crash recovery flag
  4. If rc-agent crashes before writing its startup log, a partial log file exists at `C:\RacingPoint\rc-agent-startup.log` with the last phase name reached before exit
**Plans:** 2/2 plans complete
Plans:
- [ ] 18-01-PLAN.md — Self-heal module + startup log + main.rs wiring (HEAL-01, HEAL-03)
- [ ] 18-02-PLAN.md — StartupReport protocol + core handler (HEAL-02)

### Phase 19: Watchdog Service
**Goal**: rc-watchdog.exe runs as a Windows SYSTEM service that auto-restarts rc-agent in Session 1 after any crash — so an unhandled panic or OOM kill no longer leaves the pod permanently dead until a human physically intervenes
**Depends on**: Phase 18
**Requirements**: SVC-01, SVC-02, SVC-03, SVC-04
**Success Criteria** (what must be TRUE):
  1. After rc-agent is forcibly killed (TaskKill) on any pod, rc-watchdog detects the absence and restarts it in Session 1 within 10 seconds — verified by watching `tasklist` and the pod reconnecting to rc-core
  2. After a pod reboots with no one logged in, rc-watchdog starts automatically and then starts rc-agent in Session 1 without any manual login — the kiosk lock screen appears within 60 seconds of Windows boot
  3. rc-core receives a crash report from the watchdog within 30 seconds of rc-agent dying — the report includes exit code, crash time, and restart count
  4. rc-agent running under the watchdog shows Session# = 1 in `tasklist /v` output — confirmed on Pod 8 canary before fleet rollout
**Plans:** 2/2 plans complete
Plans:
- [ ] 19-01-PLAN.md — Create rc-watchdog crate with service entry, poll loop, Session 1 spawn, and crash reporting (SVC-01, SVC-02, SVC-03)
- [ ] 19-02-PLAN.md — rc-core crash report endpoint + install script + Pod 8 canary verification (SVC-03, SVC-04)

### Phase 20: Deploy Resilience
**Goal**: Deploying a new rc-agent binary is safe — the previous binary is preserved for rollback, health is verified after swap, and if health fails the pod automatically reverts — so a bad deploy can never leave all 8 pods permanently offline
**Depends on**: Phase 17
**Requirements**: DEP-01, DEP-02, DEP-03, DEP-04
**Success Criteria** (what must be TRUE):
  1. After a successful deploy to any pod, `rc-agent-prev.exe` exists at `C:\RacingPoint\` — confirmed by checking the file listing via pod-agent exec after deploy
  2. If a deployed binary crashes immediately on startup, the pod automatically rolls back to the previous binary within 60 seconds — verified by deploying a known-bad binary and watching the pod recover
  3. Staging `rc-agent-new.exe` on a pod does not trigger a Windows Defender quarantine — Defender exclusion for the staging filename is present and verified via registry check at startup
  4. After a fleet deploy across all 8 pods, rc-core logs a per-pod summary showing which pods succeeded, which failed, and which were retried — Uday can see the outcome without SSHing into each pod
**Plans:** 2/2 plans complete
Plans:
- [ ] 20-01-PLAN.md — Self-swap binary preservation + DeployState::RollingBack + automatic rollback on health failure (DEP-01, DEP-02)
- [ ] 20-02-PLAN.md — Defender exclusion self-heal + fleet deploy summary with retry + Pod 8 canary verification (DEP-03, DEP-04)

### Phase 21: Fleet Health Dashboard
**Goal**: Uday can open his phone and see the real-time health of all 8 pods on a single screen — which pods are connected, which are reachable, what version is running, how long they have been up — so he knows the fleet state without calling James
**Depends on**: Phase 19, Phase 20
**Requirements**: FLEET-01, FLEET-02, FLEET-03
**Success Criteria** (what must be TRUE):
  1. Uday opens http://192.168.31.23:3300/fleet on his phone and sees a grid of all 8 pods with their current status — no login required, page loads within 3 seconds
  2. The dashboard shows WebSocket connected status and HTTP reachable status as two separate indicators — a pod with WS up but HTTP blocked is visually distinct from a fully healthy pod
  3. Each pod card shows the rc-agent version number and uptime — after a fleet deploy, Uday can confirm all 8 pods show the new version without running any commands
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 16 → 17 → 18 → 19 → 20 → 21

Note: Phase 16 (Firewall) is independent and ships first for immediate pain relief. Phase 17 (WebSocket Exec) requires rc-common protocol additions as its first implementation step — both rc-agent and rc-core are built together in this phase. Phase 18 (Self-Healing) can be developed in parallel with 17 but deploys after 16 is live. Phase 19 (Watchdog) must come after 18 because crash-restarts bring up a fresh agent — that agent needs firewall and self-healing to work on first restart. Phase 20 (Deploy Resilience) needs WebSocket exec as its fallback path (Phase 17). Phase 21 (Fleet Dashboard) is read-only and depends on health data from all preceding phases.

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
| 14. Events and Championships | v3.0 | 0/? | Deferred | - |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Deferred | - |
| 16. Firewall Auto-Config | v4.0 | 1/1 | Complete | 2026-03-15 |
| 17. WebSocket Exec | v4.0 | 3/3 | Complete | 2026-03-15 |
| 18. Startup Self-Healing | v4.0 | 2/2 | Complete | 2026-03-15 |
| 19. Watchdog Service | v4.0 | 2/2 | Complete | 2026-03-15 |
| 20. Deploy Resilience | 2/2 | Complete   | 2026-03-15 | - |
| 21. Fleet Health Dashboard | v4.0 | 0/? | Not started | - |
