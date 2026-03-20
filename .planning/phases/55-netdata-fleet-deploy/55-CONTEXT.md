# Phase 55: Netdata Fleet Deploy - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Netdata agent installed on racecontrol server (.23) and all 8 pods, collecting real-time CPU/RAM/disk/network metrics with auto-generated dashboards at :19999 on each host. Pods deployed via rc-agent :8090 exec without physical access.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation decisions delegated to Claude. Sensible defaults:

**Install Method:**
- Download Netdata Windows MSI from official source to James's deploy-staging HTTP server (:9998)
- Pods download from LAN (`http://192.168.31.27:9998/netdata.msi`) — faster, works offline
- Silent install: `msiexec /i netdata.msi /quiet /norestart`
- Server (.23) installed first (direct access or webterm), then pods via rc-agent :8090

**Fleet Deploy Strategy:**
- Server (.23) first — verify dashboard at :19999
- Pod 8 canary — verify via :8090 exec + check :19999 dashboard
- Pods 1-7 sequential — same pattern, one at a time
- Defender exclusion may be needed (like rc-agent install — `Add-MpPreference -ExclusionPath`)

**Dashboard Access:**
- Standalone per-pod — each pod runs its own Netdata at :19999, no central parent
- LAN-only access (no internet exposure — pods are on 192.168.31.x subnet)
- No password — LAN is trusted, venue-internal only
- James accesses dashboards by browsing to `http://192.168.31.{IP}:19999`

**Verification:**
- `curl -sf http://192.168.31.{IP}:19999/api/v1/info` returns JSON with version info
- E2E script to check all 9 hosts (server + 8 pods)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Research
- `.planning/research/FEATURES.md` §Monitoring — Netdata feature analysis, MSI silent install, <5% CPU
- `.planning/research/STACK.md` — Netdata vs Grafana comparison

### Deploy Infrastructure
- `C:\Users\bono\racingpoint\deploy-staging\` — staging area for MSI file
- `CLAUDE.md` §Network Map — all pod IPs for fleet deploy
- `CLAUDE.md` §Deployment Rules — verification sequence, canary-first

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-agent :8090 exec endpoint — used to run install commands on pods remotely
- deploy-staging HTTP server (:9998) — serves files to pods over LAN
- `deploy_pod.py` pattern — sequential pod operations with status tracking

### Established Patterns
- Silent MSI install via rc-agent exec (like Tailscale install — `install-tailscale.bat` in deploy-staging)
- Defender exclusions applied before install (proven pattern from install.bat v5)
- Pod 8 canary, then fleet rollout

### Integration Points
- rc-agent :8090 `/exec` on each pod
- deploy-staging :9998 serves the MSI file
- Netdata :19999 on each host after install

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard Netdata MSI deployment. Key success metric: `http://192.168.31.{IP}:19999` shows live dashboard on all 9 hosts.

</specifics>

<deferred>
## Deferred Ideas

- Netdata Cloud integration — optional, requires account signup, future consideration
- Netdata parent node for centralized view — standalone per-pod is simpler for now
- Custom Netdata dashboards — auto-generated dashboards are sufficient for v9.0

</deferred>

---

*Phase: 55-netdata-fleet-deploy*
*Context gathered: 2026-03-20*
