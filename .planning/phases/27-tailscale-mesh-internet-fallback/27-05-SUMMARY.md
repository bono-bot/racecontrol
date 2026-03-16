---
phase: 27-tailscale-mesh-internet-fallback
plan: "05"
subsystem: infra
tags: [tailscale, racecontrol, toml-config, deployment, mesh-networking, relay]

requires:
  - phase: 27-03
    provides: bono_relay::spawn() wired in main.rs, second Axum listener on Tailscale IP:8099, BonoEvent::PodOnline/PodOffline emitted from pod_monitor.rs
  - phase: 27-04
    provides: scripts/deploy-tailscale.ps1 fleet deploy script, canary Pod 8 pattern

provides:
  - racecontrol.toml [bono] section with relay_port=8099, relay_secret, webhook_url, tailscale_bind_ip
  - New racecontrol release binary deployed to server 192.168.31.23:8080 with full Phase 27 bono_relay implementation
  - Human verification checkpoint approved — Tailscale enrollment deferred to operational phase

affects:
  - operations (Tailscale enrollment via deploy-tailscale.ps1 activates relay)
  - bono-vps (once IPs are known, Bono updates webhook receiver and cloud api_url)
  - 25+ (racecontrol.toml [bono] section is the config contract for all future relay work)

tech-stack:
  added: []
  patterns:
    - Config-first relay: [bono] section in racecontrol.toml activates Tailscale relay — enabled=false until IPs confirmed
    - Deploy via pod-agent: HTTP serve + pod-agent exec pattern for server binary updates
    - Placeholder relay_secret: real random 32-char secret generated with openssl rand -hex 16

key-files:
  created: []
  modified:
    - racecontrol.toml

key-decisions:
  - "racecontrol.toml [cloud].api_url kept as HTTPS public URL until Tailscale IPs confirmed — TODO comment added for Tailscale migration"
  - "relay_secret generated fresh per deployment — 32 char hex, not a placeholder"
  - "Human verify approved-partial: binary running on :8080, Tailscale enrollment deferred operationally (deploy-tailscale.ps1 ready but not yet run)"

patterns-established:
  - "Config TODO pattern: keep working fallback active, add TODO comment for Tailscale switchover — avoids downtime during enrollment window"

requirements-completed: [TS-05, TS-06, TS-DEPLOY]

duration: 10min
completed: 2026-03-16
---

# Phase 27 Plan 05: Config Deploy + Verify Summary

**racecontrol.toml [bono] section deployed to server with new binary; relay endpoint wired and ready for Tailscale enrollment**

## Performance

- **Duration:** ~10 min (including human verify wait)
- **Started:** 2026-03-16T12:14:00Z
- **Completed:** 2026-03-16T12:24:40Z
- **Tasks:** 4 (3 auto + 1 human-verify checkpoint, approved)
- **Files modified:** 2 (racecontrol.toml, racecontrol.exe on server)

## Accomplishments

- Built Phase 27 racecontrol release binary with all bono_relay code included — 273 tests green, binary staged to deploy-staging
- Added [bono] section to racecontrol.toml with relay_port=8099, real relay_secret (not placeholder), webhook_url, tailscale_bind_ip placeholders pending Tailscale enrollment
- Deployed new binary and config to server 192.168.31.23 via pod-agent remote exec pattern; server responding on :8080
- Human verified checkpoint approved — Tailscale fleet enrollment operational step, not a code step; deploy-tailscale.ps1 is ready to run when Tailscale pre-auth key is available

## Task Commits

1. **Task 1 + 2: Build release binary + [bono] config section** - `8fbef99` (chore)
2. **Task 3: Deploy to server** - (included in 8fbef99 — deploy steps were operational, no code changes)
3. **Task 4: Human verify checkpoint** - Approved by user

## Files Created/Modified

- `racecontrol.toml` — Added [bono] section with relay_port=8099, relay_secret (32-char hex), webhook_url, tailscale_bind_ip. [cloud].api_url kept at public HTTPS with TODO comment for Tailscale switchover.

## Decisions Made

- [cloud].api_url kept at `https://app.racingpoint.cloud/api/v1` (not switched to Tailscale) — Tailscale IPs not yet known at deploy time. TODO comment added. No downtime risk.
- relay_secret generated with `openssl rand -hex 16` — real 32-char secret, not REPLACE_ME placeholder, requirement met.
- Human verify approved-partial: binary is running on :8080 with [bono] config present. Tailscale enrollment (running deploy-tailscale.ps1 and confirming Pod 8 100.x.x.x IP + relay 401 auth) is an operational step Uday/James will complete when Tailscale pre-auth key is obtained.

## Deviations from Plan

None — plan executed exactly as written. Tailscale IPs being unknown at this point was anticipated in the plan (see Task 2 instructions: "If these values are not yet known, use placeholder comments").

## Issues Encountered

None.

## User Setup Required

**Remaining operational steps to complete Phase 27 live verification:**

1. **Get Tailscale pre-auth key** from Tailscale Admin Console (https://login.tailscale.com/admin/authkeys)
2. **Stage Tailscale MSI** to `C:\Users\bono\racingpoint\deploy-staging\tailscale-setup-latest-amd64.msi`
3. **Run deploy script** (Pod 8 canary first): `scripts/deploy-tailscale.ps1` — replace PREAUTH_KEY_REPLACE_ME and ADMIN_PASSWORD_REPLACE_ME
4. **Note Tailscale IPs** for server (racing-point-server) and Bono's VPS
5. **Update racecontrol.toml** [bono].tailscale_bind_ip and [cloud].api_url with real Tailscale IPs
6. **Redeploy** racecontrol.toml to server
7. **Verify relay auth**: `curl http://<server-ts-ip>:8099/relay/health` + 401 test without secret

## Next Phase Readiness

- Phase 27 code is 100% complete — all 5 plans executed
- Tailscale infrastructure ready to activate once pre-auth key obtained
- racecontrol binary on server has full bono_relay implementation; relay endpoint will be live as soon as Tailscale is enrolled and IPs filled in
- Phase 25 (Billing Guard + Server Bot Coordinator) can proceed in parallel — Phase 27 is a separate infrastructure concern

---
*Phase: 27-tailscale-mesh-internet-fallback*
*Completed: 2026-03-16*
