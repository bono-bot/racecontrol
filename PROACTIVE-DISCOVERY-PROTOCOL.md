# Proactive Discovery Protocol (PDP) v1.0

**Purpose:** Enable the AI agent to independently discover broken integrations, dead connections, misconfigured values, and non-functional UI elements — WITHOUT the user pointing them out first.

**Root cause (researched via 4-model MMA, 2026-04-04):** AI coding agents are structurally reactive. CGP prevents false completion claims but does not drive discovery. RLHF trains agents to follow instructions, not audit systems. The missing capability is a proactive inspection loop with layered oracles — code tracing alone misses runtime failures, and endpoint probing alone misses wiring gaps.

**When to run:** Before working on ANY module, feature, or page. Also after any deploy that touches a module's backend.

**Cost:** ~5-15 minutes per module. Saves hours of user-reported bug discovery.

---

## The 8 Steps

### Phase 1: Static Analysis (Code Only — No Network)

#### Step 1: MAP — Find All Files
Locate every file belonging to the module across all layers.

```
Glob: **/cameras/**/*.{tsx,ts,jsx,rs,toml,json}
Glob: **/*camera*/**
Grep: "camera" in routes.rs, main.rs, config files
```

**Output:** File inventory with layer tags (frontend, backend-API, backend-logic, config, test, docs).

#### Step 2: WIRE — Trace Frontend → Backend Connections
Extract every outbound call from the frontend and every route from the backend. Cross-reference.

```
# Frontend: extract all fetch/axios/WS calls
Grep: "fetch\(|axios\.|new WebSocket\(" in frontend files → list of URLs

# Backend: extract all registered routes
Grep: "\.get\(|\.post\(|\.put\(|\.delete\(|\.route\(" in routes.rs → list of paths

# Cross-reference
comm -23 <(sort frontend_urls.txt) <(sort backend_routes.txt)  # Frontend calls with no backend
comm -23 <(sort backend_routes.txt) <(sort frontend_urls.txt)  # Backend routes with no frontend caller
```

**Output:** Wiring map. Orphaned frontend calls = broken features. Orphaned backend routes = dead code.

#### Step 3: HANDLERS — Trace Every UI Control
For every interactive element (`<button>`, `<select>`, `onClick`, `onSubmit`, `onChange`, `href`):

1. What is the handler function?
2. Does the handler call an API? Which one?
3. Does the handler update state? What state?
4. Is there visual feedback (loading state, error state, success state)?

**Output:** Control inventory table:
```
| # | Element | Line | Handler | Backend Call | State Update | Feedback |
```

Elements with empty handler, no backend call where one is expected, or no feedback = flagged.

### Phase 2: Dynamic Analysis (Network Probing)

#### Step 4: PROBE — Hit Every Endpoint
For every API endpoint found in Steps 2-3, run a live probe:

```bash
curl -s -w "\n%{http_code}:%{size_download}" <URL>
```

**Flag if:**
- HTTP 4xx/5xx → endpoint broken
- HTTP 200 but body < 15 bytes (`{}`, `[]`, `""`) → endpoint returns empty
- HTTP 200 but body is HTML when JSON expected → catch-all/wrong route
- Timeout > 5s → endpoint hanging
- Response shape doesn't match frontend's TypeScript interface → type mismatch

**Output:** Endpoint health table:
```
| Endpoint | Method | HTTP Status | Body Size | Valid Shape? | Issue |
```

#### Step 5: SANITY — Check Config Values Against Domain Knowledge
For every config value (FPS, timeout, rate, limit, threshold, URL, port, count):

| Domain | Sane Range | Flag If |
|--------|-----------|---------|
| Camera FPS | 5-60 | < 5 or > 120 |
| Timeout (ms) | 1000-60000 | 0 or > 300000 |
| Refresh interval (ms) | 500-30000 | < 100 or > 60000 |
| Port | 1-65535 | 0, or known-conflict |
| URL/IP | Resolves | Empty, localhost in prod, hardcoded dev IP in cloud |
| Count/limit | > 0 | 0 or negative when positive expected |
| Label text | Matches function | "0.5 fps" for a 2-second polling interval |

**Output:** Config sanity table with flagged values and why.

### Phase 3: Behavioral Analysis (Runtime)

#### Step 6: NAVIGATE — Check All Navigation Paths
For every page in the module:

1. What pages link TO this page? (grep for the route)
2. What pages does this page link FROM? (grep for `<Link>`, `<a>`, `router.push`)
3. Are there orphaned pages (exist but unreachable from navigation)?
4. Are there dead-end pages (no way to continue or go back)?

**Output:** Navigation graph. Orphans and dead-ends = flagged.

#### Step 7: CONTRACT — Define "What Working Looks Like"
For each feature in the module, write a health contract:

```yaml
module: cameras
contracts:
  - name: "Grid view loads"
    check: "GET /api/v1/cameras returns array with length > 0"
    probe: "curl -s :8096/api/v1/cameras | jq length"
    threshold: "> 0"

  - name: "Snapshots refresh"
    check: "GET /api/v1/cameras/nvr/{ch}/snapshot returns image > 10KB"
    probe: "curl -s -o /dev/null -w '%{size_download}' :8096/api/v1/cameras/nvr/1/snapshot"
    threshold: "> 10000"

  - name: "Live stream connects"
    check: "WS /api/v1/stream/ws/{ch} returns 101 Upgrade"
    probe: "curl -s -o /dev/null -w '%{http_code}' -H 'Upgrade: websocket' :8096/api/v1/stream/ws/1"
    threshold: "== 101"

  - name: "Playback search returns data"
    check: "GET /api/v1/playback/search returns array (not error)"
    probe: "curl -s ':8096/api/v1/playback/search?camera=entrance&start=...&end=...'"
    threshold: "no 'error' key in response"

  - name: "Navigation complete"
    check: "Live page links to Playback, Playback links to Live"
    probe: "grep 'playback' cameras/page.tsx && grep 'cameras' playback/page.tsx"
    threshold: "both return results"
```

**Output:** Contract file (YAML or markdown). Each contract is a testable assertion.

**NOTE:** Contracts are written DURING the audit, not before. The audit discovers what matters; the contract encodes it for future runs. Over time, contracts accumulate and the protocol becomes faster (Step 7 becomes "run existing contracts + add new ones").

#### Step 8: BROWSER — Headless Behavioral Verification
**Requires:** Playwright installed on James (.27). Test file location: `tests/e2e/playwright/`

For each critical user flow:

```typescript
// Example: cameras page health check
test('cameras grid loads with live snapshots', async ({ page }) => {
  await page.goto('http://192.168.31.23:3200/cameras');
  // Grid should have camera tiles
  const tiles = page.locator('[class*="aspect-video"]');
  await expect(tiles).toHaveCount({ minimum: 1 }, { timeout: 10000 });
  // At least one snapshot should have loaded (img with src containing 'snapshot')
  const loadedImg = page.locator('img[src*="snapshot"]').first();
  await expect(loadedImg).toBeVisible({ timeout: 10000 });
});

test('fullscreen stream connects', async ({ page }) => {
  await page.goto('http://192.168.31.23:3200/cameras');
  // Click first camera tile
  const firstTile = page.locator('img[src*="snapshot"]').first();
  await firstTile.click();
  // Fullscreen overlay should appear
  await expect(page.locator('.fixed.inset-0')).toBeVisible({ timeout: 5000 });
  // Stream status should reach 'connected' within 10s
  // (check for green dot or absence of spinner)
});

test('playback page reachable from cameras', async ({ page }) => {
  await page.goto('http://192.168.31.23:3200/cameras');
  // Should have a link/button to playback
  const playbackLink = page.locator('a[href*="playback"], button:has-text("playback")');
  await expect(playbackLink).toBeVisible({ timeout: 5000 });
});
```

**When Playwright is not available:** Fall back to curl + DOM inference:
- `curl` the page HTML, grep for expected elements
- Check that `<script>` tags reference expected bundles
- Verify `NEXT_PUBLIC_` env vars are baked correctly

**Output:** Playwright test results (PASS/FAIL per assertion) or curl-based DOM check results.

---

## Report Format

After running all 8 steps, present findings as:

```markdown
## PDP Audit: [Module Name]

### Summary
- Files scanned: N
- Endpoints probed: N
- Controls audited: N
- Issues found: N (X critical, Y medium, Z low)

### Issues

#### [CRITICAL] Issue Title
- **What:** Description
- **Evidence:** curl command + output, or code reference
- **Impact:** What breaks for the user
- **Fix complexity:** Low/Medium/High

#### [MEDIUM] Issue Title
...

### Working Correctly
- List of controls/endpoints that passed all checks

### Not Testable (requires human/browser)
- List of things that need visual verification
```

---

## Integration with CGP v4.0

PDP is a **pre-action discovery phase**, not a verification gate. It runs BEFORE H1 (Problem Before Action) for the specific module being worked on.

**Recommended addition to CGP Soft Gates:**

### S6: Proactive Discovery Before Module Work
**When:** Before fixing, enhancing, or auditing any module/feature.
**What:** Run PDP Steps 1-6 minimum. Steps 7-8 for critical modules.
**Why:** Discovery finds issues the user hasn't reported yet. Fixing 5 issues in one pass is cheaper than 5 separate sessions.

---

## Coverage Model

| Detection Layer | What It Catches | Coverage |
|----------------|----------------|----------|
| Step 2 (Wire) | Frontend→Backend disconnects, orphan routes | ~30% of issues |
| Step 3 (Handlers) | Dead buttons, missing feedback, empty handlers | ~20% |
| Step 4 (Probe) | Broken endpoints, empty responses, wrong data | ~25% |
| Step 5 (Sanity) | Bad config values, misleading labels | ~10% |
| Step 6 (Navigate) | Unreachable pages, dead-end flows | ~5% |
| Step 8 (Browser) | Rendering bugs, state management, race conditions | ~10% |
| **Total automated** | | **~90-95%** |
| Human visual check | Screen positioning, physical display, UX feel | ~5-10% |

---

## Accumulation: Contracts Get Smarter Over Time

Each PDP run produces contracts (Step 7). These persist in `tests/contracts/` as YAML files. On subsequent runs:

1. **Run existing contracts first** — fast regression check
2. **Then run discovery steps** — find NEW issues
3. **Add new contracts** for anything found

After 10+ runs, the protocol has a comprehensive contract library and Step 7 becomes the primary check, with discovery steps catching only new/changed features.

---

## Origin

Developed 2026-04-04 via 4-model MMA consensus (GPT-5.4, Claude Opus, Gemini Pro, Nemotron).
- GPT-5.4: Academic research on autonomous exploratory testing (WebExplor, 6-layer protocol)
- Claude Opus: Root cause analysis — no persistent health contract, no outer control loop
- Gemini Pro: Concrete implementation — grep routes, curl endpoints, cross-reference
- Nemotron: SRE adaptation — synthetic monitoring, chaos engineering, contract testing

First tested on cameras module: found 7 issues (2 critical, 3 medium, 2 low), including 2 the user hadn't reported.
