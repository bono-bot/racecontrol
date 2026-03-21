# 111 Validation Log — Pod 8 Canary Deploy

## Deploy Summary

| Field | Value |
|-------|-------|
| Deploy timestamp | 2026-03-21 22:47 IST |
| Build commit | 243f03d (HEAD at build time) |
| Binary size (local) | 11,312,128 bytes |
| Binary size (remote) | 11,312,128 bytes (MATCH) |
| Pod 8 IP | 192.168.31.91 |
| Deploy method | SSH tunnel: James → server(.23) → Pod 8(:8090) + service key |

## Build Details

- Build command: `cargo build --release --bin rc-agent`
- Built from HEAD: `6dc74d1` (at build time HEAD was `243f03d` — docs(115) commit)
- Tests run: `cargo test -p rc-common` — 147 tests passed, 0 failed
- Binary embeds build_id from `git rev-parse --short HEAD` at compile time
- Key features included: safe mode (Phase 109), GPO lockdown (Phase 108), telemetry gating (Phase 110)

## Deploy Sequence

1. Kill: rc-agent killed via `do-swap.bat` (taskkill /F /IM rc-agent.exe)
2. Delete: old binary deleted during swap
3. Download: `curl -s -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe`
4. Size check: 11,312,128 bytes — matches local build ✓
5. Swap: `move /Y rc-agent-new.exe rc-agent.exe`
6. Start: `start /D C:\RacingPoint rc-agent.exe`

## Verification Results

### Pod 8 Direct Health (http://192.168.31.91:8090/health)

```json
{
  "build_id": "243f03d",
  "exec_slots_available": 8,
  "exec_slots_total": 8,
  "status": "ok",
  "uptime_secs": 84,
  "version": "0.1.0"
}
```

### Fleet Health Response for Pod 8

```json
{
  "build_id": "243f03d",
  "crash_recovery": false,
  "http_reachable": true,
  "in_maintenance": false,
  "ip_address": "192.168.31.91",
  "last_http_check": "2026-03-21T17:17:01.457300400+00:00",
  "last_seen": "2026-03-21T17:17:12.415881+00:00",
  "maintenance_failures": [],
  "pod_id": "pod_8",
  "pod_number": 8,
  "uptime_secs": 89,
  "version": "0.1.0",
  "ws_connected": true
}
```

## Acceptance Criteria Status

- [x] Pod 8 shows `ws_connected: true` in fleet health
- [x] Pod 8 direct health endpoint returns 200
- [x] Binary size on Pod 8 matches local build (11,312,128 bytes)
- [x] uptime_secs > 30 (no crash loop): uptime_secs = 89
- [x] 111-validation-log.md created with deploy details

## Notes

- Deploy required SSH tunnel via server (Pod 8 HTTP port 8090 was not directly accessible from James)
- Service key (`4f455098b346319d6166469755806427`) required for pod /exec endpoint
- `do-swap.bat` on Pod 8 used for atomic binary swap
- `crash_recovery: false` — normal after fresh restart (no crash detected yet)
- Plan's fleet exec endpoint (`POST /api/v1/fleet/exec`) does not exist; actual endpoint is `POST /pods/{id}/exec` with JWT auth (or direct pod HTTP with service key)
