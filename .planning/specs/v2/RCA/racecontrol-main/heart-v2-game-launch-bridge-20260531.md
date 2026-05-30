# §S-146 RCA — heart-V2 ↔ game_launch bridge (the missing rc-agent green-light handshake)

**Date:** 2026-05-31 · **Surface:** racecontrol Rust heart `crates/racecontrol/src/api/heart_v2.rs` (`launch`) ↔ V1 launch path `crates/racecontrol/src/api/game_launch.rs` (`launch_game`) → `game_launcher::handle_dashboard_command` → `game_launcher_ops::launch_game` → rc-agent WS · **Branch:** `feat/heart-v2-session-surface` · **Task:** blocker-cluster-1 from the overnight cross-layer gap-find (workflow `wez7nidhg`) — *"heart-V2 launch never starts the actual game on the pod."* **MTC:** PASS (`.planning/specs/v2/MECHANISM-TRUST/heart-v2-game-launch-bridge-20260531.json`). **Foundational boundaries crossed:** **pod-state-channel** (rc-agent WS + `active_games` tracker) + the **billing-coupling boundary** (V1 `active_timers`). **Authoring authorization:** autonomous, per the standing "apply recommendations autonomously" rule — this RCA is the *pre-coding analysis gate*, not a boundary mutation. **The proposed code change is NOT authorized by this RCA:** it requires **MMA Step-1 DIAGNOSE (≥5 OpenRouter models / ≥3 vendor families) + per-PR Captain auth** before any implementation — standing-autonomy verbs do NOT satisfy this gate (foundational-boundary rule).

---

## §1 — Boundary map (paths + lines)

**V2 side — launch sets billing-start signal but never starts the game:**
- `heart_v2.rs:221` `HeartStore::launch` — sets `state = Running`, `green_light_at = Some(now)`, occupies the pod (`pod.current_session`), returns `LaunchOutcome::Ok`. No agent dispatch.
- `heart_v2.rs:568` `async fn launch` (HTTP handler) — calls `store.launch`, persists (L3-1), broadcasts `PodStateSnapshot` over SSE.
- `heart_v2.rs:632` — the **mock launcher mimic**: spawns a task that flips `Loading → Running` after a short delay ("mock-heart 50ms launcher mimic"). This is a *simulation* of a launch, not a launch.
- `heart_v2.rs:11-15` (docstring, explicit): scope is "launch / pause / resume / switch-game / end … **NOT the rc-agent green-light handshake** (billing starts at launch via `green_light_at = now`, mock-heart parity)."
- `heart_v2.rs:760` route `/heart/sessions/launch` → `launch`. The proxy POSTs here (`admin-proxy-james/src/m5-handlers.ts launchSessionHandler`); the kiosk/panel subscribe to `/heart/pods/state/stream`.

**V1 side — the real rc-agent handshake (deployed, working):**
- `api/game_launch.rs:67` `launch_game` (HTTP `/api/v1/games/launch`) — validity gate (Phase 361-01) → reliability warning (INTEL-01) → `game_launcher::handle_dashboard_command(LaunchGame{…})` → on `Ok` updates `billing_sessions` (DB-2/S-3) → **closed-loop verify** reading the pod's tracker (`api/game_launch.rs:370-386`) → on `"No agent connected"` falls back to `relay_game_launch_to_venue` (HTTP relay over Tailscale, `:396/:455-505`).
- `game_launcher.rs:233` `handle_dashboard_command` → `game_launcher_ops::launch_game`.
- `game_launcher_ops.rs:29-330` — the dispatch engine: RESIL-06 semaphore (`:36-50`); arg validation LAUNCH-06 (`:56-58`); `catalog::validate_launch_combo` vs pod manifest (`:60-69`); STATE-03 feature-flag (`:71-82`); `launch_id` mint (`:88`); **FSM-03 billing gate** (`:96-115`); **FSM-08 DB-before-launch split guard** (`:117-160`); LIFE-04/LAUNCH-05 double-launch guard (`:190-200`); LAUNCH-08 dynamic timeout (`:204-216`); `launch_state_machine.start_launch` + `DashboardEvent::LaunchStatusChanged` (`:234-245`); **LAUNCH-04 TOCTOU re-check + `billing_session_id` capture + tracker insert** under `active_games.write()` (`:246-296`); WS send via `agent_senders.get(pod_id)` with **retry-once + ACK waiter** (`command_id` oneshot in `pending_command_acks`, `:300-330`).
- `game_launcher.rs:80` `active_games: RwLock<HashMap<String,GameTracker>>` — the per-pod launch authority; `:29-40` `GameTracker { game_state, launched_at, dynamic_timeout_secs, billing_session_id, launch_id, … }`.
- `billing_game_status_defer.rs:20-34` `check_launch_timeouts` (BILL-12) — the TTL reconciler for stuck `Launching`.

**The gap (grep-proven):** `heart_v2.rs` contains **zero** references to `game_launcher` / `launch_game` / `handle_dashboard_command` / `DashboardCommand` / `CoreToAgentMessage`. No `heart→game_launcher` wiring exists anywhere in `crates/racecontrol/src/`. The two pod-launch authorities (`HeartStore` and `active_games`) are fully disjoint.

---

## §2 — Inherited-issue catalogue (§S-61 V1 failure-modes + `v1_process_mess_audit` categories A-J)

The V1 launch path is a *fortress of guards*, each a tombstone for a past production failure. Heart-V2's launch inherits **none** of them — inheritance-by-omission. The most dangerous is the audit-blind echo class:

| V1 guard (failure-mode it fixes) | Path | A-J category | Heart-V2 status |
|---|---|---|---|
| **Stale/echo "verified" → audit-blind success** — old global `AtomicBool last_launch_verified` reported `verified=true` from a *prior* pod's launch; replaced by per-pod tracker `GameState::Running` check (`api/game_launch.rs:370-373`). | game_launch.rs | **audit-blind echo-as-success** (the PR#66 §S-146 mistake-class) | **RE-INTRODUCED.** The 50ms `Loading→Running` mimic (`heart_v2.rs:632`) reports Running with **no process confirmation** — exactly the echo-as-success class. |
| **Fire-and-forget WS send** (GAP-1) → now retry-once + `command_id` ACK correlation (`game_launcher_ops.rs:300-330`). | game_launcher_ops.rs | delivery non-atomic | Not present (no send at all). |
| **Orphaned launch / billing-before-launch** (FSM-08 DB-before-launch + LAUNCH-04 TOCTOU). | game_launcher_ops.rs:117-160,246-270 | ordering / orphan | Not present; ordering inverted (see §3). |
| **Double-launch** (LIFE-04/LAUNCH-05). | game_launcher_ops.rs:190-200 | concurrency | `HeartStore::launch` has its OWN `PodNotEmpty` guard (`heart_v2.rs:226`) — *partial* parity, but on the V2 tracker only; the V1 `active_games` tracker is unaware, so a V1 kiosk launch + a V2 heart launch on the same pod would not see each other. |
| **Free-gaming** (FSM-03 reject if no active billing session). | game_launcher_ops.rs:96-115 | billing integrity | Intentionally absent in V2 (proxy owns wallet+402) — *this is the seam, not a bug* (see §4). |
| **Paused-launch reject** (LAUNCH-03). | game_launcher_ops.rs:100-110 | state machine | Not present. |
| **Stuck-Launching** (BILL-12 timeout reconciler). | billing_game_status_defer.rs | supervision | Not present (no Launching state on the agent at all). |
| **Port-starvation** (RESIL-06 semaphore, max 4). | game_launcher_ops.rs:36-50 | resource | Not present. |

**TWO authorities over one pod (the structural inherited risk):** V1 `active_games` and V2 `HeartStore` both model the pod's session/launch state and both project it (V1 via `DashboardEvent`, V2 via SSE `PodStateSnapshot`). On a real pod they would diverge unless reconciled. This is the **pod-state-channel** foundational-boundary concern.

---

## §3 — Past-bug disposition

| Past bug | Disposition in V1 | Disposition for the V2 bridge |
|---|---|---|
| Stale global launch-verified bool (cross-pod false positive) | **root-caused** (per-pod tracker `GameState::Running`) | **MUST NOT regress.** The heart-V2 50ms mimic *is* this class re-expressed. The bridge must replace the mimic with a real ACK + closed-loop tracker verify before reporting Running / setting green-light. |
| Fire-and-forget agent send | **root-caused** (retry-once + `command_id` ACK, WSCMD-01/Phase-312) | **reuse, don't re-derive.** Bridge must dispatch through the same ACK-correlated send. |
| Billing-before-launch / orphaned launch | **root-caused** (FSM-08 DB-before-launch, LAUNCH-04 TOCTOU) | V2 inverts the ordering: `green_light_at = now` is set **at launch-request**, *before* any confirmation the game runs (`heart_v2.rs:221`). For V2 this is "bill-before-confirm." Bridge must set green-light **after** confirmed-Running (see §5). |
| Launch timeout / stuck Launching | **root-caused** (BILL-12 reconciler) | Bridge must surface a launch-failed terminal outcome + a TTL so a never-Running pod doesn't stay falsely Occupied. |
| Double-launch | **root-caused** (LAUNCH-05 on `active_games`) | Bridge must make the V1 and V2 trackers mutually visible, OR route all launches through one authority (open Q, §4). |

No *unresolved* or *patched-only* V1 bug is being carried forward silently — every guard above is root-caused in V1. The defect is that the V2 path does not **reuse** the root-caused machinery. Anti-pattern explicitly blocked: *"patch V1 forward"* — the fix is to make heart-V2 call the root-caused V1 transport, not to re-implement a parallel (and weaker) launcher.

---

## §4 — V2-alignment delta

**The core conflict.** V1 launch is *billing-coupled*: it requires a V1 `active_timer` and **rejects** launch without one (FSM-03), captures `billing_session_id` into the tracker, and pulls `duration_minutes` from the V1 billing timer to arm the pod-side `SessionEnforcer`. V2 deliberately moved billing **out** of the heart — the proxy owns the wallet + the 402 gate; "no money in the heart." Therefore a naive `heart_v2 → game_launcher_ops::launch_game` call would be **rejected by FSM-03** (no V1 `active_timer` exists for a proxy-owned V2 session). The bridge is an *architectural seam*, not a mechanical call.

**V2 bug-free bar (the test that fails today):** first-INR e2e on a real pod = register(OTP) → topup → **launch → the game actually starts** → tick-debit → end → bill. Heart-V2 sets `green_light_at` and projects RUNNING but never dispatches `CoreToAgentMessage::LaunchGame` → on real hardware the meter ticks and the kiosk says RUNNING while Assetto Corsa never launches. This fails the bar even with everything else green. **This is blocker-cluster-1's significance.**

**Candidate deltas (to be MMA-stressed, NOT decided in this RCA):**
- **(A) Decouple the dispatch core.** Refactor `game_launcher_ops::launch_game` into (i) a V1 billing-gate wrapper (FSM-03/FSM-08/TOCTOU, unchanged for V1 callers) and (ii) a pure `dispatch_launch_to_agent(state, pod_id, sim_type, launch_args, launch_id)` core (semaphore + tracker-insert + ACK send + closed-loop verify). heart-V2 calls (ii) with the **V2 session as the launch authority**. *Cleanest V2 alignment; blast radius = V1 launch internals (must prove V1 kiosk/PIN flows unbroken).*
- **(B) V1-compatible shim timer.** heart-V2 synthesizes a minimal V1 `active_timer` so the existing `launch_game` accepts it. *Reuses every guard unchanged but re-couples V2 to V1 billing state — anti-V2; reintroduces the thing we decoupled. Disfavored.*
- **(C) New V2 agent-command path.** heart-V2 drives `CoreToAgentMessage::LaunchGame` (already in the protocol) reusing the WS transport + ACK + closed-loop verify, with V2-session semantics and a V2-owned tracker. *Most V2-pure but re-derives the guard chain — high regression risk (must explicitly port the stale-verify / fire-and-forget / double-launch / timeout guards). Disfavored unless A's blast radius is unacceptable.*
- **(D) Proxy-orchestrated.** The proxy calls V1 `/api/v1/games/launch` after the heart confirms session + green-light. *Keeps the heart pure but splits launch authority across proxy + heart and creates two SSE projections + an ownership question for the closed-loop verify. Ordering-fragile.*

**Direction (recommendation, for MMA to confirm/refute):** lean **(A)**. In V2 the *heart* owns the pod-state-channel, so the heart should own the agent handshake, reusing the V1 transport via a decoupled dispatch core. Replace the FSM-03 V1-billing gate with the **proxy green-light** as the V2 precondition (the proxy already gated on wallet+402). Couple the green-light *ordering* fix with it (below).

---

## §5 — V2-framed proposal

1. **Decouple dispatch (delta A).** Extract `dispatch_launch_to_agent(...)` from `game_launcher_ops::launch_game`: the semaphore (RESIL-06), `active_games` tracker insert (LAUNCH-04 TOCTOU adapted to a V2 authority), the ACK-correlated WS send (retry-once, `command_id`), and the closed-loop `GameState::Running` verify (LAUNCH-08 timeout). The existing `launch_game` becomes `billing_gate(...) → dispatch_launch_to_agent(...)` so **every current V1 caller is byte-for-byte unchanged**. Regression budget: the full `game_launcher_tests.rs` suite must stay green.
2. **heart-V2 calls the core.** `HeartStore::launch` (or a new async path off the handler, lock-dropped before `.await` per existing discipline) invokes `dispatch_launch_to_agent` after the proxy's wallet+402+session. The V2 session is the launch authority; **no V1 `active_timer` / `billing_sessions` row is created** (V1/V2 isolation preserved — assert in test).
3. **Ordering fix (closes the V2 bill-before-confirm bug).** `green_light_at` is set **only after** `dispatch_launch_to_agent` confirms `GameState::Running` (closed-loop), not at request time. Until then the session is `Loading` (a real Loading, not the 50ms mimic). Replace `heart_v2.rs:632` mimic with the real handshake outcome.
4. **Failure mapping (no money harm).** dispatch-send-fail / ACK-timeout / not-Running-within-timeout → new `LaunchOutcome::LaunchFailed` → heart returns failed + sets **no** `green_light_at` → the proxy does NOT start billing. SSE emits a failed-launch event so the pod display can leave RUNNING. *(Note: the error-state DISPLAY polish is V2.1-FROZEN per the scope-freeze; the heart emitting a launch-failed SSE event is in-scope for first-INR — it is the difference between "billed for nothing" and "not billed.")*
5. **One tracker or two (OPEN — for MMA).** Either (a) heart-V2 populates/reads the same `active_games` tracker (keeps V1's double-launch guard + BILL-12 timeout + agent status-update path working for V2 launches), or (b) a parallel V2 tracker fed by the same agent status stream. (a) is less code and reuses supervision; (b) is cleaner isolation. MMA to decide; default lean (a) because the *pod* is the single physical resource and one authority is safer than two.
6. **SessionEnforcer duration.** Arm the pod-side enforcer from the **V2 session tier/entitlement** (subscription.yaml, pending Captain ratify) instead of the V1 billing timer's `duration_minutes`.
7. **Tests:** (a) heart-V2 launch with a mock `agent_sender` → assert `CoreToAgentMessage::LaunchGame` sent + tracker Running + `green_light_at` set **after** verify; (b) agent never ACKs → assert `LaunchFailed` + **no** `green_light_at` + failed SSE event; (c) double-launch guard fires across a V1+V2 launch on the same pod; (d) V1-isolation: a V2 launch creates no V1 `active_timer`/`billing_sessions` row; (e) regression: full `game_launcher_tests.rs` green after the refactor.

**Pre-coding gate (MANDATORY, foundational boundary):** **MMA Step-1 DIAGNOSE** (≥5 OpenRouter models, ≥3 vendor families) BEFORE writing code — stress: (1) one-tracker-vs-two; (2) heart-owns-handshake vs proxy-owns-handshake (A vs D); (3) the green-light ordering fix correctness; (4) blast radius of refactoring `launch_game` (does decoupling break the V1 kiosk/PIN `launch_or_assist` flows, the `force_supersede` game-switch, or `relaunch_game`?); (5) stale-verify regression risk if the 50ms mimic isn't fully removed. Then **per-PR Captain auth** at merge. This RCA + MTC are the inputs to that DIAGNOSE; they do not substitute for it.

---

## §6 — Open decisions surfaced (for Captain / MMA)

- **Owner of the bridge code:** Rust heart (this crate) per delta A/C, or split with the proxy per delta D? The overnight gap-find listed this owner as "unknown." This RCA's analysis points the *handshake* at the heart (it owns the pod-state-channel) with the proxy retaining wallet+green-light — but that is a recommendation for MMA to confirm, not a settled decision.
- **Frozen-scope check:** the launch-failed **SSE event** is first-INR-in-scope; the launch-failed **display polish** is V2.1-FROZEN. Confirm the line.
- **Sequencing vs. cluster-2 (Replit wallet HOLD+402):** the green-light precondition assumes the proxy's wallet HOLD+402 gate exists. It does **not** yet (gap-find cluster-2, Replit-owned). The heart bridge can be built and unit-tested against a mock green-light independently, but the *e2e* first-INR proof needs both. Recommend the heart bridge RCA→MMA→build proceeds in parallel with the Replit wallet gate, joined at the e2e smoke.

---

## §7 — MMA Step-1 DIAGNOSE outcome (2026-05-31 · delta A CONFIRMED + 2 refinements)

**Run:** surface `MMA-HEART-V2-BRIDGE-DELTA-A-bono-2026-05-31`, 5 OpenRouter models / 5 vendor families called ($0.0637). 4 returned usable content (deepseek-r1, nvidia-nemotron, qwen3-coder fully substantive across all 5 Qs; gemini-2.5-pro truncated after Q1; kimi-k2.5 returned an empty body — all reasoning tokens, no message). Raw: `/tmp/mma-heart-v2-bridge-results/`; spend: `comms-link/data/openrouter-spend-bono.jsonl`. Consensus below is **unanimous among responders** (≥3 vendor families substantive on every question).

**Verdict: delta A is the sound design.** No model proposed a better delta; all rejected delta D's split ownership. Two refinements are **adopted into the proposal** (they supersede §5 where they differ):

**R1 — Extract, don't rewrite (blast-radius mitigation; Q4 unanimous). Refines §5 step 1.** Do NOT mutate `launch_game`'s body. Sequence: (1) extract `dispatch_launch_to_agent(...)` as a NEW fn; (2) `launch_game` calls it AFTER its billing gate, parity preserved — safest variant (nvidia): leave `launch_game` byte-unchanged + add a shim, migrate V1 callers in a follow-up PR; (3) heart-V2 calls `dispatch_...` directly; (4) **feature-flag the V2 launch path** until E2E-verified ("no partial rollouts on a money path"). Invariants the refactor MUST preserve (Q4 consensus): PIN-auth `launch_or_assist` tracker-insert-before-challenge ordering; `force_supersede` atomic stop-then-launch under one `active_games.write()`; **`relaunch_game` must preserve `billing_session_id`**; FSM-08 DB-before-launch; NeedsManualIntervention card emission.

**R2 — Reconciler for the confirm-before-bill failure window (NEW; raised independently by deepseek + nvidia + qwen; Q3). Adds a step to §5.** The ordering fix (green-light only after Running) does not remove the failure — it *moves* it: game reaches Running but the heart crashes before persisting/sending `green_light_at` → customer plays **FREE**. Required mitigation: a boot/periodic **reconciler** that detects `active_games[pod] == Running` && the V2 session has no `green_light_at` → force-set `green_light_at` + persist + emit SSE. This composes with the L3-1 rehydrate path and is the **same mechanism** that resolves L3-1's accepted-residual C (stuck-Occupied) — one reconciler, two bugs. **Open:** first-INR-in-scope or fast-follow? Lean **in-scope** — without it the ordering fix merely trades overbill for free-play.

**Per-question consensus:**
- **Q1 (one tracker):** UNANIMOUS — reuse the single `active_games`. A parallel V2 tracker = split-brain/double-launch; BILL-12 would watch only the V1 tracker, leaving a V2 zombie that never times out. Delta A's funnel-through-`active_games` prevents it.
- **Q2 (owner):** UNANIMOUS — the heart owns the handshake AND the closed-loop verify (it owns the pod-state-channel + the agent-WS telemetry the proxy cannot see). Delta D's double-SSE (V1 DashboardEvent vs V2 SSE) is a real correctness hazard → racey billing + customer disputes, not cosmetic.
- **Q3 (ordering):** UNANIMOUS — confirm-before-bill is correct; bill-before-confirm overbills failed launches. (See R2 for the residual window.)
- **Q5 (stale-verify):** UNANIMOUS — remove the 50ms mimic; every path to green-light/Running-SSE must traverse the real closed-loop verify (V1 `launch_game`→dispatch, V2 `HeartStore::launch`→dispatch, `relaunch_game`, rehydration-on-restart, and any error/else branch that currently pretends success).

**MUST-FIX-BEFORE-MERGE (consensus, ranked — per-PR Captain-auth foundational surface):**
1. **V1-isolation test** — a V2 launch creates **zero** V1 `active_timer` / `billing_sessions` rows (query DB after launch). *(deepseek + nvidia + qwen all rank top.)*
2. **Closed-loop coverage** — mock agent never ACKs ⇒ no `green_light_at`, no Running SSE; ACK dropped after send ⇒ treated as failed.
3. **Full `game_launcher_tests.rs` regression unchanged** — PIN-auth, force-supersede, relaunch, split-session, manual-intervention all green.
4. **V1+V2 same-pod concurrency** — one wins via the `active_games` lock; the other is rejected.
5. **Restart/reconciliation** — crash post-Running pre-green-light ⇒ reconciler (R2) sets green-light on boot.
6. **BILL-12 timeout** ⇒ LaunchFailed ⇒ no billing.

**Gate remaining:** MMA Step-1 = satisfied. The code change still requires **per-PR Captain auth** to implement (foundational pod-state-channel) — standing-autonomy verbs do NOT satisfy.
