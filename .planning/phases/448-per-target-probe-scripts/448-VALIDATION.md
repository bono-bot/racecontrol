---
phase: 448
slug: per-target-probe-scripts
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-24
---

# Phase 448 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Fills Nyquist Dimension 8.

Authoritative source for validation details: `448-RESEARCH.md` §6 (Validation Architecture) + §7 (Failure Mode Matrix). This doc is the executor-facing contract.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Node.js test runner (node --test) + bash integration tests; reuses Phase 447 ajv ^8.17.1 + ajv-formats ^3.0.1 |
| **Config file** | package.json `scripts.test:fleet-probe` (NEW in Wave 0); no separate vitest/jest config |
| **Quick run command** | `npm run test:fleet-probe` |
| **Full suite command** | `npm run test:fleet-drift && npm run test:fleet-probe && bash scripts/fleet-probe/probe-all.sh --dry-run` |
| **Estimated runtime** | ~30 seconds (unit) / ~3 min (full + integration dry-run) |

---

## Sampling Rate

- **After every task commit:** Run `npm run test:fleet-probe`
- **After every plan wave:** Run full suite (above)
- **Before `/gsd:verify-work`:** Full suite must be green AND `bash scripts/fleet-probe/probe-all.sh --canary --dry-run` exits 0 with 2 manifest entries (server + pod 8)
- **Max feedback latency:** 30 seconds (unit); 180 seconds (integration incl. dry-run orchestrator)

---

## Per-Task Verification Map

Populated during planner step. Every task touching a probe-*.sh, probe-common.sh, or test file MUST have a corresponding row here with an `<automated>` verify or explicit Wave 0 dependency.

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| *To be populated by planner — each probe script gets at least one unit test row (REQ PROBE-01 .. PROBE-08) + one integration-validation row (REQ PROBE-09 orchestrator)* | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/fleet-probe/helpers.mjs` — mock target responders (SSH stdout capture, HTTP response fixtures, relay JSON shapes)
- [ ] `tests/fleet-probe/fixtures/` — canned `probe_status: probe_ok` + `probe_failed` manifest fixtures per target
- [ ] `scripts/fleet-probe/validate-manifest-file.mjs` — thin ajv-CLI wrapper used by `write_manifest` under `FLEET_PROBE_VALIDATE=1` + by unit tests
- [ ] `scripts/fleet-probe/lib/probe-common.sh` — shared helpers (manifest writers, SHA256 utilities, IST timestamp via `scripts/ist-now.sh`, JSON assembly, `probe_errors[]` writer)
- [ ] `package.json` — add `scripts.test:fleet-probe`
- [ ] `tests/fleet-probe/schema-compat.test.mjs` — assert every fixture validates against `schemas/fleet-manifest.schema.json`

Existing Phase 447 infrastructure covers: ajv installation, manifest schema, 8 example manifests, `schemas/examples/_meta.json`, test:fleet-drift command.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Server .23 SSH credential continuity | PROBE-01 | Session-scoped SSH auth (no service acct) — user must re-auth if stale | Run `ssh ADMIN@100.125.108.37 "hostname && whoami && ver"` manually; EXIT=0 means probe-server.sh can proceed |
| Pod 8 canary liveness | PROBE-02 | Pod 8 is physical sim — can be powered off | Run `ssh User@100.98.67.67 "hostname && whoami"` manually; EXIT=0 means probe-pod.sh 8 can proceed |
| POS .130 reachability | PROBE-03 | POS is sporadically-on (Tailscale flapping known per memory); unreachable path must be tested live at least once | Confirm POS is booted + on WiFi, then `ssh pos1 "hostname"` — if UNREACHABLE, probe-pos.sh must emit `probe_status: probe_failed` with `unreachable` reason, not fail the orchestrator |
| Cloud admin gate detection | PROBE-06 | Gate state (`ADMIN_COMING_SOON_GATE=1|0`) visible only via live HEAD; mock can't reproduce 307 behavior | After probe-cloud-admin.sh writes its manifest, manually check the emitted JSON's `reachability.http_redirects` field contains the 307 redirect to `/coming-soon` (or 200 if un-gated) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (populated by planner)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (test helpers, validate-manifest-file CLI, probe-common.sh)
- [ ] No watch-mode flags (we use single-run node --test + bash)
- [ ] Feedback latency < 30s (unit) / < 180s (full+integration dry-run)
- [ ] `nyquist_compliant: true` set in frontmatter after planner populates the verification map

**Approval:** pending
