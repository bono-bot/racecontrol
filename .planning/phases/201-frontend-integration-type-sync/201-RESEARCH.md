# Phase 201: Frontend Integration & Type Sync — Research

**Researched:** 2026-03-26
**Domain:** TypeScript type synchronization, Next.js frontend updates, contract testing
**Confidence:** HIGH — all findings from direct codebase inspection

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion.

### Claude's Discretion
- How to update shared-types package (`packages/shared-types/`)
- Contract test approach (vitest vs jest, snapshot vs assertion)
- Which UI components need updates per app
- How to handle WaitingForGame display on kiosk ("Loading..." state)
- Whether to add new admin pages for launch matrix or integrate into existing
- Deploy sequence for 3 Next.js apps

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SYNC-01 | `packages/shared-types/src/billing.ts` BillingSessionStatus has EXACTLY the same 10 variants as Rust | billing.ts currently has 8 variants; 2 missing: `waiting_for_game`, `cancelled_no_playable`. `paused_game_pause` and `paused_disconnect` are also missing from the TS type |
| SYNC-02 | Kiosk local `BillingStatus` type removed — only re-exports from `@racingpoint/types` | kiosk/src/lib/types.ts line 48 has a local `BillingStatus` with only 6 variants, used by `RecentSession.status` on line 96 |
| SYNC-03 | Contract test validates all 10 BillingSessionStatus variants | billing.contract.test.ts currently lists only 8 in VALID_BILLING_STATUSES array |
| SYNC-04 | New `ws-dashboard.contract.test.ts` validates BillingTick and GameStateChanged payload shapes including LaunchDiagnostics | ws-messages.contract.test.ts exists but covers OTA/flags messages only, not BillingTick/GameStateChanged |
| SYNC-05 | Pre-commit hook or CI check for variant count drift between Rust and TS | No such guard exists currently |
| SYNC-06 | `docs/openapi.yaml` BillingSessionStatus enum updated (10 variants) and new endpoints documented | Currently openapi.yaml has BillingSessionStatus but missing new variants and 4 new endpoints |
| SYNC-07 | GameState `loading` variant already exists in TS but `stopping` is already there too — all 6 variants present | pod.ts line 25 already has all 6 GameState variants including `stopping`. No change needed |
| KIOSK-01 | Kiosk handles all 10 billing status variants (no unhandled states) | `billing_session_changed` handler at useKioskSocket.ts line 150 only checks `completed`, `cancelled`, `ended_early` — new `cancelled_no_playable` not handled |
| KIOSK-02 | Kiosk panel shows live session view for all non-terminal billing statuses | `derivePodState` in KioskPodCard.tsx: if billing exists and not completed/ended_early → on_track. WaitingForGame sessions ARE in billingTimers (sent as synthetic BillingSessionInfo from waiting_for_game map) — need visual differentiation |
| KIOSK-03 | Kiosk timer shows "Game Loading..." spinner when status=waiting_for_game, countdown when active | `LiveSessionPanel.tsx` and `PodKioskView.tsx` only check `paused_manual` — no handling for `waiting_for_game` state |
| KIOSK-04 | Kiosk crash recovery displays "Relaunching..." with amber background, "Session Paused" after max retries | No crash recovery UI exists yet; `paused_game_pause` state renders as generic `on_track` state |
| KIOSK-05 | Kiosk reliability warning banner for <70% combos + "Suggest Alternative" modal | No reliability UI in kiosk; requires calling `GET /api/v1/games/alternatives` |
| WEB-01 | Billing page hides End/Pause buttons when status=waiting_for_game | billing/page.tsx line 122 only checks `paused_manual` for isPaused — no handling for `waiting_for_game` |
| WEB-02 | Billing page shows distinct badge colors for 3 paused states | billing/history/page.tsx statusColors only has 5 entries; StatusBadge.tsx has no entries for paused states |
| WEB-03 | StatusBadge.tsx renders all 10 BillingSessionStatus variants with distinct colors | Currently only 12 generic status values, no billing-specific entries for `waiting_for_game`, `paused_game_pause`, `paused_disconnect`, `cancelled_no_playable` |
| ADMIN-01 | Admin billing history shows all 10 billing statuses with badge colors | web/src/app/billing/history/page.tsx statusColors missing new variants |
| ADMIN-02 | Games page active games table shows game_state column with color-coded values | games/page.tsx uses StatusBadge but `stopping` and `loading` not in StatusBadge colors map |
| ADMIN-03 | New `/games/reliability` page showing per-pod/per-game success rates | No such page exists; requires new Next.js page + `getLaunchMatrix()` API call |
| ADMIN-04 | `src/lib/api/metrics.ts` exports typed methods for all 4 metrics endpoints | web/src/lib/api.ts has no metrics functions; routes exist at `/api/v1/metrics/launch-stats`, `/metrics/billing-accuracy`, `/games/alternatives`, `/admin/launch-matrix` |
</phase_requirements>

---

## Summary

Phase 201 is a pure frontend sync phase: Rust backends (Phases 194-200) have shipped new billing states, game states, and metrics APIs, but the TypeScript types, component rendering logic, and contract tests were not updated during those phases. The gap is well-defined and bounded.

The shared-types package (`packages/shared-types/`) is the single source of truth for TS↔Rust type parity. It currently has 8 `BillingSessionStatus` variants but Rust has 10. The two missing variants are `waiting_for_game` and `cancelled_no_playable`. Two other variants — `paused_game_pause` and `paused_disconnect` — are ALREADY in Rust (`BillingSessionStatus::PausedGamePause`, `BillingSessionStatus::PausedDisconnect`) but MISSING from the TS type union.

The web dashboard (`web/`) has its own local `BillingSession` type in `web/src/lib/api.ts` (line 733) that inline-declares 6 variants, separate from shared-types. This is the primary type drift source for the web app. Similarly, `GameState` in `web/src/lib/api.ts` (line 818) declares only 5 variants, missing `loading`.

**There is no admin app at a separate path.** The web app at `:3200` IS the staff-facing dashboard covering both "web" (billing, drivers, fleet) and "admin" (games, analytics, settings). All web/admin requirements map to `web/src/`.

**Primary recommendation:** Update shared-types first → update all local duplicate types to import from shared-types → update UI components → update contract tests → add drift prevention script → rebuild and deploy all 3 apps.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| vitest | ^2.1.0 | Contract tests | Already used in `packages/contract-tests` |
| TypeScript | ^5.6.0 | Type safety | Project-wide |
| Next.js | (installed) | Frontend apps | kiosk/web/pwa all use Next.js |
| Tailwind CSS | (installed) | Styling | Project-wide, all badge/color classes use Tailwind |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @racingpoint/types | workspace | Shared TS types | All 3 apps should import from this package |

**Installation:** No new packages needed — all tools already installed.

**Test command:**
```bash
cd packages/contract-tests && npm test
```

---

## Architecture Patterns

### Current Type Hierarchy (with gaps)

```
packages/shared-types/src/billing.ts          ← SOURCE OF TRUTH (8 variants, needs 10)
  exports BillingSessionStatus

kiosk/src/lib/types.ts
  imports BillingSessionStatus from @racingpoint/types  ← CORRECT
  ALSO declares local BillingStatus (6 variants)       ← MUST REMOVE
  RecentSession uses local BillingStatus               ← MUST UPDATE

web/src/lib/api.ts
  declares BillingSession with inline status union (6 variants)  ← MUST UPDATE
  declares GameState (5 variants, missing "loading")             ← MUST UPDATE

pwa/src/lib/api.ts
  declares BillingSession with status: string                    ← string is OK for PWA
```

### Rust BillingSessionStatus (CONFIRMED — 10 variants)
From `crates/rc-common/src/types.rs` line 332:
```rust
pub enum BillingSessionStatus {
    Pending,          // → "pending"
    WaitingForGame,   // → "waiting_for_game"   ← MISSING from TS
    Active,           // → "active"
    PausedManual,     // → "paused_manual"
    PausedDisconnect, // → "paused_disconnect"  ← MISSING from TS
    PausedGamePause,  // → "paused_game_pause"  ← MISSING from TS
    Completed,        // → "completed"
    EndedEarly,       // → "ended_early"
    Cancelled,        // → "cancelled"
    CancelledNoPlayable, // → "cancelled_no_playable" ← MISSING from TS
}
```
All use `#[serde(rename_all = "snake_case")]`.

### Rust GameState (CONFIRMED — 6 variants)
From `crates/rc-common/src/types.rs` line 412:
```rust
pub enum GameState {
    Idle,       // → "idle"
    Launching,  // → "launching"
    Loading,    // → "loading"
    Running,    // → "running"
    Stopping,   // → "stopping"
    Error,      // → "error"
}
```
kiosk/src/lib/types.ts `GameState` (re-exported from shared-types/pod.ts) already has all 6. web/src/lib/api.ts `GameState` is missing `"loading"`.

### New API Endpoints (Rust, Phase 200, already deployed)
```
GET /api/v1/metrics/launch-stats    → LaunchStatsResponse
GET /api/v1/metrics/billing-accuracy → BillingAccuracyResponse
GET /api/v1/games/alternatives      → AlternativeCombo[]
GET /api/v1/admin/launch-matrix     → LaunchMatrixRow[]
```

Response shapes confirmed from `crates/racecontrol/src/api/metrics.rs`:
```typescript
interface LaunchStatsResponse {
  success_rate: number;
  avg_time_to_track_ms: number | null;
  p95_time_to_track_ms: number | null;
  total_launches: number;
  common_failure_modes: FailureMode[];
  last_30d_trend: string;
}

interface BillingAccuracyResponse {
  avg_delta_ms: number | null;
  max_delta_ms: number | null;
  sessions_with_zero_delta: number;
  sessions_where_billing_never_started: number;
  false_playable_signals: number;
}

interface AlternativeCombo {
  car: string | null;
  track: string | null;
  success_rate: number;
  avg_time_ms: number | null;
  total_launches: number;
}

interface LaunchMatrixRow {
  pod_id: string;
  total_launches: number;
  success_rate: number;
  avg_time_ms: number | null;
  top_3_failure_modes: FailureMode[];
  flagged: boolean;  // true when success_rate < 0.70
}
```

### App Structure (CONFIRMED)
```
racecontrol/
├── packages/
│   ├── shared-types/src/        # SOURCE OF TRUTH for types
│   │   ├── billing.ts           # BillingSessionStatus (ADD 4 variants)
│   │   ├── pod.ts               # GameState (all 6 OK), Pod, etc.
│   │   └── index.ts             # re-exports
│   └── contract-tests/src/      # vitest fixture-based tests
│       ├── billing.contract.test.ts     # UPDATE: add 4 missing variants
│       └── [NEW] ws-dashboard.contract.test.ts  # ADD: BillingTick, GameStateChanged
├── kiosk/src/
│   ├── lib/types.ts             # REMOVE local BillingStatus, update RecentSession
│   ├── hooks/useKioskSocket.ts  # UPDATE: billing_session_changed handler + new WS events
│   └── components/
│       ├── KioskPodCard.tsx     # UPDATE: derivePodState for waiting_for_game
│       ├── PodKioskView.tsx     # UPDATE: waiting_for_game display in deriveKioskState
│       ├── LiveSessionPanel.tsx # UPDATE: show "Game Loading..." when waiting_for_game
│       └── SessionTimer.tsx     # UPDATE: spinner vs countdown based on status
├── web/src/
│   ├── lib/api.ts               # UPDATE: BillingSession status union (6→10), GameState (add "loading")
│   │   [NEW] lib/api/metrics.ts # ADD: getLaunchStats, getBillingAccuracy, getLaunchMatrix, getAlternatives
│   ├── hooks/useWebSocket.ts    # UPDATE: handle new WS events (sentinel_changed, etc.)
│   ├── components/StatusBadge.tsx  # UPDATE: add 10 billing status colors
│   ├── app/billing/page.tsx     # UPDATE: hide buttons for waiting_for_game, show badges
│   ├── app/billing/history/page.tsx  # UPDATE: add colors for new statuses
│   ├── app/games/page.tsx       # UPDATE: game_state column color-coding
│   └── app/games/reliability/   # ADD: new page for launch matrix
└── pwa/src/                     # PWA uses status: string — minimal changes needed
```

### Recommended Project Structure for New Files
```
packages/contract-tests/src/
├── ws-dashboard.contract.test.ts    # NEW
└── fixtures/
    └── ws-dashboard.json            # NEW fixture for BillingTick, GameStateChanged

web/src/lib/api/
└── metrics.ts                       # NEW metrics API client

web/src/app/games/reliability/
└── page.tsx                         # NEW launch matrix page

scripts/
└── check-billing-status-parity.js  # NEW drift prevention script
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Type variant count check | Custom build tool | Simple Node.js script parsing Rust enum and TS union | Regex on both files is sufficient; no AST parser needed |
| Component tests | E2E browser tests | Vitest fixture-based contract tests | Browser tests are overkill for type shape validation |
| Custom badge system | New component | Update existing StatusBadge.tsx | StatusBadge already used in 10+ places, adding variants is 1-line-per-variant |
| New shared-types package | New npm package | Add to existing `packages/shared-types/` | Infrastructure already exists |

---

## Common Pitfalls

### Pitfall 1: Forgetting to Rebuild Before Deploy
**What goes wrong:** Updated shared-types but didn't rebuild kiosk/web/pwa — apps still serve old JS from `.next/standalone/`.
**Why it happens:** Next.js bakes `NEXT_PUBLIC_` vars and types at build time, not runtime.
**How to avoid:** After any type change, rebuild all 3 apps. CLAUDE.md rule: after every deploy, `curl _next/static/` returns 200.
**Warning signs:** App shows correct JSON from API but renders wrong UI (e.g., shows spinner when it should show countdown).

### Pitfall 2: Local Type Shadowing Shared Types
**What goes wrong:** `kiosk/src/lib/types.ts` has a local `BillingStatus` type (line 48) that shadows the imported `BillingSessionStatus`. Components using `BillingStatus` won't see the new variants.
**Why it happens:** Legacy local type was never removed when shared-types was adopted.
**How to avoid:** Remove `BillingStatus` entirely from kiosk/types.ts. Update `RecentSession.status` to use `BillingSessionStatus` from shared-types.

### Pitfall 3: Web App Has Its Own BillingSession Type
**What goes wrong:** `web/src/lib/api.ts` declares `BillingSession` locally (line 724) with only 6 status variants. Components in web/ use this type, not shared-types.
**Why it happens:** Web app predates shared-types package adoption; never migrated.
**How to avoid:** Update the inline type union in api.ts — or better, import `BillingSessionStatus` from shared-types and use it. Either approach is fine; don't create a third type.

### Pitfall 4: billing_session_changed Terminal State Logic
**What goes wrong:** `useKioskSocket.ts` billing_session_changed handler (line 150) removes the session from billingTimers ONLY for `completed`, `cancelled`, `ended_early`. If `cancelled_no_playable` arrives, it stays in billingTimers forever.
**Why it happens:** Handler wasn't updated when new terminal states were added in Phase 198.
**How to avoid:** Add `cancelled_no_playable` to the terminal state check.

### Pitfall 5: isPaused Logic in Web Billing Page
**What goes wrong:** `web/src/app/billing/page.tsx` line 122: `isPaused = billing?.status === "paused_manual"`. Buttons (End/Pause/Resume) are shown/hidden based on this. `waiting_for_game` shows End+Pause buttons that would error.
**Why it happens:** Only one paused state existed when written.
**How to avoid:** Check for `waiting_for_game` specifically — hide buttons, show "Loading..." badge.

### Pitfall 6: CountdownTimer Counts Down During waiting_for_game
**What goes wrong:** `LiveSessionPanel.tsx` line 55-57 runs a countdown interval whenever status is not `paused_manual`. But during `waiting_for_game`, there are 0 remaining_seconds — the countdown immediately hits 0 and may trigger end-session logic.
**Why it happens:** `waiting_for_game` sessions sent as synthetic BillingSessionInfo with `remaining_seconds` from the original request, but billing hasn't started so countdown shouldn't run.
**How to avoid:** Add `waiting_for_game` to the conditions that pause the countdown interval.

### Pitfall 7: Drift Prevention Script Must Be Cross-Platform
**What goes wrong:** A drift prevention script using Unix-only tools (grep, sed) fails on the Windows server where Next.js apps run.
**Why it happens:** racecontrol runs on Windows (CLAUDE.md).
**How to avoid:** Write the drift check as a Node.js script (`scripts/check-billing-status-parity.js`) — reads Rust file and TS file, counts variants, fails with exit code 1 if they don't match. Node.js works on all platforms.

---

## Code Examples

### Correct BillingSessionStatus Type (all 10 variants)
```typescript
// packages/shared-types/src/billing.ts
// Source: crates/rc-common/src/types.rs BillingSessionStatus enum
export type BillingSessionStatus =
  | "pending"
  | "waiting_for_game"      // NEW: game launched, waiting for PlayableSignal
  | "active"
  | "paused_manual"
  | "paused_disconnect"     // NEW: customer disconnected during session
  | "paused_game_pause"     // NEW: customer hit ESC (AC STATUS=PAUSE)
  | "completed"
  | "ended_early"
  | "cancelled"
  | "cancelled_no_playable"; // NEW: game crashed before PlayableSignal (BILL-06)
```

### Terminal State Guard (billing_session_changed handler)
```typescript
// kiosk/src/hooks/useKioskSocket.ts
case "billing_session_changed": {
  const session = msg.data as BillingSession;
  setBillingTimers((prev) => {
    const next = new Map(prev);
    const TERMINAL = ["completed", "cancelled", "ended_early", "cancelled_no_playable"] as const;
    if (TERMINAL.includes(session.status as typeof TERMINAL[number])) {
      next.delete(session.pod_id);
      // ... split continuation logic unchanged ...
    } else {
      next.set(session.pod_id, session);
    }
    return next;
  });
  break;
}
```

### WaitingForGame Countdown Guard
```typescript
// LiveSessionPanel.tsx and KioskPodCard.tsx
useEffect(() => {
  // Don't run countdown when status pauses billing
  if (billing.status === "paused_manual" || billing.status === "waiting_for_game") return;
  const iv = setInterval(() => {
    setLocalRemaining((prev) => Math.max(0, prev - 1));
  }, 1000);
  return () => clearInterval(iv);
}, [billing.id, billing.status]);
```

### StatusBadge for All 10 Billing Statuses
```typescript
// web/src/components/StatusBadge.tsx — add to colors Record
waiting_for_game:      "bg-purple-900/50 text-purple-400",   // Loading...
paused_disconnect:     "bg-orange-900/50 text-orange-400",   // Disconnected
paused_game_pause:     "bg-yellow-900/50 text-yellow-400",   // Game Crashed
paused_manual:         "bg-blue-900/50 text-blue-400",       // Paused
cancelled_no_playable: "bg-red-900/50 text-red-400",         // Never Started
```

### Drift Prevention Script (Node.js)
```javascript
// scripts/check-billing-status-parity.js
const fs = require('fs');
const rustFile = fs.readFileSync('crates/rc-common/src/types.rs', 'utf8');
const tsFile = fs.readFileSync('packages/shared-types/src/billing.ts', 'utf8');

// Count Rust variants: lines between enum BillingSessionStatus { ... }
const rustMatch = rustFile.match(/enum BillingSessionStatus \{([^}]+)\}/s);
const rustVariants = (rustMatch?.[1].match(/^\s+\w+/gm) || []).length;

// Count TS variants: lines with | "..."
const tsVariants = (tsFile.match(/\| "[^"]+"/g) || []).length;

if (rustVariants !== tsVariants) {
  console.error(`BillingSessionStatus drift: Rust has ${rustVariants}, TS has ${tsVariants}`);
  process.exit(1);
}
console.log(`OK: BillingSessionStatus has ${rustVariants} variants in both Rust and TS`);
```

### Metrics API Client
```typescript
// web/src/lib/api/metrics.ts
import { fetchApi } from '../api';

export interface LaunchStatsResponse {
  success_rate: number;
  avg_time_to_track_ms: number | null;
  p95_time_to_track_ms: number | null;
  total_launches: number;
  common_failure_modes: FailureMode[];
  last_30d_trend: string;
}

// ... (all 4 interfaces + 4 export functions)

export function getLaunchMatrix(game: string): Promise<LaunchMatrixRow[]> {
  return fetchApi<LaunchMatrixRow[]>(`/admin/launch-matrix?game=${encodeURIComponent(game)}`);
}
```

### New /games/reliability Page (admin, INTEL-04)
```typescript
// web/src/app/games/reliability/page.tsx
"use client";
// Fetches GET /api/v1/admin/launch-matrix?game=assetto_corsa
// Shows per-pod rows with success_rate colored: <70% = red (flagged), 70-90% = amber, >90% = green
// Data from getLaunchMatrix() in web/src/lib/api/metrics.ts
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Local BillingStatus type in kiosk | Import from @racingpoint/types | Phase 201 (this phase) | Single source of truth, no drift |
| Inline status union in web/api.ts | Use BillingSessionStatus from shared-types | Phase 201 (this phase) | TypeScript catches drift at compile time |
| 8 billing status variants | 10 variants (Phase 198 added WaitingForGame, CancelledNoPlayable) | Phase 198 | New UI states needed |

**Outdated patterns:**
- `BillingStatus` local type in kiosk: Must be removed (shadowing the correct shared type)
- `VALID_BILLING_STATUSES` array in billing.contract.test.ts: Must list all 10 variants
- `statusColors` in billing/history/page.tsx: Only 5 entries — must cover all 10

---

## Open Questions

1. **WaitingForGame in billingTimers**
   - What we know: Rust billing.rs line 1023 — `waiting_for_game` entries are sent via a synthetic `BillingSessionInfo` with `status: WaitingForGame`. These ARE broadcast as `billing_session_list` and `billing_tick`.
   - What's unclear: Do they have `remaining_seconds > 0`? The actual billing hasn't started so countdown would show arbitrary numbers.
   - Recommendation: Show "Game Loading..." text instead of countdown. Set countdown interval to skip when `status === "waiting_for_game"`.

2. **KIOSK-05 Game Picker Context**
   - What we know: Reliability warning should appear when staff selects a combo with <70% success rate, before launch confirmation.
   - What's unclear: The game picker flow (SetupWizard or GamePickerPanel) — what step is the right integration point.
   - Recommendation: Add reliability check in the game picker's launch confirmation step, not in the initial selection. Call `/api/v1/games/alternatives` only when warning needed.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | vitest 2.1.0 |
| Config file | `packages/contract-tests/vitest.config.ts` |
| Quick run command | `cd packages/contract-tests && npm test` |
| Full suite command | `cd packages/contract-tests && npm test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SYNC-01 | BillingSessionStatus has exactly 10 variants | unit | `cd packages/contract-tests && npm test` | ✅ — update billing.contract.test.ts |
| SYNC-03 | Contract test validates all 10 variants | unit | `cd packages/contract-tests && npm test` | ✅ — update VALID_BILLING_STATUSES array |
| SYNC-04 | ws-dashboard contract test validates BillingTick shape | unit | `cd packages/contract-tests && npm test` | ❌ Wave 0 — create ws-dashboard.contract.test.ts |
| SYNC-05 | Drift prevention script exits 1 on mismatch | unit | `node scripts/check-billing-status-parity.js` | ❌ Wave 0 — create script |
| SYNC-06 | OpenAPI spec has 10 variants | manual | inspect docs/openapi.yaml | ✅ — manual update to yaml |
| KIOSK-01..05 | Kiosk handles new states | manual | visual inspection | N/A — requires running kiosk |
| WEB-01..03 | Web handles new states | manual | visual inspection | N/A — requires running web |
| ADMIN-01..04 | Admin metrics page and badges | manual | visual inspection | N/A — requires running web |

### Sampling Rate
- **Per task commit:** `cd packages/contract-tests && npm test`
- **Per wave merge:** `cd packages/contract-tests && npm test` (full suite = same, 38 tests → more after additions)
- **Phase gate:** Contract tests green + all 3 apps rebuilt + static file check passes

### Wave 0 Gaps
- [ ] `packages/contract-tests/src/ws-dashboard.contract.test.ts` — covers SYNC-04 (BillingTick and GameStateChanged payload shapes)
- [ ] `packages/contract-tests/src/fixtures/ws-dashboard.json` — fixture data for ws-dashboard contract test
- [ ] `scripts/check-billing-status-parity.js` — covers SYNC-05 (drift prevention)

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `crates/rc-common/src/types.rs` — BillingSessionStatus enum (10 variants), GameState enum (6 variants), all serde annotations verified
- Direct code inspection: `crates/racecontrol/src/api/metrics.rs` — LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow struct definitions
- Direct code inspection: `crates/rc-common/src/protocol.rs` — DashboardEvent enum, SentinelChanged variant, BillingTick
- Direct code inspection: `packages/shared-types/src/billing.ts` — current 8 variants (gap confirmed)
- Direct code inspection: `packages/contract-tests/src/billing.contract.test.ts` — VALID_BILLING_STATUSES confirms 8 variants currently tested
- Direct code inspection: `kiosk/src/lib/types.ts` — local BillingStatus type (line 48) confirmed
- Direct code inspection: `web/src/lib/api.ts` — local BillingSession type (line 724) with inline 6-variant union
- Direct code inspection: `web/src/components/StatusBadge.tsx` — missing billing-specific entries
- Direct code inspection: `kiosk/src/hooks/useKioskSocket.ts` — billing_session_changed terminal state logic
- Direct code inspection: `kiosk/src/components/PodKioskView.tsx` — deriveKioskState missing waiting_for_game handling

### Secondary (MEDIUM confidence)
- CLAUDE.md project instructions — Standing Rules for Next.js deploy (standalone, static files, NEXT_PUBLIC_ vars)
- 201-CONTEXT.md — phase boundary and integration point list confirmed against actual code

---

## Metadata

**Confidence breakdown:**
- Type gaps (shared-types): HIGH — direct file inspection of both Rust and TypeScript
- UI component gaps: HIGH — code read of all relevant components
- New API endpoint shapes: HIGH — read from Rust struct definitions in metrics.rs
- Contract test gaps: HIGH — ran existing tests (38 pass), confirmed missing test files
- Deploy process: HIGH — documented in CLAUDE.md Standing Rules

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable — no Rust changes expected during this phase)
