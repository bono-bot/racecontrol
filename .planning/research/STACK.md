# Stack Research

**Domain:** Connectivity & Redundancy — DHCP stability, Tailscale-based remote exec, health monitoring, config sync, and auto-failover for a Rust/Axum sim racing venue management system
**Researched:** 2026-03-20 IST
**Confidence:** HIGH (official Tailscale docs, Microsoft Learn, crates.io, and existing codebase verified)
**Milestone:** v10.0 Connectivity & Redundancy

---

> **Scope:** v10.0 ONLY. Existing stack (Rust/Axum, Next.js, SQLite, Tailscale mesh on all nodes,
> bono_relay.rs, cloud_sync.rs, reqwest, tokio, axum 0.8) is NOT re-researched here.
> Focus: what gets ADDED or CHANGED for DHCP stability, remote exec, health monitoring,
> config sync to cloud, auto-failover, failback, and failover notifications.

---

## Problem Space Summary

v10.0 has seven distinct technical problems, each with a different solution domain:

| Problem | Current State | Target State |
|---------|--------------|--------------|
| Server .23 DHCP drift | IP drifts nightly; MAC changed 2026-03-17 | Permanent IP lock — DHCP reservation or static assignment |
| Remote exec from James to server | No working method (WinRM/Salt/OpenSSH all scrapped) | `tailscale ssh` to server's OpenSSH via Tailscale mesh |
| Server health monitoring | None — James discovers outages manually | James's racecontrol crate polls server health continuously |
| Config sync (racecontrol.toml → cloud) | Only DB tables synced (cloud_sync.rs); TOML file not sent | Config pushed to Bono VPS on change so cloud can mirror venue settings |
| Auto-failover | Pods connect only to local server (.23); if .23 dies, all pods go idle | Pods detect server absence and switch WS URL to Bono's VPS |
| Failover notifications | Only email alerts for pod issues | Email + WhatsApp to Uday when failover fires |
| Failback | No concept of primary/secondary | Pods return to .23 when it recovers, within 30–60s |

---

## Recommended Stack

### 1. DHCP Stability — Server .23 IP Lock

**Approach A (preferred): DHCP reservation on TP-Link router .1**

No new code. Router web UI → DHCP → Address Reservation → add MAC `10-FF-E0-80-B1-A7` → assign `192.168.31.23`. Survives reboots. No Windows components needed.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| TP-Link router DHCP reservation (web UI) | n/a | Pin server .23 to MAC `10-FF-E0-80-B1-A7` | Zero-code, router survives independently of Windows. Lease is permanent (not nightly expiry). TP-Link supports reservations outside the dynamic pool range — confirmed in TP-Link FAQ. |

**Approach B (fallback): Static IP on server Windows NIC**

Use PowerShell `New-NetIPAddress` if TP-Link reservation is unavailable or the MAC drifts again. Run on server as ADMIN via web terminal or pendrive session.

```powershell
# Run on server (.23) as ADMIN
$iface = Get-NetAdapter | Where-Object { $_.MacAddress -eq "10-FF-E0-80-B1-A7" } | Select-Object -ExpandProperty Name
Remove-NetIPAddress -InterfaceAlias $iface -Confirm:$false -ErrorAction SilentlyContinue
New-NetIPAddress -InterfaceAlias $iface -IPAddress 192.168.31.23 -PrefixLength 24 -DefaultGateway 192.168.31.1
Set-DnsClientServerAddress -InterfaceAlias $iface -ServerAddresses 192.168.31.1
```

**No new Rust dependencies needed for this problem.** Pure infrastructure config.

---

### 2. Remote Exec: James (.27) → Server (.23) via Tailscale

**Key constraint:** Tailscale SSH server does NOT support Windows (confirmed open GitHub issue #14942 as of early 2026 — Tailscale SSH is a custom Go implementation, not wrapped OpenSSH). The workaround is: enable Windows' native OpenSSH Server on the server, then route the SSH connection through Tailscale's network layer.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| OpenSSH Server (Windows built-in) | Windows 11 built-in (OpenSSH 9.x, ships with Windows 11) | SSH access to server .23 | Native Windows component — no download needed. Works with standard `ssh` client and `scp`. Tailscale provides the encrypted mesh routing; OpenSSH provides the shell. `Add-WindowsCapability` failed on server previously (corrupted component store per MEMORY.md), but a fresh install via `winget install Microsoft.OpenSSH.Beta` or an MSI may work. Must verify. |
| Tailscale mesh (existing) | existing on all nodes | Encrypted routing for SSH on Tailscale IP (100.x.x.x) | Already installed. No new Tailscale config needed — just open port 22 on the server's Windows Firewall for Tailscale interface only. |
| `tokio::process::Command` (existing dep) | tokio 1.x (existing) | Execute SSH commands programmatically from James's racecontrol monitor process | Already available in the codebase. Use `Command::new("ssh").args([...]).output().await` inside the health monitor to run one-shot admin commands on .23. No new crate dependency. |

**OpenSSH install on server (alternative to Add-WindowsCapability):**

```powershell
# Option 1: winget (avoids corrupted component store)
winget install Microsoft.OpenSSH.Beta

# Option 2: Download MSI directly from GitHub releases
# https://github.com/PowerShell/Win32-OpenSSH/releases

# Once installed:
Set-Service sshd -StartupType Automatic
Start-Service sshd
# Allow key auth for James (.27 key):
# Add James's pub key to C:\Users\ADMIN\.ssh\authorized_keys on server
New-NetFirewallRule -Name "OpenSSH-Server-Tailscale" -DisplayName "OpenSSH via Tailscale" `
  -Protocol TCP -LocalPort 22 -Action Allow -InterfaceAlias "Tailscale"
```

**What remote exec unlocks:** James's health monitor can run `ssh ADMIN@100.71.226.83 "restart-service racecontrol"` when server health probes fail. This is the missing piece for autonomous recovery vs. just alerting.

**Note on rc-agent remote_ops (:8090):** Pods already have HTTP remote exec at port 8090. This is sufficient for pod management. SSH on server is specifically for the server-side recovery path. Do not conflate the two.

---

### 3. Health Monitoring — James Watches Server .23

**Architecture:** A new background task `server_monitor::spawn()` runs inside the existing racecontrol binary on James's machine (.27). The monitor uses the existing `reqwest` HTTP client to probe server .23's health endpoint on a 10-second interval, with hysteresis to avoid flapping alerts.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `reqwest` 0.12 (existing dep) | 0.12.x (existing in racecontrol/Cargo.toml) | HTTP probes to `http://192.168.31.23:8080/health` every 10 seconds | Zero new dependencies. The existing `state.http_client` (`reqwest::Client`) is already configured with connection pooling and timeout. Reuse it directly. |
| `tokio::time::interval` (existing dep) | tokio 1.x (existing) | Drive probe loop with configurable interval | Standard async interval pattern — already used in cloud_sync.rs and pod_monitor.rs. |
| Hysteresis counter (plain Rust) | n/a | Require N consecutive failures before declaring server down | Prevents alert storms from transient 1-second glitches. Pattern already in cloud_sync.rs (`RELAY_DOWN_THRESHOLD = 3`, `RELAY_UP_THRESHOLD = 2`). Copy the same approach. |
| `AtomicBool` in AppState (existing pattern) | std | Share server-up/down state across tasks | Already used for `ws_connected` in rc-agent self_monitor.rs. Add `server_healthy: AtomicBool` to James's AppState. |

**Health probe target on server:** The existing `/health` endpoint (or `/relay/health` from bono_relay.rs) already responds `{"status": "ok"}`. No new server-side endpoint needed for basic liveness.

**For richer health (capacity to detect DB issues, not just process liveness):** Add a `/health/deep` endpoint to racecontrol that checks: HTTP server responding + SQLite readable + at least one pod connected. Returns 200 or 503 with a JSON body. This is a standard pattern documented in the 2026 Rust health check guides.

```rust
// Pattern for server_monitor.rs — no new crates
async fn probe_server(client: &reqwest::Client, url: &str) -> bool {
    client
        .get(url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

---

### 4. Config Sync — Push racecontrol.toml to Bono VPS

**Approach:** Extend the existing `cloud_sync.rs` to also push the racecontrol.toml file contents (or a structured representation of it) to Bono's VPS on startup and whenever the file changes. Use SHA-256 hash comparison to detect changes without polling the filesystem every second.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `sha2` crate | **0.10.8** | Hash racecontrol.toml on startup and at interval; push to Bono only when hash changes | Pure Rust, no-std capable, part of RustCrypto ecosystem. 393M total crate downloads — ecosystem standard. The only new Cargo dependency for this feature. Hash 32 bytes once every 30s — negligible CPU cost. |
| `tokio::time::interval` (existing) | tokio 1.x | Drive 30-second config hash check loop | Same pattern as cloud_sync.rs existing sync loop. |
| `reqwest` (existing) | 0.12.x | POST config payload to Bono's `/relay/config-sync` endpoint | Reuse existing `state.http_client`. Same pattern as bono_relay.rs event push. |

**What gets synced:** The full racecontrol.toml contents (redacted of secrets: strip `relay_secret`, JWT keys, OAuth tokens before transmission) plus a content hash for Bono to detect stale pushes.

**Bono-side endpoint:** Add `POST /relay/config-sync` to Bono's VPS relay router. Bono stores the config payload in memory and uses it when constructing failover responses. This is a small addition to bono_relay.rs.

**Config push payload:**

```rust
#[derive(Serialize)]
struct ConfigSyncPayload {
    venue_name: String,
    pod_count: u32,
    billing_rates: Vec<BillingRate>,   // from DB via existing sync
    config_hash: String,               // SHA-256 of sanitized TOML
    timestamp: String,                 // IST ISO8601
}
```

**Cargo.toml addition:**

```toml
sha2 = "0.10"
```

---

### 5. Auto-Failover — Pods Switch from Local Server to Bono VPS

**This is the most architecturally significant change.** Currently, rc-agent has a single `CoreConfig { url: String }` pointing to `ws://192.168.31.23:8080/ws/agent`. Failover requires rc-agent to maintain two URLs and switch between them.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `tokio::sync::watch` channel | tokio 1.x (existing) | Broadcast failover state changes to the WS reconnect loop in rc-agent | `watch::channel` lets the health probe task write the active URL, and the WS connection loop reads the latest value without polling. Zero new crate — `tokio::sync::watch` is already available. Pattern: one writer (failover_monitor task), multiple readers (WS loop, kiosk, lock_screen HTTP client). |
| Config: `core.failover_url` (new TOML field) | n/a | Store Bono's WS URL in rc-agent TOML per pod | `ws://100.x.x.x:8080/ws/agent` (Bono's Tailscale IP). Simple String field. Pods only use this when local server is unreachable. No library needed. |
| HTTP health probe (reqwest, existing in rc-agent) | 0.12.x (existing) | rc-agent probes `http://192.168.31.23:8080/health` before WS reconnect attempts | rc-agent already has `reqwest` in its `Cargo.toml`. The probe runs every 15s when WS is disconnected. If 3 consecutive probes fail, treat as failover condition. |

**Failover state machine in rc-agent:**

```
CONNECTED_PRIMARY → probe fails 3x → FAILOVER_ACTIVE → WS connects to Bono URL
FAILOVER_ACTIVE → primary probe succeeds 2x → FAILBACK_PENDING → close Bono WS → reconnect primary
```

**Key constraint:** Failover URL (Bono's Tailscale IP `100.x.x.x`) requires Tailscale to be connected on the pod. Tailscale is already installed and running on all 8 pods (Phase 27, v5.0). This is a prerequisite that is already met.

**Config addition to rc-agent-pod{N}.toml:**

```toml
[core]
url = "ws://192.168.31.23:8080/ws/agent"
failover_url = "ws://100.x.x.x:8080/ws/agent"  # Bono's Tailscale IP
health_probe_url = "http://192.168.31.23:8080/health"
failover_probe_failures = 3   # consecutive failures before switching
failback_probe_successes = 2  # consecutive successes before switching back
```

**Important:** Bono's VPS must run a compatible version of racecontrol with the same WebSocket protocol. The existing ws/agent endpoint (rc-common protocol, AgentMessage enum) is already on both local and cloud. No protocol changes needed.

---

### 6. Failover Notifications — Alert Uday

**Extend the existing email alert system.** No new mechanism needed.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `send_email.js` shell-out (existing) | existing | Email Uday when failover fires | Already used by pod_monitor.rs and watchdog.rs for email alerts. Reuse exactly the same pattern: `tokio::process::Command::new("node").args(["send_email.js", ...])`. No new dependencies. |
| Evolution API (WhatsApp, existing in auth.rs) | existing | WhatsApp message to Uday on failover/failback | Already configured in `AuthConfig` with `evolution_url`, `evolution_api_key`, `evolution_instance`. The same `reqwest` POST pattern used for OTP delivery can be reused for failover alerts. Extract a `send_whatsapp_alert()` function from the existing OTP code path. |

**Alert content:**

- Failover fired: `"[ALERT] Racing Point: Server .23 unreachable. Pods switched to cloud VPS. Time: {IST}"`
- Failback: `"[RESOLVED] Racing Point: Server .23 recovered. Pods returned to local. Time: {IST}"`

**Rate limiting:** Use the existing `WatchdogConfig.email_venue_cooldown_secs` pattern — don't alert more than once per 5 minutes for the same event.

---

### 7. Bono VPS — Failover Reception

Bono's VPS (72.60.101.58) already runs racecontrol with cloud_sync and bono_relay. For failover to work, Bono needs to:

1. Accept ws/agent connections from pods (already possible — same Axum WS handler)
2. Receive config sync pushes from the venue server (new `/relay/config-sync` endpoint)
3. Route BonoEvents back to James for awareness of pod state during failover (already done via bono_relay.rs event push)

**No new dependencies on Bono's side.** All changes are route additions to existing bono_relay.rs and small AppState extensions.

---

## New Cargo Dependencies (Minimal)

The constraint "no new language runtimes" is satisfied — everything stays Rust + existing Node.js.

| Crate | Version | Added To | Purpose |
|-------|---------|----------|---------|
| `sha2` | `0.10.8` | racecontrol/Cargo.toml | Config file hash for change detection before pushing to Bono. The only net-new crate for all of v10.0. |

**All other capabilities use existing crates:**

| Capability | Uses Existing Crate |
|------------|---------------------|
| Health probes | `reqwest` 0.12 (already in both racecontrol and rc-agent) |
| Probe intervals | `tokio::time::interval` (existing) |
| Failover state broadcast | `tokio::sync::watch` (existing, part of tokio 1.x) |
| SSH exec from James | `tokio::process::Command` (existing) |
| Failover alerts (email) | `send_email.js` shell-out pattern (existing) |
| Failover alerts (WhatsApp) | `reqwest` + Evolution API pattern (existing in auth.rs) |
| Config serialization | `serde_json` / `toml` (existing) |
| Hysteresis counters | plain Rust `u32` counters (no crate) |

---

## New Modules to Create

| Module | Location | Purpose |
|--------|----------|---------|
| `server_monitor.rs` | `crates/racecontrol/src/` | Health probe loop for server .23 (runs in James's monitor instance). Manages `AtomicBool server_healthy`, drives email/WA alerts, triggers SSH recovery. |
| `failover_monitor.rs` | `crates/rc-agent/src/` | Health probe loop for local server from each pod. Manages failover state machine, switches `active_server_url` via `watch::Sender`. |
| `config_pusher.rs` | `crates/racecontrol/src/` | Reads racecontrol.toml, hashes it (sha2), POSTs sanitized config to Bono on startup and on change. Runs as background task. |

**Modifications to existing files:**

| File | Change |
|------|--------|
| `crates/rc-agent/src/main.rs` | Add `failover_url`, `health_probe_url`, `failover_probe_failures`, `failback_probe_successes` to `CoreConfig`. Spawn `failover_monitor::spawn()`. Pass `watch::Receiver<String>` (active URL) to WS connect loop instead of static string. |
| `crates/racecontrol/src/bono_relay.rs` | Add `POST /relay/config-sync` endpoint to `build_relay_router()`. Accept and store `ConfigSyncPayload`. |
| `crates/racecontrol/src/config.rs` | Add `HealthMonitorConfig` struct with probe interval, failure threshold, SSH recovery enabled flag. |
| `crates/racecontrol/src/state.rs` | Add `server_healthy: Arc<AtomicBool>` for James's monitor status. |

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| TP-Link DHCP reservation for server IP | Windows static IP (`New-NetIPAddress`) | Static IP loses DHCP server's awareness — won't appear in ARP tables reliably; router sees it as unknown host. DHCP reservation is the right tool. Static IP is the fallback if reservation fails. |
| OpenSSH + Tailscale mesh for remote exec | WinRM over Tailscale | WinRM failed on this network previously (MEMORY.md). OpenSSH uses port 22 + SSH keys — simpler, cross-platform tooling (same as Linux). |
| OpenSSH + Tailscale mesh for remote exec | Tailscale SSH server on Windows | NOT SUPPORTED on Windows as of early 2026 (GitHub issue #14942). Tailscale SSH is a custom Go implementation, not wrapped OpenSSH. No Windows support. |
| `tokio::sync::watch` for failover URL | `tokio::sync::Mutex<String>` | Mutex requires lock acquisition on every reconnect attempt. `watch` is lock-free for readers — correct pattern for a frequently-read, rarely-written value. |
| `sha2` for config change detection | `tokio::fs::metadata` mtime | mtime is unreliable on Windows (DST, NTP adjustments can change mtime without content change). Content hash is authoritative. |
| `sha2` for config change detection | `notify` crate (filesystem watcher) | `notify` adds an extra async channel and OS-specific event handling. For a 30-second poll interval, a simple hash comparison is simpler and sufficient. |
| Evolution API WhatsApp for failover alerts | Twilio / other SMS | Evolution API is already configured and working for OTP delivery. No new provider setup. |
| Single new `/health/deep` endpoint on server | Separate health service | Adding a route to the existing Axum server requires zero new infrastructure. A separate health service would need its own port and process management. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `tailscale-localapi` crate (jtdowney) | Read-only: gets node status and certs, cannot execute commands or check peer health in a useful way. Limited API surface. Not needed. | `tailscale status --json` via `tokio::process::Command` if you need peer info; Tailscale mesh itself for routing. |
| DNS-based failover (split DNS, Consul) | Adds a whole new service (Consul/CoreDNS) to a venue that already struggles with infrastructure complexity. DNS TTL caching (typically 5–300s) also creates unpredictable failover timing. | Direct IP switching in rc-agent — the WS URL is swapped programmatically with zero DNS dependency. |
| WireGuard directly (without Tailscale) | Tailscale already runs on all nodes and works. Raw WireGuard needs key distribution, routing config, and NAT traversal handled manually. No benefit over existing Tailscale mesh. | Tailscale (existing) |
| `reconnecting-websocket` crate | rc-agent already has its own WS reconnect loop with backoff. Adding a wrapper crate changes ownership semantics and makes the failover URL switching harder to integrate. | Existing reconnect loop + `watch::Receiver<String>` for URL switching |
| Docker/containerization on server | Server .23 runs Windows. Docker on Windows requires Hyper-V or WSL2 (latter has BIOS AMD-V issue, v6.0 blocked). Gaming-adjacent infrastructure — container overhead not justified. | Native Windows services (existing pattern) |
| SaltStack (v6.0) | Still blocked — WSL2 portproxy + BIOS AMD-V. The OpenSSH path is simpler and available now. | OpenSSH + `tokio::process::Command` for remote exec |
| `notify` crate for config file watching | Overkill for a config that changes at most a few times per day. Polling every 30s with a SHA-256 hash is simpler and has zero platform-specific behavior. | `sha2` hash comparison in a 30s loop |

---

## Integration with Existing Stack

| Existing Component | Integration Point | Notes |
|--------------------|-------------------|-------|
| `bono_relay.rs` (event push, relay endpoint) | Add `/relay/config-sync` route to `build_relay_router()` | Same auth pattern (X-Relay-Secret header). Minimal change. |
| `cloud_sync.rs` (30s DB sync loop) | `config_pusher.rs` runs independently on the same interval | Do NOT fold into cloud_sync.rs — config push has different semantics (push-only, file-based, hash-guarded). Keep separate. |
| `rc-agent/src/main.rs` `CoreConfig` | Add 3 new optional fields with defaults matching current behavior | `failover_url = None` by default → no behavior change on existing pods until TOML is updated. |
| `pod_monitor.rs` (server-side pod monitoring) | `server_monitor.rs` is a complementary monitor — same concept but James-side | Keep them separate: pod_monitor.rs monitors pods; server_monitor.rs monitors the server itself. |
| Email alerts via `send_email.js` | Failover alerts reuse the same shell-out pattern in `server_monitor.rs` | Same rate limiting via cooldown timestamp as watchdog.rs. |
| WhatsApp OTP in `auth.rs` | Extract `notify_whatsapp()` helper — reuse for failover alerts | Do not duplicate the Evolution API HTTP call logic — extract to a shared function in a new `alerts.rs` module or inline in `server_monitor.rs`. |
| Tailscale mesh (all nodes) | Provides the routing for failover WS URL and SSH exec on .23 | Already configured. Prerequisite satisfied. |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `sha2@0.10.8` | Rust 1.60+, `no_std` capable | Current stable (2025). Compatible with Rust 1.93.1 on James's machine. No feature flags needed for this use case. |
| OpenSSH Server (Windows built-in) | Windows 11 (all pods and server) | `Add-WindowsCapability` failed on server's corrupted component store — use `winget install Microsoft.OpenSSH.Beta` or Win32-OpenSSH MSI as alternative. Must verify on server before committing to SSH-based recovery. |
| `tokio::sync::watch` | tokio 1.x | Ships with tokio 1.0+. Already in workspace dependencies. Zero version concern. |
| `tokio::process::Command` | tokio 1.x | Same — ships with tokio. Used for SSH exec and `send_email.js` calls. |

---

## Sources

- [Tailscale SSH GitHub issue #14942](https://github.com/tailscale/tailscale/issues/14942) — Tailscale SSH server NOT supported on Windows; issue closed pointing to #4697 for Windows-specific development — HIGH confidence (official Tailscale GitHub)
- [Microsoft Learn: Add-DhcpServerv4Reservation](https://learn.microsoft.com/en-us/powershell/module/dhcpserver/add-dhcpserverv4reservation?view=windowsserver2022-ps) — PowerShell DHCP reservation cmdlet — HIGH confidence (official Microsoft docs)
- [Microsoft Learn: Get started with OpenSSH Server for Windows](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse) — `Add-WindowsCapability` install path — HIGH confidence (official Microsoft docs)
- [TP-Link DHCP Address Reservation FAQ](https://www.tp-link.com/us/support/faq/182/) — Router-side MAC reservation procedure — HIGH confidence (official TP-Link docs)
- [sha2 crates.io](https://crates.io/crates/sha2) — version 0.10.8 current stable; 393M downloads; RustCrypto ecosystem — HIGH confidence (official crate registry)
- [tailscale-localapi Rust crate](https://github.com/jtdowney/tailscale-localapi) — local node status and cert only, no remote exec capability — MEDIUM confidence (GitHub README, fetched live)
- [How to Build Health Check Endpoints with Dependencies in Rust (OneUptime 2026)](https://oneuptime.com/blog/post/2026-01-25-health-check-endpoints-dependencies-rust/view) — dependency-aware health check patterns, 503 on critical dep failure — MEDIUM confidence (recent 2026 article, community source)
- [tokio::sync::watch docs](https://docs.rs/tokio/latest/tokio/sync/watch/index.html) — lock-free broadcast for frequently-read, rarely-written values — HIGH confidence (official tokio docs)
- [Set-NetIPAddress PowerShell](https://learn.microsoft.com/en-us/powershell/module/nettcpip/set-netipaddress) — static IP assignment via PowerShell on Windows 11 — HIGH confidence (official Microsoft docs)
- Existing codebase analysis: `bono_relay.rs`, `cloud_sync.rs`, `config.rs`, `rc-agent/main.rs`, `rc-installer/main.rs` — HIGH confidence (source of truth for integration points)

---

*Stack research for: v10.0 Connectivity & Redundancy — DHCP stability, remote exec, health monitoring, config sync, auto-failover, failback for Racing Point eSports*
*Researched: 2026-03-20 IST*
