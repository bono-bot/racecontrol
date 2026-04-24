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

**Shared library:** `scripts/fleet-probe/lib/probe-common.sh` provides `write_manifest`, `sha256_of`, `iso_ist_now`, `probe_status_from_errors`, and IST-offset helpers. Use these when adding a new probe -- do NOT re-implement JSON escaping inline.

**Summary index builder:** `scripts/fleet-probe/build-meta-index.py` reads the 15 per-target manifests after the run and assembles `_meta.json` with ordered `targets[]` + `status_summary` counters.

## Troubleshooting

### "MANIFEST_TS not set" in a direct probe invocation

You invoked a probe script directly without exporting `MANIFEST_TS`. Use the orchestrator (which sets it automatically) or export manually:

```
export MANIFEST_TS=$(date -u +%Y-%m-%dT%H%M%SZ)
bash scripts/fleet-probe/probe-james.sh
```

### All pods return `probe_status: probe_failed` with `auth_gap: stale_sentry_key`

Your local `SENTRY_KEY` does not match what is in server .23's `racecontrol.toml`. Run:

```
bash scripts/deploy-preflight.sh <build-id>
```

to re-validate fleet-wide auth, then re-run the probe.

### `probe-vps.sh` returns `probe_failed` with `access_gap: RELAY_LOCAL_DOWN`

The comms-link relay on James :8766 is not listening. Verify `COMMS_PSK` is exported, then check:

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

If one resolves and the other does not, it is a DNS issue (A record missing). If both fail, it is a cert/TLS issue -- likely on Bono VPS.

## Related

- **Phase 447:** fleet-manifest schema, versioning doc, 8 example manifests
- **Phase 449:** first full-fleet probe run (execution gate, consumes this orchestrator)
- **Phase 451:** deploy graph (consumes `state/fleet-manifest/<ts>/*.json`)
- **Phase 452:** diff tool (consumes manifests + produces `DRIFT-REPORT.md`)
- **Phase 454:** scheduled + reported drift audit via cron/schtasks

---

*Shipped: Phase 448 Plan 08, 2026-04-24.*
