# v29.0 Full Assessment Synthesis — 15-Model MMA Audit

**Date:** 2026-03-30 IST
**Models:** 15 (5 security, 5 business logic, 5 edge cases)
**Auditor:** James Vowles (Opus 4.6)

---

## Security Models (5)
- GPT-5.4, GLM-5, Gemini, DeepSeek-R1, Sonnet

## Business Logic Models (5)
- MiniMax, MiMo, Mistral, Qwen3, DeepSeek-V3

## Edge Case Models (5)
- GPT-4.1, QwQ, Hermes, Llama4, Nemotron

---

## CONFIRMED P1 FIXES (Applied)

### FIX-01: Escalation logic — High + >2 attempts stuck at Technician
- **Consensus:** 5/15 models (minimax, mimo, mistral, dsv3, qwen3)
- **File:** `crates/racecontrol/src/escalation.rs:24`
- **Bug:** `auto_fix_attempts <= 2 || severity == "High"` — the `||` means `severity == "High"` is always true for High, so the `Technician` arm always fires. `auto_fix_attempts > 2` branch unreachable for High.
- **Impact:** High-severity issues with 3+ failed auto-fix attempts never escalate to Manager.
- **Fix:** Restructured logic: check `auto_fix_attempts > 2` first (always Manager), then `severity == "High"` (Technician).
- **Tests:** Added `test_high_many_attempts_escalates_to_manager` and `test_low_many_attempts_escalates_to_manager`. All 10 tests pass.

### FIX-02: Feedback metrics — total includes unevaluated (NULL) predictions
- **Consensus:** 3/15 models (minimax, mimo, dsv3)
- **File:** `crates/racecontrol/src/feedback_loop.rs:130`
- **Bug:** `total` counts ALL rows (including `was_accurate IS NULL`), but `accurate` counts only `= 1` and `false_pos` counts only `= 0`. NULL rows inflate denominator, understating precision and FPR.
- **Impact:** 20 unevaluated predictions out of 100 total: precision shows 60% instead of real 75%.
- **Fix:** Added `AND was_accurate IS NOT NULL` to total query.

### FIX-03: Feedback metrics — recall = precision (misleading)
- **Consensus:** 5/15 models (minimax, mimo, mistral, dsv3, qwen3)
- **File:** `crates/racecontrol/src/feedback_loop.rs:178`
- **Bug:** `recall: precision` is statistically meaningless. Recall requires false negative tracking.
- **Fix:** Set `recall: 0.0` with TODO comment. Cannot fix properly without schema change (needs `missed_failures` tracking). No longer misleads operators into thinking recall is measured.

### FIX-04: Dynamic pricing — zero base price produces misleading recommendations
- **Consensus:** 4/15 models (minimax, mimo, mistral, dsv3)
- **File:** `crates/racecontrol/src/dynamic_pricing.rs:55`
- **Bug:** `current_price_paise = 0` → `0 * change_bp = 0` → recommended = 0 with `change_pct = 15.0%`. Displays "15% premium" for a price that stayed at zero.
- **Fix:** Early return when `current_price_paise == 0` with `change_pct: 0.0` and clear reason string. Also added `.max(0)` to prevent negative prices and `.clamp(-10000, 10000)` on basis points.

### FIX-05: Sensitive endpoints exposed in public_routes (no auth)
- **Consensus:** 5/5 security models
- **Endpoints moved from public_routes to staff_routes:**
  - `/debug/db-stats` — leaks table names, row counts
  - `/metrics/launch-stats` — operational launch statistics
  - `/metrics/billing-accuracy` — financial accuracy metrics
  - `/admin/launch-matrix` — fleet reliability intelligence
  - `/mesh/solutions`, `/mesh/solutions/{id}` — troubleshooting knowledge base
  - `/mesh/incidents` — historical failure modes and incident data
  - `/mesh/stats` — fleet health statistics
  - `/cameras/health` — internal camera infrastructure status
- **Left public:** `/games/alternatives` (customer-facing combo recommendations)
- **Verified:** No duplicate route registrations (`grep | sort | uniq -d` returns empty).

### FIX-06: Ollama error messages leak internal infrastructure details
- **Consensus:** 5/5 security models
- **File:** `crates/racecontrol/src/ollama_client.rs:49,69`
- **Bug:** Error strings included internal model names (`qwen2.5:3b`, `llama3.1:8b`), reqwest connection errors (revealing IPs), and HTTP status codes.
- **Fix:** Replaced with generic `"AI diagnosis service unavailable"` / `"AI diagnosis service error"`. Full details logged server-side via `tracing::error!`.

---

## CONFIRMED P2 FIXES (Applied)

### FIX-07: Alert engine integer division truncates percentage display
- **Consensus:** 3/15 models (minimax, mimo, dsv3)
- **File:** `crates/racecontrol/src/alert_engine.rs:38,84`
- **Bug:** `((avg_rev - today_rev) * 100) / avg_rev` is integer division. 33.3% shows as 33%.
- **Fix:** Cast to f64 for percentage calculation: `((a - b) as f64 * 100.0 / a as f64).round() as i64`.

---

## CONFIRMED FALSE POSITIVES (Not Fixed)

### FP-01: EBITDA best/worst day logic inverted
- **Claimed by:** 2 models (mimo, dsv3)
- **Actual code (lines 705-711):**
  ```rust
  match &best_day {
      Some((_, best_val)) if day_ebitda <= *best_val => {} // do nothing if current <= best
      _ => best_day = Some((date_str.clone(), day_ebitda)), // update if current > best OR first day
  }
  ```
- **Verdict:** CORRECT. The `_` arm fires when ebitda > best_val (guard fails) or when None (first iteration). The pattern is unusual but logically sound.

### FP-02: SQL injection in maintenance_store
- **Claimed by:** 1 model (dsr1)
- **Verdict:** FALSE POSITIVE. All queries use `sqlx` parameterized placeholders (`?1`, `?2`). No string concatenation in SQL. 4 other security models confirmed this.

### FP-03: Staff routes "permissive JWT" is exploitable
- **Claimed by:** 3 security models (gpt54, glm5, dsr1)
- **Verdict:** KNOWN DESIGN. The comment describes the middleware logging warnings for debugging — it does NOT mean requests proceed without auth. The middleware enforces JWT validation. The word "permissive" refers to allowing expired-but-recently-valid tokens with a warning, not allowing unauthenticated access.

### FP-04: SSRF via Ollama client
- **Claimed by:** 2 models (gemini, dsr1)
- **Verdict:** FALSE POSITIVE. URL is hardcoded constant (`192.168.31.27:11434`), not user-controlled. No SSRF vector exists.

### FP-05: TaskStatus serialization inconsistency
- **Claimed by:** 3 models (minimax, mistral, dsv3)
- **Verdict:** FRAGILE BUT FUNCTIONAL. Write path strips quotes (`"Open"` → `Open`), read path re-adds them (`Open` → `"Open"` → deserialize). The round-trip works because both paths are consistent. Changing either side risks breaking existing DB data. Noted as tech debt, not a bug.

### FP-06: Payroll rounding errors
- **Claimed by:** 3 models (mimo, mistral, dsv3)
- **Verdict:** ACCEPTABLE. `(total_hours * 60.0).round() as i64` is standard IEEE 754 rounding for sub-minute precision. The error is at most 0.5 minutes per employee per day — negligible for an eSports venue's payroll.

### FP-07: Column name typo `wallet_debit_paice`
- **Claimed by:** 1 model (mistral)
- **Verdict:** FALSE POSITIVE. Searched actual code — column is `wallet_debit_paise` everywhere. Model hallucinated the typo.

### FP-08: Revenue aggregation `unwrap_or(0)` masks DB errors
- **Claimed by:** 2 models (mimo, dsv3)
- **Verdict:** INTENTIONAL DESIGN. `unwrap_or(0)` on revenue queries is the correct behavior for empty/missing data days. Using `?` would crash the aggregator loop on any missing-data day.

### FP-09: nvidia-smi hangs in Session 0
- **Claimed by:** 3 edge models
- **Verdict:** KNOWN — already mitigated. Pod agents run in Session 1 per standing rule. The server doesn't call nvidia-smi. Edge case models were auditing rc-agent code which already has Session 1 enforcement.

---

## DEFERRED (Valid but Not Code Fixes)

### DEF-01: Rate limiting on new endpoints
- **Consensus:** 5/5 security models
- **Status:** P2 — requires `tower-governor` or similar crate integration. Not a code bug but architecture improvement. Deferred to v29.1.

### DEF-02: HR endpoints need manager+ role gating
- **Consensus:** 4/5 security models
- **Status:** P2 — requires role-checking middleware in staff_routes sub-router. HR data (`/hr/recognition`) is behind staff JWT but not role-gated. Deferred to v29.1 (needs role middleware refactor).

### DEF-03: Cloud sync version mismatch
- **Consensus:** 4/5 edge models
- **Status:** P2 — new sync payloads (maintenance_events, employees) may fail on pre-v29 Bono VPS. Needs payload versioning or backward-compatible serialization. Deferred to next cloud deploy.

### DEF-04: Concurrent task updates (no optimistic locking)
- **Consensus:** 3/5 edge models
- **Status:** P2 — last-write-wins on maintenance_tasks. Low risk with small team. Add `updated_at` version check in v29.1.

### DEF-05: Startup order dependencies
- **Consensus:** 3/5 edge models
- **Status:** P2 — some background tasks may start before all tables are initialized. Existing code handles this gracefully with `unwrap_or` defaults, but startup ordering should be reviewed.

### DEF-06: `revenue_other_paise` never populated
- **Consensus:** 2/5 biz models
- **Status:** P3 — field exists but has no data source. EBITDA reports show ₹0 for "other revenue". Document as manual-entry field or add a source.

---

## SUMMARY

| Category | P1 Fixed | P2 Fixed | False Positives | Deferred |
|----------|----------|----------|-----------------|----------|
| Escalation Logic | 1 | — | — | — |
| Feedback Metrics | 2 | — | — | — |
| Dynamic Pricing | 1 | — | — | — |
| Auth/Route Security | 1 | — | 1 | 1 |
| Info Leakage | 1 | — | — | — |
| Alert Engine | — | 1 | — | — |
| EBITDA | — | — | 1 | — |
| SQL Injection | — | — | 1 | — |
| Rate Limiting | — | — | — | 1 |
| Cloud Sync | — | — | — | 1 |
| Concurrency | — | — | — | 1 |
| Serialization | — | — | 1 | — |
| **Total** | **6** | **1** | **9** | **6** |

## Verification

```
cargo check -p racecontrol-crate  ✅ (0 new warnings)
cargo check -p rc-agent-crate     ✅ (0 new warnings)
cargo test -- escalation           ✅ (10/10 pass, including 2 new tests)
Route uniqueness check             ✅ (0 duplicates)
```

## Files Modified

1. `crates/racecontrol/src/escalation.rs` — Fixed escalation logic + added tests
2. `crates/racecontrol/src/feedback_loop.rs` — Fixed total query + recall metric
3. `crates/racecontrol/src/dynamic_pricing.rs` — Zero price guard + negative price clamp
4. `crates/racecontrol/src/ollama_client.rs` — Generic error messages
5. `crates/racecontrol/src/alert_engine.rs` — Float percentage calculations
6. `crates/racecontrol/src/api/routes.rs` — Moved 8 endpoints from public to staff routes
