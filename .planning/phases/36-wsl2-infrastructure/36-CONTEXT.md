# Phase 36: WSL2 Infrastructure - Context

**Gathered:** 2026-03-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up WSL2 Ubuntu 24.04 with mirrored networking on James's machine (.27), install salt-master 3008 LTS and salt-api (rest_cherrypy), open both firewall layers (Windows Defender + Hyper-V), configure auto-start on boot, and verify the full stack is reachable from the pod subnet. No Rust code in this phase — pure infrastructure.

</domain>

<decisions>
## Implementation Decisions

### Boot reliability
- Task Scheduler trigger at login (bono user), not at startup (SYSTEM)
- Salt is down between reboot and login — acceptable since James auto-logs in
- A startup script in WSL2 starts salt-master + salt-api: `wsl -e bash -c "sudo service salt-master start && sudo service salt-api start"`

### Salt-master crash recovery
- Claude's discretion — recommend systemd `Restart=always` inside WSL2 for both salt-master and salt-api units (zero cost, automatic recovery within seconds)

### Firewall scope
- Claude's discretion on Hyper-V firewall rule: recommend targeted ports (4505, 4506, 8000) over blanket allow-all — tighter security posture for venue LAN
- Claude's discretion on Windows Defender scope: recommend LAN-only (192.168.31.0/24 remote address) for defense-in-depth, consistent with existing firewall patterns in rc-agent's firewall.rs

### Resource allocation
- WSL2 RAM limit: 4 GB via `.wslconfig` `[wsl2] memory=4GB`
- WSL2 CPU: no limit (Salt is bursty, idle 99% of the time)
- Swap: Claude's discretion on swap size

### Salt-api auth (not discussed — Claude's discretion)
- Recommend PAM or sharedsecret auth for simplicity on a LAN-only deployment
- Token stored in racecontrol.toml `[salt]` section for Phase 38 to consume

### Claude's Discretion
- salt-master/salt-api systemd restart policy
- Hyper-V firewall: targeted ports vs blanket (recommended: targeted)
- Defender firewall: LAN-scoped vs any-source (recommended: LAN-scoped)
- Salt-api auth mechanism (PAM vs sharedsecret)
- WSL2 swap allocation
- Exact Task Scheduler XML configuration

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### WSL2 networking
- `.planning/research/STACK.md` — WSL2 mirrored mode setup, .wslconfig config, Hyper-V firewall command, Windows Defender firewall rules
- `.planning/research/ARCHITECTURE.md` — WSL2 networking decision with evidence, salt-api REST integration seam

### Salt installation
- `.planning/research/STACK.md` — Salt 3008 LTS install via bootstrap script, salt-api rest_cherrypy config, master config
- `.planning/research/SUMMARY.md` — Executive summary of all research, version requirements, pitfall summary

### Pitfalls
- `.planning/research/PITFALLS.md` — WSL2 NAT pitfall (P1), Hyper-V firewall silent drop (P2), Defender quarantine (P4), DHCP drift (P11)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/firewall.rs` — Existing pattern for Windows Firewall rule management via `netsh advfirewall`. Same pattern can inform the Defender rules for 4505/4506/8000.
- `deploy-staging/install.bat` — Current install script pattern for elevated auto-run. Task Scheduler setup can follow similar elevation pattern.

### Established Patterns
- HKLM Run keys used for rc-agent auto-start on pods — Task Scheduler is the equivalent pattern for WSL2 on James's machine
- `racecontrol.toml` config sections — `[salt]` section follows the existing `[bono]` section pattern for external service config

### Integration Points
- James's machine (.27) static IP — WSL2 mirrored mode will share this IP for salt-master
- Existing Tailscale mesh on .27 — Salt operates on LAN IPs (192.168.31.x), not Tailscale IPs. No conflict.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Research STACK.md and SUMMARY.md contain exact commands for every step.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 36-wsl2-infrastructure*
*Context gathered: 2026-03-17*
