---
phase: 354-ui-hardening
plan: 02
status: complete
commit: 4c24bad (racingpoint-admin repo)
---

# 354-02 Summary: Loading State Skeleton Upgrades

## What shipped

Replaced 13 "Loading..." text-only states with animated `SkeletonTable` components across 11 admin dashboard pages. Column counts matched to each page's actual table structure.

## Pages upgraded

| Page | Loading states | SkeletonTable cols |
|------|---------------|--------------------|
| leaderboard | 1 | 4 |
| calendar | 1 | 3 |
| waivers | 2 | 9 + 1 |
| packages | 1 | 4 |
| coupons | 1 | 7 |
| cafe | 1 | 4 |
| pricing | 1 | 7 |
| cafe/inventory | 1 | 6 |
| tournaments | 1 | 5 |
| memberships | 2 | 5 + 6 |
| bookings | 1 | 9 |

## Verification

- TypeScript: 0 errors before, 0 errors after
- Visual: deferred to deploy (loading states are transient <1s)

## Not in this commit

- Empty state messages (audit needed per page)
- Toast on all mutations (audit needed)
- Error boundary components
