---
phase: 176-protocol-foundation-cargo-gates
plan: "03"
subsystem: ci-and-documentation
tags: [ci, cargo-features, single-binary-tier, policy]
dependency_graph:
  requires: ["176-01", "176-02"]
  provides: ["CF-03"]
  affects: [".github/workflows/ci.yml", "CLAUDE.md"]
tech_stack:
  added: []
  patterns: ["GitHub Actions CI", "cargo --no-default-features build verification"]
key_files:
  created:
    - path: ".github/workflows/ci.yml"
      purpose: "CI workflow verifying default and minimal feature builds for rc-agent and rc-sentry"
  modified:
    - path: "CLAUDE.md"
      purpose: "Added single-binary-tier deployment policy as a standing rule"
decisions:
  - "single-binary-tier policy documented as standing rule: --no-default-features builds are CI-only, never deployed to pods"
  - "CI uses windows-latest runner to match pod target platform (Windows)"
  - "CI verifies rc-agent-crate and rc-sentry for both default and --no-default-features"
metrics:
  duration_seconds: 89
  completed_date: "2026-03-24T09:52:00+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
---

# Phase 176 Plan 03: CI Verification + Single-Binary-Tier Policy Summary

**One-liner:** GitHub Actions CI added for rc-agent/rc-sentry minimal+default builds; single-binary-tier runtime-flag policy documented in CLAUDE.md.

## What Was Built

### Task 1: CI workflow for minimal and default feature builds

Created `.github/workflows/ci.yml` with a `build` job on `windows-latest` that:
- Builds entire workspace with default features (`cargo build --workspace`)
- Runs `cargo test -p rc-common` and `cargo test -p rc-agent-crate`
- Builds `rc-agent-crate --no-default-features` (minimal: no ai-debugger, process-guard, keyboard-hook, http-client)
- Builds `rc-sentry --no-default-features` (minimal: no watchdog, tier1-fixes, ai-diagnosis)
- Explicitly re-builds both with default features to confirm the full binary compiles

### Task 2: Single-binary-tier policy in CLAUDE.md

Added standing rule to the Deploy section of CLAUDE.md:
- States that all pods run the SAME binary compiled with default features
- Clarifies `--no-default-features` is CI-only, NEVER deployed to pods
- Explains why: per-pod compile-time variants = combinatorial explosion (8 pods x N features)
- Points to v22.0 Phase 177+ runtime feature flags as the correct mechanism

## Verification Results

| Check | Result |
|-------|--------|
| `grep -c "no-default-features" .github/workflows/ci.yml` | 2 (rc-agent + rc-sentry) |
| `grep -c "single-binary-tier" CLAUDE.md` | 1 |
| `grep "rc-agent-crate.*no-default-features"` | Found |
| `grep "rc-sentry.*no-default-features"` | Found |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `46a8d275` | chore(176-03): add CI workflow with minimal and default feature builds |
| 2 | `776f8a78` | docs(176-03): add single-binary-tier policy to CLAUDE.md standing rules |

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- `.github/workflows/ci.yml` — FOUND
- `CLAUDE.md` contains "single-binary-tier" — FOUND
- Commit `46a8d275` — FOUND
- Commit `776f8a78` — FOUND
