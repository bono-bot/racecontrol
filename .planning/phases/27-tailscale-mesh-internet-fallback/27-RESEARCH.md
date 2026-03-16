# Phase 27: Tailscale Mesh + Internet Fallback - Research

**Researched:** 2026-03-16
**Domain:** Tailscale Windows Service deployment, WinRM-based fleet provisioning, Axum interface binding, webhook push pattern
**Confidence:** HIGH (Tailscale official docs verified, existing codebase patterns confirmed)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Use WinRM (port 5985 — confirmed open on all 8 pods) for silent push
- Same Windows admin credentials on all pods — one script deploys to all 8 + server
- Download Tailscale installer from James's HTTP deploy server (deploy-staging), run silent install via WinRM
- Pre-auth key baked into deploy script — pods join Racing Point Tailscale network automatically with zero interaction
- Install as **Windows Service** (Tailscale default on Windows) — survives reboots without user login, no Session 1 dependency
- Canary: deploy to Pod 8 first, verify Tailscale IP assigned and reachable, then roll to remaining pods
- `cloud_sync.rs` routes through Bono's **Tailscale IP** instead of public internet (72.60.101.58)
- Change is a config value in `racecontrol.toml` — `[cloud].api_url` points to `http://<bono-tailscale-ip>/api/v1`
- No parallel/fallback to public internet — Tailscale is the primary path. If Tailscale is down, sync fails gracefully (existing retry logic already handles this)
- Real-time telemetry pushed: lap times, sector splits, speed, g-force from pods via server to Bono over Tailscale
- Game state + pod health pushed: pod online/offline, session active, game running, FFB status
- rc-agent LAN-only binding unchanged; Bono's commands relay through server
- Bono commands: Bono → server Tailscale IP (new HTTP endpoint on racecontrol, Tailscale interface only) → pod via existing WebSocket/pod-agent
- Server pushes events to Bono's VPS as they happen — not polling
- Events: session_start, session_end, lap_recorded, pod_offline, pod_online, billing_end
- When customer books via PWA and pays, Bono's VPS triggers game launch via Tailscale → server relay → rc-agent

### Claude's Discretion
- Tailscale device naming convention (pod-1 through pod-8, racing-point-server, bono-vps)
- Event payload schema for push events
- HTTP endpoint design on racecontrol for Bono's inbound commands
- Tailscale ACL policy (which devices can reach which)

### Deferred Ideas (OUT OF SCOPE)
- Fallback trigger logic (Tailscale down → fall back to public internet)
- AI debug logs streaming to Bono
- Direct Bono → pod commands (bypassing server) — LAN-only binding for rc-agent is a standing decision
- Auth key rotation policy / Tailscale admin console access for Bono
</user_constraints>

---

## Summary

Phase 27 installs Tailscale as a Windows Service on all 8 pods and the Racing Point server using WinRM + PowerShell remoting from James's machine, then extends racecontrol to push real-time events to Bono's VPS over the Tailscale mesh and receive inbound commands back. The deploy sequence is: download Tailscale MSI to deploy-staging, push via WinRM to each pod, silent MSI install, run `tailscale.exe up --unattended --auth-key=<key>`, verify 100.x.x.x IP assigned. The Tailscale Windows service (`tailscaled`) runs as a SYSTEM-level Windows service — it starts before user login, survives reboots, and has no Session 1 dependency.

On the racecontrol side, two changes are needed: (1) `cloud_sync.rs` `api_url` config value updated to Bono's Tailscale IP — this is a one-line TOML change because `api_url` is already `Option<String>`. (2) A new `bono_relay.rs` module adds a Tokio background task that pushes events to Bono's webhook endpoint using the existing `reqwest 0.12` client, plus a new Axum route bound only to the Tailscale interface (100.x.x.x) that receives Bono's inbound commands.

Axum binds to a specific interface by passing a specific `SocketAddr` to `tokio::net::TcpListener::bind()`. The current architecture uses one `bind_addr` from config — the cleanest approach is to spawn a second Axum listener bound to the Tailscale IP in addition to the existing LAN listener, or add a Tailscale-specific bind address to config and use it for the relay endpoint.

**Primary recommendation:** Two-listener approach — main server stays on `0.0.0.0:8080` (unchanged), new relay endpoint binds to `100.x.x.x:8081` (Tailscale IP). No middleware changes needed; Tailscale network isolation handles access control at the OS level.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Tailscale Windows MSI | Latest stable | WireGuard-based mesh VPN, Windows Service install | Official distribution, auto-registers `tailscaled` service |
| `reqwest 0.12` | Already in workspace | HTTP client for webhook push to Bono | Already used in `cloud_sync.rs`, `ai.rs` — zero new deps |
| `tokio` | Already in workspace | Async background task for event push loop | Already used everywhere |
| `axum 0.8` | Already in workspace | New Tailscale-bound HTTP endpoint for inbound commands | Already used for all routes |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| WinRM/PowerShell Remoting | Built into Windows | Push Tailscale installer from James's machine to pods | Confirmed port 5985 open on all pods |
| `serde_json` | Already in workspace | Event payload serialization | Already used everywhere |
| msiexec | Windows built-in | Silent MSI install on remote pods | Standard Windows silent install mechanism |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Two Axum listeners | Middleware IP filter on single listener | Two listeners is simpler — binding OS-level prevents any port confusion; middleware filter is fragile if Tailscale IP changes |
| Push via webhook | Pull polling from Bono | Push is lower latency and matches the event-driven architecture; Bono's VPS already has HTTP endpoints |
| reqwest for webhook | tokio-tungstenite WebSocket | HTTP POST webhook is stateless, simpler to implement, matches existing cloud_sync pattern |

**Installation (pods, via WinRM):**
```powershell
# Step 1: Download MSI to pod from James's deploy-staging HTTP server
Invoke-Command -ComputerName $podIp -Credential $cred -ScriptBlock {
    Invoke-WebRequest -Uri "http://192.168.31.27:9998/tailscale-setup-latest-amd64.msi" `
        -OutFile "C:\RacingPoint\tailscale.msi"
}

# Step 2: Silent install — registers tailscaled Windows Service automatically
Invoke-Command -ComputerName $podIp -Credential $cred -ScriptBlock {
    Start-Process msiexec.exe -ArgumentList `
        '/i "C:\RacingPoint\tailscale.msi" /quiet /norestart TS_UNATTENDEDMODE=always TS_NOLAUNCH=true' `
        -Wait -PassThru
}

# Step 3: Join tailnet with pre-auth key
Invoke-Command -ComputerName $podIp -Credential $cred -ScriptBlock {
    param($authKey, $hostname)
    & "C:\Program Files\Tailscale\tailscale.exe" up `
        --unattended `
        --auth-key=$authKey `
        --hostname=$hostname `
        --reset
} -ArgumentList $PREAUTH_KEY, "pod-$podNumber"
```

---

## Architecture Patterns

### Recommended Module Structure
```
crates/racecontrol/src/
├── bono_relay.rs         # NEW — Bono webhook push + inbound command endpoint
├── config.rs             # ADD BonoConfig struct with tailscale_ip, webhook_url, relay_port
├── cloud_sync.rs         # CHANGE — api_url in racecontrol.toml updated, no code change
└── main.rs               # ADD — spawn bono_relay, bind second listener on Tailscale IP
```

```
scripts/
└── deploy-tailscale.ps1  # NEW — WinRM fleet deploy script (runs on James's machine)
```

### Pattern 1: Tailscale Windows Service Deploy via WinRM

**What:** From James's machine (.27), use PowerShell `Invoke-Command` over WinRM to push MSI + run `tailscale.exe up` on each pod.

**When to use:** Initial fleet provisioning and any re-enrollment.

**Key insight — the two-step install:** The MSI installs the `tailscaled` Windows Service but does NOT join the tailnet. The separate `tailscale.exe up --auth-key=...` step authenticates and joins. These are two distinct operations.

**Verified commands (source: hellocharli/tailscale-unattended PowerShell script):**
```powershell
# Silent MSI install (registers service, no GUI launched)
msiexec.exe /i "tailscale.msi" /quiet TS_UNATTENDEDMODE=always TS_NOLAUNCH=true

# Join tailnet (authenticate + connect)
& "C:\Program Files\Tailscale\tailscale.exe" up --unattended --auth-key=tskey-xxxxx --reset
```

**Service survival:** The MSI installs `tailscaled` as a Windows service with `StartType=Automatic`. It starts at boot before any user logs in — no Session 1 dependency. This is the critical advantage over rc-agent (which needs HKLM Run key at user login).

**Verify after deploy:**
```powershell
# Check service is running
Invoke-Command -ComputerName $podIp -Credential $cred -ScriptBlock {
    Get-Service Tailscale | Select-Object Status, StartType
}

# Check assigned Tailscale IP
Invoke-Command -ComputerName $podIp -Credential $cred -ScriptBlock {
    & "C:\Program Files\Tailscale\tailscale.exe" ip -4
}

# Confirm reachable from James's machine (after James also joins tailnet)
Test-NetConnection -ComputerName <pod-tailscale-ip> -Port 8090
```

### Pattern 2: cloud_sync.rs — One-Line Config Change

**What:** Update `racecontrol.toml` to point `[cloud].api_url` at Bono's Tailscale IP instead of 72.60.101.58.

**When to use:** After Tailscale is live on server and Bono's VPS joins the same tailnet.

**Code change required:** None. The `api_url` field is already `Option<String>` in `CloudConfig`. The sync loop reads it at startup.

```toml
# Before (public internet)
[cloud]
api_url = "https://app.racingpoint.cloud/api/v1"

# After (Tailscale mesh — Bono's 100.x.x.x IP)
[cloud]
api_url = "http://100.x.x.x/api/v1"
```

**Note:** Switch from `https://` to `http://` because Tailscale encrypts the tunnel at the WireGuard layer. No TLS needed inside the mesh. This is standard Tailscale usage — the docs explicitly note that internal mesh traffic is already encrypted.

### Pattern 3: bono_relay.rs — Event Push Module

**What:** New module that: (a) accepts events from other racecontrol modules via an mpsc channel, (b) runs a Tokio background task that POSTs events to Bono's webhook URL using reqwest.

**Pattern mirrors `cloud_sync.rs` exactly:**
```rust
// Source: existing cloud_sync.rs spawn() pattern
pub fn spawn(state: Arc<AppState>) {
    let bono = &state.config.bono;
    if !bono.enabled { return; }

    let webhook_url = match &bono.webhook_url {
        Some(url) => url.clone(),
        None => return,
    };

    tokio::spawn(async move {
        let mut rx = state.bono_event_tx.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = push_event(&state, &webhook_url, event).await {
                        tracing::error!("Bono webhook push failed: {}", e);
                        // Non-fatal — existing retry semantics: next event will retry
                    }
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    });
}

async fn push_event(state: &Arc<AppState>, url: &str, event: BonoEvent) -> anyhow::Result<()> {
    state.http_client
        .post(url)
        .json(&event)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    Ok(())
}
```

### Pattern 4: Axum Second Listener on Tailscale Interface

**What:** Bind a second Axum router exclusively to the Tailscale IP (100.x.x.x) on a dedicated port. Bono's inbound commands arrive here. The existing router on `0.0.0.0:8080` is unchanged.

**Why second listener, not middleware filter:** Binding at the OS level (via `TcpListener`) is more reliable than filtering by source IP in middleware. Tailscale guarantees that traffic arriving on 100.x.x.x came through the authenticated mesh. No unauthorized client can reach this port even if they spoof an IP.

**Verified pattern — Axum binds to specific IP via TcpListener:**
```rust
// Source: Axum docs + tokio::net::TcpListener
// In main.rs, after spawning all background tasks:

// Existing listener (unchanged)
let main_listener = tokio::net::TcpListener::bind(&bind_addr).await?;

// New Tailscale relay listener (only if tailscale_ip is configured)
if let Some(ts_ip) = &state.config.bono.tailscale_bind_ip {
    let ts_addr = format!("{}:{}", ts_ip, state.config.bono.relay_port.unwrap_or(8081));
    let ts_listener = tokio::net::TcpListener::bind(&ts_addr).await?;
    let relay_app = build_relay_router(state.clone());
    tokio::spawn(async move {
        tracing::info!("Bono relay endpoint on {}", ts_addr);
        axum::serve(ts_listener, relay_app).await.unwrap();
    });
}

// Original serve call unchanged
axum::serve(main_listener, app).await?;
```

The relay router only exposes the command relay endpoint:
```rust
fn build_relay_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/relay/command", post(bono_relay::handle_command))
        .route("/relay/health", get(|| async { "ok" }))
        .with_state(state)
}
```

### Pattern 5: Config Addition for [bono] Section

Follows the exact same pattern as existing `[cloud]` config in `config.rs`:

```rust
#[derive(Debug, Default, Deserialize)]
pub struct BonoConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Bono's Tailscale IP for webhook pushes (e.g., "http://100.x.x.x/webhooks/racecontrol")
    pub webhook_url: Option<String>,
    /// Local Tailscale IP to bind relay endpoint on (e.g., "100.y.y.y")
    pub tailscale_bind_ip: Option<String>,
    /// Port for Bono relay endpoint (default: 8081)
    #[serde(default = "default_relay_port")]
    pub relay_port: u16,
}
fn default_relay_port() -> u16 { 8081 }
```

```toml
[bono]
enabled = true
webhook_url = "http://100.x.x.x/webhooks/racecontrol"
tailscale_bind_ip = "100.y.y.y"   # server's own Tailscale IP
relay_port = 8081
```

### Pattern 6: Event Payload Schema (Claude's Discretion)

Use a tagged enum following the existing `AgentMessage` pattern in `rc-common/src/protocol.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BonoEvent {
    SessionStart { pod_number: u32, driver_name: String, game: String, session_id: String },
    SessionEnd { pod_number: u32, session_id: String, duration_secs: u64, paise_charged: i64 },
    LapRecorded { pod_number: u32, session_id: String, lap_time_ms: u64, track: String, car: String },
    PodOffline { pod_number: u32, ip: String, last_seen_secs_ago: u64 },
    PodOnline { pod_number: u32, ip: String, tailscale_ip: Option<String> },
    BillingEnd { pod_number: u32, session_id: String, driver_id: String },
}
```

### Pattern 7: PWA Game Launch via Tailscale Relay

**Flow:** Bono's VPS POSTs to `http://<server-tailscale-ip>:8081/relay/command` with a `LaunchGame` payload. The relay endpoint validates a shared secret, then uses the existing `ws::send_command_to_pod()` function (already exists) to relay to the pod's rc-agent.

```rust
// Relay endpoint handler
async fn handle_command(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(cmd): Json<RelayCommand>,
) -> impl IntoResponse {
    // Validate shared secret from header
    let secret = headers.get("X-Relay-Secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if secret != state.config.bono.relay_secret.as_deref().unwrap_or("") {
        return (StatusCode::UNAUTHORIZED, "bad secret").into_response();
    }
    // Forward to pod via existing WebSocket channel
    match cmd {
        RelayCommand::LaunchGame { pod_number, game, track, car } => {
            // use existing game_launcher::launch_game_on_pod()
        }
        RelayCommand::StopGame { pod_number } => { ... }
    }
    (StatusCode::OK, "queued").into_response()
}
```

### Anti-Patterns to Avoid

- **Don't bind main server to Tailscale IP only:** Pods connect to server via LAN (192.168.31.x WebSocket). If server only binds to Tailscale IP, all 8 pods lose connectivity. Keep `0.0.0.0:8080` for LAN.
- **Don't use HTTPS inside Tailscale mesh:** Tailscale WireGuard encrypts all mesh traffic. Adding TLS on top is redundant. HTTP on Tailscale IP is the standard pattern.
- **Don't copy Tailscale MSI over WinRM directly:** The "second hop" restriction in WinRM prevents accessing network shares from within the remote session. Use James's HTTP deploy server (`192.168.31.27:9998`) as the download source — pods pull from James, not push from James.
- **Don't run `tailscale.exe` check before service starts:** After MSI install, give the `tailscaled` service 3-5 seconds to start before running `tailscale up`. Use `Start-Sleep 5` between install and auth steps.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Encrypted mesh VPN | Custom WireGuard config, manual key exchange | Tailscale | Key distribution, NAT traversal, ACLs, device inventory all pre-built |
| Fleet WinRM deploy | Custom SSH agent or custom installer service | PowerShell `Invoke-Command` over WinRM | WinRM already open on all pods, no new service needed |
| Auth key management | Custom token rotation | Tailscale pre-auth key (reusable) | One key for all 8 pods, stored in deploy script |
| Webhook retry logic | Custom retry queue | Let failures log + next event retries naturally | Events are ephemeral; missing one push is acceptable, not mission-critical |
| Interface-specific routing | iptables/netsh rules | Second Axum listener on Tailscale IP | OS-level binding is cleaner and more reliable |

**Key insight:** Tailscale solves the hardest parts (NAT traversal, key exchange, encryption, routing) so the implementation is 90% config and 10% code.

---

## Common Pitfalls

### Pitfall 1: WinRM Double-Hop (UNC Path Restriction)
**What goes wrong:** `Invoke-Command -ScriptBlock { Start-Process msiexec.exe /i \\192.168.31.27\share\tailscale.msi }` fails silently. WinRM sessions cannot authenticate against remote shares from within the remote session.
**Why it happens:** Kerberos credential delegation is not configured in workgroup environments.
**How to avoid:** Never reference UNC paths in WinRM script blocks. Always download the MSI to a local path on the pod first (`Invoke-WebRequest` from within the `Invoke-Command` block), then install from that local path.
**Warning signs:** msiexec returns exit code 1603 or 1619 without an obvious error.

### Pitfall 2: Tailscale Service Not Started Before `tailscale up`
**What goes wrong:** Running `tailscale.exe up` immediately after MSI install fails because `tailscaled` service hasn't fully started.
**Why it happens:** MSI install completes and control returns to PowerShell, but the service is still initializing.
**How to avoid:** Insert `Start-Sleep -Seconds 5` between install and the `tailscale up` command. Alternatively poll `Get-Service Tailscale` until Status is Running.
**Warning signs:** `tailscale up` returns "failed to connect to local Tailscale daemon" or similar.

### Pitfall 3: Tailscale IP Not Stable Across Reboots
**What goes wrong:** Server's Tailscale IP changes after reboot, breaking `racecontrol.toml` config.
**Why it happens:** Tailscale 100.x.x.x IPs are persistent — tied to device identity, not session. However, if a device is removed from the admin console and re-enrolled with a new key, it gets a new IP.
**How to avoid:** Use device stable addresses. Never remove and re-add devices during operations. If Tailscale IP must appear in config, document it clearly and provide a `tailscale ip -4` verification step.
**Warning signs:** cloud_sync fails with connection refused after a server re-enrollment.

### Pitfall 4: `tailscale up --reset` Clears Previous Flags
**What goes wrong:** If a pod already has Tailscale installed, `--reset` clears all previous configuration including advertised routes and exit node flags.
**Why it happens:** `--reset` is intended for clean re-enrollment.
**How to avoid:** Only use `--reset` on first install. For subsequent re-auths, omit `--reset`.
**Warning signs:** Pod disappears from Tailscale admin console shortly after re-running deploy script.

### Pitfall 5: CORS on Relay Endpoint
**What goes wrong:** Bono's VPS (100.x.x.x) cannot reach the relay endpoint because the CORS middleware on the main router rejects the request.
**Why it happens:** The relay endpoint is on a separate router/listener — CORS from the main app's `CorsLayer` does NOT apply to the relay router.
**How to avoid:** The relay router is intentionally separate and has no CORS middleware — it's a machine-to-machine API, not browser-facing. The only auth is the `X-Relay-Secret` header.
**Warning signs:** Bono's VPS gets 403 or CORS error. Check that the relay router is truly separate from the main router.

### Pitfall 6: Tailscale Key Expiry
**What goes wrong:** Pre-auth keys expire (default 90 days). New pods cannot join after key expires.
**Why it happens:** Tailscale auth keys have a maximum 90-day lifetime.
**How to avoid:** Use a reusable pre-auth key. Document expiry date in deploy script. Mark key rotation as an operational task (Uday's calendar). Consider using an OAuth client for programmatic key generation (no expiry on the OAuth client itself).
**Warning signs:** Deploy script reports "invalid auth key" on a new pod.

---

## Code Examples

### Existing Pattern: cloud_sync.rs spawn() — Mirror for bono_relay.rs

```rust
// Source: crates/racecontrol/src/cloud_sync.rs:74-106
// The bono_relay.rs spawn() function mirrors this EXACT structure:
pub fn spawn(state: Arc<AppState>) {
    let cloud = &state.config.cloud;
    if !cloud.enabled { return; }
    let api_url = match &cloud.api_url {
        Some(url) => url.clone(),
        None => { tracing::warn!("no api_url"); return; }
    };
    // ...
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(tick_interval));
        loop {
            interval.tick().await;
            if let Err(e) = sync_once_http(&state, &api_url).await {
                tracing::error!("Cloud sync failed: {}", e);
            }
        }
    });
}
```

### Existing Pattern: reqwest 0.12 POST — Same as HTTP push

```rust
// Source: crates/racecontrol/src/cloud_sync.rs (sync_push function)
// Uses state.http_client (Arc<reqwest::Client>) already in AppState
state.http_client
    .post(&push_url)
    .json(&payload)
    .timeout(Duration::from_secs(30))
    .send()
    .await?;
```

### Existing Pattern: Config struct with Option<String> — Same for BonoConfig

```rust
// Source: crates/racecontrol/src/config.rs:49-70 (CloudConfig)
#[derive(Debug, Default, Deserialize)]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    pub api_url: Option<String>,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    // ...
}
```

### Tailscale Windows Deploy Commands (Verified)

```powershell
# Source: https://github.com/hellocharli/tailscale-unattended (verified 2026-03-16)

# 1. Download MSI from James's HTTP server (inside WinRM session — avoids double-hop)
Invoke-WebRequest -Uri "http://192.168.31.27:9998/tailscale-setup-latest-amd64.msi" `
    -OutFile "C:\RacingPoint\tailscale.msi"

# 2. Silent MSI install — registers tailscaled Windows Service
Start-Process msiexec.exe -ArgumentList `
    '/i "C:\RacingPoint\tailscale.msi" /quiet /norestart TS_UNATTENDEDMODE=always TS_NOLAUNCH=true' `
    -Wait -PassThru

# 3. Wait for service to start
Start-Sleep -Seconds 5

# 4. Join tailnet with pre-auth key
& "C:\Program Files\Tailscale\tailscale.exe" up `
    --unattended `
    --auth-key=tskey-xxxxx `
    --hostname="pod-8" `
    --reset

# 5. Verify assigned IP
& "C:\Program Files\Tailscale\tailscale.exe" ip -4
```

### Tailscale API: Generate Pre-Auth Key

```bash
# Source: Tailscale API v2 (https://tailscale.com/kb/1101/api)
curl -s -X POST https://api.tailscale.com/api/v2/tailnet/-/keys \
  -H "Authorization: Bearer $TAILSCALE_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "capabilities": {
      "devices": {
        "create": {
          "reusable": true,
          "ephemeral": false,
          "preauthorized": true,
          "tags": ["tag:venue"]
        }
      }
    },
    "expirySeconds": 7776000,
    "description": "RacingPoint venue pods"
  }'
```

### Axum Second Listener on Tailscale IP

```rust
// Source: tokio::net::TcpListener pattern (axum docs, verified)
// Tailscale IP is 100.x.x.x — bind relay router exclusively here
let ts_addr = format!("{}:8081", tailscale_ip);
let ts_listener = tokio::net::TcpListener::bind(&ts_addr).await?;
let relay_router = Router::new()
    .route("/relay/command", post(bono_relay::handle_command))
    .route("/relay/health", get(|| async { "ok" }))
    .with_state(state.clone());
tokio::spawn(async move {
    axum::serve(ts_listener, relay_router).await.unwrap();
});
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Public internet (72.60.101.58) for cloud_sync | Tailscale mesh for cloud_sync | Phase 27 | All billing/driver sync is private, no data in transit over public internet |
| No remote access from Bono to venue | Tailscale relay via server for remote commands | Phase 27 | PWA game launch from cloud becomes possible |
| Manual on-site deployment only | WinRM fleet deploy from James's machine | Phase 27 | 8-pod deploy in one script run |
| Tailscale GUI install (requires login) | MSI silent install + `tailscale up --unattended` | Tailscale v1.30+ | Service-level, survives reboots without Session 1 |

**Deprecated/outdated:**
- Manual browser-based Tailscale login per device: replaced by pre-auth key in deploy script
- Separate `TS_AUTHKEY` MSI property: not supported by Tailscale MSI. Auth is a separate `tailscale.exe up --auth-key=...` step after MSI install

---

## Open Questions

1. **Bono's Tailscale IP**
   - What we know: Bono's VPS (srv1422716.hstgr.cloud / 72.60.101.58) will join the tailnet, but we don't know its Tailscale 100.x.x.x IP until it joins.
   - What's unclear: Whether Bono's VPS already has Tailscale installed (this is out-of-scope for this phase per CONTEXT.md).
   - Recommendation: Use a placeholder in `racecontrol.toml` (`[bono].webhook_url = ""`). Bono fills in his own Tailscale IP after joining. The code gracefully no-ops when `enabled = false` or `webhook_url = None`.

2. **Relay Secret for Inbound Command Auth**
   - What we know: The relay endpoint must authenticate Bono's VPS to prevent replay attacks from other tailnet members.
   - What's unclear: What shared secret mechanism — static string in config is simplest.
   - Recommendation: Add `relay_secret` to `[bono]` config. Header `X-Relay-Secret: <value>`. Document it as a one-time setup value that Bono and James agree on. TOML config, not env var (consistent with existing auth patterns).

3. **Windows service start type on pods already seen to have issues**
   - What we know: GitHub issues report `tailscaled` sometimes fails to auto-start after reboot on some Windows versions (issues #793, #3186 in tailscale/tailscale).
   - What's unclear: Whether pods (Windows 10/11, gaming config) are affected.
   - Recommendation: After deploy, verify `Get-Service Tailscale | Select StartType` shows `Automatic`. If issues are found post-Phase 27, add a startup check to rc-agent that verifies Tailscale is connected before reporting online.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (built-in, no separate framework) |
| Config file | `Cargo.toml` workspace + per-crate |
| Quick run command | `cargo test -p racecontrol-crate --lib 2>&1 \| tail -20` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TS-01 | `BonoConfig` deserializes from TOML with correct defaults | unit | `cargo test -p racecontrol-crate config::tests::bono_config_defaults` | ❌ Wave 0 |
| TS-02 | `bono_relay::spawn()` no-ops when `enabled = false` | unit | `cargo test -p racecontrol-crate bono_relay::tests::spawn_disabled` | ❌ Wave 0 |
| TS-03 | `bono_relay::spawn()` no-ops when `webhook_url = None` | unit | `cargo test -p racecontrol-crate bono_relay::tests::spawn_no_url` | ❌ Wave 0 |
| TS-04 | `BonoEvent` serializes to expected JSON shape | unit | `cargo test -p racecontrol-crate bono_relay::tests::event_serialization` | ❌ Wave 0 |
| TS-05 | Relay endpoint returns 401 with wrong secret | integration (manual) | Manual: `curl -X POST http://100.x.x.x:8081/relay/command` without correct header | N/A — requires live Tailscale |
| TS-06 | `cloud_sync.rs` uses `api_url` from config (no hardcoded IP) | unit | Existing: `cargo test -p racecontrol-crate` (cloud_sync tests already pass) | ✅ |
| TS-DEPLOY | All 8 pods show Tailscale IP `tailscale ip -4` | smoke (manual) | `Invoke-Command ... { & tailscale.exe ip -4 }` for each pod | N/A — deploy-time |

**Manual-only justification for TS-05 and TS-DEPLOY:** These require live Tailscale network connectivity. They cannot be automated in unit tests without a running tailnet.

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate --lib 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate`
- **Phase gate:** Full suite green + Pod 8 canary verified (Tailscale IP reachable from James's machine) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Unit tests for `BonoConfig` defaults — add to `config.rs` `#[cfg(test)]` block (pattern matches existing `watchdog_config_deserializes_with_defaults` test)
- [ ] Unit tests for `bono_relay.rs` spawn guard conditions (enabled/no-url) — new test module
- [ ] Unit tests for `BonoEvent` serialization — new test in `bono_relay.rs`

---

## Sources

### Primary (HIGH confidence)
- Tailscale official docs (kb/1189) — MSI properties list, silent install syntax
- Tailscale official docs (kb/1278) — tailscaled Windows service, `net start/stop Tailscale`
- Tailscale official docs (kb/1088) — unattended mode, `--unattended` flag behavior
- Tailscale official docs (kb/1085) — auth key types, `tailscale up --auth-key=` CLI syntax
- `crates/racecontrol/src/cloud_sync.rs` — existing spawn/http pattern that bono_relay mirrors
- `crates/racecontrol/src/config.rs` — existing Config struct pattern for BonoConfig addition
- `crates/racecontrol/src/main.rs` — Axum `tokio::net::TcpListener::bind()` + `axum::serve()` pattern

### Secondary (MEDIUM confidence)
- github.com/hellocharli/tailscale-unattended — verified exact MSI + `tailscale up` command sequence
- Tailscale API v2 create key endpoint — cross-referenced with official API docs
- Tailscale GitHub issues #793, #3186 — Windows service startup edge cases documented

### Tertiary (LOW confidence)
- WebSearch results on WinRM MSI deploy patterns — standard patterns, multiple sources agree

---

## Metadata

**Confidence breakdown:**
- Tailscale install commands: HIGH — verified via official docs + open-source reference script
- Windows Service behavior: HIGH — official tailscaled docs confirm service-level operation
- cloud_sync config change: HIGH — source code read directly, `api_url` is `Option<String>`, no code change needed
- Axum second listener: HIGH — standard Rust `TcpListener::bind` pattern, Axum docs confirmed
- bono_relay module design: HIGH — mirrors existing `cloud_sync.rs` and `action_queue.rs` patterns exactly
- WinRM deploy approach: MEDIUM — standard PowerShell remoting, double-hop pitfall well-documented
- Tailscale API key generation: MEDIUM — API endpoint structure confirmed, exact OAuth flow not needed for Phase 27 (manual key is fine)

**Research date:** 2026-03-16
**Valid until:** 2026-06-16 (Tailscale stable; auth key API format occasionally changes — re-verify if >90 days)
