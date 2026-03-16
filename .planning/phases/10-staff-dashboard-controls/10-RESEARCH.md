# Phase 10: Staff Dashboard Controls - Research

**Researched:** 2026-03-14
**Domain:** Pod power management + kiosk lockdown controls (Rust/Axum backend + Next.js frontend)
**Confidence:** HIGH

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KIOSK-01 | Staff can toggle full pod lockdown (taskbar, Win key, Edge kiosk) on or off for a specific pod from the staff kiosk dashboard | `kiosk_lockdown_enabled` setting already propagated via `SettingsUpdated` → `KioskManager.activate()/deactivate()` — needs dedicated API route + UI |
| KIOSK-02 | Staff can lock or unlock all 8 pods at once from the staff kiosk dashboard | Extend existing settings broadcast with `kiosk_lockdown_enabled=true/false` to all agents simultaneously |
| PWR-01 | Staff can power off a specific pod remotely from the staff kiosk dashboard | `POST /pods/{id}/shutdown` already exists in racecontrol; button exists in `/control` page |
| PWR-02 | Staff can restart a specific pod remotely from the staff kiosk dashboard | `POST /pods/{id}/restart` already exists; button exists in `/control` page |
| PWR-03 | Staff can power on a specific pod remotely from the staff kiosk dashboard (Wake-on-LAN) | `POST /pods/{id}/wake` already exists; WoL button exists in `/control` page |
| PWR-04 | Staff can power off all 8 pods at once from the staff kiosk dashboard | `POST /pods/shutdown-all` already exists; button in `/control` page; not yet in `/staff` page |
| PWR-05 | Staff can restart all 8 pods at once from the staff kiosk dashboard | `POST /pods/restart-all` already exists in racecontrol, but NOT wired to `api.ts` or `/control` UI yet |
| PWR-06 | Staff can power on all 8 pods at once from the staff kiosk dashboard (Wake-on-LAN) | `POST /pods/wake-all` already exists; button in `/control` page |
</phase_requirements>

## Summary

Phase 10 requires adding pod power management and kiosk lockdown controls to the staff kiosk dashboard. The backend infrastructure for power commands (shutdown, restart, Wake-on-LAN) is **already fully implemented** in `crates/racecontrol/src/wol.rs` and exposed as HTTP routes. The kiosk lockdown mechanism (`kiosk_lockdown_enabled` setting → `SettingsUpdated` → `KioskManager.activate()/deactivate()`) is **already wired end-to-end** through the existing settings broadcast path. What is missing is: (1) dedicated API routes for lockdown toggle (per-pod and all-pods), (2) `restartAllPods` missing from `api.ts`, and (3) the `/staff` page's 8-pod grid lacks per-pod power buttons and any lockdown controls.

The `/control` page already has per-pod shutdown/restart/wake buttons and bulk wake/shutdown buttons. However, `/control` is a separate page from the main `/staff` terminal. The phase goal is to surface these controls in the staff dashboard (wherever staff actually operate from). The simplest path is to add a "Pod Controls" panel to the existing `/staff` page and/or link from `/staff` to the already-built `/control` page and complete the missing functionality there.

**Primary recommendation:** Build a dedicated `PodControlPanel` component that works in both the `/staff` and `/control` pages. Add the two missing lockdown API routes (`POST /pods/{id}/lockdown` and `POST /pods/lockdown-all`) in racecontrol, add `restartAllPods` to `api.ts`, and expose all 8 controls (wake, shutdown, restart per-pod; wake-all, shutdown-all, restart-all, lock-all, unlock-all) with confirmation dialogs and visible status feedback.

---

## Standard Stack

### Core (already in project)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust/Axum | project standard | HTTP routes for pod commands | All racecontrol API routes use Axum |
| reqwest | project standard | HTTP client for pod-agent calls | Used by `wol.rs` shutdown/restart |
| tokio::net::UdpSocket | std | WoL magic packet broadcast | Used by `wol::send_wol` |
| Next.js (App Router) | project standard | Kiosk frontend | `/staff` and `/control` pages |
| Tailwind CSS | project standard | Styling | All kiosk UI uses Tailwind |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | project standard | JSON for pod-agent exec payload | Used by all existing wol.rs calls |

### What Already Exists (do NOT rebuild)

| Capability | File | Status |
|------------|------|--------|
| WoL magic packet | `crates/racecontrol/src/wol.rs:send_wol()` | COMPLETE |
| Pod shutdown via pod-agent | `wol.rs:shutdown_pod()` | COMPLETE |
| Pod restart via pod-agent | `wol.rs:restart_pod()` | COMPLETE |
| HTTP routes: per-pod wake/shutdown/restart | `api/routes.rs` lines 35-40 | COMPLETE |
| HTTP routes: wake-all/shutdown-all/restart-all | `api/routes.rs` lines 41-43 | COMPLETE |
| Kiosk lockdown: activate/deactivate | `rc-agent/src/kiosk.rs` | COMPLETE |
| Lockdown toggle via settings key | `rc-agent/src/main.rs:1476-1483` | COMPLETE - reacts to `kiosk_lockdown_enabled=true/false` in `SettingsUpdated` |
| Settings broadcast to all agents | `racecontrol/src/state.rs:broadcast_settings()` | COMPLETE |
| Settings persistence (SQLite `kiosk_settings`) | `api/routes.rs:update_kiosk_settings()` | COMPLETE |
| Per-pod power buttons (wake/restart/shutdown) | `kiosk/src/app/control/page.tsx` | COMPLETE (in `/control` page) |
| Bulk wake/shutdown buttons | `kiosk/src/app/control/page.tsx` | COMPLETE (in `/control` page) |

### What is Missing

| Capability | Gap |
|------------|-----|
| Per-pod lockdown toggle API route | No `POST /pods/{id}/lockdown` route exists |
| Bulk lockdown-all / unlock-all API route | No `POST /pods/lockdown-all` route exists |
| `restartAllPods` in `kiosk/src/lib/api.ts` | Missing — route `/pods/restart-all` exists in racecontrol but not wired to frontend |
| Lockdown toggle buttons in UI | Not in `/control` or `/staff` page |
| Restart-all button in `/control` page | Missing from bulk controls row |
| Pod status confirmation after power commands | Pod goes Offline after shutdown — status visible via WebSocket but no explicit feedback toast |

---

## Architecture Patterns

### How Lockdown Toggle Works (existing path)

```
Staff clicks "Lock Pod N"
    → frontend: PUT /api/kiosk/settings { "kiosk_lockdown_enabled": "true" }  ← current path (global)
    → racecontrol: update_kiosk_settings() writes to SQLite kiosk_settings
    → state.broadcast_settings() iterates agent_senders, sends CoreToAgentMessage::SettingsUpdated
    → rc-agent main.rs:1476 matches "kiosk_lockdown_enabled" == "true" → kiosk.activate()
```

The **current settings mechanism is global** — `broadcast_settings` sends to ALL agents. For KIOSK-01 (per-pod lockdown), we need a targeted send to a single agent.

### Per-Pod Lockdown Toggle (new pattern needed)

Two viable approaches:

**Option A: New dedicated route `POST /pods/{id}/lockdown`**
- racecontrol adds a Rust handler that looks up the pod's `agent_senders` entry and sends `CoreToAgentMessage::SettingsUpdated` with `kiosk_lockdown_enabled` to only that pod's sender channel
- Clean, explicit, does not touch `kiosk_settings` DB (ephemeral command, not persisted per-pod)
- Recommended

**Option B: Extend `kiosk_settings` with per-pod key**
- Key format: `kiosk_lockdown_pod_1=true`, `kiosk_lockdown_pod_2=false`
- `broadcast_settings` would need to parse per-pod keys and route correctly
- More complex, persists across restarts (could be a feature or a bug)
- Not recommended (over-engineered for this use case)

**Decision: Use Option A** — a dedicated route that sends `CoreToAgentMessage::SettingsUpdated { settings: {"kiosk_lockdown_enabled": "true"/"false"} }` directly to the target pod's WebSocket channel. Does not persist to DB (lockdown resets to default on rc-agent restart, which is safe behavior).

### All-Pods Lockdown (KIOSK-02)

Two sub-actions: lock-all and unlock-all.

```
POST /pods/lockdown-all        → body: { "locked": true }  or false
    → iterate all agent_senders
    → send SettingsUpdated { "kiosk_lockdown_enabled": "true" } to each
```

This is equivalent to what `broadcast_settings` already does, but scoped to only the lockdown key and without DB persistence.

### Power Command Confirmation (success criteria #3)

After sending shutdown/restart/wake, staff need to see the pod status change. The existing WebSocket `DashboardEvent::PodUpdate` flow already handles this:
- Shutdown: pod-agent receives `shutdown /s /f /t 0`, pod goes offline, racecontrol marks `PodStatus::Disabled`, broadcasts `PodUpdate`
- Restart: similar — pod goes offline briefly, returns as `Idle` when rc-agent reconnects
- Wake: WoL packet sent, pod will appear as `Offline` until it boots and rc-agent connects

The `/staff` page already subscribes to `PodUpdate` via `useKioskSocket`. Pod cards already reflect `PodStatus::Offline` / `PodStatus::Disabled` / `PodStatus::Idle`. So status confirmation is automatic — no additional code needed for the WebSocket layer.

### Recommended Project Structure for New Files

```
crates/racecontrol/src/api/routes.rs     — add 2 new Axum route functions (lockdown_pod, lockdown_all_pods)
kiosk/src/lib/api.ts                 — add lockdownPod(), lockdownAllPods(), restartAllPods()
kiosk/src/app/control/page.tsx       — add lockdown buttons, restart-all button
kiosk/src/app/staff/page.tsx         — add "Pod Controls" section or link to /control
kiosk/src/components/PodControlBar.tsx  — (optional) shared component for per-pod power+lockdown row
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WoL magic packet | Custom UDP broadcast | `wol::send_wol()` already in wol.rs | Already handles broadcast addr, 102-byte packet format |
| Shutdown/restart via OS | winapi calls or registry | `shutdown_pod()`/`restart_pod()` via pod-agent `/exec` | pod-agent already runs as SYSTEM, handles `shutdown /s /f /t 0` |
| Kiosk mode toggle | Custom process/registry manipulation | `KioskManager.activate()/deactivate()` + `SettingsUpdated` | Full Win32 hook (taskbar, keyboard hook) already in `kiosk.rs` |
| Broadcast to all agents | Manual sender loop | `state.broadcast_settings()` | Already handles per-pod billing guard and screen blanking |

---

## Common Pitfalls

### Pitfall 1: Route Registration Order (Axum static vs dynamic segments)

**What goes wrong:** In Axum, `POST /pods/lockdown-all` and `POST /pods/{id}/lockdown` can conflict if the static segment `lockdown-all` is registered AFTER the dynamic `{id}` route. Axum matches in registration order.

**Why it happens:** `/pods/lockdown-all` would match `{id}` = "lockdown-all" if `/pods/{id}/...` is registered first.

**How to avoid:** Register all static path routes (`/pods/wake-all`, `/pods/shutdown-all`, `/pods/restart-all`, `/pods/lockdown-all`) **before** the dynamic `{id}` routes. Already done correctly in the existing routes — follow the same pattern. Add `/pods/lockdown-all` in the same block as the other `-all` routes.

**Warning signs:** 405 Method Not Allowed or 404 on the new lockdown routes when hitting from the frontend.

### Pitfall 2: Sending to Disconnected Agent Channels

**What goes wrong:** `agent_senders.get(pod_id)` returns a sender for a pod that has disconnected. Sending to it returns an error that is silently swallowed.

**Why it happens:** `agent_senders` map is not cleaned up immediately on disconnect in all code paths.

**How to avoid:** Use `is_closed()` check before sending (same pattern as `is_ws_alive()` in pod_monitor.rs). For lockdown toggle, return `{ "error": "pod not connected" }` if sender is missing or closed, not a silent success.

### Pitfall 3: Lockdown Persisting After rc-agent Restart

**What goes wrong:** If lockdown is toggled via the new route (ephemeral, no DB write), rc-agent restart will reset kiosk to its default state from `kiosk.toml` config (`enabled: true`). Staff may expect the remote lockdown toggle to survive a restart.

**Why it happens:** The `SettingsUpdated` approach is in-memory only — not persisted.

**How to avoid:** Decide and document: lockdown toggle via dashboard is ephemeral (resets to config default on restart). This is the **correct behavior** for Phase 10 — rc-agent starts locked by default anyway (`kiosk.enabled = true`). If persistent per-pod lockdown state is ever needed, that's a future feature.

### Pitfall 4: Locking a Pod During Active Billing

**What goes wrong:** Staff locks a pod that has an active billing session. The taskbar and keyboard hooks engage while a game is running, which may block game controls that use Win key.

**Why it happens:** `KioskManager.activate()` installs the keyboard hook unconditionally.

**How to avoid:** racecontrol's lockdown route should check if pod has active billing (`state.billing.active_timers.read().await.contains_key(&pod_id)`) and return an error or warning. Do NOT activate kiosk on a pod with active billing. This mirrors the watchdog pattern in `pod_monitor.rs:274`.

### Pitfall 5: Restart-All Without Guard

**What goes wrong:** `restart_all_pods` sends restart to every online pod, including pods with active billing sessions — disrupting customers mid-race.

**Why it happens:** Current `restart_all_pods` in routes.rs skips only `Offline` and `Disabled` pods (line 560), not pods with active billing.

**How to avoid:** For Phase 10, the restart-all/shutdown-all operations should display a strong confirmation dialog ("This will restart ALL pods including active sessions"). The backend already accepts the command — the protection is a UX gate. Optionally add a billing guard in `shutdown_all_pods` and `restart_all_pods` on the backend, but that is scope-creep for Phase 10.

### Pitfall 6: `window.confirm()` on Kiosk Display

**What goes wrong:** `window.confirm()` dialogs (already used in `/control/page.tsx`) block the JS event loop and look out of place on a kiosk touchscreen.

**Why it happens:** Existing code uses `window.confirm()` for shutdown confirmation.

**How to avoid:** Phase 10 can keep `window.confirm()` (it already works on desktop browser). If Uday later wants a styled modal, that is Phase 11/polish. Do not introduce a modal component library for this phase.

---

## Code Examples

### New racecontrol Axum route: lockdown_pod (per-pod)

```rust
// Source: patterns from routes.rs lines 413-450, state.rs broadcast_settings
// POST /pods/{id}/lockdown — Body: { "locked": true }
async fn lockdown_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Guard: do not lock pods with active billing
    if locked && state.billing.active_timers.read().await.contains_key(&id) {
        return Json(json!({ "error": "pod has active billing session" }));
    }

    let senders = state.agent_senders.read().await;
    let Some(sender) = senders.get(&id) else {
        return Json(json!({ "error": "pod not connected" }));
    };
    if sender.is_closed() {
        return Json(json!({ "error": "pod not connected" }));
    }

    let mut settings = std::collections::HashMap::new();
    settings.insert(
        "kiosk_lockdown_enabled".to_string(),
        if locked { "true" } else { "false" }.to_string(),
    );
    let msg = CoreToAgentMessage::SettingsUpdated { settings };
    let _ = sender.send(msg).await;

    Json(json!({ "ok": true, "pod_id": id, "locked": locked }))
}
```

### New racecontrol Axum route: lockdown_all_pods

```rust
// Source: patterns from restart_all_pods (routes.rs:555-572)
// POST /pods/lockdown-all — Body: { "locked": true }
async fn lockdown_all_pods(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let active_timers = state.billing.active_timers.read().await;
    let senders = state.agent_senders.read().await;
    let mut results = Vec::new();

    for (pod_id, sender) in senders.iter() {
        if sender.is_closed() {
            results.push(json!({ "pod_id": pod_id, "status": "not_connected" }));
            continue;
        }
        // Skip pods with active billing if locking
        if locked && active_timers.contains_key(pod_id) {
            results.push(json!({ "pod_id": pod_id, "status": "skipped_billing_active" }));
            continue;
        }
        let mut settings = std::collections::HashMap::new();
        settings.insert("kiosk_lockdown_enabled".to_string(), if locked { "true" } else { "false" }.to_string());
        let msg = CoreToAgentMessage::SettingsUpdated { settings };
        let _ = sender.send(msg).await;
        results.push(json!({ "pod_id": pod_id, "status": "sent" }));
    }

    Json(json!({ "ok": true, "locked": locked, "results": results }))
}
```

### Route registration (in router() fn) — insert before {id} routes

```rust
// Source: routes.rs lines 41-43 (existing bulk routes pattern)
// Add these with the other static bulk routes:
.route("/pods/lockdown-all", post(lockdown_all_pods))
// And with the per-pod routes:
.route("/pods/{id}/lockdown", post(lockdown_pod))
```

### Frontend: api.ts additions

```typescript
// Source: kiosk/src/lib/api.ts lines 283-289 (existing power ops pattern)
lockdownPod: (id: string, locked: boolean) =>
  fetchApi<{ ok: boolean; pod_id: string; locked: boolean }>(
    `/pods/${id}/lockdown`,
    { method: "POST", body: JSON.stringify({ locked }) }
  ),

lockdownAllPods: (locked: boolean) =>
  fetchApi<{ ok: boolean; results: unknown[] }>(
    "/pods/lockdown-all",
    { method: "POST", body: JSON.stringify({ locked }) }
  ),

restartAllPods: () =>
  fetchApi<{ status: string; results: unknown[] }>("/pods/restart-all", { method: "POST" }),
```

### Frontend: Lockdown toggle button in /control page

```tsx
// Source: kiosk/src/app/control/page.tsx bulk controls pattern (lines 107-119)
// Add to the bulk controls row:
<button
  onClick={handleLockAll}
  className="px-3 py-1 rounded text-xs font-semibold bg-orange-900/50 text-orange-400 border border-orange-800 hover:bg-orange-800/60 transition-colors"
>
  Lock All
</button>
<button
  onClick={handleUnlockAll}
  className="px-3 py-1 rounded text-xs font-semibold bg-zinc-700 text-zinc-300 border border-zinc-600 hover:bg-zinc-600 transition-colors"
>
  Unlock All
</button>
<button
  onClick={handleRestartAll}
  className="px-3 py-1 rounded text-xs font-semibold bg-yellow-900/50 text-yellow-400 border border-yellow-800 hover:bg-yellow-800/60 transition-colors"
>
  Restart All
</button>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate "Control" page with no lockdown | Extend `/control` page with lockdown | Phase 10 | Unified power + lockdown |
| Global lockdown only (settings broadcast) | Per-pod targeted lockdown (agent_senders lookup) | Phase 10 | KIOSK-01 satisfied |

---

## Open Questions

1. **Should lockdown be surfaced in `/staff` or `/control` or both?**
   - What we know: `/control` already has power buttons; `/staff` is the primary terminal staff use
   - What's unclear: Uday has not specified which page should have the lockdown controls
   - Recommendation: Add bulk Lock All / Unlock All to `/control` page (already has bulk power row). For `/staff`, add a top-bar or footer "Venue" section with Lock All / Unlock All for opening/closing. Per-pod lockdown toggle goes into `/control` page per-pod header (alongside existing wake/restart/shutdown buttons).

2. **Should `POST /pods/{id}/lockdown` persist to DB?**
   - What we know: Ephemeral (in-memory only) is safe — rc-agent default is locked
   - What's unclear: Uday may want the lock state to survive rc-agent crashes
   - Recommendation: Do NOT persist for Phase 10. The DB path (via `kiosk_settings`) adds complexity and risks confusion with the global setting. Document as known limitation.

3. **Should the "Lockdown Active" state be visible in the dashboard?**
   - What we know: `PodInfo` does not currently have a `lockdown_active` field; lockdown state lives only in rc-agent's `KioskManager`
   - What's unclear: Staff may need visual confirmation that lockdown is on/off per pod
   - Recommendation: For Phase 10, show a simple indicator (locked padlock icon) that reflects the last action sent (optimistic UI). No need to add `lockdown_active` to `PodInfo` heartbeat — that is scope for a future phase.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | none — Cargo.toml per crate |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common` |
| Full suite command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KIOSK-01 | `lockdown_pod` route sends `SettingsUpdated` with correct key to single agent | unit | `cargo test -p racecontrol-crate -- lockdown` | ❌ Wave 0 |
| KIOSK-02 | `lockdown_all_pods` route broadcasts to all connected agents, skips disconnected/billing | unit | `cargo test -p racecontrol-crate -- lockdown_all` | ❌ Wave 0 |
| PWR-01 | `shutdown_pod` sends `shutdown /s /f /t 0` via pod-agent | unit (wol.rs) | `cargo test -p racecontrol-crate -- wol` | ❌ Wave 0 (no tests in wol.rs) |
| PWR-02 | `restart_pod` sends `shutdown /r /f /t 0` via pod-agent | unit (wol.rs) | `cargo test -p racecontrol-crate -- wol` | ❌ Wave 0 |
| PWR-03 | `send_wol` sends 102-byte magic packet with correct MAC | unit (wol.rs) | `cargo test -p racecontrol-crate -- wol` | ❌ Wave 0 |
| PWR-04/05/06 | Bulk operations iterate pods, skip Offline/Disabled, send command | unit | `cargo test -p racecontrol-crate -- bulk` | ❌ Wave 0 |

**Note:** The rc-common tests (85 tests) already cover protocol serialization. Protocol changes (if any new `CoreToAgentMessage` variants are needed) would require rc-common tests. For Phase 10, no new protocol messages are needed — `SettingsUpdated` is reused.

### Sampling Rate
- **Per task commit:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common`
- **Per wave merge:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/wol.rs` — add unit tests for `parse_mac` (happy path, colon + dash separators, error cases)
- [ ] `crates/racecontrol/src/api/routes.rs` or a new `tests/lockdown_tests.rs` — unit tests for lockdown route logic (billing guard, disconnected sender guard)
- [ ] No framework install needed — `cargo test` already works (confirmed: 85 tests passing)

---

## Sources

### Primary (HIGH confidence)
- Direct codebase reading — `crates/racecontrol/src/wol.rs`, `api/routes.rs`, `state.rs`, `crates/rc-agent/src/kiosk.rs`, `kiosk/src/app/control/page.tsx`, `kiosk/src/app/staff/page.tsx`, `kiosk/src/lib/api.ts`, `crates/rc-common/src/protocol.rs`
- Test run confirmation: `cargo test -p rc-common` — 85 tests passing (2026-03-14)
- Project STATE.md — confirmed Phase 7 complete, server/racecontrol running at :8080

### Secondary (MEDIUM confidence)
- Axum route ordering behavior (static before dynamic segments) — well-documented Axum pattern, verified consistent with existing routes.rs registration order
- Win32 `SetWindowsHookExW`/`ShowWindow` for kiosk.rs — standard Windows API, no version concerns

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all key files read directly from codebase
- Architecture: HIGH — existing patterns clearly established in routes.rs and kiosk.rs
- Pitfalls: HIGH — billing guard pattern directly observed in pod_monitor.rs:274

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable codebase, no external dependencies changing)
