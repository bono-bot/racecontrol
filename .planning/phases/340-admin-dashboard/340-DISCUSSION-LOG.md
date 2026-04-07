# Phase 340: Admin Dashboard - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-07
**Phase:** 340-admin-dashboard
**Areas discussed:** Billing Reports Layout, Transaction History Display, Cash Refund UX, Credit Adjustment UX
**Mode:** --auto (all areas auto-selected, recommended options chosen)

---

## Billing Reports Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Summary cards on existing page | Add 4 metric cards to /billing/reports | ✓ |
| New dedicated wallet reports page | Separate /wallet/reports page | |
| Integrated dashboard with charts | Recharts-powered analytics page | |

**User's choice:** [auto] Summary cards — recommended because the existing reports page already has the card pattern and adding new pages increases navigation complexity.
**Notes:** Data comes from all_wallet_transactions summary endpoint (Phase 339 D-22).

---

## Transaction History Display

| Option | Description | Selected |
|--------|-------------|----------|
| Colored badge per transaction | Green for rupee, blue for credit | ✓ |
| Column with text label | Plain text "rupee" / "credit" column | |
| Icon-only indicator | Small colored dot | |

**User's choice:** [auto] Colored badge — recommended because it matches existing txn_type badge pattern in wallet-transactions page.

---

## Cash Refund UX

| Option | Description | Selected |
|--------|-------------|----------|
| Modal with max display + confirmation | Show max, input amount, confirm dialog | ✓ |
| Inline form in transaction list | Expand-in-place refund form | |
| Separate cash refund page | Dedicated /wallet/cash-refund route | |

**User's choice:** [auto] Modal with confirmation — recommended because ConfirmDialog component exists and modal pattern is consistent with existing refund UX.

---

## Credit Adjustment UX

| Option | Description | Selected |
|--------|-------------|----------|
| Toggle add/remove in single modal | One modal with add/remove toggle | ✓ |
| Two separate buttons | "Add Credits" and "Remove Credits" | |

**User's choice:** [auto] Toggle modal — recommended because it reduces button clutter and uses a single interaction point.

---

## Claude's Discretion

- Wallet API module organization (new file vs extend billing.ts)
- Modal layout details
- Whether to add Recharts trend visualization

## Deferred Ideas

- Trend charts with Recharts
- CSV export of transactions
- Per-driver wallet detail page
