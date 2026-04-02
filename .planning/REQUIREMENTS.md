# Requirements: v40.0 Game Launch Reliability

**Defined:** 2026-04-03
**Core Value:** Fix 4 critical architectural issues in the game launch workflow that cause silent failures, revenue loss, and pod lockouts

## WS Command Delivery (WSCMD)
- [ ] **WSCMD-01**: Server waits for agent ACK (with 5s timeout) before returning success on `/games/launch`
- [ ] **WSCMD-02**: Server waits for agent ACK (with 5s timeout) before returning success on `/games/stop`
- [ ] **WSCMD-03**: If agent doesn't ACK within timeout, server returns error to caller (not silent success)
- [ ] **WSCMD-04**: ACK protocol is backward compatible — old agents that don't ACK trigger timeout path gracefully

## Game State Resilience (GSTATE)
- [ ] **GSTATE-01**: GameTracker stuck in `Launching` for >3 minutes auto-transitions to `Error` with clear message
- [ ] **GSTATE-02**: On WS reconnect, reconciliation correctly merges pod's actual game state with server tracker (not blind overwrite)
- [ ] **GSTATE-03**: `/games/stop` clears the GameTracker entry on success (not just transition to Stopping)

## Billing Atomicity (BATOM)
- [ ] **BATOM-01**: `start_billing` holds a consistent view — no window where concurrent requests can create duplicate sessions for the same pod
- [ ] **BATOM-02**: If a billing session already exists for a pod (any status), new `start_billing` returns clear error

## Launch-Billing Coordination (LBILL)
- [ ] **LBILL-01**: Stale session cancel (5-min timeout) checks if game process is alive on the pod before cancelling
- [ ] **LBILL-02**: If game is alive but not yet Live, extend the waiting period (up to 10 minutes total for slow-loading games)
- [ ] **LBILL-03**: If game is dead AND session is waiting_for_game >5 min, cancel with full wallet refund (existing fix from 8184d4f3)

## Future Requirements
- WS command delivery for other endpoints (fleet/exec, config push)
- Billing reconciliation dashboard showing desync events
- Per-game launch timeout configuration

## Out of Scope
- Full bidirectional NTP-style time sync (200ms desync acceptable)
- Rewriting the WS protocol to binary format (JSON sufficient for 8 pods)
- Client-side retry logic in kiosk (server handles retries)

## Traceability
| REQ | Phase | Plan | Status |
|-----|-------|------|--------|
| LBILL-01 | 311 | — | Pending |
| LBILL-02 | 311 | — | Pending |
| LBILL-03 | 311 | — | Pending |
| WSCMD-01 | 312 | — | Pending |
| WSCMD-02 | 312 | — | Pending |
| WSCMD-03 | 312 | — | Pending |
| WSCMD-04 | 312 | — | Pending |
| GSTATE-01 | 313 | — | Pending |
| GSTATE-02 | 313 | — | Pending |
| GSTATE-03 | 313 | — | Pending |
| BATOM-01 | 314 | — | Pending |
| BATOM-02 | 314 | — | Pending |
