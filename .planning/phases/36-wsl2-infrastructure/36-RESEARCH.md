# Phase 36: WSL2 Infrastructure - Research

**Researched:** 2026-03-17 IST
**Domain:** WSL2 Ubuntu 24.04, SaltStack 3008 LTS, salt-api (rest_cherrypy), Windows Defender + Hyper-V firewall, Windows Task Scheduler
**Confidence:** HIGH (all findings sourced from official Microsoft and Salt/Broadcom documentation; WSL2+Salt specific setup verified against community guides aligned with official docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Task Scheduler trigger at login (bono user), not at startup (SYSTEM)
- Salt is down between reboot and login — acceptable since James auto-logs in
- A startup script in WSL2 starts salt-master + salt-api: `wsl -e bash -c "sudo service salt-master start && sudo service salt-api start"`
- WSL2 RAM limit: 4 GB via `.wslconfig` `[wsl2] memory=4GB`
- WSL2 CPU: no limit (Salt is bursty, idle 99% of the time)

### Claude's Discretion
- salt-master/salt-api systemd restart policy — recommend `Restart=always`
- Hyper-V firewall: targeted ports vs blanket — recommended: targeted (4505, 4506, 8000)
- Defender firewall: LAN-scoped vs any-source — recommended: LAN-scoped (192.168.31.0/24)
- Salt-api auth mechanism (PAM vs sharedsecret) — recommend PAM for simplicity
- WSL2 swap allocation
- Exact Task Scheduler XML configuration

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFRA-01 | WSL2 Ubuntu 24.04 with mirrored networking mode configured on James (.27), reachable from pods at 192.168.31.27 | `.wslconfig` networkingMode=mirrored + `wsl --shutdown` restart; Hyper-V firewall GUID rule; verify with `Test-NetConnection` from Pod 8 |
| INFRA-02 | salt-master 3008 LTS installed in WSL2, listening on TCP 4505/4506 | Bootstrap script install flags; `/etc/salt/master` config; `systemctl enable salt-master`; `ss -tlnp` verify |
| INFRA-03 | Both firewall layers opened — Windows Defender + Hyper-V firewall for inbound 4505/4506 on James's machine | Two separate firewall commands: `Set-NetFirewallHyperVVMSetting` (Hyper-V layer) + `New-NetFirewallRule` (Defender layer); LAN-scoped Defender rules |
| INFRA-04 | salt-api (rest_cherrypy) running in WSL2 with token auth, accessible from racecontrol server (.23) | `/etc/salt/master` rest_cherrypy config section; PAM or sharedsecret eauth; `curl localhost:8000/login` verify |
| INFRA-05 | WSL2 + salt-master + salt-api auto-start on James's machine boot via Windows Task Scheduler | Task Scheduler XML at-logon trigger; `wsl -e bash -c "..."` action; bono user context |
</phase_requirements>

---

## Summary

Phase 36 is pure infrastructure — no Rust code. The goal is a reachable Salt master on James's machine (.27) that pods on 192.168.31.x can connect to. The single hardest technical problem is networking: WSL2 uses NAT by default, giving the Ubuntu instance a 172.x.x.x IP invisible to the pod subnet. Mirrored networking mode (Windows 11 22H2+, available on James's machine) solves this permanently — WSL2 inherits the Windows LAN IP (192.168.31.27), so pods connect to `master: 192.168.31.27` with no portproxy, no IP drift, no startup scripts.

Even after mirrored mode is enabled, a second independent firewall layer (Hyper-V Firewall, added in WSL 2.0.9+) blocks inbound connections by default. This layer is separate from Windows Defender Firewall and requires its own `Set-NetFirewallHyperVVMSetting` command. The failure mode for both problems looks identical (pods cannot reach port 4505), which is why the Hyper-V rule must be applied as part of the same setup sequence, not as an afterthought.

Salt-api (rest_cherrypy) runs alongside salt-master in WSL2 and exposes a REST interface on port 8000 that the racecontrol server (.23) will use in Phase 38 to drive fleet operations. Auto-start via Windows Task Scheduler at login (not SYSTEM startup) is the correct approach for WSL2 since WSL2 requires an active user session — James's auto-login means salt is available within seconds of boot.

**Primary recommendation:** Follow the exact command sequence documented in STACK.md and ARCHITECTURE.md. All commands are verified. Run `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 (not from .27) before declaring success — this is the only true connectivity test.

---

## Standard Stack

### Core

| Technology | Version | Purpose | Why Standard |
|------------|---------|---------|--------------|
| Ubuntu 24.04 LTS (WSL2) | 24.04 | Host OS for salt-master on James's machine | Officially supported by Salt bootstrap script and Broadcom apt repo; standard WSL2 distro; no separate VM needed |
| SaltStack salt-master | **3008 LTS** | Fleet orchestration master | Only current LTS (released April 2025, support to ~2027); 3007 STS EOL 2026-03-31 this month; 3006 is legacy — 3008 is the only correct choice |
| salt-api (rest_cherrypy) | bundled with 3008 | REST interface for racecontrol-to-Salt integration | Included in salt package; enables racecontrol to issue Salt commands via HTTP without subprocess or WSL2 boundary crossing |
| WSL2 mirrored networking | Windows 11 22H2+ | Expose salt-master ports on LAN IP 192.168.31.27 | NAT mode (default) gives WSL2 a 172.x.x.x IP unreachable from pod subnet; mirrored mode shares host NIC permanently |

### Supporting Tools (bundled in Salt package)

| Tool | Purpose | Notes |
|------|---------|-------|
| `salt-key` | Accept/reject minion authentication keys | `salt-key -a pod8` accept one key; `salt-key -A` accept all (initial setup only) |
| `salt-call` | Run Salt functions locally for debugging | `salt-call --local test.ping` — does not require master connectivity |
| `salt` CLI | Master CLI for fleet commands | Used to verify connectivity: `salt 'pod8' test.ping` |
| `ss` / `netstat` | Verify master is listening on correct ports | `ss -tlnp | grep 45` inside WSL to confirm 4505/4506 bound |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| WSL2 mirrored mode | NAT + netsh portproxy | portproxy rules use a static WSL NAT IP that changes on every WSL restart; breaks overnight; unacceptable for a venue that opens at 09:00 |
| WSL2 Ubuntu | Separate Hyper-V VM | Adds IP management complexity, more RAM overhead, its own network interface setup — WSL2 is already present |
| salt-api rest_cherrypy | `salt` CLI subprocess from racecontrol | racecontrol runs on Windows (.23); salt CLI lives in WSL2 Ubuntu on .27 — subprocess would require WSL2 boundary crossing; structured JSON from salt-api is far cleaner |
| PAM eauth | sharedsecret auth | Both are valid for LAN-only; PAM is simpler (no extra config file, uses system users); sharedsecret is equally simple but requires managing the secret separately |
| Task Scheduler at-login | HKLM Run key | HKLM Run launches before WSL2 session is ready; Task Scheduler at-login fires after user session is established, ensuring WSL2 is available |

**Installation (inside WSL2 Ubuntu shell):**
```bash
# Step 1: Download and run bootstrap script
curl -fsSL https://github.com/saltstack/salt-bootstrap/releases/latest/download/bootstrap-salt.sh -o bootstrap-salt.sh
sudo sh bootstrap-salt.sh -M -N -P stable 3008
# -M = install master, -N = no minion, -P = allow pip if needed

# Step 2: Configure /etc/salt/master
sudo tee -a /etc/salt/master << 'SALTCONF'
interface: 0.0.0.0
auto_accept: False
file_roots:
  base:
    - /srv/salt
pillar_roots:
  base:
    - /srv/pillar

# salt-api (rest_cherrypy)
rest_cherrypy:
  port: 8000
  disable_ssl: True

external_auth:
  pam:
    saltadmin:
      - .*
      - '@wheel'
      - '@runner'
SALTCONF

# Step 3: Enable and start
sudo systemctl enable salt-master
sudo systemctl start salt-master
sudo systemctl enable salt-api
sudo systemctl start salt-api
```

**Windows PowerShell (as Administrator) — firewall setup:**
```powershell
# 1. Set .wslconfig (run as bono user, not admin)
# C:\Users\bono\.wslconfig:
# [wsl2]
# networkingMode=mirrored
# memory=4GB

# 2. Restart WSL2
wsl --shutdown

# 3. Hyper-V firewall layer (separate from Defender)
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow

# 4. Windows Defender Firewall — LAN-scoped inbound rules
New-NetFirewallRule -DisplayName "Salt Master ZMQ 4505" -Direction Inbound -Protocol TCP -LocalPort 4505 -RemoteAddress 192.168.31.0/24 -Action Allow
New-NetFirewallRule -DisplayName "Salt Master ZMQ 4506" -Direction Inbound -Protocol TCP -LocalPort 4506 -RemoteAddress 192.168.31.0/24 -Action Allow
New-NetFirewallRule -DisplayName "Salt API 8000" -Direction Inbound -Protocol TCP -LocalPort 8000 -RemoteAddress 192.168.31.0/24 -Action Allow
```

**Version verification:**
```bash
# Inside WSL2 after install
salt --version          # should show Salt 3008.x
salt-api --version      # should show same
```

---

## Architecture Patterns

### Recommended Project Structure (WSL2 Ubuntu file layout)

```
/etc/salt/
├── master              # Salt master config (interface, file_roots, rest_cherrypy, eauth)
└── pki/
    ├── master/
    │   ├── minions/    # Accepted minion public keys (pod1...pod8, server)
    │   └── minions_pre/ # Pending (unaccepted) minion keys
    └── master.pem      # Master private key

/srv/salt/              # Master fileserver root (rc-agent.exe will live here in Phase 38)
└── top.sls             # Highstate targeting (not needed for Phase 36)

/srv/pillar/            # Pillar root (not used in Phase 36)
```

### Pattern 1: WSL2 Mirrored Networking

**What:** Add `networkingMode=mirrored` to `C:\Users\bono\.wslconfig`, restart WSL2 with `wsl --shutdown`. WSL2 instance then shares Windows host's NIC at 192.168.31.27. Salt master binds to `interface: 0.0.0.0` inside Ubuntu, which resolves to 192.168.31.27 from a pod's perspective.

**When to use:** Always — this is the required networking mode for this deployment. NAT is never acceptable.

**Verification:**
```bash
# Inside WSL2 — should show 192.168.31.27, not 172.x.x.x
wsl hostname -I
ip addr show | grep "192.168"

# From Pod 8 (Windows PowerShell) — the TRUE connectivity test
Test-NetConnection 192.168.31.27 -Port 4505
# Must return: TcpTestSucceeded : True
```

### Pattern 2: Two-Layer Firewall Opening

**What:** When WSL2 mirrored mode is active, there are two independent firewall layers that must both be open: (1) Windows Defender Firewall (the familiar one, configured via `netsh` or PowerShell `New-NetFirewallRule`) and (2) Hyper-V Firewall (added in WSL 2.0.9+, configured via `Set-NetFirewallHyperVVMSetting` or `New-NetFirewallHyperVRule`). Missing either layer produces a timeout failure.

**Example — verify both layers:**
```powershell
# Check Hyper-V firewall setting
Get-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}'
# Look for: DefaultInboundAction : Allow

# Check Defender rules for Salt ports
Get-NetFirewallRule -DisplayName "Salt*" | Format-Table -AutoSize
```

### Pattern 3: salt-api PAM Auth

**What:** salt-api uses PAM authentication against a Linux system user (`saltadmin`) in WSL2 Ubuntu. The racecontrol server (.23) will POST credentials to `http://192.168.31.27:8000/login` to get a token, then use that token in subsequent `/run` calls.

**When to use:** All racecontrol-to-Salt API calls in Phase 38. For Phase 36, verify the login endpoint works from .23 with a curl test.

**Login request:**
```bash
# From racecontrol server (.23) or test from James's machine:
curl -s -k http://192.168.31.27:8000/login \
  -H "Accept: application/json" \
  -d username=saltadmin \
  -d password=<password> \
  -d eauth=pam
# Expected: {"return": [{"token": "...", "expire": ..., "user": "saltadmin", ...}]}
```

**Create the saltadmin user (inside WSL2):**
```bash
sudo useradd -m -s /bin/bash saltadmin
sudo passwd saltadmin  # set a strong password
# Store password in racecontrol.toml [salt] section for Phase 38
```

### Pattern 4: Task Scheduler At-Login Auto-Start

**What:** Windows Task Scheduler task triggered at login of user `bono` runs `wsl -e bash -c "sudo systemctl start salt-master && sudo service salt-api start"`. This fires after the user session is established, ensuring WSL2 is ready.

**When to use:** INFRA-05. Task Scheduler is the correct approach because WSL2 requires an active user session; HKLM Run keys fire too early (before WSL2 session is initialized).

**Task Scheduler XML (create via `schtasks /create` or Import-ScheduledTask):**
```xml
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <UserId>RACING-POINT-JAMES\bono</UserId>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>RACING-POINT-JAMES\bono</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <ExecutionTimeLimit>PT1M</ExecutionTimeLimit>
    <StartWhenAvailable>true</StartWhenAvailable>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>wsl</Command>
      <Arguments>-e bash -c "sudo service salt-master start &amp;&amp; sudo service salt-api start"</Arguments>
    </Exec>
  </Actions>
</Task>
```

**Alternative simpler form (PowerShell):**
```powershell
$action = New-ScheduledTaskAction -Execute "wsl" `
    -Argument '-e bash -c "sudo service salt-master start && sudo service salt-api start"'
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "bono"
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 1)
Register-ScheduledTask -TaskName "SaltMasterStart" -Action $action `
    -Trigger $trigger -Settings $settings -RunLevel Highest -Force
```

**Sudoers entry required (inside WSL2) — to allow passwordless service start:**
```bash
echo "bono ALL=(ALL) NOPASSWD: /usr/sbin/service salt-master *, /usr/sbin/service salt-api *" \
  | sudo tee /etc/sudoers.d/salt-services
```

### Pattern 5: systemd Restart=always for salt-master and salt-api

**What:** Override systemd unit files to add `Restart=always` so both services recover from crashes without WSL2 restart. Since WSL2 in mirrored mode supports systemd (Ubuntu 24.04 default), this is zero-cost crash recovery.

**When to use:** Always — CONTEXT.md leaves this to Claude's discretion, and `Restart=always` has no downside on a stable LAN-only master.

```bash
# Create override directories and files
sudo mkdir -p /etc/systemd/system/salt-master.service.d
sudo tee /etc/systemd/system/salt-master.service.d/restart.conf << 'EOF'
[Service]
Restart=always
RestartSec=5
EOF

sudo mkdir -p /etc/systemd/system/salt-api.service.d
sudo tee /etc/systemd/system/salt-api.service.d/restart.conf << 'EOF'
[Service]
Restart=always
RestartSec=5
EOF

sudo systemctl daemon-reload
```

### Anti-Patterns to Avoid

- **NAT mode with portproxy:** portproxy rules reference a WSL NAT IP (172.x.x.x) that changes on every WSL restart. The rules silently route to a dead IP after overnight restart. WSL2 fails to auto-start at venue open time.

- **Blanket Hyper-V allow-all as firewall rule:** The CONTEXT.md recommends targeted ports (4505, 4506, 8000). `Set-NetFirewallHyperVVMSetting -DefaultInboundAction Allow` allows all inbound to WSL2, not just Salt ports. For a LAN-only venue this is acceptable risk, but use targeted `New-NetFirewallHyperVRule` for each port if security posture matters.

- **Relying on WSL2 systemd boot config alone (`/etc/wsl.conf [boot] command`):** The `/etc/wsl.conf [boot] command` starts services when WSL2 first launches but is unreliable when WSL2 starts from a Task Scheduler trigger. Use both: `[boot] command` as the primary, Task Scheduler as the Windows-side trigger that ensures WSL2 is running.

- **Testing connectivity from James's machine only:** `Test-NetConnection 192.168.31.27 -Port 4505` from .27 is not a valid test — the Windows host can reach WSL2 regardless of networking mode. The test must run from an actual pod (.89, .33, .28, etc.) or from the server (.23).

- **Setting `auto_accept: True` in `/etc/salt/master`:** Acceptable for this closed venue LAN (CONTEXT.md notes this is Claude's discretion), but requires binding the master to the venue NIC only (`interface: 192.168.31.27`) to prevent any non-venue device from auto-registering. If auto_accept is False, run `salt-key -A` after first minion connects (Phase 37 work).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WSL2 auto-start | Custom watchdog process to start WSL2 | Windows Task Scheduler at-login trigger | Task Scheduler is the system mechanism; a custom watchdog adds another process to manage |
| Port forwarding | netsh portproxy rules with dynamic WSL IP | WSL2 mirrored networking mode | Portproxy rules stale after every WSL restart; mirrored mode is permanent configuration |
| Salt service crash recovery | A PowerShell watchdog polling `sc query` | systemd `Restart=always` inside WSL2 | systemd is already present in WSL2 Ubuntu 24.04; 5-second automatic restart at OS level with zero extra code |
| salt-api token auth | Custom auth middleware | PAM eauth (built into salt-api) | PAM uses system user credentials; zero extra config beyond creating a user and adding to sudoers |
| Firewall management | Rust code to manage firewall rules | `New-NetFirewallRule` PowerShell + `Set-NetFirewallHyperVVMSetting` | One-time setup commands; no need for programmatic management for the master machine |

**Key insight:** Every problem in this phase has a standard Windows/Linux system mechanism that handles it. The value of this phase is correct configuration, not custom code.

---

## Common Pitfalls

### Pitfall 1: WSL2 NAT Makes Salt Ports Unreachable from LAN

**What goes wrong:** Salt master runs in WSL2 default NAT mode — gets a 172.x.x.x IP invisible to 192.168.31.x pod subnet. Pods cannot connect. `salt-key -L` shows zero pending keys. Nothing indicates a network problem.

**Why it happens:** WSL2 uses a Hyper-V virtual switch with NAT — the instance IP is only routable from the Windows host itself.

**How to avoid:** Enable mirrored networking in `.wslconfig` before installing Salt. Run `wsl --shutdown` and verify `wsl hostname -I` shows `192.168.31.27`.

**Warning signs:** `wsl hostname -I` shows `172.x.x.x`; `Test-NetConnection` from Pod 8 returns `TcpTestSucceeded: False`.

### Pitfall 2: Hyper-V Firewall Silently Blocks Inbound Even After Mirrored Mode

**What goes wrong:** A second firewall layer (Hyper-V Firewall, WSL 2.0.9+) has `DefaultInboundAction: Block`. Windows Defender rules do not affect it. Failure looks identical to Pitfall 1.

**Why it happens:** Microsoft added Hyper-V Firewall as an independent security layer. It requires `Set-NetFirewallHyperVVMSetting` or `New-NetFirewallHyperVRule` — not `netsh advfirewall`.

**How to avoid:** Run the Hyper-V firewall command as part of the same setup sequence as `.wslconfig`, not after troubleshooting failures.

**Warning signs:** `ss -tlnp | grep 45` inside WSL shows LISTEN on 0.0.0.0:4505, mirrored mode active, yet `Test-NetConnection` from Pod 8 still times out (timeout = firewall drop; connection refused = port closed — these are different failure modes).

### Pitfall 3: Testing Connectivity From James's Machine (.27) Instead of From a Pod

**What goes wrong:** `Test-NetConnection 192.168.31.27 -Port 4505` from .27 succeeds in both NAT and mirrored modes because the Windows host can always reach WSL2. The real test only passes once a pod can connect.

**Why it happens:** The Windows host and WSL2 always share a local virtual interface regardless of networking mode.

**How to avoid:** Always run the connectivity test from Pod 8 (via remote exec if needed): `Test-NetConnection 192.168.31.27 -Port 4505`. This is the INFRA-01 success criterion.

**Warning signs:** Setup "worked" on .27 but pod minion connects to nothing; `salt-key -L` shows no pending keys after Pod 8 minion starts.

### Pitfall 4: WSL2 Not Ready When Task Scheduler Fires

**What goes wrong:** Task Scheduler at-login fires the `wsl` command before WSL2 has initialized its userspace. The service start command fails silently. Salt is not running after login.

**Why it happens:** At-login triggers fire very early in the login sequence. WSL2 initialization requires a short warm-up period on first start.

**How to avoid:** Add a `Start-Sleep 5` before the service start commands, or use a wrapper `.bat` script: `timeout /t 5 /nobreak > nul && wsl -e bash -c "sudo service salt-master start && sudo service salt-api start"`. Also configure `[boot] command` in `/etc/wsl.conf` as a belt-and-suspenders approach — it fires when WSL2 itself starts.

**Warning signs:** Immediately after login, `wsl -- sudo service salt-master status` shows not running; starting manually works fine.

### Pitfall 5: salt-api Not in PATH or Not Installed by Bootstrap Script

**What goes wrong:** Bootstrap script with `-N` (no minion) flag installs salt-master but not all salt packages. `salt-api` may not be installed, and `systemctl enable salt-api` fails with "Unit not found."

**Why it happens:** The bootstrap script with `-M -N` installs master-only packages. salt-api is a separate package (`salt-api`) that may not be pulled in automatically.

**How to avoid:** After bootstrap, explicitly install: `sudo apt-get install -y salt-api`. Verify: `which salt-api && salt-api --version`.

**Warning signs:** `systemctl status salt-api` shows "Unit salt-api.service could not be found."

### Pitfall 6: sudoers Entry Missing — Task Scheduler Prompt for Password

**What goes wrong:** The Task Scheduler action runs `sudo service salt-master start` but the bono user requires a password for sudo. The command hangs or fails silently because there's no terminal for password input.

**Why it happens:** sudo requires interactive authentication unless NOPASSWD is configured for the specific commands.

**How to avoid:** Add a sudoers file for salt service management before testing the Task Scheduler action. Verify with `sudo -n service salt-master start` (the `-n` flag = non-interactive; if it fails, sudoers is not configured correctly).

**Warning signs:** Manual WSL `sudo service salt-master start` works but Task Scheduler action results in salt-master not running after login.

---

## Code Examples

Verified patterns from official sources and STACK.md/ARCHITECTURE.md:

### WSL2 .wslconfig (C:\Users\bono\.wslconfig)

```ini
# Source: Microsoft WSL docs https://learn.microsoft.com/en-us/windows/wsl/networking
[wsl2]
networkingMode=mirrored
memory=4GB
# CPU: no limit (per user decision)
# swap: 2GB recommended (half of RAM limit, covers Salt burst startup)
swap=2GB
```

### /etc/salt/master (minimal Phase 36 config)

```yaml
# Source: Salt docs https://docs.saltproject.io/en/latest/ref/configuration/master.html
# Bind all interfaces — in mirrored mode, this picks up 192.168.31.27
interface: 0.0.0.0

# Never auto-accept in general, but acceptable for closed venue LAN
# If auto_accept: True, MUST bind interface: 192.168.31.27 (not 0.0.0.0) for safety
auto_accept: False

# File roots — rc-agent.exe and state files live here (Phase 38+)
file_roots:
  base:
    - /srv/salt

pillar_roots:
  base:
    - /srv/pillar

# salt-api REST interface (rest_cherrypy module)
rest_cherrypy:
  port: 8000
  disable_ssl: True    # LAN-only deployment — no TLS needed

# External auth for salt-api — PAM uses Linux system users
external_auth:
  pam:
    saltadmin:
      - '.*'           # Allow all functions
      - '@wheel'
      - '@runner'

# Faster dead-minion detection (Phase 37+)
ping_interval: 20
```

### /etc/wsl.conf (inside WSL2 Ubuntu)

```ini
# Source: Microsoft WSL docs
[boot]
systemd=true
# Belt-and-suspenders: also start services when WSL2 boots
# (Task Scheduler handles the Windows-side trigger)
command="service salt-master start && service salt-api start"
```

### Connectivity Verification Sequence (run in this order)

```bash
# Step 1: Verify mirrored mode active (inside WSL2)
ip addr show | grep "192.168.31"
# Must show: inet 192.168.31.27/24

# Step 2: Verify salt-master listening (inside WSL2)
sudo ss -tlnp | grep "4505\|4506\|8000"
# Must show: LISTEN on 0.0.0.0:4505, 0.0.0.0:4506, 0.0.0.0:8000

# Step 3: Verify Hyper-V firewall setting (Windows PowerShell, admin)
Get-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}'
# Look for: DefaultInboundAction : Allow

# Step 4: Verify from Pod 8 (the only test that matters for INFRA-01)
# Run via pod-agent :8090 or physical access:
# Test-NetConnection 192.168.31.27 -Port 4505
# Must return: TcpTestSucceeded : True

# Step 5: Verify salt-call locally (inside WSL2)
sudo salt-call --local test.ping
# Must return: local: True

# Step 6: Verify salt-api login from .23 (INFRA-04)
curl -s http://192.168.31.27:8000/login \
  -H "Accept: application/json" \
  -d username=saltadmin \
  -d password=<password> \
  -d eauth=pam
# Must return HTTP 200 with JSON token
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WSL2 NAT + netsh portproxy | WSL2 mirrored networking mode | Windows 11 22H2 (Sept 2022) | Eliminates portproxy fragility; WSL2 shares LAN IP permanently |
| No Hyper-V firewall | Hyper-V Firewall added with DefaultInboundAction:Block | WSL 2.0.9 (2023) | Requires additional `Set-NetFirewallHyperVVMSetting` step that older guides don't mention |
| Salt MSI installer (Windows) | Salt EXE installer (NSIS-based) | Salt 3007+ | MSI format used in 3006 era; current 3007/3008 Windows packages are EXE with `/S` silent flags |
| salt-master on 3007 STS | salt-master on 3008 LTS | 2025 April (3008 release) | 3007 EOL is 2026-03-31 (this month); only 3008 is appropriate for new deployments |

**Deprecated/outdated:**
- Salt 3007 STS: EOL 2026-03-31. Do not install.
- Salt 3006 LTS: Legacy, security patches only. Do not install for new deployments.
- MSI installer format for Salt Windows packages: replaced by EXE in 3007/3008 era.
- `/etc/wsl.conf [boot] command` as sole auto-start: unreliable alone; pair with Task Scheduler.

---

## Open Questions

1. **Exact Windows hostname for Task Scheduler XML**
   - What we know: User is `bono`, machine is James's workstation at .27
   - What's unclear: The exact machine name for the `<UserId>MACHINENAME\bono</UserId>` field
   - Recommendation: Use `$env:COMPUTERNAME` in the PowerShell setup script to populate this dynamically, or use `.\bono` for local user reference

2. **WSL2 distro already installed vs fresh install**
   - What we know: James's machine has WSL2 available (Windows 11 Pro with RTX 4070)
   - What's unclear: Whether an Ubuntu 24.04 distro is already installed or needs `wsl --install -d Ubuntu-24.04`
   - Recommendation: First task in Wave 1 should be `wsl --list --verbose` to check; install if absent

3. **saltadmin password storage**
   - What we know: Password goes in racecontrol.toml `[salt]` section for Phase 38
   - What's unclear: Whether the password should live in racecontrol.toml now (Phase 36) or be deferred to Phase 38
   - Recommendation: Create the saltadmin user in Phase 36 with a strong password; document the placeholder `[salt]` section in racecontrol.toml but leave the Rust config struct for Phase 38

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust) — 269 unit tests + 66 integration tests, all passing as of 2026-03-17 |
| Config file | No separate config — `cargo test -p racecontrol-crate` from repo root |
| Quick run command | `cargo test -p racecontrol-crate --lib` (unit tests only, ~0.02s) |
| Full suite command | `cargo test -p racecontrol-crate` (unit + integration, ~45s) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | Notes |
|--------|----------|-----------|-------------------|-------|
| INFRA-01 | WSL2 mirrored mode, 192.168.31.27 reachable on :4505 from Pod 8 | manual smoke | `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 | Requires physical pod access or pod-agent exec; cannot be automated in Rust test suite |
| INFRA-02 | salt-master 3008 LTS installed, listening :4505/:4506 | manual smoke | `sudo ss -tlnp \| grep 45` inside WSL2; `sudo salt-call --local test.ping` | Infrastructure-only; no Rust test surface |
| INFRA-03 | Both firewall layers open for 4505/4506/8000 | manual smoke | `Get-NetFirewallHyperVVMSetting` + `Get-NetFirewallRule -DisplayName "Salt*"` in PowerShell | Windows system state; no Rust test surface |
| INFRA-04 | salt-api running, login endpoint returns 200+token from .23 | manual smoke | `curl http://192.168.31.27:8000/login -d username=saltadmin -d password=... -d eauth=pam` from .23 | HTTP integration test; runs from server, not from Rust test suite |
| INFRA-05 | After full reboot, salt-master and salt-api running within 60s | manual smoke | Reboot James's machine; after auto-login, check `wsl -- sudo service salt-master status` within 60s | Temporal test; cannot be automated |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol-crate --lib` — confirm existing 269 unit tests still pass (Phase 36 adds no Rust code, but confirms baseline)
- **Per wave merge:** `cargo test -p racecontrol-crate` — full suite including integration tests
- **Phase gate:** All 5 INFRA requirements verified manually as described above; full test suite green

### Wave 0 Gaps

None — existing test infrastructure covers all phase requirements (Phase 36 is infrastructure-only, no Rust code added; all success criteria are manually verified infrastructure states).

The existing test suite (269 unit + 66 integration) serves as a regression check to confirm no unintended side effects. No new test files needed for Phase 36.

---

## Sources

### Primary (HIGH confidence)
- [Microsoft WSL Networking docs](https://learn.microsoft.com/en-us/windows/wsl/networking) — mirrored mode, NAT limitations, Hyper-V firewall layer (updated 2025-12)
- [Salt Windows Install Guide](https://docs.saltproject.io/salt/install-guide/en/latest/topics/install-by-operating-system/windows.html) — installer format, config paths, silent install params
- [Salt Version Support Lifecycle](https://docs.saltproject.io/salt/install-guide/en/latest/topics/salt-version-support-lifecycle.html) — 3007 EOL 2026-03-31, 3008 LTS to ~2027
- [Salt Firewall Guide](https://docs.saltproject.io/en/3007/topics/tutorials/firewall.html) — ports 4505/4506, minion-outbound-only model
- [Broadcom Port Requirements KB](https://knowledge.broadcom.com/external/article/403589/port-requirements-for-saltminionsaltmast.html) — confirms 4505/4506 TCP on master only
- [rest_cherrypy docs](https://docs.saltproject.io/en/latest/ref/netapi/all/salt.netapi.rest_cherrypy.html) — salt-api HTTP REST configuration
- [Salt Bootstrap Script](https://github.com/saltstack/salt-bootstrap) — Ubuntu 24.04 + 3008 stable install flags
- [Salt Master Configuration Reference](https://docs.saltproject.io/en/latest/ref/configuration/master.html) — interface, auto_accept, file_roots, external_auth
- `.planning/research/STACK.md` — WSL2 mirrored mode setup, salt-master install, salt-api config
- `.planning/research/ARCHITECTURE.md` — WSL2 networking decision with evidence, salt-api REST integration seam
- `.planning/research/PITFALLS.md` — WSL2 NAT (P1), Hyper-V firewall (P2), Defender quarantine (P4)

### Secondary (MEDIUM confidence)
- [WSL mirrored mode practical guide](https://informatecdigital.com/en/wsl2-advanced-guide-to-network-configuration-and-nat-and-mirrored-modes/) — Hyper-V firewall rule, .wslconfig setup (community, aligns with Microsoft docs)
- [GitHub: WSL2 mirrored mode multicast bug #10535](https://github.com/microsoft/WSL/issues/10535) — confirms unicast TCP (Salt ZMQ) is NOT affected by the known multicast limitation
- [GitHub: salt-minion service restart stops but doesn't start #65577](https://github.com/saltstack/salt/issues/65577) — Windows service restart bug (Phase 37 concern, not Phase 36)

### Tertiary (context)
- MEMORY.md — pod subnet 192.168.31.x, pod MAC addresses, James's machine .27, DHCP drift history
- `.planning/research/SUMMARY.md` — executive summary of all v6.0 research

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified against official Salt and Microsoft documentation; version lifecycle (3007 EOL this month, 3008 LTS) is authoritative
- Architecture: HIGH — WSL2 mirrored networking decision backed by official Microsoft docs and confirmed community patterns; both firewall layers documented with verified commands
- Pitfalls: HIGH — all critical pitfalls have official source references; WSL2 NAT and Hyper-V firewall pitfalls are the primary risk area and both documented with exact diagnostic commands

**Research date:** 2026-03-17 IST
**Valid until:** 2026-04-17 (stable infrastructure area; WSL2 networking and Salt 3008 LTS will not change significantly in 30 days)
