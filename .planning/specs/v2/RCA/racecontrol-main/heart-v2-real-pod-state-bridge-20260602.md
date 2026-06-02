# 5-Section RCA — Heart-V2 Real Pod-State Bridge (Facet-7 end-to-end flow)

- **Author:** bono · **Date:** 2026-06-02 · **Branch:** `feat/heart-v2-real-pod-state` (off `origin/main` `66a02154`)
- **Boundary class:** FOUNDATIONAL (pod-state-channel) → MMA Step-1 DIAGNOSE required before PLAN execution; per-PR Captain merge auth.
- **Upstream gate:** mechanism-trust-check `MECHANISM-TRUST/heart-v2-real-pod-state-20260602.json` = PASS-WITH-MITIGATIONS.
- **V2 doctrine alignment:** moves the heart `PodState` toward the *Pod = state-channel* premise (heart owns the canonical pod-state projection) by making it reflect **real** pod runtime, not just proxy-issued session ops.

## Goal
Make the staff/customer panels render **real** heart state. The in-memory SSE firehose already exists; the gap is that real pod runtime (crash/exit reported by the rc-agent) never reaches the heart's `PodState`. Scope: end-to-end functional flow (NOT the deferred Postgres `pod_state_projection` durable read-model).

---

## §1 — Boundary map (where V2 crosses into V1)

**V2 side (heart-V2 session surface):** `crates/racecontrol/src/api/heart_v2.rs`
- `PodState` wire struct (L127) == `PodStateSnapshot` (`packages/contracts/openapi/session.yaml` v0.1.10); `HeartStore` (L184, in-memory `pods`+`sessions` behind one RwLock).
- Transition mutators: `launch_loading` L335, `promote_to_running` L378, `fail_launch` L395, `reconcile_green_light` L422, `end` L443.
- SSE firehose: `pods_state_stream` L958 (broadcast `heart_stream_tx`, Lagged→full-snapshot resync); `heart_routes()` mounts `/heart/pods/state/stream` at ROOT, unauthenticated LAN-internal (admin-proxy = auth boundary).
- Real-launch HTTP path: `launch` handler L677 reads flag `heart_v2_real_launch` (L686, default OFF) → `launch_real` L725 → dispatch with **`launch_args = None`** → green-light only on `verified_running`.

**V1 side (game launcher runtime truth):** `crates/racecontrol/src/game_launcher_state.rs`
- `handle_game_state_update(state, info)` L13 — the seam where the agent's runtime state lands. Updates `active_games` (write lock L34), `state.pods.current_game` (L120), billing AC timers (L252), chain-failure tracker (L275+), dashboard_tx. **Never touches `state.heart` or `state.heart_stream_tx`** (grep-confirmed empty).
- State→string map: `Running→"running"` L133, `Error→"crashed"` L135, `Idle→"stopped"` L136.

**IPC seam (agent→server):** `crates/racecontrol/src/ws/agent_game.rs` L81 `handle_game_state_update` → L103 calls `game_launcher::handle_game_state_update`. Input = `AgentMessage::GameStateUpdate(GameLaunchInfo{ session_id, sim_type, game_state, pid, last_exit_code })`, emitted by `crates/rc-agent/src/ws_handler.rs` (Launching/Running/Idle/Error, ~11 sites).

**Launch dispatch:** `crates/racecontrol/src/game_launcher_ops.rs` `dispatch_launch_to_agent` (L508+) — `verified_running` is the ONLY billing-start signal (confirm-before-bill); `launch_args` flows to `validate_args` + `make_launch_message`.

**Config keys / external contracts:**
- `heart_v2_real_launch` feature flag (`flags.rs:184 update_flag` → SQLite `feature_flags` + cache + `broadcast_flag_sync`).
- `RACECONTROL_HEART_URL` (admin-proxy `apps/admin-proxy-james/src/m5-handlers.ts:17`, default `http://127.0.0.1:8090` = **mock-heart**) → `sse-bridge.ts:220` fetch `${heartUrl}/heart/pods/state/stream`.

**The crossing being added:** today V1 (`game_launcher_state`) owns runtime truth and the V2 heart `PodState` is blind to it after the initial promote. I3 adds the V1→V2 propagation seam.

---

## §2 — Inherited-issue catalogue (V1 footguns at this boundary)

1. **GameTracker-stuck-in-Launching / `ok:true ≠ delivered`** (CLAUDE.md Crash-Loop §). WS `GameStateUpdate` is a fire-and-forget push (not acked); a dropped WS loses a state update. The heart could miss a crash/exit.
2. **Zombie `GameStateUpdate(Running)` racing stop-cleanup** (`game_launcher_state.rs:187`, `game_launcher.rs:142` `window_secs` reject). The agent sim-polling loop can emit a stale `Running` after stop — the bridge must NOT resurrect a freed/ended heart pod.
3. **F-05 overwrite-before-read billing class** (CLAUDE.md Financial-flow §). The `end` path must preserve `credits_debited` for reconciliation; an exit-bridge must not double-bill or refund.
4. **Lock-across-`.await` deadlock class** (v27.0 MMA: `agent_senders.read()` held across 8 sends). The bridge takes a NEW lock (`state.heart`) inside a hot path that already holds `active_games` — must drop the first before awaiting the second.
5. **Noisy `Error` (11 emission sites in rc-agent)** (`game_launcher_state.rs:183` R.4). `GameState::Error` is frequent/transient; the bridge must not free a **green-lit (billed)** pod on a transient Error.
6. **Boot-fetch silent-default class** (allowlist empty-default; CLAUDE.md). Analog: `m5-handlers.ts:17` silently falls back to mock-heart `:8090` if `RACECONTROL_HEART_URL` is unset — the panel shows mock state with no error.

---

## §3 — Past-bug disposition

| Issue | Disposition | Anchor |
|---|---|---|
| GameTracker-stuck-in-Launching | PATCHED-ONLY (60s Launching timeout + dispatch-poll); the underlying un-acked WS push remains → **open RCA item**, mitigated by the 15s reconciler backstop | CLAUDE.md Crash-Loop §; `reconcile_green_light` L422 |
| Zombie Running race | ROOT-CAUSED-AND-FIXED (`window_secs` reject) | `game_launcher.rs:142` |
| F-05 overwrite-before-read | ROOT-CAUSED-AND-FIXED (V1 billing); **NOT-APPLICABLE to V2** heart `end` (heart never writes wallet — proxy owns billing) but the reason-mapping must stay correct | `.planning/audits/ROOT-CAUSE-ANALYSIS-F05` |
| Lock-across-await | ROOT-CAUSED-AND-FIXED as a doctrine (snapshot→drop→await); bridge must comply | CLAUDE.md never-hold-lock |
| Noisy Error | KNOWN — design constraint for I3 (Error must be billing-neutral) | `game_launcher_state.rs:183` |
| Silent mock fallback | UNRESOLVED → **mitigated in this work** (I1 probe + I4 fail-closed) | MTC Q5 |

---

## §4 — V2-alignment delta

**Should-be (V2 doctrine — Pod = state-channel):** the heart `PodState` is the canonical projection of **real** pod runtime; consumers (pod-display/kiosk/launch-portal/staff-tablet) render it via SSE. When a game crashes or exits on a real pod, the heart's `PodState` reflects it within one transition and the panels update.

**Is (today):** the heart `PodState` is a write-only projection of **proxy-issued session ops** (launch/pause/resume/switch/end) plus the heart's own dispatch-poll promotion. The rc-agent's real runtime (`GameStateUpdate` Crash/Exit) lands only in the V1 `active_games`/`state.pods` and is **never propagated** to `state.heart`. The panels also point at mock-heart by default (`m5-handlers.ts:17`).

**Gap:** missing V1→V2 propagation seam (the bridge) + prod pointing at mock. Real pod state does not flow end-to-end.

---

## §5 — Proposed change (V2-framed)

**I2 (launch_args):** populate `launch_real`'s `launch_args` from `req.preset_id`/`tier`/`game` (AC single-player first-INR scope) + derive `duration_minutes` from the V2 tier, so the agent launches the correct car/track and `validate_args` passes. Multiplayer/lobby args = V2.1-frozen.

**I3 (the bridge — core wire):** in `game_launcher_state.rs:handle_game_state_update`, **after the `active_games` write lock drops**, when the flag `heart_v2_real_launch` is ON and the pod has a live heart session, propagate to `state.heart`:
- agent `Running` (re-deliver) → idempotent `promote_to_running` (covers a missed dispatch-poll window);
- agent `Error` → a new billing-NEUTRAL `mark_crashed` (sets `display_message`, keeps the session for reconciliation; **never grants/revokes `green_light_at`**); must respect the zombie `window_secs` reject and ignore transient Error on a green-lit pod per policy decided by MMA;
- agent `Idle` (clean exit/stop) → heart `end(sid, "game_exit")` → frees the pod;
then `heart_stream_tx.send(snapshot)`. Flag-gated (prod unchanged until I5); idempotent; the 15s reconciler is the backstop for dropped `GameStateUpdate`.

**Seam decision (→ MMA Step-1 adjudicates):**
- **Option A** — mutate `state.heart` inside `handle_game_state_update` (lowest latency; touches the V1 hot path; lock-ordering risk).
- **Option B** — extend the existing 15s `reconcile_heart_green_light_once` to also detect Idle/Error and end/mark heart sessions (V1 hot path byte-unchanged; up to 15s latency on the panel).

**I4 (cutover/guard):** set `.23` `RACECONTROL_HEART_URL=:8080`; change `m5-handlers.ts:17` default to **fail-closed** (throw if unset in prod) — closes the silent-mock-fallback (MTC Q5).

**I5 (activation):** flip `heart_v2_real_launch` ON (config_push) AFTER I2+I3 deployed + Pod-8 canary verified.

**Invariants preserved:** confirm-before-bill (green-light only on real `verified_running`); billing-neutral crash bridge; no double-spend / no free session; never hold a lock across `.await`; `#[serde(deny_unknown_fields)]`; idempotent transitions.

**Scope-freeze classification:** launch-portal staff rendering (pod frees on crash/exit so staff can re-launch) = **first-INR (in-scope)**; pod-display **customer error-display screens** = **V2.1-FROZEN** (I3 sets `display_message` only; adds no new customer-facing error states).
