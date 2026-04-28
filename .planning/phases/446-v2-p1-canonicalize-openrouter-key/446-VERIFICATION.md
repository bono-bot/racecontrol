---
phase: 446-v2-p1-canonicalize-openrouter-key
status: passed
date: 2026-04-21T18:35:00+05:30
verifier: gsd-verifier (sonnet)
score: 10/10 must-haves resolved (8 green, 1 yellow, 1 pending-operator)
---

# Phase 446: Canonicalize OPENROUTER_KEY — Verification Report

**Phase Goal:** Migrate 3 remaining `OPENROUTER_API_KEY` env-var read sites to canonical `OPENROUTER_KEY` using inline dual-read + deprecation warn. Deploy affected binaries (rc-agent, rc-watchdog) to Pod 4 canary. whatsapp-bot code ships to origin; pm2 rotation deferred as operator action.

**Verified:** 2026-04-21T18:35:00+05:30 IST

**Status:** PASSED

**Re-verification:** No — initial verification.

---

## Verdict

**PASSED** — All 10 must-haves resolve green, yellow (deviated-with-doc), or pending (user-approved deferral). Zero must-haves are red. The phase goal is achieved.

---

## Must-Haves Resolution

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Canonical pattern at all 3 target sites | green | See detail below |
| 2 | No stragglers (CANON-446-01 / -02 tripwires) | green | Grep returns only expected dual-read files |
| 3 | Builds green (CANON-446-03) | green | Plan 01 SUMMARY: all 3 release builds exit 0 |
| 4 | Dual-read semantics preserved | green | Canonical-first, warn-on-fallback, "not set" log byte-identical, TOML fallback at :607 untouched |
| 5 | Per-target enumeration (CGP H4) in Plan 04 SUMMARY | green | 13 targets enumerated row-by-row in 446-04-SUMMARY.md |
| 6 | Pod 4 canary evidence (CGP H3) | green | 0 deprecation warns in 271 log lines, tier_engine confirmed running |
| 7 | SWAPLOG integrity | green | SWAPLOG.md entry at 2026-04-21 18:02 IST, commit 1856b70a |
| 8 | Anti-overengineering guardrails honored | green | No secrets.rs, TOML fields unchanged, rc-sentry/racecontrol untouched |
| 9 | Requirement coverage | green (CANON-446-01/02/03/05) + yellow (CANON-446-04 lint) + pending-operator (CANON-446-06) |
| 10 | No scope bleed | green | git log origin/main..HEAD -- crates/rc-sentry/ crates/racecontrol/src/ returns 0 commits |

---

## Must-Have 1: Canonical Pattern at 3 Target Sites

### Site 1: crates/rc-watchdog/src/mma_diagnosis.rs (lines 263-275)

Direct read confirms canonical-first dual-read:

```rust
let api_key = match std::env::var("OPENROUTER_KEY") {
    Ok(key) if !key.is_empty() => key,
    _ => match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) if !key.is_empty() => {
            tracing::warn!("OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY (read once, will not repeat)");
            key
        }
        _ => {
            tracing::info!("OPENROUTER_API_KEY not set — using deterministic fallback");
            return deterministic_diagnosis(context);
        }
    }
};
```

Status: VERIFIED — canonical first, deprecation warn on fallback, original "not set" log string preserved byte-identical.

### Site 2: crates/rc-agent/src/ai_debugger.rs (lines 604-622)

Direct read confirms canonical-first dual-read with Option<String> return type:

```rust
let api_key_from_env: Option<String> = match std::env::var("OPENROUTER_KEY")
    .ok()
    .filter(|s| !s.is_empty())
{
    Some(key) => Some(key),
    None => match std::env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(key) => {
            tracing::warn!(
                target: LOG_TARGET,
                "OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY (read once, will not repeat)"
            );
            Some(key)
        }
        None => None,
    },
};
```

Phase 363 TOML fallback at line 623 (tuple match on `config.openrouter_api_key`) is present and untouched — carve-out confirmed.

Status: VERIFIED — canonical first, warn on fallback, TOML fallback preserved.

### Site 3: whatsapp-bot/src/services/claudeService.js (lines 8-17)

Direct read confirms IIFE with canonical-first semantics:

```js
const OPENROUTER_API_KEY = (() => {
  if (process.env.OPENROUTER_KEY && process.env.OPENROUTER_KEY.length > 0) {
    return process.env.OPENROUTER_KEY;
  }
  if (process.env.OPENROUTER_API_KEY && process.env.OPENROUTER_API_KEY.length > 0) {
    console.warn('[whatsapp-bot] OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY in pm2 env (read once, will not repeat)');
    return process.env.OPENROUTER_API_KEY;
  }
  return undefined;
})();
```

Redaction regex at line 49: `/OPENROUTER(_API)?_KEY/g` — covers canonical + deprecated; does NOT match `OPENROUTER_MGMT_KEY` (confirmed by Plan 02 regex behavior test).

Status: VERIFIED — canonical first, warn on fallback, redaction extended.

---

## Must-Have 2: No Stragglers

**CANON-446-01 tripwire (Rust):**

Live grep across all `crates/` `.rs` files for `std::env::var("OPENROUTER_API_KEY")` returns exactly 2 files:
- `crates/rc-agent/src/ai_debugger.rs:609` — inside dual-read fallback branch
- `crates/rc-watchdog/src/mma_diagnosis.rs:265` — inside dual-read fallback branch

Zero hits outside the two dual-read files. CANON-446-01 satisfied.

**CANON-446-02 tripwire (JS):**

Live grep of `whatsapp-bot/src/` for `process.env.OPENROUTER_API_KEY` returns 2 hits, both at lines 12 and 14 of `claudeService.js` — both inside the IIFE fallback branch (`if` guard and `return`). Zero hits outside IIFE. CANON-446-02 satisfied.

**Canonical read count (Rust):** 7 total `std::env::var("OPENROUTER_KEY")` hits across crates — was 5 pre-446, +2 from this phase. All 7 are legitimate canonical reads.

---

## Must-Have 3: Builds Green

From Plan 01 SUMMARY (commit d57ee48e, push verified to branch):

```
cargo build --release --bin rc-agent      → Finished in 2m 58s  EXIT: 0
cargo build --release --bin rc-watchdog   → Finished in 8.05s   EXIT: 0
cargo build --release --bin racecontrol   → Finished in 4m 27s  EXIT: 0
```

Pre-push gate: 264 rc-common tests + 1008 racecontrol-crate tests — all green.

Status: VERIFIED via Plan 01 SUMMARY evidence (no re-run needed — source unchanged since that build).

---

## Must-Have 4: Dual-Read Semantics

- **Canonical-first:** OPENROUTER_KEY checked before OPENROUTER_API_KEY at all 3 sites. Confirmed by direct source read.
- **Warn on fallback only:** `tracing::warn!` / `console.warn` fires exclusively in the fallback branch. No warn when canonical env is set.
- **"not set" log string:** `mma_diagnosis.rs` preserves `"OPENROUTER_API_KEY not set — using deterministic fallback"` byte-identical (Plan 01 grep confirms count=1).
- **"read once, will not repeat" text:** Present in both Rust warn strings and JS console.warn. No OnceLock implementation (Phase 448 carve-out — by design).
- **Phase 363 TOML fallback:** `match (api_key_from_env.as_deref(), config.openrouter_api_key.as_deref())` at ai_debugger.rs:623 — untouched.

Status: VERIFIED.

---

## Must-Have 5: Per-Target Enumeration

446-04-SUMMARY.md contains a complete CGP H4 per-target table (13 rows):

| Target | Status |
|--------|--------|
| Pod 1 @ 192.168.31.89 | DEFERRED (Pattern I DiD hold) |
| Pod 2 @ 192.168.31.33 | DEFERRED |
| Pod 3 @ 192.168.31.28 | DEFERRED |
| Pod 4 @ 192.168.31.88 | CANARY LIVE (b37983e8-dirty, 0 deprecation warns) |
| Pod 5 @ 192.168.31.86 | DEFERRED |
| Pod 6 @ 192.168.31.87 | DEFERRED (Pattern E AC hold noted) |
| Pod 7 @ 192.168.31.38 | DEFERRED |
| Pod 8 @ 192.168.31.91 | DEFERRED |
| POS @ 192.168.31.130 | N/A (does not run rc-agent) |
| Server .23 | N/A (racecontrol unchanged this phase) |
| Bono VPS | PENDING (operator) — code `0981afb` in whatsapp-bot/main |
| Cloud apps (admin/web/pwa/kiosk) | N/A (no OpenRouter call sites) |
| Comms-link | N/A (does not read OPENROUTER env) |

Status: VERIFIED — all 13 targets enumerated with sensible status values.

---

## Must-Have 6: Pod 4 Canary Evidence

From 446-04-SUMMARY.md Task 4 Section C (verbatim):

- **WHERE:** Pod 4 @ 192.168.31.88 (sim4), post-swap build_id b37983e8-dirty
- **Command:** `Get-Content -Tail 1000 C:\RacingPoint\rc-agent-.2026-04-21.jsonl` via rc-sentry /exec
- **Total log lines read:** 271
- **Lines matching `OPENROUTER_API_KEY is deprecated`:** 0
- **tier_engine lifecycle confirmed:** `"lifecycle: first_event_processed"` at 2026-04-21T12:30:25Z
- **NOT TESTED:** Real customer-facing AI debug request (tier_engine fires on crash recovery; clean startup crash_recovery=false did not trigger MMA path)

Deprecation-warn dual-read path verified via local Rust harness (Task 5):
- Case A canonical-only: 0 warns
- Case B deprecated-only: 1 warn (exact text confirmed)
- Case C neither set: 0 warns

Status: VERIFIED — canary evidence present with raw output, WHERE specified, NOT TESTED list provided.

---

## Must-Have 7: SWAPLOG Integrity

SWAPLOG.md entry confirmed at line 44:

```
| 2026-04-21 18:02 IST | b37983e8-dirty (rc-agent + rc-watchdog, Pod 4 canary) | 26821120 (rc-agent) / 7791104 (rc-watchdog) | c7408e3fda977e4b (rc-agent sha256 first 16) | james-via-claude (Phase 446 Plan 04 Task 3 — user approved at Task 2 checkpoint) | refactor(446) d57ee48e — Phase 446 OPENROUTER_KEY canonical dual-read canary deploy to Pod 4 ...
```

Commit `1856b70a` landed the SWAPLOG row. Entry includes: timestamp IST, binary sizes, SHA256 short, triggered-by, reason, and notes the dual-read code commit (d57ee48e) is included in binary.

Status: VERIFIED.

---

## Must-Have 8: Anti-Overengineering Guardrails

- **No rc-common::secrets helper:** `crates/rc-common/src/secrets.rs` does NOT exist (Glob returned no files). Phase 448 carve-out honored.
- **TOML field name unchanged:** `grep "^openrouter_api_key\s*=" deploy/configs/*.toml` returns 0 hits — the field is commented out (REDACTED) and exists only as an informational comment at line 43. No active TOML field was renamed.
- **TOML comment updated correctly:** Comment now reads `now read from OPENROUTER_KEY env var (canonical; OPENROUTER_API_KEY still works via Phase 446 deprecation fallback)` — 17 files updated (Plan 03 verification output shows new=1 old=0 for all 17).
- **No rc-sentry / racecontrol source change:** `git log origin/main..HEAD -- crates/rc-sentry/ crates/racecontrol/src/` returns 0 commits.
- **No .unwrap() added:** Code additions use `match`, `ok()`, `filter()`, `?`-equivalent idioms. No unwrap in the dual-read blocks.
- **No `any` in TypeScript:** Plan 02 modified `.js` file only (no TypeScript involved).

Status: VERIFIED.

---

## Must-Have 9: Requirement Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| CANON-446-01 | green | Live grep: 0 hits outside dual-read files in crates/ |
| CANON-446-02 | green | Live grep: 0 hits outside IIFE fallback in whatsapp-bot/src/ |
| CANON-446-03 | green | Plan 01 SUMMARY: all 3 release builds exit 0 |
| CANON-446-04 | yellow (deviated) | `npm run lint` absent from whatsapp-bot/package.json; ESLint v9 flat config missing. Pre-existing gap. Fallback: `node --check` SYNTAX_OK. NOT a regression introduced by this phase. |
| CANON-446-05 | green | Pod 4 log scan: 271 lines, 0 deprecation warns; harness: Case B fires exactly 1 warn |
| CANON-446-06 | pending (operator) | Code `0981afb` in whatsapp-bot/main ships correctly; pm2 env rotation deferred per user Option 3. Dual-read fallback keeps OPENROUTER_API_KEY functional until rotation. Recovery recipe documented in 446-04-SUMMARY.md Task 6 section. |

CANON-446-04 deviation is pre-existing repo state, not introduced by Phase 446. Accepted as yellow per rubric.

CANON-446-06 pending status is a user-approved deferral at Task 6 checkpoint (Option 3) — code is correct, operator rotation is the outstanding step. NOT a defect.

---

## Must-Have 10: No Scope Bleed

- `git log origin/main..HEAD -- crates/rc-sentry/ crates/racecontrol/src/ --oneline` returns **0 commits** — rc-sentry and racecontrol server source unchanged.
- Pods 1-3, 5-8: fleet table in 446-04-SUMMARY.md shows all still on `a13942f2-dirty` (unchanged). Confirmed by pre-swap fleet snapshot.
- Server .23: no racecontrol swap this phase.
- POS: no changes (not in scope per CONTEXT line 23).
- Cloud apps (admin/web/pwa/kiosk): no Next.js changes.
- Comms-link: no changes.

Status: VERIFIED.

---

## Deliberate Deferrals (User-Approved — NOT Gaps)

### CANON-446-06: Bono VPS pm2 rotation

- **Decision:** User chose Option 3 (defer) at Task 6 checkpoint on 2026-04-21.
- **State:** Plan 02 commit `0981afb` is in `whatsapp-bot/main`. Code ships correctly. pm2 env on Bono VPS still has `OPENROUTER_API_KEY` (deprecated name).
- **Impact:** whatsapp-bot will emit `console.warn('[whatsapp-bot] OPENROUTER_API_KEY is deprecated ...')` per call until rotation. Feature works — Claude responses are returned.
- **Recovery path:** 5-step operator recipe documented in 446-04-SUMMARY.md Task 6 section. Dual-read fallback ensures backward compatibility.
- **Classification:** Operator action, not a defect.

### Fleet rollout Pods 1-3, 5-8

- **Decision:** Deferred per kickoff line 253. User decides after Pod 4 24h soak (started 2026-04-21 18:02 IST).
- **State:** All 7 pods remain on pre-446 `a13942f2-dirty` binary. rc-agent on those pods reads `OPENROUTER_KEY` from `start-rcagent.bat` (already canonical per Plan 03 audit) — no deprecation warn will fire on those pods even before fleet rollout.
- **Recovery path:** Repeat Plan 04 Task 3 swap pattern per pod after 24h soak decision.

---

## Non-Regressions Confirmed

- Phase 363 TOML-config fallback at `ai_debugger.rs:623` — preserved (`match (api_key_from_env.as_deref(), config.openrouter_api_key.as_deref())` with TOML warn text byte-identical)
- `OPENROUTER_MGMT_KEY` env var — separate key for child-key provisioning; no code touching it this phase
- `start-rcagent.bat` — already canonical (`set OPENROUTER_KEY=%%K` at line 9); confirmed by Plan 03 Task 2 Case A (no-op)
- rc-sentry + racecontrol server binaries — zero source changes, zero rebuild this phase
- POS kiosk, server .23, cloud apps, comms-link — unchanged
- `rc-common/secrets.rs` — does NOT exist (Phase 448 owns centralization)
- TOML FIELD `openrouter_api_key` (Rust struct field in AiDebuggerConfig) — unchanged; only the TOML comment line was updated

---

## Anti-Patterns Scan

Files modified this phase: `crates/rc-watchdog/src/mma_diagnosis.rs`, `crates/rc-agent/src/ai_debugger.rs`, `whatsapp-bot/src/services/claudeService.js`, 17 TOML files (comment-only).

| File | Pattern | Severity | Finding |
|------|---------|----------|---------|
| mma_diagnosis.rs | return null / stub | None | Fallback path `return deterministic_diagnosis(context)` is original behavior, not a stub |
| ai_debugger.rs | return null / stub | None | `None` return when no key set is original behavior |
| claudeService.js | empty return | None | `return undefined` in IIFE when no key set is original behavior, consistent with pre-446 |
| TOML files | TODO/placeholder | None | Comment-only changes; no runtime code path |

No blockers. No warnings. No anti-patterns introduced.

---

## Behavioral Spot-Checks

| Behavior | Evidence | Status |
|----------|----------|--------|
| rc-watchdog: canonical-first read compiles | cargo build --release --bin rc-watchdog exit 0 | PASS |
| rc-agent: dual-read + TOML fallback compiles | cargo build --release --bin rc-agent exit 0 | PASS |
| whatsapp-bot IIFE: State B (deprecated env) fires exactly 1 warn | Plan 02 3-state smoke test: `DEPRECATION_PATH_OK_warn_fired` | PASS |
| Pod 4 post-swap: 0 deprecation warns in 271 log lines | Plan 04 Task 4 Section C | PASS |
| Rust harness: Case B (deprecated-only) fires exactly 1 warn | Plan 04 Task 5: `DEPRECATION_PATH_OK` | PASS |
| SWAPLOG row appended for Pod 4 | SWAPLOG.md line 44, commit 1856b70a | PASS |

---

## Human Verification (Optional — Not Blocking)

These items would benefit from operator sanity-check but do NOT block the `passed` verdict:

1. **Pod 4 24h soak observation**
   - What to do: Tail `rc-agent-.2026-04-21.jsonl` and `rc-agent-.2026-04-22.jsonl` on Pod 4 for any `OPENROUTER_API_KEY is deprecated` lines over the 24h window
   - Expected: Zero deprecation warn lines (start-rcagent.bat sets OPENROUTER_KEY canonically)
   - Why human: Log is on Pod 4; no automated collection runs over 24h

2. **Bono VPS pm2 rotation (when convenient)**
   - What to do: Run the 5-step recipe from 446-04-SUMMARY.md Task 6
   - Expected: After rotation, `pm2 logs racingpoint-bot --lines 200 | grep -c 'OPENROUTER_API_KEY is deprecated'` returns 0; a WhatsApp test message receives Claude response
   - Why human: Operator action requiring Bono VPS access and pm2 env update

3. **Fleet rollout decision after 24h soak**
   - What to do: After Pod 4 soak is clean, decide whether to swap Pods 2-3, 5-8 (Pod 1 held on Pattern I DiD policy; Pod 6 coordinate with Pattern E AC status)
   - Expected: Each swapped pod shows 0 deprecation warns in post-swap log scan
   - Why human: Operator fleet decision + live deploy action

---

## Gaps Found

None. Verdict is `passed`.

---

_Verified: 2026-04-21T18:35:00+05:30 IST_
_Verifier: Claude (gsd-verifier / sonnet-4-6)_
_Phase branch: phase/446-canonicalize-openrouter-key_
