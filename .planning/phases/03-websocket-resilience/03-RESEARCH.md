# Phase 3: WebSocket Resilience - Research

**Researched:** 2026-03-13
**Domain:** WebSocket keepalive (Rust/axum + tokio-tungstenite), reconnect backoff, React debounce/memoization
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Kiosk disconnect UX:**
- Silent debounce: pod card stays green/online during the 15s debounce window. Staff only sees "Disconnected" after 15s confirmed absence. No false alarm flashing during game launches.
- Single "Offline" state after debounce expires — no timed stages or age indicators. Activity log has timestamps if staff needs history.
- Card color change only on offline — no toast notifications, no sound alerts. Uday already gets email alerts from Phase 2 watchdog.
- Kiosk's OWN WebSocket connection also uses 15s debounce before showing disconnected in the header. Brief racecontrol restarts are invisible.

**Pod screen during WS drop:**
- During active billing: customer sees NOTHING on WS drop. Game keeps running locally. Lock screen does not show "Disconnected". The drop is completely invisible to the customer.
- During idle (no billing): lock screen shows "Disconnected" IMMEDIATELY on WS drop. No debounce for idle pods — staff needs to know unoccupied pods lost connection.
- On reconnect after a drop during active billing: silent resume. No "Connection restored" toast or notification to the customer. They never knew anything happened.
- Game keeps running during long WS drops (2+ minutes) — no warning overlay, no billing pause, no action on pod side. pod_monitor on racecontrol handles alerting staff via email.
- Full re-register on every reconnect — pod sends fresh Register message with complete PodInfo. Same as initial connect. racecontrol gets accurate state immediately.

**Reconnect aggressiveness:**
- rc-agent uses fast-then-backoff: first 3 attempts at 1s intervals (covers brief CPU spike blips), then exponential backoff 2s→4s→...→30s max.
- Kiosk frontend keeps current 3s fixed retry interval — simple, fast enough for staff-facing LAN connection. 15s debounce hides brief drops anyway.
- Both WS-level ping (from racecontrol) AND application-level heartbeat (from rc-agent at 5s) — belt-and-suspenders. WS ping keeps TCP alive during CPU spikes. App heartbeat carries pod state data.
- racecontrol sends WS ping frames every 15s to all connected agents. Low overhead, frequent enough to prevent TCP idle timeout during shader compilation (typically 10-30s).

**Performance targets:**
- WS command round-trip (racecontrol → rc-agent → response) must complete under 200ms during normal operation on LAN.
- ALL kiosk interactions — pod card clicks, page transitions, state updates, button responses — must respond within 100ms.
- Log slow round-trips: tracing::warn! when WS command round-trip exceeds 200ms. No metrics dashboard, no Prometheus — just log lines for debugging.
- No WebSocket message compression (permessage-deflate) — LAN bandwidth is not the bottleneck, pod state messages are small (~1-2KB), compression adds latency.
- Optimize per-pod card updates: only re-render the specific pod card that changed (React.memo + stable keys), not all 8 cards on every WS message.
- Use React 18 auto-batching for rapid-fire WebSocket messages — no custom requestAnimationFrame batching needed.

### Claude's Discretion
- WS ping/pong implementation details (axum built-in vs manual frames)
- Exact debounce implementation in useKioskSocket.ts (setTimeout vs useRef timer)
- How to measure WS round-trip time for the tracing::warn! threshold
- React.memo granularity — which sub-components of pod cards to memoize
- Whether to add a pong timeout on racecontrol side (and what threshold)

### Deferred Ideas (OUT OF SCOPE)
- Start billing timer on confirmed game launch (not at session creation) — billing/session management phase
- Billing timer pause during pod downtime — already captured as deferred in Phase 2
- WS message compression — explicitly decided against for LAN setup, revisit if multi-venue with WAN
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONN-01 | WebSocket ping/pong keepalive prevents drops during game launch CPU spikes | axum Message::Ping sent every 15s from racecontrol send_task; tungstenite auto-responds with Pong on agent side |
| CONN-02 | Kiosk debounces disconnect events — only shows "Disconnected" after 15s+ confirmed absence | useRef timer pattern in useKioskSocket.ts onclose; disconnectTimer ref cleared on reconnect |
| CONN-03 | rc-agent reconnects automatically with short backoff on WebSocket drop | Modify existing reconnect_delay logic: attempt_count guards first 3 retries at 1s before exponential |
| PERF-03 | WebSocket command round-trip (racecontrol → rc-agent → response) stays under low threshold | Timestamp in CoreToAgentMessage; agent echoes timestamp back; core logs tracing::warn! if >200ms |
| PERF-04 | Kiosk UI interactions (page loads, button responses, state updates) feel instant to staff | React.memo on KioskPodCard; React 18 auto-batching handles rapid WS messages; stable pod.id keys |
</phase_requirements>

---

## Summary

Phase 3 is a targeted resilience improvement across three layers: the server-side WebSocket (axum racecontrol), the client WebSocket agent (tokio-tungstenite rc-agent), and the kiosk React frontend. The changes are surgical modifications to existing code paths — not rewrites.

The core problem: Assetto Corsa shader compilation spikes the pod CPU for 10-30 seconds. During this window, the rc-agent event loop cannot pump the WebSocket, so the TCP connection can appear dead to the OS and get terminated by idle-timeout rules. The fix is periodic WS ping frames from racecontrol (server-initiated, every 15s) which keep TCP alive even when the agent's event loop is busy. Tungstenite automatically queues pong replies; when the agent's event loop resumes it drains the send queue and the pong goes out, proving liveness.

The kiosk debounce prevents staff seeing false "Disconnected" flashes during the same CPU spike window. Because the agent heartbeat (every 5s) may be delayed up to 30s during shader compilation, a 15s debounce window on the kiosk side matches the worst-case single heartbeat skip without introducing enough delay to hide real failures. Round-trip measurement gives observability into latency without needing a metrics stack.

**Primary recommendation:** Add a `tokio::time::interval(15s)` ping task inside `handle_agent`'s send_task, modify rc-agent's reconnect_delay to track attempt count for fast-then-backoff, add a `disconnectTimer` useRef in `useKioskSocket.ts`, and wrap `KioskPodCard` with `React.memo`.

---

## Standard Stack

### Core (already in use — verified from Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8 | HTTP + WebSocket server (racecontrol) | Already in use; provides `Message::Ping` variant |
| tokio-tungstenite | 0.26 | WebSocket client (rc-agent) | Already in use; auto-queues pong replies to pings |
| futures-util | 0.3 | SinkExt/StreamExt for WS split | Already in use across both crates |
| React 18 | current (Next.js app) | Kiosk frontend | Auto-batching handles burst WS messages natively |

### No New Dependencies
Phase 3 adds zero new dependencies. All required primitives exist:
- `tokio::time::interval` — for 15s ping loop (already used in rc-agent main.rs)
- `Message::Ping(bytes::Bytes::new())` — axum 0.8 Message enum has Ping variant (confirmed HIGH confidence)
- `useRef` + `setTimeout` — standard React hooks in kiosk (already imported in useKioskSocket.ts)
- `React.memo` — standard React API (kiosk uses functional components)

---

## Architecture Patterns

### Pattern 1: Server-Side Ping in send_task (racecontrol ws/mod.rs)

**What:** Replace the current `handle_agent` send_task (which only forwards mpsc messages) with a task that also fires WS ping frames every 15s.

**Current code (ws/mod.rs lines 54-62):**
```rust
let send_task = tokio::spawn(async move {
    while let Some(cmd) = cmd_rx.recv().await {
        if let Ok(json) = serde_json::to_string(&cmd) {
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    }
});
```

**New pattern — add ping interval alongside mpsc:**
```rust
// Source: axum docs.rs 0.8 Message::Ping variant (confirmed)
let send_task = tokio::spawn(async move {
    let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&cmd) {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                // Empty payload — standard keepalive ping
                if ws_sender.send(Message::Ping(bytes::Bytes::new())).await.is_err() {
                    break;
                }
            }
        }
    }
});
```

**Why `MissedTickBehavior::Skip`:** If the send channel is busy (e.g. a large burst of billing ticks), a missed ping tick should be skipped rather than burst-fired on resume. Skip is the correct policy for keepalive pings.

**Why `tokio::select!` instead of two spawns:** The ws_sender (SplitSink) is not `Clone` and cannot be shared across tasks. Both ping sending and command forwarding must run in the same task. tokio::select! interleaves them safely.

**Confidence:** HIGH — axum 0.8 docs confirm Message::Ping variant exists; tokio::select! with interval is a standard tokio pattern.

### Pattern 2: Fast-Then-Backoff Reconnect (rc-agent main.rs)

**What:** Modify the existing reconnection loop to distinguish first 3 attempts (1s each) from subsequent attempts (exponential 2s→4s→...→30s max).

**Current code (main.rs ~line 421):**
```rust
let mut reconnect_delay = Duration::from_secs(1);
// ... on failure:
reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
```

**Problem with current:** First failure waits 1s (fine), second waits 2s, third waits 4s. Brief CPU spikes (1-3s) that cause a drop may not give the 3 fast retries needed to reconnect before the spike ends.

**New pattern:**
```rust
let mut reconnect_attempt: u32 = 0;

// On success: reset both
reconnect_attempt = 0;

// On failure (replace the current delay logic):
let reconnect_delay = if reconnect_attempt < 3 {
    Duration::from_secs(1)           // fast: attempt 0, 1, 2
} else {
    let exp = reconnect_attempt - 3; // exponential starts after attempt 3
    Duration::from_secs(2u64.pow(exp.min(4))) // 2s, 4s, 8s, 16s, 30s cap
        .min(Duration::from_secs(30))
};
reconnect_attempt += 1;
tokio::time::sleep(reconnect_delay).await;
```

**Attempt sequence:** 0→1s, 1→1s, 2→1s, 3→2s, 4→4s, 5→8s, 6→16s, 7+→30s cap.

**Agent behavior on disconnect (existing + unchanged):**
- `lock_screen.show_disconnected()` is called — but ONLY when no billing is active (the existing billing check on lines 1247-1260 must be preserved exactly as-is)
- Billing-active path already calls `lock_screen.show_disconnected()` only on active billing drop, then loops back. This is correct per the locked decision.

**Confidence:** HIGH — pure logic change to existing variables; no new API surface.

### Pattern 3: Kiosk Debounce in useKioskSocket.ts

**What:** Add a 15s disconnect timer (useRef) in the onclose handler. Defer `setConnected(false)` until the timer fires. Clear the timer on successful reconnect (onopen).

**Current code (useKioskSocket.ts lines 272-276):**
```typescript
socket.onclose = () => {
  setConnected(false);  // immediate — causes false flashes
  console.log("[Kiosk] Disconnected, retrying in 3s...");
  setTimeout(connect, 3000);
};
```

**New pattern:**
```typescript
// Add at hook top level:
const disconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

// In connect():
socket.onopen = () => {
  // Clear any pending disconnect timer — we reconnected in time
  if (disconnectTimerRef.current !== null) {
    clearTimeout(disconnectTimerRef.current);
    disconnectTimerRef.current = null;
  }
  setConnected(true);
  console.log("[Kiosk] Connected to RaceControl");
};

socket.onclose = () => {
  console.log("[Kiosk] Disconnected, retrying in 3s...");
  // Debounce the UI update — only show disconnected after 15s
  disconnectTimerRef.current = setTimeout(() => {
    setConnected(false);
    disconnectTimerRef.current = null;
  }, 15_000);
  // Still retry immediately — reconnect attempt is separate from UI state
  setTimeout(connect, 3000);
};
```

**Key insight:** The UI debounce (`setConnected(false)` at 15s) and the reconnect retry (3s fixed) are independent. Reconnect retries proceed immediately while the UI stays green. If a retry succeeds within 15s, the timer is cleared and staff never sees a flash.

**useRef vs useState for timer:** useRef is correct here — the timer ID does not need to trigger a re-render, only be read/written by effect cleanup.

**Cleanup:** The existing `useEffect` cleanup (`ws.current?.close()`) is sufficient. If the component unmounts while the disconnect timer is pending, the timer fires on a dead component — harmless because `setConnected` is idempotent and the component is already gone. Adding `clearTimeout(disconnectTimerRef.current)` to the cleanup is the clean option.

**Confidence:** HIGH — standard React useRef pattern for timer IDs; confirmed by React docs patterns.

### Pattern 4: KioskHeader Debounce

**What:** KioskHeader receives `connected: boolean` as a prop (from useKioskSocket). Since the debounce is in the hook (useKioskSocket), KioskHeader needs NO changes — it will only receive `connected=false` after the 15s debounce fires. This is the natural consequence of debouncing at the source.

**No code change needed in KioskHeader.tsx.** The `connected` prop will simply not be set to `false` until the debounce expires.

**Confidence:** HIGH — prop flow is confirmed by reading both files.

### Pattern 5: React.memo on KioskPodCard

**What:** Wrap the exported `KioskPodCard` function with `React.memo` to prevent re-renders when a WS message updates a different pod's state.

**Current:** Every `pod_update` WS message triggers `setPods(new Map(...))`, which causes the parent to re-render with a new `pods` Map, which re-renders ALL 8 KioskPodCard instances even though only 1 pod changed.

**New pattern:**
```typescript
// In kiosk/src/components/KioskPodCard.tsx — wrap the export:
export const KioskPodCard = React.memo(function KioskPodCard({ ... }: KioskPodCardProps) {
  // ... existing function body unchanged ...
}, (prevProps, nextProps) => {
  // Custom equality: re-render only if this pod's relevant data changed
  return (
    prevProps.pod === nextProps.pod &&
    prevProps.billing === nextProps.billing &&
    prevProps.telemetry === nextProps.telemetry &&
    prevProps.warning === nextProps.warning &&
    prevProps.gameInfo === nextProps.gameInfo &&
    prevProps.authToken === nextProps.authToken
  );
});
```

**Why custom comparator:** The default `React.memo` shallow equality would still re-render because the Map read extracts a new object reference each time (`pods.get(id)` returns the same object if we didn't mutate it, but `new Map(prev)` creates a new Map and `next.set(pod.id, pod)` overwrites only that entry — other pods' values are the same object references from the previous Map). With `setPods((prev) => { const next = new Map(prev); next.set(pod.id, pod); return next; })` (as currently coded), unchanged pod values ARE the same object references. Default React.memo would actually work correctly here. Custom comparator is therefore optional — use it only if sub-prop granularity is needed.

**Key stable-key requirement:** The parent must pass `pod.id` as the React `key` prop (which it likely already does). Stable keys prevent React from unmounting/remounting pod cards.

**Granularity recommendation (Claude's Discretion):** Wrap the full `KioskPodCard` export with `React.memo` (default equality is sufficient because unchanged pod objects are same reference). Do NOT memoize sub-components like `TransmissionToggle`, `FfbToggle`, `BlankScreenButton` — they have local state that must not be confused across re-renders.

**Confidence:** HIGH — React.memo with functional components is well-documented; Map spread preserves reference identity for unchanged entries, making default equality correct.

### Pattern 6: Round-Trip Measurement (PERF-03)

**What:** Measure time from when racecontrol sends a CoreToAgentMessage to when it receives the corresponding agent response. Log a `tracing::warn!` if >200ms.

**Decision (Claude's Discretion):** Use a lightweight in-memory `HashMap<correlation_id, Instant>` keyed by a ping ID. racecontrol adds a `Ping { id: u64 }` variant to `CoreToAgentMessage` and rc-agent adds a `Pong { id: u64 }` variant to `AgentMessage`. This is cleaner than reusing WS-level ping/pong frames (which don't carry correlation IDs through the axum API).

**Implementation touch points:**
- `rc-common/src/protocol.rs`: Add `Ping { id: u64 }` to `CoreToAgentMessage` and `Pong { id: u64 }` to `AgentMessage`
- `racecontrol/src/ws/mod.rs`: In send_task, send `CoreToAgentMessage::Ping { id }` every 30s (separate from WS-level ping every 15s); record `Instant::now()` in a `HashMap` keyed by id
- `racecontrol/src/ws/mod.rs`: In receive loop, on `AgentMessage::Pong { id }`, look up the Instant, compute elapsed, `tracing::warn!` if >200ms
- `rc-agent/src/main.rs`: Handle `CoreToAgentMessage::Ping { id }` in the `_ => {}` arm → send `AgentMessage::Pong { id }` immediately

**Why not WS-level ping frames:** The tungstenite pong is generated automatically with no application-layer callback. We cannot timestamp the round-trip at the application level using protocol-level frames.

**Why not per-command:** The decision says "WS command round-trip" generally. A periodic heartbeat ping is simpler than instrumenting every command and covers the common case (LAN latency check).

**Confidence:** HIGH — this is a standard correlation-ID pattern; no new dependencies needed.

### Recommended File Change Map

```
crates/racecontrol/src/ws/mod.rs        # Ping task in send_task + Pong receive handler
crates/rc-common/src/protocol.rs    # Add Ping/Pong to CoreToAgentMessage/AgentMessage
crates/rc-agent/src/main.rs         # Fast-then-backoff + handle Ping → send Pong
kiosk/src/hooks/useKioskSocket.ts   # disconnectTimerRef + 15s debounce
kiosk/src/components/KioskPodCard.tsx  # React.memo wrap
```

KioskHeader.tsx: NO CHANGE (debounce happens at hook level).
rc-agent lock_screen behavior: NO CHANGE (existing billing check already handles customer invisibility).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WebSocket pong response | Manual pong send in rc-agent | tungstenite auto-queues pong | Automatic per RFC 6455; manual pong can race with auto-pong and cause protocol errors |
| WS frame keepalive | Custom TCP keepalive or SO_KEEPALIVE tuning | `Message::Ping` via axum | OS TCP keepalive has a 2-hour default; application-layer ping is the standard approach |
| React render optimization | Custom memoization cache, requestAnimationFrame batching | `React.memo` + React 18 auto-batching | React 18 already batches async state updates from event handlers including WebSocket onmessage; no custom batching needed |
| Disconnect debounce | Complex state machine | Single `setTimeout` + `useRef` | 15s single-shot timer is the simplest correct implementation |
| Round-trip measurement | Prometheus/metrics exporter | `tracing::warn!` with `Instant` | Decided: no metrics dashboard; structured logs are sufficient for a 8-pod venue |

**Key insight:** tungstenite's automatic pong is critical — DO NOT add a `Message::Pong` send in the rc-agent receive loop. Tungstenite has already queued it; a manual pong replaces the automatic one, which can cause frame ordering issues.

---

## Common Pitfalls

### Pitfall 1: Double-Pong from Manual + Auto Response
**What goes wrong:** If rc-agent's `_ => {}` arm in the ws_rx match is changed to explicitly send a pong back for ping frames, tungstenite would have already queued an automatic pong. Two pong frames violate RFC 6455 and some WebSocket implementations close the connection.
**Why it happens:** Developers unfamiliar with tungstenite assume they must handle ping frames manually.
**How to avoid:** Leave rc-agent's `_ => {}` arm unchanged for WS protocol frames. Only handle the application-level `CoreToAgentMessage::Ping { id }` message to send back `AgentMessage::Pong { id }`.
**Warning signs:** Test with a WS debugger and see two consecutive Pong frames after one Ping.

### Pitfall 2: Stale Timer in React Closure
**What goes wrong:** If `disconnectTimerRef` is captured in a stale closure (e.g., inside a `useCallback` that doesn't re-create on ref change), `clearTimeout` receives the wrong timer ID.
**Why it happens:** useRef value changes do not trigger hook re-runs; closures capture the `ref.current` value at the time of creation.
**How to avoid:** Always access the timer as `disconnectTimerRef.current` (reading the `.current` property) inside the handler, not as a captured value. Because `useRef` returns a stable object, `disconnectTimerRef.current` always reads the latest value.
**Warning signs:** Disconnect toast appears even after quick reconnect.

### Pitfall 3: Reconnect Loop Runs While Old Send_Task Still Alive
**What goes wrong:** In racecontrol's handle_agent, if send_task is not aborted before the next iteration and a new connection registers for the same pod, the old send_task's mpsc sender lingers in `agent_senders`. Commands may go to a dead sender.
**Why it happens:** The `send_task.abort()` is already at the end of handle_agent (line 392), but if handle_agent returns early (e.g., before pod registers), the abort still fires. The stale-conn_id guard (lines 349-390) correctly prevents the old connection from overwriting the new one's state.
**How to avoid:** This is already handled by the `is_closed()` check (Phase 02 fix). The ping task runs inside send_task, so aborting send_task kills both the command forwarder and the ping timer simultaneously. No extra cleanup needed.
**Warning signs:** Pods showing "connected" in agent_senders but never responding.

### Pitfall 4: Kiosk Disconnect Timer Not Cleared on Unmount
**What goes wrong:** If the kiosk page unmounts while a 15s disconnect timer is pending, the setTimeout callback fires on an unmounted component, calling `setConnected(false)` on a dead component.
**Why it happens:** `useEffect` cleanup only closes the WebSocket, not the pending timer.
**How to avoid:** Add `clearTimeout(disconnectTimerRef.current)` to the useEffect cleanup function alongside `ws.current?.close()`.
**Warning signs:** React "Can't perform state update on unmounted component" warning in browser console (React 17 and earlier; React 18 silently ignores it but it's still a logic error).

### Pitfall 5: Ping Interval Races with cmd_rx Close
**What goes wrong:** In the racecontrol send_task, if cmd_rx is dropped (because handle_agent returns), the `cmd_rx.recv()` returns None and the select! terminates. The ping_interval.tick() arm continues firing even though there's nothing to do.
**Why it happens:** tokio::select! exits on the first arm to resolve, but if `cmd_rx.recv()` returns `None` first, the loop breaks. This is correct behavior — send_task should exit when cmd_rx closes.
**How to avoid:** Use `while let Some(cmd) = cmd_rx.recv()` inside the select! arm that handles commands, OR handle `None` from recv by breaking the loop. The `tokio::select!` pattern shown in Pattern 1 above already handles this: `Some(cmd) = cmd_rx.recv()` will not match on None, causing the select! to only resolve via the ping tick, which will then fail to send on the closed ws_sender and break.
**Warning signs:** Ping tasks accumulating after connections drop; check with `tokio::runtime::Handle::current().metrics()`.

### Pitfall 6: Fast-Then-Backoff Resets on Partial Failure
**What goes wrong:** If reconnect_attempt resets to 0 on successful TCP connect but the Register message send fails, the next failure starts from 0 again (3 more fast attempts instead of escalating).
**Why it happens:** The success path resets attempt count, but a failure during registration is distinct from a connection failure.
**How to avoid:** Only reset `reconnect_attempt = 0` after Register send succeeds (not just after `connect_async` succeeds). The existing code already does `reconnect_delay = Duration::from_secs(1)` only on stream success. Mirror this pattern: set attempt=0 only after the Register send completes without error.

---

## Code Examples

### Send Ping from racecontrol send_task

```rust
// Source: axum 0.8 docs.rs - Message::Ping variant confirmed
use axum::extract::ws::Message;
use tokio::time::{interval, Duration, MissedTickBehavior};

let send_task = tokio::spawn(async move {
    let mut ping_interval = interval(Duration::from_secs(15));
    ping_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(msg) => {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    None => break, // cmd_rx closed — handle_agent is exiting
                }
            }
            _ = ping_interval.tick() => {
                if ws_sender.send(Message::Ping(bytes::Bytes::new())).await.is_err() {
                    tracing::warn!("Ping send failed — agent connection dropped");
                    break;
                }
                tracing::trace!("WS ping sent to agent (conn_id={})", conn_id);
            }
        }
    }
});
```

**Note on bytes dependency:** `bytes::Bytes::new()` requires the `bytes` crate. axum already depends on it transitively. Confirm by checking `Cargo.lock` — if `bytes` is not a direct dependency of racecontrol, add `bytes = "1"` to racecontrol/Cargo.toml.

### Fast-Then-Backoff in rc-agent main.rs

```rust
// Source: project-specific pattern; standard tokio idiom
let mut reconnect_attempt: u32 = 0;

loop {
    let ws_result = tokio::time::timeout(
        Duration::from_secs(10),
        connect_async(&config.core.url),
    ).await;

    let (ws_stream, _) = match ws_result {
        Ok(Ok(stream)) => {
            reconnect_attempt = 0; // reset only on successful connection
            stream
        }
        Ok(Err(e)) => {
            let delay = reconnect_delay_for_attempt(reconnect_attempt);
            tracing::warn!("Failed to connect: {}. Attempt {}. Retrying in {:?}...",
                e, reconnect_attempt, delay);
            lock_screen.show_disconnected();
            tokio::time::sleep(delay).await;
            reconnect_attempt += 1;
            continue;
        }
        Err(_) => {
            let delay = reconnect_delay_for_attempt(reconnect_attempt);
            tracing::warn!("Connection timed out. Attempt {}. Retrying in {:?}...",
                reconnect_attempt, delay);
            lock_screen.show_disconnected();
            tokio::time::sleep(delay).await;
            reconnect_attempt += 1;
            continue;
        }
    };

    // ... register ...
    // If register fails:
    // reconnect_attempt += 1; continue;
    // If register succeeds:
    // reconnect_attempt = 0; (already reset above, but reset again to be safe)
}

fn reconnect_delay_for_attempt(attempt: u32) -> Duration {
    if attempt < 3 {
        Duration::from_secs(1)                                     // fast: 1s, 1s, 1s
    } else {
        let exp = (attempt - 3).min(4);                            // cap exponent to avoid overflow
        Duration::from_secs(2u64.pow(exp)).min(Duration::from_secs(30)) // 2s, 4s, 8s, 16s, 30s
    }
}
```

### Kiosk Debounce Pattern

```typescript
// Source: Standard React useRef pattern — React docs
const disconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

const connect = useCallback(() => {
  if (ws.current?.readyState === WebSocket.OPEN) return;
  const socket = new WebSocket(WS_URL);

  socket.onopen = () => {
    // Cancel pending "show disconnected" timer — we reconnected in time
    if (disconnectTimerRef.current !== null) {
      clearTimeout(disconnectTimerRef.current);
      disconnectTimerRef.current = null;
    }
    setConnected(true);
    console.log("[Kiosk] Connected to RaceControl");
  };

  socket.onclose = () => {
    console.log("[Kiosk] Disconnected, retrying in 3s...");
    // Debounce UI — only show disconnected after 15s of confirmed absence
    if (disconnectTimerRef.current === null) {
      disconnectTimerRef.current = setTimeout(() => {
        setConnected(false);
        disconnectTimerRef.current = null;
        console.log("[Kiosk] 15s debounce expired — marking disconnected");
      }, 15_000);
    }
    setTimeout(connect, 3000); // continue retrying regardless
  };

  socket.onerror = () => {
    socket.close();
  };

  ws.current = socket;
}, []);

useEffect(() => {
  connect();
  return () => {
    ws.current?.close();
    // Clean up debounce timer on unmount
    if (disconnectTimerRef.current !== null) {
      clearTimeout(disconnectTimerRef.current);
    }
  };
}, [connect]);
```

### Application-Level Round-Trip Ping (PERF-03)

```rust
// In rc-common/src/protocol.rs — add to CoreToAgentMessage:
CoreToAgentMessage::Ping { id: u64 },

// In rc-common/src/protocol.rs — add to AgentMessage:
AgentMessage::Pong { id: u64 },

// In racecontrol ws/mod.rs — inside handle_agent, alongside send_task:
use std::collections::HashMap;
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};

static PING_COUNTER: AtomicU64 = AtomicU64::new(0);
let pending_pings: Arc<Mutex<HashMap<u64, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

// In send_task (add a 30s ping_measure_interval alongside the 15s keepalive ping):
// Send measurement ping every 30s (separate from keepalive ping every 15s)
// Record Instant in pending_pings

// In receive loop, match AgentMessage::Pong { id }:
AgentMessage::Pong { id } => {
    if let Some(sent_at) = pending_pings.lock().await.remove(&id) {
        let elapsed = sent_at.elapsed();
        if elapsed.as_millis() > 200 {
            tracing::warn!(
                "WS round-trip slow: pod {} took {}ms (threshold 200ms)",
                pod_info.number, elapsed.as_millis()
            );
        } else {
            tracing::debug!("WS round-trip: pod {} {}ms", pod_info.number, elapsed.as_millis());
        }
    }
}

// In rc-agent main.rs — in the CoreToAgentMessage match:
rc_common::protocol::CoreToAgentMessage::Ping { id } => {
    let pong = AgentMessage::Pong { id };
    let json = serde_json::to_string(&pong)?;
    let _ = ws_tx.send(Message::Text(json.into())).await;
}
```

**Simplification option:** If `Arc<Mutex<HashMap>>` feels heavy, use a single `Option<(u64, Instant)>` since only one outstanding ping is needed at a time. Send new ping only after previous pong received (or after timeout).

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Manual pong in WS receive loop | tungstenite auto-queues pong | Correct per RFC 6455; no application code needed |
| Fixed reconnect backoff (all retries same delay) | Fast-then-backoff (3x1s then exponential) | Covers CPU spike blips without flooding server |
| Immediate disconnect UI update | 15s debounce timer | Staff never sees spurious flash during game launch |
| Render all 8 pods on any state change | React.memo per-pod | Only affected pod re-renders |
| No round-trip visibility | tracing::warn! on >200ms | Debugging latency issues without metrics stack |

**No deprecated approaches in this phase** — all changes add to or modify existing patterns rather than replacing them.

---

## Open Questions

1. **bytes crate availability for `Message::Ping(bytes::Bytes::new())`**
   - What we know: axum and tokio-tungstenite both depend on `bytes` crate transitively
   - What's unclear: Whether `bytes` is available as a direct dep in racecontrol without explicit Cargo.toml entry
   - Recommendation: Check `cargo tree -p racecontrol-crate | grep bytes` in the plan's Wave 0 task; if not direct, add `bytes = "1"` to racecontrol/Cargo.toml. Alternatively use `Message::Ping(vec![].into())` — axum's Message::Ping accepts `Bytes` which can be constructed from `Vec<u8>` via `.into()`.

2. **Pong timeout on racecontrol side (Claude's Discretion)**
   - What we know: WS-level pong is auto-sent by tungstenite; app-level Pong has the 30s measurement window
   - What's unclear: Whether a "pong not received in Xs → close connection" guard is needed
   - Recommendation: Do NOT add a pong timeout for this phase. The existing `is_closed()` check in pod_monitor already detects dead connections. Adding a pong timeout would create a second detection path that could race with the Phase 2 watchdog. Keep it simple: if pong doesn't arrive in the measurement window, the next ping measurement attempt will show high latency (or the WS will have already dropped due to the agent's event loop being dead, which the OS will detect via TCP RST).

3. **React.memo custom comparator necessity**
   - What we know: `setPods((prev) => { const next = new Map(prev); next.set(pod.id, pod); return next; })` preserves object references for unchanged pods
   - What's unclear: Whether Next.js / React 18 concurrent renderer's scheduler might cause ref identity to break
   - Recommendation: Start with default React.memo (no custom comparator). Add custom comparator only if profiling shows unexpected re-renders. Default equality checks prop references, which are stable for unchanged map entries.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (built-in, no extra setup) |
| Config file | None — uses `#[cfg(test)]` inline modules |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| Full suite command | Same — all 3 crates (47 tests baseline) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONN-01 | WS ping sent every 15s from racecontrol send_task | unit | `cargo test -p racecontrol-crate ws_ping_keepalive` | ❌ Wave 0 |
| CONN-02 | Kiosk shows connected during 15s debounce window | manual | Browser dev tools — disconnect racecontrol, verify header stays green for 15s | N/A manual |
| CONN-03 | First 3 reconnect attempts at 1s, then exponential | unit | `cargo test -p rc-agent-crate reconnect_delay_for_attempt` | ❌ Wave 0 |
| PERF-03 | tracing::warn! fires when round-trip >200ms | unit | `cargo test -p racecontrol-crate ws_round_trip_slow_logs_warn` | ❌ Wave 0 |
| PERF-04 | Only changed pod card re-renders | manual | React DevTools Profiler — trigger pod_update for Pod 1, verify Pods 2-8 show no render | N/A manual |

### Sampling Rate
- **Per task commit:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Per wave merge:** Same (all 47+ tests green)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] Unit test for `reconnect_delay_for_attempt(attempt: u32)` pure function — cover attempt 0, 1, 2 (→1s), attempt 3 (→2s), attempt 7+ (→30s cap) in `rc-agent/src/main.rs` test module
- [ ] Unit test for WS ping interval logic in send_task — requires extracting ping logic into a testable fn or using a mock ws_sender
- [ ] Unit test for Pong round-trip warn threshold — requires mock Instant or Duration injection

*(If pure functions are extracted as recommended, the test files need no new infrastructure — they live inline in the existing `#[cfg(test)] mod tests` blocks)*

---

## Sources

### Primary (HIGH confidence)
- `axum 0.8 docs.rs` — `Message::Ping(Bytes)` variant confirmed, pong auto-sent by server on incoming ping
- `tungstenite docs` — "Upon receiving ping messages, tungstenite queues pong replies automatically. The next call to read, write or flush will write & flush the pong reply."
- `axum::discussions::1340` — Confirmed: "tungstenite will automatically respond to pings... you should never need to manually handle Message::Ping"
- Project codebase (racecontrol/src/ws/mod.rs, rc-agent/src/main.rs, kiosk/src/hooks/useKioskSocket.ts) — read directly; line numbers cited

### Secondary (MEDIUM confidence)
- React 18 automatic batching — confirmed via reactwg/react-18 Discussion #21 (official React team authored)
- React.memo functional component wrapping — confirmed from React docs pattern (standard API)

### Tertiary (LOW confidence)
- `bytes::Bytes::new()` as ping payload — inferred from axum type signatures; verify bytes availability with `cargo tree`

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in Cargo.toml, no new deps needed
- Architecture: HIGH — concrete code changes identified with exact file/line context
- Pitfalls: HIGH — pong auto-reply and closure staleness are well-documented; others derived from reading existing code
- Validation: HIGH — pure function extraction makes all logic unit-testable

**Research date:** 2026-03-13
**Valid until:** 2026-06-13 (stable APIs — axum, tokio, React 18 are mature)
