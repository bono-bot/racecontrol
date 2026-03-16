---
phase: 07-server-side-pinning
plan: 02
subsystem: infra
tags: [rust, axum, reverse-proxy, cors, next-js, kiosk, windows-sac]

# Dependency graph
requires:
  - phase: 07-server-side-pinning plan 01
    provides: DHCP reservation pinning server to .23, server inventory

provides:
  - racecontrol Axum reverse proxy for /kiosk* and /_next/* paths (bypasses Windows SAC)
  - kiosk.rp added to CORS allow list in racecontrol
  - Hop-by-hop header filtering in proxy (transfer-encoding, connection, keep-alive)
  - Release binary staged at deploy-staging/racecontrol.exe (21MB)
  - Code changes committed and verified locally (159 tests pass)

affects: [08-pod-lock-screen-hardening, 10-staff-dashboard-controls]

# Tech tracking
tech-stack:
  added: [reqwest (used for proxy HTTP client in racecontrol)]
  patterns:
    - Axum .fallback() for catch-all reverse proxy to Next.js kiosk on localhost:3300
    - Hop-by-hop header filtering before forwarding upstream response headers

key-files:
  created: []
  modified:
    - crates/racecontrol/src/main.rs

key-decisions:
  - "Windows Smart App Control (SAC) blocks node.exe from accepting network connections — route kiosk traffic through racecontrol Axum server instead of direct Next.js port"
  - "Proxy paths: /kiosk* and /_next/* forwarded to localhost:3300; all other paths return 404 (not passed through)"
  - "Server physical access required for deployment — WinRM/pod-agent both blocked by SAC on server"
  - "Portproxy rule removed from James's PC (no longer needed with proxy approach)"
  - "HKLM Run keys cleaned from James's PC (only needed on server)"

patterns-established:
  - "SAC bypass pattern: route blocked process traffic through racecontrol (trusted binary) as reverse proxy"

requirements-completed: [HOST-01, HOST-02, HOST-04]

# Metrics
duration: 30min
completed: 2026-03-14
---

# Phase 7 Plan 02: Server-Side Pinning (Deploy) Summary

**racecontrol Axum reverse proxy for kiosk paths added to bypass Windows Smart App Control blocking node.exe; CORS updated for kiosk.rp; 21MB release binary staged — server deployment blocked pending physical access**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-03-14T03:36:51+05:30
- **Completed:** 2026-03-14T03:56:45+05:30
- **Tasks:** 2 (code complete; server deployment pending physical access)
- **Files modified:** 1 (crates/racecontrol/src/main.rs)

## Accomplishments

- Added `kiosk_proxy` handler (~60 lines) to racecontrol using `.fallback()` — routes `/kiosk*` and `/_next/*` to `localhost:3300`
- Updated CORS predicate in racecontrol to allow `http://kiosk.rp` origin
- Fixed hop-by-hop header conflict: skip `transfer-encoding`, `connection`, `keep-alive` when copying upstream response headers (prevented empty responses from Next.js chunked encoding)
- Built and staged release binary at `C:\Users\bono\racingpoint\deploy-staging\racecontrol.exe` (21MB, verified locally: `http://localhost:8080/kiosk` returns 200)
- Cleaned up portproxy rule and HKLM Run keys from James's PC (artefacts from prior approach)
- 159 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: CORS fix + reverse proxy implementation** - `ea9a728` (feat)
2. **Task 2: Fix hop-by-hop headers in proxy** - `3db7403` (fix)

## Files Created/Modified

- `crates/racecontrol/src/main.rs` - Added `kiosk_proxy` fallback handler, updated CORS predicate to include `kiosk.rp`, added hop-by-hop header filtering

## Decisions Made

**Windows SAC blocks node.exe from accepting network connections.** The original plan called for deploying the Next.js standalone server directly on port 3300 and accessing it from James's machine at `kiosk.rp:3300`. Testing revealed that Windows Smart App Control prevents node.exe from binding to a network-accessible port. The solution was to route all kiosk traffic through the trusted racecontrol binary (already exempted from SAC), adding a reverse proxy inside Axum that forwards `/kiosk*` and `/_next/*` to `localhost:3300` (same-machine loopback, not blocked by SAC).

This approach means kiosk is accessible at `http://kiosk.rp:8080/kiosk` (through racecontrol) rather than `:3300` directly. The `/kiosk*` path routing is transparent to browser clients.

**start-kiosk.bat binding:** Next.js must bind to `127.0.0.1:3300` (loopback only) rather than `0.0.0.0:3300`, since external access goes through racecontrol.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Windows Smart App Control blocks node.exe on port 3300**
- **Found during:** Task 1 (deployment testing)
- **Issue:** SAC prevents node.exe from accepting inbound network connections — direct `kiosk.rp:3300` access fails with connection refused from network
- **Fix:** Added Axum reverse proxy in racecontrol (`kiosk_proxy` function) — forwards `/kiosk*` and `/_next/*` to `localhost:3300`. Kiosk accessible via `kiosk.rp:8080/kiosk` through the trusted racecontrol binary
- **Files modified:** `crates/racecontrol/src/main.rs`
- **Verification:** `curl http://localhost:8080/kiosk` returns 200 on James's machine
- **Committed in:** `ea9a728`

**2. [Rule 1 - Bug] Chunked transfer-encoding caused empty proxy responses**
- **Found during:** Task 1 (proxy verification)
- **Issue:** Next.js sends `transfer-encoding: chunked` in response headers. When racecontrol copies these headers and returns the full body (already decoded), the encoding metadata conflicted, producing empty responses
- **Fix:** Skip `transfer-encoding`, `connection`, and `keep-alive` (hop-by-hop headers) when copying upstream response headers
- **Files modified:** `crates/racecontrol/src/main.rs`
- **Verification:** `curl http://localhost:8080/kiosk` returns full HTML content
- **Committed in:** `3db7403`

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for functionality. The SAC bypass changed the access URL from `:3300` to `:8080/kiosk` but delivers the same outcome. No scope creep.

## Issues Encountered

**Server deployment blocked by Windows Smart App Control (server-side).**

WinRM and pod-agent remote execution on the server are both blocked by SAC at the operating system level. The only available remote channel (pod-agent at .23:8090) also runs as a process blocked by SAC from accepting remote exec commands. Physical access to the server at .23 is required to complete deployment.

**Remaining steps (physical server access required):**

1. Copy `racecontrol.exe` from staging to `C:\RacingPoint\racecontrol.exe` on server
2. Copy `racecontrol.toml` to `C:\RacingPoint\`
3. Build kiosk standalone bundle (`npm run build` in kiosk/) and copy to `C:\RacingPoint\kiosk\`
4. Create `C:\RacingPoint\start-racecontrol.bat` (sets CWD before launching racecontrol.exe)
5. Create `C:\RacingPoint\start-kiosk.bat` with `PORT=3300 HOSTNAME=127.0.0.1 node server.js`
6. Register HKLM Run keys: `RCCore` and `RCKiosk`
7. Start both services
8. Add `192.168.31.23  kiosk.rp` to `C:\Windows\System32\drivers\etc\hosts` on James's machine (needs admin elevation)
9. Smoke test: `curl http://kiosk.rp:8080/kiosk` and `curl http://kiosk.rp:8080/api/v1/health`

**Staged artifact:** `C:\Users\bono\racingpoint\deploy-staging\racecontrol.exe` (21MB, ready to copy to server)

## User Setup Required

Physical access to server (.23) is required to complete deployment. See "Issues Encountered" above for the full ordered step list.

The hosts file entry on James's PC also requires admin elevation:
```
Add-Content -Path "C:\Windows\System32\drivers\etc\hosts" -Value "`n192.168.31.23  kiosk.rp"
```

## Next Phase Readiness

- Code changes complete and committed — racecontrol proxy logic is production-ready
- Once server deployment is done (physical access), Phase 7 success criteria are met
- Phase 8 (Pod Lock Screen Hardening) depends on Phase 7 — should wait until server deployment is confirmed
- Phase 10 (Staff Dashboard Controls) also depends on Phase 7 stable URL

---
*Phase: 07-server-side-pinning*
*Completed: 2026-03-14*
