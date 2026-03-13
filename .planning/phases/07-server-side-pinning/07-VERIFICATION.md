---
phase: 07-server-side-pinning
verified: 2026-03-14T05:00:00+05:30
status: gaps_found
score: 3/7 must-haves verified
re_verification: false
gaps:
  - truth: "Staff can open http://kiosk.rp:3300/kiosk in a browser on James's machine and reach the staff kiosk"
    status: failed
    reason: "Hosts file entry for kiosk.rp is NOT present on James's machine. No entry in C:\\Windows\\System32\\drivers\\etc\\hosts. URL cannot resolve."
    artifacts:
      - path: "C:\\Windows\\System32\\drivers\\etc\\hosts (on James's PC)"
        issue: "kiosk.rp entry missing — grep found NO_KIOSK_ENTRY"
    missing:
      - "Add '192.168.31.23  kiosk.rp' to C:\\Windows\\System32\\drivers\\etc\\hosts (requires admin elevation)"

  - truth: "After a full server reboot, the kiosk is accessible within 60 seconds — no manual start needed"
    status: failed
    reason: "Server deployment is blocked by Windows Smart App Control (SAC). The new rc-core binary and kiosk standalone bundle have NOT been copied to the server. HKLM Run keys for RCCore and RCKiosk have NOT been registered on the server. Services are not running on server. Physical server access is required."
    artifacts:
      - path: "C:\\RacingPoint\\rc-core.exe (on server .23)"
        issue: "Binary not deployed — server deployment blocked by SAC"
      - path: "C:\\RacingPoint\\kiosk\\server.js (on server .23)"
        issue: "Kiosk standalone bundle not deployed — server deployment blocked by SAC"
      - path: "C:\\RacingPoint\\start-rc-core.bat (on server .23)"
        issue: "Startup wrapper not created — requires physical server access"
      - path: "C:\\RacingPoint\\start-kiosk.bat (on server .23)"
        issue: "Startup wrapper not created — requires physical server access"
    missing:
      - "Physical access to server .23 required to run the 9-step deploy list from 07-02-SUMMARY.md"
      - "Copy racecontrol.exe from deploy-staging to C:\\RacingPoint\\rc-core.exe on server"
      - "Copy racecontrol.toml to C:\\RacingPoint\\ on server"
      - "Transfer and extract kiosk-standalone.zip to C:\\RacingPoint\\kiosk\\ on server"
      - "Create C:\\RacingPoint\\start-rc-core.bat wrapper"
      - "Create C:\\RacingPoint\\start-kiosk.bat with PORT=3300 HOSTNAME=127.0.0.1"
      - "Register HKLM Run keys RCCore and RCKiosk on server"
      - "Start both services and verify ports 8080 + 3300 listening"

  - truth: "rc-core compiles and all tests pass"
    status: failed
    reason: "rc-core fails to compile: non-exhaustive patterns in crates/rc-core/src/ws/mod.rs — AgentMessage::ContentManifest(_) not covered in match arm at line 117. This was introduced by commit 25a6f79 (feat(05-01): add ContentManifest types and AgentMessage variant) which added the variant to rc-common but did not update the match in rc-core ws/mod.rs. The staged binary (21MB, Mar 14 03:56) was built before this regression was introduced."
    artifacts:
      - path: "crates/rc-core/src/ws/mod.rs"
        issue: "Match arm at line 117 missing AgentMessage::ContentManifest(_) arm — compile error E0004"
    missing:
      - "Add ContentManifest arm to match block in ws/mod.rs (either handle it or use a wildcard _ => {} catch-all)"
      - "Re-run cargo test -p rc-core to confirm all tests pass"
      - "Rebuild racecontrol.exe and re-stage"

  - truth: "Server IP address remains .23 across router restarts (DHCP reservation confirmed)"
    status: partial
    reason: "DHCP reservation was successfully created at the TP-Link EX220 router (MAC BC:FC:E7:2C:F2:CE bound to .23). Server successfully moved from .4 to .23 and pod-agent confirmed responding. However the ROADMAP Success Criterion requires 'confirmed in router admin' — this was done during Plan 01 execution but cannot be programmatically re-verified from James's machine right now."
    artifacts: []
    missing:
      - "Human spot-check: confirm DHCP reservation still shows in TP-Link admin (http://192.168.31.1) as it could have been lost if router was reset"

human_verification:
  - test: "Confirm DHCP reservation persists in TP-Link router admin"
    expected: "TP-Link admin -> Network -> LAN Settings -> Address Reservation shows MAC BC:FC:E7:2C:F2:CE bound to 192.168.31.23, status enabled"
    why_human: "Router admin has no API. Reservation was confirmed during Plan 01 execution but cannot be verified remotely."

  - test: "After server deployment: smoke test kiosk at http://kiosk.rp:8080/kiosk"
    expected: "Browser opens http://kiosk.rp:8080/kiosk and shows the Racing Point staff kiosk UI (HTML page, not an error)"
    why_human: "Server deployment requires physical access first. After deployment, full browser test needed to confirm kiosk UI renders correctly, not just HTTP 200."

  - test: "After server deployment: reboot server and verify 60-second recovery"
    expected: "After full server reboot, http://kiosk.rp:8080/kiosk returns 200 within 60 seconds with no manual intervention"
    why_human: "Requires physical server reboot; can only verify after HKLM Run keys are registered and tested."
---

# Phase 7: Server-Side Pinning Verification Report

**Phase Goal:** The staff kiosk is reachable at a stable, named address from any device on the LAN and survives server reboots without manual intervention — with zero changes to pods
**Verified:** 2026-03-14T05:00:00+05:30
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Staff can open http://kiosk.rp:8080/kiosk and reach the kiosk (actual URL after SAC bypass) | FAILED | hosts file has no kiosk.rp entry; server not deployed |
| 2 | After server reboot, kiosk accessible within 60 seconds — no manual start | FAILED | Server deployment blocked by SAC; no binaries on server, no Run keys registered |
| 3 | Server IP remains .23 across reboots (DHCP reservation done) | PARTIAL | Reservation created in Plan 01, server confirmed at .23 — but can't re-verify programmatically |
| 4 | Kiosk runs from production Next.js build, not dev server | VERIFIED | kiosk-standalone.zip staged (10MB); next.config.ts has output:"standalone"; Plan 01 confirms server runs `next start` |
| 5 | CORS predicate allows kiosk.rp origin in rc-core | VERIFIED | main.rs line 334: `origin.starts_with("http://kiosk.rp")` confirmed present in both commits ea9a728 and 3db7403 |
| 6 | Reverse proxy in rc-core routes /kiosk* and /_next/* to localhost:3300 | VERIFIED | kiosk_proxy function (lines 94-156) wired as .fallback() at line 325; hop-by-hop headers correctly filtered |
| 7 | rc-core compiles and all tests pass | FAILED | cargo build -p rc-core fails with E0004: ws/mod.rs line 117 missing ContentManifest arm |

**Score:** 3/7 truths verified (2 code truths verified, 1 infrastructure truth partial, 3 failed)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-core/src/main.rs` | CORS + reverse proxy for kiosk.rp | VERIFIED | kiosk.rp in CORS (line 334), kiosk_proxy function (lines 94-156), .fallback() wired (line 325) |
| `crates/rc-core/src/ws/mod.rs` | Compiles cleanly with ContentManifest | STUB/BROKEN | Missing match arm for AgentMessage::ContentManifest(_) — compile error E0004 |
| `deploy-staging/racecontrol.exe` | Staged release binary (21MB) | VERIFIED | Exists, 21,167,616 bytes, dated Mar 14 03:56 — but built before ContentManifest regression |
| `deploy-staging/kiosk-standalone.zip` | Kiosk standalone bundle | VERIFIED | Exists, 10,635,940 bytes, contains server.js (confirmed via unzip -l) |
| `deploy-staging/racecontrol.toml` | Config for server deployment | VERIFIED | Exists, 1,618 bytes |
| `deploy-staging/node-v22.zip` | Node.js installer for server | VERIFIED | Exists, 34,906,389 bytes |
| `C:\RacingPoint\rc-core.exe (server)` | Deployed rc-core binary | MISSING | Server deployment blocked by Windows SAC |
| `C:\RacingPoint\kiosk\server.js (server)` | Deployed kiosk standalone | MISSING | Server deployment blocked by Windows SAC |
| `C:\RacingPoint\start-rc-core.bat (server)` | rc-core startup wrapper | MISSING | Not created — physical access required |
| `C:\RacingPoint\start-kiosk.bat (server)` | Kiosk startup wrapper | MISSING | Not created — physical access required |
| `C:\Windows\System32\drivers\etc\hosts (James)` | kiosk.rp hostname entry | MISSING | grep returned NO_KIOSK_ENTRY — entry not present |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| Browser at kiosk.rp | rc-core on :8080 | hosts file mapping | NOT_WIRED | hosts entry absent on James's machine |
| rc-core .fallback() | kiosk_proxy function | .fallback(kiosk_proxy) at line 325 | WIRED | Confirmed in main.rs |
| kiosk_proxy | localhost:3300 | reqwest to http://127.0.0.1:3300 | WIRED | Line 107: `format!("http://127.0.0.1:3300{}", path_and_query)` |
| CORS predicate | kiosk.rp origin | starts_with check in main.rs | WIRED | Line 334 confirmed present |
| rc-core build | ws/mod.rs match | AgentMessage variants | BROKEN | ContentManifest arm missing — won't compile |
| HKLM Run keys on server | start-rc-core.bat + start-kiosk.bat | Windows registry Run key | NOT_WIRED | Keys not registered — server deployment not done |
| Kiosk JS (api.ts) | rc-core on :8080 | window.location.hostname | WIRED | api.ts uses `http://${window.location.hostname}:8080` — will resolve to kiosk.rp:8080 correctly |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| HOST-01 | 07-02 | Staff kiosk runs as a production Next.js build on Server (.23) — no dev server | PARTIAL | Build artifacts staged; server deployment not done; Plan 01 notes server currently runs `next start` (production) but new binary not yet deployed |
| HOST-02 | 07-02 | Staff kiosk auto-starts on Server (.23) boot via HKLM Run key (Session 1) | FAILED | HKLM Run keys RCCore and RCKiosk NOT registered on server — deployment blocked by SAC |
| HOST-03 | 07-01 | Server (.23) IP is pinned via DHCP reservation at the router | PARTIAL | Reservation created during Plan 01 (TP-Link EX220, MAC BC:FC:E7:2C:F2:CE → .23); server confirmed at .23; REQUIREMENTS.md still shows `[ ]` unchecked for HOST-03 — not updated after completion |
| HOST-04 | 07-02 | Staff can access the kiosk at kiosk.rp from any device on the LAN via hosts file entries | FAILED | hosts file entry missing on James's machine; server not deployed |

Note: REQUIREMENTS.md traceability table shows HOST-03 as `[ ] Pending` even though Plan 07-01 claims to have fulfilled it. This is a documentation inconsistency but does not affect the code state.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/rc-core/src/ws/mod.rs` | 117 | Non-exhaustive match — AgentMessage::ContentManifest(_) not handled | Blocker | rc-core will not compile; staged binary is stale (pre-regression); any rebuild fails |
| `crates/rc-core/src/main.rs` | — | No anti-patterns in Phase 7 changes | — | Clean implementation |

### Human Verification Required

#### 1. DHCP Reservation Persistence Check

**Test:** Open http://192.168.31.1 in browser, navigate to Network > LAN Settings > Address Reservation. Confirm MAC BC:FC:E7:2C:F2:CE is bound to 192.168.31.23 and is enabled (green).

**Expected:** Reservation entry visible and active. If missing, re-add it.

**Why human:** Router admin has no programmatic API; reservation could have been lost if router was reset since Plan 01.

#### 2. Post-Deployment Smoke Test

**Test:** After physical server deployment, open http://kiosk.rp:8080/kiosk in a browser on James's machine.

**Expected:** Racing Point staff kiosk UI loads (not a browser error or 502 Bad Gateway).

**Why human:** Full browser rendering check — curl returning 200 is necessary but not sufficient.

#### 3. Auto-Start Reboot Test

**Test:** After HKLM Run keys are registered on the server, perform a full server reboot and measure time to kiosk availability.

**Expected:** `curl http://kiosk.rp:8080/kiosk` returns 200 within 60 seconds of the desktop appearing — with no manual intervention.

**Why human:** Requires physical server reboot; tests the Session 1 auto-login + Run key chain.

---

## Gaps Summary

Phase 7 is **code-complete but not deployed**. Three distinct gap categories:

**Gap 1 — Compile regression (blocker):** commit `25a6f79` added `AgentMessage::ContentManifest(_)` to rc-common but did not add a matching arm in rc-core's `ws/mod.rs` line 117. This is a non-exhaustive pattern error (E0004). The staged `racecontrol.exe` (21MB, Mar 14 03:56) was built *before* this regression and is currently valid, but any future `cargo build -p rc-core` will fail. This must be fixed before the next deploy cycle.

**Gap 2 — Server deployment pending (blocker for SUCCESS criteria 1, 2, and 4):** Windows Smart App Control on the server blocks WinRM and pod-agent remote execution. All 9 deployment steps listed in 07-02-SUMMARY.md require physical server access. Nothing has been deployed to the server yet — no binary, no kiosk bundle, no startup wrappers, no Run keys. Until this is done, the phase goal is not met.

**Gap 3 — Hosts file entry missing (blocker for SUCCESS criterion 1 and 4):** `kiosk.rp` is not in `C:\Windows\System32\drivers\etc\hosts` on James's machine. Without this, the URL `http://kiosk.rp:8080/kiosk` cannot resolve. This requires a one-time admin-elevated PowerShell command:
```
Add-Content -Path "C:\Windows\System32\drivers\etc\hosts" -Value "`n192.168.31.23  kiosk.rp"
```

**What IS done and verified:**
- DHCP reservation created (server confirmed at .23 during Plan 01)
- `kiosk_proxy` reverse proxy implemented and wired in rc-core (commits `ea9a728` + `3db7403`)
- CORS predicate updated to include `kiosk.rp`
- Hop-by-hop header filtering working (prevents empty responses from chunked Next.js)
- All staging artifacts present: `racecontrol.exe` (21MB), `kiosk-standalone.zip`, `racecontrol.toml`, `node-v22.zip`
- kiosk configured as standalone production build (`output: "standalone"` in next.config.ts)
- rc-common and rc-agent: 76 tests pass

---
*Verified: 2026-03-14T05:00:00+05:30*
*Verifier: Claude (gsd-verifier)*
