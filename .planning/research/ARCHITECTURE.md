# Architecture Research

**Domain:** E2E Test Suite — multi-layer test runner combining Playwright browser tests, curl-based API tests, and remote deploy verification for a Rust/Axum + Next.js + rc-agent system
**Researched:** 2026-03-19
**Confidence:** HIGH (existing codebase read directly; Playwright official docs verified)

---

## Standard Architecture

### System Under Test (Current State)

```
James (.27)                  Server (.23)                    Pods (.89/.33/.28 etc.)
+-----------------------+    +----------------------------+  +----------------------+
|  Tests live here      |    |  racecontrol :8080          |  |  rc-agent :8090      |
|  tests/e2e/           |    |  +- /api/v1/* (Axum)        |  |  rc-sentry :8091     |
|  +- run-all.sh        +--->+  +- /kiosk/* (Next.js       |  |                      |
|  +- smoke.sh          |    |     proxy, Next on :3300)    |  |  WebSocket -> :8080  |
|  +- cross-process.sh  |    |  +- WebSocket /ws            |  |                      |
|  +- game-launch.sh    |    |                              |  |  Game PIDs on host   |
|  +- playwright/       |    |  SQLite DB: racecontrol.db   |  +----------------------+
|     +- kiosk/         |    +----------------------------+
|     +- api/           |
+-----------------------+
```

### Proposed Test Suite Architecture

```
tests/e2e/
+-----------------------------------------------------------------+
|                                                                  |
|  run-all.sh   (master runner — single entry point)              |
|  +---------+  +-----------+  +----------+  +------------------+ |
|  | Phase 1 |  |  Phase 2  |  | Phase 3  |  |    Phase 4       | |
|  | Preflight|  | API Suite |  | Browser  |  | Deploy Verify    | |
|  | (health) |  | (curl)    |  |(Playwright|  | (binary/service) | |
|  +----+----+  +-----+-----+  +-----+----+  +--------+---------+ |
|       |             |              |                 |           |
|       v             v              v                 v           |
|  smoke.sh    api/              playwright/      deploy/          |
|              +- billing.sh     +- kiosk/        +- verify.sh     |
|              +- launch.sh      |  +- wizard.spec.ts              |
|              +- game-state.sh  |  +- smoke.spec.ts               |
|              +- simtype.sh     +- api/                           |
|                                   +- billing.spec.ts             |
|                                   +- launch.spec.ts              |
+-----------------------------------------------------------------+
|                                                                  |
|  lib/                          results/                          |
|  +- common.sh (shared fns)     +- run-TIMESTAMP/                 |
|  +- pod-map.sh (IP lookup)     |  +- smoke.log                   |
|  +- assert.sh (check/pass/fail)|  +- api.log                     |
|  +- playwright.config.ts       |  +- playwright/                 |
|                                |  +- deploy.log                  |
|                                |  +- summary.json                |
+-----------------------------------------------------------------+
```

---

## Component Boundaries

| Component | Responsibility | Boundary In | Boundary Out |
|-----------|----------------|------------|--------------|
| `run-all.sh` | Master orchestrator. Runs all phases in order, collects exit codes, writes `results/summary.json`, exits with total failure count | Entry: invoked by user or CI | Delegates to: smoke.sh, api/*.sh, playwright runner, deploy/verify.sh |
| `smoke.sh` (existing) | Phase 1 — Preflight. Verifies server is alive, all expected API routes return correct status codes. Fast fail: if smoke fails, abort remaining phases | Entry: called by run-all.sh | Output: exit code 0/N, smoke.log |
| `cross-process.sh` (existing) | Schema compatibility, sync table coverage, service proxy chain checks. Runs independently of smoke | Entry: called by run-all.sh after smoke | Output: exit code 0/N, cross-process.log |
| `api/` shell scripts | Phase 2 — API pipeline tests. Each script tests a specific domain: billing lifecycle, game-state transitions, SimType parsing, double-launch guard. All use curl + python3 for JSON extraction. No browser. | Entry: called by run-all.sh | Targets: racecontrol :8080 /api/v1/* |
| `game-launch.sh` (existing) | Full gate-by-gate launch pipeline test. Already implements billing check, SimType validation, agent connectivity, double-launch guard, auto-cleanup. Belongs in api/ domain. | Entry: called from api/ phase | Targets: :8080 + :8090 + :8091 |
| `playwright/` directory | Phase 3 — Browser tests. Playwright specs only live here. Two sub-domains: `kiosk/` (UI flows, wizard steps, SSR verification) and `api/` (API assertions using `page.request` without browser navigation). | Entry: `npx playwright test` invoked by run-all.sh | Targets: :8080/kiosk/* served through racecontrol proxy |
| `playwright.config.ts` | Single config in `tests/e2e/lib/`. Defines two projects: `chromium` (kiosk browser tests) and `api` (API request tests, no browser). Sets baseURL, timeout, retries. | Read by: playwright runner | Controls: parallelism, retries, reporter path |
| `deploy/verify.sh` | Phase 4 — Deploy verification. Tests binary swap (kill, replace, restart sequence), port conflict detection (:8080 / :3300 / :8091), service restart idempotency, and config propagation. Calls rc-sentry :8091 directly for remote exec verification on pods. | Entry: called by run-all.sh | Targets: :8080 health, :8091 /exec on pod IPs |
| `lib/common.sh` | Shared POSIX functions used by all shell scripts: `pass()`, `fail()`, `skip()`, `info()` with consistent color codes, `PASS`/`FAIL`/`SKIP` counters, and `summary_exit()` that writes results and returns correct exit code | Read by: all .sh scripts via `source` | N/A |
| `lib/pod-map.sh` | Single source of truth for pod IP mapping. Defines `pod_ip pod-N` function. Currently duplicated inline in game-launch.sh — must be extracted here. | Read by: game-launch.sh, deploy/verify.sh | N/A |
| `results/` | Test run artifacts. Created fresh per invocation as `results/run-YYYYMMDD-HHMMSS/`. Contains per-phase logs, Playwright HTML report, and `summary.json` with total pass/fail/skip counts per phase. | Written by: run-all.sh + playwright reporter | Read by: CI, developer post-run |

---

## Recommended Project Structure

```
tests/e2e/
├── run-all.sh                  # Master runner — single entry point
├── smoke.sh                    # Phase 1a: server/endpoint preflight (existing)
├── cross-process.sh            # Phase 1b: schema/sync/proxy checks (existing)
│
├── api/                        # Phase 2: curl-based API tests
│   ├── billing.sh              # Billing lifecycle: start, active, stop, idle
│   ├── game-state.sh           # Game state transitions: launch, running, stop
│   ├── launch.sh               # Full launch pipeline (game-launch.sh migrated here)
│   └── simtype.sh              # SimType parsing: valid/invalid/all 5 game types
│
├── playwright/                 # Phase 3: browser tests
│   ├── kiosk/
│   │   ├── wizard.spec.ts      # Booking wizard: per-game flow (AC, F1, EVO, Rally, iRacing)
│   │   ├── smoke.spec.ts       # Page render: all kiosk routes return 200, no SSR errors
│   │   ├── staff.spec.ts       # Staff dashboard: pod list loads, controls visible
│   │   └── fleet.spec.ts       # Fleet page: pod status cards render with correct states
│   └── api/
│       ├── billing.spec.ts     # Billing API via page.request (no browser needed)
│       └── health.spec.ts      # Health + fleet health endpoint assertions
│
├── deploy/
│   └── verify.sh               # Phase 4: binary swap, port check, service restart, config propagation
│
└── lib/
    ├── common.sh               # Shared: pass/fail/skip, colors, counters, summary_exit
    ├── pod-map.sh              # Pod IP lookup: pod_ip pod-1 → 192.168.31.89
    └── playwright.config.ts    # Playwright config: chromium + api projects, baseURL, timeouts
```

### Structure Rationale

- **api/ vs playwright/api/:** Shell scripts own curl-based tests (fast, no Node dependency). Playwright owns `page.request` API tests where browser cookie/session sharing is needed or where request flows are part of a browser flow.
- **lib/common.sh:** Eliminates the copy-paste of `pass()`/`fail()`/`skip()` that currently exists in all three existing shell scripts. Single change propagates everywhere.
- **lib/pod-map.sh:** The pod IP map is currently hardcoded in game-launch.sh as a Python dict. Extract to shell once — every script that needs to reach a pod imports this.
- **deploy/ is its own phase:** Deploy verification (binary swap, port conflicts) has a different failure mode than API tests. It may modify running services. Isolated phase means run-all.sh can skip it with `--skip-deploy` when running in read-only environments (cloud, CI against staging).
- **results/ is ephemeral:** Never committed. gitignored. Created fresh per run so logs from different runs do not overwrite each other.

---

## Architectural Patterns

### Pattern 1: Phase-Gated Sequential Execution

**What:** run-all.sh runs phases in order. Each phase returns an exit code. If Phase 1 (Preflight) fails, subsequent phases are skipped, not run. The master script collects exit codes and writes a summary before exiting with total failure count.

**When to use:** When later tests depend on preconditions verified by earlier tests. A browser test against the kiosk wizard is meaningless if the server is down. Abort early saves time and avoids misleading failures.

**Trade-offs:** Sequential execution is slower than fully parallel. Acceptable here because phases are coarse-grained (not individual test cases). Individual test cases within a phase can still run in parallel.

**Example:**
```bash
# run-all.sh — phase gate pattern
run_phase() {
    local name="$1"; shift
    local cmd=("$@")
    echo "=== Phase: $name ==="
    if "${cmd[@]}" > "$RESULTS_DIR/${name}.log" 2>&1; then
        PHASE_RESULTS["$name"]="PASS"
    else
        PHASE_RESULTS["$name"]="FAIL"
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
    fi
}

run_phase "preflight"  bash tests/e2e/smoke.sh
[[ "${PHASE_RESULTS[preflight]}" == "FAIL" ]] && { echo "Preflight failed — aborting"; exit 1; }
run_phase "api"        bash tests/e2e/api/billing.sh
run_phase "browser"    npx playwright test --config tests/e2e/lib/playwright.config.ts
run_phase "deploy"     bash tests/e2e/deploy/verify.sh
```

### Pattern 2: Source-Based Shared Library for Shell Tests

**What:** All shell scripts source `lib/common.sh` at the top. `common.sh` defines the `pass`, `fail`, `skip`, `info` functions, color variables, counters (`PASS`, `FAIL`, `SKIP`), and a `summary_exit` function. Scripts call `summary_exit` as their last line which prints totals and exits with `$FAIL`.

**When to use:** Whenever more than two shell scripts need consistent output format and exit codes. Currently `smoke.sh`, `cross-process.sh`, and `game-launch.sh` all define their own versions of these functions with slight inconsistencies.

**Trade-offs:** Scripts are no longer self-contained (require `lib/common.sh` to exist). Acceptable: the whole suite is always run from the repo root via run-all.sh. Standalone execution still works if you source the lib manually.

**Example:**
```bash
#!/bin/bash
# api/billing.sh
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

echo "=== Billing API Tests ==="
HTTP=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$BASE_URL/billing/active")
[ "$HTTP" = "200" ] && pass "GET /billing/active -> 200" || fail "GET /billing/active -> $HTTP"

summary_exit  # prints totals, exits with $FAIL count
```

### Pattern 3: Playwright project split — chromium vs api

**What:** `playwright.config.ts` defines two Playwright projects. `chromium` runs actual browser tests in kiosk/. `api` runs request-only tests in playwright/api/ with `use: { browserName: undefined }` (no browser launched). Both report to the same HTML reporter.

**When to use:** When you want a single `npx playwright test` invocation to cover both browser flows and pure HTTP API assertions, with unified reporting and shared fixtures. Avoids needing a separate HTTP test framework (Jest, Supertest) alongside Playwright.

**Trade-offs:** Playwright is heavier than curl for pure API tests. Justified here because: (a) kiosk is Next.js so Playwright is already required; (b) unified HTML report is more readable than separate log files; (c) cookie sharing between browser and API test contexts is useful for authenticated flows.

**Example:**
```typescript
// lib/playwright.config.ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '../playwright',
  timeout: 30_000,
  retries: 1,
  reporter: [['html', { outputFolder: '../results/playwright' }]],
  use: {
    baseURL: process.env.RC_BASE_URL ?? 'http://192.168.31.23:8080',
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
      testMatch: 'kiosk/**/*.spec.ts',
    },
    {
      name: 'api',
      use: { browserName: 'chromium', headless: true },
      testMatch: 'api/**/*.spec.ts',
    },
  ],
});
```

### Pattern 4: rc-sentry as the deploy verification channel

**What:** Deploy verification tests (Phase 4) use rc-sentry :8091 as their remote exec channel, not rc-agent :8090 or the racecontrol :8080 WebSocket. rc-sentry is the backup exec service — it is intentionally independent of racecontrol and rc-agent. This means Phase 4 can restart rc-agent, kill racecontrol, or swap binaries and still have a verification channel.

**When to use:** Any test that needs to verify behavior across a service restart. Without rc-sentry, a test that kills racecontrol has no way to verify it came back without a network-level retry loop. With rc-sentry on :8091, the test can immediately probe the host even while the main service is restarting.

**Trade-offs:** rc-sentry is unauthenticated (by design, internal LAN only). Tests using it must run on the venue subnet or via Tailscale. Not suitable for cloud CI.

---

## Data Flow

### Master Runner Flow

```
User: bash tests/e2e/run-all.sh [--skip-deploy] [--pod pod-8] [--base-url http://...]
    |
    v
run-all.sh
    |-- mkdir results/run-$(date +%Y%m%d-%H%M%S)
    |
    |-- Phase 1: Preflight
    |       smoke.sh --> curl :8080/api/v1/health      --> PASS/FAIL
    |       cross-process.sh --> node check-schema.js  --> PASS/FAIL
    |       [abort if FAIL]
    |
    |-- Phase 2: API Tests
    |       api/billing.sh   --> curl :8080/api/v1/billing/*  --> log + exit code
    |       api/simtype.sh   --> curl :8080/api/v1/games/*    --> log + exit code
    |       api/launch.sh    --> curl gates 0-7 (game-launch) --> log + exit code
    |       api/game-state.sh--> curl :8080 + :8090           --> log + exit code
    |
    |-- Phase 3: Browser Tests
    |       npx playwright test --config lib/playwright.config.ts
    |           kiosk/smoke.spec.ts   --> Chromium -> :8080/kiosk/*
    |           kiosk/wizard.spec.ts  --> Chromium -> :8080/kiosk/book
    |           api/billing.spec.ts   --> page.request -> :8080/api/v1/*
    |       --> results/playwright/ (HTML report)
    |
    |-- Phase 4: Deploy Verification (skippable)
    |       deploy/verify.sh
    |           curl :8080/health (pre-check)
    |           curl :8091/exec on pod-8 (rc-sentry check)
    |           [simulate binary swap on pod-8 via :8091]
    |           curl :8090/health (rc-agent came back)
    |           curl :8080/fleet/health (server sees pod reconnected)
    |
    v
results/run-TIMESTAMP/summary.json
{
  "timestamp": "...",
  "phases": {
    "preflight": { "pass": 6, "fail": 0, "skip": 0 },
    "api": { "pass": 24, "fail": 0, "skip": 3 },
    "browser": { "pass": 18, "fail": 0, "skip": 0 },
    "deploy": { "pass": 8, "fail": 0, "skip": 0 }
  },
  "total_fail": 0
}

exit code = total_fail (0 = clean)
```

### Playwright Internal Flow (kiosk wizard spec)

```
wizard.spec.ts
    |
    |-- test.beforeAll: GET /api/v1/health (server alive gate)
    |
    |-- test("AC wizard flow")
    |       page.goto('/kiosk/book')
    |       page.locator('[data-testid="sim-select"]').selectOption('assetto_corsa')
    |       page.locator('[data-testid="track-select"]').isVisible()
    |       page.locator('[data-testid="car-select"]').isVisible()
    |       expect(page.locator('[data-testid="wizard-step"]')).toHaveText('Select Track')
    |
    |-- test("F1 25 wizard flow")
    |       ... (same pattern, different sim_type)
    |
    |-- test.afterAll: no cleanup needed (read-only UI test)
```

### Deploy Verification Flow

```
deploy/verify.sh
    |
    |-- pre-check: curl :8080/health → must be 200
    |-- pre-check: curl http://POD_IP:8091/ping → must be 200
    |
    |-- [Binary swap simulation on pod-8]
    |       POST :8091/exec {"cmd": "sc stop RCAgent"}
    |       sleep 2
    |       POST :8091/exec {"cmd": "sc start RCAgent"}
    |       poll :8090/health up to 30s
    |
    |-- [Port conflict check]
    |       POST :8091/exec {"cmd": "netstat -ano | findstr :8090"}
    |       verify exactly one listener
    |
    |-- [Config propagation check]
    |       GET :8080/api/v1/fleet/health
    |       verify pod-8 shows ws_connected: true within 30s
    |
    |-- summary_exit
```

---

## Build Order Implications

The following dependency graph determines what must be built before what:

```
1. lib/common.sh            (no deps — build first)
   lib/pod-map.sh           (no deps — build first)

2. smoke.sh (refactor)      (depends on: lib/common.sh — source it instead of inline fns)
   cross-process.sh (refactor) (same)

3. api/billing.sh           (depends on: lib/common.sh, lib/pod-map.sh)
   api/simtype.sh           (depends on: lib/common.sh)
   api/game-state.sh        (depends on: lib/common.sh, lib/pod-map.sh)
   api/launch.sh            (migrate game-launch.sh, depends on: lib/common.sh, lib/pod-map.sh)

4. lib/playwright.config.ts  (depends on: knowing kiosk basePath=/kiosk — already confirmed)

5. playwright/kiosk/smoke.spec.ts   (depends on: playwright.config.ts, kiosk running)
   playwright/kiosk/wizard.spec.ts  (depends on: kiosk UI having data-testid attributes)
   playwright/api/billing.spec.ts   (depends on: playwright.config.ts)

6. deploy/verify.sh         (depends on: lib/common.sh, lib/pod-map.sh, rc-sentry confirmed on pods)

7. run-all.sh               (depends on: all above — ties everything together)
```

**Critical path note:** `playwright/kiosk/wizard.spec.ts` requires `data-testid` attributes on kiosk UI elements (sim select, track select, wizard step indicator). If those attributes do not exist today, they must be added to the Next.js kiosk as part of the browser test phase — this is a kiosk source change, not just a test file. Verify before committing to wizard spec scope.

---

## Anti-Patterns

### Anti-Pattern 1: Monolithic Master Script

**What people do:** Put all test logic directly in run-all.sh — curl calls, assertions, Playwright invocation, deploy verification, all inline in one 1000-line file.

**Why it's wrong:** A single failure makes the entire file hard to debug. You cannot re-run just the API phase or just the browser phase. The file grows unbounded as tests are added. Cannot be reused for other services (POS, Admin Dashboard) as PROJECT.md requires.

**Do this instead:** run-all.sh contains only phase orchestration (call, capture exit code, summarize). All test logic lives in phase-specific files. Reusability is achieved by parameterizing phase scripts with `BASE_URL`, `POD_ID`, etc. environment variables.

### Anti-Pattern 2: Duplicating pass/fail/skip in every script

**What people do:** Each shell script defines its own color variables and counter functions (as smoke.sh, cross-process.sh, and game-launch.sh all currently do).

**Why it's wrong:** Inconsistent output formatting (smoke.sh uses `echo -e`, game-launch.sh defines `pass()` differently). The pod IP map is defined twice in Python inside game-launch.sh. A bug fix in one script does not propagate.

**Do this instead:** Extract to `lib/common.sh` and `lib/pod-map.sh`. All scripts source them. One fix propagates everywhere.

### Anti-Pattern 3: Browser tests for what curl can verify

**What people do:** Use Playwright for every assertion, including checking that `/api/v1/health` returns 200.

**Why it's wrong:** Browser tests are 10-100x slower than curl and require Chrome to be installed. Playwright should own UI flows and SSR verification. Pure HTTP assertions belong in shell + curl.

**Do this instead:** Phase 2 (api/) handles all HTTP-level API verification. Phase 3 (playwright/) handles only what requires a real browser: page rendering, wizard step progression, SSR error detection, JavaScript-driven UI state.

### Anti-Pattern 4: Tests that require real game sessions running

**What people do:** Write a test that launches AC, waits for a lap time, then asserts on telemetry. This requires a human driver and a physical pod session.

**Why it's wrong:** The test suite must be runnable by James without a customer present. Telemetry verification requires live UDP data. Game launch verification can confirm the PID was created and the launch gate sequence passed — it does not need to verify a lap completed.

**Do this instead:** Game launch tests verify the pipeline gates (billing check, SimType acceptance, agent connectivity, PID creation). Telemetry tests are a separate concern verified manually or via separate integration fixtures. Keep the E2E suite non-destructive and runnable anytime.

---

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| racecontrol :8080 | curl / Playwright page.request | Primary target. All phases hit this. |
| kiosk :3300 (proxied as :8080/kiosk) | Playwright Chromium browser | basePath=/kiosk confirmed in next.config.ts |
| rc-agent :8090 | curl (from server context) | Phase 2 — agent connectivity check via fleet/health endpoint, not direct :8090 from James |
| rc-sentry :8091 | curl POST /exec | Phase 4 only — deploy verification. Direct LAN access required. |
| SQLite racecontrol.db | sqlite3 CLI | cross-process.sh sync table coverage check. Read-only. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| run-all.sh -> phase scripts | bash subshell, captures exit code | Phase scripts are independent processes; no shared state via variables |
| run-all.sh -> playwright | npx playwright test subprocess | Playwright manages its own Node.js process; results via HTML report + exit code |
| lib/common.sh -> all .sh scripts | source (not subshell) | Shares variables (PASS, FAIL, SKIP) in same process context |
| lib/pod-map.sh -> scripts that need pod IPs | source, then call `pod_ip pod-N` | Returns IP string to variable; avoids duplicated Python dict |
| Playwright globalSetup -> specs | environment variables | Pass RC_BASE_URL, auth tokens, pod target via env; not via shared JS objects |

---

## Scaling Considerations

This test suite serves a fixed fleet (8 pods, 1 server). Scale is not user growth — it is test count growth.

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Current (smoke + 2 scripts) | Works as-is. run-all.sh adds structure without breaking anything. |
| 50+ test cases | Playwright parallelism handles browser tests automatically. Shell scripts run sequentially within each phase — fast enough for LAN curl. |
| Multi-service (POS, Admin) | run-all.sh accepts `--service` flag. Each service has its own phase scripts under `tests/e2e/services/pos/` and `tests/e2e/services/admin/`. lib/ is shared. |
| CI (cloud runner) | Pass `--skip-deploy` to omit Phase 4 (requires LAN). Set `RC_BASE_URL` to staging. Deploy verification runs only on-venue. |

---

## Sources

- Playwright official docs — global setup: https://playwright.dev/docs/test-global-setup-teardown
- Playwright official docs — configuration: https://playwright.dev/docs/test-configuration
- Playwright official docs — projects (multiple browser targets): https://playwright.dev/docs/test-configuration#projects
- Existing codebase: tests/e2e/smoke.sh, cross-process.sh, game-launch.sh — read directly 2026-03-19
- Existing codebase: kiosk/next.config.ts — basePath=/kiosk confirmed
- Existing codebase: crates/racecontrol/src/api/routes.rs — API surface read directly
- Existing codebase: crates/rc-sentry/src/main.rs — :8091 exec channel confirmed

---
*Architecture research for: E2E Test Suite — RaceControl monorepo*
*Researched: 2026-03-19*
