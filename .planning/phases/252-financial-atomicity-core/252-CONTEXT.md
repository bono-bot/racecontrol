# Phase 252: Financial Atomicity Core - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase, discuss skipped)

<domain>
## Phase Boundary

Every money-moving operation is atomic, idempotent, and race-condition-free. No double charges, no overspend, no balance drift. This phase hardens the billing start, wallet operations, session finalization, and reconciliation.

Requirements: FATM-01 (atomic billing start), FATM-02 (idempotency keys), FATM-03 (wallet row locking), FATM-04 (CAS session finalization), FATM-05 (tier/rate alignment), FATM-06 (single refund formula), FATM-12 (reconciliation job)

Depends on: Phase 251 (WAL mode + DB stability)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key implementation notes from MMA audit:
- FATM-01: Wrap wallet debit + billing_sessions INSERT + journal_entries INSERT in a single sqlx transaction. On any failure, tx.rollback() leaves wallet unchanged.
- FATM-02: Add `idempotency_key TEXT UNIQUE` column to billing_sessions and wallet_transactions. API checks for existing key before processing. Return original result on duplicate.
- FATM-03: SQLite doesn't support SELECT FOR UPDATE natively. Use BEGIN IMMEDIATE (exclusive write lock) for wallet operations. Alternative: application-level tokio::sync::Mutex keyed by driver_id.
- FATM-04: Session end uses UPDATE billing_sessions SET status='ended_early' WHERE id=? AND status='active'. Check affected rows — if 0, someone already ended it.
- FATM-05: Align tier_30min price to 75000 paise (matching 30 * 2500 rate) OR adjust Standard rate to 2333 paise/min. Pick one, document choice.
- FATM-06: Extract refund formula into a single `compute_refund()` function called from all paths (manual stop, crash auto-end, disconnect timeout).
- FATM-12: Background job every 30 minutes: sum(wallet_transactions) vs wallets.balance_paise. Log discrepancies at ERROR + WhatsApp alert.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/billing.rs` — BillingTimer, start_billing_session, end_billing_session, compute_session_cost
- `crates/racecontrol/src/wallet.rs` — credit(), debit(), refund(), ensure_wallet()
- `crates/racecontrol/src/accounting.rs` — post_topup(), post_refund(), journal entries
- `crates/racecontrol/src/api/routes.rs` — billing endpoints, wallet endpoints
- `crates/racecontrol/src/db/mod.rs` — migrations, pool creation (now with WAL from Phase 251)

### Established Patterns
- SQLite via sqlx with BEGIN IMMEDIATE for write locks
- wallet::credit() and wallet::debit() already use db transactions internally
- billing.rs has compute_session_cost() with tiered rate calculation
- Refund formula exists in at least 2 places (end_billing_session line 2267 and disconnect timeout line 1313)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
