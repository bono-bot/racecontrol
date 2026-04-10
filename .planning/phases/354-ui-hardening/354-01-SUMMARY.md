---
phase: 354-ui-hardening
plan: "01"
subsystem: admin-dashboard
tags: [ui, skeleton, toast, loading-states, alert-removal]
depends_on: []
provides: [zero-loading-text, zero-alert-calls, toast-feedback-on-mutations]
affects: [racingpoint-admin]
tech_stack:
  added: []
  patterns: [SkeletonPage, SkeletonTable, SkeletonLine, sonner-toast]
key_files:
  created: []
  modified:
    - racingpoint-admin/src/app/(dashboard)/analytics/page.tsx
    - racingpoint-admin/src/app/(dashboard)/billing/analytics/page.tsx
    - racingpoint-admin/src/app/(dashboard)/kiosk/page.tsx
    - racingpoint-admin/src/app/(dashboard)/wallet-transactions/page.tsx
    - racingpoint-admin/src/app/(dashboard)/bookings/page.tsx
    - racingpoint-admin/src/app/(dashboard)/coupons/page.tsx
    - racingpoint-admin/src/app/(dashboard)/leaderboard/page.tsx
    - racingpoint-admin/src/app/(dashboard)/pricing/page.tsx
    - racingpoint-admin/src/app/(dashboard)/tournaments/page.tsx
    - racingpoint-admin/src/app/(dashboard)/waivers/page.tsx
decisions:
  - "Used SkeletonPage(cards=4, tableRows=5) for analytics — matches 4 stat cards + chart section"
  - "Used SkeletonTable(rows=5, cols=7) for billing/analytics — exactly 7 table columns"
  - "Used SkeletonPage(cards=0, tableRows=4) for kiosk — no stat cards, 4 setting sections"
  - "Used inline SkeletonLine span for wallet-transactions refund modal — not a full-page load"
  - "All toast.error() include (err as Error).message for actionable error messages"
  - "toast.success() placed BEFORE data refresh call (load/mutate) per plan D-06"
metrics:
  duration_minutes: 5
  tasks_completed: 2
  files_modified: 10
  completed_date: "2026-04-11"
requirements: [UI-01, UI-02, UI-03, UI-04, UI-05, UI-06, UI-07]
---

# Phase 354 Plan 01: UI Hardening — Skeleton Loading + Toast Feedback Summary

**One-liner:** Replaced all remaining plain "Loading..." text with Skeleton components and replaced all `alert()` calls with `toast.success/error` across 10 admin dashboard pages.

## What Was Built

### Task 1: Replace 4 remaining Loading... text states with Skeleton components

Four pages still had plain text-only loading states after 354-02. All replaced:

| Page | Before | After |
|------|--------|-------|
| `analytics/page.tsx:38` | `<div>Loading analytics...</div>` | `<SkeletonPage cards={4} tableRows={5} />` |
| `billing/analytics/page.tsx:458` | `<div>Loading analytics...</div>` | `<SkeletonTable rows={5} cols={7} />` |
| `kiosk/page.tsx:168` | `<div>Loading kiosk settings...</div>` | `<SkeletonPage cards={0} tableRows={4} />` |
| `wallet-transactions/page.tsx:479` | `'Loading...'` (inline ternary) | `<span className="inline-block w-16 h-3 bg-rp-border/50 rounded animate-pulse" />` |

Column counts verified against actual `<th>` elements before setting `cols`.

### Task 2: Replace alert() calls with toast.success/toast.error across 7 pages

All `alert()` calls replaced. Per-page changes:

| Page | alert() calls removed | toast.success added | toast.error added |
|------|-----------------------|--------------------|-------------------|
| `bookings/page.tsx` | 1 | 1 (Booking cancelled) | 1 |
| `coupons/page.tsx` | 2 | 2 (Coupon created/deleted) | 2 |
| `kiosk/page.tsx` | 5 | 5 (Setting updated/saved, Screen blanked/restored, Experience created/deleted) | 5 |
| `leaderboard/page.tsx` | 1 | 1 (Time trial created) | 1 |
| `pricing/page.tsx` | 2 | 2 (Pricing rule created/deleted) | 2 |
| `tournaments/page.tsx` | 3 | 3 (Tournament created, Bracket generated, Match result recorded) | 3 |
| `waivers/page.tsx` | 1 | 0 (load failure — no success toast on data load) | 1 |

`import { toast } from 'sonner'` added to all 7 pages (wallet-transactions already had it).

## Verification Results

| Check | Result |
|-------|--------|
| `grep "Loading\.\.\." src/app/(dashboard)/` | 0 matches |
| `grep "alert(" src/app/(dashboard)/"` | 0 matches |
| `npx tsc --noEmit` | 0 errors |
| Toast imports on all 7 mutation pages | PASS |
| Playwright screenshots (live .23:3201) | NOT TESTED — requires admin rebuild + deploy |

## Deviations from Plan

None — plan executed exactly as written.

The plan specified 15 alert() calls across 7 pages. Actual count was 15 (5 in kiosk + 1 bookings + 2 coupons + 1 leaderboard + 2 pricing + 3 tournaments + 1 waivers = 15). Exact match.

## Known Stubs

None. All skeleton components use real column/row counts from the actual page tables. All toast messages are functional (wired to real mutation handlers).

## Deploy Note

This plan is code-complete. Deploy requires:
1. `cd racingpoint-admin && npm run build` (Next.js standalone build)
2. Deploy admin tar to server (.23:3201) and cloud (admin.racingpoint.cloud)
3. Post-deploy: run Playwright page-crawler to capture updated screenshots

Admin commit: `531d5f7` (racingpoint-admin repo, pushed to GitHub)

## Self-Check: PASSED

- Files modified: all 10 confirmed in `git diff HEAD~1 --name-only`
- Commit exists: `531d5f7` confirmed via `git log --oneline -1`
- Zero Loading... text: grep confirmed 0 matches
- Zero alert() calls: grep confirmed 0 matches
- TypeScript: tsc --noEmit confirmed 0 errors
