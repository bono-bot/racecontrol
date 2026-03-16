# Phase 35: Credits UI - Research

**Researched:** 2026-03-17
**Domain:** Frontend credits display — Rust GDI overlay + Next.js admin panel
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILLC-01 | User session cost displays in credits (1 cr = ₹1 = 100 paise) in overlay, kiosk, and admin — not rupees | format_cost() already outputs "X cr"; all three Next.js surfaces already use formatCredits; kiosk uses long-form "X credits". Grep confirms zero rupee strings in web/src and kiosk/src. |
| UIC-01 | Overlay format_cost() shows "X cr" instead of "Rs. X" (rc-agent overlay.rs) | format_cost() at line 1365 returns `format!("{} cr", paise / 100)`. Existing test_format_cost() covers 0/35000/67500/99/150 paise but does NOT include the required 4500 → "45 cr" assertion or a negative-assertion on "Rs." absence. One assertion must be added. |
| UIC-02 | Admin billing history page shows credits (replaces formatINR) | billing/history/page.tsx line 8 defines formatCredits; line 143 uses it for price column; line 174 for "Total Credits" summary. Grep confirms no rupee strings. Nothing to change. |
| UIC-03 | Admin pricing page includes Per-Minute Rates section with inline editing (replaces formatINR) | billing/pricing/page.tsx line 8 defines formatCredits; Per-Minute Rates table starts at line 361; inline edit wired to PUT /billing/rates/{id} with rate_per_min_paise: rateEditRate * 100. Grep confirms no rupee strings. Nothing to change. |
| UIC-04 | BillingStartModal shows credits (replaces formatINR) | BillingStartModal.tsx line 30 defines formatCredits; Price (credits) label at line 351. Variable named customPriceRupees (internal state name only, not displayed to user) at line 50 — label already reads "Price (credits)". Grep confirms no visible rupee strings. Nothing to change in user-facing output. |
</phase_requirements>

---

## Summary

Phase 35 is a near-complete phase. All five requirements have been pre-implemented during earlier phases. Every user-facing surface — the rc-agent GDI overlay, the admin billing history page, the admin pricing page, and BillingStartModal — already outputs "X cr" format using either the Rust `format_cost()` function or the TypeScript `formatCredits()` helper. The kiosk uses long-form "X credits" which is explicitly accepted per the UI-SPEC credit format contract.

A comprehensive grep across `web/src`, `kiosk/src`, and `crates/rc-agent/src` found zero instances of "Rs.", "₹", or "formatINR" in any user-visible output. The only grep hits are: (1) an internal React state variable named `customPriceRupees` in BillingStartModal (not rendered to DOM), (2) an internal Rust variable `balance_rupees` in lock_screen.rs (computed but never interpolated into any HTML template — it is a dead variable), and (3) `balance_rupees` in lock_screen.rs is in a between-sessions interstitial page that does not display cost at all.

The sole deliverable requiring code change is adding two assertions to the existing `test_format_cost()` function in overlay.rs: `format_cost(4500) == "45 cr"` (the exact value cited in the UIC-01 success criterion) and a negative assertion that the output does not contain "Rs." or "₹". All other success criteria are satisfied by running grep and confirming zero matches.

**Primary recommendation:** One task — add two assertions to the existing `test_format_cost()` test, then run the four grep verification checks. No new files, no new functions, no frontend changes.

---

## Standard Stack

### Core

| Layer | Tool | Version | Purpose |
|-------|------|---------|---------|
| Rust overlay formatting | `format!()` macro in overlay.rs | rustc 1.93.1 | `format_cost(paise: i64) -> String` — GDI text |
| TypeScript credits helper | File-local `formatCredits` lambda | — | `(paise: number) => \`${Math.floor(paise / 100)} cr\`` |
| Test runner (Rust) | `cargo test` | 1.93.1 | `#[cfg(test)]` block in overlay.rs |
| Test runner (TS) | None required | — | No TS tests needed — grep is the verification |

### Supporting

| Tool | Purpose |
|------|---------|
| `grep -r` / Grep tool | Verify zero rupee strings across all source trees |
| `cargo test -p rc-agent-crate` | Run overlay unit tests including format_cost |

### Alternatives Considered

None applicable — this is not a library selection problem. The existing inline helpers are the correct solution per STATE.md decision: "Phase 35 is a pure frontend pass — no Rust changes expected" (the one Rust change is a test addition, not a production code change).

---

## Architecture Patterns

### Current formatCredits Pattern (TypeScript)

Each file that needs credit display defines its own local helper. This is intentional — do not refactor into a shared util during Phase 35.

```typescript
// Source: web/src/app/billing/history/page.tsx line 8
//         web/src/app/billing/pricing/page.tsx line 8
//         web/src/components/BillingStartModal.tsx line 30
const formatCredits = (paise: number) => `${Math.floor(paise / 100)} cr`;
```

### Current format_cost Pattern (Rust)

```rust
// Source: crates/rc-agent/src/overlay.rs line 1365
fn format_cost(paise: i64) -> String {
    format!("{} cr", paise / 100)
}
```

Rust integer division is already floor for positive paise values — matches `Math.floor` in TypeScript.

### format_cost Test Block (existing)

```rust
// Source: crates/rc-agent/src/overlay.rs line 1526-1533
#[test]
fn test_format_cost() {
    assert_eq!(format_cost(0), "0 cr");
    assert_eq!(format_cost(35000), "350 cr");
    assert_eq!(format_cost(67500), "675 cr");
    assert_eq!(format_cost(99), "0 cr");   // floor division
    assert_eq!(format_cost(150), "1 cr");
}
```

Required addition (two assertions, same test function):

```rust
assert_eq!(format_cost(4500), "45 cr");          // UIC-01 exact criterion
assert!(!format_cost(4500).contains("Rs."));      // no rupee string
assert!(!format_cost(4500).contains('\u{20B9}')); // no ₹ symbol
```

### Anti-Patterns to Avoid

- **Adding a new test function:** The spec says "confirmed by unit test on the format function" — add to the existing `test_format_cost()`, not a separate test. Keeps the function as the single authoritative test for format_cost behavior.
- **Renaming customPriceRupees:** It is a private React state variable with no user-visible output. Renaming it would be an unnecessary refactor outside Phase 35 scope.
- **Touching lock_screen.rs:** The `balance_rupees` variable in lock_screen.rs is computed but never interpolated into any HTML. It does not appear in any user-visible output. Leave it alone — it is dead code, not a rupee display bug.
- **Refactoring formatCredits into a shared util:** Explicitly deferred per UI-SPEC line 129.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Credits verification | Custom test harness | `cargo test -p rc-agent-crate` | Already wired, runs in <30s |
| Rupee string scan | Manual file review | `grep -r "Rs\.\|₹\|formatINR"` | Exhaustive, reproducible, fast |

---

## Common Pitfalls

### Pitfall 1: Test Already Exists — Wrong Test Gets Added

**What goes wrong:** Adding a new test function `test_format_cost_credits()` while the existing `test_format_cost()` is present. Both pass but UIC-01 success criterion reads "confirmed by unit test on the format function" — singular. Two tests for one function is not wrong, but the plan should explicitly target the existing function to avoid planner ambiguity.

**How to avoid:** PLAN.md action must say "add assertions to the existing `test_format_cost()` function at line 1527."

### Pitfall 2: Grep Misses the Unicode ₹ Character

**What goes wrong:** Running `grep -r "Rs\." ...` only catches ASCII "Rs." but not the Unicode ₹ (U+20B9) character. A file could contain ₹ without triggering the grep.

**How to avoid:** Run two separate grep patterns or one combined: `grep -r "Rs\.\|₹\|formatINR" web/src kiosk/src crates/`. The ₹ glyph must be in the grep pattern explicitly.

**Current state:** Current grep already confirmed zero matches for both patterns across all three source trees.

### Pitfall 3: BILLC-01 Scope Confusion

**What goes wrong:** BILLC-01 says "overlay, kiosk, and admin" — planner might think the kiosk needs "X cr" (short form). It uses long-form "X credits" throughout.

**How to avoid:** Per the UI-SPEC credit format contract: kiosk uses "X credits" (long form) and this is explicitly acceptable. BILLC-01 is satisfied as long as rupee symbols are absent — the format "X credits" is valid for kiosk context.

### Pitfall 4: Treating balance_rupees as a Bug

**What goes wrong:** Grep finds `balance_rupees` in lock_screen.rs and the planner adds a task to fix it.

**How to avoid:** `balance_rupees` at lock_screen.rs line 1019 is computed in `render_between_sessions_page()` but never used in any `format!()` argument — it is a dead variable. The HTML template for that page shows remaining races, not cost. It does not output rupees to any user-facing surface. It is not in scope for Phase 35.

---

## Code Examples

### Adding the Required UIC-01 Assertions

```rust
// Source: crates/rc-agent/src/overlay.rs
// Existing test at line 1527 — add these three lines inside the function body

assert_eq!(format_cost(4500), "45 cr");              // UIC-01: exact success criterion
assert!(!format_cost(4500).contains("Rs."));          // UIC-01: no ASCII rupee prefix
assert!(!format_cost(4500).contains('\u{20B9}'));     // UIC-01: no Unicode ₹ symbol
```

### Verification Grep Commands

```bash
# Run from repo root — must return zero matches for all four success criteria
grep -r "Rs\." web/src kiosk/src crates/rc-agent/src
grep -r "₹" web/src kiosk/src crates/rc-agent/src
grep -r "formatINR" web/src kiosk/src crates/rc-agent/src
```

### Test Run Command

```bash
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cargo test -p rc-agent-crate test_format_cost -- --nocapture
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `formatINR(paise)` → "Rs. X" | `formatCredits(paise)` → "X cr" | Pre-Phase 35 (already done) | All admin/modal surfaces already use credits |
| `format!("Rs. {}", paise/100)` in overlay | `format!("{} cr", paise / 100)` | Pre-Phase 35 (already done) | GDI overlay already shows credits |

No state-of-the-art changes required in Phase 35. The phase is essentially a verification and test pass.

---

## Open Questions

1. **Dead variable `balance_rupees` in lock_screen.rs**
   - What we know: Computed at line 1019 but never interpolated into the HTML template. The `render_between_sessions_page()` function displays remaining races only — no cost.
   - What's unclear: Whether it was intentionally left for a future feature (e.g., showing wallet balance on the between-sessions screen).
   - Recommendation: Leave it alone for Phase 35. It is not a user-visible rupee display. If a future phase adds wallet balance to that screen, it can be renamed then.

2. **Verify UIC-03 inline edit saves correctly**
   - What we know: `rateEditRate * 100` converts credits → paise before the API call. The `PUT /billing/rates/{id}` endpoint from Phase 34 accepts `rate_per_min_paise`.
   - What's unclear: Whether Phase 34's PUT endpoint is actually returning 200 with the updated row (required by UIC-03 success criterion "staff can change a rate and save without leaving the page").
   - Recommendation: The grep verification for UIC-03 is "no rupee strings" only. The inline edit functional test is covered by Phase 34 integration tests (ADMIN-03). Phase 35 does not need to re-test the API layer.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[cfg(test)]` + `#[test]`) |
| Config file | none — workspace-level `Cargo.toml` |
| Quick run command | `cargo test -p rc-agent-crate test_format_cost` |
| Full suite command | `cargo test -p rc-agent-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| UIC-01 | format_cost(4500) == "45 cr", no "Rs." in output | unit | `cargo test -p rc-agent-crate test_format_cost` | Exists — add 3 assertions |
| UIC-02 | Zero formatINR/₹/Rs. in billing/history/page.tsx | grep/smoke | `grep -r "Rs\.\|₹\|formatINR" web/src/app/billing/history/` | Verified manually — no test file needed |
| UIC-03 | Zero rupee strings in billing/pricing/page.tsx, Per-Minute Rates table present | grep/smoke | `grep -r "Rs\.\|₹\|formatINR" web/src/app/billing/pricing/` | Verified manually — no test file needed |
| UIC-04 | Zero rupee strings in BillingStartModal.tsx | grep/smoke | `grep -r "Rs\.\|₹\|formatINR" web/src/components/BillingStartModal.tsx` | Verified manually — no test file needed |
| BILLC-01 | Full booking flow — no rupee strings in any surface | grep/smoke | `grep -r "Rs\.\|₹\|formatINR" web/src kiosk/src crates/rc-agent/src` | Verified manually — no test file needed |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent-crate test_format_cost`
- **Per wave merge:** `cargo test -p rc-agent-crate`
- **Phase gate:** Full suite green + grep returns zero matches before `/gsd:verify-work`

### Wave 0 Gaps

None — existing test infrastructure covers all phase requirements. The only code change is adding 3 lines to an existing test function. No new test files, no new framework config, no new fixtures needed.

---

## Sources

### Primary (HIGH confidence)

- Direct code read: `crates/rc-agent/src/overlay.rs` — format_cost() at line 1365, test_format_cost() at line 1527
- Direct code read: `web/src/app/billing/history/page.tsx` — formatCredits at line 8, usage at line 143 and 174
- Direct code read: `web/src/app/billing/pricing/page.tsx` — formatCredits at line 8, Per-Minute Rates table at line 361, inline edit PUT call
- Direct code read: `web/src/components/BillingStartModal.tsx` — formatCredits at line 30, Price (credits) label at line 351
- Direct grep: `grep -r "Rs\.\|₹\|formatINR" web/src kiosk/src crates/rc-agent/src` — zero matches confirmed
- Direct code read: `crates/rc-agent/src/lock_screen.rs` — balance_rupees at line 1019 confirmed as dead variable (never interpolated into HTML)
- `.planning/phases/35-credits-ui/35-UI-SPEC.md` — UI design contract and component inventory

### Secondary (MEDIUM confidence)

- `.planning/REQUIREMENTS.md` — UIC-01 through UIC-04 and BILLC-01 definitions with success criteria
- `.planning/ROADMAP.md` — Phase 35 goal and success criteria
- `.planning/STATE.md` — Decision log: "Phase 35 is a pure frontend pass — no Rust changes expected"

---

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — code read directly, no inference
- Architecture: HIGH — format_cost() and formatCredits() inspected at source; grep verified across all trees
- Pitfalls: HIGH — each pitfall derived from direct observation of the codebase (dead variable, existing test, unicode grep gap)

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable implementation; no external dependencies)
