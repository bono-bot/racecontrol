# Stack Research

**Domain:** SaltStack fleet management — all-Windows LAN pods, WSL2 Ubuntu master
**Researched:** 2026-03-17 IST
**Confidence:** MEDIUM-HIGH (official Salt docs + Microsoft WSL docs verified; WSL2+Salt combo has limited end-to-end guides but all individual components are well-documented)

---

> **Milestone scope:** This file covers v6.0 Salt Fleet Management ONLY — replacing
> pod-agent/remote_ops HTTP endpoint with SaltStack. Prior stack additions (Rust/Axum,
> windows-service, winreg, winapi, hidapi) remain in place and are not repeated here.
> Focus: salt-master on WSL2, salt-minion on Windows pods, deploy workflow migration.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| SaltStack salt-master | **3008 LTS** | Fleet orchestration master running in WSL2 Ubuntu | 3008 is the current active LTS (released April 2025, 2-year support to ~2027). 3007 STS hit EOL 2026-03-31 — this month. 3006 LTS is legacy. 3008 is the only correct choice for a new deployment. |
| Ubuntu 24.04 LTS (WSL2) | 24.04 | Host OS for salt-master on James's machine (.27) | Officially supported by Salt bootstrap script and Broadcom's Salt apt repo. Already a standard WSL2 distro. No separate VM needed. |
| Salt Minion Windows EXE | **3008.x-Py3-AMD64** | Agent on each pod (pod1–pod8) + server (.23) | EXE installer (not MSI) is the current standard for Windows. Bundles its own Python via relenv — no system Python install on pods. Silent install flags match required master/minion-name config. |
| WSL2 mirrored networking | Windows 11 22H2+ feature | Expose salt-master ports 4505/4506 directly on LAN IP 192.168.31.27 | Default WSL2 NAT mode gives WSL2 a private 172.x.x.x IP — pods on 192.168.31.x cannot reach it without fragile port-proxy rules. Mirrored mode mirrors Windows host network interfaces into WSL2, so the master binds to 192.168.31.27 and pods connect directly. |

### Supporting Salt Modules (bundled — no extra install)

| Module | Purpose | Command Example |
|--------|---------|----------------|
| `salt.modules.cmd` | Run arbitrary commands on Windows minions | `salt 'pod*' cmd.run 'tasklist' shell=powershell` |
| `salt.modules.win_service` | Manage Windows services (start/stop/restart/status/enable) | `salt 'pod*' service.restart rc-agent` |
| `salt.modules.cp` | File distribution from master fileserver to minions | `salt 'pod*' cp.get_file salt://rc-agent.exe 'C:\RacingPoint\rc-agent.exe' makedirs=True` |
| `salt.states.service` | Idempotent service state enforcement via state.apply | `service.running: rc-agent, enable: True` in a .sls file |
| `salt.grains` | Per-minion metadata for targeting by pod number | Set `pod_id: 3` grain in minion config; target with `salt -G 'pod_id:3' cmd.run ...` |

### Development / Operations Tools (no install — part of salt package)

| Tool | Purpose | Notes |
|------|---------|-------|
| `salt-key` | Accept/reject/list minion authentication keys on master | `salt-key -L` list; `salt-key -a pod1` accept one; `salt-key -A` accept all (initial setup only) |
| `salt-call` | Run Salt functions locally on a minion for debugging | `salt-call --local test.ping` — useful on pod without needing master connectivity |
| `salt` | Master CLI for targeting and orchestration | `salt '*' test.ping` — basic fleet connectivity check |
| `salt-master` (systemd service) | The master daemon in WSL2 | Configured to auto-start via `/etc/wsl.conf` `[boot]` section |

---

## Installation

### Step 1: WSL2 Mirrored Networking (Windows host, run once)

```powershell
# 1a. Enable mirrored networking — edit %UserProfile%\.wslconfig
# Add these lines:
# [wsl2]
# networkingMode=mirrored

# 1b. Restart WSL2 to apply
wsl --shutdown

# 1c. Allow inbound through Hyper-V firewall layer (mirrored mode adds this layer)
# Run as Administrator in PowerShell:
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow

# 1d. Open ports 4505/4506 on Windows Defender Firewall (for traffic hitting .27 from pods)
New-NetFirewallRule -DisplayName "Salt Master PUB 4505" -Direction Inbound -Protocol TCP -LocalPort 4505 -Action Allow
New-NetFirewallRule -DisplayName "Salt Master REQ 4506" -Direction Inbound -Protocol TCP -LocalPort 4506 -Action Allow
```

### Step 2: Install salt-master in WSL2 Ubuntu 24.04

```bash
# Inside WSL2 Ubuntu shell:

# 2a. Download and run bootstrap script, install master only (-M), no minion (-N), use pip if needed (-P)
curl -fsSL https://github.com/saltstack/salt-bootstrap/releases/latest/download/bootstrap-salt.sh -o bootstrap-salt.sh
sudo sh bootstrap-salt.sh -M -N -P stable 3008

# 2b. Configure /etc/salt/master (minimum required settings)
# interface: 0.0.0.0        # bind all interfaces (picks up 192.168.31.27 in mirrored mode)
# auto_accept: False        # NEVER true in production — use salt-key to accept manually
# file_roots:
#   base:
#     - /srv/salt           # place rc-agent.exe and state files here
# pillar_roots:
#   base:
#     - /srv/pillar

# 2c. Auto-start salt-master when WSL2 boots
# Add to /etc/wsl.conf:
# [boot]
# command = service salt-master start

sudo systemctl enable salt-master
sudo systemctl start salt-master
```

### Step 3: Install salt-minion on Windows pods (silent, via install.bat)

```bat
:: Download installer from Broadcom artifactory (update version number as needed):
:: https://packages.broadcom.com/artifactory/saltproject-generic/windows/3008.x/Salt-Minion-3008.x-Py3-AMD64.exe

:: Silent install — specify master IP and minion ID
:: Run as Administrator (SYSTEM account via install.bat works)
Salt-Minion-3008.x-Py3-AMD64.exe /S /master=192.168.31.27 /minion-name=pod1

:: Config written to: C:\ProgramData\Salt Project\Salt\conf\minion
:: Service "salt-minion" installed as Windows service, Automatic start (Delayed)
:: Minion makes outbound connections ONLY — no inbound firewall rules needed on pods
```

**Minion naming convention:** `pod1` through `pod8` for gaming PCs, `server` for .23. This enables `salt 'pod*'` glob to target all pods without the server.

### Step 4: Accept minion keys and verify

```bash
# On master (WSL2 shell):
salt-key -L              # list pending unaccepted keys
salt-key -a pod1         # accept pod1's key
salt-key -a pod2         # ... repeat for each pod and server
# OR during initial setup only:
salt-key -A              # accept all pending (then disable auto_accept in config)

# Connectivity test
salt '*' test.ping
# Expected: pod1: True, pod2: True, ... pod8: True, server: True
```

---

## Network Ports — Complete Picture

| Port | Protocol | Direction | Who Opens It | Purpose |
|------|----------|-----------|-------------|---------|
| 4505 | TCP | Minion → Master | Master only (Windows Defender on .27 + Hyper-V firewall) | Salt publish socket (ZeroMQ PUB). Minions subscribe to receive commands. |
| 4506 | TCP | Minion → Master | Master only | Salt request/reply socket (ZeroMQ REQ). Minion returns results and fetches files. |

**Key design property:** Minions are outbound-only. They initiate connections to master:4505. No inbound rules needed on pods. Windows Defender on pods blocks unsolicited inbound — correct, leave it.

**No changes needed to existing pod firewall rules.** Port 8090 (remote_ops) will be removed from pod firewall rules as part of the remote_ops.rs deletion.

---

## WSL2 Networking — Critical Decision

| Mode | WSL2 IP | Pod Connectivity to Master | Reliability |
|------|---------|---------------------------|-------------|
| NAT (default) | 172.x.x.x | Requires `netsh interface portproxy add` forwarding on .27. Breaks after WSL2 restart. Must be scripted into startup. | Fragile |
| **Mirrored (use this)** | Shares 192.168.31.27 | Pods connect to 192.168.31.27:4505 directly. Works like bare-metal. | Stable |

**Mirrored mode requirement:** Windows 11 22H2 (build 22621+). James's machine is Windows 11 Pro — confirmed compatible.

**Additional Hyper-V firewall rule** is required when mirrored mode is enabled. Mirrored mode introduces a Hyper-V firewall layer that blocks inbound by default:
```powershell
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow
```
The GUID `{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}` is the WSL2 Hyper-V VM GUID — constant across Windows 11 installations.

---

## Windows Service Management via Salt

The `win_service` module is loaded automatically on Windows minions (replaces the Linux `service` module transparently). The state DSL is identical across platforms.

```bash
# Ad-hoc service operations from master:
salt 'pod*' service.status rc-agent       # check status on all pods
salt 'pod*' service.restart rc-agent      # restart rc-agent on all pods
salt 'pod*' service.start rc-agent        # start if stopped
salt 'pod*' service.stop rc-agent         # stop
salt 'pod8' service.status rc-agent       # canary — check pod 8 only first
```

State file for idempotent enforcement (`/srv/salt/rc_agent.sls`):
```yaml
rc-agent:
  service.running:
    - enable: True
    - name: rc-agent
```

Apply: `salt 'pod*' state.apply rc_agent`

**Known issue with salt-minion service itself:** `service.restart salt-minion` on Windows can stop the service without restarting it due to a known bug. Workaround for the salt-minion service specifically:
```bash
salt 'pod1' cmd.run 'net stop salt-minion && net start salt-minion' shell=cmd
```
This does not affect `rc-agent` — the `service.restart rc-agent` is safe.

---

## File Deployment via Salt (replaces HTTP server + curl pipeline)

**Current workflow (being replaced):**
1. Copy binary to `deploy-staging/`
2. Start Python HTTP server on :9998
3. `curl` from pod-agent to download
4. `cmd.run` to kill old + start new

**New workflow via Salt:**
```bash
# 1. Copy new binary to salt fileserver root on master
cp /mnt/c/Users/bono/racingpoint/racecontrol/target/x86_64-pc-windows-msvc/release/rc-agent.exe /srv/salt/rc-agent.exe

# 2. Canary deploy to pod 8 first (Pod 8 canary convention from PROJECT.md)
salt 'pod8' service.stop rc-agent
salt 'pod8' cp.get_file salt://rc-agent.exe 'C:\RacingPoint\rc-agent.exe' makedirs=True
salt 'pod8' service.start rc-agent
salt 'pod8' service.status rc-agent

# 3. Roll to all pods after verification
salt 'pod*' service.stop rc-agent
salt 'pod*' cp.get_file salt://rc-agent.exe 'C:\RacingPoint\rc-agent.exe' makedirs=True
salt 'pod*' service.start rc-agent
```

**Windows path note in cp.get_file:** Use single quotes around the Windows path. Forward slashes work in the destination path. The known `minionfs` bug with Windows drive letter colons does NOT affect `cp.get_file` from master fileserver — only affects `cp.push` from minion. Using `salt://` paths to push from master is safe.

---

## Targeting Patterns

| Target | Salt Command | Use Case |
|--------|-------------|----------|
| All pods | `salt 'pod*' cmd.run ...` | Fleet-wide operations |
| Single pod | `salt 'pod3' cmd.run ...` | Targeted debug |
| Server only | `salt 'server' cmd.run ...` | racecontrol service management |
| All managed nodes | `salt '*' test.ping` | Connectivity check |
| By grain | `salt -G 'pod_id:3' cmd.run ...` | Grain-based targeting |
| Compound | `salt -C 'pod* and G@os:Windows' cmd.run ...` | Compound match (rarely needed) |

---

## Minion Configuration Details

Config directory on Windows (3008): `C:\ProgramData\Salt Project\Salt\conf\`

- Main config: `C:\ProgramData\Salt Project\Salt\conf\minion`
- Drop-in config dir: `C:\ProgramData\Salt Project\Salt\conf\minion.d\`
- Cache dir: `C:\ProgramData\Salt Project\Salt\var\cache\salt\minion\`

Minimum minion config (set by installer `/master=` and `/minion-name=` flags):
```yaml
master: 192.168.31.27
id: pod1
```

Custom grain for pod targeting (add to `minion.d\grains.conf`):
```yaml
grains:
  pod_id: 1
  location: racing_point_venue
```

---

## Tailscale Interaction

**Recommendation: Use LAN IP for salt-master address, not Tailscale IP.**

Pods are on 192.168.31.x, James's machine is .27 — same L2 segment. Routing Salt through Tailscale (100.x.x.x) adds:
- Latency through VPN tunnel for commands that execute in milliseconds on LAN
- Dependency on Tailscale being up for fleet management
- No security benefit (same trusted LAN)

Configure minions with `master: 192.168.31.27`. Tailscale continues to serve its existing purpose (cloud sync, remote access) — Salt operates only on LAN.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| SaltStack 3008 | Ansible | Ansible is agentless — uses WinRM or SSH to reach Windows hosts. WinRM from .27 is blocked on this LAN (MEMORY.md §Remote access). Salt's persistent ZeroMQ agent avoids this entirely. |
| SaltStack | Puppet | Puppet agent requires JVM on the master, complex PKI/cert infrastructure, and a PuppetDB. Salt's Python+ZeroMQ stack is far leaner. Puppet is built for large enterprise — overkill for 9 nodes. |
| SaltStack | Custom HTTP exec (existing pod-agent) | This IS what we're replacing. One-off HTTP exec has no file distribution, no state enforcement, no key management, and requires a running HTTP server on .27 at deploy time. |
| WSL2 mirrored mode | NAT mode + netsh portproxy | portproxy rules disappear on WSL2 restart and require recreation via startup script. Mirrored mode is persistent and works natively. |
| WSL2 Ubuntu | Separate Hyper-V VM | WSL2 is already present on James's machine. A separate VM adds IP management complexity, more RAM overhead, and requires its own network interface configuration. |
| Salt EXE installer | Chocolatey install | Chocolatey requires internet at install time and adds a package manager layer. Direct EXE from Broadcom artifactory is self-contained and works on the LAN pendrive. |
| LAN IP (192.168.31.27) for master | Tailscale IP (100.x.x.x) | All pods are same LAN — no need for VPN routing. Adds latency and Tailscale dependency with no benefit. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Salt 3007 | STS release, EOL 2026-03-31 (this month). Starting a new deployment on 3007 means immediate EOL. | Salt 3008 LTS |
| Salt 3006 LTS | No active bug fixes, security patches only until ~2026. New deployments should start on 3008. | Salt 3008 LTS |
| `auto_accept: True` on master | Any machine on the LAN that knows the master IP can register as a minion. During initial setup use `salt-key -A` once, then leave `auto_accept: False`. | Manual `salt-key -a <name>` per minion |
| `service.restart salt-minion` from master | Known Windows bug — stops the salt-minion service but may not restart it, leaving the pod unreachable. | `cmd.run 'net stop salt-minion && net start salt-minion' shell=cmd` |
| Running salt-master on Windows natively | Salt master is not supported on Windows. This is not a workaround situation — it is not a supported configuration. | WSL2 Ubuntu only |
| MSI installer format | The modern Salt Windows installer is EXE (NSIS-based). MSI format (`Salt-Minion-3006.23-Py3-AMD64.msi`) was used in older 3006 builds but current 3007/3008 use EXE. | EXE installer with `/S` flags |

---

## Version Compatibility

| Component | Required Version | Compatible With | Notes |
|-----------|-----------------|-----------------|-------|
| salt-master | 3008 LTS | salt-minion 3008, 3006 (backward-compatible) | Master can manage older minions. Keep master and minions on same major for simplicity. |
| salt-minion Windows | 3008.x-Py3-AMD64 | Windows 10, Windows 11, Server 2019/2022 | Pods are Windows 11 — fully supported. Python bundled (relenv), no system Python needed. |
| Ubuntu (WSL2) | 24.04 LTS | salt-master 3008 bootstrap | Salt bootstrap explicitly handles 24.04 repo setup. |
| WSL2 mirrored networking | Windows 11 22H2+ (build 22621+) | James's machine confirmed Windows 11 Pro | Verify build: `winver` in Run dialog |
| Python (bundled) | 3.10 (bundled by relenv in 3008) | No host Python required | Fully self-contained in the EXE. No pip, no virtualenv needed on pods. |

---

## Sources

- [Salt Windows Install Guide](https://docs.saltproject.io/salt/install-guide/en/latest/topics/install-by-operating-system/windows.html) — installer options, config paths, silent install params — HIGH confidence (official Broadcom/Salt docs)
- [Salt Version Support Lifecycle](https://docs.saltproject.io/salt/install-guide/en/latest/topics/salt-version-support-lifecycle.html) — LTS vs STS model, 3007 EOL 2026-03-31, 3008 LTS dates — HIGH confidence (official docs)
- [Salt Firewall Guide](https://docs.saltproject.io/en/3007/topics/tutorials/firewall.html) — ports 4505/4506, minion-outbound-only model — HIGH confidence (official docs)
- [Broadcom Port Requirements KB](https://knowledge.broadcom.com/external/article/403589/port-requirements-for-saltminionsaltmast.html) — confirms 4505/4506 TCP on master only — HIGH confidence (official vendor KB)
- [Microsoft WSL Networking](https://learn.microsoft.com/en-us/windows/wsl/networking) — mirrored mode documentation, NAT mode limitations — HIGH confidence (official Microsoft docs)
- [WSL2 Mirrored Networking configuration guide](https://informatecdigital.com/en/wsl2-advanced-guide-to-network-configuration-and-nat-and-mirrored-modes/) — .wslconfig setup, Hyper-V firewall PowerShell rule — MEDIUM confidence (community, aligns with Microsoft docs)
- [salt.modules.win_service](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.win_service.html) — Windows service management API surface — HIGH confidence (official docs)
- [salt.modules.cp](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.cp.html) — cp.get_file, makedirs, fileserver paths — HIGH confidence (official docs)
- [Salt Bootstrap Script](https://github.com/saltstack/salt-bootstrap) — Ubuntu 24.04 + 3008 stable install flags — HIGH confidence (official Salt Project repo)
- [Broadcom Minion Config Location KB](https://knowledge.broadcom.com/external/article/379823/location-of-minion-config-files-on-windo.html) — `C:\ProgramData\Salt Project\Salt\conf` confirmed — HIGH confidence (official vendor KB)
- [endoflife.date/salt](https://endoflife.date/salt) — 3007 EOL 2026-03-31, 3008 LTS lifecycle dates — MEDIUM confidence (community tracker, consistent with official lifecycle page)
- [Chocolatey Salt Minion 3007.12.0](https://community.chocolatey.org/packages/saltminion) — version confirmation for latest 3007 patch release — MEDIUM confidence (community package)

---

*Stack research for: v6.0 Salt Fleet Management — WSL2 Ubuntu master, Windows 11 pod minions*
*Researched: 2026-03-17 IST*
