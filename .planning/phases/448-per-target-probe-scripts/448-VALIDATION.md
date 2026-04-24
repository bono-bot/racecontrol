---
phase: 448
slug: per-target-probe-scripts
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-24
updated: 2026-04-24
---

# Phase 448 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Fills Nyquist Dimension 8.

Authoritative source for validation details: `448-RESEARCH.md` section 6 (Validation Architecture) + section 7 (Failure Mode Matrix). This doc is the executor-facing contract.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Node.js test runner (node --test) + bash integration tests; reuses Phase 447 ajv ^8.17.1 + ajv-formats ^3.0.1 |
| **Config file** | package.json `scripts.test:fleet-probe` (added in Plan 01 Wave 0); no separate vitest/jest config |
| **Quick run command** | `npm run test:fleet-probe` |
| **Full suite command** | `npm run test:fleet-drift && npm run test:fleet-probe && bash tests/fleet-probe/smoke-orchestrator.sh` |
| **Estimated runtime** | ~30 seconds (unit) / ~3 min (full + integration smoke) |

---

## Sampling Rate

- **After every task commit:** Run `npm run test:fleet-probe`
- **After every plan wave:** Run full suite (above)
- **Before `/gsd:verify-work`:** Full suite must be green AND `bash scripts/fleet-probe/probe-all.sh --canary --dry-run` still enumerates targets (or `--canary` canary run exits 0 with 2 manifests + _meta.json)
- **Max feedback latency:** 30 seconds (unit); 180 seconds (integration incl. smoke-orchestrator.sh)

---

## Per-Task Verification Map

Every task touching a probe script or test file has at least one `<automated>` verify command. This table pre-computes the map so the executor has no ambiguity about what "done" means per task.

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-T1 | 448-01 | 0 | PROBE-09 (scaffold) | structural | `bash -n scripts/fleet-probe/lib/probe-common.sh && node --check scripts/fleet-probe/validate-manifest-file.mjs` | `scripts/fleet-probe/lib/probe-common.sh`, `scripts/fleet-probe/validate-manifest-file.mjs` | pending |
| 01-T2 | 448-01 | 0 | PROBE-09 (scaffold) | unit | `npm run test:fleet-probe` (schema-compat.test.mjs on 3 fixtures) | `tests/fleet-probe/helpers.mjs`, `tests/fleet-probe/mock-ssh-responder.sh`, `tests/fleet-probe/mock-http-server.mjs`, `tests/fleet-probe/schema-compat.test.mjs`, `tests/fleet-probe/fixtures/*.json`, `package.json` | pending |
| 02-T1 | 448-02 | 1 | PROBE-04 | unit + smoke | `npm run test:fleet-probe && bash tests/fleet-probe/smoke-james.sh` | `scripts/fleet-probe/probe-james.sh`, `tests/fleet-probe/probe-james.test.mjs`, `tests/fleet-probe/smoke-james.sh` | pending |
| 02-T2 | 448-02 | 1 | PROBE-09 (skeleton) | structural | `[ "$(bash scripts/fleet-probe/probe-all.sh --dry-run \| wc -l)" = "15" ]` | `scripts/fleet-probe/probe-all.sh` | pending |
| 03-T1 | 448-03 | 2 | PROBE-01 | unit (test scaffolding) | `node --check tests/fleet-probe/probe-server.test.mjs` | `tests/fleet-probe/probe-server.test.mjs`, `tests/fleet-probe/fixtures/server-ssh-ok.txt`, `tests/fleet-probe/fixtures/server-ssh-timeout.txt` | pending |
| 03-T2 | 448-03 | 2 | PROBE-01 | unit | `npm run test:fleet-probe` (probe-server.test.mjs green on both ok + probe_failed paths with access_gap=SSH_23) | `scripts/fleet-probe/probe-server.sh` | pending |
| 04-T1 | 448-04 | 2 | PROBE-02 | unit | `npm run test:fleet-probe` (probe-pod.test.mjs: invalid N, no SENTRY_KEY probe_failed, pod mapping) | `scripts/fleet-probe/probe-pod.sh`, `tests/fleet-probe/probe-pod.test.mjs`, `tests/fleet-probe/fixtures/pod-exec-ok.json`, `tests/fleet-probe/fixtures/pod-exec-401.json` | pending |
| 04-T2 | 448-04 | 2 | PROBE-03 | unit | `npm run test:fleet-probe` (probe-pos.test.mjs: partial tasklist, probe_failed POS_SSH_DOWN) | `scripts/fleet-probe/probe-pos.sh`, `tests/fleet-probe/probe-pos.test.mjs`, `tests/fleet-probe/fixtures/pos-ssh-partial.txt` | pending |
| 05-T1 | 448-05 | 3 | PROBE-05 | unit | `npm run test:fleet-probe` (probe-vps.test.mjs: no PSK, RELAY_DOWN, ok, exec-nonzero partial) | `scripts/fleet-probe/probe-vps.sh`, `tests/fleet-probe/probe-vps.test.mjs`, `tests/fleet-probe/fixtures/vps-relay-exec-ok.json`, `tests/fleet-probe/fixtures/vps-relay-exec-err.json` | pending |
| 05-T2 | 448-05 | 3 | PROBE-08 | unit | `npm run test:fleet-probe` (probe-relay.test.mjs: both ok, vps disconnected partial, local down probe_failed) | `scripts/fleet-probe/probe-relay.sh`, `tests/fleet-probe/probe-relay.test.mjs`, `tests/fleet-probe/fixtures/relay-health-ok.json`, `tests/fleet-probe/fixtures/relay-health-disconnected.json` | pending |
| 06-T1 | 448-06 | 3 | PROBE-06 | unit | `npm run test:fleet-probe` (probe-cloud-admin.test.mjs: ok, gate, pages_missing partial, 500) | `scripts/fleet-probe/probe-cloud-admin.sh`, `tests/fleet-probe/probe-cloud-admin.test.mjs`, `tests/fleet-probe/fixtures/cloud-admin-health-ok.json`, `tests/fleet-probe/fixtures/cloud-admin-health-gated.json` | pending |
| 06-T2 | 448-06 | 3 | PROBE-07 | unit | `npm run test:fleet-probe` (probe-cloud-rc.test.mjs: ok, 500, malformed, missing build_id) | `scripts/fleet-probe/probe-cloud-rc.sh`, `tests/fleet-probe/probe-cloud-rc.test.mjs`, `tests/fleet-probe/fixtures/cloud-rc-health-ok.json` | pending |
| 07-T1 | 448-07 | 4 | PROBE-09 | structural + python | `bash -n scripts/fleet-probe/probe-all.sh && python3 -c "import ast; ast.parse(open('scripts/fleet-probe/build-meta-index.py').read())"` | `scripts/fleet-probe/build-meta-index.py`, `scripts/fleet-probe/probe-all.sh` (replaced) | pending |
| 07-T2 | 448-07 | 4 | PROBE-09 | integration | `bash tests/fleet-probe/smoke-orchestrator.sh && npm run test:fleet-probe` | `tests/fleet-probe/smoke-orchestrator.sh`, `tests/fleet-probe/orchestrator-dry-run.test.mjs` | pending |
| 08-T1 | 448-08 | 5 | PROBE-01 (access audit doc) | docs (grep-verifiable) | `grep -c "^## Server \\.23" docs/fleet-probe/access-gaps.md && grep -c "SSH_23" docs/fleet-probe/access-gaps.md` | `docs/fleet-probe/access-gaps.md` | pending |
| 08-T2 | 448-08 | 5 | PROBE-01 / staff entry-point | docs (grep-verifiable) | `grep -c "probe-all.sh" docs/fleet-probe/README.md && grep -c "access-gaps.md" docs/fleet-probe/README.md` | `docs/fleet-probe/README.md` | pending |

*Status: pending / green / red / flaky*

---

## Requirement-to-Plan Coverage

| Requirement | Plan(s) | Task(s) | Primary test file |
|-------------|---------|---------|-------------------|
| PROBE-01 | 448-03, 448-08 | 03-T1, 03-T2, 08-T1, 08-T2 | `tests/fleet-probe/probe-server.test.mjs`, `docs/fleet-probe/access-gaps.md` |
| PROBE-02 | 448-04 | 04-T1 | `tests/fleet-probe/probe-pod.test.mjs` |
| PROBE-03 | 448-04 | 04-T2 | `tests/fleet-probe/probe-pos.test.mjs` |
| PROBE-04 | 448-02 | 02-T1 | `tests/fleet-probe/probe-james.test.mjs` + `smoke-james.sh` |
| PROBE-05 | 448-05 | 05-T1 | `tests/fleet-probe/probe-vps.test.mjs` |
| PROBE-06 | 448-06 | 06-T1 | `tests/fleet-probe/probe-cloud-admin.test.mjs` |
| PROBE-07 | 448-06 | 06-T2 | `tests/fleet-probe/probe-cloud-rc.test.mjs` |
| PROBE-08 | 448-05 | 05-T2 | `tests/fleet-probe/probe-relay.test.mjs` |
| PROBE-09 | 448-01, 448-02, 448-07 | 01-T1, 01-T2, 02-T2, 07-T1, 07-T2 | `tests/fleet-probe/schema-compat.test.mjs`, `orchestrator-dry-run.test.mjs`, `smoke-orchestrator.sh` |

---

## Wave 0 Requirements (Plan 01)

- [ ] `tests/fleet-probe/helpers.mjs` -- mock target responders (startMockHttpServer, makeMockSshEnv, loadFixture, validateAgainstSchema)
- [ ] `tests/fleet-probe/fixtures/` -- canned ok/partial/probe_failed manifest fixtures (3 initial; more added by Plans 03-06)
- [ ] `tests/fleet-probe/mock-ssh-responder.sh` -- shell script that reads `PROBE_SSH_SCENARIO` file + emits matching stdout + exit code
- [ ] `tests/fleet-probe/mock-http-server.mjs` -- re-exports startMockHttpServer from helpers.mjs
- [ ] `scripts/fleet-probe/validate-manifest-file.mjs` -- thin ajv-CLI wrapper used by `write_manifest` under `FLEET_PROBE_VALIDATE=1`
- [ ] `scripts/fleet-probe/lib/probe-common.sh` -- shared helpers (10 functions locked in Plan 01 interface block)
- [ ] `package.json` -- `scripts.test:fleet-probe` entry preserving existing `test:fleet-drift`
- [ ] `tests/fleet-probe/schema-compat.test.mjs` -- assert every fixture validates against `schemas/fleet-manifest.schema.json`

Existing Phase 447 infrastructure that Wave 0 reuses (no re-install): ajv ^8.17.1, ajv-formats ^3.0.1, manifest schema, 8 example manifests, `schemas/examples/_meta.json`, `test:fleet-drift` command.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Server .23 SSH credential continuity | PROBE-01 | Session-scoped SSH auth (no service acct) -- user must re-auth if stale | Run `ssh ADMIN@100.125.108.37 "hostname && whoami && ver"` manually; EXIT=0 means probe-server.sh can proceed in Phase 449 |
| Pod 8 canary liveness | PROBE-02 | Pod 8 is physical sim -- can be powered off | Run `ssh User@100.98.67.67 "hostname && whoami"` manually; EXIT=0 means probe-pod.sh 8 can proceed in Phase 449 |
| POS .130 reachability | PROBE-03 | POS is sporadically-on (Tailscale flapping known per memory); unreachable path must be tested live at least once | Confirm POS is booted + on WiFi, then `ssh pos1 "hostname"` -- if UNREACHABLE, probe-pos.sh must emit `probe_status: probe_failed` with `POS_SSH_DOWN` reason, not fail the orchestrator |
| Cloud admin gate detection | PROBE-06 | Gate state (`ADMIN_COMING_SOON_GATE=1|0`) visible only via live HEAD; mock can't reproduce the real 307 flow end-to-end | After probe-cloud-admin.sh writes its manifest in Phase 449, check `scheduled_tasks[]` for `{name: "ADMIN_COMING_SOON_GATE", state: "active"}` when the real gate is on |
| First-time Phase 449 run | PROBE-09 | First live invocation of the whole orchestrator against real fleet is outside this phase's scope (that IS Phase 449) | `bash scripts/fleet-probe/probe-all.sh` exits 0, writes 15 manifests + _meta.json, all validate via `FLEET_PROBE_VALIDATE=1` |

---

## Validation Sign-Off

- [x] All 15 tasks have `<automated>` verify or explicit Wave 0 dependency (populated above)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify (every task has a grep or node or bash verify command)
- [x] Wave 0 covers all MISSING references (test helpers, validate-manifest-file CLI, probe-common.sh)
- [x] No watch-mode flags (single-run node --test + bash + python3)
- [x] Feedback latency < 30s (unit); < 180s (full+smoke)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending executor run of Plan 01.
