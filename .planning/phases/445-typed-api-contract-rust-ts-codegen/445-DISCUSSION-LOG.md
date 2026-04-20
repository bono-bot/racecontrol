# Phase 445: Typed API Contract (Rust→TS codegen) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 445-CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 445-typed-api-contract-rust-ts-codegen
**Mode:** `--auto` (no interactive Q&A — recommended defaults selected and logged)
**Areas discussed (auto-selected, all):** Toolchain, Migration target, API contract surface scope, Contract delivery, Dual-write + migration policy, Enum representation, CI enforcement, WS protocol, Success-criteria test

---

## Toolchain

| Option | Description | Selected |
|--------|-------------|----------|
| utoipa + ts-rs | First-class axum OpenAPI + derive-based TS generation, both active | ✓ |
| aide + ts-rs | Similar OpenAPI scope, less mature axum integration |  |
| utoipa + typeshare | typeshare needs a separate config file |  |
| utoipa + specta | Newer/less battle-tested |  |

**Auto-selected:** utoipa + ts-rs (recommended default).
**Rationale:** Maximum axum compatibility and simplest derive ergonomics. Documented in D-01, D-02.

---

## Migration target (first consumer)

| Option | Description | Selected |
|--------|-------------|----------|
| admin | Smallest surface (~30–40 routes per admin graphify corpus), documented API-error pain, highest value-per-line | ✓ |
| kiosk | Largest surface; would mean rewriting kiosk's API client in one phase |  |
| web dashboard | Medium surface; less documented pain than admin |  |
| pwa | Customer-facing; highest caution required; worst first-migration target |  |

**Auto-selected:** admin.
**Rationale:** Memory documents repeated `rcFetch`/`proxyFetch` shape bugs, auth boundary confusion, cloud↔venue divergence. Smallest surface proves the pattern fastest. D-04.

---

## API contract surface scope

| Option | Description | Selected |
|--------|-------------|----------|
| Admin-used routes only (~30–40) | Incremental; proves pattern; low blast radius | ✓ |
| All routes (~400) | One-shot migration; huge PR; annotation fatigue |  |
| Types only, routes later | Generates TS structs but no OpenAPI — halfway solution |  |

**Auto-selected:** Admin-used routes only.
**Rationale:** CONTEXT.md flags 400-endpoint annotation as a known risk. Incremental avoids it. D-06.

---

## Contract delivery

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse `@racingpoint/types` package; add `generated/` subdir | Package exists with 9 hand-written modules; generated slots in cleanly | ✓ |
| New `@racingpoint/types-generated` package | Fragments the namespace; consumers need two imports |  |
| Overwrite hand-written `src/` directly | No dual-write safety net |  |
| Per-repo copy (no workspace package) | Cloud↔venue divergence risk |  |

**Auto-selected:** Reuse existing package with `generated/` subdir.
**Rationale:** Package exists; clean diff during dual-write; preserves git history. D-08, D-09, D-10.

---

## Dual-write + migration policy

| Option | Description | Selected |
|--------|-------------|----------|
| Dual-write for one release (2–3 weeks) | Safe rollback; drift audit per type | ✓ |
| Single-shot replace | Faster; no rollback path |  |
| Dual-write indefinite | Tech debt accumulates |  |

**Auto-selected:** Dual-write for one release cycle.
**Rationale:** If generated types miss a hand-written edge case, rollback is `git revert` of one re-export line. D-11, D-12.

---

## Enum representation

| Option | Description | Selected |
|--------|-------------|----------|
| Externally tagged (serde default) | ts-rs handles cleanly; most compatible | ✓ |
| Internally tagged | Breaks with `#[serde(flatten)]` |  |
| Adjacently tagged | Verbose on TS side |  |

**Auto-selected:** Externally tagged.
**Rationale:** Serde default; least surprise; safest generation. D-13.

**Safety gate logged:** If any existing Rust enum uses non-external tagging, DO NOT blindly convert — audit separately. D-14.

---

## CI enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Strict drift check from day one | Hard gate; PR fails on any drift | ✓ |
| Advisory-only initially | Drift re-accumulates silently |  |
| Periodic regeneration only | Drift detectable only at generation time |  |

**Auto-selected:** Strict from day one.
**Rationale:** Advisory defeats the purpose — hard gate is the structural fix. Matches existing `security-check.js` pattern. D-15, D-16, D-17.

---

## WS protocol versioning

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to separate phase | Safe; focus on HTTP first | ✓ |
| Include in this phase | Fleet-wide blast radius; `handle_ws_message` is the mega-hub |  |
| WS types only (no versioning) | Halfway solution; increases scope without solving WS bugs |  |

**Auto-selected:** Defer.
**Rationale:** `handle_ws_message()` is graphify-verified as the ecosystem's biggest function-level hub (107 edges racecontrol, 91 edges rc-agent). HTTP-first is the safe path. D-18, D-19.

---

## Success-criteria test (regression fixture)

| Option | Description | Selected |
|--------|-------------|----------|
| Deliberate field-name mismatch in `packages/contract-tests/` | Explicit negative test; locks CI gate works | ✓ |
| Positive test only | Can't prove CI actually catches drift |  |
| No fixture | Invisible regression; can't verify |  |

**Auto-selected:** Deliberate mismatch fixture.
**Rationale:** Original CONTEXT.md lists this in success criteria. Locks the acceptance test for the planner. D-20.

---

## Claude's Discretion (not decided — planner/researcher flexibility)

- Exact `utoipa` vs `utoipa-axum` vs `utoipa-swagger-ui` crate mix
- `ts-rs` per-struct `#[ts(export_to = "...")]` vs central export config
- File-level organization of generated `.ts` output (one file vs per-module)
- CI tool specifics (GitHub Action vs local pre-commit vs both)
- Version bump policy for `@racingpoint/types`
- Exact `deploy-audit.sh` manifest field name

## Deferred Ideas

See `445-CONTEXT.md` `<deferred>` section for the full list. Key items:
- Full migration of kiosk/web/pwa — follow-on phases
- WS protocol versioning — separate phase
- Annotating the ~370 non-admin routes — incremental follow-on
- Deleting hand-written `packages/shared-types/src/*.ts` — only after dual-write release
- True mesh intelligence — separate aspiration

## Scope-creep redirections

None during auto run (no user input to creep from).
