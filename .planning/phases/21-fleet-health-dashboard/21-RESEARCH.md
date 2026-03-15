# Phase 21: Fleet Health Dashboard - Research

**Researched:** 2026-03-15
**Domain:** Next.js 16 + Axum — read-only status dashboard, per-pod health data aggregation
**Confidence:** HIGH

## Summary

Phase 21 is the final phase of v4.0. Its goal is a read-only, mobile-first page at `/fleet` that
shows Uday the real-time health of all 8 pods — WebSocket connection status, HTTP reachability,
agent version, and uptime — without requiring any login. This is a pure presentation layer on top
of data that Phases 16–20 already produce.

The key architectural insight: **all required health data already exists in AppState today**, but
two fields are missing. `agent_senders` tracks WS-connected pods. HTTP reachability requires a
new periodic probe (one GET to `http://{pod_ip}:8090/health`). The `StartupReport` message carries
`version` and `uptime_secs`, but those values are currently only logged — they must be stored in a
new `pod_startup_reports` map in `AppState`.

The frontend is a new Next.js page `kiosk/src/app/fleet/page.tsx` that:
1. Polls `GET /api/v1/fleet/health` every 5 seconds (or receives a new `DashboardEvent::FleetHealth`
   push — polling is simpler, adequate, and zero risk to existing WS protocol).
2. Renders an 8-card grid optimised for a phone screen (single column on mobile, 2-col on tablet+).

**Primary recommendation:** Add a `pod_fleet_health` state map to `AppState`, populate it from
`StartupReport` + periodic HTTP probe + WS-connected check, expose via `GET /api/v1/fleet/health`,
and build a standalone `/fleet` page in the kiosk app.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FLEET-01 | Kiosk /fleet page shows all 8 pods with real-time status (WS connected, HTTP reachable, version, uptime) | New Next.js page `kiosk/src/app/fleet/page.tsx` polling a new `GET /api/v1/fleet/health` endpoint |
| FLEET-02 | Pod status distinguishes WS-connected vs HTTP-reachable as separate indicators | Two boolean fields (`ws_connected`, `http_reachable`) in the API response; distinct visual treatment in the card |
| FLEET-03 | Dashboard accessible from Uday's phone, no login required, loads within 3 seconds | Mobile-first single-column Tailwind grid; endpoint is public (no auth guard); `last_seen` data is already in-memory |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Next.js | 16.1.6 (already in use) | `/fleet` page | Existing kiosk app — same framework, same dev server on :3300 |
| React | 19.2.3 (already in use) | UI components | Existing choice |
| Tailwind CSS | 4.x (already in use) | Styling | All existing kiosk pages use it |
| Axum | 0.8 (already in use) | New `/fleet/health` endpoint | Existing backend framework |
| reqwest | 0.12 (already in use) | HTTP probe to pod :8090 | Already in rc-core `AppState.http_client` |

### No New Dependencies Required
This phase adds zero new crates and zero new npm packages. Everything needed is already present.

---

## Architecture Patterns

### New Data: `PodFleetHealth` in AppState

The key gap: `StartupReport` data (version, uptime_secs) is currently logged and discarded. A new
map must store it.

```rust
// In crates/rc-common/src/types.rs (or rc-core/src/state.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodFleetHealth {
    pub pod_id: String,
    pub pod_number: u32,
    pub pod_ip: String,
    pub ws_connected: bool,         // agent_senders contains pod_id and sender is not closed
    pub http_reachable: bool,       // last HTTP probe to :8090/health succeeded
    pub agent_version: Option<String>,  // from StartupReport.version
    pub uptime_secs: Option<u64>,   // from StartupReport.uptime_secs (time since last start)
    pub last_seen: Option<DateTime<Utc>>,  // last heartbeat timestamp from PodInfo.last_seen
    pub last_http_check: Option<DateTime<Utc>>,  // when HTTP probe last ran
}
```

Add to `AppState`:
```rust
pub pod_fleet_health: RwLock<HashMap<String, PodFleetHealth>>,
```

### Where to Update `ws_connected`

- **On `AgentMessage::Register`** → set `ws_connected = true`
- **On `AgentMessage::StartupReport`** → store `agent_version` and `uptime_secs`
- **On agent disconnect** (socket close / `AgentMessage::Disconnect`) → set `ws_connected = false`, clear `agent_version` and `uptime_secs`

This is in `crates/rc-core/src/ws/mod.rs` `handle_agent()`.

### HTTP Reachability Probe Loop

A new background task (spawned in `main.rs`) probes each pod's HTTP endpoint every 15 seconds:

```rust
// In crates/rc-core/src/fleet_health.rs (new module)
pub async fn probe_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    loop {
        interval.tick().await;
        let pod_configs = state.config.pods.list.clone(); // from racecontrol.toml
        for pod in &pod_configs {
            let url = format!("http://{}:8090/health", pod.ip);
            let reachable = state.http_client
                .get(&url)
                .timeout(Duration::from_secs(3))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            // Update pod_fleet_health map
        }
    }
}
```

**Timeout: 3 seconds per probe.** With 8 pods and sequential probing, worst case is 24 seconds.
Use `tokio::join_all` or `futures::future::join_all` to probe all 8 in parallel — reduces to 3s.

### New API Endpoint

```
GET /api/v1/fleet/health
```

No auth required (like `/api/v1/public/*` endpoints). Returns:

```json
{
  "pods": [
    {
      "pod_id": "pod_1",
      "pod_number": 1,
      "pod_ip": "192.168.31.89",
      "ws_connected": true,
      "http_reachable": true,
      "agent_version": "0.5.2",
      "uptime_secs": 3600,
      "last_seen": "2026-03-15T11:30:00Z",
      "last_http_check": "2026-03-15T11:30:10Z"
    },
    ...
  ],
  "timestamp": "2026-03-15T11:30:15Z"
}
```

### Frontend: `/fleet` Page

New file: `kiosk/src/app/fleet/page.tsx`

**Polling approach** (simpler than WS for a status page):
```typescript
// "use client"
// useEffect: fetch /api/v1/fleet/health every 5000ms
// setInterval + cleanup on unmount
```

The kiosk runs on :3300 and the API on :8080. The existing proxy in `main.rs` only proxies
`/kiosk*` and `/_next/*`. The `/fleet` page calls the API at `http://192.168.31.23:8080/api/v1/fleet/health`
or uses `window.location.hostname:8080`.

The lib/api.ts already has a pattern for API calls — add `getFleetHealth()`.

### Pod Card Layout (Mobile-First)

```
┌─────────────────────────────────────┐
│ POD 1                    v0.5.2      │
│ ● WS Connected           ↑ 2h 15m   │
│ ● HTTP Reachable                     │
└─────────────────────────────────────┘
```

Status combinations and visual treatment:
| WS | HTTP | Card appearance |
|----|------|-----------------|
| true | true | Green border — fully healthy |
| true | false | Yellow border — WS up, HTTP blocked |
| false | true | Orange border — not yet seen (HTTP up = process alive but not connected) |
| false | false | Red border / dim — offline |

CSS grid: `grid-cols-1 sm:grid-cols-2` — single column on phone (< 640px), 2 columns on tablet.

### Recommended File Structure

```
crates/rc-core/src/
├── fleet_health.rs         # NEW: PodFleetHealth struct, probe_loop(), AppState init
├── state.rs                # ADD: pod_fleet_health field
├── ws/mod.rs               # MODIFY: store StartupReport data, update ws_connected
├── api/routes.rs           # ADD: GET /fleet/health handler + route

kiosk/src/
├── app/fleet/
│   └── page.tsx            # NEW: Fleet Health Dashboard page
├── lib/
│   ├── api.ts              # ADD: getFleetHealth()
│   └── types.ts            # ADD: PodFleetHealth interface
```

### Anti-Patterns to Avoid

- **Don't reuse the WS dashboard socket for this page.** The /fleet page is a read-only status
  view for an ops user on a phone. Polling every 5 seconds is sufficient and has zero risk of
  breaking existing WS protocol.
- **Don't add version/uptime fields to `PodInfo`.** `PodInfo` is in rc-common and used everywhere
  (billing, game launcher, etc.). Fleet health is ops-only data — it belongs in a separate map.
- **Don't block the probe loop on slow pods.** Use `tokio::join_all` so one unreachable pod
  (3s timeout) doesn't delay the others.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP probe to pod | Custom TCP socket | `state.http_client.get(url)` with timeout | Already in AppState; handles redirects, connection errors, timeouts cleanly |
| Uptime formatting | Custom duration formatter | Simple arithmetic in TypeScript | `Math.floor(secs / 3600)h Math.floor((secs % 3600) / 60)m` — 2 lines |
| Status icons | SVG from scratch | Tailwind `rounded-full` color dots (same pattern as existing kiosk header) | Existing kiosk uses `w-2 h-2 rounded-full bg-green-500` — reuse the pattern |
| Auth guard | JWT middleware | None — `/fleet/health` is intentionally public | It shows version strings and IPs; not sensitive for Uday's LAN-only use case |

---

## Common Pitfalls

### Pitfall 1: `agent_senders` Key Not Present vs Sender Closed
**What goes wrong:** A pod that never connected has no key in `agent_senders`. A pod that
disconnected has a key but a closed sender. Both should show `ws_connected = false`.
**How to avoid:** `ws_connected = senders.get(pod_id).map(|s| !s.is_closed()).unwrap_or(false)`
**This pattern already exists** in `deploy.rs`'s `is_ws_connected()` — copy it exactly.

### Pitfall 2: Pod Numbers vs Pod IDs
**What goes wrong:** `AppState.pods` is keyed by pod_id (e.g. `"pod_1"`). The fleet health map
must also key by pod_id. The kiosk expects to display pods 1–8 sorted by `pod_number`.
**How to avoid:** In the API response, sort by `pod_number`. For empty slots (pods that have
never connected), create placeholder entries with `ws_connected: false, http_reachable: false`.

### Pitfall 3: Pod IP Source
**What goes wrong:** Pod IPs aren't always in `AppState.pods` on first load. `PodInfo.ip_address`
is populated from the `Register` message, but before that it comes from `racecontrol.toml`.
**How to avoid:** Use `state.config.pods.list` as the authoritative source for the 8 pod IPs.
Merge with `pods` map data at query time.

### Pitfall 4: `uptime_secs` Is Snapshot-at-Connect, Not Live Counter
**What goes wrong:** `StartupReport.uptime_secs` is the uptime at the moment the agent connected.
Displaying it verbatim will show a stale number after the agent has been running for hours.
**How to avoid:** Store `started_at = Utc::now() - Duration::from_secs(uptime_secs)` when the
StartupReport arrives. Compute live uptime as `Utc::now() - started_at` in the API response.

### Pitfall 5: CORS / Proxy on `/fleet`
**What goes wrong:** The kiosk proxy in `main.rs` only forwards `/kiosk*` and `/_next/*`. The
`/fleet` page is served by Next.js at `:3300` — but Uday opens `http://192.168.31.23:3300/fleet`,
not through the rc-core proxy.
**This is fine.** Uday should use `:3300` directly. The `/fleet` page calls the API at `:8080`.
The CORS config in `main.rs` already allows `192.168.31.*` origins.

### Pitfall 6: Version Inconsistency (Pre-Existing)
**State.md note:** "USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2".
**Impact:** The fleet dashboard will show this discrepancy accurately — which is actually
*useful* for Uday (he can see which pods need a fresh deploy). Don't paper over this.

---

## Code Examples

### Pattern: Reading ws_connected (from deploy.rs)
```rust
// Source: crates/rc-core/src/deploy.rs:is_ws_connected()
async fn is_ws_connected(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}
```

### Pattern: Background loop spawn (from main.rs)
```rust
// Source: crates/rc-core/src/main.rs — billing tick loop pattern
let fleet_state = state.clone();
tokio::spawn(async move {
    fleet_health::probe_loop(fleet_state).await;
});
```

### Pattern: Parallel HTTP probes with join_all
```rust
// Source: standard Tokio pattern — use futures::future::join_all
// futures-util is already in rc-core Cargo.toml
use futures_util::future::join_all;

let probes: Vec<_> = pod_configs.iter().map(|pod| {
    let client = state.http_client.clone();
    let url = format!("http://{}:8090/health", pod.ip);
    async move {
        client
            .get(&url)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}).collect();
let results = join_all(probes).await;
```

### Pattern: Public route (no auth)
```rust
// Source: crates/rc-core/src/api/routes.rs — /api/v1/public/* routes
// No auth extractor — just State(state) — same as public_leaderboard()
.route("/fleet/health", get(fleet_health_handler))
```

### Pattern: Kiosk page with polling
```typescript
// Source: pattern consistent with kiosk/src/app/page.tsx
"use client";
import { useState, useEffect } from "react";

export default function FleetDashboard() {
  const [pods, setPods] = useState<PodFleetHealth[]>([]);

  useEffect(() => {
    async function fetchHealth() {
      const res = await fetch(
        `http://${window.location.hostname}:8080/api/v1/fleet/health`
      );
      const data = await res.json();
      setPods(data.pods ?? []);
    }
    fetchHealth();
    const id = setInterval(fetchHealth, 5000);
    return () => clearInterval(id);
  }, []);

  // ... render
}
```

### Pattern: Uptime display (computed from stored start time)
```rust
// In fleet_health handler — compute live uptime at query time
let uptime_secs = started_at.map(|t| {
    (Utc::now() - t).num_seconds().max(0) as u64
});
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| StartupReport data logged and discarded | Store in `pod_fleet_health` map | Version and uptime become queryable |
| HTTP reachability = "can exec via pod-agent" | Periodic 3s probe to :8090/health | Independent of exec slots; shows firewall state |
| Fleet state checked by running commands | Read-only API + visual dashboard | Zero-command ops visibility for Uday |

**Not applicable (nothing deprecated in this domain for this phase).**

---

## Open Questions

1. **Pod IP source when no pods are registered yet**
   - What we know: `racecontrol.toml` has static pod IPs; `AppState.pods` only has IPs after Register
   - What's unclear: Does `state.config.pods.list` exist as a structured list, or is it just a count?
   - Recommendation: Check `crates/rc-core/src/config.rs` during Plan phase. If it's only a count,
     derive IPs from the network map (192.168.31.{89,33,28,88,86,87,38,91}) as a fallback constant.

2. **HTTP probe endpoint on pod-agent**
   - What we know: pod-agent was merged into rc-agent (Phase 13.1). rc-agent serves HTTP on port 8090.
   - What's unclear: Does `GET /health` on port 8090 return 200? Or does it need a different path?
   - Recommendation: Check `crates/rc-agent/src/remote_ops.rs` during Plan phase. If no `/health`
     endpoint exists, probe `/` or use a TCP connect check instead.

3. **`started_at` vs `uptime_secs` storage granularity**
   - What we know: `StartupReport.uptime_secs` is a u64 count-up from process start
   - What's unclear: Whether to store the raw `uptime_secs` + arrival timestamp, or a computed
     `started_at: DateTime<Utc>`
   - Recommendation: Store `agent_started_at: Option<DateTime<Utc>>` = `Utc::now() - uptime_secs`.
     Simpler to compute live uptime in the response handler.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | None (workspace Cargo.toml) |
| Quick run command | `cargo test -p rc-core` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FLEET-01 | `/fleet/health` returns 8 pod entries sorted by number | unit | `cargo test -p rc-core fleet_health` | Wave 0 |
| FLEET-01 | Placeholder entries created for pods never seen | unit | `cargo test -p rc-core fleet_health_missing_pods` | Wave 0 |
| FLEET-02 | WS-up + HTTP-blocked pod has distinct fields | unit | `cargo test -p rc-core fleet_health_ws_only` | Wave 0 |
| FLEET-02 | `ws_connected` uses sender.is_closed() correctly | unit | `cargo test -p rc-core fleet_health_ws_connected` | Wave 0 |
| FLEET-03 | `/fleet/health` requires no auth header | unit | `cargo test -p rc-core fleet_health_no_auth` | Wave 0 |
| FLEET-03 | Page loads < 3s — manual verification | manual | Visual check on Uday's phone | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-core`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-core/src/fleet_health.rs` — module with `PodFleetHealth` struct and unit tests
- [ ] Tests for `fleet_health_handler` in `routes.rs` (same pattern as `pod_status_summary_tests`)
- [ ] TypeScript: No test framework in kiosk — frontend verified manually on phone

*(Existing test infrastructure in `crates/rc-core/src/state.rs` and `crates/rc-core/tests/integration.rs` covers the data layer pattern; new fleet_health tests follow the same `#[cfg(test)]` inline pattern.)*

---

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/rc-core/src/state.rs` — AppState structure, pod_deploy_states pattern, is_ws_connected() in deploy.rs
- Codebase: `crates/rc-common/src/protocol.rs` — StartupReport message (version, uptime_secs fields)
- Codebase: `crates/rc-core/src/ws/mod.rs` — where StartupReport is received (lines 481–501); currently logs only
- Codebase: `crates/rc-core/src/api/routes.rs` — existing patterns (pod_status_summary, public routes, deploy_status)
- Codebase: `crates/rc-core/src/main.rs` — background loop spawn pattern, CORS config (192.168.31.*)
- Codebase: `kiosk/src/hooks/useKioskSocket.ts` — how dashboard receives events (WS pattern with deploy_progress)
- Codebase: `kiosk/src/lib/types.ts` — TypeScript type conventions for the project
- Codebase: `kiosk/src/app/page.tsx` — existing pod grid layout, Tailwind classes, color tokens

### Secondary (MEDIUM confidence)
- Project MEMORY.md — Network map (pod IPs), brand colors (#E10600 red, #1A1A1A black, #5A5A5A grey)
- REQUIREMENTS.md — FLEET-01, FLEET-02, FLEET-03 definitions (confirmed implementation requirements)
- STATE.md — Known issue: version string inconsistency (v0.1.0 vs v0.5.2) expected behavior

### Tertiary (LOW confidence)
- None — all findings are codebase-verified

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new deps; everything confirmed in package.json and Cargo.toml
- Architecture: HIGH — all patterns copied from existing, working code in the same repo
- Pitfalls: HIGH — specifically identified from actual codebase state (StartupReport not stored, sender key gap)
- Open questions: MEDIUM — config.rs and remote_ops.rs not read; planner should check these in Wave 0

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable codebase, no external API dependencies)
