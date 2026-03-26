# Phase 214: Bono Coordination - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Coordination protocol between James and Bono for the auto-detect pipeline. James is primary, Bono is failover. Prevents concurrent fixes on the same pod. Bono acts independently only when James is confirmed offline (relay timeout + Tailscale ping). Re-coordinates on James recovery.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase.

Key constraints:
- AUTO_DETECT_ACTIVE lock file written by James during runs, readable by Bono via relay
- Bono failover requires BOTH relay timeout AND Tailscale ping failure (100.125.108.37)
- Bono reads James completion marker before running its own detection
- On James recovery, Bono writes findings to shared channel and stops cloud-side fixes
- Existing bono-auto-detect.sh already checks James relay first -- extend it
- COORD-01: mutex/lock mechanism between James and Bono
- COORD-02: confirmed-offline detection (relay + Tailscale)
- COORD-03: findings handoff on recovery
- COORD-04: completion marker for skip-if-done logic

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- scripts/bono-auto-detect.sh -- already checks James relay first, delegates if alive
- scripts/auto-detect.sh -- PID guard (_acquire_run_lock), completion markers in audit/results/
- comms-link relay -- exec endpoint for state queries between James and Bono
- INBOX.md -- git-based async comms channel (Bono reads via git pull)

### Established Patterns
- PID file lock at /tmp/auto-detect.pid (James local)
- Cooldown file at audit/results/auto-detect-cooldown.json
- Bono relay: curl -s -X POST http://localhost:8766/relay/exec/run
- SSH fallback: ssh root@100.70.177.44 (when relay is down)
- Tailscale ping: tailscale ping 100.125.108.37

### Integration Points
- James auto-detect.sh writes AUTO_DETECT_ACTIVE and completion markers
- Bono bono-auto-detect.sh reads markers via relay before deciding to run
- Findings shared via INBOX.md git push + relay exec
- PM2 cron: James at 03:00 IST, Bono at 03:05 IST (5-min offset)

</code_context>

<specifics>
## Specific Ideas

- AUTO_DETECT_ACTIVE written at pipeline start, removed at pipeline end (trap cleanup)
- Completion marker: audit/results/last-run-summary.json with timestamp and outcome
- Bono checks completion marker age -- if < 10min old, skip run entirely
- Recovery handoff: Bono writes bono-findings.json, James reads on next boot/run

</specifics>

<deferred>
## Deferred Ideas

None -- coordination is scope-complete in this phase.

</deferred>
