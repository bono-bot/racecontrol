# Phase 43: Wizard Flows + API Pipeline Tests — Research

**Researched:** 2026-03-19
**Domain:** Playwright wizard browser tests + shell API pipeline tests (billing/launch/game-state)
**Confidence:** HIGH — all findings come from direct codebase inspection of files that already exist in the repo.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BROW-02 | AC wizard flow — full 13-step flow with track/car selection, AI config, driving settings | Wizard step analysis (see AC Flow Table); data-testid inventory from Phase 42; `useSetupWizard.ts` SINGLE_FLOW constant |
| BROW-03 | Non-AC wizard flow — simplified 5-step flow for F1 25, EVO, Rally, iRacing | `getFlow()` filter in `useSetupWizard.ts`; confirmed 5 steps: register_driver→select_plan→select_game→select_experience→review |
| BROW-04 | Staff mode booking — `?staff=true&pod=pod-8` bypass path tested end-to-end | `walkin-btn` data-testid confirmed in book/page.tsx line 476; `handleStaffWalkIn()` sets authToken="staff-walkin" and jumps to wizard phase |
| BROW-05 | Experience filtering — only selected game's experiences appear, Custom button hidden for non-AC | `getFlow()` removes select_track/select_car for non-AC; experience API returns game-filtered list; no Custom button in non-AC flow |
| BROW-06 | UI navigation — page transitions, back/forward, step indicators update correctly | `wizard-back-btn`, `wizard-step-title`, `wizard-next-btn` testids from Phase 42; `goBack()`/`goNext()` in hook |
| API-01 | Billing gates — reject launch without billing, create/end session, timer sync | game-launch.sh Gates 2, 3, 6 already cover billing gate + auto-provision; new script should formalize these gates |
| API-02 | Per-game launch — launch each installed game, verify PID or Launching state | game-launch.sh Gate 6 covers single SIM_TYPE; new script must loop over all 5 enabled games |
| API-03 | Game state lifecycle — Idle→Launching→Running→Stop→Idle, timeout at 60s | game-launch.sh Gate 6 checks state after 2s; new script needs longer polling (60s for Steam games) |
| API-04 | Steam dialog auto-dismiss — close "Support Message" windows via WM_CLOSE during launch tests | rc-agent has no built-in dialog dismissal; test must trigger via rc-agent remote_ops exec endpoint on Pod 8 |
| API-05 | Error window screenshot — capture screenshots of unexpected popup/error windows for AI debugger | rc-agent remote_ops exec can run PowerShell screenshot commands; new shell gate after launch |

</phase_requirements>

---

## Summary

Phase 43 adds two test suites to the repo: (1) a Playwright browser spec (`wizard.spec.ts`) that walks each of the five sim wizard flows step-by-step and validates staff mode and experience filtering, and (2) a set of shell API scripts (`tests/e2e/api/billing.sh` and `tests/e2e/api/launch.sh`) that exercise the full billing lifecycle and per-game launch pipeline against Pod 8.

The wizard step logic is entirely in `useSetupWizard.ts`. The AC path uses SINGLE_FLOW (12 steps minus conditional ones), while all non-AC games filter out 8 AC-only steps to produce a fixed 5-step flow. The staff mode bypass is a URL param (`?staff=true&pod=pod-8`) that exposes a `walkin-btn`; clicking it sets `authToken="staff-walkin"` and jumps directly to the wizard phase without OTP. All data-testid attributes needed for Phase 43 Playwright selectors were added in Phase 42.

The existing `game-launch.sh` already covers the billing gate (Gate 2) and single-game launch (Gate 6 for `f1_25`). Phase 43 must NOT duplicate those gates — it must add per-game loop coverage across all 5 enabled non-disabled games and formalize billing lifecycle as a standalone script. Steam dialog handling is done via the rc-agent `remote_ops` exec endpoint (port 8090 on the pod) using PowerShell `SendMessage WM_CLOSE`.

**Primary recommendation:** Two new files in `tests/e2e/api/` (billing.sh + launch.sh) plus one new Playwright spec (`tests/e2e/playwright/kiosk/wizard.spec.ts`). Do not modify the existing `game-launch.sh`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Playwright | 1.58.2 (installed) | Browser wizard automation | Already installed (Phase 41); bundled Chromium; `msedge` channel has documented 30s hang — must stay on Chromium |
| bash + curl + python3 | system | API pipeline tests | Matches existing smoke.sh / game-launch.sh pattern; no new deps |
| lib/common.sh | project | pass/fail/skip/info + summary_exit | Must source in all new .sh scripts |
| lib/pod-map.sh | project | pod_ip() function | Must source in all new .sh scripts |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@playwright/test` fixtures | 1.58.2 | test.extend with auto:true | Import `{ test, expect }` from `'../fixtures/cleanup'` — NOT from `@playwright/test` — to get automatic pod-8 cleanup before every test |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| curl + python3 for API tests | supertest / jest | supertest requires Node.js test runner; bash+curl is already in use across all 3 existing scripts — consistency wins |
| PowerShell for Steam dialog dismiss | AutoHotkey | PowerShell is already available on pods and used in rc-agent; AHK requires installation |

**No new npm installs needed.** Playwright 1.58.2 and `@playwright/test` are already in `package.json`.

---

## Architecture Patterns

### Recommended File Structure

```
tests/e2e/
├── lib/
│   ├── common.sh             # EXISTING — source in all new .sh scripts
│   └── pod-map.sh            # EXISTING — source in all new .sh scripts
├── playwright/
│   ├── fixtures/
│   │   └── cleanup.ts        # EXISTING — import { test, expect } from here
│   └── kiosk/
│       ├── smoke.spec.ts     # EXISTING — do not modify
│       └── wizard.spec.ts    # NEW — BROW-02 through BROW-06
├── api/
│   ├── billing.sh            # NEW — API-01
│   └── launch.sh             # NEW — API-02, API-03, API-04, API-05
├── game-launch.sh            # EXISTING — do not modify
└── smoke.sh                  # EXISTING — do not modify
```

### Pattern 1: Playwright Wizard Spec Structure

The wizard spec must import from the cleanup fixture (not from `@playwright/test` directly) to get automatic pre-test state cleanup on Pod 8.

**Staff mode entry — exact URL and testid sequence:**
```typescript
// Source: kiosk/src/app/book/page.tsx — lines 51-52, 474-476
// isStaffMode = searchParams.get("staff") === "true"
// walkin-btn is only rendered when isStaffMode === true

await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });
await page.locator('[data-testid="walkin-btn"]').click();
// Phase transitions to "wizard" — wizard.state.currentStep becomes "select_plan"
await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible' });
```

**Game selection testid pattern:**
```typescript
// Source: 42-01-SUMMARY.md — dynamic item buttons: data-testid={`game-option-${item.id}`}
// Game IDs come from GAMES array in constants.ts
await page.locator('[data-testid="game-option-assetto_corsa"]').click();
await page.locator('[data-testid="game-option-f1_25"]').click();
await page.locator('[data-testid="game-option-assetto_corsa_evo"]').click();
await page.locator('[data-testid="game-option-assetto_corsa_rally"]').click();
await page.locator('[data-testid="game-option-iracing"]').click();
await page.locator('[data-testid="game-option-le_mans_ultimate"]').click();
```

**Step assertions — use data-testid NOT page title text:**
```typescript
// Source: 42-01-SUMMARY.md pattern — step-{step_name} on root div of conditional block
// These testids are consistent between book/page.tsx and SetupWizard.tsx
await expect(page.locator('[data-testid="step-select-plan"]')).toBeVisible();
await expect(page.locator('[data-testid="step-select-game"]')).toBeVisible();
await expect(page.locator('[data-testid="step-select-experience"]')).toBeVisible();
await expect(page.locator('[data-testid="step-review"]')).toBeVisible();

// AC-only steps — assert NOT present for non-AC games
await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
```

**Step indicator / navigation assertion:**
```typescript
// Source: 42-01-SUMMARY.md — wizard-step-title, wizard-back-btn testids
await expect(page.locator('[data-testid="wizard-step-title"]')).toBeVisible();
await page.locator('[data-testid="wizard-back-btn"]').click();
```

### Pattern 2: AC Wizard — Full Step Sequence

The AC single-player wizard (≥20min tier) runs through SINGLE_FLOW minus conditional exclusions:

```
register_driver → select_plan → select_game → session_splits → player_mode →
session_type → ai_config → select_experience (if preset) OR
                                              select_track + select_car (if custom) →
driving_settings → review
```

**Critical branching rule (source: `useSetupWizard.ts` lines 147-157):**
- `session_splits` is skipped if `selectedTier.duration_minutes < 20`
- If `experienceMode === "preset"` (default): `select_track` and `select_car` are REMOVED — flow shows `select_experience` instead
- If `experienceMode === "custom"`: `select_experience` is REMOVED — flow shows `select_track` and `select_car`

For the BROW-02 test, use a tier with duration >= 20min and test the "preset" branch (simpler, more common). This gives:

**AC preset flow (13 steps with default "preset" experienceMode and tier >= 20min):**
```
register_driver → select_plan → select_game → session_splits → player_mode →
session_type → ai_config → select_experience → driving_settings → review
```
That is 10 steps in single-player preset mode — not 13. The full 13-step sequence only occurs in custom mode:
```
register_driver → select_plan → select_game → session_splits → player_mode →
session_type → ai_config → select_track → select_car → driving_settings → review
```
That is 11 steps. The ROADMAP says "13 steps" which includes the staff wizard's `register_driver` and all branching options counted together. For test purposes, test both the preset branch and custom branch separately.

**In book/page.tsx, `register_driver` is skipped** — wizard starts at `select_plan` because OTP auth already establishes driver identity (see page.tsx line 95-97). In the staff wizard (SetupWizard.tsx), `register_driver` is step 1.

**Non-AC flow — always 5 steps (source: `useSetupWizard.ts` lines 131-143):**
```
register_driver → select_plan → select_game → select_experience → review
```
In `book/page.tsx` (customer path) this becomes 4 steps (no register_driver):
```
select_plan → select_game → select_experience → review
```

### Pattern 3: Shell API Script Structure

```bash
#!/bin/bash
# tests/e2e/api/billing.sh   (or launch.sh)
set -uo pipefail

BASE_URL="${RC_API_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod-8}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

# ... gates using pass/fail/skip/info
summary_exit
```

**Note:** Source paths use `../lib/` because api/ is one level deeper than lib/. Verify this relative path when writing the actual scripts.

**Billing lifecycle gates for billing.sh (API-01):**
1. Create billing session on Pod 8 using driver_test_trial + tier_trial
2. Assert response has `billing_session_id` or `ok: true`
3. Attempt game launch without billing on pod-99 — assert "no active billing" rejection
4. Poll `/billing/sessions/active` — confirm pod-8 session exists
5. End session via `POST /api/v1/billing/{session_id}/stop`
6. Poll until pod-8 no longer appears in active sessions

**Per-game launch gates for launch.sh (API-02 + API-03):**
```
For each GAME_ID in [assetto_corsa, f1_25, assetto_corsa_evo, assetto_corsa_rally, iracing, le_mans_ultimate]:
  Gate: Provision billing on pod-8 (idempotent)
  Gate: Launch game
  Gate: Poll /games/active for state=Launching OR state=Running (max 60s)
  Gate: Record PID if returned
  Gate: Stop game + verify /games/active returns NONE for pod-8
  Gate: Sleep 3s between games to avoid double-launch guard
```

**Steam dialog dismiss (API-04):**
```bash
# After launch command accepted, before PID poll
POD_IP=$(pod_ip "${POD_ID}")
# Send WM_CLOSE to any Steam dialog window via PowerShell
curl -s -X POST "http://${POD_IP}:8091/exec" \
  -H "Content-Type: application/json" \
  -d '{"cmd": "powershell -NonInteractive -Command \"Add-Type -TypeDefinition '\"'\"'using System;using System.Runtime.InteropServices;public class W{[DllImport(\"\"user32\"\")]public static extern IntPtr FindWindow(string c,string t);[DllImport(\"\"user32\"\")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'\''\"'\'' ; $h=([W]::FindWindow($null,''Steam''));if($h -ne [IntPtr]::Zero){[W]::SendMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}\""}'
```

**NOTE:** The rc-agent remote_ops exec port is `8091` in `game-launch.sh` line 224 (not 8090). Confirm this — MEMORY.md says port 8090 for `rc-agent remote_ops`, but game-launch.sh hardcodes `/exec` on port 8091. Use what the existing script uses.

**Error window screenshot (API-05):**
```bash
# Use PowerShell on pod to capture screenshot via .NET Graphics
curl -s -X POST "http://${POD_IP}:8091/exec" \
  -H "Content-Type: application/json" \
  -d "{\"cmd\": \"powershell -NonInteractive -Command \\\"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { $bmp = New-Object System.Drawing.Bitmap($_.Bounds.Width, $_.Bounds.Height); $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($_.Bounds.X, $_.Bounds.Y, 0, 0, $_.Bounds.Size); $bmp.Save('C:/RacingPoint/test-screenshot.png') } \\\"\"}" \
  2>/dev/null
# Then fetch the screenshot via SCP/HTTP if available, or log that screenshot was taken
```

**Simpler approach for API-05:** The REQUIREMENTS.md says "capture screenshots of unexpected popup/error windows on pods for AI debugger analysis." The test only needs to *trigger* the screenshot mechanism and verify the mechanism works (screenshot file exists after the command). Full AI debugger routing is Phase 44 (DEPL-04). Phase 43 just wires the capture command and confirms it runs without error.

### Anti-Patterns to Avoid

- **Importing from `@playwright/test` directly in wizard.spec.ts:** Must import from `'../fixtures/cleanup'` to get the auto cleanup fixture. Using `@playwright/test` directly skips the pre-test cleanup and tests will fail on stale pod state.
- **Using `page.waitForTimeout()` for game state transitions:** Steam games take 10-90s to go from Launching to Running. Use `page.waitForSelector()` with long timeout OR a shell poll loop, not `sleep`.
- **Checking ws_connected via `/api/v1/pods`:** That endpoint lacks the field. Always use `/api/v1/fleet/health` (see Pitfall 2 in PITFALLS.md, Gate 4 in game-launch.sh).
- **Looping all game launch tests without cleanup between each:** Stop + verify game is NONE before launching the next game. Double-launch guard will block test 2+ otherwise.
- **Testing forza:** `forza` (Forza Motorsport) has `enabled: false` in `constants.ts`. Skip it. Enabled games are: assetto_corsa, assetto_corsa_evo, assetto_corsa_rally, f1_25, iracing, le_mans_ultimate, forza_horizon_5.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pre-test pod cleanup | Custom cleanup logic in wizard.spec.ts | `import { test, expect } from '../fixtures/cleanup'` | cleanup.ts already exists (Phase 42); `auto: true` means it runs before every test automatically |
| Pod IP resolution | Hardcoded IPs in launch.sh | `pod_ip()` from lib/pod-map.sh | Already sourced in game-launch.sh; single source of truth |
| pass/fail/skip counters | Custom counter in billing.sh | `pass()`, `fail()`, `skip()`, `summary_exit()` from lib/common.sh | Established pattern across all 3 existing scripts |
| Billing session provisioning | New billing create logic | Copy game-launch.sh Gate 6 pattern (`driver_test_trial` + `tier_trial`) | Pattern is tested and working; same synthetic IDs must be used consistently |
| Step title assertions | Text-based assertions on visible title | `[data-testid="wizard-step-title"]` locator | testids are more resilient to text changes; Phase 42 added them specifically for Phase 43 |

---

## Common Pitfalls

### Pitfall 1: staff mode lands on PHONE screen, not wizard directly
**What goes wrong:** Test navigates to `/book?staff=true&pod=pod-8` expecting to land on the wizard. But the page starts at `phase === "phone"` regardless of staff mode (book/page.tsx line 55: `useState<Phase>(isStaffMode ? "phone" : "phone")`). The staff mode flag only adds the `walkin-btn` to the phone screen — it does NOT skip it.
**How to avoid:** Always click `[data-testid="walkin-btn"]` after navigation in staff mode tests. Wait for `[data-testid="step-select-plan"]` to be visible before asserting wizard state. The smoke spec already demonstrates this pattern (smoke.spec.ts lines 58-68).
**Warning signs:** Test asserts `step-select-plan` is visible immediately after `goto('/book?staff=true...')` and fails with "element not visible".

### Pitfall 2: Forza Motorsport appears in game list but is disabled
**What goes wrong:** Test iterates over GAMES array and clicks `game-option-forza`. The button is rendered but disabled (`enabled: false`). Clicking it may do nothing or render an error, causing the wizard to never advance.
**How to avoid:** Only test enabled games: assetto_corsa, assetto_corsa_evo, assetto_corsa_rally, f1_25, iracing, le_mans_ultimate, forza_horizon_5. Skip `forza` (Forza Motorsport) in all tests.

### Pitfall 3: AC wizard "session_splits" step is conditional on tier duration
**What goes wrong:** Test books a trial tier (5min free trial) and expects to see the `session_splits` step. The tier duration is < 20min, so `session_splits` is filtered out of the flow. Test fails because `step-session-splits` is never visible.
**How to avoid:** The BROW-02 test must use a tier with `duration_minutes >= 20`. Check what tiers are configured on the live server. If only trial tiers are available, skip `session_splits` assertion and add a comment explaining the conditional behavior.

### Pitfall 4: Experience filtering is server-side, not client-side
**What goes wrong:** BROW-05 test selects F1 25 and expects only F1 25 experiences in `step-select-experience`. The filtering logic is: experiences are fetched from `/api/v1/kiosk/experiences` and the component renders all of them. The game filter happens in the component's render, not in the API call.
**How to avoid:** Assert that experiences shown have matching game ID. Check the actual experience data returned by the server for f1_25 — if no experiences are configured for f1_25, the step will show an empty list. The BROW-05 test may need to verify absence of other-game experiences, not presence of f1_25 ones. Also: the "Custom" button for AC is absent for non-AC games because `select_track` and `select_car` are filtered from the flow entirely.

### Pitfall 5: Remote exec port discrepancy — 8090 vs 8091
**What goes wrong:** MEMORY.md says `rc-agent remote_ops` is port 8090. But `game-launch.sh` line 224 uses port 8091 (`http://${POD_IP}:8091/exec`). Using 8090 for the Steam dialog dismiss command will get connection refused.
**How to avoid:** Use port 8091 for the `/exec` endpoint, matching what game-launch.sh already uses. Verify by checking one pod's netstat during a test run.

### Pitfall 6: Game state polling needs 60s timeout for Steam games
**What goes wrong:** API-03 test polls for `state=Running` with a 5s or 10s timeout. F1 25 and other Steam games take 30-90s from `Launching` to `Running` on the pod hardware. Test marks it as failed before the game starts.
**How to avoid:** Accept `state=Launching` as a pass condition for Steam games (matches existing game-launch.sh Gate 6 behavior). Add a 60s polling loop if full `Running` verification is needed. Document which games can be expected to reach `Running` quickly (AC, which is non-Steam) vs slowly (all Steam games).

### Pitfall 7: AC wizard "Custom" experience path vs "Preset" path
**What goes wrong:** BROW-02 test tries to assert `step-select-track` and `step-select-car` are visible for AC. But the default `experienceMode` is `"preset"` (INITIAL_STATE line 62 in useSetupWizard.ts), which REMOVES select_track and select_car from the flow. The test fails because those steps never appear.
**How to avoid:** To test the custom track/car path, the test must explicitly click a "Custom" option in `step-select-experience` to set `experienceMode = "custom"`. Alternatively, scope BROW-02 to the preset path (simpler) and add a separate BROW-02-custom test if needed.

---

## Code Examples

### Wizard Spec: Non-AC Flow Assertion Pattern

```typescript
// Source: useSetupWizard.ts getFlow() lines 131-143
// Non-AC games always produce: [register_driver,] select_plan, select_game, select_experience, review
// In book/page.tsx customer path: register_driver is skipped (handled by OTP auth)

test('non-AC wizard: F1 25 shows exactly select_plan → select_game → select_experience → review', async ({ page }) => {
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });
  await page.locator('[data-testid="walkin-btn"]').click();

  // Step 1: select_plan — pick first tier
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible' });
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // Step 2: select_game — pick F1 25
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible' });
  await page.locator('[data-testid="game-option-f1_25"]').click();

  // Step 3: select_experience (NOT select_track or select_car)
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible' });
  // Assert AC-only steps are NOT present in DOM
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
  // Pick first experience
  await page.locator('[data-testid^="experience-option-"]').first().click();

  // Step 4: review
  await page.locator('[data-testid="step-review"]').waitFor({ state: 'visible' });
});
```

### Wizard Spec: Staff Mode Bypass Pattern

```typescript
// Source: book/page.tsx lines 374-396 — handleStaffWalkIn()
// Staff walk-in: sets authToken="staff-walkin", driver=Walk-in, transitions to wizard phase

test('staff mode: walkin-btn bypasses OTP and reaches wizard', async ({ page }) => {
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });
  // Staff banner should show
  await expect(page.getByText(/Staff Mode/i)).toBeVisible();
  // Walk-in button is present (staff mode only)
  const walkinBtn = page.locator('[data-testid="walkin-btn"]');
  await expect(walkinBtn).toBeVisible();
  // OTP button should NOT be the entry point for walk-in
  await walkinBtn.click();
  // Wizard should start at select_plan — no phone/OTP step
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 });
  // Verify no OTP screen visible
  await expect(page.locator('[data-testid="booking-otp-screen"]')).not.toBeVisible();
});
```

### API billing.sh: Create + End Session Gate Pattern

```bash
# Gate: Create test billing session
BILL_RESP=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"${POD_ID}\", \"driver_id\": \"driver_test_trial\", \"pricing_tier_id\": \"tier_trial\"}" \
    "${BASE_URL}/billing/start" 2>/dev/null)
SESSION_ID=$(echo "$BILL_RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('billing_session_id') or d.get('session_id') or '')
except: print('')
" 2>/dev/null)
if [ -n "$SESSION_ID" ]; then
    pass "Billing session created: ${SESSION_ID}"
else
    BILL_ERR=$(echo "$BILL_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('error','?'))" 2>/dev/null)
    if echo "$BILL_ERR" | grep -q "already has an active"; then
        pass "Billing already active on ${POD_ID}"
    else
        fail "Could not create test billing: ${BILL_ERR}"
    fi
fi

# Gate: End session
if [ -n "$SESSION_ID" ]; then
    STOP_RESP=$(curl -s --max-time 10 -X POST \
        "${BASE_URL}/billing/${SESSION_ID}/stop" 2>/dev/null)
    if echo "$STOP_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); sys.exit(0 if d.get('ok') or d.get('stopped') else 1)" 2>/dev/null; then
        pass "Billing session ${SESSION_ID} ended cleanly"
    else
        fail "Could not stop billing session: ${STOP_RESP}"
    fi
fi
```

### API launch.sh: Per-Game Loop with State Polling

```bash
# Source: game-launch.sh Gate 6 pattern, extended to per-game loop
# Poll for game state with timeout — handles Steam games taking 30-90s
poll_game_state() {
    local pod="$1"
    local max_secs="${2:-60}"
    local i=0
    while [ "$i" -lt "$max_secs" ]; do
        STATE=$(curl -s --max-time 5 "${BASE_URL}/games/active" 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    games = data if isinstance(data, list) else data.get('games', [])
    for g in games:
        if g.get('pod_id','') == '${pod}':
            print(g.get('game_state','unknown'))
            break
    else: print('NONE')
except: print('ERROR')
" 2>/dev/null)
        if echo "$STATE" | grep -qi "running\|launching"; then
            echo "$STATE"
            return 0
        fi
        if echo "$STATE" | grep -qi "error"; then
            echo "$STATE"
            return 1
        fi
        sleep 3
        i=$((i + 3))
    done
    echo "TIMEOUT"
    return 1
}

# Enabled games to test (forza=disabled, skip it)
GAMES_TO_TEST="assetto_corsa f1_25 assetto_corsa_evo assetto_corsa_rally iracing le_mans_ultimate"
for GAME in $GAMES_TO_TEST; do
    echo "--- Testing launch: ${GAME} ---"
    # Ensure billing exists, launch, poll, stop
    # ... (full implementation follows pattern above)
done
```

---

## Wizard Step — Complete Data-testid Inventory

This table lists every testid relevant to Phase 43 wizard tests. All were added in Phase 42 (42-01-SUMMARY.md confirms 97 attributes total).

### Step Container testids (present in both book/page.tsx and SetupWizard.tsx)

| Step Name | Container testid | Notes |
|-----------|-----------------|-------|
| register_driver | `step-register-driver` | Staff wizard only (SetupWizard.tsx) |
| select_plan | `step-select-plan` | Both paths |
| select_game | `step-select-game` | Both paths |
| session_splits | `step-session-splits` | AC only (skipped if tier < 20min) |
| player_mode | `step-player-mode` | AC only |
| session_type | `step-session-type` | AC only |
| ai_config | `step-ai-config` | AC only |
| select_experience | `step-select-experience` | Present when experienceMode=preset |
| select_track | `step-select-track` | Present when experienceMode=custom (AC only) |
| select_car | `step-select-car` | Present when experienceMode=custom (AC only) |
| driving_settings | `step-driving-settings` | AC only |
| review | `step-review` | Both paths |
| multiplayer_lobby | `step-multiplayer-lobby` | Multi player only |

### Dynamic item button testids

| Element | testid Pattern | Example |
|---------|---------------|---------|
| Game option | `game-option-{game_id}` | `game-option-assetto_corsa` |
| Tier option | `tier-option-{tier_id}` | `tier-option-tier_trial` |
| Experience option | `experience-option-{exp_id}` | `experience-option-{uuid}` |
| Track option | `track-option-{track_id}` | `track-option-mugello` |
| Car option | `car-option-{car_id}` | `car-option-abarth_500` |
| Session split option | `split-option-{n}` | `split-option-1` |

### Navigation testids

| Element | testid | Notes |
|---------|--------|-------|
| Step title | `wizard-step-title` | Shows STEP_TITLES[currentStep] |
| Back button | `wizard-back-btn` | Applied to BOTH back button instances in SetupWizard footer |
| Next/Continue button | `wizard-next-btn` | Where applicable |
| Book button | `book-btn` | On review step |
| Launch button | `launch-btn` | Staff wizard review step |
| Cancel button | `cancel-btn` | Top-level cancel |
| Walk-in button | `walkin-btn` | Staff mode phone screen only |

### Phase container testids (book/page.tsx customer path)

| Phase | testid |
|-------|--------|
| Phone entry | `booking-phone-screen` |
| OTP entry | `booking-otp-screen` |
| Wizard active | `booking-wizard-screen` |
| Booking in progress | `booking-processing-screen` |
| Success | `booking-success-screen` |
| Error | `booking-error-screen` |

---

## Wizard Flow Tables

### AC Single-Player, Preset Experiences, Tier >= 20min (default happy path)

| # | Step | testid | Selection needed |
|---|------|--------|-----------------|
| 1 | select_plan | `step-select-plan` | click `tier-option-*` |
| 2 | select_game | `step-select-game` | click `game-option-assetto_corsa` |
| 3 | session_splits | `step-session-splits` | click `split-option-1` |
| 4 | player_mode | `step-player-mode` | click single player button |
| 5 | session_type | `step-session-type` | click practice/race/trackday |
| 6 | ai_config | `step-ai-config` | click continue (AI off default) |
| 7 | select_experience | `step-select-experience` | click `experience-option-*` |
| 8 | driving_settings | `step-driving-settings` | click difficulty button |
| 9 | review | `step-review` | assert visible |

### Non-AC Single-Player (F1 25, EVO, Rally, iRacing, LMU, FH5)

| # | Step | testid | Selection needed |
|---|------|--------|-----------------|
| 1 | select_plan | `step-select-plan` | click `tier-option-*` |
| 2 | select_game | `step-select-game` | click `game-option-{id}` |
| 3 | select_experience | `step-select-experience` | click `experience-option-*` |
| 4 | review | `step-review` | assert visible |

### Staff Mode Entry (book/page.tsx via walk-in)

| # | Action | testid/method |
|---|--------|--------------|
| 0 | Navigate | `page.goto('/book?staff=true&pod=pod-8')` |
| 1 | Assert staff indicator | `page.getByText(/Staff Mode/i)` |
| 2 | Click walk-in | `[data-testid="walkin-btn"]` |
| 3 | Wizard starts at select_plan | `[data-testid="step-select-plan"]` visible |

---

## What game-launch.sh Already Covers (Do Not Duplicate)

Phase 43 API scripts must NOT re-implement what game-launch.sh does. They must extend it.

| Gate | Already in game-launch.sh | Phase 43 adds |
|------|--------------------------|---------------|
| Server reachable | Gate 0 | Do not re-test in billing.sh |
| SimType accepted / invalid rejected | Gate 1 | Do not re-test |
| Billing gate (reject without billing) | Gate 2 | billing.sh formalizes as a standalone lifecycle test |
| Active sessions check | Gate 3 | billing.sh formalizes with create+end |
| Pod agent connected via /fleet/health | Gate 4 | launch.sh must include this pre-gate |
| Double-launch guard + auto-cleanup | Gate 5 | launch.sh must include this pre-gate |
| Full launch of one game (f1_25 by default) | Gate 6 | launch.sh extends to ALL 5+ enabled games |
| Kiosk experiences count | Gate 7 | BROW-05 Playwright covers filtering |
| Kiosk frontend smoke | Gate 8 | Already covered by smoke.spec.ts (Phase 42) |

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| game-launch.sh covers F1 25 by default only | launch.sh loops all enabled games | Phase 43 | Full per-game coverage |
| No browser wizard tests | wizard.spec.ts per-step per-game | Phase 43 | Catches isAc regression (Pitfall 7 in PITFALLS.md) |
| Steam dialog: accept timeout as failure | Steam dialog: attempt WM_CLOSE before polling | Phase 43 | Prevents false negative on first pod boot |
| Error screenshots: manual | Error screenshots: triggered via remote exec | Phase 43 | Foundation for Phase 44 AI debugger routing |

---

## Open Questions

1. **What billing session endpoint is `billing.sh` using to end sessions?**
   - What we know: cleanup.ts uses `POST /api/v1/billing/${session.id}/stop` (cleanup.ts line 32)
   - What's unclear: Does the server return `{ok: true}` or `{stopped: true}` or another shape?
   - Recommendation: Check by running `curl -X POST /api/v1/billing/{known_id}/stop` manually before writing the gate

2. **Does tier_trial have duration_minutes >= 20?**
   - What we know: game-launch.sh uses `tier_trial` for test billing; the free trial is 5min
   - What's unclear: If `tier_trial.duration_minutes < 20`, the AC wizard will skip `session_splits` — BROW-02 test must account for this
   - Recommendation: Fetch `/api/v1/pricing/tiers` and log tier durations at the start of billing.sh; use this to conditionally assert or skip the session_splits step

3. **What is the exact remote_ops exec port — 8090 or 8091?**
   - What we know: MEMORY.md says port 8090; game-launch.sh line 224 uses port 8091
   - What's unclear: Which is current on pods after recent changes?
   - Recommendation: Use `pod_ip pod-8` and `curl -s http://{ip}:8091/health` before assuming the exec port; document the confirmed port in a comment at the top of launch.sh

4. **Are experiences configured in the DB for non-AC games?**
   - What we know: game-launch.sh Gate 7 already handles "no experiences" gracefully with an `info` message
   - What's unclear: If no F1 25 experiences exist in DB, `step-select-experience` will show empty list — test must handle this
   - Recommendation: BROW-05 should assert the experience step is visible and renders without error even if empty; experience *content* assertion should only fire if the API returns >0 experiences for the game

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Playwright 1.58.2 (browser) + bash/curl (API) |
| Config file | `playwright.config.ts` at repo root |
| Quick run command | `npx playwright test tests/e2e/playwright/kiosk/wizard.spec.ts` |
| Full suite command | `npx playwright test && bash tests/e2e/api/billing.sh && bash tests/e2e/api/launch.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BROW-02 | AC wizard reaches review through all steps | browser (Playwright) | `npx playwright test wizard.spec.ts --grep "AC wizard"` | ❌ Wave 0 |
| BROW-03 | Non-AC wizard shows exactly 4 steps (no AC steps) | browser (Playwright) | `npx playwright test wizard.spec.ts --grep "non-AC"` | ❌ Wave 0 |
| BROW-04 | Staff walk-in reaches wizard without OTP | browser (Playwright) | `npx playwright test wizard.spec.ts --grep "staff mode"` | ❌ Wave 0 |
| BROW-05 | F1 25 shows F1 experiences only, no select_track/car | browser (Playwright) | `npx playwright test wizard.spec.ts --grep "experience filtering"` | ❌ Wave 0 |
| BROW-06 | Back/forward nav updates step indicator | browser (Playwright) | `npx playwright test wizard.spec.ts --grep "navigation"` | ❌ Wave 0 |
| API-01 | Billing create/gate/end lifecycle | shell (curl) | `bash tests/e2e/api/billing.sh` | ❌ Wave 0 |
| API-02 | Each game reaches Launching state with billing | shell (curl) | `bash tests/e2e/api/launch.sh` | ❌ Wave 0 |
| API-03 | Game state Idle→Launching→Running→Stop→Idle | shell (curl) | `bash tests/e2e/api/launch.sh` (includes state poll) | ❌ Wave 0 |
| API-04 | Steam dialog WM_CLOSE sent before timeout | shell (curl) | `bash tests/e2e/api/launch.sh --steam-dismiss` | ❌ Wave 0 |
| API-05 | Error window screenshot captured on pod | shell (curl) | `bash tests/e2e/api/launch.sh --screenshot-on-error` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `npx playwright test tests/e2e/playwright/kiosk/wizard.spec.ts` (browser only, ~60s)
- **Per wave merge:** `npx playwright test && bash tests/e2e/api/billing.sh && bash tests/e2e/api/launch.sh`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/e2e/playwright/kiosk/wizard.spec.ts` — covers BROW-02, BROW-03, BROW-04, BROW-05, BROW-06
- [ ] `tests/e2e/api/billing.sh` — covers API-01
- [ ] `tests/e2e/api/launch.sh` — covers API-02, API-03, API-04, API-05
- [ ] `tests/e2e/api/` directory — does not yet exist, must be created

*(No new framework installs needed — Playwright 1.58.2 already installed)*

---

## Sources

### Primary (HIGH confidence)
- `kiosk/src/hooks/useSetupWizard.ts` — exact step flows, `getFlow()` filtering logic, isAc check, SINGLE_FLOW and MULTI_FLOW constants
- `kiosk/src/app/book/page.tsx` — staff mode detection (lines 51-52), walkin-btn testid (line 476), handleStaffWalkIn() (lines 374-396), phase state machine
- `kiosk/src/lib/constants.ts` — GAMES list, enabled/disabled flags, DIFFICULTY_PRESETS
- `kiosk/src/components/SetupWizard.tsx` — step structure, STEP_TITLES, register_driver as first staff wizard step
- `.planning/phases/42-kiosk-source-prep-browser-smoke/42-01-SUMMARY.md` — confirmed 97 data-testid attributes, naming conventions, dynamic button patterns
- `.planning/phases/42-kiosk-source-prep-browser-smoke/42-02-SUMMARY.md` — cleanup fixture pattern, jsErrors module-level scope, import path `'../fixtures/cleanup'`
- `tests/e2e/playwright/fixtures/cleanup.ts` — actual cleanup fixture code, API_BASE and TEST_POD env vars
- `tests/e2e/playwright/kiosk/smoke.spec.ts` — existing spec patterns to follow
- `tests/e2e/game-launch.sh` — all 15 gates, billing/launch/cleanup patterns, remote exec on port 8091
- `tests/e2e/lib/common.sh` — pass/fail/skip/info/summary_exit API
- `tests/e2e/lib/pod-map.sh` — pod_ip() function
- `playwright.config.ts` — baseURL=KIOSK_BASE_URL, workers:1, reuseExistingServer:true
- `.planning/research/PITFALLS.md` — 7 real pitfalls from prior test development (all HIGH confidence)
- `.planning/research/FEATURES.md` — feature landscape, feature dependencies, anti-features

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` — accumulated decisions: Playwright 1.58.2 locked, workers:1 mandatory, shell scripts own API/Playwright owns browser layer
- `.planning/REQUIREMENTS.md` — requirement descriptions for BROW-02 through API-05
- `.planning/ROADMAP.md` — Phase 43 success criteria

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are already installed and in use
- Wizard step flows: HIGH — read directly from useSetupWizard.ts source
- data-testid inventory: HIGH — read from 42-01-SUMMARY.md and confirmed in actual tsx files
- API script patterns: HIGH — read from existing game-launch.sh
- Remote exec port (8090 vs 8091): MEDIUM — discrepancy between MEMORY.md and game-launch.sh; needs verification on pod
- Steam dialog WM_CLOSE implementation: MEDIUM — PowerShell approach is standard Windows pattern but untested against live pod

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable codebase — kiosk source changes slowly)
