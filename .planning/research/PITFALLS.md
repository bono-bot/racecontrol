# Pitfalls Research

**Domain:** Adding SaltStack fleet management to existing Windows pod fleet with WSL2 master (RaceControl v6.0)
**Researched:** 2026-03-17
**Confidence:** HIGH (WSL2 networking from official Microsoft docs; Salt service/Windows issues from official Salt docs + verified GitHub issues; cp.get_file silent failure from Salt mailing list + GitHub; path separator from Salt issue tracker)

---

## Critical Pitfalls

### Pitfall 1: WSL2 NAT Architecture Makes Salt Ports Unreachable from LAN

**What goes wrong:**
Salt master runs in WSL2 (Ubuntu) on James's machine (.27). By default WSL2 uses NAT networking — the WSL instance gets a private IP in the 172.30.x.x range, invisible to the rest of the venue LAN. Pods on 192.168.31.x cannot reach 4505/4506 on the WSL instance at all. Salt minions on pods silently fail to connect, logging "No master could be reached" indefinitely. `salt-key -L` on the master shows zero pending keys. Nothing indicates a network problem — it just looks like the minions are not installed.

**Why it happens:**
WSL2 implements networking via a Hyper-V virtual switch with NAT, meaning the WSL instance IP is only routable from the Windows host itself, not from other LAN hosts. This is documented by Microsoft: "This isn't the default case in WSL 2. WSL 2 has a virtualized ethernet adapter with its own unique IP address." Port 4505/4506 on the WSL instance are not exposed to the LAN unless explicitly forwarded or mirrored networking is used.

**How to avoid:**
Use mirrored networking mode (Windows 11 22H2+ required — James's machine qualifies). Add to `C:\Users\bono\.wslconfig`:
```
[wsl2]
networkingMode=mirrored
```
Then in elevated PowerShell, open the Hyper-V firewall (separate from Windows Defender Firewall):
```powershell
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow
```
Restart WSL after applying. In mirrored mode, WSL shares Windows's LAN IP (.27), so pods connect to 192.168.31.27:4505 and 192.168.31.27:4506. The WSL IP no longer changes on reboot. Verify before deploying any minion: `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 must return `TcpTestSucceeded: True`.

**Warning signs:**
- `salt 'pod*' test.ping` returns empty output with no timeout errors
- `salt-key -L` shows no pending keys even after minion install on a pod
- `Test-NetConnection 192.168.31.27 -Port 4505` from a pod returns `TcpTestSucceeded: False`
- `wsl hostname -I` shows 172.30.x.x instead of 192.168.31.27

**Phase to address:** Phase 1 (WSL2 Salt master setup) — must be verified before deploying any minion. This is the single most likely blocker.

---

### Pitfall 2: Hyper-V Firewall Silently Blocks Inbound to WSL2 Even After Mirrored Mode Enabled

**What goes wrong:**
Even after enabling mirrored networking, Windows 11 22H2+ with WSL 2.0.9+ activates a Hyper-V firewall layer by default with `DefaultInboundAction: Block`. This firewall is separate from Windows Defender Firewall and blocks inbound connections from the LAN to the WSL instance even when mirrored mode is active and the Windows host firewall allows it. Salt minions on pods cannot reach 4505/4506. The failure looks identical to Pitfall 1, making diagnosis confusing.

**Why it happens:**
Microsoft added Hyper-V Firewall as a security layer in WSL 2.0.9. It operates independently of the Windows host firewall. The default inbound policy is Block. The `netsh advfirewall` rules on the Windows host do not affect this layer — it requires separate `Set-NetFirewallHyperVVMSetting` or `New-NetFirewallHyperVRule` commands.

**How to avoid:**
Run after enabling mirrored mode (combined with Pitfall 1 fix):
```powershell
# Simple: allow all inbound to WSL on a private closed LAN
Set-NetFirewallHyperVVMSetting -Name '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -DefaultInboundAction Allow

# Alternative: narrow rules for Salt ports only
New-NetFirewallHyperVRule -Name "Salt-4505" -DisplayName "Salt Publisher" -Direction Inbound -VMCreatorId '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -Protocol TCP -LocalPorts 4505
New-NetFirewallHyperVRule -Name "Salt-4506" -DisplayName "Salt Request" -Direction Inbound -VMCreatorId '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' -Protocol TCP -LocalPorts 4506
```
Also ensure the Windows host firewall allows inbound on 4505/4506:
```
netsh advfirewall firewall add rule name="Salt Master" dir=in action=allow protocol=TCP localport=4505-4506
```

**Warning signs:**
- `Test-NetConnection 192.168.31.27 -Port 4505` from a pod **times out** (not "connection refused" — times out, indicating firewall drop not port closed)
- `ss -tlnp | grep 45` inside WSL shows LISTEN on 0.0.0.0:4505 and 0.0.0.0:4506 (master bound correctly)
- The failure persists even after verifying mirrored mode is active

**Phase to address:** Phase 1 (WSL2 Salt master setup) — add Hyper-V firewall rule as part of the documented setup sequence, not as an afterthought.

---

### Pitfall 3: WSL2 IP Changes on Reboot Breaks Any NAT Portproxy Fallback

**What goes wrong:**
If mirrored mode is unavailable or fails to activate (e.g., Hyper-V not enabled, older Windows build), the fallback is `netsh interface portproxy` to forward 4505/4506 from the Windows host NIC to the WSL NAT IP. The portproxy rule's `connectaddress` is a static IP captured at rule-creation time. After any WSL restart or Windows reboot, WSL gets a new NAT IP. The portproxy silently routes connections to the old dead IP. Pods report connection refused or timeout with no indication the rule is stale.

**Why it happens:**
WSL2 generates a new IP for the Hyper-V virtual adapter on each start. This is a known Microsoft limitation with NAT mode. There is no built-in mechanism to update portproxy rules when WSL's IP changes.

**How to avoid:**
Use mirrored mode (Pitfall 1) to eliminate this problem entirely. If portproxy must be used as a fallback, write a PowerShell script that: (1) queries `(wsl hostname -I).Trim()` for the current WSL IP, (2) deletes old portproxy rules with `netsh interface portproxy delete v4tov4 listenport=4505`, (3) adds fresh rules with the new IP. Schedule this script to run at logon via Task Scheduler. Never document a static WSL IP anywhere in configs.

**Warning signs:**
- Salt worked after setup but fails after the first reboot
- `netsh interface portproxy show all` shows a `connectaddress` that does not match `wsl hostname -I`
- Issue resolves temporarily after manually re-running the portproxy command with the current WSL IP

**Phase to address:** Phase 1 (WSL2 Salt master setup) — document mirrored mode as the required path; portproxy is a fallback with documented maintenance overhead.

---

### Pitfall 4: Windows Defender Quarantines Salt Minion Binary or Python Runtime After Install

**What goes wrong:**
The salt-minion installer ships with a bundled Python runtime and ZeroMQ binaries that Windows Defender heuristics flag as suspicious (packed executables, Python interpreters running as services). Defender silently quarantines files asynchronously after the installer exits. The installer returns exit code 0 (success), but the salt-minion service starts and then immediately stops. The deploy script sees install success, moves on, and the pod is never actually managed by Salt.

**Why it happens:**
Real-time protection scans newly written executables after they land on disk. Salt's bundled Python and ZeroMQ binaries match heuristic patterns for obfuscated executables. The quarantine happens 5-15 seconds after install, after the installer has already reported success. The RC project already handles this for `C:\RacingPoint\` — Salt's install path is separate and not covered by the existing exclusions.

**How to avoid:**
Add Defender exclusions before running the salt-minion installer. Extend install.bat with:
```bat
powershell -Command "Add-MpPreference -ExclusionPath 'C:\Program Files\Salt Project\Salt'"
powershell -Command "Add-MpPreference -ExclusionPath 'C:\ProgramData\Salt Project\Salt'"
powershell -Command "Add-MpPreference -ExclusionProcess 'salt-minion.exe'"
```
Run these before the silent installer command. After install, wait 15 seconds then check: `sc query salt-minion` must show STATE: RUNNING. Do not accept install success based on installer exit code alone.

**Warning signs:**
- `sc query salt-minion` shows `STATE: STOPPED` 15-30 seconds after install despite installer returning 0
- Windows Security event log shows "threat quarantined" entries timestamped near install time
- `Get-MpThreatDetection | Where ThreatName -match salt` lists salt files

**Phase to address:** Phase 2 (salt-minion bootstrap in install.bat) — exclusions must precede the installer command.

---

### Pitfall 5: Salt Minion Service Cannot Restart Itself on Windows (Stops But Does Not Start)

**What goes wrong:**
Running `salt pod1 service.restart salt-minion` stops the service but never starts it again. The pod becomes unreachable via Salt until someone physically restarts the service on-site. This is a critical operational problem: any Salt state that tries to restart the minion (e.g., after config change) permanently loses the pod until manual intervention. This has been a confirmed bug since at least 2014, with a fresh report as recently as 2024 (issue #65577).

**Why it happens:**
When salt-minion receives a restart command, it calls the Windows Service Control Manager to stop itself. Once stopped, there is no process left to call SCM to start again. On Linux, systemd handles this outside the process. On Windows, Salt implements a scheduled task workaround — but this only works when the function detects the `salt-minion` service name specifically, and breaks if `schtasks.exe` is not accessible in the session running the command.

**How to avoid:**
Configure Windows Service Recovery settings for salt-minion during minion install so SCM automatically restarts it on crash or stop:
```bat
sc failure salt-minion reset= 60 actions= restart/5000/restart/10000/restart/30000
```
This tells SCM to restart salt-minion 5s after first failure, 10s after second, 30s after third, resetting the count after 60s of clean uptime. This means if the service stops for any reason (including a failed restart attempt), SCM handles the restart — not Salt. Verify with `sc qfailure salt-minion` on each pod after install. Note: the MSI installer may overwrite service properties — check after every reinstall.

**Warning signs:**
- `salt pod1 service.restart salt-minion` returns success but subsequent `salt pod1 test.ping` times out
- `sc query salt-minion` on the pod shows `STATE: STOPPED` with no automatic recovery
- `sc qfailure salt-minion` shows no failure actions configured

**Phase to address:** Phase 2 (salt-minion bootstrap in install.bat) — add `sc failure` immediately after silent minion install.

---

### Pitfall 6: Minion Key Pending = Zero Feedback (Looks Like Network Failure)

**What goes wrong:**
After salt-minion installs and starts on a pod, `salt pod1 test.ping` returns absolutely nothing — no error, no timeout message, just an empty prompt return. New deployers spend 30-60 minutes debugging firewall rules, WSL networking, and service status when the actual fix is one command: `salt-key -A`. The minion is connected and waiting but cannot receive or respond to commands until its public key is accepted on the master.

**Why it happens:**
Salt's security model requires explicit key acceptance. The minion sends its public key on first connection, entering a "pending" queue. The master routes zero commands to pending minions. Crucially, there is no error message when a command is sent to a minion with an unaccepted key — the command simply disappears silently.

**How to avoid:**
For a closed venue LAN, add `auto_accept: True` to `/etc/salt/master` on the WSL2 instance. This accepts all new keys automatically. Acceptable risk for a private LAN with no guest or external access on the same subnet — document the decision. Alternatively, use preseed keys: pre-generate keypairs on the master with `salt-key --gen-keys=pod1`, copy `pod1.pub` to `/etc/salt/pki/master/minions/pod1`, and ship `pod1.pem`/`pod1.pub` to the pod's minion config directory before starting the service. The minion connects with a pre-accepted key and is immediately addressable. Build key acceptance verification into the deploy workflow: `salt-key -L` check after every pod deploy before testing.

**Warning signs:**
- `salt 'pod*' test.ping` returns empty output despite minion service running on the pod
- `salt-key -L` shows pod names under "Unaccepted Keys" (not "Accepted Keys")
- `salt-key -L` shows no keys at all (minion has not connected yet — different problem: networking)

**Phase to address:** Phase 1 (Salt master config) for auto_accept decision; Phase 2 (minion bootstrap) for preseed key deployment.

---

### Pitfall 7: Removing remote_ops.rs Without Auditing Shared AppState Initialization

**What goes wrong:**
`remote_ops.rs` hosts the port 8090 HTTP listener, but it may also initialize fields in `AppState` or spawn background tasks that other modules depend on. Deleting the module compiles cleanly in Rust — the compiler only catches type errors, not "initialized in deleted module, used in surviving module" patterns. The rc-agent binary starts on pods, the WebSocket connection to racecontrol establishes, but billing signals or game state updates silently fail because a shared Arc or channel was only initialized in the remote_ops startup path.

**Why it happens:**
Rust's module system does not track runtime initialization dependencies. A `Arc<Mutex<RemoteOpsState>>` stored in AppState compiles fine even if the only code that populates it is removed. The bug surfaces as a runtime panic (unwrap on None) or deadlock at the first point where the surviving code tries to read or write the now-empty field.

**How to avoid:**
Before deleting remote_ops.rs: run `grep -r "remote_ops\|RemoteOps\|port.*8090\|8090.*port" crates/` to find all references. Audit every `AppState` field that remote_ops.rs writes at startup. For each field: confirm another module initializes it, or move initialization to `main.rs`. Write characterization tests that exercise the WebSocket path (game state, billing start/stop, lock screen) without remote_ops running — make these tests pass before deletion. After deletion: deploy to Pod 8 only first, run a full billing lifecycle manually, confirm no panics in rc-agent logs before rolling to all 8 pods.

**Warning signs:**
- `cargo build` succeeds after deletion but rc-agent panics at startup on Pod 8 with `called Option::unwrap() on None` or thread panic in the AppState initialization phase
- Billing sessions start but never complete (WS disconnect not detected, session hangs)
- Lock screen does not appear on session end (game status update path broken)
- rc-agent logs show no errors but racecontrol dashboard shows pod as OFFLINE

**Phase to address:** Phase 3 (remove remote_ops.rs) — write characterization tests before deleting anything. Canary deploy to Pod 8, verify billing lifecycle, then roll to fleet.

---

### Pitfall 8: Slimmed install.bat Accidentally Removes Firewall Rules rc-agent Needs

**What goes wrong:**
The current install.bat includes `netsh advfirewall` commands for rc-agent's ports. When slimming install.bat to only "Defender exclusions + rc-agent binary + salt-minion bootstrap," the developer removes the netsh commands thinking "Salt handles firewall" or "rc-agent's Rust code handles it at startup." The pods deploy cleanly, salt-minion connects, but racecontrol cannot establish WebSocket connections to rc-agent — the entire pod control plane is dead. Billing, game launch, and lock screen all fail.

**Why it happens:**
Salt only opens firewall rules on the master machine (4505/4506 inbound to master). Salt minion deployment does not open any firewall rules on the minion machine for other services. rc-agent's own firewall auto-configuration (FW-01 through FW-03) runs at startup — but it requires the rc-agent process to have already started with appropriate privileges. If a fresh pod install runs install.bat without the netsh lines and rc-agent starts without those privileges, the firewall rules are never set.

**How to avoid:**
Keep the netsh firewall commands explicitly for rc-agent's ports in the slimmed install.bat. The slimmed script should contain in this order: (1) Defender exclusions for `C:\RacingPoint\` and Salt paths, (2) `mkdir C:\RacingPoint` if it does not exist, (3) rc-agent binary copy, (4) HKLM Run key for `start-rcagent.bat`, (5) Salt minion silent install, (6) `sc failure salt-minion` recovery config, (7) explicit `netsh advfirewall` rules for rc-agent's ports, (8) verification step (both services running, port reachable). Do not remove any netsh rule without first confirming rc-agent's Rust startup code sets that exact rule.

**Warning signs:**
- Pod deploys cleanly, `salt pod1 test.ping` returns True, but racecontrol dashboard shows pod as OFFLINE
- WebSocket connection from racecontrol to pod times out (not refused — times out indicates firewall drop)
- `salt pod1 cmd.run 'netsh advfirewall firewall show rule name=all verbose' shell=cmd` output does not include a rule for rc-agent's port

**Phase to address:** Phase 2 (install.bat rewrite) — build an explicit verification checklist into install.bat's exit sequence.

---

### Pitfall 9: Salt cmd.run Backslash Paths Silently Fail on Windows Minions

**What goes wrong:**
Salt state files are YAML. YAML double-quoted strings interpret backslashes as escape sequences. A state with `cmd.run: 'copy C:\RacingPoint\rc-agent.exe C:\RacingPoint\rc-agent.exe.bak'` sends a garbled path to the minion. The command arrives with missing characters (`C:RacingPointrc-agent.exe`), causes "The system cannot find the path specified," and Salt reports a non-zero exit code with a cryptic error rather than pointing to the escaping problem. This affects `cmd.run`, `file.managed`, and `file.file_exists` equally.

**Why it happens:**
Python parses YAML and interprets backslash sequences. `\R` becomes just `R`, `\P` becomes just `P`. Single-quoted YAML strings pass backslashes through, but cmd.exe also has its own interpretation layer. The issue is compounded by the salt:// file server, where minionfs paths with Windows drive letters (e.g., `salt:///c:/file.txt`) require removing the colon to work on Windows minions.

**How to avoid:**
Use forward slashes in all Salt state files and `cmd.run` arguments, even for Windows paths. `C:/RacingPoint/rc-agent.exe` works correctly in both cmd.exe and PowerShell on Windows. Establish this as the team convention before writing any states. When using `cmd.run` with `shell=powershell`, single-quote the outer YAML and double-quote the PowerShell string. Test every new `cmd.run` on Pod 8 first with `salt pod8 cmd.run '...' shell=cmd` before adding to a state file.

**Warning signs:**
- `salt pod8 cmd.run 'type C:\RacingPoint\rc-agent.toml' shell=cmd` returns "The system cannot find the path specified" but the file is known to exist
- Forward-slash version of the same path succeeds immediately
- `salt pod8 file.file_exists 'C:\RacingPoint\rc-agent.exe'` returns False when the file is present

**Phase to address:** Phase 3 (migrate deploy workflow to Salt) — establish path conventions as the first step before writing any state files.

---

### Pitfall 10: cp.get_file Silently Succeeds Without Transferring the File

**What goes wrong:**
`salt pod8 cp.get_file salt://rc-agent.exe C:/RacingPoint/rc-agent.exe` returns True (success), but the file is not on the pod. This is confirmed Salt behavior: `cp.get_file` does not create missing destination directories and does not report an error when the destination path is invalid — it returns True as long as the transfer itself did not throw an exception. On a fresh pod where `C:\RacingPoint\` does not exist yet, the file transfer silently drops to /dev/null.

**Why it happens:**
The Salt file transfer module treats "destination directory does not exist" as a non-error condition. The return value reflects the success of the ZeroMQ transfer protocol, not whether the file landed on disk. This affects both `cp.get_file` and `cp.get_dir` on Windows minions.

**How to avoid:**
Always precede `cp.get_file` with a directory check. In states, use a `file.directory` state before any `cp.get_file` or `file.managed`. In ad-hoc deploy commands:
```
salt pod8 file.makedirs 'C:/RacingPoint/'
salt pod8 cp.get_file salt://rc-agent.exe C:/RacingPoint/rc-agent.exe
salt pod8 file.file_exists 'C:/RacingPoint/rc-agent.exe'
```
The third command must return True before the deploy is considered successful. Never accept `cp.get_file` return value alone as proof of transfer.

**Warning signs:**
- `cp.get_file` returns True but the pod still runs the old rc-agent version (check file modification timestamp)
- `salt pod8 file.file_exists 'C:/RacingPoint/rc-agent.exe'` returns False immediately after a "successful" transfer
- Version mismatches appear between pods: some updated, some silently not

**Phase to address:** Phase 3 (migrate deploy workflow to Salt) — add mandatory post-transfer verification to every deploy procedure before rolling to fleet.

---

### Pitfall 11: DHCP Drift on Pods Breaks Minion Connection Silently

**What goes wrong:**
If a pod's IP changes via DHCP after the salt-minion is connected, the ZeroMQ TCP socket becomes stale. The minion cannot reconnect from its new IP without a service restart. The master keeps the old connection entry as "alive" because ZeroMQ heartbeats may still ACK at the TCP layer from cached state. `salt pod3 test.ping` appears in the master's accepted keys and was once responsive but now returns no output — and nothing in the logs explains why.

**Why it happens:**
ZeroMQ maintains persistent TCP connections and does not auto-detect source IP changes. Salt minion does not detect its own IP change and re-initiate. The master's connection state becomes a zombie: present in `salt-key -L` as Accepted, but unresponsive to commands. DHCP drift has already caused issues in this project (server .23 drifted .51→.23→.4→.23).

**How to avoid:**
Extend the existing DHCP reservation strategy (already applied to server .23 per HOST-01) to all 8 pods before deploying any minion. The pod MAC addresses are documented in MEMORY.md — add static leases in the router for all 8 pods. Set `master_alive_interval: 30` in each minion config so minions detect and reconnect faster when any connectivity change occurs. Also set `ping_interval: 20` in master config for faster dead-minion detection.

**Warning signs:**
- Pod minion worked, then `salt pod3 test.ping` times out with no change to the pod itself
- `salt-key -L` shows pod3 in Accepted Keys but `salt pod3 test.ping` returns empty
- Restarting the salt-minion service on the pod immediately restores connectivity
- Pod's current IP (from router DHCP table) differs from the IP it had when the minion first connected

**Phase to address:** Phase 1 (infrastructure preparation) — all 8 pods need DHCP reservations in the router before any minion is deployed.

---

### Pitfall 12: Minion ID Derives from Windows Hostname — All Pods Get Generic IDs

**What goes wrong:**
When `id:` is not set in the minion config, Salt auto-generates the minion_id from the Windows hostname. Gaming pods typically have generic hostnames like `DESKTOP-AB3F7K` or `GAMING-POD` set by Windows during OEM setup. If two pods were imaged from the same base and never had their hostnames changed, they register with the same minion_id. The first one to connect gets the key accepted; the second silently fails or overwrites the first's accepted key. Fleet targeting with `salt 'pod*' cmd.run` produces unpredictable results.

**Why it happens:**
Salt minion auto-generates minion_id at first start and caches it in `C:\ProgramData\Salt Project\Salt\var\cache\salt\minion\minion_id`. If the hostname is generic, the ID is generic. The ID is locked after first generation — changing the hostname later does not update the cached ID unless the file is manually deleted.

**How to avoid:**
Explicitly set `id: pod{N}` in each pod's `C:\ProgramData\Salt Project\Salt\conf\minion` config file before starting the minion service for the first time. The install.bat already ships per-pod config files (`rc-agent-pod{1-8}.toml`) — create corresponding `salt-minion-pod{1-8}.conf` files in the deploy kit with the correct `id:` and `master:` values pre-set. Deploy the correct config file to each pod as part of install.bat.

**Warning signs:**
- `salt-key -L` shows two pods with the same ID (one in Accepted, one in Unaccepted with identical name)
- `salt 'pod*' test.ping` returns fewer than 8 responses
- Salt commands target the wrong pod (both "pod3" instances respond, or neither does)
- Any pod's minion_id file contains a Windows hostname instead of `pod{N}`

**Phase to address:** Phase 2 (minion bootstrap / install.bat) — per-pod minion config files must be in the deploy kit before first install.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `auto_accept: True` with no minion ID validation | Zero friction for pod re-imaging | Any device on LAN subnet auto-registers and can receive fleet commands | Acceptable for closed venue LAN — document the trust decision; ensure Salt is bound only to venue NIC |
| Hardcoding WSL IP in portproxy rules | Simple one-time setup | Rules break on every reboot/WSL restart, silently | Never — use mirrored mode or dynamic portproxy script |
| Minion ID from auto-generated hostname | Zero config per pod | Duplicate IDs if pods share hostname; untargetable fleet | Never — always set explicit `id: pod{N}` in minion config |
| Deleting remote_ops.rs without characterization tests | Faster deletion | Runtime panics from uninitialized AppState fields; invisible until runtime on pod | Never — write tests first, delete second (Refactor Second rule) |
| Accepting `cp.get_file` return value as transfer success | Simpler deploy script | Silent partial deploys; version drift across pods | Never — always verify with `file.file_exists` after transfer |
| Relying on rc-agent Rust startup code to open firewall rules | DRY, no duplication | Startup code requires specific privileges and session type; fails silently on fresh image | Never for fleet deploy — keep explicit netsh in install.bat |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| WSL2 + Salt master | Assume `localhost` in WSL is reachable from LAN pods | Enable mirrored networking + Hyper-V firewall rule; verify with `Test-NetConnection` from a pod |
| Salt + Windows Defender | Install salt-minion without pre-exclusions | Add Defender exclusions for Salt paths before running installer; verify service RUNNING 15s after install |
| Salt `service.restart salt-minion` on Windows | Call remote restart expecting self-recovery | Set `sc failure` SCM recovery actions; salt-minion restarts via SCM, not via Salt |
| `cp.get_file` on Windows | Trust return value True as proof of file transfer | Follow every transfer with `file.file_exists` verification |
| Salt YAML + Windows paths | Use backslashes in state files | Use forward slashes everywhere in Salt states; test on Pod 8 first |
| Salt minion_id on Windows | Let minion auto-generate ID from hostname | Explicitly set `id: pod{N}` in each pod's minion config before first start |
| rc-agent WebSocket + remote_ops.rs removal | Delete module, trust compiler to catch all issues | Audit AppState fields initialized by remote_ops; write characterization tests; canary on Pod 8 |
| Salt file server + Windows drive paths | Use `salt:///c:/file.txt` syntax | Remove the colon: `salt://minion/c/file.txt`, or use `file.managed` with `source: salt://...` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| `salt '*' cp.get_file` fleet-wide simultaneously | All 8 pods download rc-agent.exe (~10MB) at once via Salt file server in WSL2, saturating the virtual network | Use `--batch-size 2` for binary transfers; or keep HTTP server on Windows side for binaries, Salt for config only | Immediate on large binary deploys |
| `master_alive_interval` set too low (< 30s) | Reconnect storms on stable LAN — minions constantly disconnecting and reconnecting | Set `master_alive_interval: 60` for a stable venue LAN | At 8 pods with interval < 10s: ZeroMQ connection floods |
| Salt targeting `'pod*'` during active customer sessions | Commands execute on pods mid-session: file copies interrupt game I/O, cmd.run commands consume CPU | Schedule fleet-wide Salt operations to off-hours; confirm no active billing sessions before fleet commands | Any time a Salt command does I/O on a pod during active AC or F1 session |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Salt master bound to all interfaces (0.0.0.0) in WSL2 | If WSL2 is accessible from the internet (it is not in this venue setup, but worth noting), master accepts connections from anywhere | Bind salt-master to venue LAN IP only: `interface: 192.168.31.27` in `/etc/salt/master` |
| `auto_accept: True` without binding master to venue NIC | Any device on any network segment reachable from WSL2 can auto-register as a minion | Combine auto_accept with explicit interface binding and per-pod preseed keys for belt-and-suspenders |
| Leaving port 8090 firewall rule active after removing remote_ops.rs | Open attack surface port with no service behind it (connection refused, but still fingerprintable) | Remove the 8090 rule in install.bat when remote_ops.rs is removed; verify with `netsh advfirewall firewall show rule name=all` |
| Minion config shipped without `master:` set | Minion may connect to default `salt` hostname (DNS lookup), or fail to connect, or connect to a stale cached master | Always set `master: 192.168.31.27` explicitly in every per-pod minion config file in the deploy kit |

---

## "Looks Done But Isn't" Checklist

- [ ] **WSL2 master reachable from LAN:** `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 returns `TcpTestSucceeded: True` — not just from James's machine, from an actual pod
- [ ] **Hyper-V firewall open:** Verify the Hyper-V rule is set with `Get-NetFirewallHyperVVMSetting` — connection test alone does not confirm which layer was blocking
- [ ] **Minion key accepted:** `salt-key -L` shows all 8 pod names in "Accepted Keys"; `salt 'pod*' test.ping` returns 8 responses
- [ ] **Service recovery configured:** `sc qfailure salt-minion` on each pod shows restart actions — not empty
- [ ] **Defender exclusions applied:** `Get-MpPreference | Select-Object -ExpandProperty ExclusionPath` on each pod includes Salt install directories
- [ ] **Explicit minion IDs set:** `salt 'pod*' grains.item id` returns `pod1` through `pod8`, not Windows hostnames
- [ ] **rc-agent WebSocket still works after remote_ops.rs removal:** Billing session start/stop, game launch, and lock screen all confirmed on Pod 8 before fleet rollout
- [ ] **Port 8090 rule removed from all pods:** `netsh advfirewall firewall show rule name=all` on each pod shows no rule for 8090
- [ ] **Forward slashes in all Salt states:** No backslash-in-path issues; `grep -r "\\\\" /etc/salt/` finds no backslashes in state files
- [ ] **cp.get_file transfers verified:** `salt 'pod*' file.file_exists 'C:/RacingPoint/rc-agent.exe'` returns True on all pods after every binary deploy
- [ ] **DHCP reservations in place:** Router DHCP table shows static leases for all 8 pod MACs before first minion deployment

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| WSL2 NAT blocks Salt ports | MEDIUM | Enable mirrored mode in `.wslconfig`, restart WSL, set Hyper-V firewall rule, re-test |
| Hyper-V firewall blocks inbound | LOW | Run `Set-NetFirewallHyperVVMSetting` in elevated PowerShell; immediate effect |
| Portproxy stale after reboot (NAT mode) | LOW | Delete old rules with `netsh interface portproxy delete v4tov4 listenport=4505`, re-add with current WSL IP |
| Defender quarantines salt-minion | MEDIUM | `Restore-MpThreat` to restore quarantined files, add exclusion, reinstall minion |
| Minion service stopped, no auto-restart | LOW | `salt pod{N} cmd.run 'sc start salt-minion' shell=cmd` from master; or use Pod 8 web terminal as fallback |
| Key not accepted, pod unresponsive | LOW | `salt-key -a pod{N}` on master; minion responds within seconds |
| remote_ops.rs deletion causes runtime panic | HIGH | Revert deletion from git, write characterization tests covering affected code paths, re-delete with safety net |
| install.bat removed firewall rules for rc-agent | MEDIUM | `salt pod{N} cmd.run 'netsh advfirewall firewall add rule ...' shell=cmd` for each missing rule; update install.bat |
| cp.get_file silent failure | LOW | Confirm destination directory exists with `file.makedirs`, re-run transfer, verify with `file.file_exists` |
| DHCP drift kills minion | LOW | Restart salt-minion service on pod; add DHCP reservation for that pod immediately |
| Duplicate minion IDs | MEDIUM | Stop minion on both pods, delete `C:\ProgramData\Salt Project\Salt\var\cache\salt\minion\minion_id` on both, set explicit `id:` in config, restart minion on each, accept new keys |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| WSL2 NAT breaks LAN reachability | Phase 1: WSL2 master setup | `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 returns True |
| Hyper-V firewall blocks inbound | Phase 1: WSL2 master setup | Connection test from pod succeeds; `Get-NetFirewallHyperVVMSetting` shows Allow |
| WSL2 IP changes on reboot | Phase 1: WSL2 master setup | Reboot James's machine, re-run connectivity test from Pod 8 — still True |
| DHCP drift on pods | Phase 1: infrastructure prep | Router shows static leases for all 8 pod MACs before any minion deployed |
| Defender quarantines salt-minion | Phase 2: install.bat rewrite | `sc query salt-minion` shows RUNNING 30s after install |
| Minion service cannot self-restart | Phase 2: install.bat rewrite | `sc qfailure salt-minion` shows restart actions; verified with deliberate kill + wait |
| Key acceptance dead silence | Phase 1 (auto_accept decision) + Phase 2 (preseed keys) | `salt 'pod*' test.ping` returns True for all pods without manual key accept |
| Minion ID from Windows hostname | Phase 2: minion bootstrap | `salt 'pod*' grains.item id` returns pod1..pod8 |
| remote_ops.rs deletion breaks WebSocket | Phase 3: remove remote_ops.rs | Billing lifecycle test on Pod 8 passes before fleet rollout |
| install.bat strips rc-agent firewall rules | Phase 2: install.bat rewrite | rc-agent WebSocket connected from racecontrol after clean install from slimmed script |
| Backslash paths in Salt states | Phase 3: deploy workflow migration | All states use forward slashes; `salt pod8 cmd.run` path test passes before writing states |
| cp.get_file silent failure | Phase 3: deploy workflow migration | Every deploy ends with `file.file_exists` verification returning True for all pods |

---

## Sources

- [Microsoft WSL Networking Documentation](https://learn.microsoft.com/en-us/windows/wsl/networking) — NAT vs mirrored mode, portproxy, Hyper-V firewall (updated 2024-07-16)
- [Salt Troubleshooting: Minion](https://docs.saltproject.io/en/3006/topics/troubleshooting/minion.html) — connection issues, key acceptance, Windows service problems
- [Salt Firewall Tutorial](https://docs.saltproject.io/en/latest/topics/tutorials/firewall.html) — port requirements, inbound vs outbound directions, netsh commands
- [Salt Configure Minion Reference](https://docs.saltproject.io/en/latest/ref/configuration/minion.html) — master_alive_interval, id, master options
- [Salt Windows Install Guide](https://docs.saltproject.io/salt/install-guide/en/latest/topics/install-by-operating-system/windows.html) — MSI silent install, upgrade pitfalls
- [Salt Security Documentation](https://docs.saltproject.io/salt/user-guide/en/latest/topics/security.html) — auto_accept risks, key management
- [Salt cp module documentation](https://docs.saltproject.io/en/latest/ref/modules/all/salt.modules.cp.html) — cp.get_file directory creation caveat
- [GitHub: salt-minion service restart only stops #65577](https://github.com/saltstack/salt/issues/65577) — confirmed 2024 Windows service restart bug
- [GitHub: Restarting salt-minion kills service #11726](https://github.com/saltstack/salt/issues/11726) — long-standing Windows service limitation
- [GitHub: Salt minion StreamClosedError when Master IP changes #63654](https://github.com/saltstack/salt/issues/63654) — DHCP drift / IP change impact on ZeroMQ
- [GitHub: cp.get_file silently does nothing on Windows (Salt mailing list)](https://groups.google.com/g/salt-users/c/ov9U9pRxAAs) — silent failure on missing directory
- [GitHub: Backslash not working in file.file_exists on Windows #16020](https://github.com/saltstack/salt/issues/16020) — path separator issue confirmed
- [GitHub: Minion upgrade fails on Windows 3006.9 #67054](https://github.com/saltstack/salt/issues/67054) — 2024 upgrade pitfall with "service already exists" error
- [GitHub: WSL2 NIC Bridge mode #4150](https://github.com/microsoft/WSL/issues/4150) — NAT limitation, bridged/mirrored workarounds
- [GitHub: Salt minion ID generation with hostname #31383](https://github.com/saltstack/salt/issues/31383) — minion_id generation issues
- [GitHub: salt-minion uses reverse DNS for minion_id #62478](https://github.com/saltstack/salt/issues/62478) — hostname vs FQDN vs reverse DNS
- [GitHub: salt-master behind NAT (Salt users group)](https://groups.google.com/g/salt-users/c/4BDWyQBJXs0) — NAT reachability workarounds
- [Preseed minion keys tutorial](https://docs.saltproject.io/en/latest/topics/tutorials/preseed_key.html) — pre-generating keypairs to skip pending queue

---
*Pitfalls research for: SaltStack fleet management (Windows pods + WSL2 master) — RaceControl v6.0*
*Researched: 2026-03-17*
