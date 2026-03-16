# Phase 27: Tailscale Mesh + Internet Fallback - Context

**Gathered:** 2026-03-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Install Tailscale as a Windows Service on all 8 pods, the Racing Point server (.23), and Bono's VPS — creating an encrypted mesh network. Route cloud_sync through Tailscale. Enable server-relayed remote commands from Bono to pods. Push telemetry, game state, and pod health to Bono in real time. Auth key rotation and Tailscale admin console configuration are out of scope for this phase.

</domain>

<decisions>
## Implementation Decisions

### Pod deployment method
- Use WinRM (port 5985 — confirmed open on all 8 pods) for silent push
- Same Windows admin credentials on all pods — one script deploys to all 8 + server
- Download Tailscale installer from James's HTTP deploy server (deploy-staging), run silent install via WinRM
- Pre-auth key baked into deploy script — pods join Racing Point Tailscale network automatically with zero interaction
- Install as **Windows Service** (Tailscale default on Windows) — survives reboots without user login, no Session 1 dependency
- Canary: deploy to Pod 8 first, verify Tailscale IP assigned and reachable, then roll to remaining pods

### cloud_sync routing
- `cloud_sync.rs` routes through Bono's **Tailscale IP** instead of public internet (72.60.101.58)
- Change is a config value in `racecontrol.toml` — `[cloud].api_url` points to `http://<bono-tailscale-ip>/api/v1`
- No parallel/fallback to public internet — Tailscale is the primary path. If Tailscale is down, sync fails gracefully (existing retry logic already handles this)

### New data streams to Bono
- **Real-time telemetry** — UDP game telemetry (lap times, sector splits, speed, g-force) from pods → server → pushed to Bono over Tailscale
- **Game state + pod health** — pod online/offline, session active, game running, FFB status — server pushes events as they happen
- **rc-agent remote commands** — Bono can send commands to pods via Tailscale; routed through server as relay (Bono → server Tailscale IP → server → pod via existing WebSocket). rc-agent LAN-only binding unchanged.

### Command relay architecture
- Bono → server Tailscale IP (new HTTP endpoint on racecontrol, Tailscale interface only)
- Server → pod via existing WebSocket/pod-agent infrastructure
- Consistent with Phase 01 decision: rc-agent binds to LAN only, pods never expose ports to internet directly

### Push model
- Server pushes events to Bono's VPS as they happen — not polling
- Events: session_start, session_end, lap_recorded, pod_offline, pod_online, billing_end
- Bono's VPS exposes a webhook endpoint; server posts events over Tailscale

### PWA game launches
- When customer books via PWA and pays, Bono's VPS triggers game launch via Tailscale → server relay → rc-agent
- Same command relay path as above — no new mechanism needed

### Claude's Discretion
- Tailscale device naming convention (pod-1 through pod-8, racing-point-server, bono-vps)
- Event payload schema for push events
- HTTP endpoint design on racecontrol for Bono's inbound commands
- Tailscale ACL policy (which devices can reach which)

</decisions>

<specifics>
## Specific Ideas

- Tailscale solves the bootstrap problem: it's a Windows Service, so pods are reachable even before a user logs in and rc-agent starts
- PWA game launch flow: customer pays on PWA → Bono's VPS → Tailscale → server → pod rc-agent. This is the key use case for the bidirectional command channel
- cloud_sync pointing to Tailscale IP means all billing/driver sync is now private — no more data in transit over public internet

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cloud_sync.rs` — existing 30s sync loop with reqwest 0.12; change `api_url` config to Tailscale IP
- `pod-agent :8090` HTTP exec endpoint — existing relay mechanism server can use to push commands to pods
- Existing WebSocket channel (racecontrol ↔ rc-agent) — server already has a path to every pod

### Established Patterns
- reqwest 0.12 for all HTTP — use same client for pushing events to Bono's webhook
- Tokio interval tasks — telemetry/health push can follow same pattern as cloud_sync loop
- TOML config (`racecontrol.toml`) — add `[bono]` section for Tailscale IP, webhook URL, push interval

### Integration Points
- `cloud_sync.rs` — update `api_url` to Bono's Tailscale IP
- New module: `bono_sync.rs` or extend `cloud_sync.rs` — push telemetry/health events to Bono
- New HTTP endpoint on racecontrol (bound to Tailscale interface) — receives commands from Bono, relays to pods

</code_context>

<deferred>
## Deferred Ideas

- Fallback trigger logic (Tailscale down → fall back to public internet) — decided not to implement; sync fails gracefully with existing retry
- AI debug logs streaming to Bono — noted, not selected for this phase
- Direct Bono → pod commands (bypassing server) — deferred; LAN-only binding for rc-agent is a standing decision
- Auth key rotation policy / Tailscale admin console access for Bono — operational concern, out of Phase 27 scope

</deferred>

---

*Phase: 27-tailscale-mesh-internet-fallback*
*Context gathered: 2026-03-16*
