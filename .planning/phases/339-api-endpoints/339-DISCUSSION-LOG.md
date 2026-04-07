# Phase 339: API Endpoints - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-07
**Phase:** 339-api-endpoints
**Areas discussed:** Response Field Naming, Cash Refund Endpoint Design, Topup Response Enhancement, Transaction Count
**Mode:** --auto (all areas auto-selected, recommended options chosen)

---

## Response Field Naming

| Option | Description | Selected |
|--------|-------------|----------|
| serde(rename) on WalletInfo | Keep Rust field names, rename JSON output via serde | ✓ |
| Rename Rust fields | Change both Rust and JSON names | |
| Keep _paise names | Don't rename, let frontend handle | |

**User's choice:** [auto] serde(rename) — recommended because cloud_sync builds its own JSON from SQL and won't be affected. Keeps Rust code clear about units (paise) while API speaks "credits" language.
**Notes:** Verified cloud_sync.rs uses `json_object(...)` in SQL, not WalletInfo serde.

---

## Cash Refund Endpoint Design

| Option | Description | Selected |
|--------|-------------|----------|
| New separate endpoint | POST /wallet/{driver_id}/cash-refund | ✓ |
| Modify existing refund | Add type field to existing POST /refund | |

**User's choice:** [auto] New separate endpoint — recommended because existing refund has complex MMA-203 security caps that shouldn't be entangled with cash refund logic.
**Notes:** wallet::cash_refund() already exists from Phase 338. Admin/manager auth only.

---

## Topup Response Enhancement

| Option | Description | Selected |
|--------|-------------|----------|
| Add fields + rename | Add rupee_amount, max_cash_refund; rename existing fields | ✓ |
| Rename only | Just rename existing fields to match "credits" language | |

**User's choice:** [auto] Add fields + rename — recommended because admin UI needs max_cash_refund after topup and rupee_amount for receipt display.
**Notes:** rupee_amount = original deposit amount before bonus.

---

## Transaction Count

| Option | Description | Selected |
|--------|-------------|----------|
| Compute via COUNT query | Add COUNT to get_wallet_info() SELECT | ✓ |
| Denormalized column | Add transactions_count column to wallets table | |

**User's choice:** [auto] Compute via COUNT — recommended because wallets table is small and avoids migration overhead.
**Notes:** Single extra subquery in get_wallet_info().

---

## Claude's Discretion

- Error message formatting for cash refund cap
- Whether to add GET /wallet/{driver_id}/max-cash-refund convenience endpoint
- Deprecation approach for old field names (breaking change acceptable — no external consumers)

## Deferred Ideas

None — all ideas stayed within Phase 339 scope.
