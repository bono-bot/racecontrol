# Phase 361 — Kiosk Preset Filtering + Server Gate — CONTEXT

**Milestone:** v46.0 Game Launch Diagnostics
**Status:** Not started — directory placeholder created 2026-04-09 20:55 IST, awaiting fresh-session `/gsd:plan-phase 361`
**Requirements:** GLD-A-01, GLD-A-02, GLD-A-03, GLD-A-04 (see `.planning/milestones/v46.0-REQUIREMENTS.md`)

## Goal

Prevent invalid car/track/AI combinations from reaching the game launcher by filtering them out at the kiosk UI, surfacing the already-computed `presetValidity` signal to the user, and rejecting bypass attempts at the server API. Also surface content drift — pods whose physical inventory no longer matches the expected pod TOML — so staff can see what's wrong before the next session fails.

## Problem being solved

Today the kiosk staff wizard:
1. Computes a `presetValidity` boolean but **never surfaces it in the UI** (P0 silent-loss point) — staff can click "Start Session" on an invalid config and the game will launch with wrong settings or crash.
2. Shows **all** cars and tracks in the dropdowns regardless of what's installed on the selected pod — letting staff pick a car the pod doesn't have, which then either fails silently or triggers a false verification mismatch in Phase 362.
3. The server `/sessions/start` endpoint **does not validate** the `(pod_id, game, car, track, ai_count)` tuple against a canonical validity table — a malicious or buggy client can send any combination and the server will forward it to the pod.

Fixing this closes 2 explicit silent-loss points (P0-02, P2-08) and prevents a whole class of "what did the kiosk actually send?" debugging.

## Requirements (from v46.0-REQUIREMENTS.md)

- **GLD-A-01:** Kiosk staff wizard reads each pod's game/car/track inventory from `/api/v1/pods/{id}/inventory` and filters the car/track dropdowns to only what that pod has installed.
- **GLD-A-02:** Kiosk staff wizard surfaces the previously-computed `presetValidity` value in the UI — invalid combinations disable "Start Session" with an inline reason.
- **GLD-A-03:** Server `/api/v1/sessions/start` rejects any session request whose `(pod_id, game, car, track, ai_count)` tuple fails a server-side validity check, returning HTTP 422 with a structured `{reason, suggestion}` body.
- **GLD-A-04:** Admin `/admin/content-drift` page lists pods whose physical inventory no longer matches the expected pod TOML (ACR-02 hardening for Phase 366 consumption).

## Success criteria (from v46.0-ROADMAP.md Phase 361)

1. Kiosk car/track dropdowns hide entries not installed on the selected pod (checked against `/api/v1/pods/{id}/inventory` response from a live pod).
2. Invalid combos disable "Start Session" with an inline reason message that references the failing `presetValidity` rule.
3. Server `/sessions/start` returns HTTP 422 + `{reason, suggestion}` body when a client bypasses the kiosk filter (verified via direct `curl` test with a known-bad combo).
4. Admin `/admin/content-drift` lists any pod whose physical inventory no longer matches the TOML expectation.

## Likely plan structure (for planner reference, not binding)

- **361-01-PLAN — Server inventory endpoint + validity gate** (racecontrol)
  - New `GET /api/v1/pods/{id}/inventory` returning installed games/cars/tracks from pod TOML
  - New validity check function in `crates/racecontrol/src/validation/session_validity.rs`
  - Wire into `POST /api/v1/sessions/start` handler; return HTTP 422 on failure
  - Unit tests for every invalid combo category (wrong game, wrong car for game, wrong track for car, AI count out of range)

- **361-02-PLAN — Kiosk filter + presetValidity surface** (Next.js kiosk app)
  - `SetupWizard.tsx`: fetch `/pods/{id}/inventory` on pod selection
  - Filter car/track dropdowns by inventory response
  - Surface `presetValidity` with `disabled={!valid}` on Start button + inline `<p className="text-red-500">{reason}</p>`
  - E2E Playwright test covering the happy path and one known-invalid combo

- **361-03-PLAN — Admin content-drift page + nyquist tests** (racingpoint-admin app)
  - New page `app/admin/content-drift/page.tsx`
  - Reads `/api/v1/pods/{id}/inventory` for all 8 pods, diffs against expected pod TOML, renders drift table
  - Nyquist test audit covering the validity gate logic

## Subagent gates (MANDATORY per standing rules)

- **Frontend (kiosk + admin pages):** `gsd-ui-researcher` must produce UI-SPEC.md BEFORE planning. `gsd-ui-auditor` must produce UI-REVIEW.md AFTER execution, before ship.
- **Business logic (validity gate):** `gsd-nyquist-auditor` must audit test coverage AFTER execution.
- **Cross-system data bridge:** MMA audit recommended before milestone ship (cross-boundary: kiosk ↔ server ↔ pod inventory).

## Key files to touch (preliminary — planner should verify)

- `crates/racecontrol/src/api/routes.rs` — register new `/pods/{id}/inventory` GET route
- `crates/racecontrol/src/api/pods.rs` — inventory handler reading pod TOML
- `crates/racecontrol/src/api/sessions.rs` — validity check in `start_session` handler
- `crates/racecontrol/src/validation/session_validity.rs` — NEW module for combo validation
- `deploy/configs/rc-agent-pod{1-8}.toml` — source of truth for installed content (READ ONLY)
- `kiosk/src/components/wizard/SetupWizard.tsx` — filter + presetValidity surface
- `kiosk/src/lib/api.ts` — new `api.podInventory(podId)` method
- `racingpoint-admin/app/admin/content-drift/page.tsx` — NEW admin page
- `packages/shared-types/src/index.ts` — add `PodInventory` type
- `docs/openapi.yaml` — add `/pods/{id}/inventory` endpoint spec
- `tests/contract/pod-inventory.test.ts` — NEW contract test

## Deploy manifest (preliminary)

| Artifact | Target | Why |
|----------|--------|-----|
| `racecontrol.exe` new build | Server .23 | New endpoint + validity gate |
| Kiosk rebuild | Server .23 :3300 + cloud kiosk :3300 | Filter + UI surface (dual deploy per parity rule) |
| Admin rebuild | Server .23 :3201 + cloud admin :3201 | New content-drift page (dual deploy) |
| Pod TOMLs | Pods 1-8 | Read-only; no changes unless drift found during implementation |
| OpenAPI spec | Repo | Contract documentation |
| Shared types | Repo + kiosk/admin consumers | Type contract |

## Dependencies

- **Phase 362 SHIPPED** (a9b5eaa3) — provides the SessionConfig struct and fuzzy match logic that Phase 361's validity gate will reuse for car/track normalization.
- **None blocking** — Phase 361 can start immediately.

## Context carry-over

This CONTEXT.md was written retroactively by the orchestrator session on 2026-04-09 20:55 IST as a scaffold for the planner. It deliberately does NOT replace the full `gsd-phase-researcher` + `gsd-ui-researcher` + `gsd-planner` + `gsd-plan-checker` workflow — that must run in a fresh `/clear`ed session via `/gsd:plan-phase 361` so each subagent operates on clean context and can produce H3-compliant plan artifacts.

**Do not treat this file as the authoritative PLAN.md.** It is a context bootstrap for the planner, extracted from the already-committed v46.0 requirements and roadmap files.

## Next step

In a fresh `/clear`ed session:

```bash
cd ~/racingpoint/racecontrol
/gsd:plan-phase 361
```

The plan-phase workflow will:
1. Detect this CONTEXT.md and skip the discuss-phase question dump
2. Spawn `gsd-ui-researcher` (MANDATORY — touches kiosk + admin)
3. Spawn `gsd-phase-researcher` for the server-side validity gate
4. Spawn `gsd-planner` with the research artifacts
5. Spawn `gsd-plan-checker` to verify plan quality
6. Iterate up to 3 times if quality gate fails
7. Write final 361-01-PLAN.md, 361-02-PLAN.md, 361-03-PLAN.md

Do NOT try to run this from the current session — context is already heavy and the subagents need clean state.
