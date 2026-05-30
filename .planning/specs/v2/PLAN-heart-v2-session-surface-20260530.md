# PLAN — Heart-V2 Session Surface (Rust port of apps/mock-heart)

**Branch:** `feat/heart-v2-session-surface` · **RCA:** `RCA-heart-v2-session-surface-20260530.md` · **Trust-check:** PASS 5/5 · **MMA Step-1:** DIAGNOSE done (3 vendor families: deepseek/nvidia/qwen; gemini timeout, moonshot unparsed).
**Goal:** make production `RACECONTROL_HEART_URL`→`.23:8080` serve the `/heart/*` surface the (already-built, proven) TS proxy/billing/panel stack calls, closing the one binding gap to first-INR.

## MMA Step-1 consensus → design decisions

| Finding | Models | Decision |
|---|---|---|
| Heart restart mid-session → lost in-mem state → double-charge/leak | 3/3 (blocker) | First increment: **in-memory** (mirror mock-heart). Hardening follow-on (TOP priority before a *real* customer): v2-db WAL of active sessions + replay-on-boot, OR proxy treats `/end` 404 as terminal. **Documented, not silent.** |
| Missing launch-time HOLD/402 (₹0 launch) | 3/3 (blocker) | **Stays PROXY-side** (downstream item #2) — the heart owns no wallet. The proxy's `launchSessionHandler` checks `available_paise` before forwarding. Do NOT add 402 to the Rust heart. |
| Duplicate launch / at-least-once → double session | 3/3 | **Idempotency:** terminal/no-op state checks; launch on occupied pod → 409 (mock-heart already does this). |
| `/end` 404 after restart | 2/3 | Proxy already swallows + reconciles (heartFetch best-effort); document the "treat 404-on-end as terminal" proxy guard as follow-on. |
| alarm-404 breaks auto-end? | settled by **code**, not vote | **NO** — `session-billing.ts:157-170` arms the grace `setTimeout` BEFORE the alarm POST; `heartFetch` swallows the 404. Money-stop is safe. BUT include alarm endpoints anyway (below) for the kiosk UX. |
| lock-across-await | 3/3 | snapshot under guard → **drop guard** → `let _ = tx.send()` / `.await`. Never hold across await. Broadcast buffer 256; handle `Lagged`. |
| green-light handshake deferrable? | 3/3 safe | Billing starts at launch (`green_light_at = now`, mock-heart shortcut). loading-complete handshake deferred. |

## Scope (FIRST increment)

**Endpoints (additive, mirror mock-heart + the proxy's actual calls):**
- `GET  /heart/pods` → list 8 PodState
- `GET  /heart/pods/:id` → one PodState (404 unknown)
- `GET  /heart/pods/state/stream` → SSE; cold-boot snapshot of all pods on connect + delta on every mutation + 15s heartbeat
- `POST /heart/sessions/launch` → create session (404 unknown pod / 409 occupied|maintenance)
- `POST /heart/sessions/:id/pause` · `/resume` · `/switch-game` · `/end` → idempotent transitions
- `POST /heart/pods/:id/alarm` + `DELETE /heart/pods/:id/alarm` → set/clear `pod.alarm` + broadcast (**promoted** from deferred: trivial, enables kiosk runout UX, kills 404 noise)

**Deferred (documented):** preflight/loading-complete/green-light agent handshake · `/fault` · `/preset-application` · lobby + ac-server endpoints · v2-db persistence (→ TOP follow-on) · load-test of shared-runtime contention.

## Implementation

- **New module** `crates/racecontrol/src/api/heart_v2.rs`:
  - Types: `PodLifecycle`/`SessionState` enums, `PodSession`, `PodState` (= PodStateSnapshot), `BalanceRunoutAlarm`, request structs ALL with `#[serde(deny_unknown_fields)]`.
  - `HeartStore` accessors over `RwLock<HashMap<String,PodState>>` — every mutator: take write guard → mutate → clone snapshot → **drop guard** → `let _ = tx.send(snapshot)`.
  - 11 handlers mirroring mock-heart `state.ts` semantics (launch clears stale `pod.alarm` per Task #536; end drops `pod.alarm`; switch-game 50ms loading→running; idempotent pause/resume/end).
- **AppState** (`src/state.rs`): add `heart_pods: Arc<RwLock<HashMap<String,PodState>>>` + `heart_stream_tx: broadcast::Sender<PodState>`; seed 8 empty pods at `AppState::new` (`src/main.rs`).
- **Routes** (`src/api/routes.rs`): mount SSE on the public tier, the rest on the appropriate tier — **at the bare `/heart/...` path** the proxy expects (verify the proxy's `HEART_BASE + path` has no `/api/v1` prefix; mount to match).
- Reuse existing primitives: `tokio::sync::broadcast` (like `dashboard_tx`), `axum::response::sse`, `uuid`, `chrono`.

## Verify (behavioral — H3)

- `cargo test -p <racecontrol-crate>`: launch→running+green_light_at; pause/resume/switch/end transitions; idempotent end (2× → 200 unchanged); unknown-field body → 400; launch on occupied → 409; **SSE subscriber receives a delta on mutation**; concurrent pause/resume hammer → consistent state, no panic, no deadlock (proves no lock-across-await).
- Local integration: `cargo run` on a test port → `curl POST /heart/sessions/launch` returns real session (not 404) + a second client on `/heart/pods/state/stream` shows the delta.
- Repoint a local proxy's `RACECONTROL_HEART_URL` at the local Rust heart; drive launch→pause→resume→end with a simulated wallet + fake timers; assert bill row + no double-debit.
- **Real-pod-only residual (documented):** rc-agent green-light handshake, hardware fault paths, OTP delivery, real customer payment reconciliation, multi-pod load contention.

## Gates
MAOR review before push · feature branch (no main/force-push) · per-PR Captain auth pre-committed at plan-approval · MMA Step-1 done (this file).
