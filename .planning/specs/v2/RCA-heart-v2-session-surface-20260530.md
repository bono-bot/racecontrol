# RCA — Heart-V2 Session Surface (V1-dependent-V2)

**Surface:** `POST /heart/sessions/{launch,pause,resume,switch-game,end}` + `GET /heart/pods/state/stream` (SSE) + `PodStateSnapshot` projection, ported into the racecontrol Rust core.
**Why this RCA exists:** The V2 heart lives in the **same Rust binary** as the deployed V1 heart and shares the tokio runtime, AppState lock primitives, the HTTP router, and the DB layer (V1/V2 split per PACT-20260503-003). That makes it a V1-dependent-V2 section ⇒ 5-section RCA before PLAN; pod-state-channel is a named **foundational boundary** ⇒ MMA Step-1 + per-PR Captain auth (authorized at plan-approval 2026-05-30).
**Author:** bono · **Date:** 2026-05-30 · **Mechanism-trust-check:** PASS 5/5 (`MECHANISM-TRUST/heart-v2-20260530.json`).
**Binding context:** This is THE blocker for first paying Hyderabad customer. The TS proxy/billing/panel stack is built and proven against `apps/mock-heart`; production points `RACECONTROL_HEART_URL` at `.23:8080` (the Rust crate), which has **0** `/heart/*` routes → every session op 404s in prod.

---

## §1 — Boundary map (paths + lines)

Where the new V2 heart surface touches / shares with V1:

| Boundary | V1 side (shared) | V2 heart side (new) | Coupling |
|---|---|---|---|
| **HTTP router** | `crates/racecontrol/src/api/routes.rs:114-874` (5-tier merge) | add `/heart/pods/state/stream` to public tier (L167-312); `/heart/sessions/*` + `/heart/pods*` to a new sub-router | additive `.merge()` — no V1 route changed |
| **Server bootstrap** | `src/main.rs:108-154` (config→pools→AppState::new→serve on :8080) | add `heart_pods` map + `heart_stream_tx` to AppState::new | additive AppState fields |
| **Shared state** | `src/state.rs:40-200` AppState (db, v2db, dashboard_tx, agent_senders RwLock, …) | new `heart_pods: RwLock<HashMap<String,PodState>>`, `heart_stream_tx: broadcast::Sender<PodStateSnapshot>` | new fields on the SAME Arc<AppState>; shares the tokio runtime |
| **Runtime / concurrency** | single tokio runtime; broadcast pattern (`dashboard_tx`); RwLock guards | broadcast fan-out + RwLock on heart_pods | **shared runtime** (inherited-issue #13) + **lock-discipline** (inherited-issue #10) |
| **DB** | V1 `racecontrol.db` (`state.db`) — billing_sessions/sessions/laps | heart-V2 state is **in-memory** (mirrors mock-heart); persistence (if added) → V2 `racecontrol-v2.db` (`state.v2db`) ONLY | **must NOT** read/write V1 billing tables (inherited-issue #11) |
| **Serde boundary** | V1 JSON structs | new heart request/response structs | **deny_unknown_fields** required (inherited-issue #4) |
| **Wire contract** | n/a | `session.yaml v0.1.10` + `apps/mock-heart/src/{server,state}.ts` (byte-for-byte ref) + proxy `m5-handlers.ts` | parser-backed (zod), allowlisted pod-paths |

**Net:** the port is **additive** — new routes, new AppState fields, new in-memory state, new V2-DB tables only if persistence is added. It does not modify any V1 route, V1 table, or V1 handler. The genuine shared surfaces are the **tokio runtime** and the **AppState lock primitives**.

---

## §2 — Inherited-issue catalogue (from §A-J V1 audit + post-audit classes)

Source: `rp-v2-apps/briefings/james/memory/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` (§A-J) + `racecontrol/CLAUDE.md` standing rules + MMA v27 lock-audit. Full 14-item catalogue lives in `/tmp/heart-understand/T4_v1-inherited-issues.md`. **Relevance filter to THIS in-process session/pod-state surface:**

| # | Inherited issue | Cat | V2-heart relevance |
|---|---|---|---|
| 10 | **Lock held across `.await`** (deadlock/starvation; billing.rs:416,425) | B-ext | **DIRECT — HIGH.** heart_pods RwLock + broadcast must never hold guard across await. |
| 4 | **serde silent-drop** of unknown fields (ai_difficulty dropped, API-200 + wrong behavior) | D | **DIRECT — HIGH.** heart request structs must `deny_unknown_fields`. |
| 13 | **Shared tokio runtime** — one blocked task delays billing/WS publish | B/E | **DIRECT — MEDIUM.** SSE fan-out + heart handlers share runtime with V1 fleet-health/billing. |
| 11 | **Schema coupling** — BillingSessionStatus enum drift V1↔V2 | D-ext | **INDIRECT — MEDIUM.** heart state enum is heart-local; must NOT reuse V1 enum or touch billing_sessions. |
| 6 | **Audit blind spots** — proxy checks (health-200) certify broken systems | F | **DIRECT — HIGH (verification).** Acceptance must be behavioral (session-not-404 + SSE delta), not health-200. |
| 14 | **sqlx::migrate! compile-cache** silent schema divergence | D-new | **CONDITIONAL — MEDIUM.** Only if heart-V2 adds a v2-db migration (persistence path). |
| 3 | **Boot-resilience fetch-at-boot no retry** → empty default storm | B | **LOW.** Heart-V2 state is launch-seeded in-memory, no boot-fetch dependency for the session surface. |
| 1,2,5,9,12 | Broadcast-storm router · Session-0 process model · restart-war · config-permanence · orphan watchdog | J/A/E/I | **OUT-OF-SCOPE** for the in-process port (venue/deploy-infra classes); resurface at the `.23` deploy/cutover step, not the Rust code. |
| 7,8 | Comms-link discipline · auth login-page chicken-egg | G/H | **N/A** to the heart route (no new login surface; SSE is public/IP-gated like /fleet/health). |

---

## §3 — Past-bug disposition

| # | Disposition | Action for heart-V2 |
|---|---|---|
| 10 lock-across-await | **UNRESOLVED in V1** (billing.rs still holds locks across await) | **MUST NOT inherit.** Enforce snapshot+drop-before-await; add a focused unit test that mutates+broadcasts without holding the guard. |
| 4 serde silent-drop | ROOT-CAUSED, PATCHED-ONLY (method documented, not universally applied) | **APPLY:** `#[serde(deny_unknown_fields)]` on every heart request struct + a contract test that rejects extra/missing fields. |
| 13 shared runtime | INHERITED-UNVERIFIED (architecturally present, not load-proven) | **MITIGATE:** keep heart handlers non-blocking (no sync IO, no lock-across-await); broadcast `.send().ok()` is non-blocking. Note for load-test follow-on. |
| 11 enum coupling | NOT-APPLICABLE-IF-CLEAN | **KEEP CLEAN:** heart state enum is heart-local strings mirroring mock-heart; no import of V1 BillingSessionStatus; no write to billing_sessions. |
| 6 audit blind spots | PARTIAL ROOT-CAUSE (methodology fixed) | **APPLY:** behavioral acceptance only (see §5 verification). |
| 14 migrate cache | ROOT-CAUSED, WORKED-AROUND | **GUARD:** if a v2-db migration is added, run `cargo clean -p v2-db` before test; for first increment, in-memory avoids this entirely. |
| 3 boot-resilience | PATCHED-ONLY | **N/A** for in-memory session surface. |

---

## §4 — V2-alignment delta

What changes vs. naively copying V1 patterns:

1. **Concurrency:** mirror `apps/mock-heart/src/state.ts` discipline — take the RwLock write guard, mutate the pod/session map, **drop the guard**, *then* `heart_stream_tx.send(snapshot)`. Never `.await` while holding the guard. (Closes #10.)
2. **Serde:** all heart request bodies use `deny_unknown_fields` + explicit enums; unknown game/tier/end_reason → 400, never silent-drop. (Closes #4.)
3. **State isolation:** heart-V2 session/pod state is a NEW in-memory substrate (`heart_pods`), not the V1 `sessions`/`billing_sessions` tables. The proxy's billing engine remains the wallet authority; the heart owns only session-lifecycle + pod-state projection. (Closes #11; respects the proxy/heart split per T3.)
4. **Projection fidelity:** emit `PodStateSnapshot` exactly as the panels consume it (pod_id, lifecycle, current_session, display_message, updated_at, alarm?) on every state mutation + a 15s heartbeat (CR-10 trip-wire compatible). (Matches mock-heart + T3 §3.)
5. **Idempotency:** pause/resume/switch/end are idempotent on terminal/no-op states (mirror mock-heart) — re-delivery from the proxy's at-least-once forward must not corrupt state.
6. **Verification:** behavioral, not health-200 (closes #6) — see §5.
7. **Scope discipline:** first increment ports the 6 session ops + SSE + pod read endpoints (the money path). The richer contract handshake (preflight→loading-complete→green-light, `/fault`, `/preset-application`, `/pods/{id}/alarm` POST/DELETE, lobby/ac-server endpoints) is **explicitly deferred** to a documented follow-on; first-INR bills from `green_light_at = launch` (mock-heart behavior), which is acceptable for the first customer and matches what the proxy/billing stack already expects.

---

## §5 — V2-framed proposal

**Build (Rust, additive, on `feat/heart-v2-session-surface`):**
1. New module `crates/racecontrol/src/api/heart_v2.rs` — types (`PodState`, `PodSession`, request structs with `deny_unknown_fields`), the in-memory store accessors, and the 6 session handlers + pod read + SSE handlers, mirroring `apps/mock-heart/src/{state,server}.ts` behavior and honoring T3 wire shapes.
2. AppState additions (`src/state.rs`): `heart_pods: RwLock<HashMap<String, PodState>>` seeded with 8 empty pods at `AppState::new` (`src/main.rs`); `heart_stream_tx: broadcast::Sender<PodStateSnapshot>`.
3. Route registration (`src/api/routes.rs`): `/heart/pods/state/stream` (public tier, SSE), `/heart/pods` + `/heart/pods/{id}` (read), `/heart/sessions/launch` + `/heart/sessions/{id}/{pause,resume,switch-game,end}`. Mount path-prefix consistent with how the proxy calls it (`HEART_BASE + /heart/...`; the proxy hits the bare `/heart/...` paths, not `/api/v1/heart/...` — confirm at wiring).
4. Concurrency discipline per §4.1; serde discipline per §4.2.

**Verify (behavioral — §3 #6 / H3):**
- `cargo test -p racecontrol-crate` covering: launch creates session + emits snapshot; pause/resume/switch/end transitions + idempotency; unknown-field → 400; SSE subscriber receives a delta on mutation; lock not held across await (compile + a concurrency smoke test).
- Local: `cargo run` → `curl POST /heart/sessions/launch` returns a real session (not 404) + a second terminal on `GET /heart/pods/state/stream` shows the snapshot delta.
- Prod gate (later step): repoint `RACECONTROL_HEART_URL` on `.23`; drive launch→end from launch-portal on one real pod; bill row persists.

**Gates honored:** mechanism-trust-check PASS · MMA Step-1 diagnosis (next) · MAOR review before push · feature branch (no main/force-push) · per-PR Captain auth pre-committed at plan-approval.

**Deferred (documented, not silent):** preflight/loading-complete/green-light handshake · `/fault` · `/preset-application` · alarm POST/DELETE · lobby + ac-server endpoints · v2-db persistence of heart state · load-test of shared-runtime contention (#13).
