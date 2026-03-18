# Phase 41: Test Foundation - Research

**Researched:** 2026-03-19 IST
**Domain:** Shared test infrastructure — POSIX shell library, pod IP map, Playwright install and config, cargo-nextest install and config
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FOUND-01 | Shared shell library (`lib/common.sh`) with pass/fail/skip/info helpers and exit code tracking | Three existing scripts (smoke.sh, game-launch.sh, cross-process.sh) each define their own pass/fail/skip inline — extract pattern is well-understood from reading those files directly |
| FOUND-02 | Shared pod IP map (`lib/pod-map.sh`) with all 8 pod IPs, used by all test scripts | Pod IPs confirmed in MEMORY.md network map; game-launch.sh has pod IP logic hardcoded inline — extract target is clear |
| FOUND-03 | Playwright installed with `playwright.config.ts` — bundled Chromium, `reuseExistingServer`, sequential workers | Full config pattern documented in STACK.md; root package.json exists at repo root; Node.js 22.14 confirmed on James's machine |
| FOUND-05 | cargo-nextest configured for Rust crate tests with per-process isolation and built-in retries | cargo-nextest not yet installed; Rust toolchain 1.93.1 confirmed; workspace Cargo.toml read directly; .config/ dir does not exist yet |
</phase_requirements>

---

## Summary

Phase 41 builds the test scaffold that all subsequent phases (42–44) will stand on. It is entirely infrastructure — no test cases are written in this phase, only the shared libraries, runners, and configs that test cases source and use. The deliverables are four items: `lib/common.sh`, `lib/pod-map.sh`, `playwright.config.ts`, and a `.config/nextest.toml`.

The existing shell tests (smoke.sh, game-launch.sh, cross-process.sh) each define their own `pass()`, `fail()`, `skip()` functions and color variables inline. The RESEARCH.md for the overall project identified this duplication as the top anti-pattern. Phase 41 extracts the common functions into `lib/common.sh` and refactors the three existing scripts to source it. This is a mechanical refactor — behavior does not change, output format becomes consistent, and future scripts get the helpers for free.

The Playwright side is a fresh install. The root `package.json` exists with no test dependencies yet. Node.js 22.14 is already installed on James's machine (above the 20.x minimum). `@playwright/test@1.58.2` goes into the root `package.json` devDependencies, and `playwright.config.ts` is placed at repo root. The config is fully specified in STACK.md — sequential single worker, `reuseExistingServer: true`, bundled Chromium, HTML + JUnit reporters. cargo-nextest is not yet installed; it needs one `cargo install` command, then a `.config/nextest.toml` at workspace root configures per-process isolation and retries.

**Primary recommendation:** Build lib/common.sh first (no dependencies), refactor the three existing scripts to source it, then add lib/pod-map.sh, then install and configure Playwright and nextest. Everything in this phase is additive — nothing in the existing test suite is removed or broken.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@playwright/test` | 1.58.2 | Browser automation test runner — Phase 42+ browser specs depend on this install | Current release (Jan 2026). Includes test runner, fixtures, API request context, trace capture, and retry logic. Do NOT install the lower-level `playwright` package separately. |
| `cargo-nextest` | 0.9.x (latest stable) | Rust crate test runner — replaces `cargo test` for per-process isolation | Prevents billing state leakage between Rust tests. 3x faster parallel execution. Auto-retry support. JUnit XML output. Standard in the Rust ecosystem. |
| POSIX bash | system | Shell test library — `lib/common.sh` and `lib/pod-map.sh` | No new dependency; git bash already present. POSIX-compatible for portability across James's Windows git bash and any future CI. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| TypeScript | 5.9.3 (reuse kiosk) | Type-safe playwright.config.ts | Reuse kiosk's existing TypeScript — do NOT add a new TS version. Add playwright.config.ts to the root tsconfig if one exists, otherwise a standalone ts-node invocation is not needed since Playwright 1.58 reads .ts config natively. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `@playwright/test` | Cypress | Cypress can't test Axum API in same session as kiosk UI. Not suitable. |
| `cargo nextest` | `cargo test` | Keep `cargo test --doc` for doctests (nextest doesn't support them). For all `#[test]` functions, nextest wins on isolation. |

**Installation:**
```bash
# From racecontrol repo root:

# 1. Install Playwright (root package.json devDependency)
npm install -D @playwright/test@1.58.2

# 2. Install Playwright's Chromium browser only (not all browsers — saves ~350MB)
npx playwright install chromium

# 3. Install cargo-nextest (one-time, adds to ~/.cargo/bin/)
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cargo install cargo-nextest --locked
```

**Version verification:** `@playwright/test` 1.58.2 verified current in STACK.md (npm registry checked 2026-03-19). cargo-nextest 0.9.x — install with `--locked` to pin the resolver; `cargo nextest --version` after install to confirm.

---

## Architecture Patterns

### Recommended Project Structure (Phase 41 scope only)

```
tests/e2e/
├── smoke.sh              (existing — refactor to source lib/common.sh)
├── game-launch.sh        (existing — refactor to source lib/common.sh + lib/pod-map.sh)
├── cross-process.sh      (existing — refactor to source lib/common.sh)
└── lib/                  (NEW — created in Phase 41)
    ├── common.sh         (pass/fail/skip/info helpers, counters, summary_exit)
    └── pod-map.sh        (pod IP lookup function for all 8 pods)

playwright.config.ts      (NEW — repo root, consumed by Phase 42+ specs)
.config/
└── nextest.toml          (NEW — workspace root, consumed by all Rust crate tests)
```

The `tests/e2e/playwright/` directory and `tests/e2e/lib/playwright.config.ts` (alternative location noted in ARCHITECTURE.md) are NOT created in Phase 41 — that structure is for Phase 42 when the first Playwright specs are written. Phase 41 only installs and configures Playwright at the repo root level.

### Pattern 1: Shell Shared Library (source-based)

**What:** All shell scripts source `lib/common.sh` at the top using `SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)` to compute the path portably. `common.sh` defines `pass`, `fail`, `skip`, `info`, color vars (tty-conditional), counters, and `summary_exit`.

**When to use:** Whenever a shell script needs to emit pass/fail results. Phase 42+ scripts source this file from day one.

**Example (from ARCHITECTURE.md, verified pattern):**
```bash
#!/bin/bash
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"

echo "=== My Test Section ==="
HTTP=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$BASE_URL/health")
[ "$HTTP" = "200" ] && pass "GET /health -> 200" || fail "GET /health -> $HTTP (expected 200)"

summary_exit   # prints totals, exits with $FAIL count
```

**Refactoring existing scripts:** The three existing scripts each define their own `pass()`/`fail()`/`skip()` inline (confirmed by reading smoke.sh and cross-process.sh directly). Replace those inline definitions with a single `source "$SCRIPT_DIR/lib/common.sh"` line. smoke.sh uses `set -euo pipefail` — keep this. game-launch.sh uses `set -uo pipefail` (no `-e` because it has conditional failure logic) — keep as-is.

### Pattern 2: Pod IP Map Function

**What:** `lib/pod-map.sh` defines a `pod_ip` function that takes a pod name (e.g., `pod-1`) and echoes its IP. All 8 IPs from MEMORY.md network map are encoded here as the single source of truth.

**When to use:** Any script that constructs a URL for a specific pod. Currently game-launch.sh uses a hardcoded `POD_IP` variable — replace with `POD_IP=$(pod_ip "$POD_ID")`.

**Example:**
```bash
#!/bin/bash
# lib/pod-map.sh
# Single source of truth for pod IP addresses.
# Usage: POD_IP=$(pod_ip pod-8)

pod_ip() {
  case "$1" in
    pod-1) echo "192.168.31.89" ;;
    pod-2) echo "192.168.31.33" ;;
    pod-3) echo "192.168.31.28" ;;
    pod-4) echo "192.168.31.88" ;;
    pod-5) echo "192.168.31.86" ;;
    pod-6) echo "192.168.31.87" ;;
    pod-7) echo "192.168.31.38" ;;
    pod-8) echo "192.168.31.91" ;;
    *)     echo ""; return 1 ;;
  esac
}
```

### Pattern 3: Playwright Config at Repo Root

**What:** `playwright.config.ts` at the repo root (`racecontrol/playwright.config.ts`). Uses `testDir: './tests/e2e/playwright'` pointing to where Phase 42 will create specs. The `webServer` block uses `reuseExistingServer: true` so Playwright attaches to the already-running venue kiosk rather than spawning a new one.

**When to use:** This file is inert until Phase 42 creates the first spec. Installing it in Phase 41 means Phase 42 has nothing to set up and can write the first spec immediately.

**Full config (from STACK.md — HIGH confidence, sourced from official Playwright docs):**
```typescript
// playwright.config.ts  (repo root)
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e/playwright',
  fullyParallel: false,            // MANDATORY: game launch tests mutate live pod state
  retries: process.env.CI ? 2 : 1,
  workers: 1,                      // MANDATORY: single worker, no parallel game launches
  reporter: [
    ['html', { open: 'never' }],
    ['junit', { outputFile: 'test-results/junit.xml' }],
  ],
  use: {
    baseURL: process.env.KIOSK_BASE_URL ?? 'http://192.168.31.23:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'off',
  },
  projects: [
    {
      name: 'chromium-kiosk',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npm run start',
    url: 'http://192.168.31.23:3300/kiosk',
    reuseExistingServer: true,     // MANDATORY: venue kiosk is already running
    timeout: 30_000,
    cwd: './kiosk',
  },
});
```

Note on `baseURL`: STACK.md uses `http://localhost:3300` as default for dev, but the venue runs kiosk on `192.168.31.23:3300` proxied through racecontrol at `:8080`. Using `RC_BASE_URL` or `KIOSK_BASE_URL` env vars keeps this configurable. The default should be the venue server IP so tests work without env setup when run on-site.

### Pattern 4: cargo-nextest Configuration

**What:** `.config/nextest.toml` at workspace root configures per-process isolation and retries for all Rust crate tests. nextest reads this file automatically from `{workspace-root}/.config/nextest.toml`.

**When to use:** After `cargo install cargo-nextest --locked`. This file makes `cargo nextest run` (with no extra flags) behave correctly for this project.

**Example:**
```toml
# .config/nextest.toml
[profile.default]
retries = { backoff = "fixed", count = 2, delay = "1s" }

[profile.default.junit]
path = "test-results/nextest.xml"
```

The `test-isolation = "process"` setting is nextest's default and does not need to be explicitly stated. Per-process isolation is on by default in nextest — each test binary runs in its own subprocess. This is what prevents billing state from leaking between Rust tests.

### Anti-Patterns to Avoid

- **Do not use `set -e` in lib/common.sh itself.** Scripts that source common.sh may have their own `set` options. The library should not impose `set -e` on its callers — some scripts (like game-launch.sh) intentionally omit `-e` to handle conditional failures. Set options belong in each top-level script, not in sourced libraries.
- **Do not hardcode `echo -e` color sequences in common.sh output without a TTY check.** smoke.sh already handles this correctly with `[ -t 1 ]`. common.sh must do the same — CI runs that capture output would otherwise contain raw ANSI escape codes in log files.
- **Do not place `playwright.config.ts` inside `tests/e2e/lib/`.** ARCHITECTURE.md shows it there in the full final structure, but STACK.md places it at repo root. Repo root placement is correct for Phase 41 — it is discovered automatically by `npx playwright test` without any `--config` flag. The `tests/e2e/lib/` placement documented in ARCHITECTURE.md is the Phase 44 final state when `run-all.sh` needs to specify `--config` explicitly.
- **Do not install Playwright in `kiosk/` (the Next.js app).** It belongs in the root `package.json` devDependencies. kiosk already has no test dependencies — do not add them there.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Browser automation | Custom puppeteer wrapper | `@playwright/test` | Playwright has built-in retry, trace, fixtures, and API request context. Puppeteer requires a separate runner and assertion library. |
| Test process isolation in Rust | Per-test `Arc<Mutex<>>` state management | cargo-nextest default behavior | nextest's per-process model isolates tests at the OS level — no shared global state possible. Custom synchronization is error-prone and doesn't protect against `static` variables. |
| Shell test output formatting | Custom color/counter per-script | `lib/common.sh` | Already proven by existing scripts — the duplication exists and causes inconsistency. One library eliminates the problem permanently. |
| Pod IP lookup | Hardcoded IP inline in each script | `lib/pod-map.sh` | Pod IPs are already duplicated across scripts. When an IP changes (the server already drifted from .51 to .23), every script that hardcodes it must be updated. One map, one change. |

---

## Common Pitfalls

### Pitfall 1: `summary_exit` exits with non-zero when FAIL=0 but SKIP>0

**What goes wrong:** A naive `exit $FAIL` works correctly, but if `summary_exit` is implemented as `exit $((FAIL + SKIP))` it will cause scripts with intentional skips to report failure.

**Why it happens:** Confusing "we skipped this test" (expected) with "this test failed" (unexpected).

**How to avoid:** `summary_exit` exits with `$FAIL` only, not `$FAIL + $SKIP`. SKIPs are informational — they do not indicate a problem.

**Warning signs:** A script that deliberately skips a test (e.g., "skip: no active billing session — cannot test launch") returns non-zero and blocks `run-all.sh`.

### Pitfall 2: `set -euo pipefail` in common.sh breaks caller scripts

**What goes wrong:** If `lib/common.sh` contains `set -euo pipefail`, every script that sources it inherits these flags, even scripts written to tolerate non-zero intermediate return codes.

**Why it happens:** `set` in a sourced file affects the current shell, not a subshell.

**How to avoid:** `lib/common.sh` must NOT contain any `set` options. Let each top-level script manage its own error handling.

### Pitfall 3: Playwright install targeting `kiosk/` instead of repo root

**What goes wrong:** Running `npm install -D @playwright/test` from `kiosk/` adds the dependency to `kiosk/package.json` and makes Playwright a dependency of the Next.js kiosk build. This bloats the kiosk bundle and complicates deploys.

**Why it happens:** `kiosk/` is the most active npm directory and may be the cwd when running commands.

**How to avoid:** Always run Playwright install from repo root (`C:/Users/bono/racingpoint/racecontrol/`). The root `package.json` already exists (confirmed) and accepts devDependencies.

### Pitfall 4: `npx playwright install` (all browsers) instead of `chromium` only

**What goes wrong:** Downloads ~500MB of Firefox and WebKit binaries that will never be used on this project.

**Why it happens:** `npx playwright install` with no argument downloads all browsers.

**How to avoid:** Always `npx playwright install chromium`. This downloads ~150MB Chrome for Testing. Document this in comments.

### Pitfall 5: cargo-nextest not in PATH when shell scripts call it

**What goes wrong:** `cargo nextest run` fails with "command not found" if PATH doesn't include `~/.cargo/bin`.

**Why it happens:** MEMORY.md confirms PATH must be explicitly set: `export PATH="$PATH:/c/Users/bono/.cargo/bin"`. Shell scripts that invoke nextest must include this export, or the caller environment must already have it.

**How to avoid:** Any script or Makefile target that calls `cargo nextest` must begin with `export PATH="$PATH:/c/Users/bono/.cargo/bin"`.

### Pitfall 6: SCRIPT_DIR resolution fails when script is called with relative path

**What goes wrong:** `SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)` works correctly when called as `bash tests/e2e/smoke.sh` from the repo root, but if called as `cd tests/e2e && bash smoke.sh`, `dirname "$0"` returns `.` and `cd .` gives the current directory, not the script's directory.

**Why it happens:** `$0` is the invocation path, not necessarily the script's actual path.

**How to avoid:** The `cd "$(dirname "$0")" && pwd` pattern is correct and handles both cases. Test by calling each refactored script both from repo root and from its own directory.

---

## Code Examples

Verified patterns from ARCHITECTURE.md (HIGH confidence — derived from existing codebase):

### lib/common.sh — complete structure
```bash
#!/bin/bash
# lib/common.sh
# Shared helpers for all RaceControl E2E test scripts.
# Source this file at the top of every .sh test script.
# DO NOT add 'set' options here — let callers manage their own error handling.

# Colors — only emit ANSI codes when stdout is a terminal
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    NC='\033[0m'
else
    GREEN='' RED='' YELLOW='' CYAN='' NC=''
fi

# Counters
PASS=0
FAIL=0
SKIP=0

pass() { PASS=$((PASS+1)); echo -e "  ${GREEN}PASS${NC}  $1"; }
fail() { FAIL=$((FAIL+1)); echo -e "  ${RED}FAIL${NC}  $1"; }
skip() { SKIP=$((SKIP+1)); echo -e "  ${YELLOW}SKIP${NC}  $1"; }
info() { echo -e "  ${CYAN}INFO${NC}  $1"; }

# Call as the last line of every test script.
# Prints a summary and exits with the number of failures.
summary_exit() {
    local total=$((PASS + FAIL + SKIP))
    echo ""
    echo "========================================"
    echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC} (${total} total)"
    echo "========================================"
    if [ "$FAIL" -gt 0 ]; then
        echo -e "${RED}FAILED${NC}"
    else
        echo -e "${GREEN}PASSED${NC}"
    fi
    exit "$FAIL"
}
```

### lib/pod-map.sh — pod IP lookup
```bash
#!/bin/bash
# lib/pod-map.sh
# Single source of truth for pod IP addresses.
# Source this file in any script that needs to reach a specific pod.
# Usage: POD_IP=$(pod_ip pod-8)

pod_ip() {
  case "$1" in
    pod-1) echo "192.168.31.89" ;;
    pod-2) echo "192.168.31.33" ;;
    pod-3) echo "192.168.31.28" ;;
    pod-4) echo "192.168.31.88" ;;
    pod-5) echo "192.168.31.86" ;;
    pod-6) echo "192.168.31.87" ;;
    pod-7) echo "192.168.31.38" ;;
    pod-8) echo "192.168.31.91" ;;
    *)
      echo "" >&2
      echo "ERROR: Unknown pod '$1'. Valid: pod-1 through pod-8." >&2
      return 1
      ;;
  esac
}
```

### Refactored script header (how existing scripts change)
```bash
#!/bin/bash
# (existing script header/comments remain unchanged)

set -uo pipefail   # keep each script's own set options

# Replace all inline pass/fail/skip/color definitions with:
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/pod-map.sh
source "$SCRIPT_DIR/lib/pod-map.sh"   # only if script uses pod IPs

# Rest of script body is unchanged.
# Replace final "exit $FAIL" or manual summary with:
summary_exit
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `cargo test` (built-in) | `cargo nextest` | 2024 — nextest reached stable | Per-process isolation, 3x faster, auto-retry. `cargo test` still needed for `--doc` tests. |
| Puppeteer + Jest | `@playwright/test` | 2022 — Playwright 1.20+ reached production quality | Single framework for browser + API tests. Built-in trace, retry, fixtures. No separate runner setup. |
| Inline pass/fail helpers per script | `source lib/common.sh` | This phase | Consistent output, one place to fix bugs, counters work across all scripts. |

**Deprecated/outdated:**
- `playwright` (base npm package): All current Playwright documentation assumes `@playwright/test`. The base package predates the test runner and requires writing your own runner. Do not install it.
- `msedge` channel in playwright.config.ts: Has a documented 30-second hang after each headed test (GitHub issue #22776). Use bundled Chromium.

---

## Open Questions

1. **Where exactly does game-launch.sh resolve the pod IP today?**
   - What we know: game-launch.sh uses `POD_ID=pod-8` and constructs URLs with it, but the Python dict for IP lookup was mentioned in ARCHITECTURE.md
   - What's unclear: The first 80 lines of game-launch.sh were read — the Python pod IP dict may be in the lower portion of the file
   - Recommendation: Read the full game-launch.sh before refactoring to ensure the pod IP extraction is correctly migrated to `pod_ip()` function call

2. **Should `playwright.config.ts` be placed at repo root or at `tests/e2e/lib/playwright.config.ts`?**
   - What we know: STACK.md places it at repo root; ARCHITECTURE.md shows it at `tests/e2e/lib/` for the final run-all.sh phase (Phase 44)
   - What's unclear: Phase 41 only installs it — Phase 44 wires it into run-all.sh with `--config` flag
   - Recommendation: Place at repo root in Phase 41. It is discoverable by default (`npx playwright test` from root). Phase 44 can add `--config tests/e2e/lib/playwright.config.ts` and move it if needed. Do not move preemptively.

3. **Does the root tsconfig.json need to include `playwright.config.ts`?**
   - What we know: There is no root tsconfig.json confirmed (kiosk has its own at `kiosk/tsconfig.json`); Playwright 1.58 reads `.ts` config files natively via its own TS transform
   - What's unclear: Whether a root `tsconfig.json` is required or whether Playwright's built-in TS support is sufficient
   - Recommendation: Playwright 1.58 ships its own TypeScript compilation for config files — no root tsconfig.json is needed. The config will compile correctly without one.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest 0.9.x (Rust); @playwright/test 1.58.2 (browser/TS) |
| Config file | `.config/nextest.toml` (Wave 0 gap — create in this phase); `playwright.config.ts` at repo root (Wave 0 gap — create in this phase) |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo nextest run --workspace` |
| Full suite command | `cargo nextest run --workspace && npx playwright test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FOUND-01 | `lib/common.sh` exists and defines pass/fail/skip/info/summary_exit | smoke: source and call each function, verify exit code | `bash -c 'source tests/e2e/lib/common.sh && pass "ok" && summary_exit'` exits 0 | ❌ Wave 0 |
| FOUND-01 | `summary_exit` exits with FAIL count, not SKIP count | unit: call fail once, verify exit 1; call skip once, verify exit 0 | manual verify during authoring | ❌ Wave 0 |
| FOUND-02 | `lib/pod-map.sh` defines all 8 pod IPs correctly | smoke: call pod_ip for each pod-N, compare to expected | `bash -c 'source tests/e2e/lib/pod-map.sh && [ "$(pod_ip pod-8)" = "192.168.31.91" ]'` | ❌ Wave 0 |
| FOUND-03 | `playwright.config.ts` exists at repo root | smoke: `npx playwright test --list` returns no errors | `npx playwright test --list 2>&1 \| grep -v "^$"` exits 0 | ❌ Wave 0 |
| FOUND-03 | Playwright Chromium browser is installed | smoke: `npx playwright install chromium` is idempotent | `npx playwright install chromium` (verify ~150MB download) | ❌ Wave 0 |
| FOUND-05 | cargo-nextest installed and runnable | smoke: `cargo nextest run --workspace` succeeds | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo nextest run --workspace` | ❌ Wave 0 |
| FOUND-05 | `.config/nextest.toml` exists with retries config | smoke: inspect file contents | file existence + `cargo nextest run --workspace` exits 0 | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `bash -c 'source tests/e2e/lib/common.sh && echo ok'` (common.sh syntax check) + `cargo check --workspace`
- **Per wave merge:** `cargo nextest run --workspace` + `npx playwright test --list`
- **Phase gate:** All per-wave commands green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/e2e/lib/common.sh` — covers FOUND-01
- [ ] `tests/e2e/lib/pod-map.sh` — covers FOUND-02
- [ ] `playwright.config.ts` (repo root) — covers FOUND-03
- [ ] `.config/nextest.toml` — covers FOUND-05
- [ ] Framework install: `npm install -D @playwright/test@1.58.2 && npx playwright install chromium` — FOUND-03
- [ ] Framework install: `cargo install cargo-nextest --locked` — FOUND-05

---

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` — Playwright 1.58.2 config, cargo-nextest install, version compatibility, full playwright.config.ts
- `.planning/research/ARCHITECTURE.md` — directory structure, lib/common.sh and lib/pod-map.sh patterns, build order, component boundaries
- `.planning/research/SUMMARY.md` — pitfall catalogue, feature priorities, phase rationale
- `.planning/REQUIREMENTS.md` — FOUND-01 through FOUND-05 requirement text, phase 41 scope boundary
- `.planning/STATE.md` — locked decisions: workers:1, fullyParallel:false, reuseExistingServer:true, Playwright 1.58.2 version lock
- `tests/e2e/smoke.sh` — read directly: inline pass/fail/skip functions to extract
- `tests/e2e/cross-process.sh` — read directly: inline pass/fail/skip functions to extract
- `tests/e2e/game-launch.sh` (lines 1–80) — read directly: inline pass/fail/skip confirmed; pod IP handling in lower portion (not read)
- `Cargo.toml` — read directly: workspace members, Rust toolchain context
- `.cargo/config.toml` — read directly: `+crt-static` flag confirmed, no nextest config present
- `kiosk/package.json` — read directly: no test deps, TypeScript 5.x present, no Playwright
- `package.json` (repo root) — read directly: exists, no test deps, `type: commonjs`, Node.js entry point for devDependencies
- Bash: `node --version` — v22.14.0 confirmed (above 20.x minimum for Playwright 1.58.2)
- Bash: `cargo nextest --version` — not installed confirmed

### Secondary (MEDIUM confidence)
- Official Playwright docs (playwright.dev) — config reference, webServer block, reuseExistingServer, bundled Chromium behavior — verified in STACK.md
- Official cargo-nextest docs (nexte.st) — per-process isolation, .config/nextest.toml location — verified in STACK.md

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified from npm registry and STACK.md which sourced official Playwright docs
- Architecture: HIGH — derived from reading existing codebase files directly, not from generic patterns
- Pitfalls: HIGH — pitfall 1–5 in this document derived from actual code read and known project constraints; pitfall 6 is standard POSIX bash behavior

**Research date:** 2026-03-19 IST
**Valid until:** 2026-04-19 (Playwright releases patch versions frequently but 1.58.2 is locked in STATE.md)
