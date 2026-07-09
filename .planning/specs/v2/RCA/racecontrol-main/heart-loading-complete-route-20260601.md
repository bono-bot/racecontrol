# §S-146 Supplemental RCA — heart `POST /heart/sessions/{sid}/loading-complete` route

**Date:** 2026-06-01 IST · **Author:** bono · **Surface:** `crates/racecontrol/src/api/heart_v2.rs` (bono/racecontrol lane)
**Parent RCA:** `.planning/specs/v2/RCA/racecontrol-main/heart-v2-game-launch-bridge-20260531.md` (`89764c39`) — this is a **supplemental**: the mechanism (`promote_to_running`) was already root-caused + MMA-Step-1'd + tested there; this delta is **pure HTTP wire-exposure** of that already-gated method.
**Gap closed:** G-NEW-9 (`V2-COMPONENT-WORKFLOWS-CLOSED-LOOP-2026-05-31.md`, Captain-ratified) — Captain decision G-DUP-C4 `green_light_at = confirm-before-bill`.
**Foundational boundary:** billing (grants `green_light_at` = billing-start). Per-PR Captain auth: covered by "I authorize you to fix the issues" (2026-06-01, the enumerated money-path/green_light_at queue).

## 1. Boundary map (paths + lines)

- **New route:** `heart_routes()` `crates/racecontrol/src/api/heart_v2.rs:1001-1013` — adds `.route("/heart/sessions/{sid}/loading-complete", post(loading_complete))`.
- **New handler:** `loading_complete()` — delegates to `HeartStore::promote_to_running(sid)` (`heart_v2.rs:378-391`, **pre-existing**) → `persist_and_respond` (`heart_v2.rs:847-861`, **pre-existing**).
- **Contract source-of-truth:** `rp-v2-apps/packages/contracts/openapi/session.yaml:95` (`#4b` `reportLoadingComplete` — `POST /sessions/{id}/loading-complete`, F6SystemJWT-only, "emits server-side `green_light_at`").
- **Proxy forward (Replit lane, NOT this PR):** `admin-proxy-james` must forward `reportLoadingComplete` → `POST /heart/sessions/{sid}/loading-complete`. This PR builds the heart endpoint; the proxy forwarder is the Replit-lane half (tracked, not bono-solo).

## 2. Inherited-issue catalogue (V1 failure-mode review)

**N/A — V2-isolated.** heart-V2 state is in-memory + never touches V1 billing tables (`heart_v2.rs:23` module doc; `billing_session_id=None` on the real-launch path, `heart_v2.rs:750`). The route shares **no schema/state with V1**. No category A–J (`session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md`) boundary is crossed. The §S-61 V1 failure-mode set does not touch the V2 heart route table.

## 3. Past-bug disposition

- **Bill-at-launch (mock-heart parity, `green_light_at = now` at request time)** — `ROOT-CAUSED-AND-FIXED` in parent bridge RCA (`89764c39`): replaced by confirm-before-bill via `launch_loading` + `promote_to_running` behind flag `heart_v2_real_launch`. This route is the **missing external trigger** for `promote_to_running` (the rc-agent callback path); without it, the only promote trigger is the internal `launch_real` closed-loop verify (`heart_v2.rs:759`).
- **Free-play window (heart crash post-Running, pre-green-light)** — `ROOT-CAUSED-AND-FIXED` in parent via R2 reconciler `reconcile_green_light` (`heart_v2.rs:422`). This route does not regress it (idempotent promote; reconciler still covers the crash path).

## 4. V2-alignment delta

- **Should be:** the contract declares `POST /sessions/{id}/loading-complete` as the AgentCallback that flips Loading→Running + grants `green_light_at` (confirm-before-bill). The TS proxy/billing layer already expects it.
- **Gap:** the Rust heart exposes `pause/resume/switch-game/end` but **not** `loading-complete` — the contract-declared route 404s on the heart. The mechanism exists (`promote_to_running`) but has no wire path callable by the rc-agent.
- **Delta:** add the route. Moves the heart toward contract-completeness for the confirm-before-bill loop (I4 control↔money coherence: charged ⟺ delivered).

## 5. V2-framed proposal

Add `loading_complete` axum handler (mirrors `pause`/`resume`/`end`: `store.promote_to_running(sid)` → `persist_and_respond`) + register `/heart/sessions/{sid}/loading-complete`. Idempotent (re-delivered callback no-ops, `heart_v2.rs:380`); unknown session → 404 (via `persist_and_respond` `None` arm). Unauthenticated at the heart (LAN-internal; proxy = auth boundary, consistent with all `/heart/*` routes). Adds one HTTP-level test (route is reachable, flips Loading→Running + grants green-light, idempotent, 404 on unknown).

### Mechanism-trust-check (delivery surface)

The delivery surface (heart→persist→SSE) is the **same** as the existing `pause/resume/end` mutators, already trust-checked in the parent bridge work: (1) atomic — single `heart.write()` guard; (2) no TTL sentinel involved; (3) behavioral-verify = HTTP-level test asserts state flip + green-light grant (not echo); (4) single-target = per-session `sid`; (5) `persist_and_respond` write-through-before-broadcast ordering (MMA A1) is the shared contract. **PASS — covered by parent.**

### MMA Step 1 disposition

Covered by parent bridge RCA `89764c39` (5-model MMA on the confirm-before-bill mechanism). This delta introduces **no new root-cause surface** (wire-exposure of an already-MMA'd, V2-isolated method). No fresh MMA Step 1 required; flagged for Captain review.
