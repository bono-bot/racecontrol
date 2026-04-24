# Phase 448: Per-Target Probe Scripts — Research

**Researched:** 2026-04-24
**Domain:** Fleet-wide read-only probe orchestration emitting SCHEMA-01-compliant JSON manifests
**Confidence:** HIGH (built entirely on existing scripts, endpoints, and access paths already in production use)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **Shell + language:** Bash (Git Bash on Windows). No PowerShell for probe logic (fragile quoting via SSH). No Node.
- **ASCII-only** source — `feedback_ascii_only_script_constraint.md`. No em-dashes, no smart quotes.
- `set -eo pipefail` at the top of every script.
- **Probe invocation contract:**
  - stdout: single JSON line `{target_id, probe_status, duration_ms, errors_count}`
  - exit code: always 0 unless the script itself crashes (target unreachable => `probe_status: probe_failed` manifest, NOT script failure)
  - side effect: writes `state/fleet-manifest/$MANIFEST_TS/$target_id.json` where `MANIFEST_TS` is an env var exported by orchestrator
- **Orchestrator contract (`probe-all.sh`):** one `MANIFEST_TS=$(date -u +%Y-%m-%dT%H%M%SZ)` at start; pods 1-8 in parallel; server + POS + james + vps + cloud-admin + cloud-rc + relay sequential; `_meta.json` written after; exit 0 even if individual probes marked `probe_failed`.
- **Error classes:** `probe_failed` (connect-stage failure) vs `partial` (reachable but >=1 sub-probe failed).
- **Auth sources (read from env only — never auto-source .env.local):**
  - Server .23: SSH default key at `ADMIN@100.125.108.37` (Tailscale)
  - Pods: `X-Service-Key` header with `SENTRY_KEY` env var (or `RCAGENT_SERVICE_KEY`) against rc-sentry :8091 (read) + rp-bono-exec relay for sensitive shell
  - POS: SSH `User@100.95.211.1`
  - Bono VPS: comms-link relay `POST http://localhost:8766/relay/exec/run` with `COMMS_PSK`
  - Cloud admin: HTTPS + `STAFF_JWT`
  - Cloud racecontrol: public `/api/v1/health` (no auth)
  - Relay: public `/relay/health` on both nodes
- **Schema versioning decree:** probes emit `schema_version: "1.0"` exactly; unknown-field forward-compat is on the READER side (ajv validator from 447-03), not the writer side.
- **Security boundary:** `env_vars_hash` hashes NAMES only (`env | sort | awk -F= '{print $1}' | sha256sum`). `cmdline_hash` hashes full cmdline — NEVER capture raw cmdline.
- **Shared lib (`lib/probe-common.sh`):** `json_escape`, `write_manifest`, `sha256_of`, `iso_ist_now`, `probe_status_from_errors`.
- **Timeouts:** 15s connect, 30s total per sub-probe, 120s orchestrator total.
- **Idempotent re-runs:** same `MANIFEST_TS` => overwrite directory.
- **Dry-run mode:** `probe-all.sh --dry-run` enumerates targets without making network calls.
- **PROBE-01 access-audit:** probe-server.sh attempts SSH; on failure => `probe_failed` + `docs/fleet-probe/access-gaps.md` + `probe_errors[].access_gap` field. In-phase remediation only if trivial; otherwise Uday follow-up.

### Claude's Discretion

- Exact Bash function signatures + internal implementation of `lib/probe-common.sh` (as long as contract methods exist).
- Test harness structure under `tests/fleet-probe/` — unit (mock-driven) vs integration (real network) split.
- Whether probe-pod.sh should support Pod 8 canary flag (`--pod 8 --canary`) — PACT-012 Uday-arbitrated Option A recommends supporting canary usage, so YES include.
- Exactly how `probe-server.sh` performs the Q5 three-way drift diff (D:\racecontrol.toml vs SSH-read C:\RacingPoint\racecontrol.toml vs git); my recommendation is: emit the three SHA256s inside `config_hash` with distinct keys (`racecontrol.toml`, `racecontrol.toml@server_live`, `racecontrol.toml@james_proxy`) and let Phase 452 diff-tool surface the three-way divergence — keep probe thin.
- Failure-mode error messages and diagnostic hints (free-form strings in `probe_errors[].error`).

### Deferred Ideas (OUT OF SCOPE)

- Signed/attested manifests — v53.1+.
- Historical diff (manifest N vs N-1) — v53.1.
- MI-triggered probes — Phase 455.
- Chrome DevTools probe of kiosk browser state — v53.1.
- Formal `schemas/fleet-targets.json` registry — 11-target list lives in shell CASE; elevate only if brittle.
- Automated Server .23 SSH remediation — docs only.
- Cross-repo probe surfaces outside the 11 named targets.
- Build/deploy graph generation (Phases 450-451), diff tooling (Phase 452), lifecycle write-back (Phase 455).

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROBE-01 | Probe Server .23 via SSH + capture normalized manifest; access audit + doc gaps | SSH to `ADMIN@100.125.108.37` verified this session (EXIT=0). `deploy-server.sh` + `deploy-preflight.sh` are the template: `ssh -o ConnectTimeout=5 ... findstr`. SWAPLOG.md last row gives `last_deploy_ts`. |
| PROBE-02 | Probe Pods 1-8 via rp-bono-exec relay + HTTP `/debug` + `/health` | `deploy-pod.sh` + `deploy-preflight.sh` already use `http://{pod}:8091/exec` with `X-Service-Key`. Pod 8 SSH verified this session. Pod 1-8 IPs in CLAUDE.md network map. |
| PROBE-03 | Probe POS .130 via Tailscale SSH; kiosk Chrome + BillingDashboard state | POS Tailscale IP `100.95.211.1` (node `pos1`). Reuse `deploy-preflight.sh`-style SSH. Existing `pos_130.json` example already demonstrates `partial` + `probe_errors[]` (tasklist WMI denial). |
| PROBE-04 | Probe James .27 localhost — comms-link relay, hooks, MCP servers | Pure local `ps`/`tasklist`/`reg query`/`schtasks` — no network. Template: `audit/autostart-surfaces.sh` already enumerates systemd/cron/pm2 on Linux — needs Windows sibling. |
| PROBE-05 | Probe Bono VPS via comms-link relay `/relay/exec/run` | Pattern in `auto-detect.sh:316`, `bono-server-monitor.sh:180`, `env-drift-check.sh:36`. `COMMS_PSK` env. Linux `ps`/`systemctl`/`pm2 jlist`. `autostart-surfaces.sh` is the near-verbatim probe — wrap its JSON output into SCHEMA-01 shape. |
| PROBE-06 | Probe cloud admin via HTTPS + staff JWT | `GET https://admin.racingpoint.cloud/api/health` returns `build_id`, `git_commit`, `pages_missing` (per STATE.md Phase 445 completion evidence). `ADMIN_COMING_SOON_GATE` flag documented in memory. |
| PROBE-07 | Probe cloud racecontrol (Bono VPS :8080) | `GET /api/v1/health` returns `build_id`. `scripts/auto-detect.sh:bono_status` is the template. `racingpoint.cloud` resolves via Bono VPS. |
| PROBE-08 | Probe comms-link relay (James :8766 + VPS :8765) | `GET /relay/health` returns `{connected: bool, ...}`. `auto-detect.sh:304` is the template. Composite: single manifest `target_id: relay_james` with both endpoints inside `running_procs` + per-endpoint status encoded in `probe_errors[]` when one side is down. |
| PROBE-09 | `probe-all.sh` orchestrates all 8 probe types (11 targets) into one `MANIFEST_TS` dir | Parallelism: `&` + `wait` for pods; sequential for auth-rate-limited targets. `_meta.json` shape locked in 447-02 example. |

</phase_requirements>

## Summary

Phase 448 is almost entirely an **assembly job over existing access paths and existing scripts** — every auth credential, every HTTP endpoint, every SSH target, and every JSON-shaping helper already exists somewhere in the repo. The work is (1) compose them into 8 per-target probe scripts, (2) add a thin shared lib (`probe-common.sh`) for hash/JSON helpers, (3) add an orchestrator that spawns them into one timestamped directory, (4) unit-test each probe by mocking the target response and asserting the emitted manifest passes the Phase 447 ajv validator. No new cross-system bridges; no MMA required (single-AI, in-domain, read-only — PACT trust-default).

**Primary recommendation:** Pattern every probe on two existing scripts — `scripts/deploy-preflight.sh` for the auth/timeout/target-enumeration pattern, and `scripts/audit/autostart-surfaces.sh` for JSON-assembly-via-shell-heredoc + `python3 -m json.tool` validation — then wrap each invocation in a `write_manifest` call that pretty-prints into `state/fleet-manifest/$MANIFEST_TS/$target_id.json` and invokes Phase 447's ajv validator gated behind `FLEET_PROBE_VALIDATE=1`.

## Access Path Inventory

Per-target table. **11 targets** across 8 probe script classes (pods 1-8 share one probe). Confidence HIGH on every row — each path was either verified live this session or is in active production use by existing scripts.

| # | target_id | host / IP | role | Primary Access | Auth | Secondary | Blast Radius | Status |
|---|-----------|-----------|------|----------------|------|-----------|--------------|--------|
| 1 | `server_23` | `Racing-Point-Server` / `192.168.31.23` (LAN), `100.125.108.37` (Tailscale) | `server` | Tailscale SSH `ADMIN@100.125.108.37` | default SSH key (Windows OpenSSH) | HTTP `GET :8080/api/v1/health` for `build_id` | READ-ONLY — `tasklist`, `schtasks /Query`, `reg query`, `Get-FileHash`, `findstr` on toml | SSH verified EXIT=0 this session (2026-04-24) |
| 2 | `pod_1` | `RCPOD-1` / `192.168.31.89` | `pod` | rc-sentry `POST :8091/exec` + HTTP `GET :8090/debug` + `GET :8090/health` | `X-Service-Key: $SENTRY_KEY` | Tailscale SSH `User@100.92.122.89` fallback | READ-ONLY exec commands (`tasklist`, `schtasks /Query`, `reg query`). rc-sentry is the exec handler — do not `taskkill` rc-sentry itself. | Pattern in use by `deploy-pod.sh`, `deploy-preflight.sh` |
| 3 | `pod_2` | `192.168.31.33` / TS `100.105.93.108` | `pod` | Same as pod_1 | Same | TS SSH | Same | |
| 4 | `pod_3` | `192.168.31.28` / TS `100.69.231.26` | `pod` | Same | Same | TS SSH | Same | |
| 5 | `pod_4` | `192.168.31.88` / TS `100.75.45.10` | `pod` | Same | Same | TS SSH | Same | |
| 6 | `pod_5` | `192.168.31.86` / TS `100.110.133.87` | `pod` | Same | Same | TS SSH | Same | |
| 7 | `pod_6` | `192.168.31.87` / TS `100.127.149.17` | `pod` | Same | Same | TS SSH | Same | |
| 8 | `pod_7` | `192.168.31.38` / TS `100.82.196.28` | `pod` | Same | Same | TS SSH | Same | |
| 9 | `pod_8` | `RCPOD-8` / `192.168.31.91` / TS `100.98.67.67` | `pod` | Same; **canary default** | Same | TS SSH | Same. PACT-012 Option A: support `--canary` flag to run probe-pod.sh only on pod_8. | SSH verified EXIT=0 this session (`sim8\user`) |
| 10 | `pos_130` | `POS1` / `192.168.31.130` / TS `100.95.211.1` | `pos` | Tailscale SSH `User@100.95.211.1` | default SSH key | — | READ-ONLY. NOTE: `tasklist` over SSH can hit WMI access-denied (see `pos_130.json` example demonstrating `partial` class). Fall back to `tasklist /SVC /FO CSV` if `/V` fails. | POS known online; `fleet-sync-status.sh:267` already hits `:3300/api/health` |
| 11 | `james_27` | `JAMES-PC` / `192.168.31.27` | `james` | **Localhost — no network** | none (probe runs on James itself) | — | READ-ONLY. Probe executes `tasklist /V /FO CSV`, `schtasks /Query /V /FO LIST`, `reg query`, `dir C:\Users\bono\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup`, `pm2 list --no-color` (if installed). | Always available — probe never emits `probe_failed` for james_27 |
| 12 | `bono_vps` | `srv1422716.hstgr.cloud` (approx `45.11.110.250`) | `vps` | comms-link relay `POST http://localhost:8766/relay/exec/run` | `COMMS_PSK` env | SSH `root@100.70.177.44` (Tailscale) fallback | READ-ONLY `ps -eo`, `systemctl list-unit-files`, `pm2 jlist`, `sha256sum`, `cat /root/racecontrol.toml | sha256sum`. `autostart-surfaces.sh` already does 90% of this. | Relay endpoint in active use by `auto-detect.sh`, `bono-server-monitor.sh`, `self-patch.sh` |
| 13 | `cloud_admin` | `admin.racingpoint.cloud` / `45.11.110.250` | `cloud_admin` | HTTPS `GET /api/health` | `Authorization: Bearer $STAFF_JWT` (for authed routes); /api/health is public | — | READ-ONLY. Public `/api/health` returns `{healthy, build_id, git_commit, pages_missing}`. Gated-page check via STAFF_JWT-authed GET to surface `ADMIN_COMING_SOON_GATE` state. | Verified post-Phase 445 live (STATE.md:49) |
| 14 | `cloud_racecontrol` | `racingpoint.cloud` (Bono VPS :8080 via pm2) / `45.11.110.250` | `cloud_racecontrol` | HTTP `GET http://localhost:8080/api/v1/health` from Bono (via relay) OR direct HTTP `GET https://racingpoint.cloud/api/v1/health` | public | `pm2 show racecontrol` via relay | READ-ONLY. Same health shape as server_23. `auto-detect.sh:bono_status` is the exact template. | In active production use |
| 15 | `relay_james` | James :8766 + VPS :8765 (composite) | `relay` | Local HTTP `GET http://localhost:8766/relay/health` + relay-passthrough `GET :8765/relay/health` | none | — | READ-ONLY. `{connected, status, queue_depth, last_sync}` shape from `auto-detect.sh:304`. No binary_sha256 / config_hash — emit empty `{}`, `build_id: null` (see `relay_james.json` example). | Local probe always available |

**Note on "11 targets, 8 probe types":** the schema role enum is 8 values (server, pod, pos, james, vps, cloud_admin, cloud_racecontrol, relay) but pods 1-8 share one probe script that takes a `<pod_N>` arg. Orchestrator fan-out = 11 child invocations = 11 manifest files.

## Reference Implementations

Existing scripts that probe authors should READ BEFORE writing each new probe. Each row = one near-complete template.

| New probe | Emulate | Why |
|-----------|---------|-----|
| `probe-server.sh` | `scripts/deploy-server.sh` (12-step swap with SSH error handling) + `scripts/deploy-preflight.sh:36-47` (findstr sentry_service_key pattern over SSH) | 900+ lines of Tailscale-SSH-from-Git-Bash patterns; every quoting edge case solved (`2>nul` not `2>/dev/null`, `ping -n 4` not `timeout`). |
| `probe-pod.sh` | `scripts/deploy-pod.sh` (rc-sentry /exec usage + X-Service-Key) + `scripts/deploy-preflight.sh:50-78` (authed-exec loop over 8 pod IPs) | rc-sentry /exec payload-file pattern (write JSON with Write tool, curl -d @file) is the only reliable exec path. Never inline JSON. |
| `probe-pos.sh` | `scripts/fleet-sync-status.sh:267` (POS :3300/api/health curl) + deploy-preflight SSH pattern applied to `User@100.95.211.1` | POS has both kiosk HTTP and SSH; probe should capture both. |
| `probe-james.sh` | **No direct template exists** — but `scripts/audit/autostart-surfaces.sh` is the shape (ps/systemd/cron enumerator emitting SCHEMA-like JSON). Windows sibling needed — closest inspiration is `scripts/healing/escalation-engine.sh` tasklist usage. | Pure localhost; no network path to emulate. Windows `tasklist /V /FO CSV`, `schtasks /Query /V /FO LIST`, `reg query HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`, plus `dir "$STARTUP_FOLDER"`. |
| `probe-vps.sh` | `scripts/audit/autostart-surfaces.sh` (Linux ps/systemd/cron enumerator — wrap into SCHEMA-01) + `scripts/auto-detect.sh:316` (relay exec pattern) + `scripts/smart-pipes/env-drift-check.sh:36` (concise relay usage) | `autostart-surfaces.sh` already emits pm2 list + cron + systemd + XDG as a single JSON blob. Probe sends that script to run on VPS via relay chain, catches stdout, parses, re-assembles under SCHEMA-01 field names. |
| `probe-cloud-admin.sh` | `scripts/bono-auto-detect.sh:255` (app:port:/api/health triple loop) + `scripts/db-sync/RESTORE-DRILL.md:98` (admin.racingpoint.cloud health curl) + `scripts/deploy/deploy-nextjs.sh:204` (build_id + git_commit extraction from /api/health) | Pure curl; `jq` for `build_id` + `git_commit`; STAFF_JWT only needed for gate-state probe (Coming Soon gate flag). |
| `probe-cloud-rc.sh` | `scripts/auto-detect.sh:286-301` (bono_health extraction) + `scripts/deploy/check-health.sh` | Cloud racecontrol has same `/api/v1/health` shape as server_23; near-identical logic minus SSH. |
| `probe-relay.sh` | `scripts/auto-detect.sh:303-341` (relay_health + exec round-trip + chain round-trip) | Composite: James local `/relay/health` + VPS `/relay/health` via relay passthrough. Running_procs = synthetic from pm2-list + James tasklist (node.exe filter). |
| `probe-all.sh` | `scripts/wait-for-pods.sh` (pod-loop pattern) + `scripts/deploy/deploy-all-pods.sh` (sequential-with-collected-status pattern) + `scripts/auto-detect.sh` (sectioned probe-by-probe flow with per-section status accumulator) | Orchestrator is structural — read these three to pick the parallel+sequential mix without re-inventing. |
| `lib/probe-common.sh` | `scripts/lib/ssh-helpers.sh` (lib-file-per-function pattern, source-style) + `scripts/ist-now.sh` (ISO-timestamp helper that handles Git-Bash `TZ=Asia/Kolkata` silent failure via manual `+19800` seconds) | IST offset computation MUST use `UTC_EPOCH + 19800` pattern from `ist-now.sh` — `TZ=Asia/Kolkata date` returns UTC unchanged on Git Bash (standing rule in CLAUDE.md). |

**Key file excerpt — `scripts/ist-now.sh` pattern (MUST use for `probed_at_ist`):**

```bash
# From scripts/ist-now.sh (verified authoritative; standing rule in CLAUDE.md)
UTC_EPOCH=$(date -u +%s)
IST_EPOCH=$((UTC_EPOCH + 19800))  # 5*3600 + 30*60 = 19800
# ISO-8601 with +05:30 suffix:
date -u -d "@$IST_EPOCH" '+%Y-%m-%dT%H:%M:%S+05:30'
```

**Key file excerpt — rc-sentry /exec authed call (from `deploy-preflight.sh:50-70`):**

```bash
# Write payload to file first — Git Bash JSON escaping rule (CLAUDE.md standing rule)
cat > /tmp/probe-exec.json <<EOF
{"cmd":"tasklist /V /FO CSV"}
EOF

AUTH_RESULT=$(curl -s --connect-timeout 3 --max-time 15 \
  -H "X-Service-Key: ${SENTRY_KEY}" \
  -H "Content-Type: application/json" \
  -d @/tmp/probe-exec.json \
  "http://${pod_ip}:8091/exec" 2>/dev/null || echo "")
```

**Key file excerpt — comms-link relay exec (from `auto-detect.sh:316`):**

```bash
exec_result=$(curl -s --max-time 15 -X POST "$RELAY_URL/relay/exec/run" \
  -H "Content-Type: application/json" \
  -d '{"command":"bash_script","reason":"probe-vps-448"}' 2>/dev/null || echo "")
exec_exit=$(echo "$exec_result" | jq -r '.exitCode // -1')
```

## Dependency Audit

External tools that probes rely on, and availability on James .27 (the invoking machine).

| Dependency | Required by | Available on James .27 | Version | Fallback |
|------------|-------------|-----------------------|---------|----------|
| `bash` (Git Bash) | All probes | YES | MSYS2/Git-for-Windows | none — hard dep |
| `curl` | probe-pod, probe-vps, probe-cloud-admin, probe-cloud-rc, probe-relay, orchestrator | YES (Git Bash bundled) | 7.x+ | none — hard dep |
| `ssh` | probe-server, probe-pod (fallback), probe-pos | YES (Windows OpenSSH + Git Bash) | OpenSSH_8.x+ | none — hard dep for SSH targets |
| `scp` | probe-server (if file copy needed) | YES | via OpenSSH | none |
| `jq` | All probes (parse /api/health, /relay/health) | YES (verified in use by `auto-detect.sh`, `bono-auto-detect.sh`) | 1.6+ | `python3 -c 'import json; ...'` (also available) |
| `python3` | `probe-common.sh` JSON assembly + IST fallback | YES (per `ist-now.sh` fallback path; verified) | 3.9+ | Pure bash with `printf` — verbose but possible |
| `sha256sum` | `sha256_of` helper (for local files) | YES (MSYS2) | coreutils 8.x | `certutil -hashfile <f> SHA256` (Windows native) or `shasum -a 256` |
| `node --test` + `ajv` | `tests/fleet-probe/*` unit tests | YES (Node 22.22.0, ajv 8.17.1 + ajv-formats 3.0.1 installed in Phase 447-03) | ajv ^8.17.1 | none — reuse Phase 447's test infra verbatim |
| `tailscale` CLI | probe-server (optional — for pre-flight reachability), probe-pos, probe-vps fallback | YES (verified — CLAUDE.md network map records TS IPs in active use) | — | Skip reachability pre-check; rely on SSH ConnectTimeout |
| `openssl` | Not needed for probes (only rotate-credentials uses it) | YES | — | — |
| **Remote dependencies** | | | | |
| `tasklist /V /FO CSV` | probe-server, probe-pod, probe-pos, probe-james | YES on all Windows targets | built-in | `wmic process get` (legacy but present) |
| `schtasks /Query /V /FO LIST` | Same | YES | built-in | `reg query` on schtask registry hive (fragile) |
| `reg query` HKLM/HKCU Run | Same | YES | built-in | PowerShell `Get-ItemProperty` (do NOT use over SSH per standing rule) |
| `Get-FileHash -Algorithm SHA256` | probe-server, probe-pod binary hashing | YES (PowerShell 5.1+ bundled with Win 10/11) | — | `certutil -hashfile <f> SHA256` — shell-friendlier (no nested PS invocation), recommended |
| `pm2 jlist` / `pm2 list --no-color` | probe-vps, probe-cloud-admin (cloud admin runs on pm2), probe-cloud-rc, probe-relay (VPS side) | YES on Bono VPS (verified in use by `autostart-surfaces.sh` + `auto-detect.sh`) | 5.x | `ps -ef | grep node` — less structured |
| `ps -eo comm,pid,args` | probe-vps (Linux running_procs) | YES on Bono VPS | coreutils | — |
| `sha256sum` (remote file hash on Linux) | probe-vps | YES on Bono VPS | — | — |

**Missing dependencies with no fallback:** none — every dependency is already in use elsewhere in the repo, on the same machines.

**Missing dependencies with fallback:** none — list is complete.

**Recommended preference:** use `certutil -hashfile <f> SHA256 | find /V ":"` in remote Windows commands over `Get-FileHash` — avoids PowerShell-over-SSH quoting hell (standing rule: "NEVER edit remote files via inline PowerShell over SSH"). For READ-ONLY hash queries the rule is weaker but still worth avoiding. `certutil` is native cmd.exe and outputs hex on a clean line.

## Manifest Assembly Pattern

Every probe follows the same 5-phase shape. `lib/probe-common.sh` provides the shared helpers.

### Phase structure (per probe)

1. **Pre-flight** — validate `MANIFEST_TS` env var set; target args parsed; required secrets present in env.
2. **Connect-stage probe** — single fast round-trip (SSH hostname + HTTP /health + relay /health). On failure => `probe_status: probe_failed`, skip to step 5.
3. **Sub-probes** — each of {binary_sha256, build_id, config_hash, running_procs, scheduled_tasks, autostart_entries, env_vars_hash, last_deploy_ts} runs in a sub-function with its own try/catch. Failures append `{sub_probe, error}` to local `PROBE_ERRORS` array. `probe_status` computed at end via `probe_status_from_errors`.
4. **Assemble manifest** — build JSON via the `write_manifest` helper.
5. **Emit status + write** — stdout single-line status JSON; `write_manifest` pretty-prints to `state/fleet-manifest/$MANIFEST_TS/$target_id.json`.

### Shared lib contract (`lib/probe-common.sh`)

```bash
# Source-able; callers: source "$(dirname "$0")/lib/probe-common.sh"

# JSON helpers
json_escape()                # escape string for JSON embed
write_manifest()             # ARGS: target_id manifest_json ; writes state/fleet-manifest/$MANIFEST_TS/$target_id.json (pretty-printed)
                             # If FLEET_PROBE_VALIDATE=1, pipes through node --input-type=module ajv-harness
                             # Returns 0 on success; 1 on schema validation failure when validator enabled

# Hash helpers (cross-platform)
sha256_of()                  # ARG: filepath OR stdin ; emits lowercase 64-char hex
sha256_of_remote_file()      # ARGS: ssh_target remote_path ; uses ssh + certutil or sha256sum depending on remote OS

# Time helpers
iso_ist_now()                # emits "2026-04-24T17:30:00+05:30" using UTC_EPOCH+19800 pattern from ist-now.sh
                             # MUST NOT use TZ=Asia/Kolkata (standing rule)

# Status/error helpers
probe_status_from_errors()   # ARG: count of connect-stage errors + count of other errors
                             # Returns: "ok" (0 errors) | "probe_failed" (>=1 connect-stage) | "partial" (>=1 sub-probe)

# Env-var-name hashing (SECURITY BOUNDARY: names only, never values)
env_names_hash()             # emits sha256 of `env | sort | awk -F= '{print $1}' | sha256sum`
                             # SSH variant: env_names_hash_remote() ARGS: ssh_target

# Process-line hashing (SECURITY BOUNDARY: hash cmdline, never store raw)
cmdline_hash()               # ARG: full cmdline string ; emits sha256
```

### `write_manifest` implementation sketch

```bash
write_manifest() {
  local target_id="$1"
  local manifest_json="$2"
  local out_dir="state/fleet-manifest/$MANIFEST_TS"
  mkdir -p "$out_dir"
  local out_file="$out_dir/$target_id.json"

  # Validate JSON is well-formed before writing
  if ! echo "$manifest_json" | python3 -m json.tool > "$out_file.tmp" 2>/dev/null; then
    echo "ERROR: $target_id manifest is not valid JSON" >&2
    rm -f "$out_file.tmp"
    return 1
  fi

  # Optional schema validation (gate behind env var for speed on fleet runs)
  if [ "${FLEET_PROBE_VALIDATE:-0}" = "1" ]; then
    if ! node scripts/fleet-probe/validate-manifest-file.mjs "$out_file.tmp" >&2; then
      echo "ERROR: $target_id manifest failed schema validation" >&2
      rm -f "$out_file.tmp"
      return 1
    fi
  fi

  mv "$out_file.tmp" "$out_file"
}
```

The `validate-manifest-file.mjs` is a 40-line wrapper around Phase 447's `ajv.compile(schema)` logic — read Phase 447's `tests/fleet-drift/validate-manifest.test.mjs` lines 380-395 for the exact import + compile pattern; the new wrapper takes `process.argv[2]` as the manifest path, returns exit 1 with readable errors on failure.

### Orchestrator (`probe-all.sh`) shape

```bash
export MANIFEST_TS=$(date -u +%Y-%m-%dT%H%M%SZ)
mkdir -p "state/fleet-manifest/$MANIFEST_TS"

# Sequential: server, pos, james, vps, cloud-admin, cloud-rc, relay
bash scripts/fleet-probe/probe-server.sh
bash scripts/fleet-probe/probe-pos.sh
bash scripts/fleet-probe/probe-james.sh
bash scripts/fleet-probe/probe-vps.sh
bash scripts/fleet-probe/probe-cloud-admin.sh
bash scripts/fleet-probe/probe-cloud-rc.sh
bash scripts/fleet-probe/probe-relay.sh

# Parallel: pods 1-8
for N in 1 2 3 4 5 6 7 8; do
  bash scripts/fleet-probe/probe-pod.sh $N &
done
wait

# Assemble _meta.json (shape locked in schemas/examples/_meta.json)
python3 scripts/fleet-probe/build-meta-index.py "state/fleet-manifest/$MANIFEST_TS"
```

## Validation Architecture

Phase 447's ajv test harness (SHIPPED, 17/17 green) is the foundation. Phase 448 extends it with probe-specific unit tests.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Node built-in `node --test` (Node 22.22.0) + ajv ^8.17.1 + ajv-formats ^3.0.1 — ALL INSTALLED IN PHASE 447-03 |
| Config file | `package.json` (existing `test:fleet-drift` script; NEW `test:fleet-probe` script to add) |
| Quick run command | `npm run test:fleet-probe` |
| Full suite command | `npm run test:fleet-drift && npm run test:fleet-probe` |
| Existing infra that MUST stay green | `npm run test:fleet-drift` (17/17 from Phase 447 — validates schema + examples; MUST NOT regress when probes write real manifests) |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File |
|--------|----------|-----------|-------------------|------|
| PROBE-01 | probe-server.sh emits schema-valid manifest for mocked Server .23 SSH responses | unit (mock SSH) | `node --test tests/fleet-probe/probe-server.test.mjs` | Wave 0 |
| PROBE-02 | probe-pod.sh emits schema-valid manifest for mocked rc-sentry /exec responses | unit (mock HTTP) | `node --test tests/fleet-probe/probe-pod.test.mjs` | Wave 0 |
| PROBE-03 | probe-pos.sh emits schema-valid manifest; partial-class path when tasklist fails | unit (mock SSH with partial failure) | `node --test tests/fleet-probe/probe-pos.test.mjs` | Wave 0 |
| PROBE-04 | probe-james.sh emits schema-valid manifest from real localhost tasklist/schtasks | integration (local) | `bash tests/fleet-probe/smoke-james.sh` | Wave 0 |
| PROBE-05 | probe-vps.sh emits schema-valid manifest for mocked relay exec responses | unit (mock relay) | `node --test tests/fleet-probe/probe-vps.test.mjs` | Wave 0 |
| PROBE-06 | probe-cloud-admin.sh emits schema-valid manifest for mocked /api/health response | unit (mock HTTP) | `node --test tests/fleet-probe/probe-cloud-admin.test.mjs` | Wave 0 |
| PROBE-07 | probe-cloud-rc.sh emits schema-valid manifest for mocked /api/v1/health | unit (mock HTTP) | `node --test tests/fleet-probe/probe-cloud-rc.test.mjs` | Wave 0 |
| PROBE-08 | probe-relay.sh emits schema-valid composite manifest with both endpoints | unit (mock HTTP x2) | `node --test tests/fleet-probe/probe-relay.test.mjs` | Wave 0 |
| PROBE-09 | probe-all.sh produces 11 manifests + _meta.json in one timestamped dir | integration (all mocks wired) | `bash tests/fleet-probe/smoke-orchestrator.sh` | Wave 0 |
| ALL | Every probe's emitted manifest passes ajv validation | cross-cutting | `FLEET_PROBE_VALIDATE=1` env flag + `npm run test:fleet-probe` | Reuses 447-03 validator |
| PROBE-09 | `probe-all.sh --dry-run` enumerates 11 targets without network calls | smoke | `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c '^target='` returns `11` | Wave 0 |

### Mock Strategy

Each unit test mocks its target using one of three patterns:

1. **HTTP mocks** — spin up a local ephemeral Node server on `http://127.0.0.1:0` returning canned responses; set `PROBE_OVERRIDE_URL` env var that the probe honors. Pattern verified in existing `tests/ffi/*` and `rc-agent/Cargo.toml` wiremock usage (per 413-02 SUMMARY).
2. **SSH mocks** — instead of SSH, the probe can accept `PROBE_OVERRIDE_EXEC` env var pointing to a local shell script that mimics the remote output. Pattern: set `PROBE_SSH=/tmp/mock-ssh-responder.sh` and have probe call `$PROBE_SSH` instead of `ssh`.
3. **Relay mocks** — same as HTTP mocks but listening on `http://127.0.0.1:0/relay/exec/run` and returning the canonical `{stdout, stderr, exitCode}` envelope.

### Sampling Rate

- **Per task commit:** `npm run test:fleet-probe -- --test-name-pattern '<probe>'` — single probe's unit tests, < 5s
- **Per wave merge:** `npm run test:fleet-drift && npm run test:fleet-probe` — full suite, < 15s
- **Phase gate:** Full suite green + `FLEET_PROBE_VALIDATE=1 bash scripts/fleet-probe/probe-all.sh --dry-run` clean before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/fleet-probe/` directory — covers PROBE-01..09 unit side
- [ ] `tests/fleet-probe/fixtures/` — canned SSH/HTTP response payloads per target class
- [ ] `tests/fleet-probe/mock-ssh-responder.sh` — shell script that reads a scenario file and emits matching stdout + exit code
- [ ] `tests/fleet-probe/mock-http-server.mjs` — tiny Node HTTP server (30 lines; pattern from `http.createServer`)
- [ ] `scripts/fleet-probe/validate-manifest-file.mjs` — thin CLI wrapper around Phase 447's ajv.compile+validate pattern (used inside `write_manifest` under `FLEET_PROBE_VALIDATE=1`)
- [ ] `package.json` script: `"test:fleet-probe": "node --test tests/fleet-probe/*.test.mjs"`

Framework install: **none needed** — Phase 447-03 already installed ajv + ajv-formats; Node 22.22.0 ships with `node --test`.

## Failure Mode Matrix

For each `{target, access path}`, the `probe_status` and the specific `probe_errors[].sub_probe` / `error` strings the probe must emit. Keep the error-string vocabulary consistent across probes — Phase 452 diff tool will key off these.

| Target | Failure | probe_status | probe_errors[] entry | Notes |
|--------|---------|--------------|----------------------|-------|
| server_23 | SSH ConnectTimeout (15s) | probe_failed | `{sub_probe:"ssh_connect", error:"timeout after 15s"}` + `access_gap: "SSH_23"` | PROBE-01 audit doc trigger. Write `docs/fleet-probe/access-gaps.md` row. |
| server_23 | SSH banner prefix corruption (per standing rule) | probe_failed | `{sub_probe:"ssh_connect", error:"banner detected: <first_line>"}` | Reuse `scripts/lib/ssh-helpers.sh` safe_remote_read |
| server_23 | SSH OK, `tasklist` errors | partial | `{sub_probe:"tasklist", error:"<stderr>"}` | Continue other sub-probes |
| server_23 | SSH OK, `schtasks /Query` errors | partial | `{sub_probe:"schtasks_query", error:"<stderr>"}` | Continue |
| server_23 | SSH OK, `reg query HKLM\Run` errors | partial | `{sub_probe:"reg_query_hklm_run", error:"<stderr>"}` | Continue |
| server_23 | `Get-FileHash racecontrol.exe` fails (file locked or missing) | partial | `{sub_probe:"binary_sha256", error:"file locked or missing"}` | Try `certutil -hashfile` fallback before giving up |
| server_23 | SWAPLOG.md missing or empty | partial | `{sub_probe:"last_deploy_ts", error:"swaplog missing or empty"}` | Emit `last_deploy_ts: null` |
| server_23 | Q5 drift — D:\ vs live vs git divergence | **still probe_status=ok** but config_hash has 3 distinct entries | n/a (not an error; data observation) | Phase 452 surfaces the drift |
| pod_N | `ping -n 1 -w 2000` fails + Tailscale ping fails | probe_failed | `{sub_probe:"connectivity", error:"ping LAN+TS both timed out"}` | Use `scripts/check-alive.sh` multi-probe pattern |
| pod_N | `:8091/exec` returns 401 (stale SENTRY_KEY) | probe_failed | `{sub_probe:"auth", error:"401 unauthorized; SENTRY_KEY may be stale"}` | Caller: re-run `deploy-preflight.sh` |
| pod_N | `:8091/exec` returns 500 | partial | `{sub_probe:"exec_tasklist", error:"rc-sentry /exec 500"}` | Retry once; if still 500, mark partial |
| pod_N | `:8090/debug` returns 404 (rc-agent version pre-debug-endpoint) | partial | `{sub_probe:"debug_endpoint", error:"404 — rc-agent pre-debug version"}` | build_id from /health instead |
| pod_N | rc-agent `/health` returns 200 but no `build_id` field | partial | `{sub_probe:"build_id", error:"/health response missing build_id"}` | Emit `build_id: null` |
| pod_N | MAINTENANCE_MODE sentinel detected | **still probe_status=ok** but autostart_entries includes synthetic `{source:"schtask", key:"MAINTENANCE_MODE_ACTIVE", value:"C:\\RacingPoint\\MAINTENANCE_MODE"}` | Observation, not error | Phase 452 surfaces the halted state |
| pos_130 | SSH key auth fails | probe_failed | `{sub_probe:"ssh_connect", error:"pubkey auth failed"}` | POS SSH known to work; key rotation issue |
| pos_130 | SSH OK, `tasklist /V /FO CSV` returns WMI-denied | partial | `{sub_probe:"tasklist", error:"WMI access denied from remote SSH context"}` | Pre-existing partial-class example in `pos_130.json`; retry via `tasklist /SVC /FO CSV` |
| pos_130 | Chrome not running (kiosk dead) | **still probe_status=ok** (probe succeeded); running_procs just omits chrome | Observation | Phase 452 surfaces via diff vs baseline |
| james_27 | (local) — practically never fails | ok | n/a | Only failure mode: probe script itself crashes (bash syntax error) — not recoverable at manifest level |
| bono_vps | comms-link `/relay/health` returns `{connected:false}` | probe_failed | `{sub_probe:"relay_connect", error:"relay not connected to VPS"}` + `access_gap: "RELAY_DOWN"` | Fall back to direct SSH `root@100.70.177.44` if Tailscale up |
| bono_vps | Relay up, exec returns non-zero `exitCode` | partial | `{sub_probe:"<command>", error:"exec exit=<N> stderr=<trim>"}` | per-command |
| bono_vps | relay reachable but COMMS_PSK unset in James env | probe_failed | `{sub_probe:"auth", error:"COMMS_PSK not set in invoking shell"}` + `auth_gap: "no_comms_psk"` | Caller: `export COMMS_PSK=...` from secrets file |
| cloud_admin | DNS resolution fails | probe_failed | `{sub_probe:"dns", error:"NXDOMAIN admin.racingpoint.cloud"}` | — |
| cloud_admin | TLS handshake fails | probe_failed | `{sub_probe:"tls", error:"cert expired or SNI mismatch"}` | — |
| cloud_admin | 503 with `cloud_admin_gated` | **probe_status=ok** (gate is intentional state); captured as `scheduled_tasks` entry `{name:"ADMIN_COMING_SOON_GATE", state:"active"}` | Observation | Phase 452 surfaces intentional gate |
| cloud_admin | STAFF_JWT expired (for gated-page probe) | partial | `{sub_probe:"authed_page_check", error:"401 — STAFF_JWT expired"}` + `auth_gap: "staff_jwt_expired"` | Public /api/health already captured; only gate-state probe degrades |
| cloud_admin | `/api/health.pages_missing[]` non-empty | **probe_status=ok** (field is informational); entries surface to probe_errors[] as `{sub_probe:"pages_probe", error:"pages_missing: <list>"}` for Phase 452 | Observation | Existing shape from Phase 445-05 |
| cloud_racecontrol | HTTP 5xx on `/api/v1/health` | probe_failed | `{sub_probe:"health", error:"500 from cloud racecontrol"}` | — |
| cloud_racecontrol | HTTP 200 but JSON malformed | partial | `{sub_probe:"health_parse", error:"<jq error>"}` | — |
| relay_james | localhost :8766 not listening | probe_failed | `{sub_probe:"local_relay", error:"connection refused :8766"}` | Caller: start comms-link via watchdog schtask |
| relay_james | James local up, VPS :8765 down (relay `connected:false`) | partial | `{sub_probe:"vps_relay", error:"relay reports connected:false"}` | — |
| ALL | `MANIFEST_TS` env var not set | exit 2 (script crash, not manifest write) | n/a — bare orchestrator-enforced pre-condition | — |
| ALL | `state/fleet-manifest/$MANIFEST_TS/` path not writable | exit 2 | n/a | — |
| ALL | `FLEET_PROBE_VALIDATE=1` and emitted manifest fails ajv | exit 1 (per-probe) — orchestrator continues; `_meta.json` logs the failure | n/a (structural — means a probe has a bug) | — |

**Canonical sub_probe vocabulary** (keep stable across probes so Phase 452 can pattern-match):
`ssh_connect`, `connectivity`, `auth`, `dns`, `tls`, `health`, `health_parse`, `exec_tasklist`, `tasklist`, `schtasks_query`, `reg_query_hklm_run`, `reg_query_hkcu_run`, `startup_folder`, `debug_endpoint`, `binary_sha256`, `build_id`, `config_hash`, `env_vars_hash`, `last_deploy_ts`, `relay_connect`, `vps_relay`, `local_relay`, `pages_probe`, `authed_page_check`, `pm2_list`.

## Open Questions

All non-blocking — probe authors can proceed with recommended defaults.

1. **Question:** For probe-vps.sh, do we execute the entire `autostart-surfaces.sh` on the VPS in one relay call and parse the JSON, or split into 5-6 smaller relay calls (one per sub-probe)?
   - **What we know:** `autostart-surfaces.sh` emits one well-formed JSON blob; relay exec supports arbitrary commands up to a payload limit; `scripts/smart-pipes/env-drift-check.sh:36` already executes a multi-line heredoc via relay.
   - **What's unclear:** whether a single-shot multi-second exec on the relay hits any 30s timeout in the relay shim.
   - **Recommendation:** start with single-shot (cleaner; one roundtrip; atomic snapshot). If the relay truncates, fall back to sub-probe-per-call. Add a `PROBE_VPS_SPLIT=1` env override for easy switching.

2. **Question:** For probe-server.sh's Q5 three-way config diff, is `D:\racecontrol.toml` guaranteed accessible from Git Bash as `/d/racecontrol.toml`, and does it exist on every James session?
   - **What we know:** Q5 drift audit memory `project_q5_racecontrol_toml_drift_20260423.md` states D:\ is the proxy route. Previous session confirmed the file.
   - **What's unclear:** whether D:\ is always mounted (external drive?) or is a fixed local drive.
   - **Recommendation:** probe-server.sh should emit 3 SHA256 entries under `config_hash` keyed as `racecontrol.toml.server_live`, `racecontrol.toml.james_proxy`, `racecontrol.toml.git_head`. If `D:\racecontrol.toml` missing, emit only `server_live` + `git_head` and append `{sub_probe:"config_hash_james_proxy", error:"D:\\racecontrol.toml not found on James"}`. Non-blocking (partial class).

3. **Question:** For cloud_admin's Coming Soon gate detection, is the state exposed on `/api/health` or does it need an authed GET to `/` to see the 307?
   - **What we know:** PR #13 introduced `ADMIN_COMING_SOON_GATE=0/1` env var; middleware 307s non-public pages to /coming-soon, /api/* to 503.
   - **What's unclear:** whether the public `/api/health` stays 200 even when gate is on, or also 503s.
   - **Recommendation:** probe tries both — first public `/api/health` (build_id), then HEAD request to `/` and checks response code; if 307 => gate active; if 200 => gate inactive. No STAFF_JWT needed for gate-state-only probe.

4. **Question:** Should probe-pod.sh support a `--pod N` flag for scoped single-pod runs, and if so should `probe-all.sh` use it for its pod loop, or should probe-pod.sh be invoked as `probe-pod.sh 1` (positional)?
   - **What we know:** CONTEXT.md `<domain>` lists `probe-pod.sh <pod_N>` (positional). PACT-012 Option A wants Pod 8 canary support.
   - **What's unclear:** flag style preference only.
   - **Recommendation:** positional `probe-pod.sh <pod_N>` per CONTEXT.md, PLUS an orchestrator-level `probe-all.sh --canary` flag that invokes only `probe-pod.sh 8` + `probe-server.sh` (minimal canary manifest subset). Addresses PACT-012 without polluting probe-pod.sh with flag logic.

5. **Question:** For the PROBE-01 access-gap doc, should `docs/fleet-probe/access-gaps.md` be tracked in git or under a .gitignore'd runtime path?
   - **What we know:** CONTEXT.md says `docs/fleet-probe/access-gaps.md` without qualifier; the `docs/` path suggests tracked. But 447-01 put `state/fleet-manifest/` under `.gitignore` with `.gitkeep`.
   - **Recommendation:** Tracked in git — access-gap findings are audit-trail-class content, not ephemeral run output. Seeds future Uday remediation work. Adopt the pattern.

## Sources

### Primary (HIGH confidence — file contents directly read this session)

- `schemas/fleet-manifest.schema.json` — Phase 447 LOCKED contract; all 15 required fields, additionalProperties:true everywhere, enum values for role (8) + probe_status (3)
- `schemas/examples/*.json` (9 files) — 8 per-target + _meta.json; full shape templates including partial-class `pos_130.json`, null-build_id `relay_james.json`, null-last_deploy_ts `james_27.json`
- `.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md` — user decisions
- `.planning/REQUIREMENTS-v53.md` — PROBE-01..09 verbatim
- `.planning/STATE.md` — v53.0 progress, Phase 447 SHIPPED evidence
- `.planning/phases/447-manifest-schema-scope-lock/447-03-SUMMARY.md` + `447-VERIFICATION.md` — predecessor context (validator 17/17 green)
- `.planning/phases/447-manifest-schema-scope-lock/447-03-PLAN.md` — ajv import pattern + node test runner usage
- `C:\Users\bono\racingpoint\racecontrol\CLAUDE.md` — SSH-to-Windows rules, Git Bash JSON escaping, TZ=Asia/Kolkata silent failure, deploy patterns
- `scripts/ist-now.sh` — IST offset formula (UTC_EPOCH+19800)
- `scripts/lib/ssh-helpers.sh` — SCP-safe remote read; SSH banner detection
- `scripts/deploy-preflight.sh` — rc-sentry /exec + X-Service-Key + POD_IPS loop pattern
- `scripts/check-alive.sh` — multi-probe {ping LAN, ping Tailscale, HTTP health} + verdict matrix
- `scripts/auto-detect.sh` (lines 286-341) — cloud racecontrol + relay + exec round-trip patterns
- `scripts/bono-auto-detect.sh` (lines 138-165, 255) — relay-down fallback + app health loop
- `scripts/audit/autostart-surfaces.sh` — Linux autostart+pm2 JSON enumerator (template for probe-vps.sh)
- Phase 447 live-verification commands already shipped and green

### Secondary (MEDIUM confidence — referenced via grep but file not fully read)

- `scripts/deploy-pod.sh` (line 28+) — pod /exec pattern
- `scripts/deploy-server.sh` — server SSH + schtasks
- `scripts/healing/escalation-engine.sh:200` — relay exec
- `scripts/smart-pipes/env-drift-check.sh:36` — concise relay usage
- `scripts/fleet-sync-status.sh:267` — POS kiosk health at :3300
- `scripts/deploy/deploy-nextjs.sh:204` — /api/health build_id+git_commit pattern

### Tertiary (LOW confidence — inferred from memory)

- Exact POS .130 SSH behavior when WMI access-denied occurs on `tasklist /V` — inferred from `pos_130.json` example plus CLAUDE.md's POS notes; not validated this session. Probe should expect it and fall back to `tasklist /SVC /FO CSV`.
- Cloud admin gate-state HTTP response shape when `ADMIN_COMING_SOON_GATE=1` — based on memory `project_cloud_admin_api_404_epidemic_20260422.md` description; not verified live. Probe defensively tries both /api/health and HEAD /.

## Metadata

**Confidence breakdown:**
- Access paths: HIGH — every path in active use by existing scripts; 2 SSH targets verified EXIT=0 this session
- Reference implementations: HIGH — specific files + line numbers cited, all read this session
- Dependency audit: HIGH — every tool in active use elsewhere in repo
- Manifest assembly pattern: HIGH — schema is locked (447-VERIFICATION.md passed); helpers are thin wrappers over shell builtins
- Validation architecture: HIGH — reuses 447-03 infra verbatim; unit test strategy standard
- Failure-mode matrix: MEDIUM — comprehensive but not every edge case tested live; some entries (cloud admin gate shape, POS WMI-denied fallback) inferred from memory

**Research date:** 2026-04-24
**Valid until:** 2026-05-24 (stable — 30 days). If Phase 447 schema changes, this research invalidates.

## RESEARCH COMPLETE
