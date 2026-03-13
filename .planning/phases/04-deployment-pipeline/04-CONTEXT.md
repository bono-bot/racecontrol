# Phase 4: Deployment Pipeline Hardening - Context

**Created:** 2026-03-13

## Key Code Locations

### rc-common (shared types + protocol)

| File | What | Relevance |
|------|------|-----------|
| `crates/rc-common/src/protocol.rs` | DashboardEvent, DashboardCommand, AgentMessage, CoreToAgentMessage enums | Add DeployProgress event, Deploy command variants |
| `crates/rc-common/src/types.rs` | Shared types (PodInfo, PodStatus, SimType, etc.) | DeployState enum lives here (shared between core and kiosk) |
| `crates/rc-common/src/watchdog.rs` | EscalatingBackoff | Reused by deploy for backoff on retry |

### rc-core (server + deploy executor)

| File | What | Relevance |
|------|------|-----------|
| `crates/rc-core/src/state.rs` | AppState struct, WatchdogState enum | Add per-pod DeployState field, DeployState check for pod_monitor skip |
| `crates/rc-core/src/pod_monitor.rs` | Watchdog, verify_restart(), is_ws_alive() | Reuse verify_restart pattern for post-deploy verification; add DeployState skip check |
| `crates/rc-core/src/pod_healer.rs` | Self-healing daemon, WatchdogState skip logic | Add DeployState skip check parallel to WatchdogState skip |
| `crates/rc-core/src/api/routes.rs` | HTTP API routes | Add POST /api/deploy/:pod_id, POST /api/deploy/rolling, GET /api/deploy/status |
| `crates/rc-core/src/lib.rs` | Module declarations | Add `pub mod deploy;` |
| `crates/rc-core/src/ws/mod.rs` | WebSocket handlers, agent registration, billing resync | Agent reconnection after deploy confirms WS health |
| `crates/rc-core/src/billing.rs` | BillingTimer, active_timers | Check for active sessions before deploy |
| `crates/rc-core/src/wol.rs` | WoL, shutdown_pod(), restart_pod() | NOT used for deploy (reboots entire machine); reference for pod-agent HTTP pattern |
| `crates/rc-core/src/email_alerts.rs` | EmailAlerter, format_alert_body() | Send alert on deploy failure |
| `crates/rc-core/src/main.rs` | Server startup, module spawning | May need to spawn rolling deploy listener |

### rc-agent (pod-side)

| File | What | Relevance |
|------|------|-----------|
| `crates/rc-agent/src/main.rs` | Agent main loop, WebSocket connect, config load | Agent restarts after deploy; WS reconnection logic |

### Kiosk (Next.js dashboard)

| File | What | Relevance |
|------|------|-----------|
| `kiosk/src/hooks/useKioskSocket.ts` | WebSocket connection to rc-core | Receives DashboardEvent::DeployProgress |
| `kiosk/src/lib/types.ts` | TypeScript types matching Rust protocol | Add DeployState, DeployProgress types |
| `kiosk/src/app/page.tsx` | Main dashboard page | Deploy UI integration point |
| `kiosk/src/components/KioskPodCard.tsx` | Per-pod card component | Show deploy state per pod |
| `kiosk/src/app/settings/page.tsx` | Settings page | Deploy trigger button could live here |

### Deploy Staging (manual deploy tools)

| File | What | Relevance |
|------|------|-----------|
| `C:\Users\bono\racingpoint\deploy-staging\deploy_pod.py` | Python deploy script (5-step) | Reference implementation for deploy sequence |
| `C:\Users\bono\racingpoint\deploy-staging\deploy-cmd.json` | Single-command deploy payload | Shows the compound command approach (has gaps) |
| `C:\Users\bono\racingpoint\deploy-staging\install.bat` | USB pendrive installer (8-step) | Reference for full install sequence |
| `C:\Users\bono\racingpoint\deploy-staging\rc-agent.template.toml` | Config template with {pod_number} placeholder | Used by deploy executor to generate per-pod config |
| `C:\Users\bono\racingpoint\deploy-staging\verify-cmd.json` | Reboot command payload | NOT used for deploy (too destructive) |

### Pod Network Map (for reference)

| Pod | IP | MAC |
|-----|----|-----|
| 1 | 192.168.31.89 | 30-56-0F-05-45-88 |
| 2 | 192.168.31.33 | 30-56-0F-05-46-53 |
| 3 | 192.168.31.28 | 30-56-0F-05-44-B3 |
| 4 | 192.168.31.88 | 30-56-0F-05-45-25 |
| 5 | 192.168.31.86 | 30-56-0F-05-44-B7 |
| 6 | 192.168.31.87 | 30-56-0F-05-45-6E |
| 7 | 192.168.31.38 | 30-56-0F-05-44-B4 |
| 8 (canary) | 192.168.31.91 | 30-56-0F-05-46-C5 |

### Pod Binary Layout

| Path | File | Notes |
|------|------|-------|
| `C:\RacingPoint\rc-agent.exe` | Main agent binary | Killed during deploy |
| `C:\RacingPoint\rc-agent.toml` | Agent config | Overwritten during deploy |
| `C:\RacingPoint\pod-agent.exe` | Pod-agent binary | NOT touched during rc-agent deploy |
| `C:\RacingPoint\watchdog.bat` | Watchdog script | NOT touched during rc-agent deploy |
| `C:\RacingPoint\start-rcagent.bat` | HKLM Run startup script | NOT touched during deploy |

### Ports

| Port | Service | On |
|------|---------|----|
| 8080 | rc-core (Axum) | Server (.23) / James (.27) |
| 8090 | pod-agent (Node.js) | Each pod |
| 9998 | HTTP file server (Python) | James (.27), manual start |
| 18923 | rc-agent lock screen | Each pod (localhost only) |

---

## Key Interfaces (Code Snippets)

### AppState fields relevant to deploy

```rust
// crates/rc-core/src/state.rs
pub struct AppState {
    pub pods: RwLock<HashMap<String, PodInfo>>,
    pub agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>,
    pub billing: BillingManager,
    pub http_client: reqwest::Client,
    pub dashboard_tx: broadcast::Sender<DashboardEvent>,
    pub email_alerter: RwLock<EmailAlerter>,
    pub pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>,
    // NEW in Phase 4:
    // pub pod_deploy_states: RwLock<HashMap<String, DeployState>>,
}
```

### pod-agent /exec call pattern (from pod_monitor.rs)

```rust
let exec_url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
let result = state
    .http_client
    .post(&exec_url)
    .json(&serde_json::json!({
        "cmd": restart_cmd,
        "timeout_ms": 10000
    }))
    .timeout(Duration::from_millis(15000))
    .send()
    .await;
```

### Billing session check pattern (from pod_monitor.rs)

```rust
let has_active_billing = state
    .billing
    .active_timers
    .read()
    .await
    .contains_key(&pod.id);
```

### DashboardEvent broadcast pattern

```rust
let _ = state.dashboard_tx.send(DashboardEvent::PodRestarting {
    pod_id: pod.id.clone(),
    attempt,
    max_attempts: 4,
    backoff_label: label,
});
```

### verify_restart check sequence (pod_monitor.rs)

```rust
// 1. Process running?
let process_ok = check_process_running(&state, &pod_ip).await;
// 2. WebSocket connected?
let ws_ok = is_ws_alive(&state, &pod_id).await;
// 3. Lock screen responsive?
let lock_ok = check_lock_screen(&state, &pod_ip).await;
```

### WatchdogState skip pattern (pod_monitor.rs + pod_healer.rs)

```rust
let wd_state = {
    let states = state.pod_watchdog_states.read().await;
    states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
};
match wd_state {
    WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. } => {
        continue; // skip this pod
    }
    _ => {}
}
```
