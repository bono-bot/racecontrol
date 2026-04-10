# Phase 354: UI Hardening - Research

**Researched:** 2026-04-11
**Domain:** Next.js admin dashboard UI state management (racingpoint-admin repo)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** `/memberships` and `/wallet-transactions` hidden from sidebar with comments in `AdminLayout.tsx:62,77`. Routes still accessible via direct URL — intentional.
- **D-02:** No other dead routes identified. All nav items point to existing functional pages.
- **D-03:** Use existing `SkeletonTable` from `src/components/Skeleton.tsx` as the standard loading state.
- **D-04:** Pages that already use `Skeleton*` components are the reference pattern. Match their style.
- **D-05:** Empty state: inline pattern `<div className="text-center text-rp-grey py-12"><Icon /><p>No {items} found</p></div>`.
- **D-06:** Toast on mutations via `sonner` v2.0.7. All mutations call `toast.success()` on 200 and `toast.error()` on failure.
- **D-07:** Error state: inline red-box pattern from `/customers` page.
- **D-08:** `/settings/health` already exists with `useSWR` skeleton loading.
- **354-01 ALREADY DONE** — Nav cleanup shipped in prior session.
- **354-03 ALREADY DONE** — `/settings/health` live tiles with `useSWR` + `refreshInterval: 10000` already ships.

### Claude's Discretion

- Exact skeleton layout per page (match nearest existing page pattern)
- Whether to create a shared `<EmptyState>` component or keep inline
- Order of page upgrades (suggest: most-used pages first)

### Deferred Ideas (OUT OF SCOPE)

- Centralized `<ErrorBoundary>` component wrapping all dashboard pages
- `error.tsx` file in app router for automatic error catching
- Automated visual regression tests for loading/empty/error states
- Feature flag system for hiding/showing nav items dynamically
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| UI-01 | `/memberships` + `/wallet-transactions` hidden from nav | DONE — `AdminLayout.tsx:62,77` already hides both (354-01 shipped) |
| UI-02 | Loading skeletons on every rcFetch call — no blank screens | PARTIALLY DONE — 354-02 shipped. 4 remaining text-only loading states confirmed |
| UI-03 | Empty states on every list page | NEEDS WORK — some pages have empty states, several use generic patterns only |
| UI-04 | Error toasts on every mutation success and failure | NEEDS WORK — 15 `alert()` calls identified across 7 pages, no success toasts on those pages |
| UI-05 | `/settings/health` page with live per-subsystem tiles | DONE — `refreshInterval: 10000` confirmed in page source |
| UI-06 | Degraded banner component per page | DONE — `ConnectionIndicator.tsx` already handles degraded/offline/recovery states globally |
| UI-07 | 46-page Playwright smoke test | EXISTS — `crawl-all-pages.spec.ts` covers 45 routes already, needs gap-check |
</phase_requirements>

---

## Summary

Phase 354 is a hardening pass on the `racingpoint-admin` Next.js repo. Three sub-plans were specified; two are already shipped. The research uncovered the precise remaining work:

**354-01 (nav cleanup)** is complete. Both `/memberships` and `/wallet-transactions` have been commented out of `navSections` in `AdminLayout.tsx` with explicit v47.0 annotation comments. No other dead routes found.

**354-02 (loading skeletons)** is largely complete per `354-02-SUMMARY.md` (commit `4c24bad`). However, 4 text-only loading states were missed: `analytics/page.tsx:38`, `billing/analytics/page.tsx:458`, `kiosk/page.tsx:168`, and an inline `wallet-transactions/page.tsx:479`. The primary remaining work is (a) adding `toast.success` + replacing `alert()` with `toast.error` on 7 pages, and (b) adding empty state messages to pages that render blank table bodies.

**354-03 (`/settings/health` live tiles)** is complete. The page uses `useSWR` with `refreshInterval: 10000` (10s) and `keepPreviousData: true`. It shows per-app status badges, response times, last-checked timestamps, and a deploy history table. No work needed.

**Remaining work for UI-02/03/04:** 4 skeleton misses, 15 alert() calls across 7 pages, missing success toasts on mutation pages, and some empty state message quality issues.

**Primary recommendation:** The only plan needing execution is a focused **354-02-remaining** plan: fix 4 skeleton gaps + replace 15 alert() with toast.success/toast.error across 7 pages + verify empty states.

---

## Standard Stack

### Core (all already installed, versions verified)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| next | 16.1.6 | App framework | Project standard |
| swr | ^2.4.1 | Data fetching + caching | Used project-wide for data fetching |
| sonner | ^2.0.7 | Toast notifications | Already installed, Toaster in root layout |
| typescript | (via next) | Type safety | Project standard |
| tailwindcss | (via next) | Styling | Project standard |

### Internal (already exist in repo)

| Component | File | Purpose |
|-----------|------|---------|
| `SkeletonLine` | `src/components/Skeleton.tsx` | Single line pulse |
| `SkeletonCard` | `src/components/Skeleton.tsx` | Card-shaped pulse |
| `SkeletonTable` | `src/components/Skeleton.tsx` | Table with header + N rows x M cols |
| `SkeletonPage` | `src/components/Skeleton.tsx` | Full page: header + cards + table |
| `ConnectionIndicator` | `src/components/ConnectionIndicator.tsx` | Global degraded/offline banner |
| `useToast` | `src/components/Toast.tsx` | Backward-compat wrapper around sonner |

**Import pattern for new toast code:**
```typescript
import { toast } from 'sonner';
// Use directly: toast.success('...') / toast.error('...')
```

**Do NOT use `useToast` wrapper for new code.** It works but `import { toast } from 'sonner'` is the current standard (per Toast.tsx comment: "New code should import { toast } from 'sonner' directly").

---

## Architecture Patterns

### Loading State Pattern (reference: `src/app/(dashboard)/bookings/page.tsx`)

```typescript
// Source: racingpoint-admin/src/app/(dashboard)/bookings/page.tsx
import { SkeletonTable } from '@/components/Skeleton';

{loading ? (
  <SkeletonTable rows={5} cols={9} />
) : data.length === 0 ? (
  <div className="text-center text-rp-grey py-12">No bookings yet</div>
) : (
  <table>...</table>
)}
```

### Empty State Pattern (D-05 from CONTEXT.md)

```typescript
// Inline pattern — no separate component needed (per D-05)
<div className="text-center text-rp-grey py-12">
  <p className="text-sm">No {items} yet</p>
</div>
```

Higher-quality empty states (used on analytics/page.tsx):
```typescript
<p className="text-sm text-rp-grey py-8 text-center">
  No sales data yet — charts will appear as sales are recorded
</p>
```

### Toast on Mutation Pattern (reference: `billing/page.tsx`)

```typescript
// Source: racingpoint-admin/src/app/(dashboard)/billing/page.tsx
import { toast } from 'sonner';

try {
  await api.createItem(data);
  toast.success('Item created');
  load(); // refresh data
} catch (err) {
  toast.error('Failed to create item: ' + (err as Error).message);
}
```

**Replacing alert() calls:** Every `alert('Failed to ...')` in catch blocks becomes `toast.error('Failed to ...')`. Add matching `toast.success('...')` in the try block before `load()` or `onCreated()`.

### Error State Pattern (D-07 — reference: `customers/page.tsx`)

```typescript
// Source: racingpoint-admin/src/app/(dashboard)/customers/page.tsx
{error && (
  <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 mb-6">
    <p className="text-red-400 text-sm mb-2">Failed to load {resource}.</p>
    <button onClick={() => mutate()} className="text-xs text-red-400 underline">
      Retry
    </button>
  </div>
)}
```

### Anti-Patterns to Avoid

- **Using `alert()` for errors:** Blocks the UI thread, no styling, can't be dismissed without user action. Replace with `toast.error()`.
- **Missing success toasts:** Users don't know if their mutation worked. Always pair `toast.success()` with `toast.error()` in mutation handlers.
- **Inline loading text without skeleton:** `if (loading) return <div>Loading...</div>` gives no layout preview. Use `SkeletonTable` for tables, `SkeletonCard` for cards.
- **Empty list = blank white space:** Users can't tell if data loaded empty or failed. Always handle `length === 0` branch explicitly.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Toast notifications | Custom toast component | `toast` from `sonner` | Already installed, Toaster in root layout |
| Loading skeleton | Custom CSS spinner | `SkeletonTable`/`SkeletonCard` from `Skeleton.tsx` | Already matches design system |
| Connection status banner | Page-level degraded component | `ConnectionIndicator.tsx` (already in dashboard layout) | Already handles degraded/offline/recovery states globally |
| Retry on error | Custom retry component | Inline `<button onClick={() => mutate()}>Retry</button>` | SWR's `mutate()` is sufficient |

**Key insight:** UI-06 (degraded banner per page) is already fully satisfied by `ConnectionIndicator.tsx` which is mounted in the dashboard layout and fires on any `degraded` or `offline` connection status. No per-page banner work needed.

---

## Current State Inventory (VERIFIED by grep)

### UI-01: Nav cleanup — COMPLETE
- `AdminLayout.tsx:62` — `/memberships` commented out with `// v47.0 Phase 354-01` note
- `AdminLayout.tsx:77` — `/wallet-transactions` commented out with same note
- No other dead routes in navSections

### UI-02: Loading skeletons — MOSTLY DONE (354-02 shipped)

**354-02 shipped (11 pages):** leaderboard, calendar, waivers (2), packages, coupons, cafe, pricing, cafe/inventory, tournaments, memberships (2), bookings

**Remaining text-only loading states (4 gaps):**

| File | Line | Current | Fix |
|------|------|---------|-----|
| `analytics/page.tsx` | 38 | `<div>Loading analytics...</div>` | `<SkeletonPage cards={0} tableRows={5} />` |
| `billing/analytics/page.tsx` | 458 | `<div>Loading analytics...</div>` | `<SkeletonTable rows={5} cols={5} />` |
| `kiosk/page.tsx` | 168 | `<div>Loading kiosk settings...</div>` | `<SkeletonPage cards={0} tableRows={4} />` |
| `wallet-transactions/page.tsx` | 479 | `{... ? fmt(...) : 'Loading...'}` | `{... ? fmt(...) : <SkeletonLine className="w-16 h-3" />}` |

### UI-03: Empty states — MOSTLY DONE

**Empty states that exist and are good:**
- `coupons` — "No coupons yet"
- `packages` — "No packages configured"
- `leaderboard` — has length === 0 branches
- `tournaments` — has length === 0 branches
- `analytics` — detailed empty state messages per chart section
- `kiosk/experiences` — "No experiences configured"

**Empty states needing audit (no explicit length === 0 handling found):**
- `pricing/page.tsx` — verify empty state when no rules exist
- `waivers/page.tsx` — verify empty state message quality
- `calendar/page.tsx` — verify empty state message quality
- `memberships/page.tsx` — verify (hidden from nav but accessible)

### UI-04: Toast on mutations — NEEDS WORK

**Pages with `alert()` that need toast replacement (15 calls across 7 pages):**

| Page | alert() count | Has toast import? | Fix needed |
|------|--------------|-------------------|-----------|
| `bookings/page.tsx:75` | 1 | Yes (SkeletonTable, has `toast` from some billing) | Add `import { toast } from 'sonner'`, replace alert |
| `coupons/page.tsx:58,66` | 2 | No | Add import + replace + add success toasts |
| `kiosk/page.tsx:118,127,139,157,165` | 5 | No | Add import + replace + add success toasts |
| `leaderboard/page.tsx:79` | 1 | No | Add import + replace + add success toast |
| `pricing/page.tsx:61,66` | 2 | No | Add import + replace + add success toasts |
| `tournaments/page.tsx:93,100,128` | 3 | No | Add import + replace + add success toasts |
| `waivers/page.tsx:62` | 1 | No | Add import + replace |

**Pages that already have correct toast pattern (no work needed):**
billing, billing/rates, billing/history, config, customers, customers/[id], fleet, games, staff/manage, wallet-transactions, mesh-intelligence, hr family, sales, purchases, finance

### UI-05: /settings/health live tiles — COMPLETE
- `useSWR('/api/rc/app-health', fetcher, { refreshInterval: 10000, keepPreviousData: true })` — confirmed
- Shows status badges (ok/degraded/unreachable), response_ms, last_checked, error text
- Local SkeletonCard component used for loading state
- Deploy history table with 20-entry limit

### UI-06: Degraded banner — COMPLETE
- `ConnectionIndicator.tsx` mounted in `src/app/(dashboard)/layout.tsx`
- Shows yellow pill for `degraded`, red pill for `offline`, green "Back online" on recovery (auto-dismiss 3s)
- Uses `ConnectionContext` for status — no per-page work needed

### UI-07: 46-page Playwright smoke test — EXISTS, NEEDS VERIFICATION
- `tests/e2e/crawl-all-pages.spec.ts` — crawls 45 routes, checks for errors/redirects/blank content
- Playwright v1.58.2, config at `playwright.config.ts` (webServer: `npx next dev -p 3200`)
- 9 additional spec files cover specific page behaviors
- Count: 45 routes in crawl-all-pages — verify against current nav (46 pages per UI-07)

---

## Common Pitfalls

### Pitfall 1: Adding toast import but forgetting success toast
**What goes wrong:** Developer replaces `alert('Failed')` with `toast.error()` but forgets to add `toast.success()` in the try block — user still gets no feedback on success.
**How to avoid:** Treat mutations in pairs: every `toast.error()` needs a matching `toast.success()` in the same try/catch.

### Pitfall 2: SkeletonTable col count mismatch
**What goes wrong:** Skeleton renders different column count than actual table — layout shifts after load.
**How to avoid:** Count actual `<th>` or column headers in the page and pass exact `cols={N}` to `SkeletonTable`.

### Pitfall 3: Forgetting to add `import { toast } from 'sonner'`
**What goes wrong:** TypeScript error `Cannot find name 'toast'`. Easy to overlook when doing bulk replacements.
**How to avoid:** Always check if the file already imports toast before adding the import.

### Pitfall 4: Using `useToast` wrapper on new pages
**What goes wrong:** Adds unnecessary hook call, diverges from the preferred direct sonner import.
**How to avoid:** Always `import { toast } from 'sonner'` for new pages. `useToast` is for legacy backward compat only.

### Pitfall 5: Empty state on first render vs empty result
**What goes wrong:** Showing "No items found" during the initial loading phase before data arrives.
**How to avoid:** Always gate empty state on `!loading && data !== undefined && data.length === 0`, not just `data.length === 0`.

---

## Code Examples

### Full pattern for a page with all three states

```typescript
// Source: pattern from bookings/page.tsx + coupons/page.tsx combined
import { SkeletonTable } from '@/components/Skeleton';
import { toast } from 'sonner';

// ... in component:
const [loading, setLoading] = useState(true);
const [items, setItems] = useState<Item[]>([]);
const [error, setError] = useState<string | null>(null);

async function load() {
  setLoading(true);
  try {
    const data = await api.getItems();
    setItems(data);
  } catch (err) {
    setError((err as Error).message);
  } finally {
    setLoading(false);
  }
}

async function handleCreate(form: CreateForm) {
  try {
    await api.createItem(form);
    toast.success('Item created');
    load();
  } catch (err) {
    toast.error('Failed to create item: ' + (err as Error).message);
  }
}

// In JSX:
{error ? (
  <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4 mb-6">
    <p className="text-red-400 text-sm mb-2">{error}</p>
    <button onClick={load} className="text-xs text-red-400 underline">Retry</button>
  </div>
) : loading ? (
  <SkeletonTable rows={5} cols={4} />
) : items.length === 0 ? (
  <div className="text-center text-rp-grey py-12">
    <p className="text-sm">No items yet</p>
  </div>
) : (
  <table>...</table>
)}
```

### Replacing alert() with toast (minimal change)

```typescript
// BEFORE:
} catch { alert('Failed to create rule'); }

// AFTER:
} catch (err) { toast.error('Failed to create rule: ' + (err as Error).message); }

// Also add in try block:
toast.success('Rule created');
load();
```

---

## State of the Art

| Old Approach | Current Approach | Status |
|--------------|------------------|--------|
| `alert()` for errors | `toast.error()` from sonner | Transition in progress — 15 alerts remain |
| Text-only "Loading..." div | `SkeletonTable`/`SkeletonCard` | Mostly done — 4 gaps remain |
| No empty state | Explicit length === 0 branch | Mostly done — a few pages unverified |
| Per-page degraded component | Global `ConnectionIndicator.tsx` | DONE — already in dashboard layout |
| No smoke test | `crawl-all-pages.spec.ts` (45 routes) | EXISTS — needs count verification vs UI-07 |

**Deprecated:**
- `useToast` hook: still works (backward compat shim) but new code should not use it. Import `{ toast }` from `'sonner'` directly.

---

## Open Questions

1. **crawl-all-pages.spec.ts route count vs UI-07 "46 pages"**
   - What we know: `crawl-all-pages.spec.ts` has 45 routes listed
   - What's unclear: UI-07 says "46-page" — which route is the gap?
   - Recommendation: Count current nav items in AdminLayout (including settings, settings/health, staff-level pages) and reconcile. Likely one of: `/staff`, `/settings`, `/settings/pipeline`, or `/presets`.

2. **`pricing/page.tsx` empty state**
   - What we know: No explicit `length === 0` branch found in grep
   - What's unclear: Does the table render empty tbody or is there an implicit empty state?
   - Recommendation: Read the page during execution to confirm.

3. **`billing/live/page.tsx` loading state type**
   - What we know: Has `if (loading) {` branch at line 255
   - What's unclear: What does it render — text or skeleton?
   - Recommendation: Verify during execution. `billing/live` is an operational page (active sessions) — check if 354-02 covered it.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Build + test | Yes | v22.22.0 | — |
| Playwright | UI-07 smoke test | Yes | 1.58.2 | — |
| sonner | Toast (UI-04) | Yes | ^2.0.7 | — |
| swr | Data fetching (UI-02) | Yes | ^2.4.1 | — |
| next | Framework | Yes | 16.1.6 | — |
| racingpoint-admin repo | All work | Yes | C:/Users/bono/racingpoint/racingpoint-admin/ | — |

No missing dependencies. All tooling is available.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Playwright 1.58.2 |
| Config file | `racingpoint-admin/playwright.config.ts` |
| Quick run command | `cd C:/Users/bono/racingpoint/racingpoint-admin && npx playwright test tests/e2e/crawl-all-pages.spec.ts --reporter=list` |
| Full suite command | `cd C:/Users/bono/racingpoint/racingpoint-admin && npx playwright test --reporter=list` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| UI-01 | `/memberships` + `/wallet-transactions` not in nav | smoke | `crawl-all-pages.spec.ts` (checks no redirect/broken) | Yes |
| UI-02 | No blank screens during loading | smoke | `crawl-all-pages.spec.ts` (EMPTY_OR_MINIMAL_CONTENT check) | Yes |
| UI-03 | Empty list shows message not blank | manual-only | Visual verification — empty states depend on real data being absent | N/A |
| UI-04 | Mutations show success/failure toast | manual-only | Toast assertions require triggering actual mutations in test | N/A |
| UI-05 | `/settings/health` tiles update live | smoke | `npx playwright test --grep "health"` | Yes (02-dashboard.spec.ts) |
| UI-06 | Degraded banner shows on connection loss | manual-only | Requires network interruption simulation | N/A |
| UI-07 | 46-page smoke test | smoke | `crawl-all-pages.spec.ts` | Yes (45 routes, 1 gap TBD) |

### Sampling Rate

- Per task commit: `npx playwright test tests/e2e/crawl-all-pages.spec.ts` (full crawl — catches blank/broken pages)
- Per wave merge: Full suite `npx playwright test`
- Phase gate: Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- None for skeleton/toast/empty-state work (TypeScript compilation is the verification gate)
- For UI-07: Reconcile 45 vs 46 routes in `crawl-all-pages.spec.ts` — add the missing route

---

## Sources

### Primary (HIGH confidence)

- Direct file reads: `AdminLayout.tsx`, `Skeleton.tsx`, `Toast.tsx`, `ConnectionIndicator.tsx`, `base.ts`, `layout.tsx`, `settings/health/page.tsx`, `customers/page.tsx`
- Direct grep: all `loading ?`, `alert()`, `toast.`, `SkeletonTable` patterns across dashboard pages
- `354-02-SUMMARY.md` — confirms 354-02 shipped commit `4c24bad` with 11 pages upgraded

### Secondary (MEDIUM confidence)

- `354-CONTEXT.md` — decisions D-01 through D-08 (project decisions)
- `package.json` version numbers for sonner, swr, next, playwright

---

## Metadata

**Confidence breakdown:**
- Current state inventory: HIGH — all findings from direct file reads and grep
- Remaining work list: HIGH — specific file paths and line numbers verified
- Pattern recommendations: HIGH — based on existing code in repo, not external sources

**Research date:** 2026-04-11
**Valid until:** Until next commit to racingpoint-admin (file-level findings)
