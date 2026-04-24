# Fleet Probe -- Access Gaps

**Status:** Scaffold shipped in Phase 448 Plan 08. Phase 449 populates this file with live findings from the first full-fleet probe run.

## Purpose

This document catalogs every access-gap class the Phase 448 probe scripts can surface, and tracks remediation status per real-world incident. When a probe emits `probe_status: probe_failed` with an `access_gap` field in its `probe_errors[]` entry, a row in this file records the finding.

Access-gap IDs are stable strings used by probe scripts; they become part of the `state/fleet-manifest/<iso-ts>/<target_id>.json` output and are consumed by Phase 452's diff tool.

## Access-Gap Catalog

## Server .23 (SSH)

- **Access method:** Tailscale SSH `ADMIN@100.125.108.37` (Windows OpenSSH server, default key auth)
- **Fallback:** LAN SSH `ADMIN@192.168.31.23` via Tailscale jump through Bono VPS (not yet implemented in probe-server.sh)
- **Gap IDs produced by probe-server.sh:**
  - `SSH_23` -- SSH ConnectTimeout (15s) or pubkey failure
- **Owner if gap persists:** Uday (physical access required if key regen needed)
- **Current status:** SSH_23 CLEARED -- Server .23 SSH access verified 2026-04-24 18:58 IST (Racing-Point-Server / racing-point-se\admin). Phase 449 first run expected to return probe_status ok for SSH connect stage.

## Pods (rc-sentry /exec on :8091)

- **Access method:** HTTP POST to `http://<pod_ip>:8091/exec` with `X-Service-Key: $SENTRY_KEY` header
- **Pod IPs:** 192.168.31.{89,33,28,88,86,87,38,91} for pods 1..8 respectively (LAN); Tailscale fallback per CLAUDE.md
- **Gap IDs produced by probe-pod.sh:**
  - `no_sentry_key` (auth_gap) -- SENTRY_KEY env var unset in invoking shell
  - `stale_sentry_key` (auth_gap) -- 401 returned; key rotated on server but not re-synced to invoker
  - implied `probe_failed` via `connectivity` when both LAN and Tailscale are unreachable
- **Owner if gap persists:** Operator (run `deploy-preflight.sh` to resync keys)
- **Current status:** _(to be populated on first Phase 449 run)_

## POS .130

- **Access method:** Tailscale SSH `User@100.95.211.1` (default key)
- **Secondary:** HTTP `http://192.168.31.130:3300/api/health` for kiosk build fingerprint
- **Gap IDs produced by probe-pos.sh:**
  - `POS_SSH_DOWN` (access_gap) -- SSH timeout or pubkey failure
- **Known partial class:**
  - `tasklist` WMI access-denied via remote SSH context (canonical, reproduced in `schemas/examples/pos_130.json`). Probe still succeeds; status downgrades to `partial`. Fallback `tasklist /SVC /FO CSV` via rp-bono-exec is the deferred remediation.
- **Owner if gap persists:** Operator (POS physical access, check WiFi + Tailscale node)
- **Current status:** POS_SSH_DOWN OPEN -- POS .130 (100.95.211.1) was unreachable during Phase 448 probe session (2026-04-24). probe-pos.sh will emit probe_failed with access_gap: POS_SSH_DOWN on Phase 449 run. Remediation: physical POS access, verify WiFi + Tailscale node registration.

## James .27 (localhost)

- **Access method:** Local `tasklist`/`schtasks`/`reg query`
- **Gap IDs:** none expected -- this is the always-available class
- **Known-fail mode:** bash syntax error in probe-james.sh itself (caught by CI `bash -n` gate)
- **Current status:** OK (verified in Plan 02 smoke test)

## Bono VPS (comms-link relay)

- **Access method:** Local HTTP POST `http://localhost:8766/relay/exec/run` (comms-link relay proxies to VPS)
- **Gap IDs produced by probe-vps.sh:**
  - `no_comms_psk` (auth_gap) -- COMMS_PSK env var unset
  - `RELAY_DOWN` (access_gap) -- `/relay/health` returns `connected: false`
  - `RELAY_LOCAL_DOWN` (access_gap) -- James-side relay not listening on :8766
- **Owner if gap persists:** Operator (check `CommsLink-DaemonWatchdog` schtask on James; COMMS_PSK from secrets file)
- **Current status:** _(to be populated on first Phase 449 run)_

## Cloud admin (HTTPS)

- **Access method:** HTTPS GET `https://admin.racingpoint.cloud/api/health` (public), HEAD `/` for gate detection
- **Gap IDs produced by probe-cloud-admin.sh:**
  - `staff_jwt_expired` (auth_gap) -- only affects authed-page probe; public /api/health still captured
  - indirect `health` failure -- HTTP 5xx from /api/health
- **Intentional state:** ADMIN_COMING_SOON_GATE=1 surfaces as a `scheduled_tasks` entry (not an error). Phase 452 flags it for operator review.
- **Owner if gap persists:** Bono (cloud owner); Uday escalation for prolonged gate
- **Current status:** _(to be populated on first Phase 449 run)_

## Cloud racecontrol (HTTPS)

- **Access method:** HTTPS GET `https://racingpoint.cloud/api/v1/health` (public)
- **Gap IDs produced by probe-cloud-rc.sh:** no named access_gaps; failure modes are `connectivity`, `health` (non-200), `health_parse` (malformed), `build_id` (missing field)
- **Owner if gap persists:** Bono (cloud racecontrol redeploy if build_id stale)
- **Current status:** _(to be populated on first Phase 449 run)_

## Relay (composite: James :8766 + VPS :8765)

- **Access method:** Local HTTP GET `http://localhost:8766/relay/health` (reports both sides)
- **Gap IDs produced by probe-relay.sh:**
  - `RELAY_LOCAL_DOWN` (access_gap) -- James :8766 not listening
  - `vps_relay` (sub_probe, partial class) -- James up but VPS reports `connected: false`
- **Owner if gap persists:** Operator (James side) / Bono (VPS side)
- **Current status:** _(to be populated on first Phase 449 run)_

## Gap Resolution Log

| Date (IST) | Target | Gap ID | Discovery run_id | Remediation | Status |
|------------|--------|--------|------------------|-------------|--------|
| 2026-04-24 18:58 IST | server_23 | SSH_23 | Phase 448 Plan 03 access audit | SSH key-auth verified: Racing-Point-Server / racing-point-se\admin responded to Tailscale SSH (ADMIN@100.125.108.37) | CLEARED |
| _(Phase 449 first run populates additional rows)_ | | | | | |

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
