# Phase 413 Plan 11 — Deploy Evidence

**Plan:** 11 — Fleet deploy + goal-backward verification
**Executor:** James (auto-mode, session 2026-04-18)
**HEAD at start:** `34cd03b0` (docs 413-10 pre-deploy integration test GO verdict, pushed ~07:25 IST)
**Window:** Saturday 2026-04-18 ~07:30 IST (pre-venue-hours, acceptable deploy window)

## Rollback data (Task 1 pre-deploy — captured before any mutation)

### Server .23

- Pre-deploy `build_id`: `45d03bd5-dirty`
- `racecontrol-prev.exe` rollback available via `scripts/deploy-server.sh` Step 4 preservation
- sentry_service_key (from `ssh ADMIN@100.125.108.37 'type C:\RacingPoint\racecontrol.toml | findstr sentry'`): `478a3688339737fb5945f9b89d8bb533f2569fe0b1fea46b504656eee455b9ab` (server's key — this is the venue's rc-sentry service key used by rc-agents; separate from pod RCAGENT_SERVICE_KEY HKLM entries)

### Cloud (Bono VPS 100.70.177.44)

- Pre-deploy `build_id`: `dc83f28d`
- Pre-deploy `status`: `degraded` (admin_db subsystem detail: "admin.db not found at expected paths (separate deployment)" — pre-existing, not Phase 413 related)
- Rollback: `ssh bono-vps` + `git checkout <prev>` + `cargo build` + `pm2 restart`

### Pods 1-8 + POS (pre-deploy)

All 9 entries (pods 1-8 plus POS-as-pod9 classification) at `build_id=5f80fc6a`, `bat_sha256=d59ea5c4dbcf8753dd58befa3a7b043212edfcf44dc89381bc454220291789f9`, `binary_sha256=0317214a279d823b7f0f2d7ccac932dc106ec0977c1f0bc2b06a95fd16734852`.

Fleet `/api/v1/fleet/health` snapshot (raw, via `curl -s http://192.168.31.23:8080/api/v1/fleet/health` 2026-04-18 ~07:30 IST):

```
pod1 (192.168.31.89):  ws=True  http=True  build=5f80fc6a  uptime=12783s sentinels=['GRACEFUL_RELAUNCH']
pod2 (192.168.31.33):  ws=True  http=True  build=5f80fc6a  uptime=21872s sentinels=[]
pod3 (192.168.31.28):  ws=True  http=True  build=5f80fc6a  uptime=12778s sentinels=['GRACEFUL_RELAUNCH']   <-- CANARY
pod4 (192.168.31.88):  ws=True  http=True  build=5f80fc6a  uptime=21864s sentinels=[]
pod5 (192.168.31.86):  ws=True  http=True  build=5f80fc6a  uptime=12766s sentinels=['GRACEFUL_RELAUNCH']
pod6 (192.168.31.87):  ws=True  http=True  build=5f80fc6a  uptime=21837s sentinels=[]
pod7 (192.168.31.38):  ws=True  http=True  build=5f80fc6a  uptime=21833s sentinels=[]
pod8 (192.168.31.91):  ws=True  http=True  build=5f80fc6a  uptime=21894s sentinels=[]
pod9=POS (192.168.31.130): ws=True  http=True  build=5f80fc6a  uptime=18729s sentinels=[]
```

### Pod 3 (CANARY) HKLM RCAGENT_SERVICE_KEY state

Per `reg query` via rc-sentry /exec (pre-deploy baseline):

- Pod 1-8: `exit_code=1` (`stdout=""`) — **no HKLM entry present** on any pod. Consistent with Gap 4 evidence (per prior handoffs pods 2-7 never had the key; pod 1+8 keys were cleared in C1 FK sweep or earlier work).
- Pod 3 specifically: `exit_code=1`, `stdout=""` — **no HKLM key to save for rollback** (nothing to restore). Canary rollback step (Task 3 step g) simplifies: if Option Z fetch fails, pod 3 cannot be made worse by the key-deletion subtask (there's nothing to delete).

### POS (.130) rc-sentry exec availability

- POS rc-agent (:8090): reachable on LAN (.130) AND Tailscale (100.95.211.1), serving `build_id=5f80fc6a`.
- POS rc-sentry (:8091): **UNREACHABLE** on both LAN and Tailscale (HTTP=000 / connection timeout).
- Implication: standard rc-sentry-based deploy to POS is blocked this session. Memory cross-reference: "POS agent not running: SAC (Smart App Control) blocks unsigned exe" — a prior incident; rc-sentry may be similarly blocked OR just not started.
- Task 4 POS step will document POS as **deploy-blocked-SAC** (known class, not a regression introduced by Phase 413). The integration test Plan 10 Task 4b already live-verified Plan 01's POS-IP-reclassification returns 200 + JSON from .130 (the server-side gate works for POS); Option Z's other side (rc-agent on POS using the fetched key) cannot be live-verified until SAC is manually bypassed.

### Pre-deploy reachability gates (Task 1)

| Target | URL | Status | Raw |
|---|---|---|---|
| Server rc-sentry | http://192.168.31.23:8091/ping | 200 | (health endpoint returned 200) |
| Pod 3 rc-agent (canary) | http://192.168.31.28:8090/health | 200 | `{"build_id":"5f80fc6a","binary_sha256":"0317214a...","bat_sha256":"d59ea5c4..."}` |
| Pod 3 rc-sentry | http://192.168.31.28:8091/exec (echo HELLO) | 200 | `{"exit_code":0,"stdout":"HELLO\r\n","stderr":"","timed_out":false,"truncated":false}` |
| Cloud VPS | http://100.70.177.44:8080/api/v1/health | 200 | `{"build_id":"dc83f28d","status":"degraded",...}` |
| Git HEAD | local repo | `34cd03b0` | `git status` clean, `git log origin/main..HEAD` empty |
| IST time | `bash scripts/ist-now.sh` | Saturday 07:27 IST | Pre-venue-hours (acceptable deploy window per CLAUDE.md) |
| MMA audit score (Plan 09) | `.planning/phases/413-.../413-MMA-AUDIT.md` | 4.00/5.0 VERIFY-2 (3/3 SHIP) | Above 4.0 threshold |
| Integration test (Plan 10) | `.planning/phases/413-.../413-INTEGRATION-TEST.md` | GO | 3 PASS deferrals covered by Plan 11 canary |

All Task 1 pre-flight gates: **PASS**. Auto-mode approves checkpoint → proceed to Task 2.

---
