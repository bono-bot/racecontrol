---
phase: 448-per-target-probe-scripts
plan: 08
type: execute
wave: 5
depends_on: ["448-07"]
files_modified:
  - docs/fleet-probe/access-gaps.md
  - docs/fleet-probe/README.md
autonomous: true
requirements: [PROBE-01]
gap_closure: false

must_haves:
  truths:
    - "docs/fleet-probe/access-gaps.md exists, is git-tracked, and documents the per-target access-gap catalog that Phase 449 execution will populate with live findings"
    - "docs/fleet-probe/README.md gives staff a single entry-point explaining how to run fleet-probe + where manifests land + how to interpret probe_status"
    - "Access-gap vocabulary used by probe scripts (SSH_23, POS_SSH_DOWN, RELAY_DOWN, RELAY_LOCAL_DOWN, auth_gap: no_sentry_key|no_comms_psk|stale_sentry_key|staff_jwt_expired) is documented in one place"
    - "docs explicitly note that Server .23 SSH access audit is Phase 448's PROBE-01 deliverable; if SSH works today, this phase documents it; if SSH is gapped, this phase appends the gap row"
  artifacts:
    - path: "docs/fleet-probe/access-gaps.md"
      provides: "Catalog of known access gaps per target with status, remediation owner, remediation status"
      min_lines: 60
      contains: "## Server .23"
    - path: "docs/fleet-probe/README.md"
      provides: "Staff entry-point doc: how to run probe-all.sh, where manifests land, how to read _meta.json, the probe_status state machine, link to access-gaps.md"
      min_lines: 80
  key_links:
    - from: "docs/fleet-probe/README.md"
      to: "scripts/fleet-probe/probe-all.sh"
      via: "usage instructions referencing the orchestrator"
      pattern: "probe-all\\.sh"
    - from: "docs/fleet-probe/README.md"
      to: "docs/fleet-probe/access-gaps.md"
      via: "markdown link"
      pattern: "access-gaps\\.md"
    - from: "docs/fleet-probe/access-gaps.md"
      to: "the 11-host fleet (15 target_ids)"
      via: "one section per target"
      pattern: "## Pod|## Server|## POS|## Bono VPS|## Cloud|## Relay"

deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: none
  data_files: none
  bat_file: none
  cloud_parity: [none]
  targets: [james]
---

<objective>
Wave 5 closeout: Ship the two docs under `docs/fleet-probe/`: access-gaps.md (PROBE-01 audit trail scaffold) + README.md (staff entry-point). Both git-tracked; both referenced from the orchestrator output and the planning tree.

Purpose: PROBE-01 requires the access audit to be either fixed in-phase OR documented. Phase 449 will populate access-gaps.md with live findings; this plan ships the scaffold so there is a known landing place. README.md is the "first-time staff" doc so Uday or a future session can run the fleet probe without reading CONTEXT.md.

Output: 2 new markdown docs.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md

# Orchestrator the README references
@scripts/fleet-probe/probe-all.sh
@scripts/fleet-probe/lib/probe-common.sh
@scripts/fleet-probe/validate-manifest-file.mjs

# Access-gap vocabulary used across probes
@scripts/fleet-probe/probe-server.sh
@scripts/fleet-probe/probe-pos.sh
@scripts/fleet-probe/probe-pod.sh
@scripts/fleet-probe/probe-vps.sh
@scripts/fleet-probe/probe-relay.sh
@scripts/fleet-probe/probe-cloud-admin.sh
@scripts/fleet-probe/probe-cloud-rc.sh

# Schema reference
@schemas/fleet-manifest.schema.json

<interfaces>
**docs/fleet-probe/access-gaps.md sections (LOCKED):**
```
# Fleet Probe -- Access Gaps

Intro block: what this doc is, how it's populated, where each entry comes from.

## Server .23 (SSH)
  - Status: (to be determined on first Phase 449 run)
  - Access method: Tailscale SSH ADMIN@100.125.108.37
  - Gap IDs produced by probe-server.sh: SSH_23
  - Owner: Uday (physical access if key regen needed)

## Pods (rc-sentry /exec)
  - Status: typically OK; stale SENTRY_KEY is the usual gap
  - Gap IDs: stale_sentry_key, no_sentry_key

## POS .130
  - Gap IDs: POS_SSH_DOWN
  - Known partial: tasklist WMI-denied (per schemas/examples/pos_130.json)

## James .27 (localhost)
  - No gaps expected (always-available class)

## Bono VPS (comms-link relay)
  - Gap IDs: no_comms_psk, RELAY_DOWN, RELAY_LOCAL_DOWN

## Cloud admin (HTTPS)
  - Gap IDs: staff_jwt_expired (authed-page probe only); Coming Soon gate is intentional state

## Cloud racecontrol (HTTPS)
  - No known gaps

## Relay (composite)
  - Gap IDs: RELAY_LOCAL_DOWN (James side), vps_relay (VPS side reports connected=false)

## Gap Resolution Log
  - Empty placeholder table (Phase 449 appends rows)
```

**docs/fleet-probe/README.md sections:**
```
# Fleet Probe -- Staff Guide

## What it does
## Quick start
   bash scripts/fleet-probe/probe-all.sh           # all 15 targets
   bash scripts/fleet-probe/probe-all.sh --canary  # server + pod 8 only
   bash scripts/fleet-probe/probe-all.sh --dry-run # enumerate, no network
## Environment variables
   SENTRY_KEY, COMMS_PSK, STAFF_JWT (optional), FLEET_PROBE_VALIDATE (optional)
## Output layout
   state/fleet-manifest/<iso-ts>/<target_id>.json  (one per target)
   state/fleet-manifest/<iso-ts>/_meta.json        (summary index)
## probe_status state machine
   ok / partial / probe_failed  -- what each means
## Validating a manifest on demand
   node scripts/fleet-probe/validate-manifest-file.mjs <manifest.json>
## Access gaps
   see access-gaps.md
## Architecture
   sequential cluster + parallel pods; 8 probe types; 15 manifest files.
## Troubleshooting
   MANIFEST_TS not set -> export it
   SENTRY_KEY stale -> run deploy-preflight.sh
   COMMS_PSK missing -> export from ~/.claude-secrets (NEVER commit)
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create docs/fleet-probe/access-gaps.md scaffold</name>
  <files>docs/fleet-probe/access-gaps.md</files>
  <read_first>
    - scripts/fleet-probe/probe-server.sh (grep for SSH_23 access_gap string)
    - scripts/fleet-probe/probe-pos.sh (grep for POS_SSH_DOWN)
    - scripts/fleet-probe/probe-vps.sh (grep for RELAY_DOWN, no_comms_psk)
    - scripts/fleet-probe/probe-relay.sh (grep for RELAY_LOCAL_DOWN, vps_relay)
    - scripts/fleet-probe/probe-pod.sh (grep for stale_sentry_key, no_sentry_key)
    - scripts/fleet-probe/probe-cloud-admin.sh (grep for ADMIN_COMING_SOON_GATE, staff_jwt_expired)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md section 7 (canonical sub_probe vocabulary)
  </read_first>
  <action>
Create `docs/fleet-probe/access-gaps.md` with the following content (ASCII only, no em-dashes, no emojis):

```markdown
# Fleet Probe -- Access Gaps

**Status:** Scaffold shipped in Phase 448 Plan 08. Phase 449 populates this file with live findings from the first full-fleet probe run.

## Purpose

This document catalogs every access-gap class the Phase 448 probe scripts can surface, and tracks remediation status per real-world incident. When a probe emits `probe_status: probe_failed` with an `access_gap` field in its `probe_errors[]` entry, a row in this file records the finding.

Access-gap IDs are stable strings used by probe scripts; they become part of the `state/fleet-manifest/<iso-ts>/<target_id>.json` output and are consumed by Phase 452's diff tool.

## Access-Gap Catalog

### Server .23 (SSH)

- **Access method:** Tailscale SSH `ADMIN@100.125.108.37` (Windows OpenSSH server, default key auth)
- **Fallback:** LAN SSH `ADMIN@192.168.31.23` via Tailscale jump through Bono VPS (not yet implemented in probe-server.sh)
- **Gap IDs produced by probe-server.sh:**
  - `SSH_23` -- SSH ConnectTimeout (15s) or pubkey failure
- **Owner if gap persists:** Uday (physical access required if key regen needed)
- **Current status:** _(to be populated on first Phase 449 run)_

### Pods (rc-sentry /exec on :8091)

- **Access method:** HTTP POST to `http://<pod_ip>:8091/exec` with `X-Service-Key: $SENTRY_KEY` header
- **Pod IPs:** 192.168.31.{89,33,28,88,86,87,38,91} for pods 1..8 respectively (LAN); Tailscale fallback per CLAUDE.md
- **Gap IDs produced by probe-pod.sh:**
  - `no_sentry_key` (auth_gap) -- SENTRY_KEY env var unset in invoking shell
  - `stale_sentry_key` (auth_gap) -- 401 returned; key rotated on server but not re-synced to invoker
  - implied `probe_failed` via `connectivity` when both LAN and Tailscale are unreachable
- **Owner if gap persists:** Operator (run `deploy-preflight.sh` to resync keys)
- **Current status:** _(to be populated on first Phase 449 run)_

### POS .130

- **Access method:** Tailscale SSH `User@100.95.211.1` (default key)
- **Secondary:** HTTP `http://192.168.31.130:3300/api/health` for kiosk build fingerprint
- **Gap IDs produced by probe-pos.sh:**
  - `POS_SSH_DOWN` (access_gap) -- SSH timeout or pubkey failure
- **Known partial class:**
  - `tasklist` WMI access-denied via remote SSH context (canonical, reproduced in `schemas/examples/pos_130.json`). Probe still succeeds; status downgrades to `partial`. Fallback `tasklist /SVC /FO CSV` via rp-bono-exec is the deferred remediation.
- **Owner if gap persists:** Operator (POS physical access, check WiFi + Tailscale node)
- **Current status:** _(to be populated on first Phase 449 run)_

### James .27 (localhost)

- **Access method:** Local `tasklist`/`schtasks`/`reg query`
- **Gap IDs:** none expected -- this is the always-available class
- **Known-fail mode:** bash syntax error in probe-james.sh itself (caught by CI `bash -n` gate)
- **Current status:** OK (verified in Plan 02 smoke test)

### Bono VPS (comms-link relay)

- **Access method:** Local HTTP POST `http://localhost:8766/relay/exec/run` (comms-link relay proxies to VPS)
- **Gap IDs produced by probe-vps.sh:**
  - `no_comms_psk` (auth_gap) -- COMMS_PSK env var unset
  - `RELAY_DOWN` (access_gap) -- `/relay/health` returns `connected: false`
  - `RELAY_LOCAL_DOWN` (access_gap) -- James-side relay not listening on :8766
- **Owner if gap persists:** Operator (check `CommsLink-DaemonWatchdog` schtask on James; COMMS_PSK from secrets file)
- **Current status:** _(to be populated on first Phase 449 run)_

### Cloud admin (HTTPS)

- **Access method:** HTTPS GET `https://admin.racingpoint.cloud/api/health` (public), HEAD `/` for gate detection
- **Gap IDs produced by probe-cloud-admin.sh:**
  - `staff_jwt_expired` (auth_gap) -- only affects authed-page probe; public /api/health still captured
  - indirect `health` failure -- HTTP 5xx from /api/health
- **Intentional state:** ADMIN_COMING_SOON_GATE=1 surfaces as a `scheduled_tasks` entry (not an error). Phase 452 flags it for operator review.
- **Owner if gap persists:** Bono (cloud owner); Uday escalation for prolonged gate
- **Current status:** _(to be populated on first Phase 449 run)_

### Cloud racecontrol (HTTPS)

- **Access method:** HTTPS GET `https://racingpoint.cloud/api/v1/health` (public)
- **Gap IDs produced by probe-cloud-rc.sh:** no named access_gaps; failure modes are `connectivity`, `health` (non-200), `health_parse` (malformed), `build_id` (missing field)
- **Owner if gap persists:** Bono (cloud racecontrol redeploy if build_id stale)
- **Current status:** _(to be populated on first Phase 449 run)_

### Relay (composite: James :8766 + VPS :8765)

- **Access method:** Local HTTP GET `http://localhost:8766/relay/health` (reports both sides)
- **Gap IDs produced by probe-relay.sh:**
  - `RELAY_LOCAL_DOWN` (access_gap) -- James :8766 not listening
  - `vps_relay` (sub_probe, partial class) -- James up but VPS reports `connected: false`
- **Owner if gap persists:** Operator (James side) / Bono (VPS side)
- **Current status:** _(to be populated on first Phase 449 run)_

## Gap Resolution Log

| Date (IST) | Target | Gap ID | Discovery run_id | Remediation | Status |
|------------|--------|--------|------------------|-------------|--------|
| _(first Phase 449 run populates this table)_ | | | | | |

## Access-Gap Vocabulary -- Quick Reference

Keep this list synced with the actual strings emitted by probe scripts. When a new gap class is introduced, add the ID here AND in the relevant probe-*.sh.

| Gap ID | Field | Probe | Severity |
|--------|-------|-------|----------|
| SSH_23 | access_gap | probe-server.sh | P1 |
| POS_SSH_DOWN | access_gap | probe-pos.sh | P1 |
| RELAY_DOWN | access_gap | probe-vps.sh | P0 (blocks all VPS visibility) |
| RELAY_LOCAL_DOWN | access_gap | probe-vps.sh, probe-relay.sh | P0 (blocks all downstream: VPS, cloud health via relay) |
| no_sentry_key | auth_gap | probe-pod.sh | P2 (operator-fixable) |
| stale_sentry_key | auth_gap | probe-pod.sh | P1 |
| no_comms_psk | auth_gap | probe-vps.sh | P2 |
| staff_jwt_expired | auth_gap | probe-cloud-admin.sh | P3 (gated-page probe only) |

---

*Scaffolded: 2026-04-24 by Phase 448 Plan 08. First population: Phase 449.*
```
  </action>
  <verify>
    <automated>test -f docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Server \\.23" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Pods" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## POS" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## James" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Bono VPS" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Cloud admin" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Cloud racecontrol" docs/fleet-probe/access-gaps.md &amp;&amp; grep -c "^## Relay" docs/fleet-probe/access-gaps.md</automated>
  </verify>
  <acceptance_criteria>
    - `test -f docs/fleet-probe/access-gaps.md` exits 0
    - `grep -c "^## Server \\.23" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## Pods" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## POS" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## James" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## Bono VPS" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## Cloud admin" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## Cloud racecontrol" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "^## Relay" docs/fleet-probe/access-gaps.md` == 1
    - `grep -c "SSH_23" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "POS_SSH_DOWN" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "RELAY_DOWN" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "RELAY_LOCAL_DOWN" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "no_sentry_key" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "stale_sentry_key" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "no_comms_psk" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "staff_jwt_expired" docs/fleet-probe/access-gaps.md` >= 1
    - `grep -c "Gap Resolution Log" docs/fleet-probe/access-gaps.md` == 1
    - `wc -l docs/fleet-probe/access-gaps.md | awk '{print $1}'` >= 60
    - ASCII-only: `python3 -c "open('docs/fleet-probe/access-gaps.md','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>docs/fleet-probe/access-gaps.md scaffold exists with 8 per-target sections + vocabulary quick-reference + empty resolution log table ready for Phase 449 to populate.</done>
</task>

<task type="auto">
  <name>Task 2: Create docs/fleet-probe/README.md staff entry point</name>
  <files>docs/fleet-probe/README.md</files>
  <read_first>
    - scripts/fleet-probe/probe-all.sh (usage block at top for --help text)
    - scripts/fleet-probe/lib/probe-common.sh (env variable list)
    - schemas/fleet-manifest.schema.json (probe_status enum)
    - docs/fleet-probe/access-gaps.md (Task 1 output — link from README)
    - docs/fleet-drift/schema-versioning.md (shape/style precedent for docs/fleet-* docs)
  </read_first>
  <action>
Create `docs/fleet-probe/README.md` (ASCII only, no em-dashes, no emojis):

```markdown
# Fleet Probe -- Staff Guide

**Phase 448 deliverable.** Single-invocation read-only snapshot of every deployable surface in the Racing Point fleet, emitted as SCHEMA-01-compliant JSON manifests.

## What it does

`bash scripts/fleet-probe/probe-all.sh` collects a point-in-time snapshot of:

- Binary SHA256 + build_id on Server .23, all 8 pods, POS kiosk, and both cloud services
- Running processes, scheduled tasks, and autostart entries on every Windows target
- Config file SHA256 (`racecontrol.toml` 3-way drift on Server .23; `rc-agent.toml` on pods; `/root/racecontrol.toml` on Bono VPS)
- Env-var-names hash (values NEVER captured -- security boundary)
- Last-deploy timestamp from SWAPLOG.md (Server .23) or null where unknowable
- Comms-link relay connect state (James <-> VPS)

Output: one JSON file per target under `state/fleet-manifest/<iso-ts>/<target_id>.json`, plus `_meta.json` summary index. Manifests never overwrite across runs -- each MANIFEST_TS is a fresh directory.

## Quick start

```
# Full run -- 15 targets, ~60-120s
bash scripts/fleet-probe/probe-all.sh

# Canary -- server .23 + pod 8 only, ~15s
bash scripts/fleet-probe/probe-all.sh --canary

# Dry run -- enumerate 15 targets, no network calls
bash scripts/fleet-probe/probe-all.sh --dry-run
```

## Environment variables

Pre-export these in your shell before invoking the orchestrator. The orchestrator does NOT auto-source `.env.local` or any secrets file.

| Variable | Required by | Purpose |
|----------|-------------|---------|
| `SENTRY_KEY` (or `RCAGENT_SERVICE_KEY`) | `probe-pod.sh` | `X-Service-Key` header for rc-sentry :8091/exec |
| `COMMS_PSK` | `probe-vps.sh` | Auth header for comms-link relay /exec/run |
| `STAFF_JWT` | `probe-cloud-admin.sh` (authed-page probe only) | Bearer token for gated cloud-admin routes |
| `FLEET_PROBE_VALIDATE=1` | optional | Run ajv schema validation on every write (slower, catches bugs) |
| `MANIFEST_TS` | optional | Override timestamp (same MANIFEST_TS -> same dir, supports idempotent re-run) |

Missing `SENTRY_KEY` makes pod probes emit `probe_failed` with `auth_gap: no_sentry_key` -- the orchestrator keeps running; only pod manifests degrade.

## Output layout

```
state/fleet-manifest/
  2026-04-24T103000Z/
    server_23.json
    pod_1.json
    ...
    pod_8.json
    pos_130.json
    james_27.json
    bono_vps.json
    cloud_admin.json
    cloud_racecontrol.json
    relay_james.json
    _meta.json          # summary index with status_summary counts
```

`state/fleet-manifest/` is git-ignored per Phase 447 Plan 01 -- manifests never pollute version control.

## probe_status state machine

Every manifest has a `probe_status` field with exactly one of these values:

| Status | Meaning | Manifest data |
|--------|---------|---------------|
| `ok` | All sub-probes succeeded | All fields populated |
| `partial` | Reachable but >=1 sub-probe failed | `probe_errors[]` enumerates which sub-probes failed; populated fields still valid |
| `probe_failed` | Target unreachable (connect-stage failure) | `binary_sha256: {}`, `build_id: null`, `config_hash: {}`, `running_procs: []`, `scheduled_tasks: []`, `autostart_entries: []`; `probe_errors[0]` carries connect reason and often an `access_gap` tag |

Rule: a probe_failed target still produces a manifest file -- never a missing file. The orchestrator's completeness contract is "15 manifest files per run, always".

## Validating a manifest on demand

```
node scripts/fleet-probe/validate-manifest-file.mjs state/fleet-manifest/<ts>/<target>.json
```

Exit 0 = valid; exit 1 = invalid (ajv prints errors on stderr); exit 2 = file IO error.

## Access gaps

When a probe emits `probe_status: probe_failed` with an `access_gap` field, the incident is catalogued in `docs/fleet-probe/access-gaps.md` with remediation owner and status. Read that file before asking "why did pod X return probe_failed again?"

See: [access-gaps.md](./access-gaps.md)

## Architecture

**Sequential cluster (7 probes):** server_23, pos_130, james_27, bono_vps, cloud_admin, cloud_racecontrol, relay_james. Run in order to avoid auth-rate-limit issues and to keep the manifest-write serial.

**Parallel pods (1..8):** All 8 pod probes fan out via `&` + `wait`. Pod probes only touch rc-sentry :8091/exec + rc-agent :8090/health; no inter-pod dependencies.

**Shared library:** `scripts/fleet-probe/lib/probe-common.sh` provides `write_manifest`, `sha256_of`, `iso_ist_now`, `probe_status_from_errors`, and IST-offset helpers.

**Summary index builder:** `scripts/fleet-probe/build-meta-index.py` reads the 15 per-target manifests after the run and assembles `_meta.json` with ordered `targets[]` + `status_summary` counters.

## Troubleshooting

### "MANIFEST_TS not set" in a direct probe invocation

You invoked a probe script directly without exporting `MANIFEST_TS`. Use the orchestrator (which sets it automatically) or export manually:

```
export MANIFEST_TS=$(date -u +%Y-%m-%dT%H%M%SZ)
bash scripts/fleet-probe/probe-james.sh
```

### All pods return `probe_status: probe_failed` with `auth_gap: stale_sentry_key`

Your local `SENTRY_KEY` does not match what's in server .23's `racecontrol.toml`. Run:

```
bash scripts/deploy-preflight.sh <build-id>
```

to re-validate fleet-wide auth, then re-run the probe.

### `probe-vps.sh` returns `probe_failed` with `access_gap: RELAY_LOCAL_DOWN`

The comms-link relay on James :8766 is not listening. Check:

```
schtasks /Query /TN "CommsLink-DaemonWatchdog"
curl -s http://localhost:8766/relay/health
```

### All cloud probes return probe_failed

DNS or TLS failure. Check:

```
curl -sI https://racingpoint.cloud/api/v1/health
curl -sI https://admin.racingpoint.cloud/api/health
```

If one resolves and the other doesn't, it's a DNS issue (A record missing). If both fail, it's a cert/TLS issue -- likely on Bono VPS.

## Related

- **Phase 447:** fleet-manifest schema, versioning doc, 8 example manifests
- **Phase 449:** first full-fleet probe run (execution gate, consumes this orchestrator)
- **Phase 451:** deploy graph (consumes `state/fleet-manifest/<ts>/*.json`)
- **Phase 452:** diff tool (consumes manifests + produces `DRIFT-REPORT.md`)
- **Phase 454:** scheduled + reported drift audit via cron/schtasks

---

*Shipped: Phase 448 Plan 08, 2026-04-24.*
```
  </action>
  <verify>
    <automated>test -f docs/fleet-probe/README.md &amp;&amp; grep -c "probe-all.sh" docs/fleet-probe/README.md &amp;&amp; grep -c "access-gaps.md" docs/fleet-probe/README.md &amp;&amp; grep -c "probe_status" docs/fleet-probe/README.md &amp;&amp; grep -c "Phase 449" docs/fleet-probe/README.md</automated>
  </verify>
  <acceptance_criteria>
    - `test -f docs/fleet-probe/README.md` exits 0
    - `grep -c "probe-all.sh" docs/fleet-probe/README.md` >= 3
    - `grep -c "access-gaps.md" docs/fleet-probe/README.md` >= 1
    - `grep -c "probe_status" docs/fleet-probe/README.md` >= 3
    - `grep -c "SENTRY_KEY" docs/fleet-probe/README.md` >= 2
    - `grep -c "COMMS_PSK" docs/fleet-probe/README.md` >= 2
    - `grep -c "STAFF_JWT" docs/fleet-probe/README.md` >= 1
    - `grep -c "Phase 447" docs/fleet-probe/README.md` >= 1
    - `grep -c "Phase 449" docs/fleet-probe/README.md` >= 1
    - `grep -c "Phase 452" docs/fleet-probe/README.md` >= 1
    - `grep -c "validate-manifest-file.mjs" docs/fleet-probe/README.md` >= 1
    - `grep -c "_meta.json" docs/fleet-probe/README.md` >= 2
    - `grep -c "Troubleshooting" docs/fleet-probe/README.md` == 1
    - `wc -l docs/fleet-probe/README.md | awk '{print $1}'` >= 80
    - ASCII-only: `python3 -c "open('docs/fleet-probe/README.md','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>Staff entry-point doc exists with quick-start, env var table, output layout, probe_status state machine, architecture summary, troubleshooting section, and cross-references to access-gaps.md + sibling Phases 447/449/451/452/454.</done>
</task>

</tasks>

<verification>
- `test -f docs/fleet-probe/access-gaps.md` and `test -f docs/fleet-probe/README.md` both pass
- `grep -c "access-gaps.md" docs/fleet-probe/README.md` >= 1 (README links to sibling doc)
- `npm run test:fleet-probe` still exits 0 (Wave 4 regression)
- `npm run test:fleet-drift` still exits 0 (Phase 447 regression)
- `bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l` == 15 (Wave 1 regression)
</verification>

<success_criteria>
- Phase 448's PROBE-01 access-audit deliverable is met: either gap is fixed in-phase (Plan 03 SSH verification), or a documented landing place exists for future findings (this plan)
- Staff can read ONE file (docs/fleet-probe/README.md) to understand how to run the fleet probe end-to-end
- Handoff to Phase 449 is clean: access-gaps.md has section scaffold + resolution log table ready for population
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-08-SUMMARY.md` with:
- Files created (2 docs)
- Grep counts for every required string in both docs
- Confirmation that Plan 07 regression tests still pass
- Phase 448 closeout checklist:
  - [ ] 8 probe scripts + 1 orchestrator + 1 shared lib + 1 validator + 1 meta builder = 12 scripts
  - [ ] 10 unit/integration tests green (npm run test:fleet-probe)
  - [ ] 2 docs shipped under docs/fleet-probe/
  - [ ] PROBE-01..09 all addressed (see summary requirements map)
  - [ ] state/fleet-manifest/ still gitignored (Phase 447 Plan 01 regression preserved)
- Handoff to Phase 449: "Run `bash scripts/fleet-probe/probe-all.sh` with fresh SENTRY_KEY + COMMS_PSK exported. Validate all 15 manifests pass ajv. Append live findings to docs/fleet-probe/access-gaps.md."
</output>
