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

## Progress

**Execution Order:**
Phases execute in numeric order: 36 → 37 → 38 → 39 → 40

Note: Phase 36 (WSL2 Infrastructure) is the non-negotiable critical path — the mirrored networking and Hyper-V firewall must be verified from an actual pod before any minion is installed or any Rust code is written. Phase 37 (Pod 8 Canary) validates the networking with a real minion and rewrites install.bat — this template is reused in Phase 40. Phase 38 (salt_exec.rs) must compile and be tested against live Pod 8 before any module is considered migrated. Phase 39 (remote_ops.rs Removal) requires characterization tests before any deletion — Refactor Second standing rule. Phase 40 (Fleet Rollout) is the irreversible step; no billing session should be interrupted.

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
