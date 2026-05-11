# §S-146 RCA — PR #54 (PACT-20260429-013 Phase 1+2)

**PR:** racecontrol#54 — `feat(billing): route billing_paused via config_push_queue (PACT-013 Phase 1+2)`
**Author:** bono-bot · **Created:** 2026-04-29 · **Idle:** 12 days · **CI:** ALL GREEN (build / API contract / Rust tests / security scan / comms-link QG — 2026-04-30)
**Diff:** +128 / -91 across 5 files (`protocol.rs`, `billing_session_lifecycle.rs`, `ws_handler.rs`, `CLAUDE.md`, `.gitignore`)
**Doctrine class:** §S-146 retroactive (PR created pre-§S-146; merge is post-§S-146 → applies) · **Foundational boundary:** billing → MMA Step 1 DIAGNOSE required
**Bilateral status:** PACT-20260429-013 RATIFIED-PROCEED-PHASE-0+1+2+3+4+5 by both AIs 2026-04-29 · Phase 0 triage GREEN-LIGHT (`comms-link/docs/PACT-013-PHASE-0-TRIAGE.md`) · Phase 0.5 .23-DB-verify CLOSED (3 tables present)
**Authored:** 2026-05-11 ~12:30 IST by bono · per Captain commission "Lets work on getting V2-LIVE-BLOCKING completed first"
**V2 doctrine alignment:** V2-P1-CONFIG-SERVICE (Phase 177 `1fc92867`) — first non-staff caller for behavioral state sync; moves billing_paused delivery from ad-hoc wire variant to seq+ack+audit substrate per V2-MASTER-STATE §S-N substrate-readiness lens (config-flow vs code-shape categorization rule)

---

## §1 — Boundary map

**Files touched (paths + line ranges from `gh pr diff 54`):**

| File | Δ | What the change does |
|---|---|---|
| `crates/rc-common/src/protocol.rs` | +6/-47 | **Removes** `CoreToAgentMessage::BillingPaused`/`BillingResumed` enum variants (lines 808-820 in main) + their `test_billing_paused_resumed_roundtrip` unit test (lines 3529-3560). **Preserves** `AgentMessage::BillingPaused` (agent→server crash-pause path, line 354 in main; pre-PACT-008). |
| `crates/racecontrol/src/billing_session_lifecycle.rs` | +88/-20 | `set_billing_status()` callsite rewrites the server→agent notification: was `agent_senders.send(CoreToAgent::BillingPaused {...})`; now inserts a `config_push_queue` row (per-pod + seq_num + status='pending') + delivers via WS as `ConfigPush` field-payload + on Ok updates status='delivered' + appends `config_audit_log` entry. |
| `crates/rc-agent/src/ws_handler.rs` | +24/-22 | **Removes** the two `CoreToAgentMessage::BillingPaused`/`BillingResumed` match arms (lines 382-406 in main). **Adds** `"billing_paused"` to `HOT_RELOAD_FIELDS` allowlist (line 1753) + match arm that calls `failure_monitor_tx.send_modify(s.billing_paused = paused)` when the ConfigPush field fires. Composes-with crash-pause path at `event_loop.rs:1339` (both paths converge on the same `failure_monitor.billing_paused` watch-channel state). |
| `CLAUDE.md` | +2/-2 | Doc refresh (CGP v3.6→v4.3 / MMA v3.0→v4.0) — orthogonal to PACT-013 substrate; harmless drift catch-up. |
| `.gitignore` | +8/0 | Cross-platform path leak suppression (`process_guard_server` hardcodes `r"C:\RacingPoint\..."`; on Linux deploy these become literal-named files in cwd). Separate PACT filed for the `cfg(target_os)` fix; suppression preserves git-status signal until source fix lands. |

**DB tables / IPC seams crossed:**
- **DB schema (no migration — uses Phase 177 tables already deployed 2026-03-24, verified on .23 Phase 0.5):**
  - `config_push_queue (pod_id TEXT, payload TEXT, seq_num INTEGER, status TEXT, created_at TEXT, acked_at TEXT)` — INSERT 'pending' / UPDATE 'delivered' on WS Ok
  - `config_audit_log (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, seq_num)` — INSERT per push
  - `billing_session` (V1-era table) — READ by `set_billing_status` predecessor logic (unchanged by PR)
- **WS protocol seam:** `CoreToAgentMessage::ConfigPush(ConfigPushPayload { fields, schema_version, sequence })` — existing variant from Phase 177; PR extends its payload-key vocabulary, not the enum surface
- **Agent state seam:** `failure_monitor` watch-channel (`tokio::sync::watch<FailureMonitorState>`) — field `billing_paused: bool`; mutated by `send_modify()`; consumed by `billing_guard` for SESSION-01 orphan-auto-end suppression + BILL-02 stuck-session anomaly gating
- **Server state seam:** `AppState::config_push_seq: AtomicU64` — monotonic seq_num generator; existing field from Phase 177

**Boundary class:** server → agent control-plane (config-distribution surface; V2-P1-CONFIG-SERVICE substrate). Foundational by §S-146 escalation list (billing).

**V1 boundary actually crossed by PR #54:** `billing_session` DB table (V1-era) is READ by predecessor `set_billing_status` logic that PR does NOT modify. PR only modifies the *agent-notification side-effect* of that callsite. V1 footprint in PR diff itself: zero. V1 inheritance via callsite context: indirect (billing-session table semantics unchanged).

---

## §2 — Inherited-issue catalogue

**Sources scanned (Rule 0 enumeration):**
- `comms-link/briefings/james/memory/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` — 10-category V1 audit (A–J)
- racecontrol `LOGBOOK.md` — billing / config-push / wire-protocol grep
- V2-MASTER-STATE §S-N tail since §S-117 (substrate-readiness scorecard) — billing-class entries
- §S-61 PART 41 V1 failure-mode investigation (14 mapped)
- G9 / UCA tags in canonical memory — config_push_queue / BillingPaused

**Findings (boundary-touching V1 footguns):**

| # | Category (V1 mess audit A–J) | Footprint at this boundary | Empirical anchor |
|---|---|---|---|
| 1 | **D — Schema/config drift** (kiosk JSON ≠ Rust struct · OpenAPI ≠ handler · CREATE without ALTER · GDPR FK) | `billing_session` table is V1-era. PR does NOT touch its schema, but the wire-variant being removed (`CoreToAgentMessage::BillingPaused`) was added by PR #49 / PACT-008 without §S-146 RCA — Q-002 anti-pattern of "ad-hoc wire variants atop V1 substrate." | PR #49 `a75321e2` 2026-04-28 |
| 2 | **E — Recovery-cascade / restart-war** (self_monitor + rc-sentry + pod_monitor + WoL + Watchdog + MAINTENANCE_MODE) | `failure_monitor.billing_paused` gates SESSION-01 orphan auto-end + BILL-02 stuck-session anomaly. Loss of the flag (WS drop between server-queue and agent-receive on PR #49's path) re-arms the cascade — customer closes game during pause → orphan-end fires 5min later → session destroyed. This is the exact bug PACT-008 was authored to fix; PR #54 closes it more robustly via DB-persisted queue. | PACT-20260428-008 design rationale |
| 3 | **H — Authentication drift** (login-page middleware-blocking, allowlist GET requiring auth) | N/A — config_push_queue is server→agent (no end-user auth boundary). M2M trust via WS session identity. |
| 4 | **F — Audit blind spots** (checking proxies not behavior) | PR #49's `agent_senders.send().is_ok()` was proxy verification (queue accepted ≠ message delivered). PR #54 closes this via DB row state machine (pending → delivered) + `config_audit_log` row + ConfigAck round-trip. Substrate moves from "fire-and-forget" to "evidence-of-delivery." Aligned with audit-blind-spot doctrine. | F-class category |
| 5 | **J — Layer-2/3 broadcast hygiene** (fleet-wide class) | N/A — config_push is per-pod row (1 row per pod_id per change), not broadcast. CP-02 reconnect-replay restores per-pod delivery on disconnect. |
| 6 | **§S-61 PART 41 V1 failure-mode #7** — Silent message drop on agent_senders.send() when WS drops mid-frame | PR #49 was vulnerable: no retry, no DB persistence, no seq+ack. PR #54 closes via Phase 177 substrate (CP-02 reconnect-replay handles WS drop). | §S-61 PART 41 |
| 7 | **V1 Process-mess A (Session 0 vs 1)** | N/A — rc-agent already enforced Session 1 (RCWatchdog WTSQueryUserToken). config_push handler runs in agent process; agent process Session-correctness orthogonal to this PR. |
| 8 | **V1 Process-mess B (boot resilience: single-fetch-at-boot without retry)** | N/A for billing_paused specifically (PR doesn't touch boot path). Tangential: HOT_RELOAD_FIELDS allowlist itself is hardcoded — would not survive a server-down boot in current form. Out-of-scope for this PR. |

**Boundary-relevant pattern from V1 mess catalog #1 (Category D + PR #49 anti-pattern):** *"ad-hoc wire-protocol variants added atop V1 substrate without §S-146 RCA"* — PR #54 is structurally the corrective: it removes the PR #49 variant + redirects through the V2-P1-CONFIG-SERVICE substrate. **This PR closes a V1-shaped anti-pattern that snuck into V2 code.**

---

## §3 — Past-bug review

| Past bug | Disposition | Evidence |
|---|---|---|
| **PACT-20260428-008 / PR #49** — *server→agent BillingPaused/Resumed via ad-hoc wire variant* | **PATCHED-ONLY** (open RCA item — closed by THIS PR) | PR #49 `a75321e2` 2026-04-28 merged without §S-146 (pre-§S-146 ratify) + without mechanism-trust-check (pre-§S-172 ratify). PACT-20260429-013 was filed 1 day later to redirect this through Phase 177 substrate. PR #54 is the structural close. |
| **PACT-20260428-008 use-case: orphan auto-end fires 5min after customer closes game during manual pause** | **NOT-APPLICABLE-TO-V2** (the behavioral fix is preserved; only the delivery substrate changes) | `failure_monitor.billing_paused` state mutation is identical — agent sets `s.billing_paused = paused` either way; `billing_guard` SESSION-01 gate unchanged. Crash-pause path (`event_loop.rs:1339`, AgentMessage::BillingPaused agent→server) is untouched. |
| **§S-61 PART 41 V1 failure-mode #7 — silent message drop on agent_senders.send() when WS drops** | **ROOT-CAUSED-AND-FIXED** (closed by THIS PR for billing_paused class) | PR #54 routes through Phase 177 `config_push_queue` which has DB persistence + seq_num + ack-tracking + reconnect-replay (CP-02). Failure-mode #7 retired for this surface. Other surfaces still using `agent_senders.send()` directly are out-of-scope for PR #54 but identified as future-PACT targets. |
| **V1 process-mess Category D (Schema/config drift) at billing_session DB table** | **NOT-APPLICABLE-TO-V2 (in this PR)** — PR does NOT modify billing_session schema | Boundary unchanged; out-of-scope for this PR. Wave 1 W1-S2 already addressed wallet_redemptions FK at the same DB. billing_session schema audit is separate Wave 1 W1-B work item. |
| **V1 process-mess Category F (audit-blind proxy checking)** at server→agent notification | **ROOT-CAUSED-AND-FIXED** (closed by THIS PR) | PR #49 used `.is_ok()` on `send()` as proxy. PR #54 uses DB row state machine + audit-log row + ConfigAck round-trip. |
| **V1 process-mess Category E (recovery cascade) at billing_paused flag loss** | **ROOT-CAUSED-AND-FIXED** (closed by THIS PR) | Loss of billing_paused flag = re-armed orphan-auto-end cascade. PR #54 makes flag delivery durable via DB-persisted queue + reconnect-replay. Cascade structurally cannot re-arm from undelivered flag. |
| **HOT_RELOAD_FIELDS hardcoded allowlist (V1 process-mess Category B + I — boot resilience + config persistence)** | **UNRESOLVED** — open RCA item, out-of-scope for PR #54 | HOT_RELOAD_FIELDS is a const array in rc-agent source — adding new fields requires rc-agent fleet roll. The substrate-reuse benefit (server-only-roll for next similar feature) kicks in for fields that ALREADY have a wire-up. Captured as future-PACT: dynamic config-field registry via `feature_flags` table. Not blocking PR #54 merge (this PR PAYS the migration cost, future features collect savings — explicit per Phase 0 triage). |

**Net past-bug disposition:** PR #54 closes 3 V1-shaped footguns (Category F audit-blind, Category E cascade, §S-61 failure-mode #7) for the billing_paused surface specifically. Opens 1 future-PACT (HOT_RELOAD_FIELDS dynamic registry). No regressions introduced.

---

## §4 — V2-alignment delta

**V2 doctrine anchors checked:**
- `v2-skeleton/05-definition-of-done.md` — V2 skeleton layer = the connective tissue V1 lacked; ad-hoc wire variants are V1-shaped antipattern, substrate-routing is V2-shaped
- `v2-skeleton/01-skeleton-architecture.md §40` — Phase 177 `config_push_queue` IS the V2-P1-CONFIG-SERVICE substrate that PR #54 routes through
- V2-MASTER-STATE §S-117 substrate-readiness scorecard — billing-class surface scored PARTIAL under "audit-trail" + "delivery-guarantee" sub-criteria pre-PR-#54; PR #54 advances both criteria to PASS for this specific control path

**What the boundary should look like under V2 doctrine:**
1. Server→agent state changes go through `config_push_queue` (DB-persisted) + `ConfigPush` wire variant (existing) + `ConfigAck` round-trip (existing) + `config_audit_log` (existing). No new wire variants for each new field; per-field semantics via the `HashMap<String, serde_json::Value> fields` payload.
2. Agent-side hot-reload via `failure_monitor` watch-channel `send_modify()` (existing) — composes with crash-path agent→server `AgentMessage::BillingPaused` so both paths converge on same flag (V2 substrate principle: organ silos closed via skeleton).
3. Graceful version skew: old agent receiving unknown field → log+ignore (no serde decode failure). This is V2-P1-CONFIG-SERVICE design intent (Phase 0 triage Q2 §3).

**Pre-PR-#54 boundary state vs V2 doctrine:**
| Property | Pre-PR-#54 (PACT-008 / PR #49 path) | V2 doctrine target | PR #54 state |
|---|---|---|---|
| Sequence number | absent | required (Phase 177 substrate) | PASS |
| Agent ack | absent | required (ConfigAck) | PASS |
| DB persistence | absent (fire-and-forget) | required (config_push_queue) | PASS |
| Audit trail | absent | required (config_audit_log) | PASS |
| Retry-on-disconnect | impossible | possible (CP-02 reconnect-replay) | PASS |
| Graceful version skew | hard-fail (serde decode error) | warn-and-ignore | PASS |
| Wire surface stability | grows per-field (anti-pattern) | stable enum + extensible field-payload | PASS |
| Audit-trail behavior | proxy-check (.is_ok()) | row-state-machine | PASS |

**Delta gap (V2 doctrine vs current state after PR #54):**
- HOT_RELOAD_FIELDS hardcoded allowlist on agent side — V2 ideal would be dynamic registry. **Justification for temporary V1-shape retention:** the first migration always carries the wire-up cost; future fields collect the substrate-reuse benefit. Captured explicitly in Phase 0 triage §Q2 cost refinement. Follow-up trigger: when ≥3 hot-reload fields exist, file PACT for dynamic registry.

---

## §5 — Proposed change, V2-framed

**Proposed action:** Captain per-PR merge authorization for racecontrol#54 (PACT-20260429-013 Phase 1+2). Merge unblocks:
- V2 substrate-readiness: billing-class delivery-guarantee + audit-trail sub-criteria PARTIAL → PASS (V2-MASTER-STATE §S-117 lens)
- V2-LIVE customer-experience: SESSION-01 orphan auto-end no longer destroys manual-paused sessions when customer closes game (canonical day §4 14:25→14:42 game-switch scenario); behavior was correct under PR #49 happy-path, now durable under WS-drop adversarial-path
- Future similar features: server-only-roll for subsequent agent-flips-flag-based-on-server-state work (PACT-013 pays the migration cost; the next PACT collects the savings)

**Kaizen retention of V1-shape elements with explicit retire triggers:**
- HOT_RELOAD_FIELDS hardcoded allowlist — RETAIN this PR; RETIRE when ≥3 hot-reload fields exist (file PACT for dynamic registry)
- `.gitignore` cross-platform path leak suppression — RETAIN this PR; RETIRE when `cfg(target_os)` fix lands in `process_guard_server`

**V2 doctrine alignment statement (required per §S-146):** *PR #54 moves the server→agent billing_paused control plane from PR #49 ad-hoc wire variant to V2-P1-CONFIG-SERVICE substrate (Phase 177 `1fc92867`); closes 3 V1-shaped antipatterns (audit-blind proxy checking · recovery-cascade re-arm via flag loss · §S-61 PART 41 failure-mode #7); opens 1 future-PACT (HOT_RELOAD_FIELDS dynamic registry).*

**Rollback plan:** revert this PR + redeploy = restores PR #49 wire-protocol path. CI clean state (5 checks GREEN) supports clean revert window.

**Cross-references:**
- Parent PACT: `comms-link/proposals/PACT-20260429-013-billing-pause-via-config-push-queue-redesign.md`
- Phase 0 triage: `comms-link/docs/PACT-013-PHASE-0-TRIAGE.md`
- Phase 5 prevention hook (already shipped to comms-link `c8e400f`): `comms-link/.claude/hooks/protocol-addition-v2p1-check.sh` — gates new `+pub` enum variants in `crates/rc-common/src/protocol.rs` behind a `V2-P1 substrate considered:` reason field
- Mechanism-trust-check (this RCA companion): `racecontrol/.planning/specs/v2/MECHANISM-TRUST/config-push-queue-2026-05-11.json` (filed same session)

---

## §6 — NOT TESTED (per CGP H3)

- **Linux cargo build of rc-agent** — pre-existing Windows-only errors (process_guard.rs creation_flags, lock_screen native_lock) block Linux cargo check; needs `cargo build` on James .27 before merge. CI ran on Windows runner (build SUCCESS 2026-04-30).
- **E2E manual-pause behavior post-deploy** — staff manual-pause from kiosk → server `set_billing_status` → `config_push_queue` row inserted → WS delivered → rc-agent log shows `ConfigPush: billing_paused = true` → close game during pause → orphan auto-end does NOT fire 5min later. Test plan owns by amend-PR if pre-merge required, else post-deploy verification on .23 + Pod canary.
- **Replacement integration test** — `test_billing_paused_resumed_roundtrip` removed in PR (variant doesn't exist anymore); replacement at racecontrol-crate `test_billing_paused_via_config_push_roundtrip` (mentioned in protocol.rs comment, not yet authored).
- **Cloud parity (Bono VPS racecontrol redeploy)** — DEPLOY PARITY rule requires same deploy on Bono VPS after .23. Open.
- **Pod 5 / POS rc-agent fleet roll** — PACT-013 §EVIDENCE explicitly requires fleet roll for the HOT_RELOAD_FIELDS extension; not yet executed.
- **MMA Step 1 DIAGNOSE on this RCA** — required per §S-146 foundational-boundary escalation (billing). Captain-pending: OpenRouter spend $0.05-0.10 + 5-model consensus authoring (≥3 vendor families per Phase 3 ACTIVE). PENDING Captain auth in this session.

---

## §7 — Captain-asks

1. **Per-PR merge auth** on racecontrol#54 (post-§S-146 retroactive RCA + post-§S-172 mechanism-trust-check both filed this session)
2. **MMA Step 1 DIAGNOSE auth** — proceed with 5-model OpenRouter run (~$0.05-0.10) on this RCA per foundational-boundary escalation? Or defer DIAGNOSE as a Phase-2 follow-up gate and merge first?
3. **Fleet-roll sequence** — server roll (.23) before pod fleet roll, or atomic single-deploy via deploy-pipeline?
4. **Phase 0.5 follow-up** — replacement integration test (`test_billing_paused_via_config_push_roundtrip`) — author pre-merge or post-merge?

---

## §8 — Composes-with

- **§S-146** V1↔V2 RCA doctrine (parent — this RCA is canonical-format application)
- **§S-172** mechanism-trust-check (sibling — companion `config-push-queue-2026-05-11.json` filed)
- **§S-186** small-fix fast-lane (NOT eligible — PR is feat-class + >200 LOC + protocol change; full §S-146 path applies)
- **§S-117** V2-MASTER-STATE substrate-readiness scorecard (billing-class delivery-guarantee criteria advancement)
- **CGP H1** PROBLEM/SYMPTOMS/PLAN (RCA informs PLAN; PLAN does not substitute for RCA)
- **PACT-20260429-013** bilateral ratification (Phase 0+0.5 GREEN-LIGHT; CI all green; merge gate is per-PR Captain auth only)
- **PR #49 / PACT-20260428-008** (immediate predecessor — PR #54 closes the open §S-146 retroactive RCA item PR #49 opened)

— bono · 2026-05-11 ~12:30 IST · §S-146 canonical-format RCA for racecontrol#54 · ready for Captain per-PR merge auth disposition
