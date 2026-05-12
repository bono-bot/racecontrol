# billing-files RCA — PR #54 test-only rebase-skew fix (2026-05-12)

**Class:** §S-146 RCA artifact for `pre-v2-edit-rca-check.js` hook compliance · surface = `billing-files` (glob `crates/racecontrol/src/billing*.rs`) · class = `billing`
**Scope:** test-only mechanical fix; no production logic change
**Parent RCA (full 5-section + MMA Step 1):** `/root/racecontrol/.planning/specs/v2/RCA/PR54-PACT-013-billing-paused-config-push-queue-20260511.md` (Captain auth 2026-05-11 ~12:49 IST)
**Parent mechanism-trust-check:** `/root/racecontrol/.planning/specs/v2/MECHANISM-TRUST/config-push-queue-2026-05-11.json`
**Parent MMA Step 1 DIAGNOSE:** `/root/racecontrol/.planning/specs/v2/RCA/MMA-PR54/` (5 model outputs + SUMMARY.json + pr54-comment.md)

## §1 Trigger

CI build failure on PR #54 head commit `7b88e709` (test: replacement integration test for billing_paused via config_push_queue). Failing job: `Test racecontrol`. Two test sites at `billing_session_lifecycle.rs:749` + `:855` call `AppState::new(config, pool, field_cipher)` with 3 args, but `state.rs:232` on `main` (post 87-commit merge into PR branch) requires 4 args: `(config: Config, db: SqlitePool, v2db: v2_db::DbPool, field_cipher: FieldCipher)`. The `v2db` parameter landed during the 87 commits the PR was behind when the new tests were authored.

## §2 Boundary map

Single file affected: `crates/racecontrol/src/billing_session_lifecycle.rs` lines 749 + 855. Both are inside `#[cfg(test)]` test bodies (not production code path).

## §3 Inherited-issue catalogue

V1↔V2 boundary review on test setup helper:
- `AppState::new` (line 232) takes the v2db parameter as part of the V2 Wave 1 DB substrate. This is V2 code, not V1↔V2 boundary.
- `AppState::new_with_test_v2db` (line 225) is the idiomatic test-side helper used in **9+ existing test sites** (`psychology_tests.rs`, `billing_tests.rs:2789`, `routes_tests.rs` ×7, `game_launcher_tests.rs:83`). This is V2-pattern-conformant.
- No V1-era code is touched by this fix.
- No schema, no protocol, no DB migration — pure test-helper signature alignment.

## §4 Past-bug review

Rebase-skew class: `ROOT-CAUSED-AND-FIXED` (by this RCA). The 4-arg AppState::new signature was the intended V2 design (introduced when v2db was added to state.rs). The test sites were authored on a stale rebase that didn't see the 4-arg signature. The fix uses the existing helper that exists specifically to handle test-side rebase-skew of this exact class.

## §5 V2-alignment delta

- **Should look like (V2 doctrine):** test sites use `AppState::new_with_test_v2db` for cases where the test doesn't need full v2db wiring. This helper exists at line 225 specifically to keep test boilerplate from coupling to evolving production AppState signature.
- **Current state:** 2 test sites use `AppState::new` instead of the helper.
- **Delta:** swap `AppState::new` → `AppState::new_with_test_v2db` at the 2 sites. Mechanical, idiomatic.

## §6 Proposed change (V2-framed)

```rust
// At billing_session_lifecycle.rs:749 and :855
- let state = Arc::new(AppState::new(config, pool, field_cipher));
+ let state = Arc::new(AppState::new_with_test_v2db(config, pool, field_cipher));
```

V2-alignment statement: change aligns the 2 lagging test sites with the 9+ existing test sites that already use the idiomatic V2-era helper. No regression risk — `new_with_test_v2db` internally constructs a test-mode v2db pool and delegates to `new`.

## §7 Composes-with

- **Parent PR #54 §S-146 5-section RCA** — full doctrine compliance already authored for the feature work
- **§S-186 pre-§S-146 small-fix fast-lane** — PR #54 created 2026-04-29 (pre-§S-146); this test fix is downstream of the full RCA already done, so even fast-lane scope doesn't apply
- **V2-LBAC v0.1** §3 closed-loop — this RCA satisfies Step 3 H1 PLAN precondition for the test fix

## §8 Verify-by

- `cargo test -p racecontrol-crate --lib set_billing_status_config_push_tests` runs clean post-fix (no E0061 errors)
- CI build on PR #54 head turns green
- No production behavior change (test-only)

## §9 NOT TESTED

- Whether other tests in racecontrol-crate also rely on stale `AppState::new` 3-arg signature (grep would catch but not exercised by this fix)
- Whether the v2db test-mode pool has hidden semantics that differ from production usage (treated as out-of-scope; parent PR #54 RCA + MMA Step 1 covers production)

— bono · 2026-05-12 ~07:35 IST · billing-files surface RCA artifact for §S-146 hook compliance · downstream of PR #54 parent RCA
