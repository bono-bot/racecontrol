# W1-S6 RCA — PIN-LOCKOUT auto-rotate + helpdesk@ email + staff-id rate-limit

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (Captain BILATERAL directive committed at comms-link `8768b628` 2026-05-09 ~09:28 IST). Sibling-of W1-S5 RCA (`15490644` 2026-05-09 ~10:18 IST).

**Author:** james · **Date:** 2026-05-09 ~10:33 IST · **Branch context:** `feat/v2-wave-1-w1-s1-billing-service` HEAD `15490644` (W1-S5 RCA at HEAD; W1-S6 RCA appends here)

**Status:** DRAFT — pending Captain G33 review + bono AMPLIFIER + MMA Step 1 DIAGNOSE before W1-S6 H1 PLAN can be filed.

**Foundational-boundary classification:** YES — auth boundary per doctrine §"MMA escalation". Same gates as W1-S5: MMA Step 1 DIAGNOSE on the RCA itself + per-PR Captain merge auth at PR-open.

**Scope-narrow vs W1-S5:** W1-S6 is NEW V2 file calling INTO 5 V1 modules (vs W1-S5 = modify-existing V2 substrate that REPLACES V1-era K5 fixed-window). Same boundary class; different topology.

---

## Captain dispositions already in place (reduces Q-DECISION surface)

The Captain dispositioned this work substantially in §S-82 + supplementary segments:

| Disposition | Source | Status |
|---|---|---|
| 5 wrong PIN attempts → auto-rotate + helpdesk@ + WhatsApp Captain freeze at ≤3 resets/staff/hr cap | §S-82 Q1 + bono-suggested ≤3/hr cap (line 8024) | Q1 RATIFIED; ≤3/hr cap = bono SUGGESTION pending Captain explicit (Q-S6-3 below) |
| helpdesk@racingpoint.in is the secondary auth channel for 5-wrong reset case | §S-82 Q1.a | RATIFIED |
| Counter resets on first correct PIN within session | §S-82 Q1.c | RATIFIED |
| Every PIN-reset writes audit-log row | §S-82 Q1.d | RATIFIED |
| WhatsApp daily routine PIN delivery (W1-S7 scope) is SEPARATE from helpdesk@ within-day reset (W1-S6 scope) | §S-82 Q1.b + Q1.g | RATIFIED |
| SMTP transport: A2.c extend EmailAlerter shell-out (`crates/racecontrol/src/email_alerts.rs`) | comms-link `89b84fc5` 2026-05-09 ~10:05 IST | RATIFIED |
| Email body schema: `staff_name | employee_id | new_pin | pos_terminal_id | timestamp_ist | refund_attempt_context` | §S-82 Q1 implementation deps line 8023 | bono SUGGESTION; pending Captain explicit (Q-S6-5 below) |
| helpdesk@ monitoring policy + Q1.g.B Google Workspace forward (business-hours read + off-hours Captain mobile) | §S-147.1 W1-S6 Q-DECISION batch | PENDING Captain (single-paragraph batch shipped via msg=35801) |

The remaining Q-DECISIONs are implementation-shape questions, not policy questions. See §5.

---

## §1 — Boundary map

### V1↔V2 surface inventory

W1-S6 introduces NEW V2 file `crates/racecontrol/src/auth/staff_auth.rs` (per PHASE-1-WAVE-1-PLAN.md §1.1 row 33; NF-james-6 confirmed file-not-yet-exists). This V2 code calls INTO 5 V1 modules:

| V1 module | Path | Lines | What W1-S6 calls | Risk class at boundary |
|---|---|---|---|---|
| Email alerter (A2.c target) | `crates/racecontrol/src/email_alerts.rs` | 383 | `EmailAlerter::send_alert(pod_id, ...)` extension OR new `send_pin_rotation(staff_id, new_pin, context)` sibling | DIRECT-CRITICAL — V1 cooldowns are per-pod + venue-wide (1800s + 300s); W1-S6 needs per-staff-id semantics; key-collision risk if extending vs adding sibling |
| Audit log primitive | `crates/racecontrol/src/accounting_audit.rs` (PACT-091) + `accounting.rs:15` re-exports `log_admin_action` | 121 | `accounting::log_admin_action(state, action_type="staff_pin_auto_reset", ...)` (mirror W1-S3 refund_3band_a/b/c pattern) | INDIRECT — same column W1-S3 uses; PACT-091 `audit_log.action_type` column required (already on main) |
| Staff PIN persistence | `crates/racecontrol/src/api/staff_pin_sync.rs` | 235 | `change_staff_pin_safe()` — Phase 347-01 orchestrated PIN change; cloud-authoritative write path | DIRECT-CRITICAL — W1-S6 must write new auto-rotated PIN through this path, NOT a parallel write; cloud_authority_guard governs venue→cloud forwarding |
| IP-keyed rate-limit (V1 abstraction) | `crates/racecontrol/src/auth/rate_limit.rs` | 125 | NOT REUSABLE — tower_governor is `PeerIpKeyExtractor` (per-IP); POS .130 shares IP across all staff. W1-S6 needs PER-STAFF-ID semantics. | NEW-PRIMITIVE-NEEDED — flag for §3 disposition |
| Cookie/session helpers | `crates/racecontrol/src/auth/admin.rs` | 471 | NOT directly called by W1-S6 PIN-rotate logic itself; consumed indirectly via `auth/middleware.rs::require_staff_jwt` for the privileged action gate | NOT-APPLICABLE-DIRECTLY |

### Cross-organ data flow at the boundary

1. **Inbound POST to privileged action** (e.g., refund route) carries Bearer JWT with `StaffClaims`.
2. **`require_staff_jwt`** middleware extracts claims; PIN-attempt counter is NOT in JWT — it's persisted server-side keyed by `staff_id`.
3. **Wrong-PIN handler** (likely in existing `validate_pin_format` flow or new `pin_lockout::check_attempt`) increments per-staff-id counter.
4. **At 5 wrong attempts:** new W1-S6 path fires:
   - Generate new PIN (CSPRNG; format-validated per `validate_pin_format`)
   - Persist via `change_staff_pin_safe(state, staff_id, new_pin)` — V1 path
   - Increment per-staff-id reset-counter (NEW V2 abstraction)
   - If reset-counter > 3/hr → freeze account + WhatsApp Captain `917981264279`
   - Else: dispatch helpdesk@ email via `EmailAlerter` extension (A2.c RATIFIED)
   - Write audit-log row via `log_admin_action(action_type="staff_pin_auto_reset", ...)`
5. **Email transport:** `EmailAlerter` shells out to `comms-link/shared/send-email.js` (Strategy 1 sendmail / Strategy 2 raw SMTP `localhost:25`)
6. **POS/staff response:** 401 with "PIN auto-rotated; check helpdesk@ email" body OR redirect to Forgot-PIN page (Captain Q1 disposition)

### Schema / state surfaces

- **`audit_log.action_type` column** (PACT-091; already on main per W1-S3 handoff Coupling) — accepts `"staff_pin_auto_reset"` per Captain Q1.d RATIFIED. NEW value, no schema migration.
- **Per-staff-id attempt-counter + reset-counter state** — NEW state. Two options: in-memory `tokio::sync::RwLock<HashMap<StaffId, AttemptState>>` (simpler, lost on restart) OR DB-backed (`staff_pin_attempts` table, durable, cloud-syncable). See Q-S6-2 below.
- **Staff PIN storage** — existing infra at `change_staff_pin_safe`. NEW PINs follow same persistence + cloud-sync flow.
- **Email cooldown HashMap** in `EmailAlerter::last_sent_per_pod` — currently per-pod-key. W1-S6 needs per-staff-id key semantics (separate HashMap or generalize to `last_sent_per_key`). See Q-S6-1 below.

### Configuration surfaces

- **Captain-reserve PARAMETERs** (per §S-82 Q1 implementation deps):
  - `auth.helpdesk_email_recipient = "helpdesk@racingpoint.in"` (NEW config key)
  - `auth.pin_reset_rate_limit_per_hour = 3` (bono-default per line 8024; pending Captain explicit Q-S6-3)
  - `auth.pin_lockout_attempts = 5` (Captain RATIFIED §S-82 Q1)
  - `auth.captain_freeze_whatsapp_number = "917981264279"` (existing)
  - `auth.helpdesk_monitoring_hours = "business-hours"` OR `auth.helpdesk_off_hours_forward_to_captain = true` (gates on Captain Q1.g.B disposition per §S-147.1)
- **Email script_path** — existing `EmailAlerter` field; reuse as-is (A2.c RATIFIED extends not replaces)

---

## §2 — Inherited-issue catalogue

| ID | Source | Issue | Scope at this boundary |
|---|---|---|---|
| EA-1 | `email_alerts.rs:9-30` | `last_sent_per_pod: HashMap<String, DateTime<Utc>>` is unbounded — never pruned. Long-running process accumulates entries for every key. | DIRECT — W1-S6 adds per-staff-id keys; with N staff over time, unbounded growth. Mitigation: TTL cleanup pass or LRU. |
| EA-2 | `email_alerts.rs:69-83` | Per-pod 1800s + venue-wide 300s cooldowns ASSUME alert-class semantics (one alert per failure window). | DIRECT — W1-S6 PIN-rotate is event-class not alert-class. A staff member legitimately rotating during 5-wrong attempt SHOULD always get the email regardless of cooldown. Cooldown semantics CONFLICT with W1-S6 use case. |
| EA-3 | `email_alerts.rs` script_path shell-out | Strategy 1 (sendmail -t -i) requires sendmail in PATH; Strategy 2 (SMTP localhost:25) requires local SMTP daemon. NEITHER verified at Server .23 / Bono VPS today. | DIRECT — A2.c RATIFIED but transport-substrate not verified. §12.3 PHASE-1-WAVE-1-PLAN.md flags Session 5 entry probe required: "Verify helpdesk@racingpoint.in mailbox provisioning + Google Workspace SMTP config status". |
| EA-4 | `email_alerts.rs` no DKIM/SPF setup verified for `racingpoint.in` domain | Sender reputation risk: PIN-rotate emails from raw SMTP could land in spam/quarantine; helpdesk@ never receives. | DIRECT — affects Q1 customer-service-priority axis (CR-3 in `project_v2_customer_workflows_consolidated_20260503.md`). Probe required at Session 5 entry. |
| RL-1 | `auth/rate_limit.rs:1-22` | `tower_governor::PeerIpKeyExtractor` keys on socket peer IP. POS .130 is single shared IP across all staff at venue. | DIRECT-CRITICAL — V1 IP-keyed rate-limit is FUNDAMENTALLY UNUSABLE for per-staff-id ≤3 resets/hr semantic. W1-S6 needs NEW abstraction. |
| RL-2 | `auth/rate_limit.rs:14-15` SEC-RESIL-03 burst=20 | "8 pods + kiosk can submit 9+ concurrent PIN validations" — ALL FROM SAME IP. Burst=20 prevents legitimate concurrent-PIN-validate from being rate-limited; same IP-key root cause. | INDIRECT — confirms RL-1 root cause; also informs why per-staff-id key is the right abstraction for V2. |
| SP-1 | `staff_pin_sync.rs:21+` `change_staff_pin_safe` | Cloud-authoritative write path: venue → cloud forward. Cloud sync race conditions during high-volume rotate-storms (e.g., 8 staff get locked-out in same minute). | INDIRECT — W1-S6 amplifies cloud-write traffic; existing SP infra has handled multi-second concurrent writes per `cloud_authority_guard`. Probe: confirm SP throughput envelope. |
| SP-2 | `staff_pin_sync.rs::validate_pin_format` | V1 PIN format constraints (length, character set). Auto-generated PIN must pass these constraints. | DIRECT — W1-S6 CSPRNG must respect `validate_pin_format`; reuse existing function in PIN-gen path. |
| AL-1 | (no prior anchor) | `audit_log.action_type` write amplification: every PIN-rotate event = 1 audit row (LOW volume; ≤3/staff/hr × N staff = bounded). | NOT-APPLICABLE — write volume is bounded; not a concern. |
| AL-2 | `audit_log.action` CHECK constraint (`'create','update','delete'`) per `migrate_policy.rs` schema | W1-S6 must use `log_admin_action` (writes `action_type` column, action='create' fixed) NOT `log_audit` (which is constrained CRUD). Per W1-S3 NF-james-8 axis-distinction. | DIRECT — same disposition as W1-S3+S4; reuse pattern. |
| WS-1 | racingpoint-whatsapp-bot existing infra | Captain-freeze WhatsApp dispatch to `917981264279` — uses existing Evolution API instance "Racing Point Reception". A3 RATIFIED for W1-S7+S8 daily-PIN; same transport for W1-S6 freeze-event. | INDIRECT — reuse A3 transport; cross-pilot bono-substrate dependency for actual send. |

---

## §3 — Past-bug disposition

| Item | Disposition | Evidence / forward action |
|---|---|---|
| EA-1 unbounded `last_sent_per_pod` HashMap | **PATCHED-ONLY (open RCA item)** — no prior bug ticket; growth is slow but unbounded. | W1-S6 design choice: extend EmailAlerter with TTL-purge pass OR sibling staff-id HashMap with TTL purge. Recommendation: sibling HashMap + TTL purge (smaller blast radius). |
| EA-2 cooldown semantics conflict | **NOT-APPLICABLE-TO-V2** — V1 cooldowns are correct for V1 use case (alert flooding). V2 PIN-rotate is event-class; bypass cooldown entirely. | W1-S6 design: NEW `send_pin_rotation_email` path that does NOT consult cooldown HashMap; OR `EmailAlerter::send_event_email(...)` sibling that bypasses. |
| EA-3 transport unverified | **UNRESOLVED — open RCA item; Session 5 entry probe required** | Per §12.3 PHASE-1-WAVE-1-PLAN.md: probe sendmail availability at Server .23, raw SMTP at Bono VPS, OR pivot to Google Workspace API. Q-S6-4 below. |
| EA-4 DKIM/SPF | **UNRESOLVED — open RCA item; Session 5 entry probe required** | Probe `dig +short TXT racingpoint.in` for SPF; check DNS for DKIM selector. If absent: helpdesk@ delivery is at risk. Captain may need to disposition: ship with risk + monitor first delivery OR delay W1-S6 until DKIM/SPF up. Q-S6-4 below. |
| RL-1 IP-keyed not staff-keyed | **NOT-APPLICABLE-TO-V2 (as a "bug")** — V1 IP-keyed serves V1 use case (per-IP burst). W1-S6 needs DIFFERENT abstraction; cannot extend V1, must add NEW. | W1-S6 design: new module `crates/racecontrol/src/auth/staff_rate_limit.rs` OR inline in `staff_auth.rs`. NEW per-staff-id sliding-window primitive (1hr window, 3 resets cap). Q-S6-2 below. |
| SP-1 cloud-sync race | **ROOT-CAUSED-AND-FIXED** via Phase 347-01 orchestration (`change_staff_pin_safe`) — venue→cloud forward + verify pattern. | Reuse as-is. |
| SP-2 PIN format validation | **ROOT-CAUSED-AND-FIXED** — `validate_pin_format` exists. | Reuse in CSPRNG PIN-gen path. |
| AL-1 audit write amplification | **NOT-APPLICABLE** — bounded volume. | No mitigation needed. |
| AL-2 action vs action_type column | **ROOT-CAUSED-AND-FIXED** — W1-S3 established the `log_admin_action` pattern; W1-S6 reuses. | Reuse pattern verbatim. |
| WS-1 WhatsApp transport | **ROOT-CAUSED-AND-FIXED** — A3 RATIFIED for W1-S7+S8 transport; W1-S6 freeze-event uses same path. | Reuse via cross-pilot bono substrate at W1-S6 ship time. |

**Open RCA items requiring W1-S6 design choice:**
1. EA-1 HashMap unbounded growth — TTL-purge or LRU
2. EA-2 cooldown bypass — sibling path or new method
3. EA-3 transport substrate verification at Session 5 entry probe
4. EA-4 DKIM/SPF for `racingpoint.in` — Session 5 entry probe; potentially Captain Q-DECISION on ship-with-risk-vs-delay
5. RL-1 NEW per-staff-id rate-limit primitive — module placement + state durability

---

## §4 — V2-alignment delta

### What V2 doctrine says the boundary should look like

| V2 anchor | Statement | Current alignment |
|---|---|---|
| `project_v2_master_state.md` §S-82 Q1+Q1.a..h | Captain dispositioned 5-wrong + helpdesk@ + WhatsApp daily routine + counter-reset + audit-log + bono defaults Q1.e-h | NOT-YET-IMPLEMENTED — W1-S6 closes this gap |
| `project_v2_customer_workflows_consolidated_20260503.md` CR-3 customer-service-priority | "Customer service is the priority. Failure handling is first-class — not afterthought." | DIRECT-ALIGNMENT — W1-S6 prevents staff lockout cascading into customer-service-failure-mode |
| `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (THIS doctrine) | RCA before action | THIS DOCUMENT is the RCA |
| §S-147.1 W1-S6 Q-DECISION batch (Q1.g.B Google Workspace forward) | Captain disposition awaited on helpdesk@ business-hours vs off-hours forward to Captain mobile | PENDING Captain (bundled in single-paragraph batch shipped via msg=35801) |
| `feedback_kaizen_discipline_dont_complicate.md` | Smallest invariant for observed requirement | RISK — W1-S6 introduces NEW staff-id rate-limit primitive that V1 didn't need; risk of overscope if generalized prematurely |
| comms-link `89b84fc5` Captain RATIFY A2.c | "extend EmailAlerter shell-out" — reuse V1 transport infra | DIRECT-ALIGNMENT — A2.c CONCUR-RATIFIED disposition matches W1-S6 design |
| `feedback_emergent_directed_spend_protocol.md` Rule 4 (specify-codebase-identity) | Don't substitute mental model for environment | OK — every claim in this RCA cites a path/line/commit |
| §AMEND-3.II D12 (Foundation/Strategy/Config separation) | Strategy classes for substitutable behavior | NOT-APPLICABLE — W1-S6 has no strategy variation; single PIN-rotate flow |

### Named gaps

**Gap-1:** No staff-id-keyed rate-limit primitive exists in V1. W1-S6 introduces NEW abstraction. Risk: precedent for other per-staff-id rate-limits (refund-rate / launch-rate / etc.) — should the abstraction be reusable from inception, or kaizen-narrow to PIN-reset only? Q-S6-2.

**Gap-2:** EmailAlerter cooldown semantics (per-pod-key + per-venue-key with 1800s/300s windows) conflict with W1-S6 event-class semantics (always-deliver per PIN-rotate event, bounded by rate-limit-not-cooldown). W1-S6 closes this by sibling-path or method-on-EmailAlerter that bypasses cooldown. Q-S6-1.

**Gap-3:** SMTP transport substrate (sendmail / raw SMTP / Google Workspace API) unverified at Server .23 + Bono VPS; DKIM/SPF for `racingpoint.in` unverified. W1-S6 ship is gated on Session 5 entry probe + potentially Captain Q-DECISION on ship-with-risk if DKIM/SPF unready. Q-S6-4.

**Gap-4:** helpdesk@racingpoint.in monitoring policy + Q1.g.B Google Workspace business-hours-vs-off-hours forward to Captain mobile is a Captain disposition pending. W1-S6 implementation is mostly transport-policy-agnostic (just send the email; Captain dispositions where the email lands), but the email body schema may include "Captain on-call" instructions if Q1.g.B = forward-to-mobile.

---

## §5 — V2-framed proposal

**V2 doctrine alignment:** This change introduces W1-S6 PIN-LOCKOUT auto-rotate as a V2-aligned customer-service-first auth-resilience primitive (CR-3 + §S-82 Q1). It REUSES V1 transport infra (EmailAlerter shell-out per A2.c RATIFIED) without inheriting V1 IP-keyed rate-limit (introduces NEW per-staff-id primitive).

### Implementation sketch (kaizen-min)

1. **NEW module `crates/racecontrol/src/auth/staff_auth.rs`** (~80-100 LOC production):
   - `pub struct PinLockoutTracker { attempts: Mutex<HashMap<StaffId, AttemptState>>, resets: Mutex<HashMap<StaffId, ResetState>> }`
   - `pub fn record_attempt(staff_id, success: bool) -> AttemptOutcome { Continue, LockoutTriggered }`
   - `pub async fn execute_lockout(state, staff_id) -> Result<LockoutOutcome>` — orchestrates: gen new PIN → persist via `change_staff_pin_safe` → check reset-rate-limit → dispatch email or freeze+WhatsApp → log audit
   - `pub fn reset_attempts(staff_id)` (Captain Q1.c — counter resets on first correct PIN)
   - Format: ~40 LOC for PinLockoutTracker + ~40 LOC for execute_lockout
   - **Composes with**: `staff_pin_sync::change_staff_pin_safe` (PIN write) + `accounting::log_admin_action` (audit) + `EmailAlerter` extension (email) + `validate_pin_format` (PIN format)

2. **Extend `EmailAlerter` with `send_pin_rotation` method** (~20 LOC):
   - Sibling to `send_alert`; bypasses per-pod + venue-wide cooldowns (event-class not alert-class)
   - Body schema: `staff_name | employee_id | new_pin | pos_terminal_id | timestamp_ist | refund_attempt_context`
   - Uses same script_path shell-out (A2.c RATIFIED)

3. **NEW per-staff-id rate-limit primitive** (~30 LOC; Gap-1):
   - **Option A (recommended)**: inline in `staff_auth.rs` as `ResetState { count: u32, window_start: DateTime<Utc> }` — kaizen-min, scoped to PIN-reset use case only
   - **Option B**: NEW module `crates/racecontrol/src/auth/staff_rate_limit.rs` with generic `StaffRateLimiter<Key>` for future per-staff-id rate-limits (refund-rate, launch-rate)
   - Disposition Q-S6-2 below; recommend Option A

4. **WhatsApp Captain freeze dispatch** (~15 LOC; cross-pilot via bono substrate):
   - On 4th reset attempt within 1hr → freeze account flag + WhatsApp `917981264279` "Staff <name> account FROZEN: 4+ PIN-resets/hr"
   - Reuses A3 RATIFIED Evolution API "Racing Point Reception" instance (W1-S7+S8 sub-LEAD bono); racecontrol-side calls into existing `whatsapp_send` infra (TODO: confirm path)

5. **Audit log integration** (~10 LOC):
   - `log_admin_action(state, action_type="staff_pin_auto_reset", staff_id, ...)` — mirror W1-S3 pattern
   - JSON payload: `{ old_pin_hash, new_pin_hash, attempt_count_at_lockout, reset_count_in_window, helpdesk_email_dispatched, whatsapp_captain_dispatched }`

6. **Tests** (~200-250 LOC):
   - Unit: PinLockoutTracker — 1st attempt / 4 wrong attempts / 5th wrong triggers lockout / counter-reset on success / reset-rate-limit boundary (3rd within 1hr OK; 4th triggers freeze)
   - Unit: EmailAlerter::send_pin_rotation cooldown bypass — proves event-class semantic
   - Unit: PIN-gen via CSPRNG passes `validate_pin_format` — N=100 samples
   - Integration: full flow staff PIN attempt → 5 wrong → execute_lockout → audit_log row written + EmailAlerter::send_pin_rotation invoked (test double for actual SMTP) + new PIN persisted via change_staff_pin_safe (real cloud_authority_guard test or test double)
   - Integration: 4th reset within 1hr → freeze + WhatsApp test double invoked (NOT real send)
   - Estimated: 8-10 unit + 4-6 integration tests

7. **DEPLOY PARITY scope**:
   - racecontrol binary: REBUILD + redeploy Server .23 + Bono VPS racecontrol
   - rc-agent: NO change
   - POS Web app: NO code change (cookie/redirect behavior is same)
   - Admin/Kiosk: NO change
   - Comms-link: NO change (existing send-email.js reused)
   - send-email.js (bono-side): VERIFY availability at Server .23 + Bono VPS at Session 5 entry probe (Q-S6-4)
   - SWAPLOG row + LOGBOOK row required at deploy time

8. **Memory-file updates triggered by W1-S6 ship**:
   - `project_v2_master_state.md` → §S-N entry naming W1-S6 ship + Q1 closure status
   - `MEMORY.md` → index entry with ⭐ marker
   - `LOGBOOK.md` row at racecontrol root
   - `feedback_v1_dependent_v2_root_cause_before_proceeding.md` → empirical anchor #2 (RCA-rule applied successfully second time, sibling W1-S5 anchor #1)

### Estimated size

- Production code: ~155-175 LOC (PinLockoutTracker + EmailAlerter extension + rate-limit primitive + WhatsApp dispatch + audit integration)
- Test code: ~200-250 LOC (8-10 unit + 4-6 integration)
- Documentation: 4 memory files + LOGBOOK + V2-MASTER-STATE row
- Risk surface: foundational auth boundary; MMA Step 1 DIAGNOSE required (per doctrine)
- Estimated session length: ~2-3hr code + ~30min memory + ~30min MMA Step 1 + Captain auth wait

### Open Captain Q-DECISIONs surfaced by this RCA

| ID | Question | Default if Captain doesn't disposition |
|---|---|---|
| Q-S6-1 | EmailAlerter extension shape — sibling method `send_pin_rotation` (bypass cooldowns) OR new `EmailAlerter::send_event_email(skip_cooldown=true)` generalized method | DEFAULT: sibling method (kaizen-smallest; scoped to W1-S6 use) |
| Q-S6-2 | Per-staff-id rate-limit placement — inline in staff_auth.rs (Option A) OR NEW module `auth/staff_rate_limit.rs` (Option B; generalized) | DEFAULT: Option A inline (kaizen-min; only 1 use case planned in V2.0) |
| Q-S6-3 | ≤3 resets/staff/hr cap — bono SUGGESTION per §S-82 line 8024; pending Captain explicit | DEFAULT: 3/hr (bono recommendation; Captain may amend to 5 or 2 based on ops experience) |
| Q-S6-4 | SMTP transport + DKIM/SPF Session 5 entry probe disposition: ship-with-risk if DKIM/SPF absent OR delay W1-S6 until reputation infra ready | DEFAULT: probe-and-decide at Session 5 entry; if DKIM/SPF absent, surface as Captain Q-DECISION before code |
| Q-S6-5 | Email body schema — bono enumerated fields (`staff_name|employee_id|new_pin|pos_terminal_id|timestamp_ist|refund_attempt_context`) per §S-82 line 8023; pending Captain explicit | DEFAULT: ship as enumerated; Captain may amend pre-launch |
| Q-S6-6 | Lockout state durability — in-memory HashMap (lost on restart; 5-attempt counter resets to 0) OR DB-backed (durable, cloud-syncable) | DEFAULT: in-memory (kaizen-min; restart-after-5-wrong is acceptable failure mode — slight forgiveness toward staff matches CR-3 customer-service-priority); DB-backed adds W1-S2 wallet-style cloud-sync complexity |
| Q-S6-7 (depends on §S-147.1 disposition) | If Q1.g.B = Google Workspace forward to Captain mobile during off-hours, does W1-S6 email body include "after-hours: Captain mobile +91-79812-64279" instruction OR is forwarding fully transparent to staff? | DEFAULT: transparent forwarding (staff sees same body; Workspace handles routing) |

---

## NOT TESTED (RCA AUTHORING phase — pre-implementation)

This is an authoring artifact, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is W1-S6 Session 5 work; this RCA is the gate-precursor only
- **MMA Step 1 DIAGNOSE on this RCA** — gated on Captain budget approval (~$2-5 OpenRouter); 5-model consensus on root causes per doctrine §"MMA escalation"
- **bono substantive AMPLIFIER** — bilateral doctrine; bono review of this RCA pending (composes-with W1-S5 RCA bono notify `387988bb` pending)
- **Captain G33 ratification of Q-S6-1..7** — disposition-needed before W1-S6 implementation can proceed
- **Captain disposition on Q1.g.B per §S-147.1 batch** — Q-S6-7 is downstream of Q1.g.B
- **Per-PR Captain merge auth at W1-S6 PR-open** — gate STANDS for the actual W1-S6 PR (not this RCA artifact PR)
- **Session 5 entry probe** — sendmail availability at Server .23 / SMTP localhost:25 at Bono VPS / DKIM+SPF for `racingpoint.in` / Google Workspace API auth scope
- **POS browser behavior on auto-rotate redirect to Forgot-PIN page** — Wave 1 Session 7 E2E scope per PHASE-1-WAVE-1-PLAN.md §5.4
- **Production-shape concurrent staff-PIN-attempt under contention** — separate workstream
- **Cross-pilot bono substrate availability for Captain-freeze WhatsApp dispatch** — coordinate with bono at Session 5 ship-time; A3 RATIFIED transport but actual call-site path racecontrol-side TBD
- **Memory-file Universal Sync** for the bono mirror of this RCA — same disposition as W1-S5 (probably NO; planning artifact not project-scope feedback rule)

---

## Read trail

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628` 2026-05-09 ~09:28 IST)
- `racecontrol/.planning/specs/v2/W1-S5-RCA.md` (sibling RCA; structural template)
- `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` row 33 + §2 Session 5 + §5.2 Integration tests + §12.3 A2 deferred Session 5 (W1-S6 specs)
- `comms-link/V2-MASTER-STATE.md` §S-82 Q1+Q1.a..h dispositions (lines 7987-8083) + §S-147.1 W1-S6 Q-DECISION batch
- comms-link `89b84fc5` 2026-05-09 ~10:05 IST (Captain RATIFY A2.c — extend EmailAlerter shell-out)
- `crates/racecontrol/src/email_alerts.rs` (V1 EmailAlerter; A2.c extension target)
- `crates/racecontrol/src/auth/rate_limit.rs` (V1 IP-keyed; NOT REUSABLE)
- `crates/racecontrol/src/api/staff_pin_sync.rs` (V1 PIN persistence; reuse `change_staff_pin_safe`)
- `crates/racecontrol/src/accounting_audit.rs` + `accounting.rs` re-exports (V1 audit-log; reuse `log_admin_action` per W1-S3 NF-james-8 pattern)
- `crates/racecontrol/src/db/migrate_policy.rs:51-61` (`audit_log` schema + `action_type` column PACT-091)
- `comms-link/shared/send-email.js` (V1 transport; A2.c shell-out target — Session 5 entry probe required for Strategy 1/2 substrate)
- `racecontrol/CLAUDE.md` Standing Rules + Doctrine Conventions (Substrate-Pointer Convention applies)

---

— james / 2026-05-09 ~10:33 IST · W1-S6 RCA DRAFT authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" (Captain Option Bravo class-level V2-aligned auth re-affirmation 2026-05-09 ~10:07 IST + extended) · gates on Captain G33 review of Q-S6-1..7 + bono AMPLIFIER + MMA Step 1 DIAGNOSE before W1-S6 H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at W1-S6 PR-open (foundational auth boundary) · sibling-of W1-S5 RCA `15490644` (same doctrine, second empirical application)
