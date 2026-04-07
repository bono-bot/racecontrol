# Phase 340: Admin Dashboard - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Add credit/rupee management UI to the admin portal (`racingpoint-admin/` repo). Update `/billing/reports` with wallet breakdown cards, `/billing/history` or `wallet-transactions` with currency_type badges, add cash refund button with max cap display, add credit adjustment button. Deploy to BOTH local (.23:3201) and cloud.

**Depends on:** Phase 339 (API endpoints must return new field names)
**IMPORTANT:** This phase modifies the `racingpoint-admin/` repo, NOT `racecontrol/`. The admin portal is a separate Next.js app.

</domain>

<decisions>
## Implementation Decisions

### Billing Reports Page (/billing/reports)
- **D-01:** Add 4 summary cards to the existing `/billing/reports` page showing wallet-level aggregates
- **D-02:** Cards: "Rupee Deposits" (from `total_rupee_deposits` in summary), "Bonus Credits" (from `total_bonus_credits`), "Credits Spent" (from `total_debits_paise`), "Cash Refunds" (from `total_cash_refunds`)
- **D-03:** Data source: existing `GET /wallet/transactions?date=YYYY-MM-DD` endpoint — the `summary` object now includes these breakdowns (Phase 339 D-22)
- **D-04:** Reuse the existing card/metric pattern from the reports page (Tailwind, dark theme, Racing Point brand colors)

### Wallet Transactions Page (currency_type badge)
- **D-05:** Add a colored badge per transaction in the `wallet-transactions/page.tsx` showing `currency_type`
- **D-06:** Badge colors: "rupee" → green background (`bg-green-900/30 text-green-400`), "credit" → blue background (`bg-blue-900/30 text-blue-400`). Matches existing txn_type badge pattern.
- **D-07:** The `currency_type` field is already returned by the API (Phase 339 SC-4). Frontend just needs to read and display it.

### Cash Refund Button
- **D-08:** Add "Cash Refund" action button in the wallet-transactions page, per-driver context (when viewing a specific driver's transactions)
- **D-09:** Button click opens a modal/dialog showing: driver name, current balance, max refundable amount (from `max_cash_refund` field in wallet info)
- **D-10:** Modal has: amount input (pre-filled with max or empty), notes field (optional), confirmation step via ConfirmDialog component
- **D-11:** Calls `POST /api/rc/wallet/{driver_id}/cash-refund` with `{ amount_paise, notes }`
- **D-12:** On success: show toast with refund amount and new balance, refresh transaction list
- **D-13:** On error (exceeds cap): show error toast with max allowed amount
- **D-14:** Button only visible to admin/manager role (NOT cashier) — enforce via role check in UI. The API also enforces this server-side.

### Credit Adjustment Button
- **D-15:** Add "Adjust Credits" button alongside existing "Top Up" button in wallet-transactions page
- **D-16:** Opens modal with: amount input, reason dropdown (bonus, correction, penalty, other), free-text notes field
- **D-17:** For ADDING credits: calls existing `POST /api/rc/wallet/{driver_id}/topup` with method "adjustment"
- **D-18:** For REMOVING credits: calls existing `POST /api/rc/wallet/{driver_id}/debit` with reason from dropdown
- **D-19:** Toggle in modal: "Add Credits" (default) / "Remove Credits" — switches which endpoint is called
- **D-20:** Requires reason to be selected before submit (Zod validation)

### API Integration
- **D-21:** Use the existing `rcFetch()` pattern from `src/lib/api/base.ts` for all new API calls
- **D-22:** Add new functions to a wallet API module: `getWalletInfo(driverId)`, `cashRefund(driverId, amount, notes)`, `adjustCredits(driverId, amount, reason, notes)`
- **D-23:** The wallet info response now uses new field names from Phase 339: `balance_credits`, `rupee_deposited`, `rupee_refunded`, `bonus_credited`, `max_cash_refund`, `total_spent`, `transactions_count`

### Deploy
- **D-24:** Build and deploy to BOTH local (server .23:3201) and cloud (racingpoint.cloud:3201)
- **D-25:** Use existing Docker build pipeline: `npm run build` → standalone output → Docker image OR direct `node server.js`
- **D-26:** Verify accessibility at both URLs after deploy

### Claude's Discretion
- Whether to create a new wallet API module file or add functions to existing billing.ts
- Exact modal layout and field ordering
- Whether to add Recharts chart for rupee vs bonus vs spending trend (nice-to-have, not in SC)
- Loading skeleton vs spinner for async operations

</decisions>

<specifics>
## Specific Ideas

- The admin portal uses pure Tailwind CSS (no shadcn/ui, no MUI) — all new UI must match existing patterns
- Brand colors: Racing Red `#E10600`, Card `#222222`, Border `#333333` — defined in globals.css as CSS custom properties
- SWR for data fetching with polling — use existing `useSWR` pattern for wallet data
- ConfirmDialog component exists — reuse for cash refund confirmation
- Toast notifications via sonner — reuse for success/error feedback
- react-hook-form + Zod for form validation — use for cash refund and adjustment modals
- The proxy route at `/api/rc/[...path]` forwards to racecontrol API — all wallet endpoints accessible via `rcFetch('/wallet/...')`

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Admin portal pages (PRIMARY — being modified)
- `racingpoint-admin/src/app/(dashboard)/billing/reports/page.tsx` — Reports page (add wallet summary cards)
- `racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx` — Wallet transactions page (add currency_type badge, cash refund button, adjustment button)
- `racingpoint-admin/src/app/(dashboard)/billing/history/page.tsx` — Billing history (may need currency_type badge too)

### Admin portal infrastructure (patterns to follow)
- `racingpoint-admin/src/lib/api/base.ts` — rcFetch pattern with circuit breaker
- `racingpoint-admin/src/lib/api/billing.ts` — Existing billing API functions (pattern reference)
- `racingpoint-admin/src/components/ConfirmDialog.tsx` — Reusable confirmation dialog
- `racingpoint-admin/src/components/AdminLayout.tsx` — Navigation sidebar (no changes needed)
- `racingpoint-admin/src/app/globals.css` — Brand colors CSS custom properties

### Phase 339 API contract (what the admin portal consumes)
- `.planning/phases/339-api-endpoints/339-CONTEXT.md` — API field names and endpoint designs
- `crates/racecontrol/src/api/routes.rs` — Server-side handlers (reference for request/response shapes)

### Business rules
- Memory: `~/.claude/projects/C--Users-bono/memory/project_credits_rupees_separation.md` — Full business model
- Memory: `~/.claude/projects/C--Users-bono/memory/reference_admin_dashboard_architecture.md` — Admin portal architecture

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ConfirmDialog` component — confirmation modals with yes/no
- `rcFetch()` — proxy-based API client with circuit breaker
- `useSWR` pattern — data fetching with polling and cache
- `sonner` toast — success/error notifications
- `react-hook-form` + `Zod` — form validation
- Existing badge patterns in wallet-transactions for txn_type styling

### Established Patterns
- Pages use `"use client"` directive with SWR for data fetching
- Tailwind-only styling — no CSS modules, no styled-components
- Dark theme with card backgrounds (`bg-[#222222]`), borders (`border-[#333333]`)
- Modal/dialog pattern: state toggle + overlay + form + submit handler
- API calls through `/api/rc/[...path]` proxy route (adds auth token from cookie)

### Integration Points
- `/api/rc/wallet/{driver_id}` — GET wallet info (returns new field names from Phase 339)
- `/api/rc/wallet/{driver_id}/cash-refund` — POST cash refund (Phase 339)
- `/api/rc/wallet/{driver_id}/topup` — POST topup/adjustment
- `/api/rc/wallet/{driver_id}/debit` — POST debit/removal
- `/api/rc/wallet/transactions?date=YYYY-MM-DD` — GET all transactions with summary

</code_context>

<deferred>
## Deferred Ideas

- Trend charts (rupee deposits vs bonus vs spending over time) — could use Recharts, but not in SC
- Export functionality (CSV download of transactions) — future enhancement
- Per-driver wallet detail page — could be a Phase 340.1 polish item

</deferred>

---

*Phase: 340-admin-dashboard*
*Context gathered: 2026-04-07*
*[auto] All decisions derived from ROADMAP.md success criteria + codebase analysis. Admin portal patterns from racingpoint-admin/ repo.*
