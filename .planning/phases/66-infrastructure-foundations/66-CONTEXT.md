# Phase 66: Infrastructure Foundations - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Pin server .23 IP permanently (DHCP reservation + static IP), verify James can execute commands on server .23 via rc-agent :8090 (over Tailscale or LAN), and verify James can execute commands on Bono's VPS via comms-link exec_request protocol. This is the mandatory gate for all v10.0 phases.

</domain>

<decisions>
## Implementation Decisions

### DHCP Stability
- Belt-and-suspenders: static IP on server NIC **AND** TP-Link router DHCP reservation for MAC 10-FF-E0-80-B1-A7 → 192.168.31.23
- Claude's discretion on whether to disable DHCP client on the server NIC or keep it with reservation
- Claude's discretion on DHCP pool range adjustment if router doesn't support reservation outside pool
- Server MAC must be verified on-site before setting reservation (documented change 2026-03-17 to Gigabyte Z870 NIC)

### Remote Exec: James → Server .23
- Use rc-agent :8090 exec endpoint (already running on server)
- Server Tailscale IP unknown — discover during phase execution via `tailscale status` on server
- If Tailscale IP available, prefer it for consistency with future failover paths; LAN .23 is fallback
- James does NOT need direct exec on individual pods — only through racecontrol or server

### Remote Exec: James → Bono VPS
- Use comms-link exec_request protocol (already shipping in v2.0)
- Add failover-specific commands to COMMAND_REGISTRY: `activate_failover`, `deactivate_failover`, `racecontrol_health`, `config_apply`
- Keep existing commands (git_pull, restart_daemon, health_check) alongside new ones
- Approval tiers for new commands: `racecontrol_health` = AUTO, `config_apply` = NOTIFY, `activate_failover`/`deactivate_failover` = NOTIFY

### Verification
- DHCP: reboot server, wait 2 minutes, ping .23 from James — manual verification, definitive gate
- Bono exec: verify heartbeat flowing (channel alive) + send exec_request round-trip (command path works)
- Server exec: POST to rc-agent :8090 via discovered IP, confirm output returned

### Claude's Discretion
- Exact PowerShell commands for static IP assignment on server
- Whether to use `netsh` or `New-NetIPAddress` for static IP
- DNS settings for the static NIC config (likely 192.168.31.1 as gateway/DNS)
- TP-Link router admin UI steps (model-dependent)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Network Infrastructure
- `CLAUDE.md` §Network Map — All pod/server/James IPs, MACs, and Tailscale addresses
- `.planning/research/STACK.md` — DHCP reservation approaches, OpenSSH alternatives, Tailscale limitations
- `.planning/research/PITFALLS.md` — DHCP reservation gotchas, MAC change history, false positive risks

### Remote Execution
- `../comms-link/shared/exec-protocol.js` — COMMAND_REGISTRY, approval tiers, buildSafeEnv()
- `../comms-link/shared/protocol.js` — MessageType.exec_request, exec_result, exec_approval
- `crates/rc-agent/src/remote_ops.rs` — Existing :8090 HTTP exec endpoint on server + all pods

### Research
- `.planning/research/ARCHITECTURE.md` — Integration points, failover architecture, build order
- `.planning/research/FEATURES.md` — Health check patterns, failover trigger mechanics

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `remote_ops.rs` (rc-agent): HTTP exec endpoint on :8090 — already handles command execution with slot-based concurrency and CLOSE_WAIT prevention
- `comms-link/shared/exec-protocol.js`: COMMAND_REGISTRY with 3-tier approval system — extend with failover commands
- `comms-link/james/exec-handler.js`: James-side exec handler — processes incoming exec_requests

### Established Patterns
- COMMAND_REGISTRY: frozen object with binary + args + tier + timeout per command — add new commands following this pattern
- comms-link protocol envelope: `createMessage(type, from, payload)` with UUID + timestamp
- rc-agent :8090 uses `exec_command()` with slot-based concurrency (MAX_CONCURRENT_EXECS=8)

### Integration Points
- James → Server .23: HTTP POST to rc-agent :8090/exec with command in body
- James → Bono: comms-link WebSocket → exec_request message → Bono exec-handler processes → exec_result returned
- Phase 67 (Config Sync) depends on Bono exec path being verified here
- Phase 68 (SwitchController) depends on server .23 IP being stable

</code_context>

<specifics>
## Specific Ideas

- Server reboot test is the definitive DHCP gate — if .23 doesn't come back after reboot, nothing else matters
- Failover commands in COMMAND_REGISTRY should follow existing naming convention (snake_case, descriptive)
- `activate_failover` and `deactivate_failover` are the critical Bono-side commands that Phase 69 will use

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 66-infrastructure-foundations*
*Context gathered: 2026-03-20*
