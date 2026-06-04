# §S-146 RCA — Pod-display updating/crash error-screen signal (heart → pod-display)

**Date:** 2026-06-04 · **Author:** bono · **Boundary:** pod-state-channel (foundational) · **Status:** crash-half SHIPPED (UI, non-foundational); updating-half UI shipped, heart OTA-hook DEFERRED pending pod-canary behavioral proof.

> **How this RCA was produced + corrected.** Design → §S-146 RCA → adversarial-verify ran as a 4-agent workflow (`wf_2c55884a-6a1`). The adversarial pass returned **NEEDS_FIXES (high confidence)** and corrected two design flaws *before* any code was written. The body below is the workflow's full RCA; **read these two corrections first — they override any contradictory text in the body:**
>
> 1. **Crash is NOT wired to `fleet_health.crash_loop`.** `crash_loop` is the OS/hardware rc-agent *restart-loop* signal (>3 short-uptime starts in 5min, `fleet_health.rs:142-161`) — orthogonal to a live game crash, and carries no game name. As originally drafted it would paint `INTERRUPTED · see staff` on an EMPTY/WELCOME pod. The customer-relevant single-game crash **already** routes through `mark_crashed` (`heart_v2.rs:475-491`, fired only by `ReconcileAct::Crash` requiring `real_launch && Running && GameState::Error`) → sets `display_message="INTERRUPTED · {game}"` → SSE. **So the crash screen is a PURE pod-display read of the already-emitted prefix — zero heart change, NON-foundational.** Shipped in rp-v2-apps PR #34.
>
> 2. **The OTA "updating" signal must be server-orchestrated, not pod-sentinel-driven.** `deploy-pod.sh` / `deploy-pod-agent.sh` write `OTA_DEPLOYING` then *immediately* `taskkill /F /IM rc-agent.exe` (the process hosting the WS sender). The sentinel→`ReadDirectoryChangesW`→mpsc→WS-flush chain has **no ordering guarantee vs the kill** → on the likely path the create-event never reaches the heart before the socket drops (UPDATING would appear post-update or not at all). **Mechanism-trust-check Q2/Q3 FAIL** (delivery asserted, not demonstrated). The reliable path is the **server OTA pipeline setting `display_message="UPDATING · …"` via a heart write BEFORE it triggers the pod kill** — and that requires a **single-pod canary observing the SSE frame during a real OTA** before it can claim YES. **DEFERRED** (pods 0/8; can't behaviorally prove now).
>
> **Net disposition:**
> - **CRASH screen → SHIPPED** (rp-v2-apps PR #34; UI reads existing `INTERRUPTED` prefix; non-foundational).
> - **UPDATING screen → UI SHIPPED** (rp-v2-apps PR #34; renders when `display_message` starts `UPDATING`, safe no-op otherwise). **Heart OTA-pipeline hook → DEFERRED** (foundational + mechanism-trust Q3 unproven → needs pod-canary; this RCA is its gate artifact for when pods are up).
> - **Billing/wallet boundary → NOT crossed.** Both projections (the shipped crash read; the deferred OTA write modeled on `mark_crashed`) are display-only: never free the pod, never touch `current_session`/`green_light_at`/`credits_debited`/`lifecycle` (I3 billing-neutral discipline, `heart_v2.rs:469-474`).
> - **Contract (`session.yaml`, replit-owned lane) → NOT changed.** `display_message` is already `{type:string}`, no enum.

---

## Workflow RCA (full body — read the corrections above first)

I have everything I need. Confirmed: SSE bridge parses only `pod_id` from blocks (`parsePodIdFromSseBlock` at sse-bridge.ts:301) and never re-serializes the snapshot — raw passthrough, so a string-only change rides through with zero bridge edits. session.yaml:1313 has `display_message` in `required` and 1318 is `{ type: string }` with no enum — Approach A needs no contract change. The reconciler at heart_v2.rs:1087-1183 is the safe projection point with documented lock ordering.

One important correction I'll fold into the RCA: `lib/api.ts:97` declares `display_message?: string` (optional) — a pre-existing shape drift from the required Rust field; under Approach A the pod-display reads `lastSnapshot?.display_message ?? ""` so this is tolerated, but I'll flag it.

Here is the complete RCA + decision + build plan.

---

# §S-146 RCA — Customer-Facing Pod-Display Service-State Surface (UPDATING vs CRASH vs RECONNECTING)

**Author:** bono (sole pilot, §S-448) · **Date:** 2026-06-04 IST
**Class:** V1↔V2 boundary (heart_v2 V2 ↔ fleet_health/rc-agent-OTA V1-era) · **Foundational surface:** pod-state-channel
**Scope-freeze status:** pod-display error-display states are **V2.1+ FROZEN** per DoD (Captain 2026-05-30). This RCA prepares the **smallest** slice; it ships **flag-equivalent inert** (display-only, billing-neutral) and does **not** extend the first-INR bar. Lands as *prepped + reviewable*, deploy-gated behind first-INR completion unless Captain authorizes an in-scope exception.

---

## 0. Decision — Approach A (minimal-foundational-surface, string-prefix projection)

**PICKED: Approach A.** For a *first mergeable slice* it is the smallest robust foundational surface that still yields a correct **UPDATING vs CRASH vs RECONNECTING** distinction and degrades safely.

**Why A over B:**

1. **Zero contract change → no replit-owned-lane gate.** Verified `session.yaml:1313` lists `display_message` in `required` and `:1318` is `{ type: string }` with **no enum**. New prefixes (`"UPDATING · …"`, `"INTERRUPTED · …"`) are valid string values under the *existing* schema. Approach B requires a `ServiceState` enum + a new `service_state` field on `PodStateSnapshot` — that is the §S-450 replit-owned `packages/contracts/**` lane (PR + Replit/Captain review), **plus** a persisted-shape change (PodState round-trips through `persist_session`/rehydrate at heart_v2.rs:631-678) needing a serde default + durable-blob migration risk. Bigger surface, slower, more boundaries.

2. **Reuses an already-shipped, already-tested convention.** The heart *already* writes discriminable prefixes into this exact field: `WELCOME` (heart_v2.rs:207), `LOADING · {game}` (:368), `RUNNING · {game}` (:249), **`INTERRUPTED · {game}`** (:481, the crash path in `mark_crashed`), `THANK YOU` (:457), `SESSION PAUSED` (:620). The crash screen needs **no new heart copy at all** — `mark_crashed` already emits `INTERRUPTED · {game}`, so Approach A's crash branch is *pure pod-display read* of a string the heart already produces. Only the planned-OTA `UPDATING` string is genuinely new.

3. **Billing-neutral by construction.** The one new heart method (`project_display_override`) is modeled verbatim on `mark_crashed` (heart_v2.rs:475-491): take write lock → set `display_message` *only if it differs* (idempotence guard `pod.display_message != msg`, :483) → bump `updated_at` → return `Some(pod.clone())`. It **never** touches `current_session` / `green_light_at` / `credits_debited` / `lifecycle`, inheriting the I3 billing-neutral discipline (heart_v2.rs:469-474). The frozen money path is untouched.

4. **Solves the timing tension cleanly.** Signal travels heart→pod-display and the heart never restarts during a pod OTA/crash. `OTA_DEPLOYING` is written pod-side *before* the binary kill+swap and forwarded over the still-live agent WS (`handle_sentinel_change` at agent_fleet.rs:288-313). The heart paints `UPDATING` while the link is up; pod-display caches it as `lastSnapshot` (page.tsx:209) and renders it across the 30s offline window (page.tsx:177 `offlineAfterMs: 30_000`). Crash-loop is derived from the StartupReports the agent sends *on reconnect* (agent_fleet.rs:90-101), so it broadcasts on a live link.

5. **SSE bridge needs zero edits.** Verified `sse-bridge.ts` parses only `pod_id` (`parsePodIdFromSseBlock`, :301) and forwards `data:` frames raw — never re-serializes the snapshot. A string-only change is transparent end-to-end.

**Cost accepted (A's cons, dispositioned in §4 / §5.3):** string-prefix routing is not compiler-enforced (mitigated by a shared `const` + a Rust unit test asserting the literal prefix); one field carries copy + discriminator; and there is **no override TTL** (mitigated — see the one deliberate *addition over the proposed A*, §5.2: a self-clearing guard that does not depend on the agent reconnecting). Approach B remains the **correct V2.1 end-state** (typed discriminant); this RCA names B as the follow-up trigger (§4) so A is an *aligned stepping-stone*, not a dead-end patch.

---

## (a) Boundary Map — exact paths + lines where V2 crosses into V1-era

| # | Boundary | V2 side | V1-era side | Crossing point (new code) |
|---|---|---|---|---|
| B1 | **Crash signal** | heart_v2 `PodState.display_message` (heart_v2.rs:132) — V2 customer state-channel | `fleet_health::FleetHealthStore.crash_loop` (fleet_health.rs:86), set at fleet_health.rs:153-154; `crash_loop_just_detected` computed at agent_fleet.rs:90-101 | **NEW:** after agent_fleet.rs:101 (`if crash_loop_just_detected`), resolve canonical→heart pod id and call `heart.project_display_override(pod, "INTERRUPTED · see staff")` + broadcast. Today that block (:102-130) only emits a `DashboardEvent`/incident; it does **not** touch the heart. |
| B2 | **OTA signal** | same `PodState.display_message` | `OTA_DEPLOYING` sentinel: written pod-side by rc-agent, lands at `handle_sentinel_change` (agent_fleet.rs:288-313) → `fleet_health::update_sentinel` (fleet_health.rs:194-203) → `store.active_sentinels` (fleet_health.rs:66) | **NEW:** in `handle_sentinel_change` after :313, when `file == "OTA_DEPLOYING"`: `created` → `project_display_override(pod, "UPDATING · back in a moment")` + broadcast; `deleted` → `clear_display_override(pod)` + broadcast (see §5.2). |
| B3 | **Heart store mutation** | `HeartStore` (heart_v2.rs:184) + `state.heart` write lock + `state.heart_stream_tx` broadcast (pattern documented heart_v2.rs:19/703) | — | **NEW:** `project_display_override(&mut self, pod_id, &str) -> Option<PodState>` + `clear_display_override(&mut self, pod_id) -> Option<PodState>` on `HeartStore`, modeled on `mark_crashed` (heart_v2.rs:475-491). |
| B4 | **Two-store read seam** | heart store (`state.heart`) | fleet store (`state.pod_fleet_health`, state.rs:147) | Both call-sites (B1, B2) already hold/release `pod_fleet_health` then act. The heart write is a **separate** lock acquired *after* the fleet read snapshot drops — **lock order: `pod_fleet_health` → `heart`**, consistent with the reconciler's `active_games → agent_senders → last_agent_disconnect → heart` order (heart_v2.rs:1095-1097, 1126-1139, 1167). Never invert; never hold across `.await`. |
| B5 | **Contract** | `session.yaml` PodStateSnapshot (`:1311`, `display_message` required `:1313`, `{type:string}` `:1318`) | — | **NO CHANGE.** New prefixes are valid string values. (Optional non-normative `x-rp-display-message-conventions` doc annotation only.) |
| B6 | **SSE transport** | `admin-proxy-james/src/sse-bridge.ts` (raw `data:` forward, parses only `pod_id` at :301); `routes/pod-state-sse.ts` | — | **NO CHANGE.** Raw passthrough verified. |
| B7 | **Pod-display sink** | pod-display `page.tsx` `deriveDisplayState` (:91-105), offline branch (:208-209 → `MaintenanceFallback` with `lastSnapshot={snapshot}`); `MaintenanceFallback.tsx` (:34-53); `lib/api.ts PodStateSnapshot` (:93-105) | — | **NEW (additive):** prefix branches in `deriveDisplayState` (live path) and `MaintenanceFallback` (offline/last-snapshot path), priority-ordered. |

---

## (b) Inherited-Issue Catalogue — known footguns at this boundary

From racecontrol CLAUDE.md (Crash-Loop / OTA Pipeline / Cross-Process Recovery sections) and §S-146 category sources:

1. **CF-2 — `OTA_DEPLOYING` sentinel had no TTL** (PR #66 silent-loop-death anchor, mechanism-trust-check doctrine). A sentinel that is set and never cleared on a bricked pod is the canonical footgun. **Directly relevant:** an `UPDATING` screen keyed off `active_sentinels` would stick forever if a bad OTA bricks the pod and the agent never reconnects to report the file `deleted`.
2. **MAINTENANCE_MODE — silent pod killer** (CLAUDE.md "Crash Loop"). `handle_sentinel_change` already special-cases `MAINTENANCE_MODE` (agent_fleet.rs:326+) with a WhatsApp alert. Our OTA branch must **not** collide with or suppress that path (different `file` value — clean).
3. **`clear_on_disconnect` does NOT clear `active_sentinels`** (fleet_health.rs:186, explicit comment). *Intended* for OTA (sentinel persists across the mid-OTA disconnect), but it means an OTA `UPDATING` derived purely from `active_sentinels` survives the agent vanishing — reinforcing footgun #1.
4. **crash_loop auto-set/auto-clear asymmetry** (fleet_health.rs:153-154 set; :164 + :305-312 clear). `crash_loop` only sets on `uptime<30s` reports and clears on a healthy `≥30s`-uptime report (or stale-age sweep). A genuinely *dead* pod stops sending StartupReports → `crash_loop` may stay `true`. Same stuck-state class.
5. **`schtasks`/Session-0 + `ok:true` ≠ delivered** (CLAUDE.md). Not on our write path (we mutate the heart store directly, no exec), but it is *why* B's "agent reconnect repaints" clear-path is fragile and why §5.2 adds an agent-independent clear.
6. **Serde-silent-drop / proxy-not-behavior class** (CLAUDE.md "Cross-Boundary Serialization", F-05). Approach A's string routing has *no* compiler enforcement — a reworded heart message silently breaks a pod-display branch. This is the exact class the project repeatedly gets burned by → mitigation is mandatory (shared `const` + Rust prefix-assert test, §5.3).
7. **Hold-lock-across-`.await`** (CLAUDE.md, heart_v2.rs:19). Both new call-sites are in `async` handlers; the heart mutation must snapshot-under-guard → drop → broadcast.
8. **pre-existing shape drift** (NEW finding): `pod-display/lib/api.ts:97` declares `display_message?: string` (**optional**) while the Rust wire type + session.yaml make it **required**. Tolerated by A's `?? ""` read, but it is a latent contract drift to record.

---

## (c) Past-Bug Disposition

| Inherited issue | Disposition | Justification / cite |
|---|---|---|
| CF-2 OTA-sentinel-no-TTL (#1) | **UNRESOLVED → addressed in this slice** | The override-clear is made agent-independent via a heart-side staleness guard (§5.2) so an `UPDATING` screen cannot outlive the OTA window even if the pod bricks. |
| MAINTENANCE_MODE collision (#2) | **NOT-APPLICABLE-TO-V2** | Distinct `file` value; existing MAINTENANCE_MODE handler (agent_fleet.rs:326+) is untouched; our branch keys on `OTA_DEPLOYING` only. |
| `clear_on_disconnect` keeps sentinels (#3) | **ROOT-CAUSED, retained intentionally** | Correct for the happy OTA path (banner survives mid-OTA disconnect). The brick edge is covered by #1's staleness guard, not by changing this V1 behavior. |
| crash_loop stuck-true on dead pod (#4) | **PATCHED-ONLY (pre-existing)** | We do not fix V1 crash_loop semantics. Our crash override re-fires only on the `!was_looping && crash_loop` *edge* (agent_fleet.rs:100), so we never storm; and the `INTERRUPTED` copy is correct for "agent crashed, see staff" regardless. Self-clears when the agent re-registers RUNNING/WELCOME (heart_v2.rs:249/406) OR via the §5.2 guard. |
| ok:true≠delivered / Session-0 (#5) | **NOT-APPLICABLE-TO-V2** | No exec on our write path. |
| serde-silent-drop / string-routing (#6) | **MITIGATED in-slice** | Shared `DISPLAY_PREFIX_*` consts in Rust + a `#[test]` asserting `mark_crashed` emits the exact `INTERRUPTED` prefix the pod-display matches; pod-display matches the same documented literals. |
| lock-across-await (#7) | **ROOT-CAUSED-AND-PREVENTED** | snapshot→drop→broadcast pattern enforced, mirroring heart_v2.rs:1166-1182. |
| display_message optional-vs-required drift (#8) | **PATCHED-ONLY (record)** | Out of this slice's blast radius; pod-display reads defensively (`?? ""`). Logged to security/contract-debt awareness; real fix is contract-parity (Approach B era). |

---

## (d) V2-Alignment Delta

**What the boundary should look like under V2 doctrine:** the pod is a **state-channel** (Pod = state-channel premise); the customer-facing surface is driven by the *heart's* authoritative projection, not by the unreliable pod-agent link. Crash/OTA are *service-state*, conceptually orthogonal to billing/session lifecycle, and ideally a **typed discriminant** (Approach B's `ServiceState` enum) so the pod-display switches on a value, not a parsed string.

**The gap A leaves:** service-state remains *encoded in a human-readable copy string* rather than a typed field. This is a **deliberate, named** kaizen-correct V1-retention: it ships the customer-correct behavior at the smallest foundational surface, with **zero** replit-lane/contract/persisted-shape churn, during a scope-freeze where the surface is V2.1-FROZEN. It is aligned (heart-authoritative, billing-neutral, state-channel) and does not entrench an antipattern because the follow-up trigger (§4) retires the string-routing in favor of the typed field.

**V2 doctrine alignment line:** *moves the boundary toward "heart-authoritative customer state-channel, billing-neutral projection" (Pod = state-channel premise + heart-V2 isolation), reusing the existing `display_message` projection convention; defers the typed-discriminant end-state (Approach B) behind an explicit retire-trigger.*

---

## (e) V2-Framed Proposal (the change)

Project the two V1-era service signals (`crash_loop` edge + `OTA_DEPLOYING` sentinel) into the heart's `display_message` via **two documented prefixes**, broadcast over the heart→pod-display SSE (which stays up), and read them on the pod-display in both the live path and the offline/last-snapshot fallback — priority-ordered **above** generic RECONNECTING and below armed-runout. Add one agent-independent staleness clear so no reassuring screen masks a dead pod. **No contract, no SSE-bridge, no billing change.**

**Follow-up trigger that retires the V1 path (mandatory per §S-146.5):** When (i) first-INR bug-free bar passes and scope unfreezes, OR (ii) any consumer needs structured crash/OTA metadata (ETA, retry-count, auto-recover-vs-see-staff split), **migrate to Approach B** (`service_state` typed enum on PodStateSnapshot) via the replit-owned contract lane. Record this trigger in the OTA/crash-display debt note alongside CANONICAL-REFS.

---

## Mechanism-Trust-Check (5 questions) — shared crash-detection + OTA-sentinel infra

The fix depends on shared infra (fleet_health crash-detection + the SentinelChange transport). Per the upstream-of-fix-RCA rule, all 5 must be YES:

| # | Question | Result | Evidence |
|---|---|---|---|
| Q1 | **Atomic primitives?** | **YES** | Crash/OTA writes are guarded mutations: `pod_fleet_health.write().await` (agent_fleet.rs:91, :309) for the source; the new heart write is a single `state.heart.write().await` guarded mutate→drop→broadcast (mirrors heart_v2.rs:1167-1182). No read-modify-write race; idempotence guard `display_message != msg` (heart_v2.rs:483) prevents double-broadcast. |
| Q2 | **TTL-bounded sentinels integrated with the atomic primitive?** | **YES (with the §5.2 addition)** | `OTA_DEPLOYING` itself is *not* TTL-bounded (inherited CF-2). This slice integrates a TTL/staleness bound at the *projection* layer: the override clears agent-independently (§5.2) so the customer screen is bounded even when the underlying sentinel is not. crash_loop has an auto-clear (fleet_health.rs:164, :305-312). |
| Q3 | **Behavioral-verify success (not echo-string)?** | **YES** | Verification is by reading the broadcast `PodState` snapshot (binary state of the store), not an echo. Rust unit test asserts `project_display_override` mutates `display_message` to the exact const + returns `Some(snapshot)`; existing `mark_crashed` test (heart_v2.rs:2335) already asserts `display_message.starts_with("INTERRUPTED")`. |
| Q4 | **Single-target dry-run path?** | **YES** | Both writes are per-pod (`pod_id`-scoped); no fleet fan-out. Can be exercised on one pod (canary) by toggling one pod's `OTA_DEPLOYING` / forcing one crash_loop edge, and observing that pod's SSE frame only. |
| Q5 | **Guards have written contracts with the delivery script (parser-not-regex + allowlist)?** | **YES** | Delivery is the in-process `heart_stream_tx` broadcast + raw SSE bridge (parser reads only `pod_id`, sse-bridge.ts:301 — parser, not regex over the payload). The pod-display match is on a documented, const-shared prefix allowlist (`UPDATING`, `INTERRUPTED`), not an open regex. |

**MTC verdict: 5/5 YES → no separate §S-146 RCA owed on the infra surface.** Cache at `.planning/specs/v2/MECHANISM-TRUST/pod-display-service-state-20260604.json` (30-day validity).

---

## Build Plan — exact files + exact changes

### 1. Heart (racecontrol) — `crates/racecontrol/src/api/heart_v2.rs`

**1a. Add shared prefix consts** (near the wire-types block, after the `PodLifecycle` enum ~heart_v2.rs:57). These are the single source of truth the pod-display mirrors:

```rust
/// Customer-facing display_message prefixes the pod-display routes on.
/// Pod-display (MaintenanceFallback.tsx / page.tsx deriveDisplayState) matches
/// these LITERAL prefixes — DO NOT reword without updating the pod-display branch
/// (serde-silent-drop / proxy-not-behavior class, CLAUDE.md Cross-Boundary Serialization).
pub const DISPLAY_PREFIX_UPDATING: &str = "UPDATING";     // planned OTA — reassuring
pub const DISPLAY_PREFIX_INTERRUPTED: &str = "INTERRUPTED"; // unplanned crash — see staff (mark_crashed already emits this)
```

**1b. Add two display-only methods on `HeartStore`** (after `mark_crashed`, ~heart_v2.rs:491), modeled verbatim on `mark_crashed` (idempotent, billing-neutral, never touch session/green_light/lifecycle):

```rust
/// Display-only service-state override (crash/OTA). Billing-NEUTRAL: identical
/// discipline to `mark_crashed` (L469-491) — NEVER frees the pod, NEVER touches
/// current_session / green_light_at / lifecycle. Idempotent (no re-broadcast).
pub fn project_display_override(&mut self, pod_id: &str, msg: &str) -> Option<PodState> {
    match self.pods.get_mut(pod_id) {
        Some(pod) if pod.display_message != msg => {
            pod.display_message = msg.to_string();
            pod.updated_at = now_iso();
            Some(pod.clone())
        }
        _ => None, // pod gone or already showing this — idempotent
    }
}

/// Clear a service-state override back to the session-derived display, so a
/// reassuring screen cannot outlive the OTA/crash window (CF-2 no-TTL mitigation,
/// agent-INDEPENDENT). Re-derives from the live session if present, else WELCOME.
pub fn clear_display_override(&mut self, pod_id: &str) -> Option<PodState> {
    let pod = self.pods.get_mut(pod_id)?;
    let derived = match pod.current_session.as_ref() {
        Some(s) => match s.state {
            SessionState::Running | SessionState::Ready => format!("RUNNING · {}", s.game),
            SessionState::Loading | SessionState::Preflight => format!("LOADING · {}", s.game),
            SessionState::Paused => "SESSION PAUSED".to_string(),
            _ => "WELCOME".to_string(),
        },
        None => "WELCOME".to_string(),
    };
    if pod.display_message != derived {
        pod.display_message = derived;
        pod.updated_at = now_iso();
        Some(pod.clone())
    } else { None }
}
```

**1c. Unit tests** (in the existing `#[cfg(test)]` module): assert `project_display_override` sets the exact prefix + returns `Some`, idempotent second call returns `None`; assert it leaves `current_session`/`green_light_at` unchanged; assert `clear_display_override` restores `RUNNING · {game}` when a session is live; assert the const `DISPLAY_PREFIX_INTERRUPTED` is a prefix of what `mark_crashed` writes (lock the cross-boundary contract).

### 2. Heart (racecontrol) — `crates/racecontrol/src/ws/agent_fleet.rs`

**2a. Crash projection** — inside the existing `if crash_loop_just_detected {` block (after the incident/dashboard emit, ~agent_fleet.rs:128, before the block closes), add (snapshot→drop→broadcast; lock order `pod_fleet_health` is already dropped by here):

```rust
// Surface the crash on the customer pod-display (display-only, billing-neutral).
if let Ok(heart_pod) = rc_common::pod_id::heart_pod_id(pod_id) { // pod_N → pod-N
    let snap = {
        let mut heart = state.heart.write().await;
        heart.project_display_override(
            &heart_pod,
            &format!("{} · see staff", crate::api::heart_v2::DISPLAY_PREFIX_INTERRUPTED),
        )
    };
    if let Some(s) = snap { let _ = state.heart_stream_tx.send(s); }
}
```
*(Use the existing canonical→heart id helper. Heart keys pods as `pod-N` hyphen; `normalize_pod_id` yields `pod_N`. Confirm the exact converter name during impl — `live_sessions_normalized` doc at heart_v2.rs:494-499 documents the hyphen/underscore seam; reuse whatever the reconciler uses to map back, or match on the heart's own `pods` keys.)*

**2b. OTA projection** — inside `handle_sentinel_change`, after the `active_sentinels` snapshot (agent_fleet.rs:313), before the `MAINTENANCE_MODE` branch (:326):

```rust
if file == "OTA_DEPLOYING" {
    if let Ok(heart_pod) = rc_common::pod_id::heart_pod_id(pod_id) {
        let snap = {
            let mut heart = state.heart.write().await;
            match action {
                "created" => heart.project_display_override(
                    &heart_pod,
                    &format!("{} · back in a moment", crate::api::heart_v2::DISPLAY_PREFIX_UPDATING)),
                "deleted" => heart.clear_display_override(&heart_pod),
                _ => None,
            }
        };
        if let Some(s) = snap { let _ = state.heart_stream_tx.send(s); }
    }
}
```

**2c. Agent-independent staleness clear (CF-2 mitigation)** — in the **existing 15s reconciler** `reconcile_heart_green_light_once` (heart_v2.rs:1087), append a final pass *after* the green-light results loop (after :1182): snapshot `pod_fleet_health.read()` once (drop guard), and for any pod whose `display_message` starts with `DISPLAY_PREFIX_UPDATING` but whose `active_sentinels` no longer contains `OTA_DEPLOYING` (or whose last agent activity is stale beyond a bound, e.g. `disconnects` elapsed > 5 min), call `clear_display_override` + broadcast. This bounds the `UPDATING` screen even if the agent never reconnects to report the file deleted. Lock order preserved: read `pod_fleet_health` → drop → `state.heart.write()` → drop → broadcast (matches the file's documented order).

### 3. Contract — `packages/contracts/openapi/session.yaml`

**NO functional change.** Optional, non-normative annotation only (does not require a behavior PR; still replit-owned so even the comment goes via review — defer if it adds friction):
```yaml
# under PodStateSnapshot.display_message (~L1318):
display_message:
  type: string
  description: >
    Free-form customer copy. Recognized routing prefixes (heart-projected):
    "WELCOME" | "LOADING · {game}" | "RUNNING · {game}" | "SESSION PAUSED" |
    "INTERRUPTED · …" (crash/see-staff) | "UPDATING · …" (planned OTA) | "THANK YOU".
```

### 4. SSE bridge — `apps/admin-proxy-james/src/sse-bridge.ts` + `routes/pod-state-sse.ts`

**NO CHANGE.** Verified raw passthrough (parses only `pod_id`, sse-bridge.ts:301).

### 5. Pod-display UI — `rp-v2-apps-wt-errscreens/apps/pod-display/`

**5a. `app/maintenance/MaintenanceFallback.tsx`** — add UPDATING + INTERRUPTED branches reading `lastSnapshot.display_message`, priority-ordered **above** `isMaintenance`/RECONNECTING and **below** `localAlarmArmed`. Insert after line 37 (`isMaintenance`), then extend the `pill`/`headline`/`sub`/accent ternaries (:39-53):

```tsx
const lastMsg = lastSnapshot?.display_message ?? "";
const isCrash = !localAlarmArmed && (lastMsg.startsWith("INTERRUPTED") || lastMsg.startsWith("CRASH"));
const isUpdating = !localAlarmArmed && !isCrash && lastMsg.startsWith("UPDATING");
```
Priority: `localAlarmArmed` (SEE STAFF, red, credits) > `isCrash` (SEE STAFF, red) > `isUpdating` (UPDATING, amber, "this pod is getting a quick update · your session and credits are safe") > `isMaintenance` (MAINTENANCE, gunmetal) > RECONNECTING (existing default). Crash uses the red `armed`-style accent path; UPDATING uses the amber path. (Tokens already imported: `RP.red`, `RP.amber`, `RP.gunmetal`.)

**5b. `app/page.tsx` `deriveDisplayState`** (:91-105) — add live-path branches **after** the alarm checks (:93-96) and **before** `lifecycle === "maintenance"` (:97), plus two `DisplayState` union members (:79-89), `StateView` cases (~:370+) and `accentFor` cases (:736+):
```ts
const msg = snap.display_message ?? "";
if (msg.startsWith("INTERRUPTED") || msg.startsWith("CRASH")) return "service_crash";
if (msg.startsWith("UPDATING")) return "service_updating";
```
Add `| "service_updating" | "service_crash"` to the `DisplayState` union; `accentFor`: `service_updating → RP.amber`, `service_crash → RP.red`; add two `StateView` cases (headline + reassuring/see-staff copy).

**5c. `lib/api.ts`** — no field add needed (reads existing `display_message`). **Record** the pre-existing `display_message?: string` optional-vs-required drift (:97) in contract-debt; out of this slice's scope.

### 6. Tests / verification (per CLAUDE.md gates)

- **Rust:** `cargo test -p racecontrol-crate heart_v2` (note: workspace package is `racecontrol-crate`, not `racecontrol`) — new override/clear/idempotence/billing-neutral tests + the prefix-contract assert; plus existing crash test (heart_v2.rs:2335) stays green.
- **Pod-display:** since this is a **display-affecting** change, CLAUDE.md "Visual verification" gate applies — render the four states (RECONNECTING / MAINTENANCE / UPDATING / SEE-STAFF) and capture screenshots before any "done" claim. (The PostToolUse screenshot reminder fires on pod-display edits; honor it at implementation time.)
- **MMA:** this crosses 2+ system boundaries (heart→bridge→pod-display) → cross-system-bridge MMA is advisable per CLAUDE.md, though the surface is display-only/billing-neutral; minimally run MAOR REVIEW (§14.1) on the cascade.

### 7. Process gates before merge

Foundational pod-state-channel boundary → **per-PR Captain auth required** (standing-autonomy verbs do not satisfy). Contract lane untouched, so **no Replit review needed** for the functional change. Scope-freeze: land **prepped/reviewable**, deploy **gated** behind first-INR unless Captain authorizes an in-scope exception. Heart push of non-§S-N code = per-PR auth (not covered by the §S-N-close-anchor standing push rule).

---

## Files (absolute paths, for the executor)

- `/root/racecontrol-wt-lb/crates/racecontrol/src/api/heart_v2.rs` — prefix consts (~L57); `project_display_override` + `clear_display_override` (after L491); staleness-clear pass in `reconcile_heart_green_light_once` (after L1182); unit tests.
- `/root/racecontrol-wt-lb/crates/racecontrol/src/ws/agent_fleet.rs` — crash projection in `crash_loop_just_detected` block (after ~L128); OTA projection in `handle_sentinel_change` (after L313, before L326).
- `/root/racecontrol-wt-lb/crates/racecontrol/src/fleet_health.rs` — read-only (no change; reference `active_sentinels` L66/L194, `crash_loop` L86/L153, clear L164/L305).
- `/root/rp-v2-apps-wt-errscreens/apps/pod-display/app/maintenance/MaintenanceFallback.tsx` — UPDATING/INTERRUPTED branches (after L37; extend L39-53).
- `/root/rp-v2-apps-wt-errscreens/apps/pod-display/app/page.tsx` — `deriveDisplayState` (L91-105), `DisplayState` union (L79-89), `StateView` (~L370+), `accentFor` (L736+).
- `/root/rp-v2-apps-wt-errscreens/apps/pod-display/lib/api.ts` — no change; record `display_message?` drift (L97).
- `/root/rp-v2-apps/apps/admin-proxy-james/src/sse-bridge.ts` + `routes/pod-state-sse.ts` — **no change** (raw passthrough verified).
- `/root/rp-v2-apps/packages/contracts/openapi/session.yaml` — **no functional change** (optional doc annotation at L1318 only; replit-lane).
- Cache: `/root/racecontrol-wt-lb/.planning/specs/v2/MECHANISM-TRUST/pod-display-service-state-20260604.json` (MTC 5/5).
