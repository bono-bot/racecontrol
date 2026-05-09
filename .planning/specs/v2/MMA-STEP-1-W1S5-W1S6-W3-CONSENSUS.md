# MMA Step 1 DIAGNOSE — Consensus Report

**Scope**: W1-S5 + W1-S6 + W3 RCA triplet (foundational-boundary class)

**Run**: 2026-05-09 (per Unified MMA Protocol v3.0)
**Models**: deepseek/deepseek-r1-0528 / qwen/qwen3-coder / xiaomi/mimo-v2-pro / google/gemini-2.5-flash / mistralai/mistral-small-2603
**Wall-clock**: ~93s parallel
**Total spend**: $0.1065 (per .tmp/mma-step1-results.json + spend ledger)
**RCAs reviewed**:
- W1-S5 racecontrol `bda06dc8` (290 lines, sliding-window idle-timeout)
- W1-S6 racecontrol `bda06dc8` (273 lines, NEW staff_auth.rs PIN-LOCKOUT)
- W3 racecontrol `78f82654` (388 lines, wallet HOLD-RELEASE-CAPTURE; PACT-024 status corrected via comms-link §S-151)

**Total findings ingested**: 84 (deepseek-r1-0528=12, qwen3-coder=12, mimo-v2-pro=21, gemini-2.5-flash=19, mistral-small-2603=20)

**Cluster summary**:
- CONSENSUS (≥3/5 models agree): **14**
- MINORITY (2/5 models): **5**
- SINGLETON (1/5 models): **20**

Per MMA Protocol v3.0 Step 1, **CONSENSUS findings are mandatory inputs to Step 2 PLAN**. MINORITY findings are evaluated by Step 4 VERIFY adversarial models. SINGLETONS are noted for traceability but not gating.

Cluster ordering: model-coverage descending, then severity descending.

Fuzzy-match parameters: same-RCA-bucket + Jaccard token similarity ≥ 0.18 (title + boundary + rationale[:200]; stopwords stripped).

---

## §1 — CONSENSUS findings (≥3/5 models)

These are the root causes the MMA protocol classifies as REQUIRED inputs to Step 2 PLAN authoring.

### 1. [P0] [W1-S5] create_staff_jwt defaults to 'cashier' role; sliding-window refresh may downgrade non-cashier roles

- **Models**: 5/5 — deepseek-r1-0528, qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.93
- **Inherited from V1**: yes
- **Boundaries cited**: auth/middleware.rs:144-161 | auth/middleware.rs:251-273 | crates/racecontrol/src/auth/middleware.rs:251-273

**Per-model views:**

- **deepseek-r1-0528** [P0 c=0.95]: Cashier role downgrade on token refresh
  - _Rationale_: V1-era create_staff_jwt defaults to 'cashier' role. Sliding-window re-issuance risks downgrading elevated roles (manager/superadmin) if naively reused. Propagates V1 security flaw into V2 auth substrate.
  - _Mitigation_: Implement refresh helper using create_staff_jwt_with_role(claims.role) explicitly in middleware re-issuance path. Add test cases for role preservation.
- **qwen3-coder** [P0 c=0.9]: Role downgrade risk in JWT re-issuance
  - _Rationale_: The V1-era `create_staff_jwt` function defaults to cashier role. If the sliding-window refresh helper naively uses this instead of preserving the existing claim's role, it could silently downgrade non-cashier roles on every refresh, violating auth invariants.
  - _Mitigation_: Extract refresh helper that explicitly preserves `claims.role` from the original token; never default to cashier. Add test case verifying manager/superadmin roles are not downgraded during refresh.
- **mimo-v2-pro** [P0 c=0.85]: Role downgrade on sliding-window refresh via create_staff_jwt default
  - _Rationale_: V1 create_staff_jwt defaults to 'cashier' role. If sliding-window refresh naïvely extracts helper from create_staff_jwt, manager/superadmin roles get downgraded to cashier on every refresh. This is a DIRECT-CRITICAL inherited issue from §2 catalogue.
  - _Mitigation_: Extract mint_refreshed_jwt(claims) helper that uses claims.role (NOT default cashier). Add test 'Manager + superadmin role NOT downgraded on refresh' with role assertions before/after refresh.
- **gemini-2.5-flash** [P1 c=1]: `create_staff_jwt` Cashier-Default Downgrade Risk on Refresh
  - _Rationale_: The V1 `create_staff_jwt` function defaults to issuing a 'cashier' role. If the W1-S5 sliding-window refresh helper is naïvely extracted or implemented from this V1 function, it risks downgrading non-cashier roles (e.g., manager, superadmin) to 'cashier' on every token refresh. T
  - _Mitigation_: Ensure the `mint_refreshed_jwt` helper explicitly uses `claims.role` (i.e., `create_staff_jwt_with_role(claims.role)`) and does not default to 'cashier'. Add a dedicated test case to verify that manager and superadmin roles are preserved across token refreshes.
- **mistral-small-2603** [P0 c=0.97]: create_staff_jwt defaults to 'cashier' role; sliding-window refresh may downgrade non-cashier roles
  - _Rationale_: V1 `create_staff_jwt` hard-codes role='cashier'. The W1-S5 proposal extracts a refresh helper that reuses this function. If a manager or superadmin session refreshes, the role would be silently downgraded to cashier, breaking authorization invariants. The RCA must mandate `create
  - _Mitigation_: Refactor refresh helper to call `create_staff_jwt_with_role(claims.role)`; add unit test that asserts role preservation across 100 refresh cycles; add lint to forbid direct use of `create_staff_jwt` in new code.

### 2. [P0] [W3] F-05 UPDATE-then-SELECT-same-column anti-pattern must not reappear in W3 capture path

- **Models**: 5/5 — deepseek-r1-0528, qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.98
- **Inherited from V1**: yes
- **Boundaries cited**: wallets.rs | wallets.rs:124-338 | crates/v2-db/src/wallets.rs:capture

**Per-model views:**

- **deepseek-r1-0528** [P0 c=0.98]: F-05 anti-pattern reintroduction risk
  - _Rationale_: V1 F-05 UPDATE-then-SELECT pattern caused ₹162.50/customer loss. W3 capture path must update wallet_holds THEN select credits_held - identical hazardous pattern. Not codified in lint/rules.
  - _Mitigation_: Enforce snapshot-read-before-write in capture flow. Add F-05 regression test using historical pattern checks.
- **qwen3-coder** [P0 c=0.98]: F-05 UPDATE-then-SELECT anti-pattern risk in capture path
  - _Rationale_: The F-05 anti-pattern (UPDATE-then-SELECT same column causing data loss) was PATCHED-ONLY in V1 but never codified as lint. W3's capture path introduces complex wallet state transitions that are at high risk of reintroducing this critical pattern without explicit guardrails.
  - _Mitigation_: Add explicit design comment and test case `f05_anti_pattern_regression_check` ensuring all wallet updates read snapshot values BEFORE computing new states. Never read-after-write the same column.
- **mimo-v2-pro** [P0 c=0.95]: F-05 UPDATE-then-SELECT anti-pattern reintroduction risk in capture path
  - _Rationale_: The V1 F-05 bug (₹162.50 customer loss) was caused by UPDATE-then-SELECT same column in end_billing_session. W3 capture path must read credits_held BEFORE updating balance_credits, not after. The anti-pattern is PATCHED-ONLY in V1 and not codified as lint anywhere, making W3 capt
  - _Mitigation_: Add explicit test `f05_anti_pattern_regression_check` that verifies capture reads hold.credits_held snapshot before any wallet UPDATE. Add code comment at capture implementation site referencing F-05 ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md and the ₹162.50 incident.
- **gemini-2.5-flash** [P0 c=1]: F-05 Anti-pattern Reintroduction Risk in Wallet Capture
  - _Rationale_: The F-05 anti-pattern (UPDATE-then-SELECT same column) caused customer loss in V1 and is not codified as a lint or test pattern. The W3 wallet capture path, involving atomic updates to `wallets.balance_credits` and `wallet_holds.credits_held`, is highly susceptible to reintroduci
  - _Mitigation_: Implement explicit F-05 anti-pattern regression checks in W3 tests. Add a code comment at the `WalletService::capture` implementation site explicitly referencing the F-05 anti-pattern and detailing the TOCTOU-safe snapshot-then-update logic. Propose a separate Standing Rule sub-P
- **mistral-small-2603** [P0 c=0.99]: F-05 UPDATE-then-SELECT-same-column anti-pattern must not reappear in W3 capture path
  - _Rationale_: The V1 `end_billing_session` UPDATE wallets THEN SELECT same column for refund calculation caused ₹162.50 customer loss per early-end. The W3 proposal introduces `WalletService::capture` that UPDATEs wallets and INSERTs wallet_redemptions. If the implementation reads `balance_cre
  - _Mitigation_: Enforce invariant in `WalletService::capture`: 'Read wallet snapshot BEFORE any UPDATE; compute delta from snapshot; never read balance_credits after UPDATE.' Add a regression test `f05_anti_pattern_regression_check` that asserts the pattern does not exist.

### 3. [P2] [W1-S5] Plan-author '7-min' vs 4 anchors saying '30-min' idle-timeout inconsistency

- **Models**: 5/5 — deepseek-r1-0528, qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.88
- **Inherited from V1**: yes
- **Boundaries cited**: config/services.rs:442 | config/services.rs:151 | PHASE-1-WAVE-1-PLAN.md rows 21+33

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.85]: Idle timeout configuration inconsistency
  - _Rationale_: PHASE-1-WAVE-1-PLAN.md cites 7-min timeout while V2.1 PIN/middleware/config default to 30-min. Unaddressed V1 documentation flaw propagates operational confusion. Violates §AMEND-3.II D12 Config discipline.
  - _Mitigation_: Amend PHASE-1-WAVE-1-PLAN.md rows 21+33 to 30-min during W1-S5 ship. Add configuration parity test.
- **qwen3-coder** [P2 c=0.75]: Plan-author inconsistency: 7-min vs 30-min timeout
  - _Rationale_: PHASE-1-WAVE-1-PLAN.md cites 7-min idle timeout while all technical anchors (middleware comment, Captain §S-82 Q3, config default) specify 30-min. This creates internal inconsistency requiring explicit disposition.
  - _Mitigation_: Amend PHASE-1-WAVE-1-PLAN row 21+33 from '7-min' to '30-min' in same W1-S5 ship commit. Flag as plan-author typo class resolution per Captain G33 Q-S5-7 disposition.
- **mimo-v2-pro** [P2 c=0.85]: Plan-author '7-min' vs '30-min' inconsistency
  - _Rationale_: PLAN rows 21+33 cite '7-min fixed-window' but 4 anchors (V2.1 PIN, middleware.rs:124, §S-82 Q3, config/services.rs:442) say 30-min. Captain Q-S5-7 dispositioned as plan-author typo class (a). W1-S5 ship must amend plan.
  - _Mitigation_: At W1-S5 ship: amend PHASE-1-WAVE-1-PLAN row 21+33 from '7-min' to '30-min' in same commit. Add comment referencing Captain Q-S5-7 ACCEPT-DEFAULT.
- **gemini-2.5-flash** [P2 c=1]: PLAN-1 '7-min' Idle-Timeout Inconsistency
  - _Rationale_: The `PHASE-1-WAVE-1-PLAN.md` (rows 21+33) cites a '7-min' fixed-window idle-timeout, directly contradicting four other canonical sources (V2.1 PIN, `middleware.rs:124`, Captain §S-82 Q3, `config/services.rs:442`) that all specify '30-min'. This internal inconsistency, while a 'pl
  - _Mitigation_: As per Captain's disposition Q-S5-7, the W1-S5 implementation ship MUST amend `PHASE-1-WAVE-1-PLAN.md` row 21+33 from '7-min' to '30-min' in the same commit, ensuring all documentation aligns with the ratified 30-minute idle-timeout.
- **mistral-small-2603** [P1 c=0.95]: Plan-author '7-min' vs 4 anchors saying '30-min' idle-timeout inconsistency
  - _Rationale_: PHASE-1-WAVE-1-PLAN.md rows 21+33 cite '7-min fixed-window for staff-elevated session' while V2.1 PIN file, middleware.rs:124 doc comment, Captain §S-82 Q3 verbatim, and config/services.rs:442 all say '30-min sliding-window'. This is a plan-author typo class that must be correcte
  - _Mitigation_: Amend PHASE-1-WAVE-1-PLAN.md rows 21+33 to '30-min sliding-window' in the W1-S5 commit. Add a regression test that asserts idle_timeout_secs=1800 in the sliding-window path.

### 4. [P0] [W1-S6] V1 IP-keyed rate-limit is fundamentally unusable for per-staff-id ≤3 resets/hr semantic

- **Models**: 4/5 — qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.95
- **Inherited from V1**: yes
- **Boundaries cited**: auth/rate_limit.rs:1-22 | crates/racecontrol/src/auth/rate_limit.rs | crates/racecontrol/src/auth/rate_limit.rs (IP-keyed) + staff_auth.rs new primitive

**Per-model views:**

- **qwen3-coder** [P0 c=0.95]: IP-keyed rate-limit incompatible with per-staff semantics
  - _Rationale_: V1's IP-keyed rate-limiting is fundamentally incompatible with W1-S6's requirement for per-staff-id ≤3 resets/hr. POS terminals share IPs, making V1 rate-limiting ineffective and necessitating a completely new abstraction.
  - _Mitigation_: Implement NEW per-staff-id rate-limit primitive in `staff_auth.rs` using sliding window (1hr, 3 resets cap). Do NOT attempt to extend V1 IP-keyed infrastructure.
- **mimo-v2-pro** [P0 c=0.85]: IP-keyed rate-limit unusable for per-staff-id ≤3 resets/hr semantic
  - _Rationale_: V1 tower_governor uses PeerIpKeyExtractor. POS .130 shares IP across all staff. Per-staff-id rate-limit (≤3 resets/hr) is FUNDAMENTALLY UNUSABLE with IP key. W1-S6 must introduce NEW per-staff-id primitive, not extend V1.
  - _Mitigation_: New inline ResetState { count: u32, window_start: DateTime<Utc> } in staff_auth.rs (kaizen-min). Add test proving per-staff-id isolation: two staff from same IP get independent counters.
- **gemini-2.5-flash** [P0 c=1]: V1 IP-keyed Rate-Limit Unusable for Per-Staff-ID Semantics
  - _Rationale_: The existing `tower_governor::PeerIpKeyExtractor` in `auth/rate_limit.rs` keys rate limits on the peer IP. For POS terminals, all staff share a single IP, rendering this V1 primitive fundamentally unusable for the W1-S6 requirement of a per-staff-ID PIN reset rate limit (e.g., ≤3
  - _Mitigation_: Implement a new, dedicated per-staff-ID rate-limiting primitive (e.g., `PinLockoutTracker` with `ResetState`) inline within `staff_auth.rs` as per Captain's disposition Q-S6-2, explicitly avoiding any reuse of the V1 IP-keyed rate-limit module.
- **mistral-small-2603** [P0 c=0.99]: V1 IP-keyed rate-limit is fundamentally unusable for per-staff-id ≤3 resets/hr semantic
  - _Rationale_: The V1 `auth/rate_limit.rs` uses `tower_governor::PeerIpKeyExtractor`, keying on socket peer IP. POS .130 is a single shared IP across all staff at the venue. The W1-S6 requirement is per-staff-id ≤3 PIN-reset attempts per hour. The V1 abstraction cannot satisfy this requirement;
  - _Mitigation_: Introduce new module `crates/racecontrol/src/auth/staff_rate_limit.rs` with a `StaffRateLimiter` that keys on staff_id. Default to in-memory HashMap with TTL cleanup; add a TODO for future DB-backed variant if abuse pattern emerges.

### 5. [P1] [W1-S6] EmailAlerter's last_sent_per_pod HashMap is unbounded and never pruned

- **Models**: 4/5 — deepseek-r1-0528, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.86
- **Inherited from V1**: yes
- **Boundaries cited**: email_alerts.rs | email_alerts.rs:9-30 | crates/racecontrol/src/email_alerts.rs

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.9]: Unbounded email cooldown HashMap growth
  - _Rationale_: V1 EmailAlerter's last_sent_per_pod HashMap never prunes entries. W1-S6 adds per-staff-id keys causing unbounded memory growth under long uptimes. Inherited V1 resource leak amplifies in V2.
  - _Mitigation_: Implement LRU cache with TTL eviction in EmailAlerter extension. Add memory monitoring for staff_auth.rs.
- **mimo-v2-pro** [P1 c=0.7]: EmailAlerter unbounded HashMap growth with per-staff-id keys
  - _Rationale_: V1 last_sent_per_pod HashMap is unbounded (never pruned). W1-S6 adds per-staff-id keys. With N staff over time, memory grows unbounded. This is a PATCHED-ONLY inherited issue (EA-1) that W1-S6 exacerbates.
  - _Mitigation_: Sibling staff-id HashMap with TTL purge (e.g., 24h expiry). Add periodic cleanup task or LRU. Alternatively, use per-event bypass (Q-S6-1) that doesn't consult cooldown HashMap at all.
- **gemini-2.5-flash** [P2 c=0.9]: Unbounded `last_sent_per_pod` HashMap Growth in EmailAlerter
  - _Rationale_: The `EmailAlerter` module uses an unbounded `last_sent_per_pod: HashMap<String, DateTime<Utc>>` which, while slow-growing, will accumulate entries for every unique key (pod_id, and now potentially staff_id for W1-S6) over a long-running process. This represents a memory leak that
  - _Mitigation_: Implement a TTL-based pruning mechanism or use an LRU cache for the `last_sent_per_pod` HashMap (or its staff-ID equivalent). The recommendation is to use a sibling HashMap for staff-ID keys with TTL purge, minimizing blast radius and addressing the unbounded growth for W1-S6's n
- **mistral-small-2603** [P1 c=0.96]: EmailAlerter's last_sent_per_pod HashMap is unbounded and never pruned
  - _Rationale_: The V1 `EmailAlerter` uses an unbounded HashMap `last_sent_per_pod` to enforce cooldowns. The W1-S6 proposal adds per-staff-id keys for PIN-rotate emails. Without TTL cleanup or LRU, the HashMap grows indefinitely, risking memory exhaustion on long-running processes.
  - _Mitigation_: Extend `EmailAlerter` with a sibling HashMap `last_sent_per_staff` and a periodic TTL cleanup task (e.g., every 5 minutes). Add a Prometheus gauge `email_alerts.last_sent_map_size` for observability.

### 6. [P1] [W1-S6] SMTP transport substrate (sendmail/raw SMTP/Google Workspace API) unverified at Server .23/Bono VPS; DKIM/SPF for racingpoint.in unverified

- **Models**: 4/5 — deepseek-r1-0528, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.90
- **Inherited from V1**: yes
- **Boundaries cited**: staff_auth.rs | comms-link/shared/send-email.js | crates/racecontrol/src/email_alerts.rs

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.85]: Unverified email transport substrate
  - _Rationale_: V1 EmailAlerter's sendmail/SMTP transport unverified at Server .23/Bono VPS. DKIM/SPF unconfigured for racingpoint.in risks email delivery failure. Inherited V1 infra flaw compromises V2 PIN recovery.
  - _Mitigation_: Session 5 entry probe for SMTP/sendmail. Implement fallback to Google Workspace API if DKIM/SPF fails.
- **mimo-v2-pro** [P2 c=0.8]: SMTP transport substrate unverified at Server .23/Bono VPS
  - _Rationale_: EmailAlerter shells out to send-email.js (Strategy 1 sendmail / Strategy 2 raw SMTP). Neither verified at production hosts. DKIM/SPF for racingpoint.in unverified. Email delivery risk could cause PIN-rotate emails to never arrive.
  - _Mitigation_: Session 5 entry probe: verify sendmail at Server .23, SMTP localhost:25 at Bono VPS, dig +short TXT racingpoint.in for SPF. If DKIM/SPF absent, surface as Captain Q-DECISION before code (ship-with-risk vs delay).
- **gemini-2.5-flash** [P1 c=1]: Email Transport Substrate and DKIM/SPF Unverified
  - _Rationale_: The underlying SMTP transport (sendmail/raw SMTP) for `EmailAlerter` is unverified at Server .23 and Bono VPS. Crucially, the DKIM/SPF setup for `racingpoint.in` is also unverified. This poses a critical risk that PIN-rotate emails will land in spam/quarantine, making them undeli
  - _Mitigation_: Execute the Session 5 entry probe to verify sendmail/SMTP availability and DKIM/SPF records. If DKIM/SPF are absent or transport is not functional, surface this as a P0 Captain Q-DECISION BEFORE code ship, with options to delay W1-S6 or ship with documented risk and immediate mon
- **mistral-small-2603** [P1 c=0.93]: SMTP transport substrate (sendmail/raw SMTP/Google Workspace API) unverified at Server .23/Bono VPS; DKIM/SPF for racingpoint.in unverified
  - _Rationale_: The W1-S6 proposal shells out to `send-email.js` for helpdesk@ emails. The substrate (sendmail in PATH, raw SMTP localhost:25, or Google Workspace API) is not verified at Server .23 or Bono VPS. Additionally, DKIM/SPF for racingpoint.in is unverified, risking email delivery failu
  - _Mitigation_: Run Session 5 entry probe: verify sendmail availability at Server .23, raw SMTP at Bono VPS, and Google Workspace API auth scope. If DKIM/SPF is absent, surface as Captain Q-DECISION before code: ship-with-risk + monitor first delivery OR delay W1-S6 until DKIM/SPF up.

### 7. [P1] [W1-S6] V1 cooldown semantics (per-pod 1800s + venue-wide 300s) conflict with W1-S6 event-class semantics (always-deliver per PIN-rotate)

- **Models**: 4/5 — qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.91
- **Inherited from V1**: yes
- **Boundaries cited**: email_alerts.rs:9-30 | email_alerts.rs:69-83 | crates/racecontrol/src/email_alerts.rs

**Per-model views:**

- **qwen3-coder** [P1 c=0.9]: Email cooldown semantics conflict with event-class delivery
  - _Rationale_: V1 EmailAlerter cooldowns assume alert-class semantics (flood prevention). W1-S6 requires event-class delivery (always send PIN-reset email). The existing cooldown HashMap conflicts with this requirement and must be bypassed.
  - _Mitigation_: Extend EmailAlerter with `send_pin_rotation` method that bypasses cooldown HashMap entirely. Document event-class vs alert-class distinction in code comments.
- **mimo-v2-pro** [P1 c=0.8]: EmailAlerter cooldown semantics conflict with event-class PIN-rotate
  - _Rationale_: V1 EmailAlerter has per-pod 1800s + venue-wide 300s cooldowns (alert-class). W1-S6 PIN-rotate is event-class: must always deliver regardless of cooldown. Extending EmailAlerter with per-staff-id keys inherits wrong semantics.
  - _Mitigation_: New sibling method `send_pin_rotation` that bypasses cooldown HashMap entirely. Add test proving email sent even when cooldown would block alert-class send.
- **gemini-2.5-flash** [P1 c=1]: EmailAlerter Cooldown Semantics Conflict with PIN-Rotate Event
  - _Rationale_: The V1 `EmailAlerter` module has per-pod and venue-wide cooldowns (1800s/300s) designed for alert-class semantics (preventing alert flooding). The W1-S6 PIN-rotate email, however, is an event-class notification that must always be delivered to the staff member regardless of recen
  - _Mitigation_: Implement a new sibling method `send_pin_rotation_email` within `EmailAlerter` that explicitly bypasses the existing cooldown mechanisms, ensuring immediate delivery of PIN rotation notifications as per Captain's disposition Q-S6-1.
- **mistral-small-2603** [P1 c=0.95]: V1 cooldown semantics (per-pod 1800s + venue-wide 300s) conflict with W1-S6 event-class semantics (always-deliver per PIN-rotate)
  - _Rationale_: The V1 `EmailAlerter` cooldowns assume alert-class semantics (one alert per failure window). The W1-S6 PIN-rotate is event-class: every legitimate PIN-rotate should trigger an email regardless of cooldown. Using the existing cooldown HashMap would suppress legitimate emails.
  - _Mitigation_: Add a sibling method `EmailAlerter::send_pin_rotation` that bypasses the cooldown HashMap entirely. Document the invariant in the method comment: 'PIN-rotate emails are event-class; always deliver.'

### 8. [P1] [W3] wallet_redemptions row must include hold_id column to preserve bonus source-tag through HRC

- **Models**: 4/5 — qwen3-coder, mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.94
- **Inherited from V1**: no
- **Boundaries cited**: wallet_holds schema | wallet_redemptions schema | PACT-024 §3 Q5 + Wallet Framing C source-tagging

**Per-model views:**

- **qwen3-coder** [P1 c=0.92]: Bonus credit source-tag traceability gap
  - _Rationale_: Wallet Framing C requires forensic traceability of bonus credit sources through hold→capture paths. Without explicit hold_id linkage in wallet_redemptions, audit queries must JOIN through session_id, increasing complexity and failure points.
  - _Mitigation_: Add `hold_id` column to `wallet_redemptions` table with FK constraint. This enables single-hop forensic queries from redemption → hold → top-up for source-traceback.
- **mimo-v2-pro** [P1 c=0.9]: Bonus credit source-tag preservation through HRC unspecified
  - _Rationale_: Wallet Framing C requires source-tagging for refund fairness. W3 hold→capture path must preserve 'what kind of credits did customer redeem' signal. wallet_redemptions row currently has no hold_id linkage; forensic query for source-traceback is unspecified.
  - _Mitigation_: ADD COLUMN hold_id TEXT REFERENCES wallet_holds(id) to wallet_redemptions in W3 migration. Add test `bonus_source_tag_preserved_through_hrc` verifying top-up source flows through hold→capture→redemption.
- **gemini-2.5-flash** [P1 c=1]: Bonus Credit Source-Tag Preservation Through HRC State
  - _Rationale_: Wallet Framing C and PACT-024 §3 Q5 require bonus credit source-tag preservation for forensic auditability and refund fairness. The current `wallet_redemptions` table only records `credits_redeemed` as a homogeneous bucket. Without a clear linkage from `wallet_redemptions` throug
  - _Mitigation_: Add a `hold_id` column (NEW FK) to the `wallet_redemptions` table, linking it directly to `wallet_holds.id`. This provides a single-hop forensic query path to trace redeemed credits back to their hold and subsequently to their top-up source, preserving the bonus source-tag invari
- **mistral-small-2603** [P1 c=0.94]: wallet_redemptions row must include hold_id column to preserve bonus source-tag through HRC
  - _Rationale_: The W3 proposal must preserve the source-tag of bonus credits through the HOLD-RELEASE-CAPTURE state machine for forensic fairness (PACT-024 §3 Q5). The existing wallet_redemptions row does not include a hold_id. Without this column, the audit query to trace 'what kind of credits
  - _Mitigation_: Add `hold_id TEXT REFERENCES wallet_holds(id)` column to wallet_redemptions table. Update `WalletService::capture` to populate this column. Add unit test `bonus_source_tag_preserved_through_hrc` that asserts the column is set correctly.

### 9. [P2] [W1-S6] Lockout state durability: in-memory HashMap acceptable per CR-3 customer-service-priority; DB-backed deferred

- **Models**: 4/5 — deepseek-r1-0528, qwen3-coder, mimo-v2-pro, mistral-small-2603
- **Avg confidence**: 0.84
- **Inherited from V1**: no
- **Boundaries cited**: staff_auth.rs | staff_auth.rs state durability | staff_auth.rs PinLockoutTracker

**Per-model views:**

- **deepseek-r1-0528** [P0 c=0.95]: In-memory lockout state loss on restart
  - _Rationale_: Per-staff attempt counters stored in-memory will reset on process restart. Allows PIN brute-forcing after service reboot. V2 introduces new security gap absent in V1's stateless design.
  - _Mitigation_: Persist attempt counters via wallet_holds-like durable storage or implement startup state reconciliation.
- **qwen3-coder** [P2 c=0.8]: In-memory lockout state lost on restart
  - _Rationale_: W1-S6's in-memory HashMap for tracking PIN attempts/resets is kaizen-min but creates a forgiveness window: server restart after 4 wrong attempts resets counter, allowing 5th attempt without triggering lockout.
  - _Mitigation_: Accept restart-forgiveness as CR-3 customer-service-priority trade-off per Captain G33 Q-S6-6. Defer DB-backed state to V2.1 unless abuse patterns emerge requiring persistence.
- **mimo-v2-pro** [P2 c=0.7]: In-memory lockout state lost on restart
  - _Rationale_: Captain Q-S6-6 ACCEPT-DEFAULT: in-memory HashMap (kaizen-min). Restart-after-5-wrong acceptable per CR-3 customer-service-priority. However, this means a service restart forgives lockout state, potentially allowing brute-force continuation.
  - _Mitigation_: Document in code comment: 'In-memory state per Captain Q-S6-6; restart forgives lockout (slight staff forgiveness). DB-backed deferred to V2.1 if abuse pattern emerges.' Add monitoring: alert if restarts occur during active staff sessions.
- **mistral-small-2603** [P2 c=0.9]: Lockout state durability: in-memory HashMap acceptable per CR-3 customer-service-priority; DB-backed deferred
  - _Rationale_: The W1-S6 proposal uses an in-memory `PinLockoutTracker` with per-staff-id attempt/reset counters. On process restart, the counter resets, allowing up to 5 wrong PIN attempts again. This is acceptable per CR-3 (customer service is priority; slight forgiveness toward staff). DB-ba
  - _Mitigation_: Document the invariant in the tracker module: 'In-memory only; restart-after-5-wrong acceptable per CR-3.' Add a Prometheus counter `auth.pin_lockout_restart_count` to monitor restarts that reset counters.

### 10. [P0] [CROSS] Cross-RCA root cause: auth boundary changes must preserve audit_log schema and state-machine consistency across W1-S5, W1-S6, and W3

- **Models**: 3/5 — qwen3-coder, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.86
- **Inherited from V1**: partial
- **Boundaries cited**: auth/middleware.rs:103-110 ↔ wallet_hrc_bridge.rs | state-machine consistency | auth blast-radius + audit_log discipline + state-machine consistency

**Per-model views:**

- **qwen3-coder** [P1 c=0.8]: Idle-timeout vs HOLD state-machine consistency
  - _Rationale_: W1-S5 sliding-window idle-timeout and W3 HOLD-RELEASE-CAPTURE state machine both operate on session lifecycle but are gated on different terminal conditions. If idle-timeout auto-releases HOLDs while customer is still in-pod, it violates wallet atomicity.
  - _Mitigation_: Explicitly document and enforce invariant: HOLDs are session-bound NOT auth-bound. Release fires only on session terminal-state (game-running stop / launch-fail / staff cancel), NOT staff JWT expiration.
- **gemini-2.5-flash** [P2 c=0.8]: Emergent State Machine Consistency Across Foundational Boundaries
  - _Rationale_: W1-S5 introduces a sliding-window idle-timeout (a form of auth state machine), W1-S6 introduces a PIN-LOCKOUT auto-rotate (a staff auth state machine), and W3 introduces a full HOLD-RELEASE-CAPTURE wallet state machine. While each is internally consistent, there's no overarching 
  - _Mitigation_: Develop a `V2 State Machine Doctrine` outlining principles for state definition, transitions, error handling, idempotency, and cross-state machine communication. This doctrine should guide future state machine implementations and provide a framework for auditing existing ones for
- **mistral-small-2603** [P0 c=0.99]: Cross-RCA root cause: auth boundary changes must preserve audit_log schema and state-machine consistency across W1-S5, W1-S6, and W3
  - _Rationale_: W1-S5 (sliding-window JWT refresh), W1-S6 (PIN-lockout auto-rotate), and W3 (wallet HRC) all touch auth and audit boundaries. If W1-S5 adds a new response-mutating middleware layer, W1-S6 adds per-staff-id rate-limit state, and W3 adds wallet_holds state, the audit_log schema mus
  - _Mitigation_: Define a unified audit_log action_type vocabulary across RCAs: 'jwt_refresh', 'staff_pin_auto_reset', 'wallet_hold_created', 'wallet_hold_captured', 'wallet_hold_released'. Add a cross-RCA regression test that asserts no duplicate action_types and no schema conflicts.

### 11. [P0] [W3] PACT-024 Q1-Q5 dispositions outstanding 5 days; gates W3 implementation entry

- **Models**: 3/5 — mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.98
- **Inherited from V1**: no
- **Boundaries cited**: PACT-024 status | PACT-20260504-024-wallet-concurrency-idempotency-extension.md | PACT-20260504-024 status FILED-AWAITS-AMPLIFIER + james AMPLIFIER vote

**Per-model views:**

- **mimo-v2-pro** [P1 c=0.95]: PACT-024 AMPLIFIER outstanding blocks W3 implementation
  - _Rationale_: PACT-024 status is FILED-AWAITS-AMPLIFIER (5 days outstanding). james AMPLIFIER vote on Q1-Q5 required before W3 implementation can proceed. This is a governance gate, not a code issue, but blocks all W3 work.
  - _Mitigation_: james must disposition Q1-Q5 (or Captain rules via §S-N). Proposed defaults: Q1-a optimistic lock, Q2-c per-event-type+forensic, Q3-c hybrid 24h, Q4-c hybrid defer to PACT-024b, Q5-c hybrid atomic+separate-key.
- **gemini-2.5-flash** [P0 c=1]: PACT-024 AMPLIFIER Block on W3 Implementation
  - _Rationale_: The PACT-024 (wallet concurrency/idempotency) is currently FILED-AWAITS-AMPLIFIER, with the author's (james) vote outstanding for 5 days. This directly blocks the W3 implementation, as the PACT defines the core concurrency and idempotency contracts essential for the HOLD-RELEASE-
  - _Mitigation_: Captain G33 or james must explicitly disposition PACT-024 Q1-Q5, ideally ratifying the bono recommendations (Q1-a, Q2-c, Q3-c, Q4-c, Q5-c) to unblock W3 implementation. This is a critical gate for W3 entry.
- **mistral-small-2603** [P0 c=0.98]: PACT-024 Q1-Q5 dispositions outstanding 5 days; gates W3 implementation entry
  - _Rationale_: PACT-20260504-024 (wallet concurrency/idempotency) is FILED-AWAITS-AMPLIFIER. The bono recommendations (Q1-a optimistic / Q2-c per-event-type+forensic / Q3-c hybrid 24h / Q4-c hybrid / Q5-c hybrid atomic+separate-key) remain unratified. Until Q1-Q5 land, the W3 RCA proposal canno
  - _Mitigation_: Captain G33 or james must disposition Q1-Q5 via AMPLIFIER or explicit ratify. Until then, W3 PR-A cannot be filed. Document the gate in the W3 spec: 'BLOCKED on PACT-024 Q1-Q5 ratification.'

### 12. [P1] [CROSS] Inconsistent Audit-Log Discipline Across Foundational Changes

- **Models**: 3/5 — deepseek-r1-0528, mimo-v2-pro, gemini-2.5-flash
- **Avg confidence**: 0.85
- **Inherited from V1**: partial
- **Boundaries cited**: audit_log.action_type | audit_log schema + action_type column | audit-log discipline

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.9]: Inconsistent audit log volume discipline
  - _Rationale_: W1-S5 suppresses routine token refresh logs; W1-S6 logs every PIN reset; W3 logs all state transitions. No unified retention/volume doctrine across auth/wallet boundaries. Violates Wallet Framing C observability invariant.
  - _Mitigation_: Define audit log volume doctrine in §AMEND-3.II D12. Implement log-level gates per event criticality.
- **mimo-v2-pro** [P1 c=0.75]: Audit-log discipline inconsistency across triplet
  - _Rationale_: W1-S5: NO routine logging on refresh (volume concern). W1-S6: YES log every PIN-rotate (bounded ≤3/hr). W3: YES log every hold/release/capture (bounded). Inconsistent audit strategy could miss security events or flood audit_log.
  - _Mitigation_: Establish audit-log doctrine: log state-changing events (PIN-rotate, hold/release/capture) but not routine refreshes. Document in V2-MASTER-STATE §S-N entry. Ensure all three use `log_admin_action` with distinct action_type values.
- **gemini-2.5-flash** [P1 c=0.9]: Inconsistent Audit-Log Discipline Across Foundational Changes
  - _Rationale_: Both W1-S5 (auth refresh) and W1-S6 (PIN auto-rotate) involve audit logging, with W1-S5 explicitly deciding NOT to log routine refreshes to avoid amplification, while W1-S6 logs every PIN-rotate event. W3 also involves audit logging for wallet state transitions. This indicates an
  - _Mitigation_: Establish a `V2 Audit-Log Doctrine` outlining principles for what events to log (e.g., state changes, security-sensitive actions, critical failures), what not to log (e.g., routine 'heartbeat' events), and the required schema/payload. This doctrine should guide all future audit-l

### 13. [P1] [W3] PACT-024a §A SQL/Rust patterns must be re-targeted from racecontrol crate to v2-db crate

- **Models**: 3/5 — mimo-v2-pro, gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.97
- **Inherited from V1**: no
- **Boundaries cited**: PACT-024a §A targets | PACT-024a-wallet-concurrency-substrate-ship-plan.md | PACT-024a targets stale (racecontrol/wallet.rs vs v2-db/wallets.rs)

**Per-model views:**

- **mimo-v2-pro** [P2 c=0.95]: PACT-024a file-path targets stale after W1-S2 v2-db move
  - _Rationale_: PACT-024a §A references racecontrol/crates/racecontrol/src/wallet.rs (V1). W1-S2 moved canonical V2 wallet to crates/v2-db/src/wallets.rs. SQL/Rust patterns are valid but file paths need bumping.
  - _Mitigation_: Re-target PACT-024a §A to v2-db crate. No semantic change—only path bumps. Captain Q-W3-RECONCILE-1 disposition required.
- **gemini-2.5-flash** [P1 c=1]: Stale PACT-024a Targets After W1-S2 V2-DB Move
  - _Rationale_: PACT-024a, which outlines the SQL/Rust patterns for wallet concurrency, references V1 wallet paths (`racecontrol/crates/racecontrol/src/wallet.rs`). However, W1-S2 has already moved the canonical V2 wallet surface to `crates/v2-db/src/wallets.rs`. This discrepancy means the imple
  - _Mitigation_: Explicitly re-target all file paths in PACT-024a §A to the `v2-db` crate and `WalletService` as part of Q-W3-RECONCILE-1. Document that the SQL/Rust patterns are adopted verbatim, but the file paths are updated to reflect the canonical V2 wallet location.
- **mistral-small-2603** [P1 c=0.97]: PACT-024a §A SQL/Rust patterns must be re-targeted from racecontrol crate to v2-db crate
  - _Rationale_: PACT-024a §A references `racecontrol/crates/racecontrol/src/wallet.rs`, `billing.rs`, and `routes.rs`. The W1-S2 substrate moved the canonical V2 wallet to `crates/v2-db/src/wallets.rs::WalletService`. The PACT-024a patterns must be ported to v2-db crate with only file-path chang
  - _Mitigation_: Amend PACT-024a §A targets to `crates/v2-db/src/wallets.rs` and `crates/v2-db/src/idempotency.rs`. Update the §A SQL migration to target v2-db schema. Add a TODO comment referencing the original PACT-024a commit for audit trail.

### 14. [P2] [W3] Orphan hold cleanup deferred to sibling PACT

- **Models**: 3/5 — deepseek-r1-0528, qwen3-coder, mimo-v2-pro
- **Avg confidence**: 0.83
- **Inherited from V1**: no
- **Boundaries cited**: wallet_holds table schema | wallet_holds schema

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.9]: Orphaned hold reconciliation gap
  - _Rationale_: Deferring hold cleanup to PACT-024b leaves orphaned holds during W3 ship. No automated recovery mechanism for crashed-session holds. V2 introduces customer credit lockup risk.
  - _Mitigation_: Implement time-based hold expiration sweep in initial W3 migration or add manual admin tool.
- **qwen3-coder** [P2 c=0.78]: Orphan hold cleanup deferred to sibling PACT
  - _Rationale_: W3 introduces wallet_holds state machine but explicitly defers orphan-hold cleanup (HOLDs that lost their session via crash) to future PACT-024b 'Kidneys' reconciliation worker. This creates temporary inconsistency window.
  - _Mitigation_: Document orphan-hold deferral in W3 spec per Captain Q-W3-orphan-sweep. Ensure PACT-024b is filed as immediate follow-up with clear ownership and timeline to prevent long-term audit gaps.
- **mimo-v2-pro** [P2 c=0.8]: Orphan hold cleanup deferred to PACT-024b
  - _Rationale_: W3 does not include orphan-hold cleanup (holds that lost their session via crash). This is deferred to PACT-024b 'Kidneys' reconciliation worker. Orphan holds could accumulate, locking customer credits indefinitely until PACT-024b ships.
  - _Mitigation_: Add hold_timeout_secs config (default deferred). Document in W3 spec: orphan cleanup is PACT-024b scope. Add monitoring: alert if open holds older than 24h accumulate. Ensure release_hold can be called manually by staff for orphan cases.

## §2 — MINORITY findings (2/5 models)

Evaluated at Step 4 VERIFY by adversarial models; included in PLAN scope only if Step 4 score ≥ 4.0.

### 1. [P0] [W1-S5] V2.1 PACT pin file stale after pull-forward

- **Models**: 2/5 — qwen3-coder, mimo-v2-pro
- **Avg confidence**: 0.93
- **Inherited from V1**: no
- **Boundaries cited**: auth/middleware.rs:103-110

**Per-model views:**

- **qwen3-coder** [P0 c=0.95]: V2.1 PACT pin file stale after pull-forward
  - _Rationale_: The W1-S5 RCA pulls forward the V2.1 sliding-window idle timeout PACT into Wave 1. However, the original V2.1 pin file and middleware comments still reference V2.1 scope, creating a documentation inconsistency that must be resolved at ship time to avoid future confusion.
  - _Mitigation_: Amend `project_v2_1_sliding_window_idle_timeout_pact_pin.md` status to RETIRED-PULLED-FORWARD-TO-W1-S5 with commit-hash anchor; update middleware.rs comment block to reflect W1-S5 implementation per Captain §S-82 Q3 disposition.
- **mimo-v2-pro** [P2 c=0.9]: V2.1 PACT pin file stale after pull-forward
  - _Rationale_: W1-S5 pulls V2.1 sliding-window PACT forward into Wave 1. project_v2_1_sliding_window_idle_timeout_pact_pin.md still says ACTIVE-V2.1. Must be amended to RETIRED-PULLED-FORWARD-TO-W1-S5 at ship time, else future readers confused about scope.
  - _Mitigation_: At W1-S5 ship: amend V2.1 pin status to RETIRED-PULLED-FORWARD-TO-W1-S5 + commit-hash anchor. Amend middleware.rs:103-110 comment to remove 'scope-pinned to V2.1' language.

### 2. [P0] [W3] Concurrent hold atomicity hole under SQLite single-writer

- **Models**: 2/5 — mimo-v2-pro, gemini-2.5-flash
- **Avg confidence**: 0.95
- **Inherited from V1**: no
- **Boundaries cited**: crates/v2-db/src/wallets.rs:hold | crates/v2-db/src/wallets.rs:155-159

**Per-model views:**

- **mimo-v2-pro** [P0 c=0.9]: Concurrent hold atomicity hole under SQLite single-writer
  - _Rationale_: W1-S2 reserve_credits relies on SQLite single-writer + WHERE-guarded predicate for atomicity. W3 hold introduces explicit state machine with wallet_holds table. Concurrent holds for same wallet (e.g., lobby host pays for N pods) must serialize correctly. Optimistic locking (versi
  - _Mitigation_: Implement `with_optimistic_retry` wrapper per PACT-024a §A.2.1 with bounded backoff (3 retries, 10ms base * 2^attempt + jitter). Add integration test `hold_optimistic_atomic_under_concurrency` simulating 8 concurrent holds on same wallet.
- **gemini-2.5-flash** [P1 c=1]: W1-S2 No Idempotency Key Gap for Wallet Operations
  - _Rationale_: The W1-S2 `WalletService::reserve_credits` explicitly deferred idempotency keys to Wave 3 (PACT-024). Without idempotency, concurrent or retried requests for wallet operations (hold, capture, release) can lead to double-debits, double-credits, or inconsistent state, violating the
  - _Mitigation_: Implement the `idempotency_keys` table and associated `check_idempotency_key`/`record_idempotency_key` logic as defined in PACT-024a §A.2.2/A.2.3 as part of W3 PR-A. Ensure all `WalletService` operations (hold, release, capture) correctly utilize this idempotency mechanism.

### 3. [P1] [W1-S5] Response-mutating middleware layer precedent risk

- **Models**: 2/5 — qwen3-coder, mimo-v2-pro
- **Avg confidence**: 0.85
- **Inherited from V1**: no
- **Boundaries cited**: auth/middleware.rs:144-161

**Per-model views:**

- **qwen3-coder** [P1 c=0.85]: Response-mutating middleware sets precedent
  - _Rationale_: W1-S5 introduces the first response-mutating middleware layer in the auth stack (token re-issuance + Set-Cookie). This creates a precedent that future concerns (CSRF rotation, audit headers) might inherit by composition rather than explicit justification.
  - _Mitigation_: Add explicit anti-precedent comment at response-mutating site: 'Explicit one-off with named anti-precedent per Captain G33 Q-S5-6 disposition. Future response-mutating layers must justify independently, NOT inherit by precedent.'
- **mimo-v2-pro** [P1 c=0.85]: Response-mutating middleware layer precedent risk
  - _Rationale_: W1-S5 introduces NEW response-mutating middleware layer (Set-Cookie write). This sets precedent for other concerns (CSRF rotation, audit headers). Captain Q-S5-6 dispositioned as explicit one-off with anti-precedent comment, but the structural pattern exists in codebase.
  - _Mitigation_: Add anti-precedent comment at response-mutating middleware site: 'ONE-OFF per Captain Q-S5-6 2026-05-09; future CSRF/audit/SameSite rotation must justify own composition.' Ensure comment includes Captain disposition reference.

### 4. [P1] [W3] Undefined Refund-During-HOLD Interaction

- **Models**: 2/5 — mimo-v2-pro, gemini-2.5-flash
- **Avg confidence**: 0.85
- **Inherited from V1**: no
- **Boundaries cited**: crates/v2-db/src/wallets.rs:release_hold | crates/racecontrol/src/wallet_refund.rs

**Per-model views:**

- **mimo-v2-pro** [P1 c=0.8]: Refund-during-HOLD ordering unspecified
  - _Rationale_: W1-S3 refund 3-band routing must interact with W3 HOLD state. If customer requests refund while HOLD is active (game launching), the order of operations (release_hold then refund vs HOLD-aware refund) is unspecified. Wrong order could double-refund or lose credits.
  - _Mitigation_: Define explicit ordering: refund-during-HOLD must release_hold first, then route via W1-S3. Add integration test: refund during pending hold releases credits then applies 3-band routing.
- **gemini-2.5-flash** [P2 c=0.9]: Undefined Refund-During-HOLD Interaction
  - _Rationale_: The interaction between the W1-S3 3-band refund routing and a customer's active HOLD (e.g., during game launch) is undefined. If a customer requests a refund while a HOLD is active, the system needs a clear, consistent, and auditable process. Without this, there's a risk of eithe
  - _Mitigation_: As per Captain's disposition Q-W3-12, implement the refund-during-HOLD ordering such that if a session has an open hold, the system first calls `WalletService::release_hold` and then proceeds with the existing W1-S3 3-band refund routing. Document this invariant in the W3 spec, e

### 5. [P1] [W1-S5] New idle-refresh cookie may collide with existing staff-PIN cookie

- **Models**: 2/5 — gemini-2.5-flash, mistral-small-2603
- **Avg confidence**: 0.92
- **Inherited from V1**: yes
- **Boundaries cited**: crates/racecontrol/src/auth/admin.rs | auth/admin.rs (cookie helpers) + middleware.rs post-handler

**Per-model views:**

- **gemini-2.5-flash** [P2 c=0.9]: Cross-Pilot POS Browser Cookie Handling Verification
  - _Rationale_: The W1-S5 sliding-window introduces a `Set-Cookie` response header on every authenticated staff request. While the POS browser is expected to handle httpOnly cookies automatically, this is a change in the cross-pilot contract. The V2.1 PACT pin §5 explicitly flags this as an unre
  - _Mitigation_: Send a NOTIFY to bono via INBOX before merge, explicitly stating the `Set-Cookie` contract change and requesting verification that the POS browser handles it as a no-op write. Document this contract change in the W1-S5 spec and ensure E2E tests (Wave 1 Session 7 scope) cover this
- **mistral-small-2603** [P1 c=0.94]: New idle-refresh cookie may collide with existing staff-PIN cookie
  - _Rationale_: POS browser currently stores an httpOnly staff-PIN cookie. The W1-S5 proposal adds a Set-Cookie response header for the refreshed JWT. If the cookie name is not unique (e.g., 'staff_jwt' overwrites), the PIN cookie could be lost, breaking staff authentication. The RCA must mandat
  - _Mitigation_: Use unique cookie name 'staff_idle_refresh_jwt' OR document overwrite behavior with a comment at the Set-Cookie site. Add E2E test that verifies staff-PIN cookie survives 5 sliding-window refreshes.

## §3 — SINGLETON findings (1/5 model)

Listed for traceability + Step 4 VERIFY consideration. Not gating per MMA Protocol v3.0.

### 1. [P0] [CROSS] Auth blast-radius overlaps with wallet state machine

- **Models**: 1/5 — qwen3-coder
- **Avg confidence**: 0.85
- **Inherited from V1**: no
- **Boundaries cited**: auth/staff_auth.rs ↔ wallets.rs

**Per-model views:**

- **qwen3-coder** [P0 c=0.85]: Auth blast-radius overlaps with wallet state machine
  - _Rationale_: W1-S6 introduces per-staff rate-limiting and lockout primitives that directly interact with wallet operations (PIN resets, freeze events). If either auth or wallet boundary fails, it can cascade into the other through shared customer/staff identity surfaces.
  - _Mitigation_: Ensure strict separation of concerns: auth tracks staff attempts/locks; wallet tracks financial holds. Bridge modules must validate cross-boundary transitions atomically and log cross-cutting state changes for audit traceability.

### 2. [P0] [CROSS] Auth blast-radius: W1-S5 refresh + W1-S6 lockout interaction

- **Models**: 1/5 — mimo-v2-pro
- **Avg confidence**: 0.80
- **Inherited from V1**: no
- **Boundaries cited**: auth/middleware.rs + auth/staff_auth.rs

**Per-model views:**

- **mimo-v2-pro** [P0 c=0.8]: Auth blast-radius: W1-S5 refresh + W1-S6 lockout interaction
  - _Rationale_: W1-S5 sliding-window refresh re-issues JWT on every authenticated request. W1-S6 PIN-lockout fires on 5 wrong PIN attempts. If refresh happens between wrong attempts, does it reset the attempt counter? The interaction surface is unspecified and could cause lockout bypass or prema
  - _Mitigation_: Define explicit interaction contract: PIN-attempt counter is NOT reset by JWT refresh (counter is server-side keyed by staff_id, not JWT). Add integration test: refresh between wrong attempts does NOT reset counter; 5th wrong still triggers lockout.

### 3. [P0] [CROSS] V1↔V2 Bridge Module Class Consistency and Auditability

- **Models**: 1/5 — gemini-2.5-flash
- **Avg confidence**: 0.90
- **Inherited from V1**: no
- **Boundaries cited**: V1↔V2 bridge module class

**Per-model views:**

- **gemini-2.5-flash** [P0 c=0.9]: V1↔V2 Bridge Module Class Consistency and Auditability
  - _Rationale_: Both W1-S5 (auth/middleware.rs) and W3 (wallet_hrc_bridge.rs) involve significant V1↔V2 boundary interactions, with W3 explicitly introducing a new `v2_bridge` module. There's no explicit doctrine or pattern for how these bridge modules should be structured, tested, or audited, b
  - _Mitigation_: Establish a `V1↔V2 Bridge Module` doctrine document outlining best practices for structure, data contract enforcement, error handling, and observability (e.g., dedicated tracing spans, metrics). Require all future bridge modules to adhere to this pattern and include specific audi

### 4. [P0] [W1-S5] Sliding-window token refresh introduces UPDATE-then-SELECT-same-column anti-pattern if not guarded

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.98
- **Inherited from V1**: no
- **Boundaries cited**: auth/middleware.rs:119-134 (is_idle_expired refactor) + middleware.rs:144-161 (require_staff_jwt post-handler layer)

**Per-model views:**

- **mistral-small-2603** [P0 c=0.98]: Sliding-window token refresh introduces UPDATE-then-SELECT-same-column anti-pattern if not guarded
  - _Rationale_: The proposed `is_idle_expired` refactor returns an enum {Fresh, RefreshSoon, Expired}. The RefreshSoon path triggers a post-handler JWT re-issuance. If the implementation reads `iat` after updating the token (e.g., to compute new expiry), it risks the F-05 anti-pattern: UPDATE wa
  - _Mitigation_: Add invariant comment at `is_idle_expired` call site: 'Snapshot iat/now BEFORE any UPDATE; compute new token from snapshot; never read iat after UPDATE.' Propose a new Standing Rule sub-PACT to codify UPDATE-then-SELECT-same-column as a lint error (₹162.50 customer loss precedent

### 5. [P1] [W1-S5] Concurrent token refresh races

- **Models**: 1/5 — deepseek-r1-0528
- **Avg confidence**: 0.90
- **Inherited from V1**: no
- **Boundaries cited**: auth/middleware.rs:119-134

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.9]: Concurrent token refresh races
  - _Rationale_: Sliding-window re-issuance fires on every authenticated request. No mutex around JWT minting/cookie writing creates atomicity holes under high-concurrency POS traffic. V2 introduces new race condition absent in V1 fixed-window design.
  - _Mitigation_: Add per-claims mutex in refresh path or use atomic reference counters. Load-test with 8+ concurrent staff sessions.

### 6. [P1] [W3] V1↔V2 event bridge single-point failure

- **Models**: 1/5 — deepseek-r1-0528
- **Avg confidence**: 0.85
- **Inherited from V1**: no
- **Boundaries cited**: v2_bridge/wallet_hrc_bridge.rs

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.85]: V1↔V2 event bridge single-point failure
  - _Rationale_: Single bridge module handles all game_launcher state events. Crash drops capture/release signals causing wallet-state/customer-session desync. New V2 architectural SPOF.
  - _Mitigation_: Implement idempotent replay in bridge or distribute events via Redis pub/sub.

### 7. [P1] [CROSS] Asymmetric V1↔V2 bridge implementations

- **Models**: 1/5 — deepseek-r1-0528
- **Avg confidence**: 0.85
- **Inherited from V1**: no
- **Boundaries cited**: V1↔V2 bridge pattern

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.85]: Asymmetric V1↔V2 bridge implementations
  - _Rationale_: W1-S5 uses axum middleware chaining; W1-S6 calls V1 modules directly; W3 creates new event bridge. Inconsistent patterns increase maintenance/risk. Violates kaizen-discipline.
  - _Mitigation_: Codify bridge pattern in doctrine §S-146 precedent. Refactor W1-S5/W1-S6 to use W3-style explicit bridge modules.

### 8. [P1] [CROSS] Idempotency key scope collision

- **Models**: 1/5 — deepseek-r1-0528
- **Avg confidence**: 0.80
- **Inherited from V1**: no
- **Boundaries cited**: idempotency_keys table

**Per-model views:**

- **deepseek-r1-0528** [P1 c=0.8]: Idempotency key scope collision
  - _Rationale_: W1-S6 PIN resets and W3 wallet operations both use idempotency keys but lack cross-pilot coordination. Risk of key reuse across subsystems causing undetected replay. New V2 atomicity hole.
  - _Mitigation_: Implement global idempotency key namespace with event-type prefixes. Add cross-service validation in idempotency.rs.

### 9. [P1] [W1-S5] JWT secret rotation grace + sliding-window refresh interaction

- **Models**: 1/5 — mimo-v2-pro
- **Avg confidence**: 0.70
- **Inherited from V1**: yes
- **Boundaries cited**: auth/middleware.rs:79-96

**Per-model views:**

- **mimo-v2-pro** [P1 c=0.7]: JWT secret rotation grace + sliding-window refresh interaction
  - _Rationale_: V1 extract_staff_claims has jwt_secret_previous fallback. Sliding-window refresh always uses CURRENT secret. Tokens refreshed during rotation grace implicitly migrate to new secret. This side-effect is undocumented and could break clients expecting grace-period behavior.
  - _Mitigation_: Explicitly document choice in code comment: 'Refresh always uses CURRENT secret; mid-rotation refresh implicitly migrates to new secret per Captain Q-S5-4 ACCEPT-DEFAULT.' Add test proving refresh during grace uses current secret.

### 10. [P1] [CROSS] Cross-pilot transport contract changes undocumented

- **Models**: 1/5 — mimo-v2-pro
- **Avg confidence**: 0.70
- **Inherited from V1**: no
- **Boundaries cited**: cross-pilot transport contracts

**Per-model views:**

- **mimo-v2-pro** [P1 c=0.7]: Cross-pilot transport contract changes undocumented
  - _Rationale_: W1-S5: Set-Cookie response header now arrives on every authenticated staff request (POS browser must handle). W1-S6: WhatsApp dispatch for Captain freeze (bono substrate dependency). W3: Idempotency-Key header propagation for PWA/POS/Kiosk. These cross-pilot contracts are specifi
  - _Mitigation_: Create shared cross-pilot transport contract section in V2-MASTER-STATE documenting: (1) Set-Cookie behavior for POS browser, (2) WhatsApp Evolution API instance for freeze events, (3) Idempotency-Key header format for wallet operations. Notify bono via INBOX before each merge.

### 11. [P1] [W3] T-F1 Bonus Arbitrage Exploit Unaddressed Without W3 HRC

- **Models**: 1/5 — gemini-2.5-flash
- **Avg confidence**: 1.00
- **Inherited from V1**: no
- **Boundaries cited**: PACT-024 §1 trigger top-tier (gemini-3.1)

**Per-model views:**

- **gemini-2.5-flash** [P1 c=1]: T-F1 Bonus Arbitrage Exploit Unaddressed Without W3 HRC
  - _Rationale_: The T-F1 bonus arbitrage exploit (customer farming credits via forced launch failures with discount applied) is a critical financial vulnerability. Its closure is gated on the W3 HOLD-RELEASE-CAPTURE (HRC) state machine landing first. Without W3, the underlying mechanism to preve
  - _Mitigation_: Prioritize the W3 HRC implementation as the foundational substrate for future T-F1 exploit fixes. Ensure the W3 design explicitly incorporates the 'capture only on `game_running` heartbeat' and 'release on launch-fail returns FULL credits' logic to lay the groundwork for T-F1 clo

### 12. [P1] [W1-S5] Routine JWT refresh would 100x audit_log INSERT volume if logged

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.92
- **Inherited from V1**: no
- **Boundaries cited**: middleware.rs post-handler + audit_log

**Per-model views:**

- **mistral-small-2603** [P1 c=0.92]: Routine JWT refresh would 100x audit_log INSERT volume if logged
  - _Rationale_: Every authenticated staff request triggers a sliding-window refresh (post-handler Set-Cookie). If the system logs every refresh, busy POS terminals could generate 100-1000x more audit rows than today, impacting DB performance and storage. The RCA must explicitly forbid routine re
  - _Mitigation_: Add invariant comment at post-handler layer: 'Do NOT log JWT refresh events; only log 401 idle-expiry rejections.' Add a Prometheus counter `auth.jwt_refresh_total` for observability without DB write.

### 13. [P1] [W3] Separate wallet_holds table is preferred over inline wallets.held_credits column for audit clarity and concurrent holds

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.95
- **Inherited from V1**: no
- **Boundaries cited**: wallet_holds schema: separate table vs inline held_credits column

**Per-model views:**

- **mistral-small-2603** [P1 c=0.95]: Separate wallet_holds table is preferred over inline wallets.held_credits column for audit clarity and concurrent holds
  - _Rationale_: The W3 proposal can store held credits either as a separate `wallet_holds` table or as an inline `wallets.held_credits` column. A separate table provides clearer audit trail, supports multiple concurrent holds (e.g., lobby host paying for N pods), and aligns with PACT-024a §A.1.2
  - _Mitigation_: Implement `wallet_holds` as a separate table with schema per §1. Add FKs to wallets, sessions, customers. Add indexes for performance. Document the choice in the migration comment.

### 14. [P1] [CROSS] Cross-RCA root cause: V1↔V2 bridge modules (e.g., V1 game_launcher_state → V2 wallet_hrc_bridge) must not break cross-pilot contracts

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.96
- **Inherited from V1**: yes
- **Boundaries cited**: V1↔V2 bridge modules + cross-pilot contracts

**Per-model views:**

- **mistral-small-2603** [P1 c=0.96]: Cross-RCA root cause: V1↔V2 bridge modules (e.g., V1 game_launcher_state → V2 wallet_hrc_bridge) must not break cross-pilot contracts
  - _Rationale_: W1-S5 and W1-S6 modify auth boundaries; W3 introduces a V1↔V2 bridge for game_running heartbeat → wallet capture. If the bridge module uses V1 types or contracts that are not compatible with V2, cross-pilot interactions (e.g., POS .130 ↔ Server .23 ↔ Bono VPS) could break. The RC
  - _Mitigation_: Define a contract test suite that runs on every RCA merge: (1) V1 game_launcher_state event → V2 wallet_hrc_bridge → WalletService::capture must succeed; (2) V1 staff JWT extraction → W1-S5 post-handler refresh must not break POS .130 staff session; (3) V1 staff PIN attempt → W1-

### 15. [P1] [CROSS] Cross-RCA root cause: Wallet Framing C invariants (Single-Purpose Voucher, credits never expire customer-facing, cafe separate, sim+PS5 only) must be preserved across W1-S5/W1-S6/W3

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.95
- **Inherited from V1**: yes
- **Boundaries cited**: Wallet Framing C invariants + doctrine alignment

**Per-model views:**

- **mistral-small-2603** [P1 c=0.95]: Cross-RCA root cause: Wallet Framing C invariants (Single-Purpose Voucher, credits never expire customer-facing, cafe separate, sim+PS5 only) must be preserved across W1-S5/W1-S6/W3
  - _Rationale_: Wallet Framing C doctrine states: Single-Purpose Voucher; credits never expire customer-facing; cafe always separate; sim+PS5 only. W1-S5 and W1-S6 are staff-side auth changes; W3 is wallet state machine. Any change that risks violating these invariants (e.g., allowing cafe credi
  - _Mitigation_: Add a Framing-C regression test suite that runs on every RCA merge: (1) wallet_redemptions.redeemed_for IN ('sim','ps5'); (2) wallet_holds never returns credits to dormant pool; (3) wallet_topups never allows cafe credits; (4) all RCAs assert 'Single-Purpose Voucher' invariant in

### 16. [P1] [CROSS] Cross-RCA root cause: Captain dispositions and per-PR Captain merge auth gates must be honored across W1-S5, W1-S6, and W3 PRs

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.90
- **Inherited from V1**: yes
- **Boundaries cited**: Captain dispositions + per-PR Captain merge auth gates

**Per-model views:**

- **mistral-small-2603** [P1 c=0.9]: Cross-RCA root cause: Captain dispositions and per-PR Captain merge auth gates must be honored across W1-S5, W1-S6, and W3 PRs
  - _Rationale_: All three RCAs are foundational-class boundaries requiring per-PR Captain merge auth at PR-open. If Captain dispositions are not honored (e.g., Q-RECONCILE-1 for W1-S5, Q-S6-1..7 for W1-S6, Q-W3-RECONCILE-{1,2,3} for W3), the PRs cannot merge, blocking the entire Wave 1 launch-re
  - _Mitigation_: Add a cross-RCA CI gate that asserts all Captain dispositions for W1-S5, W1-S6, and W3 are CLOSED before allowing any RCA PR to be merged. Surface a Captain-facing dashboard with the gate status.

### 17. [P2] [CROSS] V1↔V2 bridge pattern inconsistency across triplet

- **Models**: 1/5 — mimo-v2-pro
- **Avg confidence**: 0.70
- **Inherited from V1**: no
- **Boundaries cited**: V1↔V2 bridge module class

**Per-model views:**

- **mimo-v2-pro** [P2 c=0.7]: V1↔V2 bridge pattern inconsistency across triplet
  - _Rationale_: W1-S5: modifies existing V2 substrate (middleware.rs). W1-S6: NEW V2 file calling INTO 5 V1 modules. W3: introduces V1↔V2 bridge for game_running heartbeat. Different topologies at same foundational boundary class could lead to inconsistent error handling or state propagation.
  - _Mitigation_: Establish V1↔V2 bridge doctrine: (1) bridge modules live in crates/racecontrol/src/v2_bridge/, (2) use typed error enums from v2-db, (3) log bridge transitions with tracing spans. Document in V2-MASTER-STATE.

### 18. [P2] [W3] Missing V1↔V2 Bridge for Game Heartbeat to Wallet Capture

- **Models**: 1/5 — gemini-2.5-flash
- **Avg confidence**: 0.90
- **Inherited from V1**: no
- **Boundaries cited**: crates/racecontrol/src/billing_game_status.rs

**Per-model views:**

- **gemini-2.5-flash** [P2 c=0.9]: Missing V1↔V2 Bridge for Game Heartbeat to Wallet Capture
  - _Rationale_: The V1 `billing_game_status.rs` module emits `game_running` heartbeats, which are the trigger for the W3 wallet capture state transition. Currently, there is no explicit V1↔V2 bridge module to translate these V1 events into calls to `WalletService::capture`. This gap means the co
  - _Mitigation_: Implement a new `crates/racecontrol/src/v2_bridge/wallet_hrc_bridge.rs` module. This module will subscribe to V1 game launcher state events and, upon launch success and the first `game_running` heartbeat, call `WalletService::capture` against the corresponding V2 hold, establishi

### 19. [P2] [W3] Idle-Timeout-During-HOLD Interaction Undefined

- **Models**: 1/5 — gemini-2.5-flash
- **Avg confidence**: 0.90
- **Inherited from V1**: no
- **Boundaries cited**: crates/racecontrol/src/auth/middleware.rs

**Per-model views:**

- **gemini-2.5-flash** [P2 c=0.9]: Idle-Timeout-During-HOLD Interaction Undefined
  - _Rationale_: The interaction between staff JWT idle-timeout (K5 fixed-window or W1-S5 sliding-window) and an active customer HOLD is undefined. If a staff session expires while a customer is in a pod with an active HOLD, the system must NOT auto-release the customer's hold, as the customer is
  - _Mitigation_: As per Captain's disposition Q-W3-13, document the invariant in the W3 spec that HOLDs are session-bound, NOT auth-bound. The release of a hold must only fire on session terminal-state (game-running stop, launch-fail, staff cancel), not due to staff authentication state changes.

### 20. [P2] [CROSS] Cross-RCA root cause: F-05 UPDATE-then-SELECT-same-column anti-pattern is not codified as a lint anywhere; W1-S5 and W3 are at-risk

- **Models**: 1/5 — mistral-small-2603
- **Avg confidence**: 0.92
- **Inherited from V1**: yes
- **Boundaries cited**: F-05 anti-pattern codification + lint candidate

**Per-model views:**

- **mistral-small-2603** [P2 c=0.92]: Cross-RCA root cause: F-05 UPDATE-then-SELECT-same-column anti-pattern is not codified as a lint anywhere; W1-S5 and W3 are at-risk
  - _Rationale_: The F-05 anti-pattern (UPDATE then SELECT same column) caused ₹162.50 customer loss and was PATCHED-ONLY in V1. It is not codified as a lint or test pattern anywhere in the codebase. W1-S5 and W3 proposals risk reintroducing this pattern if not explicitly guarded.
  - _Mitigation_: Propose a new Standing Rule sub-PACT to codify UPDATE-then-SELECT-same-column as a lint error (clippy::update_then_select_same_column). Add a regression test in W1-S5 and W3 that asserts the pattern does not exist. Document the precedent in CLAUDE.md.

---

## §4 — Counts by RCA

| RCA | CONSENSUS | MINORITY | SINGLETON |
|---|---|---|---|
| W1-S5 | 2 | 3 | 4 |
| W1-S6 | 5 | 0 | 0 |
| W3 | 5 | 2 | 5 |
| CROSS | 2 | 0 | 11 |

---

## §5 — Step 1 → Step 2 hand-off

Per MMA Protocol v3.0 Step 1 DIAGNOSE → Step 2 PLAN transition:

1. Step 2 PLAN consumes §1 CONSENSUS findings as mandatory design inputs
2. Step 2 must explicitly disposition each CONSENSUS finding (in-PLAN-scope OR explicit-defer-with-rationale)
3. §2 MINORITY findings are evaluated by Step 4 VERIFY (different models from Steps 1-3)
4. Step 2 PLAN remains gated on Captain G33 disposition of any NEW Q-DECISION surfaced by §1 (not present in original RCAs)

**PACT-024 §2.1 OPTION-A composite-#4 path-c gate state post-MMA-Step-1**: AMPLIFIER ✓ + CGP H3 ✓ + §S-N ✓ (§S-151) + **MMA Step 1 ✓** (this artifact) + RATIFY trigger PENDING-on-MMA-Steps-2+3+4. Step 2 PLAN authoring is the next gate-action.

**Cost-anchor**: $0.1065 for triplet batch vs §S-150 PR #66 single-RCA $0.083 = batch-efficiency ~38% (3 RCAs at 1.28× single-RCA cost).

**Sibling-anchor-class**: this MMA Step 1 + §S-150 PR #66 = §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application IN-PROGRESS (PR #66 was first, MERGED; W1-S5/W1-S6/W3 enter post-Step-1).

---

*Generated by .tmp/mma-step1-consensus.js from .tmp/mma-step1-results.json. Per-model raw JSON preserved in results file.*
