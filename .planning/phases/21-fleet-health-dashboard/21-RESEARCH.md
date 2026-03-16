# Phase 21: Fleet Health Dashboard - Research

**Researched:** 2026-03-15
**Domain:** Rust/Axum backend endpoint + Next.js 16 frontend page
**Confidence:** HIGH — all findings verified against actual source code

## Summary

Phase 21 adds a read-only fleet health view to the existing kiosk app at `/fleet`. The backend must expose a new `GET /api/v1/fleet/health` endpoint that combines three pieces of data: WS connection state (already in `state.agent_senders`), HTTP reachability (a live probe to `:8090/health`), and per-pod version + uptime (currently logged but not stored from `AgentMessage::StartupReport`).

The frontend is a single Next.js page at `kiosk/src/app/fleet/page.tsx` that polls the new endpoint every 5 seconds and renders 8 pod cards in a 2x4 grid. No WebSocket required — HTTP polling is sufficient for a health dashboard that refreshes every 5s. No login required (matches kiosk's other unauthenticated pages).

The key open questions from the previous research are now resolved by reading source code:

1. **`state.config.pods` does NOT have structured IP data per-pod.** The `PodsConfig` struct has only `count: u32`, `discovery: bool`, `static_pods: Vec<StaticPodConfig>`, and `healer_enabled`. IPs are stored in `state.pods` (the `RwLock<HashMap<String, PodInfo>>`), which is populated when each pod connects via WebSocket and sends a `Register` message. `PodInfo.ip_address` is the live source of truth.

2. **`GET :8090/health` on rc-agent DOES return HTTP 200 with JSON** (`{"status":"ok","version":"...","uptime_secs":...,"exec_slots_available":...,"exec_slots_total":4}`). Verified in `remote_ops.rs` lines 93-103.

3. **`StartupReport` data is NOT stored in AppState** — it is currently only logged via `tracing::info!` and `log_pod_activity`. The backend plan must add a `pod_startup_reports: RwLock<HashMap<String, StartupReportData>>` to `AppState` and populate it in the `ws/mod.rs` StartupReport handler.

**Primary recommendation:** Add a `pod_startup_reports` map to AppState (storing version + uptime_secs + crash_recovery per pod), add a background HTTP probe loop task, expose `GET /api/v1/fleet/health`, and render a simple polling page in the kiosk.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FLEET-01 | Kiosk /fleet page shows all 8 pods with real-time status (WS connected, HTTP reachable, version, uptime) | Backend endpoint + frontend page; HTTP polling every 5s; PodInfo.ip_address provides pod IPs |
| FLEET-02 | Pod status distinguishes WS-connected vs HTTP-reachable (a pod can be WS-up but HTTP-blocked) | `state.agent_senders` tracks WS; separate HTTP probe to `:8090/health` tracks HTTP; two independent boolean fields in response |
| FLEET-03 | Dashboard accessible from Uday's phone (mobile-first layout) | Next.js with Tailwind; 2-column grid on mobile (sm:grid-cols-4); no auth required |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Axum | 0.7.x (already in racecontrol) | New GET /api/v1/fleet/health endpoint | Already used for all racecontrol HTTP routes |
| tokio | 1.x (already in racecontrol) | Background HTTP probe loop task | Already the runtime |
| reqwest | 0.12.x (already in AppState as `http_client`) | HTTP GET to pod :8090/health | Re-use `state.http_client` — no new dep |
| Next.js | 16.1.6 (already in kiosk) | /fleet page | Existing app router, same as all other pages |
| Tailwind CSS | 4.x (already in kiosk) | Styling | Existing; use rp-* color tokens |
| TypeScript | 5.9.3 (already in kiosk) | Fleet page types | Existing codebase standard |

### No New Dependencies Required
All work fits within existing dependencies. The probe loop uses `state.http_client` (already a `reqwest::Client` in AppState). The frontend uses `fetch()` (no extra library needed for 5s polling).

**Installation:** None required.

## Architecture Patterns

### Recommended Project Structure (new files only)
```
crates/racecontrol/src/
└── fleet_health.rs          # New: PodFleetHealth struct, probe loop, GET handler

kiosk/src/app/fleet/
└── page.tsx                 # New: fleet dashboard page
```

### Data Flow

```
racecontrol startup
  └── spawn fleet_health probe loop (every 10s)
        └── GET http://{pod_ip}:8090/health per registered pod
              └── write bool to pod_http_reachable map in AppState

Pod WS connects → AgentMessage::StartupReport arrives
  └── ws/mod.rs handler stores {version, uptime_secs, crash_recovery}
        in pod_startup_reports map in AppState

GET /api/v1/fleet/health (called by frontend every 5s)
  └── for pod_number 1..=8:
        find pod in state.pods where number == n
        ws_connected = agent_senders.contains(pod_id) && !sender.is_closed()
        http_reachable = pod_http_reachable.get(pod_id)
        startup = pod_startup_reports.get(pod_id)
  └── return JSON array of 8 PodFleetStatus objects
```

### Pattern 1: AppState extension for new data

Two new fields added to `AppState` in `state.rs`:

```rust
/// Per-pod startup report data (version, uptime_secs, crash_recovery).
/// Populated when agent sends StartupReport after WS connect.
pub pod_startup_reports: RwLock<HashMap<String, PodStartupData>>,

/// Per-pod HTTP health probe results (true = reachable on :8090, false = blocked/down).
/// Updated every 10s by fleet_health probe loop.
pub pod_http_reachable: RwLock<HashMap<String, bool>>,
```

```rust
// Companion struct — define in fleet_health.rs, import into state.rs
#[derive(Debug, Clone)]
pub struct PodStartupData {
    pub version: String,
    pub uptime_secs: u64,
    pub crash_recovery: bool,
    pub received_at: chrono::DateTime<chrono::Utc>,
}
```

Initialize both to `HashMap::new()` in `AppState::new()`.

### Pattern 2: StartupReport storage in ws/mod.rs

The existing StartupReport handler at `ws/mod.rs:481` only logs. Add storage:

```rust
AgentMessage::StartupReport { pod_id, version, uptime_secs, crash_recovery, .. } => {
    // ... existing tracing/log_pod_activity code remains ...

    // Store for fleet health endpoint
    use crate::fleet_health::PodStartupData;
    state.pod_startup_reports.write().await.insert(
        pod_id.clone(),
        PodStartupData {
            version: version.clone(),
            uptime_secs: *uptime_secs,
            crash_recovery: *crash_recovery,
            received_at: chrono::Utc::now(),
        },
    );
}
```

### Pattern 3: HTTP probe loop (fleet_health.rs)

Pattern copied directly from `deploy.rs` `http_exec_on_pod` which uses the same `state.http_client`:

```rust
// Source: racecontrol/src/deploy.rs http_exec_on_pod pattern
pub fn start_probe_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let pods_snapshot: Vec<(String, String)> = {
                let pods = state.pods.read().await;
                pods.values()
                    .map(|p| (p.id.clone(), p.ip_address.clone()))
                    .collect()
            };
            for (pod_id, pod_ip) in pods_snapshot {
                let url = format!("http://{}:8090/health", pod_ip);
                let reachable = state.http_client
                    .get(&url)
                    .timeout(Duration::from_secs(3))
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
                state.pod_http_reachable.write().await.insert(pod_id, reachable);
            }
        }
    });
}
```

### Pattern 4: GET /api/v1/fleet/health response shape

```rust
#[derive(Serialize)]
pub struct PodFleetStatus {
    pub pod_number: u32,                        // 1-8 (always present)
    pub pod_id: Option<String>,                 // None if never registered
    pub ws_connected: bool,
    pub http_reachable: bool,
    pub version: Option<String>,               // from StartupReport
    pub uptime_secs: Option<u64>,              // from StartupReport
    pub crash_recovery: Option<bool>,          // from StartupReport
    pub ip_address: Option<String>,            // from PodInfo
    pub last_startup_report_at: Option<String>, // ISO-8601 timestamp
}
```

The endpoint always returns exactly 8 entries (one per pod number 1-8), regardless of how many pods have connected.

### Pattern 5: Frontend polling (Next.js page)

Plain `useEffect` + `setInterval` — no WebSocket, no `useKioskSocket`. The `/kiosk/fleet` URL is the accessible path (see Pitfall 4):

```typescript
// Source: pattern matches kiosk's lib/api.ts fetchApi
"use client";
import { useState, useEffect } from "react";

const API_BASE = typeof window !== "undefined"
  ? `http://${window.location.hostname}:8080`
  : "http://localhost:8080";

export default function FleetPage() {
  const [pods, setPods] = useState<PodFleetStatus[]>([]);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    async function poll() {
      try {
        const res = await fetch(`${API_BASE}/api/v1/fleet/health`);
        if (res.ok) {
          const data: { pods: PodFleetStatus[] } = await res.json();
          setPods(data.pods);
          setLastUpdated(new Date());
          setError(false);
        }
      } catch {
        setError(true);
      }
    }
    poll();
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  }, []);
  // ...
}
```

### Pattern 6: Mobile-first card grid

The kiosk basePath is `/kiosk`. Use 2-column on mobile, 4-column on sm+ screens:

```tsx
<div className="grid grid-cols-2 sm:grid-cols-4 gap-3 p-4">
  {pods.map((pod) => (
    <PodCard key={pod.pod_number} pod={pod} />
  ))}
</div>
```

### Pattern 7: Four-state visual indicators (FLEET-02)

Two independent dots per card — one for WS, one for HTTP:

| WS | HTTP | Border | Label |
|----|------|--------|-------|
| true | true | `border-green-500` | Healthy |
| true | false | `border-yellow-500` | WS only |
| false | true | `border-amber-500` | HTTP only |
| false | false | `border-rp-border opacity-40` | Offline |

Use Tailwind tokens from globals.css: `bg-rp-card`, `border-rp-border`, `text-rp-grey`, `text-rp-red`.

### Anti-Patterns to Avoid

- **Don't probe pods from inside the HTTP handler.** The handler must read cached state only. Probing 8 pods inline would take up to 24s (8 pods * 3s timeout).
- **Don't probe from the frontend.** CORS and LAN topology mean pod IPs are not reachable from Uday's phone — probing must be server-side.
- **Don't require login.** FLEET-03 explicitly says "no login required". The kiosk's other unauthenticated pages (/, /book, /spectator) confirm this pattern.
- **Don't use `useKioskSocket` for the fleet page.** The dashboard WS feed does not emit fleet health events — adding WS complexity for a polling page is unnecessary.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP probe client | Custom TCP probe or new reqwest Client | `state.http_client` (already in AppState) | Handles timeouts, keep-alive, error handling |
| Uptime string formatting | Rust formatter | Pass `uptime_secs: u64` to frontend, format in TypeScript | Simpler, flexible display |
| Pod IP lookup | Config file parser | `state.pods.read().get_by_number(n).ip_address` | Already populated from WS Register |
| Real-time updates | WebSocket event emission | `setInterval` + `fetch` every 5s | Adequate for health dashboard, no WS complexity |

## Common Pitfalls

### Pitfall 1: state.pods is empty until pods connect
**What goes wrong:** `state.pods` is a live registry — empty on startup. If no pods are registered, the endpoint returns 0 pods and the frontend shows blank.
**Why it happens:** Pods only appear in `state.pods` after WebSocket `Register` message.
**How to avoid:** The endpoint iterates `1..=8u32` and searches `state.pods.values()` for matching `pod.number`. Missing numbers produce an entry with all status flags false.

### Pitfall 2: StartupReport not stored — version/uptime always None before fix
**What goes wrong:** Before the Phase 21 backend change, `pod_startup_reports` doesn't exist. After adding the field but before deploying rc-agent, version/uptime will be None until each pod reconnects and sends a new StartupReport.
**Why it happens:** StartupReport is only sent once per agent lifetime (at WS connect after Register).
**How to avoid:** This is expected and acceptable — the dashboard shows "—" for version/uptime on pods that haven't reconnected since deploy. Force reconnection by restarting rc-agent during fleet deploy.

### Pitfall 3: URL discrepancy in success criteria
**What goes wrong:** ROADMAP success criteria says `http://192.168.31.23:3300/fleet`. The actual path is `http://192.168.31.23:3300/kiosk/fleet` because `next.config.ts` sets `basePath: "/kiosk"`.
**Why it happens:** The success criteria was written without checking the Next.js config.
**How to avoid:** Implement at `/kiosk/fleet`. Either document the URL correctly or add an nginx/Python redirect from `/fleet` to `/kiosk/fleet` at port 3300.

### Pitfall 4: Probe loop probes only registered pods
**What goes wrong:** A pod that has never WS-connected won't be in `state.pods`, so the probe loop never probes it, so `pod_http_reachable` is empty for it, so the health endpoint shows it as HTTP-unreachable by default.
**Why it happens:** Probe loop iterates `state.pods` which only has registered pods.
**How to avoid:** This is correct behavior — if rc-agent never connected, we truly don't know the IP. The endpoint renders it as "Offline" with pod number visible but no other data.

### Pitfall 5: Uptime resets on every rc-agent restart
**What goes wrong:** After a watchdog restart, `uptime_secs` resets to near-0. This might look alarming.
**Why it happens:** `uptime_secs` in `StartupReport` is `agent_start_time.elapsed().as_secs()` — agent process uptime, not system uptime.
**How to avoid:** Label it "Agent uptime" in the dashboard, not "System uptime". A low uptime after a known deploy is expected and informative.

## Code Examples

### GET :8090/health response (verified in remote_ops.rs:93-103)
```json
{
  "status": "ok",
  "version": "0.5.2",
  "uptime_secs": 3847,
  "exec_slots_available": 4,
  "exec_slots_total": 4
}
```
Returns HTTP 200. The fleet probe only needs the 200 status — version/uptime come from `StartupReport` via the map.

### StartupReport protocol (verified in protocol.rs:100-108)
```rust
AgentMessage::StartupReport {
    pod_id: String,       // e.g. "pod_3"
    version: String,      // e.g. "0.5.2"
    uptime_secs: u64,     // agent process uptime since last start
    config_hash: String,  // SHA of config file (for change detection)
    crash_recovery: bool, // true if watchdog restarted agent after crash
    repairs: Vec<String>, // self-heal actions taken at startup
}
```

### WS connection check (verified in deploy.rs:280-286)
```rust
// Direct copy from is_ws_connected() in deploy.rs
let senders = state.agent_senders.read().await;
let ws_connected = match senders.get(pod_id) {
    Some(sender) => !sender.is_closed(),
    None => false,
};
```

### Pod IP from state.pods (verified in deploy.rs:783)
```rust
// Pattern from deploy_rolling_fleet in deploy.rs
let pods = state.pods.read().await;
let pod_ip = pods.get(&pod_id).map(|p| p.ip_address.clone());
```

### Fleet endpoint route registration (follow routes.rs pattern at line 32)
```rust
// Add in api/routes.rs Router::new() block
.route("/fleet/health", get(fleet_health))
```

### Frontend TypeScript type
```typescript
// Add to kiosk/src/lib/types.ts
export interface PodFleetStatus {
  pod_number: number;
  pod_id: string | null;
  ws_connected: boolean;
  http_reachable: boolean;
  version: string | null;
  uptime_secs: number | null;
  crash_recovery: boolean | null;
  ip_address: string | null;
  last_startup_report_at: string | null; // ISO-8601
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate pod-agent binary | pod-agent merged into rc-agent at port 8090 | Phase 13.1 (eea644e) | Single binary per pod, same port for exec and health probes |
| No WS exec fallback | HTTP first, WS fallback | Phase 17 | deploy.rs pattern available to copy |
| No version tracking | StartupReport protocol (HEAL-02) | Phase 18 | version + uptime available after each agent connect |
| Manual fleet status check | Fleet dashboard (this phase) | Phase 21 | Uday sees 8-pod status without SSH |

**Deprecated/outdated:**
- Separate pod-agent binary: gone since Phase 13.1 — do not look for it
- Port 8090 as a "pod-agent" port: still the port, but now part of rc-agent (remote_ops.rs)

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + tsc (TypeScript) |
| Config file | Cargo.toml per-crate / kiosk/tsconfig.json |
| Quick run command | `cargo test -p racecontrol-crate -- fleet` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FLEET-01 | `GET /api/v1/fleet/health` returns exactly 8 entries | unit | `cargo test -p racecontrol-crate -- fleet_health_returns_8_pods` | No — Wave 0 |
| FLEET-01 | All fields present (ws_connected, http_reachable, version, uptime) | unit | `cargo test -p racecontrol-crate -- fleet_health_fields` | No — Wave 0 |
| FLEET-02 | ws_connected reflects agent_senders state independently of http_reachable | unit | `cargo test -p racecontrol-crate -- fleet_ws_independent_of_http` | No — Wave 0 |
| FLEET-02 | http_reachable reflects pod_http_reachable map independently of WS | unit | `cargo test -p racecontrol-crate -- fleet_http_independent_of_ws` | No — Wave 0 |
| FLEET-03 | Fleet page TypeScript compiles without errors | smoke | `cd /c/Users/bono/racingpoint/racecontrol/kiosk && npx tsc --noEmit` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate -- fleet`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/fleet_health.rs` — new module with `PodStartupData`, `PodFleetStatus`, `start_probe_loop`, `fleet_health` handler, and all unit tests
- [ ] `kiosk/src/app/fleet/page.tsx` — new Next.js page
- [ ] `PodFleetStatus` TypeScript interface in `kiosk/src/lib/types.ts`

*(No new test framework install needed — cargo test and tsc already present)*

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/remote_ops.rs:93-103` — verified `GET /health` returns 200 JSON with version, uptime_secs, exec_slots; port 8090
- `crates/racecontrol/src/state.rs` — verified AppState fields: `pods`, `agent_senders`; confirmed NO `pod_startup_reports` field exists yet
- `crates/racecontrol/src/config.rs` — verified `PodsConfig` has `count`, `discovery`, `static_pods` (no per-pod IPs)
- `crates/rc-common/src/protocol.rs:100-108` — verified `StartupReport` variant fields
- `crates/rc-common/src/types.rs:51-73` — verified `PodInfo.ip_address: String`, `PodInfo.number: u32`
- `crates/racecontrol/src/ws/mod.rs:481-501` — verified StartupReport handler logs only, no AppState write
- `crates/racecontrol/src/deploy.rs:280-286` — verified `is_ws_connected()` pattern using agent_senders
- `crates/racecontrol/src/deploy.rs:30` — verified `POD_AGENT_PORT = 8090`
- `crates/rc-agent/src/main.rs:424` — verified rc-agent starts remote_ops on port 8090
- `kiosk/src/hooks/useKioskSocket.ts` — verified WS URL pattern and event handling approach
- `kiosk/next.config.ts` — verified `basePath: "/kiosk"`, port 3300 in package.json
- `kiosk/package.json` — verified Next.js 16.1.6, React 19, Tailwind 4, TypeScript 5.9.3
- `kiosk/src/app/globals.css` — verified rp-* color tokens available
- `kiosk/src/lib/api.ts` — verified `API_BASE` pattern using `window.location.hostname`
- `racecontrol.toml` — verified `pods.count = 8`, no `[[pods.static]]` entries (IPs come from agent registration only)

### Secondary (MEDIUM confidence)
- ROADMAP.md Phase 21 description — intent and success criteria (URL discrepancy noted in Pitfall 3)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all deps already in use, verified from package.json and Cargo files
- Architecture: HIGH — data structures verified from actual source, patterns copied from deploy.rs
- Pitfalls: HIGH — confirmed from source code, not guessed
- Frontend patterns: HIGH — verified from existing kiosk pages

**Research date:** 2026-03-15
**Valid until:** 2026-04-15
