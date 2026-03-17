# Project Research Summary

**Project:** RaceControl v6.0 — Salt Fleet Management (replacing pod-agent/remote_ops.rs with SaltStack)
**Domain:** Infrastructure migration — SaltStack fleet management for 8-node Windows 11 gaming pod fleet via WSL2 Ubuntu master
**Researched:** 2026-03-17
**Confidence:** MEDIUM-HIGH

## Executive Summary

RaceControl v6.0 replaces the custom HTTP endpoint (port 8090, `remote_ops.rs`) with SaltStack for all fleet management operations — binary deployment, service restart, remote command execution, and health checking. The recommended approach is a WSL2 Ubuntu master on James's machine (.27) running SaltStack 3008 LTS with mirrored networking mode, and Windows salt-minion 3008 installed on all 8 gaming pods plus the server (.23). Mirrored networking is non-negotiable: default WSL2 NAT mode gives the Ubuntu instance a 172.x.x.x IP that pods on 192.168.31.x cannot reach, while mirrored mode makes WSL2 inherit the Windows host's LAN IP (192.168.31.27) so pods connect directly. No portproxy scripts, no IP drift.

The migration scope is precisely bounded. Salt replaces only the `remote_ops.rs` HTTP exec path and the Python HTTP server + curl deploy pipeline. It does not touch the WebSocket connection (game state, billing, lock screen), UDP heartbeat, or any application logic — those channels have sub-second latency requirements that Salt's ZeroMQ cannot match. The integration seam on the Rust side is a new `salt_exec.rs` module that calls the `salt-api` REST interface via the existing `reqwest` client — no new Cargo dependencies. racecontrol modules (`deploy.rs`, `fleet_health.rs`, `pod_monitor.rs`, `pod_healer.rs`) are modified to call `salt_exec` instead of the old HTTP endpoint.

The highest-risk pitfalls are all WSL2 networking: NAT mode blocking Salt ports, a Hyper-V firewall layer that activates independently of Windows Defender (and silently drops packets even after mirrored mode is enabled), and the known Salt bug where `service.restart salt-minion` stops the Windows service but never restarts it. The recommended build order is infrastructure-first: get the WSL2 master working and verified from Pod 8 before writing a single line of Rust, then migrate server-side modules one at a time with Pod 8 as the canary, and only delete `remote_ops.rs` after every caller has been migrated.

## Key Findings

### Recommended Stack

SaltStack 3008 LTS is the only correct version for a new deployment. STS 3007 hits EOL 2026-03-31 (this month). 3006 LTS is legacy. The salt-master runs in WSL2 Ubuntu 24.04 using the official Salt bootstrap script. The salt-minion uses the EXE installer format (not MSI — the MSI was used in 3006 era) with silent install flags `/S /master=192.168.31.27 /minion-name=pod1`. All networking uses WSL2 mirrored mode, which requires Windows 11 22H2+ (James's machine is confirmed compatible) and a one-time Hyper-V firewall rule.

**Core technologies:**
- SaltStack 3008 LTS (salt-master in WSL2 Ubuntu 24.04): Fleet orchestration — only current LTS, 2-year support to ~2027
- WSL2 mirrored networking (Windows 11 22H2+): Eliminates NAT portproxy fragility — WSL2 shares LAN IP 192.168.31.27 directly
- Salt Minion 3008.x-Py3-AMD64 EXE: Windows agent on all 9 nodes — bundles its own Python via relenv, no host Python required
- `salt.modules.cmd`: Remote shell execution — replaces `/exec` HTTP endpoint
- `salt.modules.win_service`: Windows service management — replaces `sc.exe` workarounds in pod_monitor.rs
- `salt.modules.cp`: File distribution from master to minions — replaces Python HTTP server + curl pipeline
- `salt-api` (rest_cherrypy, port 8000): REST interface allowing racecontrol to call Salt via reqwest without subprocess or WSL2 boundary crossing

**Critical version requirements:**
- Salt 3008 LTS (not 3007 — EOL March 2026, not 3006 — legacy)
- Windows 11 22H2 build 22621+ for mirrored networking
- EXE installer format (not MSI) for 3007/3008 on Windows

### Expected Features

The migration is a direct capability-for-capability replacement of the custom HTTP exec layer. The MVP is: master running, all 9 minions connected and keys accepted, `cmd.run` verified, `cp.get_file` verified, `service.restart` verified, `remote_ops.rs` deleted from codebase.

**Must have (table stakes — MVP, v6.0 cannot ship without these):**
- WSL2 Salt master with mirrored networking, verified from actual pod (not just from .27)
- Salt minion on all 8 pods + server, keys accepted, `salt 'pod*' test.ping` returns 8 True responses
- Explicit minion ID convention: `pod1`–`pod8` + `server` (set at install time, never auto-generated from hostname)
- `cmd.run` remote execution — replaces `/exec` HTTP endpoint
- `cp.get_file` binary distribution — replaces Python HTTP server + curl pipeline
- `service.restart` / `service.stop` for rc-agent Windows service management
- `remote_ops.rs` deleted from rc-agent codebase
- `install.bat` slimmed: remove pod-agent kill, :8090 firewall rule, add salt-minion EXE bootstrap

**Should have (valuable, add after MVP validation):**
- Custom grains for pod identity (`pod_number`, `role`, `venue`) — enables grain-based targeting
- Nodegroup aliases in master config (`pods: 'pod*'`) — CLI ergonomics
- `state.apply` for idempotent rc-agent state enforcement — replaces manual install.bat steps
- `test=True` dry-run habit before fleet-wide state application
- Rolling deploy with `--batch-size 1` formalized from ad-hoc canary convention

**Defer to v7+:**
- `salt-run manage.status` exposed in staff dashboard (medium complexity, requires salt-api or SSH from racecontrol)
- Compound targeting for A/B config tests across fleet halves
- Pillar per-pod config (useful only when state files are complex enough to need templating)

**Anti-features — do not build:**
- Salt beacons + reactors for pod monitoring: Salt latency is seconds, WebSocket monitoring is milliseconds — two competing systems with worse performance
- Salt schedule on Windows minions: confirmed bug (#19277), cron/when schedules silently fail on Windows
- Salt `cmd.run` to launch GUI applications: Session 0 isolation prevents any GUI interaction from Salt commands
- `state.highstate` on a schedule: risk of partial application interrupting live sessions

### Architecture Approach

The architecture places Salt as a parallel fleet operations channel alongside the existing WebSocket real-time channel. WebSocket handles all game state, billing, and lock screen events (sub-second requirement). Salt handles deploy, restart, health check, and diagnostic exec (batch operations, 200-500ms latency acceptable). The integration seam is `salt_exec.rs` — a new Rust module on the server that calls `salt-api` REST via `reqwest` (already in Cargo.toml). All four modules that currently call port 8090 (deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs) are updated to call `salt_exec` instead. Only one module is deleted from rc-agent: `remote_ops.rs`.

**Major components:**
1. `salt-master` + `salt-api` (WSL2 Ubuntu .27): Fleet command hub — receives commands from salt_exec.rs via HTTP REST, forwards via ZeroMQ to minions
2. `salt-minion` (Windows service, all 9 nodes): Outbound-only agent — connects to master:4505, receives commands, no inbound firewall rules needed on pods
3. `salt_exec.rs` (NEW, racecontrol): Salt REST API client — `cmd_run`, `cp_get_file`, `ping`, `ping_all`, `service_restart`; uses existing reqwest client; token stored in racecontrol.toml `[salt]` section
4. `deploy.rs` (MODIFY): Replace HTTP steps with `salt_exec` calls; preserve `do-swap.bat` self-swap pattern (Windows OS constraint: cannot replace running binary)
5. `fleet_health.rs` (MODIFY): Replace HTTP probes with `salt_exec.ping_all()`; rename `http_reachable` to `minion_reachable`
6. `pod_monitor.rs` (MODIFY): Replace `exec_on_pod_via_http()` with `salt_exec.service_restart()`; WatchdogState FSM unchanged
7. `pod_healer.rs` (MODIFY): Replace `exec_on_pod()` HTTP helper with `salt_exec.cmd_run()`; all diagnostic parse logic unchanged
8. `remote_ops.rs` (DELETE from rc-agent): Eliminated entirely after all callers migrated

**Key patterns:**
- Salt REST API via salt-api as integration seam (no subprocess, no WSL2 boundary issues, structured JSON response)
- Preserve `do-swap.bat` for binary self-update (Salt cannot atomically replace a running binary — Windows OS constraint)
- Static grain metadata only (`pod_number`, `venue`, `role`) — never real-time state (grains are cached, only refresh on minion restart)

### Critical Pitfalls

1. **WSL2 NAT makes Salt ports unreachable from LAN** — Default WSL2 gives a 172.x.x.x IP. Pods on 192.168.31.x cannot reach master. Enable mirrored networking (`networkingMode=mirrored` in `.wslconfig`) before deploying any minion. Verify with `Test-NetConnection 192.168.31.27 -Port 4505` from an actual pod.

2. **Hyper-V firewall silently blocks inbound to WSL2 even after mirrored mode is enabled** — A separate Hyper-V firewall layer (added in WSL 2.0.9+) has `DefaultInboundAction: Block`. Windows Defender firewall rules do not affect it. Fix: `Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow` in elevated PowerShell. Failure looks identical to the NAT pitfall.

3. **Salt minion service cannot restart itself on Windows** — `service.restart salt-minion` stops the service but never starts it again (confirmed bug #65577). Fix: configure Windows Service Recovery on each pod during install: `sc failure salt-minion reset= 60 actions= restart/5000/restart/10000/restart/30000`.

4. **Windows Defender quarantines salt-minion binaries after install** — Installer returns exit code 0 but Defender asynchronously quarantines Python and ZeroMQ binaries 5–15 seconds later. Fix: add exclusions for `C:\Program Files\Salt Project\Salt` and `C:\ProgramData\Salt Project\Salt` *before* running the installer. Verify with `sc query salt-minion` 30 seconds after install.

5. **`cp.get_file` silently succeeds without transferring the file** — Salt returns True even when the destination directory does not exist. Always precede with `file.makedirs` and follow with `file.file_exists` verification. Never accept the `cp.get_file` return value alone as proof of transfer.

6. **Minion ID auto-generated from Windows hostname** — Gaming pods have generic OEM hostnames; two pods imaged from the same base get duplicate IDs. Fix: always set `id: pod{N}` explicitly in minion config before first service start. Ship per-pod `salt-minion-pod{1-8}.conf` files in the deploy kit.

7. **Deleting `remote_ops.rs` without auditing AppState initialization** — Rust compiler catches type errors, not runtime initialization dependencies. A field only populated by `remote_ops.rs` compiles fine but panics at runtime. Fix: grep all `remote_ops` references, audit AppState fields, write characterization tests covering the WebSocket path (billing lifecycle, game launch, lock screen) before deletion, canary deploy to Pod 8 and run a full billing session before fleet rollout.

## Implications for Roadmap

The build order is dictated by a hard infrastructure prerequisite: the WSL2 Salt master must be verified from an actual pod before any Rust code is written. Server-side modules migrate one at a time using Pod 8 as the canary throughout. `remote_ops.rs` is deleted last, only after every caller is migrated.

### Phase 1: WSL2 Infrastructure and Master Setup

**Rationale:** Everything depends on the master being reachable from pods. This is pure infrastructure — no Rust code. Attempting any code change before WSL2 networking is verified is wasted effort. DHCP reservations for all 8 pods must also be set here, because DHCP drift kills ZeroMQ connections silently.
**Delivers:** WSL2 Ubuntu 24.04 with salt-master 3008 and salt-api running, mirrored networking active, Hyper-V firewall open, Windows Defender firewall rules for :4505/:4506/:8000, DHCP reservations for all 8 pod MACs in router.
**Addresses:** Pitfalls 1, 2, 3 (all WSL2 networking), Pitfall 11 (DHCP drift)
**Verification gate:** `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 returns `TcpTestSucceeded: True`
**Research flag: skip.** All configuration is explicitly documented in STACK.md and ARCHITECTURE.md with exact commands. No unknowns.

### Phase 2: Salt Minion Bootstrap on Pod 8 (Canary)

**Rationale:** Validate WSL2 networking with a real minion before touching any Rust code or deploying to the fleet. This phase also rewrites `install.bat` and configures Windows Service recovery — done once on Pod 8, then replicated across the fleet in Phase 5.
**Delivers:** Salt minion 3008 installed on Pod 8 with explicit `id: pod8`, Defender exclusions pre-applied, `sc failure` recovery configured, key accepted, `salt 'pod8' test.ping` returns True. Updated `install.bat` with salt-minion bootstrap replacing pod-agent section.
**Addresses:** Pitfalls 4 (Defender quarantine), 5 (minion self-restart), 6 (minion ID from hostname), 8 (install.bat strips rc-agent firewall rules)
**Verification gate:** `salt 'pod8' cmd.run 'whoami'` returns successfully; `sc qfailure salt-minion` shows restart actions.
**Research flag: skip.** All install commands, config paths, and Defender exclusion patterns are explicitly documented in STACK.md and PITFALLS.md.

### Phase 3: `salt_exec.rs` and Server-Side Module Migration

**Rationale:** `salt_exec.rs` is the foundation that all four server-side modules import. It must compile and be tested against live Pod 8 before any module rewrite. Modules migrate in safety order: fleet_health (read-only, lowest risk) → pod_healer (diagnostic only) → pod_monitor (restarts, higher risk) → deploy (most complex, multi-step with rollback).
**Delivers:** `salt_exec.rs` with `cmd_run`, `cp_get_file`, `ping`, `ping_all`, `service_restart`; `[salt]` section in racecontrol.toml and config.rs; `SaltClient` in AppState; rewrites of fleet_health.rs, pod_healer.rs, pod_monitor.rs, deploy.rs; `http_reachable` renamed to `minion_reachable`; `build_id` moved to StartupReport in rc-common protocol.
**Avoids:** Pitfall 9 (backslash paths in Salt states — forward slashes convention established before any state is written), Pitfall 10 (`cp.get_file` silent failure — `file.file_exists` verification added to deploy flow)
**Verification gate:** All four modified modules compile; `salt 'pod8' test.ping` via salt_exec.rs returns true; deploy end-to-end to Pod 8 succeeds with rollback tested.
**Research flag: needs attention for `cp.get_file` reliability.** ARCHITECTURE.md documents a known ZeroMQ bug with `cp.get_file` for cross-VLAN scenarios and recommends using the existing curl-from-HTTP-server pattern for binary transfer. Decision needed: whether to use `salt cp.get_file` or keep the HTTP server for the binary download step and use Salt only for the trigger command. Both approaches are documented — make the call before coding `deploy.rs`.

### Phase 4: Remove `remote_ops.rs` from rc-agent

**Rationale:** Only delete after every caller on the server side is migrated and verified. Deletion is a Rust compile gate that confirms no remaining references. Canary to Pod 8 before fleet rollout.
**Delivers:** `remote_ops.rs` deleted, `remote_ops::start(8090)` call removed from `main.rs`, :8090 firewall open removed from `firewall.rs`, `cargo build` clean, rc-agent deployed to Pod 8 with full billing lifecycle verified.
**Avoids:** Pitfall 7 (AppState initialization audit — grep all references, write characterization tests, canary deploy with billing lifecycle test before fleet rollout)
**Verification gate:** Full billing lifecycle (session start, game launch, billing tick, session end, lock screen) confirmed on Pod 8 with no panics before fleet rollout.
**Research flag: needs characterization tests first.** Per the "Refactor Second" standing rule: write characterization tests covering the WebSocket path (billing lifecycle, game launch, lock screen) before deleting remote_ops.rs. The compiler will not catch runtime initialization failures.

### Phase 5: Fleet Rollout

**Rationale:** Once Pod 8 is verified clean (no remote_ops.rs, Salt minion connected, billing lifecycle working), replicate to all 7 remaining pods and the server using the updated install.bat. Key acceptance for all nodes. Fleet-wide verification.
**Delivers:** Salt minion on all 8 pods + server, all keys accepted, `salt '*' test.ping` returns 9 True responses, fleet health dashboard shows all pods `minion_reachable: true`, port 8090 firewall rule removed from all pods.
**Avoids:** Pitfall 12 (duplicate minion IDs — per-pod config files already in deploy kit from Phase 2)
**Verification gate:** `salt 'pod*' test.ping` returns 8 True; `salt 'server' test.ping` True; fleet health API shows all 8 pods minion_reachable; no active billing sessions interrupted during rollout.
**Research flag: skip.** Standard Salt fleet deployment using verified install.bat from Phase 2.

### Phase Ordering Rationale

- Infrastructure before code: The WSL2 master networking is the single critical-path dependency. All Rust work is blocked until Phase 1 and 2 are verified from an actual pod.
- Server-side migration before agent deletion: Deleting `remote_ops.rs` while any server module still calls it produces a compile error; migrating callers first makes deletion a clean final step.
- Pod 8 canary discipline throughout: Every phase that deploys to pods deploys to Pod 8 first and verifies the billing lifecycle before touching the other 7 pods.
- Safety ordering for server modules: fleet_health (read-only) → pod_healer (diagnostic) → pod_monitor (restarts) → deploy (most complex) — highest-risk module last.

### Research Flags

Phases needing deeper investigation before coding begins:

- **Phase 3 — `cp.get_file` vs curl-for-binaries decision:** ARCHITECTURE.md documents a known ZeroMQ bug where `cp.get_file` silently fails in some cross-VLAN scenarios and recommends keeping the curl-from-HTTP-server pattern for binary files. Decide which approach to use for `deploy.rs` before writing the module. The choice affects whether the Python HTTP server stays or goes entirely.

- **Phase 4 — AppState initialization audit:** Before writing characterization tests, read `remote_ops.rs` in full and grep all `AppState` fields it writes at startup. Confirm each field has an alternative initializer. This audit determines the scope of characterization test work needed.

Phases with standard patterns (skip research-phase):

- **Phase 1:** All commands explicitly documented in STACK.md and ARCHITECTURE.md with verified sources. No unknowns.
- **Phase 2:** Install commands, Defender exclusion paths, `sc failure` syntax, grains config paths all documented in STACK.md and PITFALLS.md.
- **Phase 5:** Standard `install.bat` fleet deployment. Pod 8 is the validated template.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Official Salt docs, Broadcom KB, Microsoft WSL docs all verified. Version lifecycle (3007 EOL this month, 3008 LTS) is authoritative. WSL2 mirrored mode requirements are confirmed for James's Windows 11 build. |
| Features | MEDIUM-HIGH | Salt official docs + GitHub issues verify capabilities. Known Windows-specific bugs (#19277 scheduler, #65577 service restart, #4834 Session 0 isolation) are confirmed. WSL2-as-master is an unusual setup — no end-to-end guides exist but all individual components are documented. |
| Architecture | HIGH | Existing codebase read directly (remote_ops.rs, deploy.rs, fleet_health.rs, pod_monitor.rs, pod_healer.rs, state.rs, config.rs, main.rs). All caller sites and AppState fields inventoried from source. Salt/WSL2 networking verified against official docs. |
| Pitfalls | HIGH | 12 pitfalls documented with official source references. WSL2 NAT/Hyper-V pitfalls from Microsoft docs and community guides. Salt Windows bugs from confirmed GitHub issues (2014–2024). cp.get_file silent failure from Salt mailing list + GitHub. Path separator from Salt issue tracker. |

**Overall confidence: MEDIUM-HIGH**

### Gaps to Address

- **cp.get_file reliability for binary files:** ARCHITECTURE.md notes a ZeroMQ bug with cp.get_file in some network scenarios and recommends the existing curl-from-HTTP-server pattern. The exact scope of this bug on the venue LAN (same VLAN, not cross-VLAN) is unclear. Test cp.get_file with a test binary on Pod 8 in Phase 2 before committing to it in Phase 3 for deploy.rs.

- **Grains config path on Windows 3008 minions:** ARCHITECTURE.md flags known ambiguity between `C:\salt\grains` (legacy) and `C:\ProgramData\Salt Project\Salt\conf\grains` (newer path, issue #63024). Verify actual path on Pod 8 after minion install with `salt 'pod8' grains.get pod_number`. Do not hard-code paths in deploy kit until verified.

- **salt-api token rotation plan:** The salt-api token stored in racecontrol.toml is a long-lived credential. Phase 3 must include a note in racecontrol.toml that this token needs rotation when racecontrol is redeployed. No implementation gap, but the operational procedure needs to be documented before Phase 3 ships.

- **`remote_ops.rs` AppState field inventory:** This audit must happen before Phase 4 characterization test work begins. Unknown scope until `remote_ops.rs` is fully read and all `AppState` mutation sites are identified.

## Sources

### Primary (HIGH confidence)

- [Salt Windows Install Guide](https://docs.saltproject.io/salt/install-guide/en/latest/topics/install-by-operating-system/windows.html) — installer options, config paths, silent install params
- [Salt Version Support Lifecycle](https://docs.saltproject.io/salt/install-guide/en/latest/topics/salt-version-support-lifecycle.html) — LTS vs STS model, 3007 EOL 2026-03-31
- [Salt Firewall Guide](https://docs.saltproject.io/en/3007/topics/tutorials/firewall.html) — ports 4505/4506, minion-outbound-only model
- [Broadcom Port Requirements KB](https://knowledge.broadcom.com/external/article/403589/port-requirements-for-saltminionsaltmast.html) — confirms 4505/4506 TCP on master only
- [Microsoft WSL Networking](https://learn.microsoft.com/en-us/windows/wsl/networking) — mirrored mode, NAT limitations, Hyper-V firewall
- [salt.modules.win_service](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.win_service.html) — Windows service management
- [salt.modules.cp](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.cp.html) — file distribution
- [Salt Bootstrap Script](https://github.com/saltstack/salt-bootstrap) — Ubuntu 24.04 + 3008 stable install
- [rest_cherrypy docs](https://docs.saltproject.io/en/latest/ref/netapi/all/salt.netapi.rest_cherrypy.html) — salt-api HTTP REST
- [Broadcom Minion Config Location KB](https://knowledge.broadcom.com/external/article/379823/location-of-minion-config-files-on-windo.html) — confirmed config paths on Windows
- Direct source reads (2026-03-17): `remote_ops.rs`, `deploy.rs`, `fleet_health.rs`, `pod_monitor.rs`, `pod_healer.rs`, `state.rs`, `config.rs`, `main.rs` (rc-agent), `firewall.rs`, `.planning/PROJECT.md`

### Secondary (MEDIUM confidence)

- [GitHub #19277: Windows minion cron/when schedules fail silently](https://github.com/saltstack/salt/issues/19277) — confirmed Salt scheduler bug on Windows
- [GitHub #65577: salt-minion service restart stops but doesn't start](https://github.com/saltstack/salt/issues/65577) — confirmed 2024 Windows service restart bug
- [GitHub #4834: Session 0 isolation prevents GUI interaction](https://github.com/saltstack/salt/issues/4834) — confirmed architectural limitation
- [GitHub #16340: cmd.run runas not implemented on Windows](https://github.com/saltstack/salt/issues/16340) — confirmed
- [GitHub #63024: grains path ambiguity on Windows minions](https://github.com/saltstack/salt/issues/63024) — path issue, needs verification on Pod 8
- [WSL mirrored mode practical guide](https://informatecdigital.com/en/wsl2-advanced-guide-to-network-configuration-and-nat-and-mirrored-modes/) — Hyper-V firewall rule, .wslconfig setup
- [Salt cp.get_file ZMQ issue (salt-users mailing list)](https://groups.google.com/g/salt-users/c/rtjniGu1UPM) — cross-scenario failure; use file.managed or cmd.run curl instead
- [GitHub: WSL2 NIC mirrored mode multicast bug #10535](https://github.com/microsoft/WSL/issues/10535) — confirms unicast TCP (Salt ZMQ) unaffected

### Tertiary (context)

- [endoflife.date/salt](https://endoflife.date/salt) — lifecycle dates consistent with official docs
- MEMORY.md — pod MAC addresses, subnet 192.168.31.x, James's machine .27, DHCP drift history (server .51→.23→.4→.23)

---
*Research completed: 2026-03-17 IST*
*Ready for roadmap: yes*
