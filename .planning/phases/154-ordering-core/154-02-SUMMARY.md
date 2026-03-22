---
phase: 154-ordering-core
plan: "02"
subsystem: pwa
tags: [cafe, ordering, cart, checkout, wallet]
dependency_graph:
  requires: [154-01]
  provides: [pwa-cafe-ordering]
  affects: [pwa/src/app/cafe/page.tsx, pwa/src/lib/api.ts]
tech_stack:
  added: []
  patterns:
    - React useState for ephemeral cart (no persistence, resets on refresh by design)
    - Slide-up panel pattern for cart and checkout modals
    - Wallet balance fetched at checkout time via authenticated api.wallet()
key_files:
  created: []
  modified:
    - pwa/src/lib/api.ts
    - pwa/src/app/cafe/page.tsx
decisions:
  - "Cart state held in React useState — intentionally ephemeral, resets on refresh per user decision"
  - "placeCafeOrder uses driver_id empty string — ignored by customer route per Plan 01 contract"
  - "Wallet balance fetched at checkout open time, not at page load — reduces unnecessary auth calls"
metrics:
  duration: "167s"
  completed_date: "2026-03-22T16:39:35Z"
  tasks_completed: 2
  files_modified: 2
---

# Phase 154 Plan 02: PWA Cafe Cart and Checkout Summary

PWA cafe ordering with cart state, out-of-stock indicators, checkout flow with live wallet balance, and order submission via POST /api/v1/customer/cafe/orders.

## Tasks Completed

| # | Task | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Update PWA types and add placeOrder API method | 08779e48 | pwa/src/lib/api.ts |
| 2 | Add cart state and checkout flow to PWA cafe page | 363c70fc | pwa/src/app/cafe/page.tsx |

## What Was Built

**Task 1 — api.ts changes:**
- `CafeMenuItem` interface extended with `is_countable`, `stock_quantity`, `out_of_stock` fields
- New types: `CafeOrderItem`, `CafeOrderRequest`, `CafeOrderItemDetail`, `CafeOrderResponse`
- `api.placeCafeOrder(items)` method — POST `/customer/cafe/orders` with JWT auth, driver_id empty string (ignored by route)

**Task 2 — cafe/page.tsx changes:**
- `ItemCard` component: "Add to Cart" button (Racing Red), out-of-stock badge (grey overlay), quantity +/- controls when item in cart, countable items capped at `stock_quantity`
- `CartPanel`: slide-up modal with all cart items, line totals, remove buttons, "Checkout" action
- Floating bottom bar: appears when `cartItemCount > 0`, shows count + total + "View Cart"
- `CheckoutPanel`: fetches wallet balance via `api.wallet()`, shows order summary, insufficient balance warning (disabled submit), spinner on submit, error display on failure
- `OrderConfirmation`: shows receipt number, itemised list, total paid, new wallet balance, "Order Another" button clears to menu

## Verification

```
cd pwa && npx tsc --noEmit   → PASS (no output)
cd pwa && npx next build      → PASS (/cafe builds as static page)
```

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `pwa/src/lib/api.ts` — FOUND
- `pwa/src/app/cafe/page.tsx` — FOUND
- Commit `08779e48` — FOUND
- Commit `363c70fc` — FOUND
