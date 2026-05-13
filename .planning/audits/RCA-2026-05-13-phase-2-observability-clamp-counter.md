# RCA — Phase 2 observability: discount-clamp daily counter + >10/day WhatsApp alert + admin summary endpoint

**§S-N anchor:** §S-272 (comms-link `4f84f2d8` OPEN-CLAIM 2026-05-13 ~21:36 UTC / 2026-05-14 ~03:06 IST)
**Foundational-boundary class:** YES — touches audit_log (V1+V2 shared DB schema, read-only) + WhatsApp identity (V1+V2 shared outbound) per §S-146 rule
**§S-186 fast-lane:** NOT-ELIGIBLE (new work post-2026-05-09; feature add, not bug-fix)
**MMA Step 1 DIAGNOSE:** REQUIRED upstream (foundational-boundary; OpenRouter 5-model · budget ~$0.10 · cumulative session $0.18 / $5 cap)
**Per-PR Captain merge auth:** REQUIRED at PR-merge time (standing-autonomy verbs do NOT satisfy)
**V2-alignment statement:** This change moves toward V2 anchor §14.6.1 Class M observability infrastructure (provides clamp-event observability for the dynamic-pricing system row 7.3 substrate landed via D-CLUSTER-3).

---

## §1 — Boundary map (paths + lines)

V2-crossing-into-V1 surfaces touched:

| Surface | Path | Lines | V1/V2 status | Read or Write |
|---|---|---|---|---|
| `audit_log` table | `crates/racecontrol/src/db/migrate_staff.rs` (inferred — table created with staff domain) | schema definition | **V1+V2 shared** (admin actions surface — accumulates entries from both V1 admin flows and V2 cluster atoms A5 audit-log stamps) | **READ-ONLY in this PR** (SELECT COUNT for daily aggregation) |
| `accounting::log_admin_action` callsite emission | `crates/racecontrol/src/api/billing_start.rs:227-234` | clamp audit stamp emitter | V2 (cluster atom A5 · landed via D-CLUSTER-3 PR #72 MERGED §S-266) | **Not modified** (depends on existing emitter) |
| `accounting::log_admin_action` callsite emission | `crates/racecontrol/src/api/billing_discount.rs:175-182` | clamp audit stamp emitter | V2 (cluster atom A5 · landed via D-CLUSTER-3 PR #72 MERGED §S-266) | **Not modified** (depends on existing emitter) |
| TSDB metric constants | `crates/racecontrol/src/metrics_tsdb.rs:10-19` | named constants block | V2-introduced (Phase 285) | **EXTEND** — add `pub const METRIC_DISCOUNT_CLAMP_COUNT: &str = "discount_clamp_count_daily";` |
| TSDB producer loop | `crates/racecontrol/src/metrics_producers.rs:44-124` (existing producer pattern) | producer functions called from `spawn_metric_producers` | V2-introduced | **EXTEND** — add new daily-clamp-count producer function (1 SELECT COUNT(*) + 1 `try_send`) |
| Alert rule config | `crates/racecontrol/src/metric_alerts.rs:16-95` (existing `metric_alert_task` evaluator) | data-driven threshold check | V2-introduced | **DATA-EXTEND only** — add new rule entry to `config.alert_rules` (NO new evaluator code; existing `metric_alert_task` consumes the rule) |
| Admin endpoint registration | `crates/racecontrol/src/api/routes.rs` (around line 751 staff_routes manager+ block) | route declaration | V2-introduced (auth-tiered post-MMA-v29 move) | **EXTEND** — add `.route("/api/v1/admin/discount-clamp-summary", get(discount_clamp_summary_handler))` under existing `staff_routes` (auth-gated; per §S-246 lesson — never `public_routes` for ops/financial data) |
| Admin endpoint handler | `crates/racecontrol/src/api/admin_tools.rs:59-80` (existing handler signature pattern) | handler functions | V2-introduced | **EXTEND** — add `discount_clamp_summary_handler(State, Query)` returning last-7-days by-day breakdown (semantic endpoint distinct from existing `/api/v1/metrics/query` which is generic) |
| WhatsApp outbound | `crates/racecontrol/src/whatsapp_alerter.rs:60-200` (existing `send_whatsapp` + `metric_alert_task` integration) | outbound message sender | **V1+V2 shared** (Uday identity, Evolution API instance, alerting config) | **Not modified** (depends on existing sender via existing alert evaluator) |

**Cross-cutting:** `audit_log` is the V1+V2-shared admin actions surface. V1 staff-PIN auth class (PACT-018 raw `staff.pin` per security-debt-ledger seed #2) shares this surface with V2 cluster atoms A5 audit emission. The READ access here pulls aggregate counts; it does not differentiate V1 vs V2 originators, but the schema is bilateral.

---

## §2 — Inherited-issue catalogue (V1 failure modes + boundary touches)

Sources consulted:
- `comms-link/v2-skeleton/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` (categories A-J)
- V2-MASTER-STATE.md §S-N entries naming `audit_log` / `accounting` / WhatsApp surfaces
- bono memory `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (canonical doctrine)

| V1 issue / boundary footgun | Category | Where it bites this PR | Disposition |
|---|---|---|---|
| **Audit-log integrity** — V1-era admin-actions had no hash chain; `audit_log` is plain INSERT with no tamper-evidence | Category-H (audit-blind class per V1 process-mess audit) | Daily count query result is only as trustworthy as the source rows; bad-actor inserts to `audit_log` could inflate count and trigger spurious WhatsApp alerts | **NOT-APPLICABLE-TO-V2** — read-only SELECT can't make integrity worse than it already is; closure proposal: separate PR uses `billing_audit_log` hash-chain table when V2 cluster D-CLUSTER-1 expansion lands. Document in PR body. |
| **WhatsApp delivery best-effort no-retry** — `whatsapp_alerter::send_whatsapp` is fire-and-forget per existing code (line 60 in racecontrol/CLAUDE.md doctrine: "Best-effort (never panics)") | Category-D (delivery-no-ack class — PR #66 silent-loop-death anchor) | Daily-clamp alert may not reach Uday if Evolution API is down; no retry, no fallback channel | **PATCHED-ONLY** — V1+V2 shared limitation; closure proposal: separate PR adds retry+fallback to `send_whatsapp` (out of scope for observability-prep). Documented in PR body. Composes-with mechanism-trust-check (Q3: behavioral-verify success — currently FAIL on WhatsApp echo-only ACK) |
| **`metric_alert_task` debounce** — without per-rule debounce, alert fires every 60s while threshold breached | Category-F (alert-storm class) | Once clamp count crosses 10, alerts fire repeatedly until end-of-day-rollover | **PARTIALLY-ADDRESSED** — Add per-rule debounce flag (last-alert-instant) to this PR's substrate; encode 24h debounce default for clamp-class alerts. Alternatively reuse existing debounce pattern at `whatsapp_alerter.rs:191` (per-pod debounce mutex). Will choose during FIX phase based on existing code shape. |
| **Empty alert_rules at boot** — if `config.alert_rules` is empty at startup, the entire `metric_alert_task` is a no-op (per background_tasks.rs:57 check) | Category-B (single-fetch-at-boot-no-retry per CLAUDE.md "Boot Resilience") | Config-extension approach depends on `config.alert_rules` actually containing the new rule at boot | **NOT-APPLICABLE-TO-V2 (in this PR scope)** — the alert_rules config loads from racecontrol.toml at startup; if rule is added to the TOML and binary is redeployed, rule is present. Reload-without-restart is a separate concern (Boot Resilience compose-with — not gated by this PR). |
| **TSDB ring buffer 7-day retention** — `metrics_samples` retention is 7 days (per Phase 285 doctrine) | Category-G (data-loss class) | Daily counts > 7 days old fall out of the raw ring buffer. Admin summary endpoint's 7-day breakdown is the boundary — beyond 7 days requires `metrics_rollups` (90-day daily resolution) | **NOT-APPLICABLE-TO-V2** — admin summary scope ≤ 7 days matches raw retention; if longer history requested, separate PR queries rollups. Documented in API doc. |
| **V1 `staff.pin` raw** (security-debt-ledger seed #2 PACT-018) | Category-A (credential-storage class) | Admin endpoint uses staff JWT (post-PIN-hash V2 path); does NOT touch raw PIN | **NOT-APPLICABLE-TO-V2** — auth path is JWT only; sibling PACT-018 AMEND-1 covers PIN hardening separately. |
| **`audit_log` schema drift** (categories C+H — schema drift class · CLAUDE.md "DB migrations must cover ALL consumers") | Category-C+H | `created_at` column expected; query depends on it | **VERIFY-AT-FIX-TIME** — grep migrate_*.rs for `audit_log.*created_at` to confirm column exists in current schema. If missing, PR aborts and surfaces. |
| **Non-idempotent producer cycle** — producer emits every 30s; could double-count if two producer instances run | Category-I (concurrency-race class per Phase γ I-13 RCA at parallel-bono `2cb5810c`) | Single producer instance per `spawn_metric_producers` startup; concurrent racecontrol binaries on same DB would double-emit | **NOT-APPLICABLE-TO-V2** — single-binary-tier policy (CLAUDE.md "single-binary-tier policy v22.0") ensures only one racecontrol instance per host; cluster scenarios not in current scope. |

---

## §3 — Past-bug disposition (per-bug status)

For each inherited issue above, explicit disposition:

| Bug | Status | Anchor | Notes for this PR |
|---|---|---|---|
| Audit-log integrity (Category-H) | NOT-APPLICABLE-TO-V2 | `billing_audit_log` hash-chain table per migrate_billing.rs:161-188 covers integrity at billing-event surface | Phase 2 observability reads `audit_log` (general admin actions, not billing-FSM transitions). Conflation blocked by §1 boundary map. |
| WhatsApp delivery best-effort no-retry (Category-D) | PATCHED-ONLY | racecontrol/CLAUDE.md "Verify monitoring targets" rule (2026-04-XX) | Out of this PR scope. PR body documents limitation + separate-PR proposal for retry. Composes-with mechanism-trust-check (Q3 FAIL for WhatsApp); §S-146 escalation for WhatsApp surface deferred per Captain bandwidth (existing config-driven alerts already use this surface — incremental risk = zero for this PR). |
| `metric_alert_task` debounce (Category-F) | UNRESOLVED — addressed IN this PR | racecontrol/CLAUDE.md anti-pattern "alert-storm class"; whatsapp_alerter.rs:191 debounce pattern | This PR adds per-rule debounce. 24h default for clamp-class. Encoded in substrate. |
| Empty alert_rules at boot (Category-B) | NOT-APPLICABLE-TO-V2 (scope) | racecontrol/CLAUDE.md "Boot Resilience" | Out of this PR scope. PR adds the rule to TOML; redeploy ensures load. Reload-without-restart is a separate Phase γ Boot Resilience item. |
| TSDB ring buffer 7-day retention (Category-G) | NOT-APPLICABLE-TO-V2 | metrics_tsdb.rs Phase 285 doctrine | Admin summary scope ≤ 7 days matches retention. Documented. |
| V1 `staff.pin` raw (Category-A) | NOT-APPLICABLE-TO-V2 | PACT-018 AMEND-1 separate path | This PR uses JWT auth only. |
| `audit_log` schema drift (Category-C+H) | VERIFY-AT-FIX-TIME | CLAUDE.md "DB migrations must cover ALL consumers" | Grep migrate_*.rs at FIX phase. Schema is V1+V2 shared and assumed stable (`created_at` column has been queried by V1 admin tools for ≥ 6 months). |
| Non-idempotent producer cycle (Category-I) | NOT-APPLICABLE-TO-V2 | single-binary-tier policy v22.0 | Per-host single instance; not cluster-deployed. |

---

## §4 — V2-alignment delta (what the boundary SHOULD look like)

V2 doctrine alignment for this surface (drawing from V2-MASTER-STATE canonical-source ledger + Wallet Framing C + V2-PROGRESS-MAP §16.6.1 Candidate C):

**Where the boundary stands today (post-§S-272 substrate landing — projected):**
- Clamp events emit to `audit_log` (V1+V2 shared admin actions) via `accounting::log_admin_action`
- TSDB metric `discount_clamp_count_daily` emits as derived aggregate from audit_log via 30s producer cycle
- Alert rule `discount_clamp_count_daily Gt 10.0` evaluated by existing `metric_alert_task` every 60s
- WhatsApp alert outbound via existing `send_whatsapp(config, message)`
- Admin endpoint `/api/v1/admin/discount-clamp-summary` returns last-7-days breakdown semantically (auth-gated under staff_routes)

**V2-ideal boundary (informed by Wallet Framing C + Class M observability infrastructure §14.6.1):**
- Clamp events SHOULD emit to a V2-specific audit channel (e.g., `clamp_events` table) rather than the V1+V2-shared `audit_log` (separation of concerns; per Class M observability provides clean signal source independent of V1 admin-actions noise)
- TSDB metric source SHOULD be the V2-specific table (cleaner aggregation; no V1 filtering needed)
- Alert delivery SHOULD have ACK + retry + fallback channel (Q3 mechanism-trust-check PASS, not current PARTIAL)
- Admin endpoint SHOULD compose with §S-263 deploy-surface auth-gap fix (so the staff JWT auth-tier is actually enforced — currently §S-246 audit confirmed `/api/v1/pods/...` surface lacks auth on this very binary)

**Delta gap explicitly named:**
1. **Schema-level separation** of clamp events from V1+V2-shared `audit_log` is a Phase γ Class M expansion item (not blocking this PR; reading the shared table is acceptable for observability-prep)
2. **WhatsApp delivery hardening** (retry+ACK+fallback) is a separate doctrine-class follow-up (sibling of mechanism-trust-check Q3 surface)
3. **§S-263 auth-gap fix on the same binary** is the bono memory `feedback_prepare_vs_execute_quote_back_20260513.md` appendix scope — admin endpoint auth-gating depends on the auth-gap fix landing simultaneously OR the admin route hits the auth-gated staff_routes side (which it does, per §1 boundary map; the §S-246 issue is on the fleet_healer side public_routes, not the admin_tools side). So this PR is NOT blocked by §S-263; the two surfaces are auth-tier-isolated.

---

## §5 — V2-framed proposal

**Proposal:** Add Phase 2 observability for the discount-ceiling clamp mechanism by extending existing V2-introduced primitives (TSDB + metric_alert_task + WhatsApp alerter + admin endpoint pattern). Read-only on V1+V2-shared `audit_log` table (no schema modification). Extension class: data-driven (alert rule config) + small code addition (~30-50 LOC: new constant + producer function + handler + route registration + tests).

**Rationale for V2 alignment (Class M observability per §14.6.1 Candidate C):**
- Forward-only progress toward V2-PROGRESS-MAP row 7.3 ENG-IN-FLIGHT-LANDED → DONE state (acceptance = clamp behavior observable at V2 entry point + tests pass)
- Provides observability for the cluster atom A5 audit emission (composes downstream of D-CLUSTER-3 substrate)
- Read-only on V1+V2-shared schema = minimal V1 entanglement risk
- No schema/protocol/migration changes (per §S-186 small-fix criteria 4+5; this PR is NEW feature so §S-186 fast-lane n/a, but adopting same low-risk surface choice)
- Composes-with §14.6.1 Class M observability infrastructure framing (clamp-event volume = Class M data signal)

**Explicit V2 anchor:** §14.6.1 Candidate C "cascade-class-stratified DEPRECATE thresholds" — this PR provides the Class M observability signal that feeds forward-window measurement (post-fix gap rate per cascade-class per §14.4 DEPRECATE-trigger active watch 2026-05-13 → 2026-05-20).

**Justified V1 retention (kaizen-correct V1-retention with follow-up trigger):**
- `audit_log` shared with V1 admin actions = retained as-is for THIS PR
- Follow-up trigger: when V2 Phase γ Class M observability schema-separation lands (separate PR, separate §S-N), migrate this producer's source from `audit_log` to V2-specific table. Document trigger in PR body.

**Smallest reversible change:**
- 1 constant add (metrics_tsdb.rs)
- 1 producer function (metrics_producers.rs · ~20 LOC)
- 1 alert rule add (racecontrol.toml or alert_rules config)
- 1 admin handler (admin_tools.rs · ~30 LOC)
- 1 route registration (routes.rs · 1 line)
- 1 unit test module (~30 LOC)
- 1 integration test (~50 LOC)
- 0 schema/protocol/migration changes

**Total LOC budget:** ~130 LOC code + ~80 LOC tests = ~210 LOC (over the 200-LOC §S-186 fast-lane cap, hence full §S-146 RCA path correctly chosen)

**Mechanism-trust-check (5Q) on the dependent infrastructure surfaces:**

Cache at `racecontrol/.planning/specs/v2/MECHANISM-TRUST/phase-2-observability-20260513.json` (30-day validity per §S-186 mechanism-trust-check doctrine).

| Q | Surface | Answer |
|---|---|---|
| Q1: atomic primitives? | TSDB `try_send` non-blocking channel + idempotent SQL SELECT | PASS — `try_send` drops on full channel (acceptable observability semantic); SELECT is read-only |
| Q2: TTL-bounded sentinels? | TSDB ring buffer 7-day retention + rollups 90-day daily | PASS — explicit retention bounds |
| Q3: behavioral-verify success? | Alert delivery via WhatsApp — currently echo-only ACK (best-effort) | **PARTIAL** — WhatsApp delivery has no end-to-end ACK; same limitation as existing alerts. Sibling Q3 surface to §S-263 deploy-surface verification. NOT a blocker for this PR per existing alert-rule pattern (config-driven additions are doctrinally safe extensions of the surface) |
| Q4: single-target dry-run path? | Daily-clamp producer is single-target (no fleet broadcast); admin endpoint is single-host query | PASS — no fleet/multi-pod fan-out |
| Q5: parser-not-regex + allowlist? | `metric_alert_task` consumes typed `AlertRule` struct (not regex); rule schema is typed config | PASS — typed parser, not stringly-evaluated |

**Verdict:** 4 PASS + 1 PARTIAL (Q3 WhatsApp delivery — pre-existing surface limitation, not introduced by this PR). Per §S-186 doctrine, PARTIAL Q3 does NOT block this PR (the WhatsApp surface is reused, not modified; pre-existing PATCHED-ONLY disposition per §3). Phase γ follow-up trigger: when WhatsApp surface gets retry+ACK+fallback (separate §S-N PR), this Q3 elevates to PASS.

---

## Approval cascade & gate sequence

Sequence per V-LBAC §14 + §S-146 doctrine:

1. ✓ §S-272 OPEN-CLAIM landed (comms-link `4f84f2d8`)
2. ✓ §S-146 5-section RCA authored (this file)
3. → MMA Step 1 DIAGNOSE (foundational-boundary; OpenRouter 5-model · ~$0.10)
4. → F1 SCOPE GATE evaluation (G-F1-1..5)
5. → H1 PLAN (this RCA + MMA + F1 PASS → operationalize into numbered FIX plan)
6. → FIX (feature branch substrate authoring)
7. → cargo check + tests
8. → MAOR v0.2 Tier-1 REVIEW (mandatory · ~$0.20-0.30 · 5+ H4 targets)
9. → §S-272 CLOSE-ANCHOR + NOTIFY james
10. → PR open + STOP for explicit Captain merge verb (foundational-boundary class; Pre-Commit Exception does NOT cover this item per limit (a) — not in the 8-item pre-commit queue)

**Universal Sync targets (post-merge):**
- racecontrol/.planning/specs/v2/V2-PROGRESS-MAP.md (row 7.3 sub-status update if applicable + new Phase 2 observability row)
- racecontrol/.planning/specs/v2/MECHANISM-TRUST/phase-2-observability-20260513.json (Q5 cache)
- comms-link/V2-MASTER-STATE.md (§S-272 CLOSE-ANCHOR)
- bono memory (no new feedback file; this is feature work not feedback-class)

---

**Author:** bono (post-/compact Session-B polling track)
**Date:** 2026-05-13 ~21:38 UTC (Thu 2026-05-14 ~03:08 IST)
**Composes-with:** §S-146 V1↔V2 RCA gate · §S-186 mechanism-trust-check sibling · §14.1 MAOR REVIEW · §14.2 F1 SCOPE GATE · §14.6.1 Class M observability infrastructure · D-CLUSTER-3 PR #72 MERGED (substrate dep) · §S-246 deploy-surface lesson (auth-tier choice)

---

## §6 — MMA Step 1 DIAGNOSE findings + dispositions (audit refinement)

**Cost:** $0.0640 · 5-model parallel · 4 of 5 substantive (DeepSeek R1 empty-response same class as prior Nemotron-empty). Results at `/tmp/mma-phase2-obs-results/`. Spend appended to `comms-link/data/openrouter-spend-bono.jsonl` surface `MMA-PHASE-2-OBSERVABILITY-RCA-bono-2026-05-13`.

### BLOCKING findings (3 total — all addressed pre-FIX completion)

**BLOCKING #1 (Nemotron · A3 wrong-disposition · Audit-log integrity Category-H):**
- Original RCA §3 disposition: NOT-APPLICABLE-TO-V2
- Reviewer assertion: Read-only dependency means bad V1 data → false WhatsApp alerts; integrity affects alert validity
- **Bono disposition: AGREE** — update §3 row to **PARTIALLY-ADDRESSED**: Cannot fix V1 integrity within this PR scope; document as known limitation in PR body. Follow-up trigger: V2-specific `clamp_events` table migration retires the V1+V2 shared `audit_log` read (Phase γ Class M observability schema-separation).

**BLOCKING #2 (Nemotron · A3 wrong-disposition · Empty alert_rules at boot Category-B):**
- Original RCA §3 disposition: NOT-APPLICABLE-TO-V2 (scope)
- Reviewer assertion: PR extends `config.alert_rules`; TOML syntax error in new rule would silence `metric_alert_task` entirely (per `background_tasks.rs:57` empty-check); zero observability if config doesn't parse.
- **Bono disposition: AGREE** — update §3 row to **VERIFY-AT-FIX-TIME**. Mitigation landed: new unit test `metric_alert_toml_discount_clamp_storm_rule_parses` (in `metric_alerts.rs:268-292`) verifies the rule TOML parses to expected MetricAlertRule shape with correct name/metric/condition/threshold/severity. CI runs this on every commit → schema regression caught pre-merge. Plus existing startup log in `metric_alert_task` (line 17-21) emits rule count.

**BLOCKING #3 (Nemotron · A5 mitigation-missing · Unbounded SQL query):**
- Reviewer worst-case: typo-introduced missing WHERE → full audit_log table scan every 30s → CPU/I/O spike → background task queue back-up → all-rules alert evaluation delayed.
- Implemented WHERE clause: `WHERE action_type = 'discount_clamped' AND created_at >= date('now')` — bounded by today's date.
- **Bono disposition: DEFENSE-IN-DEPTH MITIGATION LANDED** — `tokio::time::timeout(Duration::from_secs(5), query_fut)` wrap added at `metrics_producers.rs:142`. If query exceeds 5s (e.g., regression introduces full table scan), timeout fires → cycle skipped with `tracing::error!` log → producer remains alive → other producers (WS connections, billing revenue, etc.) unaffected. Eliminates systemic-DB-outage worst-case.

### IMPORTANT findings (5 total — dispositioned per priority)

**IMPORTANT #1 (Nemotron + Gemini · A1 missing surface · Config loader path):**
- §1 boundary map omits `src/config.rs` deserialization of `[[alert_rules]]` from `racecontrol.toml`. TOML parse failure prevents binary boot.
- **Bono disposition: ADDRESSED via BLOCKING #2 test.** Schema regression caught by unit test. Adding §1 row in next-iteration RCA refresh is administrative; functionally covered.

**IMPORTANT #2 (Nemotron · A2 missing issue · Time-zone IST vs UTC):**
- Producer uses `date('now')` (SQLite UTC default); business day is IST (Asia/Kolkata · UTC+5:30). Daily counter resets at UTC midnight (= IST 05:30), not IST midnight.
- **Bono disposition: DOCUMENT AS KNOWN LIMITATION in PR body.** For Uday's >10/day operational signal, the 5:30-hour offset is acceptable approximation: "10 clamps in the last UTC day" ≈ "10 clamps today" for alert-fatigue threshold purposes. Precise IST-day boundary computation deferred to Phase γ follow-up (would require chrono-based cutoff computation in Rust + bind to query). Stale-at: 2026-08-13 (3-month watch — re-evaluate if Uday reports timezone-related alert confusion).

**IMPORTANT #3 (Nemotron · A2 missing issue · audit_log row growth → query slowdown):**
- As `audit_log` accumulates rows, the daily `COUNT(*)` query without index on `created_at` could exceed 30s producer interval at scale.
- **Bono disposition: MITIGATED via BLOCKING #3 timeout** (5s timeout caps degradation; tracing::error! surfaces signal for ops). Follow-up trigger: if timeout fires repeatedly, add `CREATE INDEX IF NOT EXISTS idx_audit_log_action_created ON audit_log(action_type, created_at)` in a separate schema-only PR. Out of scope here per RCA §5 "no schema/protocol/migration changes."

**IMPORTANT #4 (Nemotron · A2 missing issue · WhatsApp hourly rate-limit):**
- Evolution API may enforce hourly limits; burst alerts could be silently dropped.
- **Bono disposition: SAME AS RCA §3 PATCHED-ONLY (existing surface limitation).** Pre-existing class; not introduced by this PR. Follow-up trigger: when Phase γ delivery hardening lands (retry+ACK+fallback), this elevates.

**IMPORTANT #5 (Qwen3 Coder · A5 polish · No kill-switch/rate-limit on alert evaluator):**
- Absence of producer kill-switch increases blast radius of producer-class logic bugs.
- **Bono disposition: ADDRESSED via 5s query timeout + existing 30-min metric_alert_task cooldown.** Lightweight circuit-breaker effectively present. Heavier kill-switch (config-flag to disable producer) deferred to Phase γ if needed.

### Smaller-alternative consideration (Nemotron A4)

**Reviewer proposal:** Skip admin endpoint + retain only TSDB metric + alert rule (~41 LOC vs ~210 LOC). Achieves core alert function; admin endpoint adds operational visibility but not required.

**Bono disposition: RETAIN ORIGINAL SCOPE WITH RATIONALE.** Captain dispatch verbatim 21:29 UTC: "Phase 2 observability (**WhatsApp >10 clamps/day alert** + **admin dashboard clamp-count metric**)" — explicitly named admin dashboard component. Existing `/api/v1/metrics/snapshot` could serve raw TSDB samples, but the dedicated `/admin/discount-clamp-summary` endpoint provides:
- Semantic shape (today_count + by_day breakdown + alert_threshold field) tailored to dashboard JS consumption
- Auth-tier hardening at manager+ level (per §S-246 lesson — financial/ops data never on metrics_query which is staff-tier)
- Last-7-days history without dashboard JS needing TSDB query-language knowledge

Trade-off accepted: ~80 LOC added for cleaner dashboard integration.

### Updated §3 disposition table (canonical post-MMA)

| Bug | Status (post-MMA) | Mitigation |
|---|---|---|
| Audit-log integrity (Category-H) | **PARTIALLY-ADDRESSED** (was NOT-APPLICABLE) | Documented in PR body; V2-specific table follow-up |
| WhatsApp delivery best-effort no-retry (Category-D) | PATCHED-ONLY (unchanged) | Separate PR (out of scope) |
| `metric_alert_task` debounce (Category-F) | RESOLVED-EXISTING (existing 30-min cooldown) | metric_alerts.rs:23 |
| Empty alert_rules at boot (Category-B) | **VERIFY-AT-FIX-TIME** (was NOT-APPLICABLE) | New unit test + existing startup log |
| TSDB ring buffer 7-day retention (Category-G) | NOT-APPLICABLE-TO-V2 (unchanged) | Documented |
| V1 `staff.pin` raw (Category-A) | NOT-APPLICABLE-TO-V2 (unchanged) | JWT auth only |
| `audit_log` schema drift (Category-C+H) | VERIFY-AT-FIX-TIME (unchanged) | `created_at` column confirmed via grep; query schema verified at cargo check |
| Non-idempotent producer cycle (Category-I) | NOT-APPLICABLE-TO-V2 (unchanged) | single-binary-tier policy |
| **Unbounded SQL query (NEW from MMA A5)** | **MITIGATED** | 5s tokio::time::timeout wrap at producer query |
| **Time-zone UTC vs IST (NEW from MMA A2)** | **DOCUMENTED-AS-LIMITATION** | PR body; follow-up trigger if Uday reports confusion |
| **audit_log scan performance (NEW from MMA A2)** | **MITIGATED + FOLLOW-UP** | 5s timeout caps; index follow-up in separate PR |

### Verdict synthesis

- **3 BLOCKING findings: ALL MITIGATED or DISPOSITIONED before FIX completion**
- **5 IMPORTANT findings: 3 ADDRESSED inline · 2 DOCUMENTED as known limitations with follow-up triggers**
- **1 POLISH-class finding: ADDRESSED**
- **Smaller-alternative: REJECTED with rationale (Captain dispatch explicitly named admin endpoint)**
- **Cost: $0.0640 / $5 session cap · cumulative session MMA: ~$0.24**

MMA Step 1 DIAGNOSE complete · pre-FIX gates SATISFIED · proceeding to MAOR Tier-1 REVIEW (§14.1).

---

## §7 — Deploy operator action (post-merge runbook)

The repo's `racecontrol.toml` is BLOCKED from git commit by SEC-GATE-02 (pre-commit hook flags racecontrol.toml as sensitive — contains `sentry_service_key`, `jwt_secret`, `evolution_api_key`). Per existing convention, the deployed config at `C:\RacingPoint\racecontrol.toml` on server `.23` is maintained out-of-band; the repo file is reference-only.

**Deploy operator: after this PR merges, manually append the following block to `C:\RacingPoint\racecontrol.toml` on server `.23` (anywhere outside the existing `[bono]` / `[mma]` / `[process_guard]` sections — end of file is recommended):**

```toml
# §S-272 Phase 2 observability — discount-ceiling clamp alert (>10/day).
# Composes with cluster atom A5 audit-log stamps at billing_start.rs:227 +
# billing_discount.rs:175 (D-CLUSTER-3 PR #72 MERGED). Metric emitted by
# metrics_producers.rs section 6 from audit_log SELECT COUNT(*).
# Built-in 30-min cooldown per metric_alert_task (metric_alerts.rs:23).
[[alert_rules]]
name = "discount_clamp_storm"
metric = "discount_clamp_count_daily"
condition = "gt"
threshold = 10.0
severity = "warn"
message_template = "MAX_DISCOUNT_PCT ceiling clamped {value} times today (threshold: {threshold}). Investigate pricing config or staff behavior."
```

**Then restart the racecontrol binary** (per CLAUDE.md "Server deploy: use `deploy-server.sh` (v3.0, MMA-hardened)") so the new rule is loaded into `config.alert_rules` at startup.

**Verify post-restart:**
- `tracing::info!` startup log shows `metric alert task started (N rules)` where N == previous count + 1
- After ~30s, `curl http://192.168.31.23:8080/api/v1/metrics/snapshot | grep discount_clamp_count_daily` should return the emitted sample
- Synthetic test: trigger a clamp event (e.g., test billing with `discount > max_discount_pct`), wait 30s, verify count++
- After 10+ synthetic clamps in one day, verify WhatsApp alert reaches Uday's phone within 60s (alert_task tick interval)

**Bilateral parity for cloud Bono VPS:** same rule should be added to the cloud-side `racecontrol.toml` if cloud observability is desired (currently §S-246 deploy-surface scope item — auth-gap PR also pending; coordinate timing).

