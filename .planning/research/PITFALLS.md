# Pitfalls Research

**Domain:** Adding connectivity reliability, health monitoring, auto-failover, config sync, and failback to an active venue management system (Racing Point v10.0)
**Researched:** 2026-03-20
**Confidence:** HIGH — pitfalls derived from documented past failures on this exact hardware (WinRM, OpenSSH, Salt, DHCP drift history), operational constraints (8 pods with live customer sessions, consumer router, Windows 11), and verified research on Tailscale SSH Windows status, split-brain patterns, and billing consistency requirements.

---

## Critical Pitfalls

### Pitfall 1: Tailscale SSH Server Does Not Run on Windows

**What goes wrong:**
The plan includes "remote exec from James to Server (.23) via Tailscale SSH." Tailscale SSH *server* (the component that accepts incoming SSH connections and authenticates via Tailscale) is NOT supported on Windows as of 2026. The feature request tracking Windows SSH server support is GitHub issue #4697 / #14942 on the Tailscale repo, and Tailscale has confirmed it is implemented in Go as part of tailscaled — not via OpenSSH — so the corrupted Windows OpenSSH component store on the server (.23) is irrelevant. Attempting to enable `tailscale ssh` on the Windows server will silently not work — the flag may be accepted but no SSH listener opens.

**Why it happens:**
Tailscale SSH server is available on Linux and macOS only. Windows machines can be SSH *clients* through Tailscale (connecting out to Linux nodes) but cannot accept incoming Tailscale SSH connections. The distinction between "Tailscale the client" (works on Windows) and "Tailscale SSH server" (Linux/macOS only) is not prominent in the docs until you look at the platform-specific notes.

**How to avoid:**
The correct architecture for remote exec to a Windows server over Tailscale is: Tailscale provides the encrypted tunnel (IP reachability), then an application-layer exec mechanism runs over that tunnel. Three viable options in order of preference:
1. **rc-agent remote_ops :8090** — already deployed, already authenticated, works over any IP including Tailscale IP. Use the server's Tailscale IP as the target.
2. **RustDesk over Tailscale** — RustDesk direct IP access via Tailscale IP provides GUI remote control when needed. No SSH server required.
3. **OpenSSH via DISM repair** — If the component store on the server (.23) can be repaired (`DISM /Online /Cleanup-Image /RestoreHealth` then `Add-WindowsCapability`), install OpenSSH Server and configure it to listen on the Tailscale IP only. This is the fallback for interactive shell access.

Do NOT plan phase timelines assuming Tailscale SSH server works on Windows. Budget time for one of the above alternatives.

**Warning signs:**
- `tailscale ssh` on the server returns a connection attempt that never completes
- Tailscale admin console shows server online but SSH column is blank/unavailable
- Phase plan references "enable SSH on server" without specifying the mechanism

**Phase to address:** Phase 1 (DHCP + Remote Exec) — verify the exec mechanism before designing any health monitoring or failover that depends on it.

---

### Pitfall 2: DHCP Reservation Fails Because MAC Address Changed (Server Already Did This)

**What goes wrong:**
A DHCP reservation is set on the router for the server (.23) using the MAC address recorded in MEMORY.md. The server gets a different IP anyway. This has already happened: the server's MAC changed from `BC:FC:E7:2C:F2:CE` (old Marvell NIC) to `10:FF:E0:80:B1:A7` (new Gigabyte Z870 onboard NIC) on 2026-03-17, and the DHCP reservation is still listed under the OLD MAC. The server will never receive the reserved IP until the reservation is updated to the new MAC. Additionally, consumer routers (including Xiaomi/TP-Link variants common in Indian venues) have documented bugs where DHCP reservations are ignored for active leases that were assigned before the reservation was created — the old lease must expire before the reservation takes effect.

**Why it happens:**
DHCP reservation works by matching the MAC address in the DHCP DISCOVER packet. A reservation for the wrong MAC is the same as no reservation. Additionally, Windows 11 has MAC address randomization enabled by default for Wi-Fi, though Ethernet typically uses the physical MAC. The server uses Ethernet, but any future NIC replacement or driver change can change the reported MAC.

**How to avoid:**
1. Before creating/updating the DHCP reservation, verify the current MAC on the server: run `getmac /v /fo list` or check `ipconfig /all` on the server, not from router ARP tables (ARP can be stale).
2. Create the reservation in the router using the current MAC (`10:FF:E0:80:B1:A7` as of 2026-03-17).
3. Force the old lease to expire: either wait for nightly lease expiry (~01:05 per MEMORY.md) or run `ipconfig /release && ipconfig /renew` on the server after the reservation is saved.
4. As a belt-and-suspenders measure, also configure a static IP directly on the server's NIC (same .23 address) alongside the DHCP reservation — the static assignment wins if DHCP fails.
5. Document the MAC in MEMORY.md with a "verify before DHCP config" note so any future NIC change triggers an update.

**Warning signs:**
- Router DHCP client list shows the server's current MAC is different from the reserved MAC
- Server gets IP from DHCP pool (not the reserved .23) after reboot
- `arp -a` on James workstation shows .23 with a MAC that doesn't match the reservation

**Phase to address:** Phase 1 (DHCP Fix) — MAC verification must be the first step before touching the router.

---

### Pitfall 3: Split-Brain During Failover — Pods Take Billing Actions on Both Servers Simultaneously

**What goes wrong:**
A pod's rc-agent reconnects to the cloud server (Bono's VPS) while the local server (.23) is believed to be down. The local server was not actually down — it was unreachable from James (.27) due to a network path issue between .27 and .23, but pods on the .31 subnet could still reach .23 directly. Now both servers believe they are authoritative for the same sessions. Billing timers run on both. A session is ended on the cloud server (customer paid) while the local server still shows it as active and charges more credits. When .23 comes back into sync, `cloud_sync.rs` resolves by phone/email lookup — but neither server knows which billing record is correct. Credits are double-charged or incorrectly refunded.

**Why it happens:**
James (.27) is the health monitor — it probes .23 and declares it down. But James's path to .23 may differ from the pods' path. On a flat LAN (192.168.31.x/24), this is rare but possible: a switch port failure, a cable issue on the uplink, or a temporary ARP storm can isolate .27 from .23 while pods remain connected. The auto-failover mechanism reads James's health verdict as authoritative and commands all pods to switch, even though .23 is still reachable by the pods.

**How to avoid:**
1. **Failover verdict must be multi-probe:** Do not trigger failover based on James's health check alone. Require at least one pod's own connectivity report to confirm .23 is unreachable from the LAN before declaring a failover. A pod that can still reach .23 is a counter-signal that overrides James's verdict.
2. **Session lease before failover:** Before switching a pod to cloud, the pod must successfully revoke its session lease from .23 (or confirm .23 is truly unreachable via direct pod probe). If the revocation succeeds, .23 already knows the session moved.
3. **Cloud server is read-only for billing until local confirmed down:** Cloud server should not start new billing ticks for sessions that originated on local server until it has received a session transfer (not just a reconnect).
4. **Failback reconciliation is mandatory:** When .23 recovers, perform a full billing reconciliation before resuming local-authoritative mode — compare session end times and credits charged on both servers, flag any mismatch for manual review.

**Warning signs:**
- Failover is triggered when `ping 192.168.31.23` from .27 fails but pod WebSocket to .23 is still connected
- Cloud server accepts billing END events for sessions it received as NEW (transfer) vs sessions it originated
- After failback, `cloud_sync.rs` shows UUID mismatch warnings for sessions that were active during failover

**Phase to address:** Phase 3 (Auto-Failover) — failover condition must be defined as "pods cannot reach .23" not "James cannot reach .23."

---

### Pitfall 4: False Positive Health Checks Causing Unnecessary Failovers During Normal Events

**What goes wrong:**
The health monitor polls `http://192.168.31.23:8080/health` every 10 seconds. A session of AC launches on Pod 4 — the server is CPU-busy for 3-4 seconds while processing the launch. The `/health` endpoint times out once. The health monitor records one failure, and since the failover threshold is set low (2 consecutive failures = failover), it triggers. All 8 pods disconnect from .23 and attempt to reconnect to Bono's VPS. Game sessions are interrupted. Customers complain. The server was never actually down.

**Why it happens:**
Normal Racing Point operations include events that create brief server unavailability: game launch (AC process spawn is CPU-intensive), Windows Update (server may restart at 2 AM), racecontrol restart (20-30 second gap between binary swap and port bind), and the existing WS_DEAD_SECS 300-second window (per commit 8a026da) that already knows slow boots are normal. A simple consecutive-failure threshold without duration awareness treats momentary load spikes as outages.

**How to avoid:**
1. **Minimum outage duration:** Do not trigger failover until the server has been unreachable for at least 60 seconds continuously — not just 2 consecutive checks.
2. **Multi-probe with jitter:** Use 3 independent probes (HTTP /health, TCP port check :8080, ICMP ping) and require all three to fail simultaneously before counting a failure.
3. **Time-gated suppression:** Suppress failover during the 01:00-03:00 AM window when Windows Update reboots are expected, and during the first 5 minutes after racecontrol restarts.
4. **Consult existing thresholds:** The codebase already uses `WS_DEAD_SECS = 300` (5-minute tolerance for slow boot reconnect) — health monitor thresholds must be calibrated to be at least as tolerant.
5. **Alert before acting:** Send a WhatsApp alert to Uday when health drops to "warning" state. Only trigger automatic failover if warning persists for 60+ seconds AND Uday has not acknowledged/cancelled.

**Warning signs:**
- Health check failures correlate with game launch events on pods (timestamps match)
- Failover happens at 2-3 AM during Windows Update window
- Failover triggers multiple times per week despite the server being "fine" operationally
- Health check timeout is shorter than the server's normal racecontrol restart time

**Phase to address:** Phase 2 (Health Monitoring) — define and document the normal-event baseline before writing any threshold logic.

---

### Pitfall 5: rc-agent Dual-Connect — Connecting to Both Local and Cloud Server Simultaneously

**What goes wrong:**
During failover, an rc-agent reconnects to Bono's VPS. During failback, the rc-agent detects .23 is back and opens a new WebSocket connection to .23 while the cloud connection is still alive (the old connection hasn't timed out yet). Now the agent has two active WebSocket connections. Both servers send commands. The rc-agent processes commands from both, leading to double-execution: a billing END command fires twice, the lock screen engages twice, or a game launch is attempted on an already-running session. The cloud server's WS_DEAD_SECS timer hasn't triggered yet so it still sends commands, and the local server's reconnection starts sending commands too.

**Why it happens:**
WebSocket reconnect logic opens a new connection before explicitly closing the old one — the close and connect are not atomic. In a failover/failback scenario with two candidate servers, the rc-agent's connection state machine must prevent simultaneous connections. The existing auto-reconnect with backoff (CONN-03) was designed for single-server reconnect, not multi-server switching.

**How to avoid:**
1. **Explicit disconnect-before-connect:** Before opening a WebSocket to a new server, explicitly close and await confirmation of close on the current connection. Add a `DISCONNECTING` state to the connection state machine.
2. **Server authority token:** When rc-agent connects to a server, the server returns an authority token that includes the server ID. rc-agent refuses commands from any server whose authority token it does not currently hold. Only one server can hold authority at a time.
3. **Failback handshake:** Failback to .23 requires .23 to request authority transfer from Bono's VPS (via Tailscale), which revokes the cloud server's authority token before the rc-agent reconnects locally.
4. **Connection state logging:** Log every connection state transition with timestamps and server identity — dual-connect is detectable in logs before it causes damage.

**Warning signs:**
- rc-agent log shows two active WebSocket connection IDs simultaneously
- Billing END events appear twice in the racecontrol database for the same session
- Lock screen engages unexpectedly on a pod that was in the middle of a session

**Phase to address:** Phase 3 (Auto-Failover) and Phase 4 (Failback) — connection state machine changes must precede any failover testing.

---

### Pitfall 6: Config Sync Overwrites Local Customizations Made During an Outage

**What goes wrong:**
During a 2-hour local server outage, Uday asks Bono to update billing rates on the cloud server — a customer negotiated a group discount. Bono edits the `billing_rates` table on Bono's VPS. When .23 recovers, the config sync runs. The sync is designed "cloud authoritative for pricing" — so the cloud pricing overwrites .23's local pricing. But between the outage and the sync completing, a staff member had also manually edited `racecontrol.toml` on .23 to temporarily set a different rate while the cloud was unreachable. That local change is silently overwritten. No one notices for two days until billing discrepancies surface.

**Why it happens:**
The existing cloud_sync.rs is designed with "cloud authoritative: drivers, pricing" and "local authoritative: billing, laps, game state." This works in steady-state but creates a conflict window when both sides are written during an outage. The sync does not track which side was written more recently — it applies the authority rule unconditionally, not the "newer wins" rule.

**How to avoid:**
1. **Timestamp-based config sync:** Add a `config_updated_at` timestamp to any config record that can be modified on both sides. On sync, apply "last-write-wins" within the authoritative domain, not just "cloud wins always."
2. **Outage detection in sync:** When .23 comes back online, compare `config_updated_at` for any records modified on .23 during the outage window with records modified on cloud during the same window. Surface conflicts to Uday before applying.
3. **Narrow the cloud-authoritative scope:** Pricing is cloud-authoritative for PWA display, but operational overrides made during an outage should be treated as local-authoritative until explicitly synced. Add an `override_until` timestamp or `local_override` flag.
4. **Config sync audit log:** Every sync event should write a log entry detailing what was overwritten. This makes conflicts detectable after the fact.

**Warning signs:**
- Billing rates change unexpectedly after .23 comes back online following an outage
- `cloud_sync.rs` log shows "applied cloud config" without a conflict check step
- Staff report that rates they set during an outage were "undone by the system"

**Phase to address:** Phase 2 (Config Sync) — conflict resolution policy must be defined before implementation, not after.

---

### Pitfall 7: Failback Resumes Sessions That Were Already Ended on Cloud

**What goes wrong:**
Pod 3 has an active session during the failover. The session is billed and ended on Bono's VPS after 45 minutes. The customer pays and leaves. When .23 recovers and failback occurs, the rc-agent reconnects to .23. .23's `cloud_sync.rs` pulls session data from the cloud, but the sync runs asynchronously and hasn't completed yet. .23's in-memory state still shows Pod 3 as "active" (it never received the END event while it was down). rc-agent reconnects and .23 thinks the pod needs re-authentication. The lock screen engages on Pod 3. A new customer is already seated and trying to start a session — their experience is broken.

**Why it happens:**
.23 was offline during the session's lifecycle. Its in-memory state is stale. Failback reestablishes the WebSocket without first reconciling pod state. The `cloud_sync.rs` pull-on-startup pattern handles this for the database tables but not for in-memory `AppState` (pod sessions, game state). The server needs to reconcile in-memory state from the sync data before allowing rc-agents to reconnect.

**How to avoid:**
1. **Sync-before-accept:** When .23 comes back online, do not accept rc-agent connections until `cloud_sync.rs` has completed a full pull from Bono's VPS. Add a startup health gate: `racecontrol` only opens its WebSocket listener after sync confirms its state is current.
2. **Session state reconciliation:** On first rc-agent reconnect after failback, the server must query the pod's current state (running game? lock screen?) and reconcile with the synced database state, not just the stale in-memory state.
3. **Graceful stale-state handling:** If sync shows a session was ended on cloud while .23 was down, the in-memory state for that pod should be set to `Idle`/`Ready` immediately, not `Active`.
4. **Failback notification to Bono:** When failback completes, Bono should notify .23 of all sessions that started, changed state, or ended during the outage — push a reconciliation payload, don't wait for the passive sync cycle.

**Warning signs:**
- Pod lock screen engages immediately after failback on a pod where the customer already finished
- .23 logs show `session_id` already present in the `sessions` table (duplicate on sync)
- Customers report being asked to authenticate again on a pod they just completed

**Phase to address:** Phase 4 (Failback) — sync-before-accept gate must be implemented before any failback testing with active sessions.

---

### Pitfall 8: OpenSSH Component Store Repair Fails on Server — No Fallback Planned

**What goes wrong:**
The v10.0 plan includes remote exec via Tailscale to the server. Tailscale SSH server doesn't work on Windows (see Pitfall 1). The fallback plan is to repair OpenSSH via `DISM /Online /Cleanup-Image /RestoreHealth` on the server. This repair requires internet access on the server and can take 30-60 minutes. On an isolated network (server behind NAT, no direct internet) or with Windows Update policies that redirect to WSUS, `DISM /RestoreHealth` fails with error `0x800f0954` ("The source files could not be found"). The repair hangs or errors out. Now there is no remote exec mechanism and the original rc-agent remote_ops on port 8090 is the only option.

**Why it happens:**
`Add-WindowsCapability` for OpenSSH requires downloading the capability package from Windows Update servers. If WSUS is configured, or if the server's Windows Update source is blocked, the download fails. The server at Racing Point has had component store issues before — the fact that OpenSSH install already failed once indicates a pre-existing component store problem that DISM may not be able to repair without a Windows installation media source.

**How to avoid:**
1. **Plan for OpenSSH failure:** The primary remote exec path must be rc-agent remote_ops :8090, which already works. OpenSSH is a nice-to-have for interactive shell, not a dependency for automated exec.
2. **Offline repair option:** Download the OpenSSH capability package (.cab file) from Microsoft Update Catalog before attempting repair — this allows offline installation (`Add-WindowsCapability -Source <local-path>`).
3. **Alternative: SSH via pendrive install:** Deploy a standalone OpenSSH MSI (not the Windows capability feature) to the server via pendrive. The MSI-based install (`OpenSSH-Win64.msi` from the PowerShell/openssh-portable releases) bypasses the component store entirely.
4. **Test first:** Before designing any phase around OpenSSH on the server, run the DISM repair and capability install once to confirm it succeeds. Commit this as a prerequisite verification step, not an assumption.

**Warning signs:**
- `Add-WindowsCapability` returns `0x800f0954` (source not found) or `0x800f081f` (store corruption)
- `DISM /Cleanup-Image /RestoreHealth` takes more than 10 minutes or returns errors
- The CBS.log (`C:\Windows\Logs\CBS\CBS.log`) shows "Package_for_OpenSSH" with a failed state

**Phase to address:** Phase 1 (Remote Exec Setup) — add "verify OpenSSH install or confirm rc-agent :8090 is sufficient" as the first acceptance criterion.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using James's health check as sole failover trigger | Simple, single point of decision | Split-brain when James's path to server differs from pods' path (switch fault, cable issue) | Never — require pod-side confirmation |
| Setting failover threshold to 2 consecutive failures | Fast failover response | False positives during normal game launches, server restarts; unnecessary pod disruption | Never — minimum 60s continuous outage |
| Cloud-authoritative config sync without timestamp tracking | Simple sync logic | Silent overwrite of local changes made during outage | Never for billing rates — add last-write-wins per record |
| Failback without sync-before-accept | Fast reconnect | Stale in-memory session state causes double billing or phantom lock screens | Never — always sync before accepting rc-agent connections |
| Skipping the DHCP + static IP belt-and-suspenders approach | Slightly simpler setup | DHCP reservation alone can fail (wrong MAC, lease timing, router bug) | Never for the server — use both static NIC config AND DHCP reservation |
| Trusting Tailscale SSH will work on Windows without testing | Saves time in planning | Phase blocked at implementation when SSH server doesn't open | Never — test on the actual server before planning timeline |
| Single-path failover notification (email only) | Simple alerting | Email delivery may fail if internet is down during the same outage that caused the failover | Never — always send WhatsApp (LAN-accessible Evolution API) as primary + email as secondary |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Tailscale on Windows server | Assume Tailscale SSH server is available | Tailscale SSH server is Linux/macOS only; use Tailscale for IP reachability only, route exec via rc-agent :8090 over the Tailscale IP |
| DHCP reservation on consumer router | Set reservation without verifying current MAC | Run `ipconfig /all` on the server; compare to router ARP table; consumer routers ignore reservations for active leases — force lease renewal after setting reservation |
| cloud_sync.rs on failback | Trust the 30s passive sync cycle to reconcile outage state | Push a dedicated reconciliation payload from Bono to .23 on failback; do not rely on passive polling to reconstruct outage state in time |
| Windows NIC MAC addresses | Assume MAC is stable | Windows 11 Wi-Fi uses random MACs by default; future hardware changes (NIC replacement, driver update) can change Ethernet MAC; pin MAC in MEMORY.md and verify before any DHCP config |
| Health monitoring HTTP endpoint | Use a fast timeout (1-2s) | Server is busy during game launches — use a 5-10s timeout minimum; a 3-4 second load spike is not an outage |
| rc-agent failover switch | Switch to cloud server immediately on first health failure | rc-agent should try a direct LAN probe to .23 before accepting James's health verdict; a routing issue on .27 should not trigger a fleet-wide failover |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Health check polling every 5s from James against the server | Server's Axum worker pool sees health check requests competing with pod WebSocket frames; brief latency spikes under load | Poll every 30s; use a dedicated lightweight `/ping` endpoint that returns 200 immediately without touching DB | During peak hours with 8 active sessions + health check flood |
| Failover sending config sync pull immediately after reconnect to cloud | Cloud VPS (512MB-1GB RAM environment) gets a flood of sync requests from all 8 pods reconnecting simultaneously | Stagger reconnect backoff per pod (pod number * 2s delay); sync only what changed since last sync, not full pull | Immediately on failover with all 8 pods reconnecting at once |
| Billing reconciliation running in the request handler on failback | HTTP timeout during reconciliation causes racecontrol to appear hung | Run reconciliation as a background task; block rc-agent connections with a "syncing" response code until complete | On failback with 3+ sessions that had activity during the outage |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Remote exec endpoint accessible on public Tailscale IP without auth | Anyone with a Tailscale account added to the network can run arbitrary commands on the server | rc-agent remote_ops :8090 requires its existing auth token; verify the token check is not bypassed when the source IP is a Tailscale address |
| Failover logic hardcodes Bono's VPS IP (72.60.101.58) in rc-agent config | VPS IP change breaks all failover; IP in source code creates a git-tracked configuration that must be updated on all 8 pods | Store cloud WS URL in `rc-agent.toml` as `cloud_ws_url`; never hardcode in source |
| Config sync sends full `racecontrol.toml` to cloud (including JWT secret, API keys) | Secrets exposed on Bono's VPS, potentially in logs or sync payloads | Config sync must use a separate `billing_rates` API endpoint, not file-level TOML sync; never sync the full config file |
| Tailscale ACL misconfiguration allowing pods to reach Bono's VPS directly | Pods could attempt to resolve health check or exec against cloud directly, creating unexpected traffic during normal operation | Tailscale ACL: only James (.27) and server (.23) have egress to Bono's VPS Tailscale node; pods reach cloud only through the server |

---

## "Looks Done But Isn't" Checklist

- [ ] **DHCP reservation active:** Verify by rebooting the server and confirming it gets .23 — not just that the reservation appears in the router UI (reservations for wrong MACs are silently ignored)
- [ ] **Remote exec working:** Verify by running a real command on the server from James via Tailscale IP — not just that Tailscale shows the server as "connected"
- [ ] **Health monitor not false-positive:** Verify by launching a game on a pod and confirming the health monitor does not trigger a failover event during the launch
- [ ] **Failover completes without billing loss:** Verify by starting a session on a pod, triggering failover, ending the session on cloud, then failback — confirm the session record appears correctly on .23 and credits are charged exactly once
- [ ] **Failback sync-before-accept working:** Verify by checking that rc-agents cannot reconnect to .23 until the sync pull from Bono's VPS has completed (the port should refuse connections for up to 30s after .23 restarts)
- [ ] **Split-brain prevention active:** Verify by disconnecting James's cable from the switch (James cannot reach .23) while leaving pods connected — confirm pods do NOT failover because they can still reach .23 directly
- [ ] **Config sync conflict detection:** Verify by writing a billing rate on .23 during a simulated outage, then syncing from cloud — confirm the conflict is flagged, not silently overwritten

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Tailscale SSH not working on server | LOW | Switch to rc-agent remote_ops :8090 over Tailscale IP; document Tailscale SSH as "not available on Windows" in PROJECT.md |
| DHCP reservation not taking effect | LOW | Force `ipconfig /release && ipconfig /renew` on server; if reservation still wrong MAC, update router; fallback: configure static IP on server NIC directly |
| Split-brain detected (both servers billing same session) | HIGH | Stop cloud server billing immediately; reconcile credits manually; apply "later end time wins" for session duration; notify Uday with summary |
| False positive failover during business hours | MEDIUM | Reconnect all pods to .23 manually via rc-agent remote_ops; review health check thresholds; add minimum-duration gate before re-enabling auto-failover |
| Failback leaves pods in stale session state | MEDIUM | For each pod: check rc-agent state vs .23 DB; if DB shows session ended but rc-agent shows active, send force-idle command via remote_ops; re-engage lock screen |
| Config sync overwrites billing rates | MEDIUM | Restore backup billing_rates row (racecontrol.db is SQLite — `sqlite3 racecontrol.db ".dump billing_rates"` from pre-sync backup); re-sync from correct source |
| Dual-connect (rc-agent on two servers) | HIGH | Kill rc-agent on affected pod via remote_ops; reconnect manually to correct server; audit session state on both servers before restarting |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Tailscale SSH not working on Windows | Phase 1: Remote Exec Setup | Test `tailscale ssh` or confirm rc-agent :8090 is the exec path before any phase design |
| DHCP reservation wrong MAC | Phase 1: DHCP Fix | Reboot server and confirm .23 is assigned before marking phase done |
| Split-brain dual billing | Phase 3: Auto-Failover | Disconnect James from switch, confirm pods do not failover; test with active session |
| False positive health check failover | Phase 2: Health Monitoring | Launch a game during health check observation; confirm zero failover events in 24h |
| Dual-connect on failback | Phase 3+4: Failover/Failback | Check rc-agent logs for simultaneous connection IDs; verify authority token revocation |
| Config sync overwrites outage changes | Phase 2: Config Sync | Simulate outage, write config locally, sync — confirm conflict detection fires |
| Failback stale in-memory session state | Phase 4: Failback | End session on cloud during simulated outage; failback; confirm pod is Idle not Active |
| OpenSSH component store broken | Phase 1: Remote Exec Setup | Attempt OpenSSH install before assuming it's available; have rc-agent :8090 as fallback |

---

## Sources

- MEMORY.md — Documents server MAC change (2026-03-17), DHCP drift history (.51→.23→.4→.23), OpenSSH component store corruption, WinRM/Salt/SSH failure history, existing cloud_sync.rs authority model
- PROJECT.md — v10.0 requirements list, remote deploy scrapped approaches, cloud_sync authoritative domains
- GitHub issue tailscale/tailscale #14942 and #4697 — Tailscale SSH server not supported on Windows (confirmed by Tailscale team)
- TP-Link Community forums — DHCP reservation bugs on consumer routers, reservation ignored for active leases, MAC binding conflicts
- AWS Builders Library "Implementing Health Checks" — consecutive failure thresholds, false positive prevention
- Nagios documentation — flapping detection, state change percentage thresholds
- WebSocket.org reconnection guide — state sync on failover, race conditions during reconnect
- Microsoft Learn — `Add-WindowsCapability` failure 0x800f0954 (source not found), DISM repair limitations, component store corruption

---
*Pitfalls research for: Connectivity reliability, health monitoring, auto-failover, config sync, and failback — Racing Point v10.0*
*Researched: 2026-03-20*
