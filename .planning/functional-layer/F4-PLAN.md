# F4 — Data-Content Validator (Functional Layer)

**Milestone:** Functional-Layer (F1-F5) — parallel to the shipped Structural
Layer (D1-D5). See `memory/session_progress_20260421_functional_layer_milestone_kickoff.md`
for full rationale.

**Branch:** `feat/f4-data-content-check-20260421`
**Base:** `main` @ `6e55202b`
**Status:** Autonomous build, first pass. R1 template-bound, R2-R4 stubbed
pending live-target access.

## Purpose

Structural checkers (D1-D5) verify *topology* — does the file exist, does the
route register, does the FSM compile, does the schema match. They green-light
even when the *content* of a file is wrong. Zero Laps P0 is the canonical
case: `python.ini` existed on all 8 pods (structural check: pass), but 7/8
were missing the `[RACECONTROL]` section (content: fail), so the AC telemetry
plugin never loaded and zero laps were ever recorded for weeks.

F4 validates content of known-critical runtime files + DB invariants + server
↔ client config mirrors. It is snapshot-style (one pass over state), not
event-stream — its sibling F2 (temporal-invariant checker) covers the
stream case. F4 catches the drift classes that look green to every existing
D-layer checker.

## Rules (v1, 4 implemented or stubbed; R5 skipped)

- **R1 — `python_ini_racecontrol_section`.** Per-pod Assetto Corsa `python.ini`
  must contain the `[RACECONTROL]` section with `ACTIVE=1`. Source of truth:
  `crates/rc-agent/src/ac_launcher.rs::ensure_python_ini_racecontrol`.
  Catches: **Zero Laps P0** (7/8 pods missing the section for weeks; 0 laps
  ever recorded; `ac0b215e` was the runtime fix, F4 R1 is the cross-check).
- **R2 — `mesh_service_key_pod_server_match`.** Per-pod `mesh_service_key`
  must match the value the server returns at `GET /api/v1/pods/mesh-service-key`.
  Catches: **Gap-4 class** — pre-Option-Z rc-agent read the key from a stale
  HKLM registry entry; mesh handshake failed silently. Phase 413.1 Plan 06
  (`9fb4a1a2`) was the runtime fix; F4 R2 is the cross-check.
- **R3 — `billing_session_total_cost_matches_events`.** For every
  `billing_session` row, `total_cost == SUM(billing_events.amount WHERE
  billing_session_id = ?)`. Catches: **silent orphan-refund class** (v45.0
  wallet bugs), **BILL-14 sim_type=None silent abort**.
- **R4 — `kiosk_settings_server_kiosk_cache_match`.** Server `kiosk_settings`
  row must match the config the kiosk actually has cached. Catches:
  **cloud-authoritative kiosk_settings flaw** surfaced 2026-04-18 during the
  fleet-wide lockdown incident.
- **R5 — `swaplog_binary_hash_parity` — SKIPPED.** Already covered end-to-end
  by `scripts/audit/fleet-swaplog-parity-check.py` (shipped in `f9c6678b`
  + updated `1025a1b6`). F4 would duplicate this detection, not add a rule
  class.

## Deliverables checklist

- [x] `.planning/functional-layer/F4-PLAN.md` (this file, Commit 1)
- [x] `.planning/functional-layer/data-content-rules.yaml` (Commit 2)
- [x] `scripts/audit/data-content-check.py` (Commit 3)
- [x] `tests/audit/test_data_content_check.py` + fixtures (Commit 4)
- [x] `pytest.ini` + `tests/audit/__init__.py` (Commit 5, if pytest needs them)
- [x] `scripts/audit/run-all-checkers.sh` registration (Commit 6)
- [x] `LOGBOOK.md` entry (Commit 7)

## Scope guards (first-pass, autonomous build)

1. **No live pod access tonight.** R1 ships with a fixture-bound implementation
   (validator runs against any python.ini path), but the CLI default does NOT
   ssh to pods or the server. R1 surfaces as `stubbed: live-probe-needed` when
   invoked without an explicit `--python-ini <path>` argument.
2. **R2-R4 stubbed.** All three need live targets (server DB, pod HKLM,
   kiosk-cached config) that require credentials this agent does not have.
   Each stubbed rule returns a structured `{status: "stubbed", todo: ...}`
   result — the framework runs end-to-end, no crashes, violation count = 0
   for stubbed rules.
3. **No deploy.** The checker is purely a scripts/audit/ artifact. No binary
   rebuild, no pm2 restart, no SCP. Integration is via
   `run-all-checkers.sh` orchestrator only.
4. **No MEMORY.md edit.** Orchestrator (parent agent) handles memory writes.
5. **No pod/server logs tailed.** Violates the "first-pass autonomous scope"
   bound.

## Test strategy

- **Unit**: pytest cases under `tests/audit/test_data_content_check.py`.
  - R1 handler: valid python.ini fixture, missing-section fixture, missing-key
    fixture.
  - Stubbed-rule dispatch: returns `stubbed` status, no exception.
  - Rules-file load: `yaml.safe_load` on `data-content-rules.yaml` succeeds,
    all 4 rules present with expected schema.
  - End-to-end: `data_content_check()` returns aggregated dict with 1
    implemented-or-stubbed result per rule, exit-code mapping is correct.
- **Integration**: `bash scripts/audit/run-all-checkers.sh` should include
  F4 as an additional row, green on a clean repo where R1 has no live input
  (returns stubbed-but-ok).
- **NOT tested** (by design, documented in the final report):
  - R1 against an actual pod python.ini (needs SSH, out of scope)
  - R2-R4 end-to-end (stubbed)
  - Performance on large rule packs (4 rules, non-issue)
  - pytest cross-platform (developed on Windows git-bash, runs via
    `python3 -m pytest`)

## Integration point

F4 is registered in `scripts/audit/run-all-checkers.sh` as a new row in the
`CHECKS` array, same shape as the other 4 checkers:

```
"data-content|true|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/data-content-check.py"
```

Generator column is `true` (no pre-generation step — rules file is
git-tracked at `.planning/functional-layer/data-content-rules.yaml`). Checker
reads rules + writes `.planning/data-content-violations.json`. Exit 0 = no
violations (including all-stubbed-runs), 1 = at least one violation, 2 =
checker internal error.

## Related

- Kickoff: `memory/session_progress_20260421_functional_layer_milestone_kickoff.md`
- Structural counterpart: D5 `workflow-graph-generate.py` + `workflow-centrality-check.py`
- Sibling: F2 temporal-invariant (not yet started — blocked on F1 ledger)
- Dedup target: F4 R5 explicitly skipped; existing
  `scripts/audit/fleet-swaplog-parity-check.py` owns that surface.
