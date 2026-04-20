---
phase: 445
slug: typed-api-contract-rust-ts-codegen
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 445 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Authoritative inputs: 445-RESEARCH.md § Validation Architecture, 445-CONTEXT.md D-15/D-16/D-17/D-20.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust) + `vitest` (TS contract tests at `packages/contract-tests/`) + `tsc --noEmit` (admin shape-check) |
| **Config file** | `packages/contract-tests/vitest.config.ts` (exists); `racingpoint-admin/tsconfig.json` (exists) |
| **Quick run command** | `cargo run --release --bin gen-types && cd packages/contract-tests && npm test -- --run regression-drift` |
| **Full suite command** | `bash scripts/check-generated-types-drift.sh && cd packages/contract-tests && npm test && cd ../../../racingpoint-admin && npx tsc --noEmit` |
| **Estimated runtime** | ~90s (gen-types ~30s + vitest ~10s + tsc ~45s) |

---

## Sampling Rate

- **After every task commit:** Run the quick run command (drift + regression-drift vitest case)
- **After every plan wave:** Run full suite (drift check + all contract tests + admin tsc)
- **Before `/gsd:verify-work`:** Full suite must be green AND `git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml` must pass
- **Max feedback latency:** 90 seconds per task

---

## Per-Task Verification Map

> Task IDs follow the `{phase}-{plan}-{wave}-{idx}` convention. The planner fills these after it produces PLAN.md files. The 6-plan structure from RESEARCH.md is seeded below; exact task IDs depend on planner output.

| Task ID (seed) | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 445-00-0-01 | 00 Safety audit | 0 | D-14 enum audit | unit | `cargo test -p racecontrol-crate enum_tagging_audit` | ❌ W0 | ⬜ pending |
| 445-00-0-02 | 00 Safety audit | 0 | D-07 admin-type whitelist | script | `bash scripts/enumerate-admin-types.sh` | ❌ W0 | ⬜ pending |
| 445-00-0-03 | 00 Safety audit | 0 | ts-rs spike | integration | `cargo run --release --bin gen-types -- --spike` | ❌ W0 | ⬜ pending |
| 445-00-0-04 | 00 Safety audit | 0 | Determinism 3× | script | `bash scripts/check-gen-types-determinism.sh` | ❌ W0 | ⬜ pending |
| 445-01-1-01 | 01 Deps + scaffolding | 1 | D-03 gen-types binary | unit | `cargo build --release --bin gen-types --features gen-types` | ❌ W0 | ⬜ pending |
| 445-01-1-02 | 01 Deps + scaffolding | 1 | feature flag | unit | `cargo build --no-default-features` (must still pass) | ✅ | ⬜ pending |
| 445-02a-2-01 | 02a rc-common derives | 2 | D-07 TS derives | unit | `cargo test -p rc-common derives_ts_for_admin_types` | ❌ W0 | ⬜ pending |
| 445-02b-2-01 | 02b utoipa annotations | 2 | 43 admin routes | integration | `cargo run --release --bin gen-types && grep -c 'operationId' docs/openapi.generated.yaml` (expect ≥ 43) | ❌ W0 | ⬜ pending |
| 445-03-3-01 | 03 Admin migration | 3 | D-08/D-09 re-exports | shape-check | `cd racingpoint-admin && npx tsc --noEmit` | ✅ | ⬜ pending |
| 445-03-3-02 | 03 Admin migration | 3 | D-12 drift audit | manual+script | `bash scripts/audit-handwritten-vs-generated.sh` | ❌ W0 | ⬜ pending |
| 445-04-4-01 | 04 CI gate + regression | 4 | D-15 drift check | script | `bash scripts/check-generated-types-drift.sh` (expect exit 0) | ❌ W0 | ⬜ pending |
| 445-04-4-02 | 04 CI gate + regression | 4 | D-20 negative fixture | vitest | `cd packages/contract-tests && npm test -- regression-drift` (expect the field-rename test to assert a FAIL) | ❌ W0 | ⬜ pending |
| 445-04-4-03 | 04 CI gate + regression | 4 | D-17 deploy manifest | script | `grep -q 'generated_types_freshness' scripts/deploy/deploy-audit.sh` | ✅ (after W4) | ⬜ pending |
| 445-05-5-01 | 05 SUMMARY | 5 | phase summary | manual | read SUMMARY.md | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/enumerate-admin-types.sh` — grep admin/`rcFetch|fetch` paths → Rust handlers → struct graph walk → produces `packages/shared-types/generated/.whitelist.txt`
- [ ] `scripts/check-gen-types-determinism.sh` — runs `gen-types` 3× back-to-back, diffs outputs byte-for-byte, fails if non-deterministic
- [ ] `scripts/check-generated-types-drift.sh` — runs `gen-types` → `git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml`
- [ ] `scripts/audit-handwritten-vs-generated.sh` — per hand-written `.ts` file in `src/`, diff structural shape against `generated/` equivalent; emits human-readable drift report
- [ ] `crates/racecontrol/src/bin/gen_types.rs` — skeleton binary (Wave 1 fills body)
- [ ] `packages/contract-tests/tests/regression-drift.test.ts` — negative fixture for D-20 (field rename on Rust side should fail admin tsc)
- [ ] Enum-tagging audit test in `crates/rc-common/tests/` that enumerates all `#[serde(tag, content)]` enums and asserts none are in the TS-derived whitelist
- [ ] Workspace-level `cargo test` passes with `--features ts-rs` and without (dual-write requirement)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Admin dashboard renders correctly post-migration | D-04 success | Visual/functional E2E not automatable in this phase | 1. Deploy admin to server .23 or Bono VPS; 2. Log in as superadmin; 3. Verify 5 key pages load without console errors: `/`, `/fleet`, `/customers`, `/sessions`, `/reports`; 4. Network tab: no 4xx on `rcFetch` calls; 5. Screenshot each page |
| CI drift check actually fails a PR | D-15 success | Requires opening a deliberate-drift PR | 1. Branch `test/drift-check-should-fail`; 2. Add a field to a Rust struct without regenerating; 3. Push; 4. Verify CI action exits non-zero with clear error |
| Dual-write rollback works | D-11 safety | Requires a live rollback drill | 1. On feature branch, revert the `src/index.ts` re-export commit; 2. `cd racingpoint-admin && npm ci && npm run build`; 3. Verify build succeeds using hand-written types |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (seeded above — planner refines)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (confirm after planner produces PLAN.md files)
- [ ] Wave 0 covers all MISSING references (8 items listed above)
- [ ] No watch-mode flags (all commands use `--run` for vitest, `--no-watch` for cargo)
- [ ] Feedback latency < 90s per task
- [ ] `nyquist_compliant: true` set in frontmatter (planner flips after PLAN.md files pass gsd-plan-checker)

**Approval:** pending
