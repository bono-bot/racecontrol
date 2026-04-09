# Phase 361-01 — Deferred Runtime Tasks

**Code-only execution:** 2026-04-09 ~22:00 IST
**Session reason:** Orchestrator session heavy; user directed code-only scope.
Runtime/deploy has high blast radius (8 live pods, server .23, Bono VPS).
Code is done and tested; runtime validation and fleet rollout are deferred to
a fresh-session execute-phase run.

## Completed in this session (code + tests only)

| Task | Deliverable | Tests | Commit |
| ---- | ----------- | ----- | ------ |
| 1 | `crates/rc-common/src/inventory_types.rs` — `PodInventory`, `GameInventory`, `AiCountRange`, `ValidityError`, `ValidityErrorCode`, `ContentDirsResponse`, `GameDirs` + rc-common module wiring in `lib.rs` | 4 pass (`cargo test -p rc-common --lib inventory_types`) | see below |
| 2 (validity gate) | `crates/racecontrol/src/validation/mod.rs` + `crates/racecontrol/src/validation/session_validity.rs` — pure `validate_session_tuple()` function, all 5 error variants + happy path + degrade-open | 11 pass (`cargo test -p racecontrol-crate --lib validation::session_validity`) | see below |
| 2 (inventory endpoint) | `crates/racecontrol/src/api/pods.rs` — `load_pod_inventory()` + `pod_inventory_handler`, registered in `staff_routes` at `GET /api/v1/pods/{id}/inventory` | 4 pass (`cargo test -p racecontrol-crate --lib api::pods`) | see below |
| 2 (config_dir) | `crates/racecontrol/src/config.rs` — `ServerConfig.config_dir` field added (default `./deploy/configs`) + `config_dir_path()` helper. NOT the production racecontrol.toml update — that is deploy-time. | (checked via compile) | see below |
| 2 (wire validity gate) | `crates/racecontrol/src/api/routes.rs::launch_game` — gate runs BEFORE `game_launcher::handle_dashboard_command().await`, pre-lock, pre-WS-dispatch. **Placement deviation from plan** — see Deviations section. | compile-only; runtime regression test deferred | see below |
| 2.6 | `crates/rc-agent/src/remote_ops.rs` — `GET /debug/content-dirs` handler behind `require_service_key` middleware in all 3 router starts (plain, `start_checked`, `start_checked_with_tls`). Sync disk enumeration of `[games.*]` install dirs. | 3 pass (`cargo test -p rc-agent-crate --bin rc-agent content_dirs`) | see below |
| OpenAPI | `docs/openapi.yaml` — `/pods/{id}/inventory` path with `staffJWT` bearerAuth + 200/401/404; `/games/launch` response schema now includes `ValidityError` oneOf variant with `status: 422`; components schemas `PodInventory`, `GameInventory`, `AiCountRange`, `ValidityError` added. | (schema additions only — spec validation deferred) | see below |

**Total new tests: 22 (all passing in this session, 0 failures).**

## Deviations from plan

### DEV-1: Validity gate wiring target (Rule 1 — plan/reality mismatch)

**Plan said:** Wire `validate_session_tuple` into `create_session` at
`crates/racecontrol/src/api/routes.rs:2541`, "immediately after Json body
deserialization and BEFORE any `state.db.begin()`."

**Reality:** `create_session` at line 2541 is a legacy handler that
deserializes `Json<Value>` and only looks up `type`, `sim_type`, `track`,
`car_class`. It does **not** receive `pod_id`, `car`, or `ai_count`. The
actual user-selected tuple flows through `launch_game` (line 5557, POST
`/games/launch`) via the `launch_args` JSON blob. A plan-faithful wiring in
`create_session` would be a no-op because four of the five required fields
are absent.

**Resolution:** Wired the gate into `launch_game` instead. Placement is still
pre-lock and pre-WS-dispatch — the gate runs BEFORE
`game_launcher::handle_dashboard_command(&state, cmd).await`, which is the
first `.await` point that acquires any lock in the launch path. Only sync
I/O (`std::fs::read_to_string` + `toml::from_str`) runs before the gate. On
validity failure, the handler returns JSON `{status: 422, code, reason,
suggestion}` — the HTTP status stays 200 for backward compatibility with
legacy callers that discriminate on the `error` body field. Kiosk 361-02
clients should switch to the `status: 422` body discriminator.

The plan's "sessions/start vs sessions" discrepancy note called this out as
"document in SUMMARY" — this is the SUMMARY documentation of that
discrepancy.

### DEV-2: Degrade-open on missing pod TOML (Rule 3 — safety for deferred 2.5)

When the pod TOML file is absent from `config_dir`, `load_pod_inventory`
returns 404. The gate wiring in `launch_game` handles this explicitly: it
logs a `tracing::warn!` and **skips validation** (degrade-open) rather than
blocking launches on missing config infrastructure. This is required
because Task 2.5 (pod TOML `[content.*]` population) is deferred — without
this fallback the first runtime rollout of the racecontrol binary would
reject every launch on every pod until Task 2.5 ships.

Once Task 2.5 populates all 8 pod TOMLs, the runtime session can remove
this degrade-open branch (or leave it as a belt-and-braces safety net —
the validity gate itself still degrades open on empty cars/tracks vecs, so
the only remaining risk is a corrupted/missing file which is also worth
logging instead of killing).

### DEV-3: rc-common module layout (Rule 3 — workspace structure)

Plan specified `crates/rc-common/src/types/inventory.rs` with a `types/`
directory module. The existing `rc-common` has a **flat** `types.rs` file,
not a directory module. Creating `types/inventory.rs` would have required
converting `types.rs` into `types/mod.rs` — a mechanical but large diff
that touches every consumer of `rc_common::types::*`.

**Resolution:** Created `crates/rc-common/src/inventory_types.rs` as a new
top-level sibling module. Import path: `rc_common::inventory_types::*`.
Same cross-boundary guarantees; zero churn on existing `types::*` imports.

## Deferred to fresh-session `execute-phase 361 --gaps-only`

### 1. Task 2.5 — Pod TOML `[content.*]` population (8 pods via SSH enumeration)

- **Why deferred:** SSH into 8 pods, `dir /B` enumeration of Steam content
  directories, and writing to `deploy/configs/rc-agent-pod{1-8}.toml`. High
  blast radius (touches committed config files that ship to every pod) and
  non-trivial to test without live pods.
- **Method:** Follow plan section A-H (SSH `pod1`..`pod8`, enumerate
  `content/cars` and `content/tracks` for each installed `[games.*]` game,
  append `[content.<key>]` sections to the respective TOMLs, preserve CRLF,
  no inline PowerShell over SSH).
- **Blocker it creates:** Until this runs, the validity gate **degrades
  open for every game on every pod** — no invalid tuple is actually
  rejected in production. The code path is active; only the data is empty.

### 2. Task 3 — Full deploy sequence

Entire STEP 0 through STEP 10 from plan section `<task>Task 3</task>`:

- STEP 0: Write production `deploy-staging/racecontrol-server23.toml` with
  `config_dir = "C:\\RacingPoint\\deploy\\configs"`.
- STEP 1: `scp deploy/configs/rc-agent-pod{1-8}.toml
  ADMIN@100.125.108.37:C:/RacingPoint/deploy/configs/` (8 files).
- STEP 2: `scp deploy-staging/racecontrol-server23.toml
  ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.toml`.
- STEP 3: `deploy-staging/deploy-server.sh` (v3.0 MMA-hardened 8-step) for
  the racecontrol binary on server .23.
- STEP 4: rc-agent fleet deploy (build → stage → curl download on each
  pod → SCP updated `start-rcagent.bat` → kill rc-agent → RCWatchdog
  restart in Session 1 → verify `build_id` on `:8090/health`). Canary
  Pod 8 first.
- STEP 5: curl verification on server .23 — 401 without JWT, 200 with,
  non-empty `cars` array.
- STEP 6: **Production regression curl** —
  ```bash
  curl -s -X POST http://192.168.31.23:8080/api/v1/games/launch \
    -H "Authorization: Bearer $JWT" \
    -H 'Content-Type: application/json' \
    -d '{"pod_id":"1","sim_type":"assetto_corsa","driver_id":"test","launch_args":"{\"car\":\"nonexistent_ferrari_9999\",\"track\":\"spa\",\"ai_count\":5}"}'
  ```
  Expected body: `{"status":422,"code":"CAR_NOT_AVAILABLE",...}`.
  **Note:** the plan says hit `/sessions` with a 422 HTTP status; this code
  wires into `/games/launch` and returns `status: 422` **in the body**.
  See DEV-1 above.
- STEP 7: rc-agent `/debug/content-dirs` curl verification on all 8 pods
  with `X-Service-Key` header.
- STEP 8: Bono VPS deploy (git pull via relay + cloud rebuild + cloud
  inventory curl verification).

### 3. Task 3 Step 9 — Nyquist audit

**Why deferred:** The gate being audited isn't deployed — runtime
verification is meaningless without the binary actually serving traffic.

**Resume:** After STEP 6 passes, spawn `gsd-nyquist-auditor` against
`crates/racecontrol/src/validation/session_validity.rs` + `api/pods.rs` +
the wiring block in `launch_game`. Capture to
`.planning/phases/361-kiosk-preset-filtering-server-gate/NYQUIST-AUDIT.md`.
Wave 2 (361-02 + 361-03) is **blocked until nyquist PASS** per plan.

### 4. Task 3 Step 10 — LOGBOOK + memory update

After runtime verification passes, append to `LOGBOOK.md` with the deploy
timestamp + build hash, and update
`~/.claude/projects/C--Users-bono/memory/MEMORY.md` "Current Deployed
Builds" table with the new racecontrol + rc-agent hashes.

## Resume command

```bash
/gsd:execute-phase 361 --gaps-only
```

(Or run tasks manually per plan `361-01-PLAN.md` sections 2.5, 3, and the
nyquist step. Reference this file for what was already done so the
continuation executor does not redo the code work.)

## NOT TESTED in this session (H3 list)

- Runtime behavior of the validity gate on server .23 (no deploy).
- Runtime behavior with **populated** pod TOMLs (Task 2.5 deferred).
- End-to-end 422 curl against a real game + fake car combo on production.
- Kiosk consumption of `/pods/{id}/inventory` (that is 361-02's scope).
- Admin content drift page consuming `/debug/content-dirs` (that is
  361-03's scope).
- Cloud parity verification on `api.racingpoint.cloud`.
- Nyquist audit coverage score.
- Contract test execution (`tests/contract/pod-inventory.test.ts`) — the
  plan specified a contract test but this scope-restricted session did
  not create it because it requires a live racecontrol process to run
  against, which the code-only scope forbids.
- `docs/openapi.yaml` validation via `swagger-cli validate` — not run in
  this session; schema additions were by inspection only.
