# Feature Research — Salt Fleet Management (v6.0)

**Domain:** SaltStack fleet management replacing custom HTTP pod-agent for 8-node Windows 11 gaming pod fleet
**Researched:** 2026-03-17
**Confidence:** MEDIUM-HIGH (Salt official docs + GitHub issues verify capabilities; WSL2-as-master is unusual setup with known networking quirks)

---

## Context: What Already Exists vs What Salt Replaces

Before mapping features, the existing stack must be clear. Salt is a replacement for specific subsystems only — not a greenfield installation.

### What Salt REPLACES (port 8090 custom HTTP endpoint)

| Existing Capability | Replacement |
|--------------------|-------------|
| `remote_ops.rs` HTTP `/exec` — run arbitrary shell commands on pods | `salt 'pod*' cmd.run '...'` |
| `remote_ops.rs` HTTP `/file` — read/write files on pods | `salt 'pod*' cp.get_file` + `file.managed` state |
| `deploy-rc-agent.py` HTTP server + curl pipeline — push binaries | `salt 'pod*' cp.get_file salt://rc-agent.exe` |
| `install.bat` firewall config, Defender exclusions, HKLM keys | Salt state files (`.sls`) |
| `pod-agent` references in codebase and deploy scripts | Removed entirely |

### What Salt DOES NOT REPLACE (must NOT be touched)

| Existing Capability | Reason to Keep |
|--------------------|----------------|
| WebSocket connection (rc-agent ↔ racecontrol, port 8080) | Real-time game state, telemetry, billing events — Salt cannot substitute sub-second event delivery |
| UDP heartbeat (port varies per pod) | Pod liveness at 6s timeout — Salt job return latency is seconds, not milliseconds |
| rc-agent lock screen, billing FSM, game launch | Application logic — Salt is infrastructure, not application |
| rc-agent firewall auto-config (`Rust netsh at startup`) | rc-agent already handles its own firewall on startup; duplication adds conflict risk |
| WS exec relay (staff run commands via kiosk) | This is a different path — staff in kiosk → racecontrol WS → rc-agent. Salt is James-only CLI |

---

## Feature Landscape

### Table Stakes (Required for the Migration to Work)

Features that must exist for Salt to replace the custom HTTP endpoint. Missing any of these = migration is incomplete, port 8090 cannot be removed.

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| Salt master on WSL2 (Ubuntu, James .27) | All Salt commands flow from here; without master the fleet is unmanaged | MEDIUM | WSL2 NAT networking means minions connect to a WSL2 IP, not .27 directly. Port forwarding via `netsh interface portproxy` required for ports 4505/4506. This is the #1 networking pitfall for this setup. |
| Salt minion on all 8 pods + server (.23) | Minions must be running and keys accepted before any fleet commands work | MEDIUM | Windows installer is an MSI. Silent install via `Salt-Minion-Setup.exe /S /master=<ip> /minion-name=pod1`. Key auto-acceptance via `auto_accept: True` on master config avoids manual `salt-key -A` per pod. |
| Minion ID naming convention (`pod1`–`pod8`, `server`) | Glob targeting `salt 'pod*' cmd.run` requires predictable names | LOW | Set `id: pod1` in `/etc/salt/minion` (or `C:\salt\conf\minion` on Windows). Set at install time, never changes. |
| Minion key acceptance | Without accepted keys, salt commands silently time out | LOW | Use `auto_accept: True` on master for trusted LAN. Pre-seed keys via `preseed_key` tutorial for zero-touch install alternatively. |
| `cmd.run` remote execution | Replaces the `/exec` endpoint — run arbitrary commands on pods | LOW | `salt 'pod1' cmd.run 'tasklist'` works. Windows-specific: commands run as SYSTEM in Session 0 — no GUI interaction possible. Keep this in mind for rc-agent restart (use `sc.exe start/stop`, not GUI launch). |
| `cp.get_file` file distribution | Replaces HTTP server + curl pipeline for binary deployment | LOW | `salt 'pod*' cp.get_file salt://rc-agent.exe dest='C:\\RacingPoint\\rc-agent.exe'` — pulls from master's file_roots. File server path configured in master config. |
| `service` module for Windows services | rc-agent runs as Windows Service since v4.0 — restart/stop via Salt must use service module, not cmd.run | LOW | `salt 'pod1' service.restart rc-agent` — Salt's `win_service` module maps cleanly to `sc.exe`. Confirmed to work for Windows Services. |
| Glob targeting (`pod*`, `pod1`, `*`) | Fleet-wide or per-pod operations | LOW | `salt 'pod*' cmd.run` — targets all minions matching glob. `salt 'pod1' cmd.run` — targets single pod. Standard Salt glob, no extra config needed. |
| Salt file server (`file_roots`) | Where master stores files (binaries, state files, configs) for minions to pull | LOW | Configure in `/etc/salt/master`: `file_roots: base: [/srv/salt]`. WSL2 path, accessible from master. Minions pull via `cp.get_file`. |

### Differentiators (Valuable But Not Blocking Migration)

Features that improve operations quality once the table stakes migration is complete.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Grains for pod identity/role | `salt -G 'role:pod' cmd.run` or `salt -G 'pod_number:3' cmd.run` lets you target by property, not just name | LOW | Set static grains in `C:\salt\conf\minion`: `grains: { role: pod, pod_number: 3, location: venue }`. Auto-populated from install.bat with pod number argument. |
| `state.apply` for configuration enforcement | Deploy a state that ensures rc-agent binary is current, Defender exclusions exist, HKLM Run key is set — all idempotent | HIGH | Replaces manual install.bat steps with repeatable Salt states. Write `.sls` files for: Defender exclusion (`cmd.run powershell -command "Add-MpPreference..."`), HKLM Run key (`reg.present`), binary file (`file.managed`). |
| Nodegroups in master config | Define `pods: 'pod*'` and `server: server` as aliases in `/etc/salt/master` — simpler CLI | LOW | `salt -N pods cmd.run 'sc query rc-agent'` — same as `salt 'pod*'` but readable. Add to master config: `nodegroups: { pods: 'pod*', server: 'server' }`. |
| Pillar for per-pod config data | Pod-specific config values (pod number, IP, toml path) stored in pillar, pulled by minion during state apply | MEDIUM | `pillar/top.sls` maps minion IDs to data files. Allows state files to be generic while per-pod values come from pillar. Example: `{{ pillar['pod_number'] }}` in state template. |
| `test=True` dry-run mode | Verify what a state would change before applying it — safe for prod fleet | LOW | `salt 'pod*' state.apply rc-agent test=True` — standard Salt feature, no extra config. High value for a live venue where wrong state = pods go down. |
| Rolling deploy with canary | Deploy to pod8 first, verify, then roll to remaining pods | MEDIUM | Not native Salt — implement as shell script: `salt pod8 state.apply rc-agent && sleep 30 && salt 'pod[1-7]' state.apply rc-agent`. Salt has `batch` option: `salt 'pod*' state.apply --batch-size 1` for one-at-a-time rolling. |
| `salt-run manage.status` | One command shows which minions are up/down | LOW | `salt-run manage.status` returns `up:` and `down:` lists. Replaces manual pod health checks. Runs on master in WSL2. |
| Compound targeting (`-C`) | Combine grain + glob: `salt -C 'G@role:pod and pod[1-4]'` — first four pods only | MEDIUM | Useful for split A/B deployments or targeted troubleshooting. Compound targeting uses `-C` flag. |

### Anti-Features (Do Not Build These)

Features that seem like natural extensions of Salt fleet management but would cause problems in this specific environment.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Salt beacons + reactors for pod monitoring | "Salt can monitor services and auto-restart them" — seems like it replaces the rc-agent watchdog | Salt beacons add background polling overhead on every pod. Beacon events have multi-second latency — too slow for game state. The rc-agent watchdog + racecontrol WS connection already do this with millisecond precision. Adding Salt reactors creates two competing monitoring systems. | Keep WebSocket-based monitoring in rc-agent. Salt is for imperative fleet ops only. |
| Salt schedule on Windows minions | "Use Salt scheduler for daily tasks on pods" | Confirmed bug (GitHub #19277): cron-based and `when`-based schedules fail on Windows minions. Only `seconds`-interval schedules work reliably. The failure is silent — schedule appears active but jobs don't run. | Use `win_task` module to create actual Windows Scheduled Tasks, or avoid scheduled automation on pods entirely. |
| Salt `cmd.run` to launch GUI applications on pods | "Use Salt to start games or the kiosk for testing" | Salt minion is a Windows Service running in Session 0. Session 0 is isolated from the user desktop (Session 1). GUI processes launched from Session 0 appear invisible — they run but no window displays. This is a confirmed Windows architectural constraint, not a Salt bug. | Continue using rc-agent's existing game launch path (which runs in Session 1 via HKLM Run key). Salt ops on pods are headless-only. |
| Salt `runas` for non-SYSTEM user execution on pods | "Run rc-agent commands as the pod user, not SYSTEM" | `cmd.run runas=username` on Windows requires also providing `password` in plaintext, and the user must be in Administrators. Password in Salt commands = credential exposure risk. Multiple open GitHub issues confirm runas is buggy on Windows (issues #16340, #47787, #61166). | Keep rc-agent as a Windows Service (SYSTEM account). Salt ops that need user context should be done at pod setup time, not runtime. |
| Salt mine for fleet-wide data collection | "Collect pod stats into Salt mine for a dashboard" | Salt mine requires scheduled `mine.update` on all minions. Adds polling load on all 8 pods. The racecontrol fleet health dashboard already collects real-time pod state via WebSocket — no duplicate needed. | Use existing racecontrol WebSocket dashboard for live pod state. Use `salt-run manage.status` for on-demand connectivity check only. |
| Full SaltStack Enterprise / SaltStack Config GUI | "A web UI for fleet management would be nice" | SaltStack enterprise GUI requires VMware licensing. Overkill for 8 pods. Adds a service to maintain. | James uses `salt` CLI from WSL2 terminal. Uday uses the kiosk/dashboard. No additional UI needed. |
| `state.highstate` on a schedule (auto-drift correction) | "Run highstate every hour to prevent config drift" | Highstate runs all states — if any state fails (e.g., network blip), it can partially apply and leave pods in a broken state. On a live venue, mid-session state application could interrupt rc-agent. | Apply states manually before opening hours. Use `test=True` to verify before apply. Never schedule states on a live pod fleet without a maintenance window. |

---

## Feature Dependencies

```
[WSL2 Salt Master]
    must-exist-before --> [Minion Key Acceptance]
    must-exist-before --> [Salt File Server]
    requires --> [WSL2 port forwarding :4505/:4506 -> master]

[Minion Key Acceptance]
    requires --> [Minion ID naming convention]
    must-complete-before --> [cmd.run works]
    must-complete-before --> [cp.get_file works]
    must-complete-before --> [service module works]

[Salt File Server]
    must-exist-before --> [cp.get_file binary distribution]
    must-exist-before --> [state.apply config enforcement]

[cmd.run remote execution]
    replaces --> [remote_ops.rs /exec endpoint]
    does-NOT-replace --> [WS exec relay in rc-agent] (different user path)
    CANNOT --> [launch GUI, interact with Session 1]

[cp.get_file binary distribution]
    replaces --> [HTTP server + curl pipeline in deploy-rc-agent.py]
    requires --> [Salt File Server configured]

[service module]
    replaces --> [cmd.run 'sc stop rc-agent'] approach
    requires --> [rc-agent running as Windows Service] (ALREADY DONE in v4.0)

[state.apply config enforcement]
    enhances --> [cp.get_file] (idempotent binary deploy)
    enhances --> [service module] (idempotent service state)
    requires --> [Pillar data] if per-pod values needed in states

[Grains for targeting]
    enhances --> [cmd.run] (target by role instead of name)
    enhances --> [state.apply] (conditional logic in state files)
    set-at --> [install.bat pod bootstrap time]

[Pillar per-pod config]
    required-by --> [Generic state files with per-pod values]
    requires --> [pillar/top.sls mapping minion_id to pillar file]

[Rolling deploy with canary]
    requires --> [cmd.run works] (pod8 verification step)
    requires --> [cp.get_file works] (binary distribution)
    uses --> [Salt --batch-size flag] (native, no extra config)
```

### Dependency Notes

- **WSL2 port forwarding is the critical path blocker.** Without it, minions on pod IPs (192.168.31.x) cannot reach the master. WSL2 uses NAT — the master's WSL2 IP is ephemeral (e.g., 172.x.x.x) and changes on reboot. Solution: configure Windows host (`netsh interface portproxy`) to forward 4505/4506 from the Windows IP (.27) to the WSL2 IP. This must survive reboots — put it in a Windows startup task.

- **Minion naming is set once at install time.** The `id:` in `C:\salt\conf\minion` does not change after key acceptance. If you rename a minion, you must revoke the old key on master and re-accept the new one. Get naming right in install.bat.

- **`state.apply` is optional for MVP.** The migration can ship with just `cmd.run` + `cp.get_file` replacing the HTTP endpoint. State files are a quality-of-life improvement for repeatable config, not a requirement to remove port 8090.

- **Session 0 isolation does not affect the deployment use case.** Binary distribution (`cp.get_file`) and service restart (`service.restart`) work fine in Session 0 because they don't need a visible window. Only GUI launch is blocked — and Salt is never used for game launch.

---

## MVP Definition

The MVP for v6.0 is specifically: remove `remote_ops.rs` and `pod-agent` references, slim `install.bat`, and prove all existing deploy/management operations work via Salt instead.

### Launch With (v6.0 MVP)

- [ ] **WSL2 Salt master running** — with port forwarding for 4505/4506 surviving reboots
- [ ] **Salt minion installed on all 8 pods + server** — via updated install.bat with silent MSI install
- [ ] **All minion keys accepted** — auto_accept or pre-seeded
- [ ] **`cmd.run` verified on each pod** — `salt 'pod*' cmd.run 'sc query rc-agent'` returns service status
- [ ] **`cp.get_file` binary deploy verified** — push a new rc-agent.exe to pod8 via Salt, verify size + service restart
- [ ] **`service.restart` verified** — `salt 'pod1' service.restart rc-agent` brings service back up
- [ ] **`remote_ops.rs` module removed** from rc-agent codebase
- [ ] **Pod-agent references removed** from all code, scripts, docs
- [ ] **install.bat slimmed** to: Defender exclusions + rc-agent binary + salt-minion MSI bootstrap (no more netsh firewall lines for port 8090, no pod-agent install)

### Add After Validation (v6.x)

- [ ] **Grains for pod identity** — set in install.bat, enables role-based targeting
- [ ] **Nodegroup aliases in master config** — `pods`, `server` shortcuts for CLI convenience
- [ ] **`state.apply` for rc-agent state** — idempotent binary + service state file replacing manual deploy steps
- [ ] **Pillar per-pod config** — pod number, toml path from pillar for generic state files
- [ ] **Rolling deploy with `--batch-size 1`** — formalize canary pod8 → remaining pods

### Future Consideration (v7+)

- [ ] **`salt-run manage.status` in staff dashboard** — expose fleet connectivity via a scheduled Salt command from the kiosk backend (medium complexity, requires exposing Salt API or SSH into WSL2 from racecontrol)
- [ ] **Compound targeting for A/B config tests** — split fleet into two groups for testing config changes

---

## Feature Prioritization Matrix

| Feature | Ops Value | Implementation Cost | Priority |
|---------|-----------|---------------------|----------|
| WSL2 master + port forwarding | HIGH (blocks everything) | MEDIUM | P1 |
| Salt minion on all pods + key acceptance | HIGH (blocks everything) | MEDIUM | P1 |
| Minion ID naming (`pod1`–`pod8`) | HIGH (targeting depends on it) | LOW | P1 |
| `cmd.run` remote execution | HIGH (replaces /exec) | LOW | P1 |
| `cp.get_file` binary distribution | HIGH (replaces curl pipeline) | LOW | P1 |
| `service.restart/stop` | HIGH (replaces sc.exe workarounds) | LOW | P1 |
| Remove `remote_ops.rs` | HIGH (goal of milestone) | LOW | P1 |
| Slim install.bat | MEDIUM (cleanup) | LOW | P1 |
| Grains for pod identity | MEDIUM | LOW | P2 |
| Nodegroup aliases | LOW (ergonomics only) | LOW | P2 |
| `state.apply` rc-agent state | MEDIUM (idempotent deploys) | HIGH | P2 |
| `test=True` dry-run habit | MEDIUM (safety) | LOW (practice, not code) | P2 |
| Rolling deploy `--batch-size 1` | MEDIUM | LOW | P2 |
| Pillar per-pod config | LOW (only useful with state files) | MEDIUM | P3 |
| `salt-run manage.status` in dashboard | LOW (existing dashboard covers this) | HIGH | P3 |

**Priority key:** P1 = must have for v6.0 to ship, P2 = add once migration verified, P3 = future

---

## What the Migration Does NOT Touch

These existing capabilities are explicitly out of scope and must remain untouched:

| Capability | Location | Why Salt Must Not Touch It |
|-----------|----------|---------------------------|
| WebSocket game state (billing, telemetry, lock screen) | rc-agent ↔ racecontrol | Sub-second latency requirement — Salt job latency is 1–5 seconds |
| UDP heartbeat | rc-agent port varies | 6s liveness timeout — Salt polling cannot match this |
| rc-agent Windows Service startup (HKLM Run key) | All 8 pods + server | Already correct; changing service startup mechanism risks Session 0 isolation bug |
| Firewall auto-config in Rust | rc-agent startup | rc-agent opens its own ports at launch; Salt firewall states would conflict or duplicate |
| Cloud sync (racecontrol → app.racingpoint.cloud) | `cloud_sync.rs` | Network layer, not pod fleet ops |
| Billing FSM, game launch, lock screen logic | Application code | Salt is infrastructure; these are application behaviors |

---

## Sources

- [SaltStack official: Spinning up Windows Minions](https://docs.saltproject.io/en/3006/topics/cloud/windows.html) — MEDIUM confidence
- [SaltStack official: Targeting Minions](https://docs.saltproject.io/en/latest/topics/targeting/index.html) — HIGH confidence
- [SaltStack official: Node groups](https://docs.saltproject.io/en/latest/topics/targeting/nodegroups.html) — HIGH confidence
- [SaltStack official: Salt File Server](https://docs.saltproject.io/en/latest/ref/file_server/index.html) — HIGH confidence
- [SaltStack official: Scheduler](https://docs.saltproject.io/salt/user-guide/en/latest/topics/scheduler.html) — HIGH confidence
- [GitHub #19277: Scheduling via Pillar on Windows minions fails for cron/when schedules](https://github.com/saltstack/salt/issues/19277) — HIGH confidence (confirmed bug)
- [GitHub #4834: salt-minion cannot interact with desktop on Windows (Session 0 isolation)](https://github.com/saltstack/salt/issues/4834) — HIGH confidence (confirmed Windows architectural limitation)
- [GitHub #16340: cmd.run runas not implemented on Windows](https://github.com/saltstack/salt/issues/16340) — HIGH confidence
- [SaltStack official: win_service module](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.win_service.html) — HIGH confidence
- [SaltStack official: Pillar Walkthrough](https://docs.saltproject.io/en/latest/topics/tutorials/pillar.html) — HIGH confidence
- [SaltStack official: Targeting using Grains](https://docs.saltproject.io/en/latest/topics/targeting/grains.html) — HIGH confidence
- PROJECT.md v6.0 requirements (2026-03-17)
- MEMORY.md: venue context (8 pods, Windows 11, Session 1 requirement, v4.0 Windows Service migration already done)

---
*Feature research for: Salt Fleet Management (v6.0), replacing pod-agent/remote_ops.rs with SaltStack*
*Researched: 2026-03-17*
