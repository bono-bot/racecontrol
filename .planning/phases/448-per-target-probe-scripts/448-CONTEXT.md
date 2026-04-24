# Phase 448: Per-Target Probe Scripts - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped per PACT trust-default for read-only infrastructure work)

<domain>
## Phase Boundary

Ship 8 probe scripts (one per target class) + `probe-all.sh` orchestrator that collectively enumerate the 11-host fleet and write SCHEMA-01-compliant manifests to `state/fleet-manifest/<iso-ts>/<target_id>.json` + `_meta.json`. Every probe is READ-ONLY against its target — no config writes, no deploys, no service restarts. Probes use existing access paths (SSH, rp-bono-exec relay, HTTP/JWT, Tailscale) — no new agent install on any target.

**What this phase delivers:**
- `scripts/fleet-probe/probe-server.sh` — Server .23 via Tailscale SSH (`ssh ADMIN@100.125.108.37`)
- `scripts/fleet-probe/probe-pod.sh <pod_N>` — rp-bono-exec relay + HTTP `:8090/debug` + `/health` + HTTP `:8091/debug` (rc-sentry)
- `scripts/fleet-probe/probe-pos.sh` — POS .130 via Tailscale SSH (`pos1 / 100.95.211.1`)
- `scripts/fleet-probe/probe-james.sh` — James .27 localhost (no SSH needed — local)
- `scripts/fleet-probe/probe-vps.sh` — Bono VPS via comms-link relay `POST /relay/exec/run`
- `scripts/fleet-probe/probe-cloud-admin.sh` — `admin.racingpoint.cloud` via HTTPS + staff JWT
- `scripts/fleet-probe/probe-cloud-rc.sh` — cloud racecontrol (VPS :8080) via HTTP
- `scripts/fleet-probe/probe-relay.sh` — comms-link relay (James :8766 + VPS :8765)
- `scripts/fleet-probe/probe-all.sh` — orchestrator, iterates all 11 targets, emits `state/fleet-manifest/<iso-ts>/` directory
- `scripts/fleet-probe/lib/probe-common.sh` — shared helpers (manifest writers, SHA256 utilities, timestamp generation, JSON assembly)
- Unit tests under `tests/fleet-probe/` that mock target responses and verify manifest shape against `schemas/fleet-manifest.schema.json`

**What this phase does NOT deliver:**
- Live fleet probing with real results (Phase 449 executes the probes against live fleet)
- Deploy-ledger write side (Phase 455 LIFECYCLE-02)
- Build/deploy graph generation (Phases 450-451)
- Any diff/report tooling (Phase 452+)
- Cross-repo probe surfaces outside the 11 named targets (cloud apps + comms-link are in-scope; any NEW target registry is deferred per CONTEXT.md of Phase 447)

</domain>

<decisions>
## Implementation Decisions

### PACT Trust-Default (no formal PACT vote — single-AI in-domain operational work)

Per `feedback_pact_protocol.md`: "Single-AI in-domain operational work (FYI only)" is trust-default. Writing read-only probe scripts that use existing access paths is in-domain for James. Decisions below are logged for transparency — not gated on Bono consent.

### Script language + shell
- **Bash (Git Bash on Windows)** — consistent with existing `scripts/deploy/*.sh` pattern in repo. No PowerShell (fragile quoting via SSH per memory). No Node (heavier runtime).
- **ASCII-only** (standing rule `feedback_ascii_only_script_constraint.md`). No em-dashes or smart quotes in script source.
- **`set -eo pipefail`** at the top of every script.

### Probe invocation contract (every probe script)
Each `probe-*.sh` script takes no args (except pod.sh which takes `<pod_N>`). Emits:
- **stdout:** single-line status `{target_id, probe_status, duration_ms, errors_count}` (JSON)
- **exit code:** always 0 unless the script itself crashes (target unreachable → `probe_status: probe_failed` manifest row, NOT script failure)
- **side effect:** writes `state/fleet-manifest/<MANIFEST_TS>/<target_id>.json` where `MANIFEST_TS` is passed via env var

### Orchestrator contract (`probe-all.sh`)
- Generates ONE `MANIFEST_TS=$(date -u +%Y-%m-%dT%H%M%SZ)` at start, exports to children
- Runs probes in parallel where safe (pods 1-8 can run concurrently; server + POS + james + vps + cloud-admin + cloud-rc + relay sequential to avoid auth rate-limit issues)
- Parallelism: use `&` + `wait` for pod fanout
- Writes `_meta.json` summary index after all probes return
- Exits 0 if orchestrator completes (even if individual probes returned probe_failed)

### Error handling — the "probe_failed" class
Every probe script handles two failure modes:
- **Unreachable target** (SSH timeout, HTTP timeout, relay unreachable) → emit `{probe_status: "probe_failed", probe_errors: [{stage: "connect", detail: "timeout after 15s"}]}` manifest
- **Reachable but partial** (SSH ok but `tasklist` errored, HTTP ok but `/debug` 401) → emit `{probe_status: "partial", probe_errors: [{stage: "tasklist", detail: "..."}]}` manifest with whatever sub-probes succeeded

Neither case crashes the script. Orchestrator continues to next target.

### SSH / auth credentials — where they live
- Server .23 SSH: Tailscale IP `ADMIN@100.125.108.37` (per CLAUDE.md). Uses default SSH key; if key-auth fails, probe marks partial and logs in errors.
- Pods 1-8: rp-bono-exec relay `POST http://localhost:8766/relay/exec/run` with `rp-bono-exec` MCP's existing auth (no new creds)
- POS: Tailscale `ssh User@100.95.211.1` (per reference_pos_chrome_kiosk.md — POS has Tailscale node `pos1`)
- Bono VPS: comms-link relay `POST http://localhost:8766/relay/exec/run` — existing PSK via `COMMS_PSK` env var
- Cloud admin: HTTPS + `STAFF_JWT` env var (probes read from `~/.claude-secrets/` or equivalent — NEVER hardcoded)
- Cloud racecontrol: public HTTP `/api/v1/health` — no auth needed for build_id
- Comms-link relay: public `/relay/health` on both nodes — no auth needed

### Probe data collection — what each probe captures
Per `schemas/fleet-manifest.schema.json` (locked in Phase 447):
- `binary_sha256` — use `sha256sum`, `Get-FileHash`, or `shasum -a 256` depending on host OS. For Windows targets: `powershell -Command "Get-FileHash ... -Algorithm SHA256"`.
- `build_id` — parse from `/health` endpoint for server + cloud-rc, from `/debug` endpoint for pods, from `Get-ItemPropertyValue` registry or `file version` for binaries without embedded build_id
- `config_hash` — SHA256 of `racecontrol.toml`, `rc-agent.toml`, etc. in-place
- `running_procs` — Windows: `tasklist /V /FO CSV`; Linux: `ps -eo comm,pid,args`. Hash cmdline to `cmdline_hash` — DO NOT capture raw cmdline (may contain secrets).
- `scheduled_tasks` — Windows: `schtasks /Query /FO LIST /V`; Linux: `crontab -l` + `systemctl list-timers`
- `autostart_entries` — Windows: 3 queries (`reg query HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`, HKCU same, Startup folder dir listing); Linux: `systemctl list-unit-files | grep enabled`
- `env_vars_hash` — `env | sort | awk -F= '{print $1}' | sha256sum` (NAMES only, never values — SECURITY BOUNDARY)
- `last_deploy_ts` — For server binary: parse from `SWAPLOG.md` last row. For pods: parse from `last_deploy` field in rc-agent config or `filemtime` on binary. Null where unknowable.

### Shared library (`lib/probe-common.sh`)
- `json_escape(str)` — escape string for JSON embed (handles quotes, backslash, unicode)
- `write_manifest(target_id, manifest_json)` — pretty-print JSON to `state/fleet-manifest/$MANIFEST_TS/$target_id.json`
- `sha256_of(filepath_or_stdin)` — cross-platform SHA256
- `iso_ist_now()` — ISO-8601 with `+05:30` offset (bash-only per CLAUDE.md timezone rule)
- `probe_status_from_errors(errors_array)` — if empty → "ok", if ≥1 connect-stage error → "probe_failed", otherwise "partial"

### What PROBE-01's "access audit" clause means
SCHEMA PROBE-01 says "Access audit + any gaps fixed (or documented) as part of this requirement." Interpretation:
- probe-server.sh attempts SSH to `ADMIN@100.125.108.37` with default key
- If it works → probe proceeds normally (no audit action needed)
- If it fails → probe_failed is emitted AND `state/fleet-manifest/<ts>/server_23.json` includes `probe_errors[].access_gap` field pointing to this phase; additional doc at `docs/fleet-drift/server-23-access-audit.md` captures what was tried and what needs fixing (Uday follow-up)

No PACT vote needed — access audit is in-domain diagnostic work.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `schemas/fleet-manifest.schema.json` — LOCKED in Phase 447; probes MUST emit conforming JSON
- `schemas/examples/*.json` — 8 reference manifests; probe scripts effectively "fill in" the `"TBD_LIVE"` values in each example
- `tests/fleet-drift/validate-manifest.test.mjs` — existing ajv validator; Phase 448 unit tests can reuse it to assert probe output shape
- `scripts/deploy/deploy-server.sh` (900+ lines) — precedent for Tailscale-SSH-from-James pattern + schtasks + exec error handling. Read before writing probe-server.sh.
- `scripts/deploy/deploy-pod.sh` — precedent for rc-sentry /exec endpoint usage. Read before probe-pod.sh.
- rp-bono-exec MCP — existing tool for pod exec. `curl POST http://localhost:8766/relay/exec/run -d '{"command":"..."}' ` pattern (per CLAUDE.md).
- comms-link relay `/relay/exec/run` — existing HTTP exec channel for Bono VPS (per CLAUDE.md).
- `SWAPLOG.md` at repo root — source of truth for Server .23 binary deploy history; probe-server.sh reads last row for `last_deploy_ts`.

### Established Patterns
- **Sequential vs parallel probes** — deploy-pod.sh runs pod-by-pod sequential for atomic safety. Read-only probes can fan out 8 pods in parallel — no write contention risk.
- **Timeout hygiene** — `ssh -o ConnectTimeout=15 -o ServerAliveInterval=5` pattern from deploy-server.sh.
- **cmd.exe hostility** (standing rule) — probes using rc-sentry `/exec` endpoint must avoid `$` and nested quotes in remote commands. Use JSON payload files + `curl -d @file.json`.
- **Windows SSH** — `2>nul` not `2>/dev/null` in remote commands; avoid Unix pipes to remote Windows hosts.
- **Session 1 requirement** — rc-agent tasklist checks must verify `Session#=Console` not `Services`. Use `tasklist /V /FO CSV | findstr /I rc-agent` pattern.

### Integration Points
- Phase 449 (First Full-Fleet Probe Run) is the EXECUTION consumer — runs `scripts/fleet-probe/probe-all.sh` against live fleet. If Phase 449 discovers access gaps, they get surfaced back to Phase 448 via `/gsd:plan-phase 448 --gaps`.
- Phase 451 (Deploy Graph) is the DATA consumer — reads `state/fleet-manifest/<ts>/*.json` and graphifies.
- `.gitignore` already excludes `state/fleet-manifest/*/` runtime output (from Phase 447) — probes write there freely without git noise.

</code_context>

<specifics>
## Specific Ideas

- **Probe timeouts:** 15s connect, 30s total per sub-probe, 120s orchestrator total. Fail-fast on connect-level errors to avoid 10-minute orchestrator runs when one target is down.
- **Idempotent re-runs:** Orchestrator with the same `MANIFEST_TS` env var should be safely re-runnable — overwriting the directory is intentional, supports "one probe failed, re-run just that target" flow.
- **Manifest validation on write:** `write_manifest()` in probe-common should optionally call `node scripts/fleet-probe/validate.mjs $file` (wrapping the Phase 447 ajv harness) — fail the single probe if its output doesn't validate, don't silently write corrupt manifests. Gate behind `FLEET_PROBE_VALIDATE=1` env var so default is off (speed).
- **Dry-run mode:** `probe-all.sh --dry-run` prints what it WOULD do without actually making network calls. Useful for CI + Phase 449 pre-flight.
- **Partial probes dominate the risk surface:** Bad `tasklist` on a pod, missing SWAPLOG row on server, stale JWT on cloud admin — all produce `probe_status: "partial"` + `probe_errors[]`. Focus test fixtures on partial cases.
- **No silent `.env.local` reads:** probe scripts read ONLY from env vars that are ALREADY set in the invoking shell. Don't auto-source `.env.local` or `~/.claude-secrets/*.env`. If `STAFF_JWT` is unset → cloud-admin probe marks partial + probe_errors gets `auth_gap: no_staff_jwt`.

</specifics>

<deferred>
## Deferred Ideas

- **Signed/attested manifests** — out of scope for v53.0. v53.1+ if threat model changes.
- **Historical diff (manifest N vs N-1)** — time-series retention; v53.1 per Phase 447 deferred.
- **Probe-from-MI** (MI triggers on-demand probes when it sees a symptom cluster it can't explain) — deferred to Phase 455 MI integration.
- **Chrome DevTools probe for kiosk state** — Phase 448 captures POS BillingDashboard via `ps`/`tasklist`, not via browser runtime state. Browser-state probing deferred to v53.1.
- **Target registry (`schemas/fleet-targets.json`)** — still deferred. 8-target enum lives in each probe script as a shell CASE. If Phase 448 finds this brittle, elevate to a formal registry via `/gsd:plan-phase 448 --gaps`.
- **Automated Server .23 SSH access fix** — if the access audit discovers a gap, Phase 448 DOCUMENTS it; remediation is a Uday follow-up (may need physical access or key regen).

</deferred>

---

*Phase: 448-per-target-probe-scripts*
*Context gathered: 2026-04-24 via PACT trust-default path (single-AI in-domain operational work). REQ-IDs: PROBE-01..09 in `.planning/REQUIREMENTS-v53.md`.*
