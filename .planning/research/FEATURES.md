# Feature Research — Connectivity & Redundancy (v10.0)

**Domain:** Venue operations — LAN server health monitoring, config sync, auto-failover (local→cloud), and failback for 8-pod sim racing venue
**Researched:** 2026-03-20
**Confidence:** HIGH (patterns verified via Tailscale official docs, AWS/GCP health check documentation, production failover research, and codebase analysis)

---

## Context: What Already Exists vs What This Milestone Adds

v10.0 is not building health monitoring from scratch. The venue already has meaningful redundancy infrastructure. The gaps are specific and well-defined.

### What Already Exists (Do NOT Duplicate)

| System | What It Does | Gap It Leaves |
|--------|-------------|---------------|
| UDP heartbeat (udp_heartbeat.rs) | rc-agent sends heartbeat every 6s; racecontrol marks pod offline if no heartbeat for 6s | Only monitors pod→server. No server→cloud health path. |
| WebSocket auto-reconnect with backoff | rc-agent reconnects to `core.url` with exponential backoff (1s→30s cap) | Only reconnects to the same URL. Cannot switch to a backup URL on persistent failure. |
| cloud_sync.rs (every 30s) | Pulls/pushes laps, drivers, pricing, wallets to Bono's VPS | Only syncs data rows — NOT racecontrol.toml config. Does not sync venue configuration. |
| bono_relay.rs (event push over Tailscale) | Pushes session/lap/pod events to Bono webhook; Bono can send relay commands back | One-directional operational events. Not a failover channel — no health signaling. |
| pod_monitor.rs + pod_healer.rs | Detects stale pods via WS/heartbeat; triggers restart sequences | Only watches pods from the server's perspective. James's machine (.27) does not independently monitor the server (.23). |
| Email alerts (email_alerts.rs) | Fires on billing failures, pod crashes | No alerts for server itself going down. No failover-trigger alerts. |
| rc-agent remote_ops :8090 | HTTP exec endpoint for remote pod commands | Sends commands to pods but has no concept of failover server. |
| Tailscale mesh (all pods, Phase 27) | Pods accessible via Tailscale IPs | Tailscale installed but not configured as a failover channel. Pods still connect to LAN IP for WS. |

### What v10.0 Adds

The four-feature shape of this milestone:
1. **DHCP fix** — Server .23 gets stable IP (MAC reservation). Foundation for everything else.
2. **Health monitoring** — James's machine (.27) continuously monitors server .23. Detects outage.
3. **Config sync** — racecontrol.toml pushed to Bono's VPS so cloud mirrors venue config.
4. **Auto-failover + failback** — Pods switch to Bono's VPS when .23 goes down; switch back when it recovers.

---

## Feature Landscape

### Table Stakes (Required for a Reliable Venue)

Features the system must have for Connectivity & Redundancy to be meaningful. Missing any of these means failover either never triggers, triggers incorrectly, or doesn't work when needed.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Stable server IP (DHCP reservation)** | Every other feature depends on .23 having a predictable address. DHCP drift has already caused outages (see MEMORY.md: "drifted .51→.23→.4→.23"). Failover logic that monitors the wrong IP is useless. | LOW | MAC reservation in DHCP server (router at .1). New MAC is 10-FF-E0-80-B1-A7 (changed 2026-03-17). One-time config in router UI. Already in v10.0 requirements. |
| **Continuous server health check from James (.27)** | Without an independent observer, nobody knows the server is down until a customer reports a problem. James's machine (.27) is always on and on the LAN — ideal monitor. | LOW | HTTP poll to `http://192.168.31.23:8080/health` on a tight interval (5–10s). Mark server DOWN after 3 consecutive failures (~15–30s total). Reset to UP after 2 consecutive successes. Uses `reqwest` in a tokio background task; no new dependencies. |
| **Multi-factor liveness check (not just one probe type)** | A single TCP/HTTP check can pass even when the service is functionally broken (port open but not processing requests). A process that has deadlocked will still accept TCP connections. | LOW | Check both: (1) HTTP GET `/health` returns 200, AND (2) response latency < 2s. Optional: check ICMP ping as a third signal — if HTTP fails but ICMP passes, the machine is up but the service is stuck. If ICMP also fails, the machine is unreachable. Different recovery actions for each. |
| **Failover notification to Uday** | Uday must know when the venue is running on cloud backup so he can decide whether to investigate or wait. Silent failover leaves Uday confused about why things are behaving differently. | LOW | Send email via existing `send_email.js` when failover triggers. Message: "Server .23 offline — pods switched to cloud backup at [IST timestamp]." Rate-limit: max 1 alert per 5 minutes. Uses existing alert infrastructure. |
| **Failback to local server** | Auto-failover without failback means the venue permanently runs on cloud after the first outage. Cloud has higher latency and lower throughput than LAN — it is emergency infrastructure, not primary. | LOW | When server .23 health check recovers (2 consecutive successes after DOWN state), send a failback signal to pods to switch back to LAN. Notify Uday: "Server .23 recovered — pods returned to local at [IST timestamp]." |
| **Failback notification to Uday** | Uday needs to know when the venue has returned to normal operation, not just when it failed over. | LOW | Mirror of failover notification. Same channel (email). Add WhatsApp via existing bot when confidence in failover/failback is established. |
| **Pods accept a backup server URL** | rc-agent currently has one `core.url` in config. Failover requires pods to know about a secondary URL (Bono's VPS WebSocket endpoint) before the outage — they cannot dynamically discover it at failover time. | MEDIUM | Add `core.fallback_url` (optional) to rc-agent TOML config. When the monitor signals failover, rc-agent switches its active WS URL to `fallback_url` and reconnects. When failback is signaled, switch back to `core.url`. Implementation path: extend `CoreConfig` struct, add URL-selector logic in the WS reconnect loop. |
| **Config sync: racecontrol.toml pushed to Bono** | Bono's VPS is only useful as a failover target if it has the same venue config (pod IPs, pricing tiers, billing rates, session rules). Without config sync, failover lands on a cloud instance with default/stale settings. | MEDIUM | Watch for racecontrol.toml changes (on write or on startup). Push to Bono's VPS via a dedicated API endpoint (`POST /admin/sync-config`). Bono applies the config on receipt. Frequency: on every restart of racecontrol + on any config change. Not a continuous loop — event-driven. |

### Differentiators (Competitive Advantage vs Simple Failover)

Features that make this failover system more robust than naive "switch URL on timeout."

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Hysteresis on failover trigger (not single-failure)** | Without hysteresis, a momentary network blip (1 failed probe) causes unnecessary failover, disrupting all 8 pods mid-session. The existing RELAY_DOWN_THRESHOLD pattern in cloud_sync.rs (3 failures before declaring down, 2 successes before declaring up) is proven and should be reused exactly. | LOW | Already implemented in cloud_sync.rs as `RELAY_DOWN_THRESHOLD = 3` and `RELAY_UP_THRESHOLD = 2`. Apply same pattern to the server health monitor. Copy the state machine, not the code. |
| **Grace period during active billing sessions** | If a pod has an active billing session and the server fails over, abruptly switching the pod's WS target during an active session risks losing the session's lap data. Grace period: complete or snapshot the current session before switching URLs. | MEDIUM | In rc-agent failover logic: if `session_active == true`, send a session-state snapshot to both old and new server URLs before switching. If old URL is unreachable, proceed with snapshot to new URL only. This prevents lost laps. |
| **Tailscale SSH remote exec from James to server** | James can currently not remotely restart racecontrol on the server. If the service hangs (port open but no responses), James must physically access the server. Tailscale SSH gives James a remote exec channel over the mesh. | MEDIUM | Tailscale SSH is a Tailscale feature (not OS SSH). Enable on server with `tailscale up --ssh`. James's machine (.27) already has Tailscale. Connection via `ssh admin@<tailscale-ip>`. Prerequisite: confirm Phase 27 Tailscale deployment is complete on server. |
| **Failover-aware config on Bono's VPS (pod IP whitelist)** | Bono's cloud instance does not know the venue's pod IPs by default. When pods connect over Tailscale during failover, Bono needs to recognize them as legitimate Racing Point pods, not unknown agents. Config sync should include pod MAC/IP/Tailscale-IP mapping. | MEDIUM | Include `pods` section of racecontrol.toml in config sync payload. Bono validates incoming WS connections against this pod whitelist on the shared secret or pod identifier in the agent handshake. |
| **Health check endpoint on Bono's VPS** | James's monitor checks server .23. It should also check that the failover target (Bono's VPS) is healthy before triggering failover — switching to a broken cloud instance is worse than staying disconnected. | LOW | HTTP GET to `https://app.racingpoint.cloud/health` before triggering failover. If both .23 and cloud are unreachable, do NOT attempt failover — notify Uday of "full outage, both local and cloud down." |
| **Split-brain prevention** | If server .23 recovers but pods don't receive the failback signal (e.g., Tailscale mesh is degraded), some pods may be on .23 and some on cloud simultaneously. Two server instances will both record sessions and laps, creating conflicting records. | MEDIUM | Prevention: failback should be server-initiated, not client-initiated. Server .23 on recovery broadcasts a "I am primary again" message over Tailscale. Bono's cloud enters read-only mode (stops accepting new sessions) when it receives this signal. Pods reconnect to .23. Simple rule: exactly one instance accepts session creation at a time. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **DNS-based failover (update A record on outage)** | "Standard approach" — change DNS TTL to 60s, point domain to backup IP on failure | Windows DNS client caches aggressively and ignores TTL on LAN hostnames. rc-agent resolves `core.url` once at connect time then uses the cached connection. DNS failover requires the client to re-resolve, which only happens after connection timeout + new connection. Total failover time: connection timeout (30s) + DNS propagation (60s TTL) + reconnect backoff. ~90–120 seconds minimum. Unacceptable for active sessions. | Direct IP failover: rc-agent holds both primary IP and fallback IP in config. On WS connection failure past threshold, switch to fallback IP immediately — no DNS lookup, no propagation delay. Failover in < 15s. |
| **Active-active dual-server (pods connected to both simultaneously)** | "No downtime, load shared between local and cloud" | Active-active means every session write (laps, billing events) must be committed to both servers with distributed consensus. The existing cloud_sync already has ID mismatch problems (local/cloud use different UUIDs, resolved by phone/email). True active-active would require a distributed database — fundamentally at odds with the SQLite-first architecture. | Active-passive: .23 is always primary, cloud is always standby. Failover is an exception state, not normal operation. Keep SQLite local. |
| **Heartbeat piggybacked on existing WebSocket** | "Reuse the WS keepalive for server health" | The WS keepalive (CONN-01: ping/pong) only proves the connection is alive — it does not prove the server is processing requests correctly. A deadlocked server will still respond to WS pings because the OS handles TCP keepalive below the application layer. | Use a separate HTTP health check to `/health` that exercises the application stack (DB connection, state access) — not just TCP connectivity. |
| **Watchdog-based failover (scheduled task on pods)** | "No need to change rc-agent — Windows scheduled task polls the server and kills/restarts rc-agent if it can't reach the server" | A scheduled task restarts the entire rc-agent process, destroying all in-flight state (current session, billing state, lap being recorded). The right behavior is to switch the WS URL and reconnect — not destroy and restart the process. | Implement URL-switch logic inside rc-agent's existing reconnect loop. The connection loop already knows when WS is dead; add the URL-switch step there. Process stays running, only the connection target changes. |
| **Config sync via filesystem sync (rsync/Syncthing)** | "Sync the entire C:\RacingPoint\ directory to Bono" | The config directory contains local state (SQLite WAL files, session state, local-authoritative billing data) that should NOT be overwritten by sync in either direction. Filesystem-level sync does not know which files are config vs state. | Push only `racecontrol.toml` via API. A dedicated `POST /admin/sync-config` endpoint on Bono's VPS receives the TOML content, validates it, and applies it. No filesystem sync tooling needed. |
| **Continuous config sync (push on every 30s loop)** | "Piggyback on cloud_sync loop for config" | racecontrol.toml changes are rare (weekly or less). Pushing the entire config every 30 seconds is noisy and creates a risk of pushing a corrupted/mid-write config. | Event-driven: push on (1) racecontrol startup, and (2) when the file's modification time changes. No polling loop. File mtime is cheap to check; push is infrequent and intentional. |

---

## Feature Dependencies

```
[Stable server IP — MAC reservation]
    required-by --> [Server health monitor from James]
    required-by --> [Tailscale SSH to server]
    required-by --> [All failover/failback logic]
    note: If IP drifts, health monitor watches wrong address

[Server health monitor (James .27)]
    required-by --> [Failover trigger]
    required-by --> [Failover notification to Uday]
    uses --> [HTTP GET /health on :8080 (existing endpoint)]
    uses --> [Hysteresis state machine (pattern from cloud_sync.rs)]

[Pods accept fallback_url in config]
    required-by --> [Auto-failover (pods switch URLs)]
    required-by --> [Failback (pods switch back)]
    requires --> [rc-agent TOML extension (CoreConfig.fallback_url)]
    requires --> [URL-switch logic in WS reconnect loop]

[Config sync: racecontrol.toml to Bono]
    required-by --> [Bono VPS useful as failover target]
    required-by --> [Pod IP whitelist on cloud]
    required-by --> [Failover-aware Bono config]
    uses --> [Bono's existing webhook/relay infrastructure]
    NOT: cloud_sync.rs rows sync (different purpose)

[Bono VPS health check]
    required-by --> [Failover trigger (must verify target is healthy)]
    uses --> [https://app.racingpoint.cloud/health (existing endpoint)]

[Tailscale SSH to server]
    requires --> [Phase 27 Tailscale deploy on server (status: partially deployed)]
    enables --> [James remote restart of racecontrol on .23]
    enables --> [Server-initiated failback signal]

[Failover trigger]
    requires --> [Server health monitor DOWN state]
    requires --> [Bono VPS health check PASS]
    requires --> [Pods have fallback_url configured]
    triggers --> [Failover notification to Uday]
    triggers --> [Pods switch to cloud URL]

[Split-brain prevention]
    requires --> [Tailscale SSH (for server-initiated failback)]
    requires --> [Bono read-only mode on primary recovery]
    prevents --> [Dual session recording]

[Grace period for active sessions]
    requires --> [Pods have fallback_url configured]
    requires --> [Session state snapshot mechanism]
    prevents --> [Lost laps during mid-session failover]
```

### Dependency Notes

- **DHCP reservation is the blocker for everything.** If .23 drifts again mid-milestone, health monitor fires false positives and failover logic breaks. Do this first, before writing any failover code.
- **`core.fallback_url` in rc-agent requires a binary re-deploy to all 8 pods.** This is the highest-effort deployment step. Build and validate config change on Pod 8 canary first.
- **Config sync to Bono is independent of failover mechanics.** It can be built and tested without triggering actual failover. Build it first — it validates the James↔Bono communication channel before relying on it under stress.
- **Split-brain prevention requires Tailscale SSH to be working on the server.** If Phase 27 server deployment is incomplete, implement a simpler prevention: Bono auto-enters read-only mode on a 60s timer after primary reconnects (not server-initiated). Less clean but doesn't require Tailscale SSH.
- **Grace period for active sessions is a differentiator, not table stakes.** MVP can failover without it; data loss risk is acceptable for short outages. Add grace period in phase 2 once basic failover is validated.

---

## MVP Definition

v10.0 is complete when a server .23 crash does not cause permanent venue downtime and does not require James or Uday to manually intervene to restore pod connectivity.

### Launch With (v10.0 MVP — Phase 1)

The minimum that achieves the stated goal.

- [ ] **DHCP reservation for .23 (MAC 10-FF-E0-80-B1-A7)** — Foundation. Stops IP drift permanently. 30-minute task in router UI.
- [ ] **Server health monitor on James's machine** — HTTP poll `/health` every 5s, DOWN after 3 consecutive failures, UP after 2 consecutive successes. Logs state transitions. No failover action yet — just detection.
- [ ] **Config sync: racecontrol.toml → Bono on startup and on change** — Ensures Bono's VPS mirrors venue config before any failover is needed. Validates James→Bono communication.
- [ ] **`core.fallback_url` in rc-agent TOML + URL-switch logic** — rc-agent can switch WS target. Deploy to Pod 8 first, then all pods after validation.
- [ ] **Failover trigger + notification** — When health monitor declares .23 DOWN and Bono VPS is healthy, signal all pods to switch to fallback_url. Email Uday.
- [ ] **Failback trigger + notification** — When .23 recovers (2 consecutive successes), signal pods back to primary URL. Email Uday.

### Add After Validation (v10.x — Phase 2)

Once basic failover/failback is confirmed working in a test scenario:

- [ ] **Tailscale SSH to server** — Remote exec channel for James. Prerequisite: Phase 27 server Tailscale confirmed. Then `tailscale up --ssh` on server.
- [ ] **Split-brain prevention** — Bono enters read-only mode when server .23 sends "I am primary" signal via Tailscale. Requires Tailscale SSH.
- [ ] **Grace period for active sessions** — Snapshot lap/session state before URL switch. Prevents lost laps during mid-session failover.
- [ ] **WhatsApp failover alerts for Uday** — Mirror email alerts to WhatsApp via existing bot. Add after email alerts are confirmed reliable.

### Future Consideration (v10.x+)

- [ ] **Failover health dashboard** — Admin panel widget showing: current primary (local/cloud), last failover timestamp, failover count in last 30 days. Low urgency — email notifications cover operational need.
- [ ] **Automatic recovery test** — Weekly scheduled drill: briefly simulate server unreachable and verify failover triggers within SLA. Requires confidence that drill won't affect active customer sessions.
- [ ] **Multi-level failover** — If both .23 and Bono are unreachable, pods enter local-only "island mode" (accept walk-in sessions without billing, store locally, sync when connectivity returns). Significant complexity; relevant if internet reliability is poor.

---

## Feature Prioritization Matrix

| Feature | Operator Value | Implementation Cost | Priority |
|---------|----------------|---------------------|----------|
| DHCP reservation for .23 | HIGH — prevents all IP-drift failures | LOW — router UI change | P1 |
| Server health monitor (James) | HIGH — enables all failover logic | LOW — 50 lines of Rust | P1 |
| Config sync racecontrol.toml → Bono | HIGH — makes cloud failover useful | MEDIUM — new API endpoint on Bono | P1 |
| `core.fallback_url` + URL-switch in rc-agent | HIGH — core mechanic of failover | MEDIUM — TOML + reconnect loop change + pod re-deploy | P1 |
| Failover trigger + Uday notification | HIGH — automates the critical response | LOW — builds on health monitor | P1 |
| Failback + Uday notification | HIGH — returns to normal without manual intervention | LOW — mirrors failover logic | P1 |
| Tailscale SSH to server | MEDIUM — remote restart capability | MEDIUM — depends on Phase 27 completion | P2 |
| Split-brain prevention | MEDIUM — prevents data conflicts during failback | MEDIUM — requires Tailscale SSH | P2 |
| Grace period for active sessions | MEDIUM — prevents mid-session lap loss | MEDIUM — session snapshot logic | P2 |
| WhatsApp failover alerts | MEDIUM — faster Uday notification | LOW — route through existing bot | P2 |
| Failover health dashboard | LOW — email covers operational need | MEDIUM — new admin UI component | P3 |
| Automatic recovery test drill | LOW — nice-to-have validation | HIGH — scheduling + session safety | P3 |
| Island mode (no connectivity) | LOW — rare scenario | HIGH — distributed state management | Avoid for now |

**Priority key:** P1 = v10.0 MVP (ship together), P2 = Phase 2 post-validation, P3 = future consideration

---

## Health Check Interval and Threshold Recommendations

Based on production patterns (AWS ELB, HAProxy, Tailscale HA) and the specific constraints of this system:

| Parameter | Recommended Value | Rationale |
|-----------|------------------|-----------|
| Health check interval | 5 seconds | Fast enough to detect outages within 15-30s. Slower than Tailscale's 15s HA threshold. Low overhead (1 HTTP GET every 5s from James's machine). |
| DOWN threshold | 3 consecutive failures | 3 × 5s = 15s before declaring DOWN. Matches Tailscale's ~15s failover timing. Prevents single-probe false positive. |
| UP threshold (recovery) | 2 consecutive successes | 2 × 5s = 10s to confirm recovery. Fast enough for failback without flip-flopping. Same pattern as cloud_sync.rs `RELAY_UP_THRESHOLD`. |
| HTTP probe timeout | 2 seconds | Distinguishes "server unreachable" from "server slow." Slow responses (>2s) count as failures. |
| Failover notification rate limit | 1 alert per 5 minutes | Prevents alert flood if server oscillates. Same pattern as existing email_alerts.rs rate limiting. |
| Config sync frequency | On startup + on mtime change | Event-driven, not polling. racecontrol.toml changes at most weekly. |
| Failback delay after recovery | 10 seconds (2 successes × 5s interval) | Short enough to restore LAN performance. Long enough to confirm recovery is stable. |

---

## Sources

- [Tailscale High Availability Docs](https://tailscale.com/kb/1115/high-availability) — HIGH confidence (official docs). Failover timing ~15s matches our DOWN threshold design.
- [Tailscale HA Subnet Router Troubleshooting](https://tailscale.com/docs/reference/troubleshooting/network-configuration/overlapping-subnet-route-failover) — HIGH confidence (official docs)
- [AWS ELB Health Check Concepts](https://docs.aws.amazon.com/elasticloadbalancing/latest/network/target-group-health-checks.html) — HIGH confidence (official AWS docs). Source for interval/threshold patterns.
- [HAProxy Health Check Tutorial](https://www.haproxy.com/documentation/haproxy-configuration-tutorials/reliability/health-checks/) — HIGH confidence (official HAProxy docs). Source for 3-5s intervals with 2-3 failure thresholds.
- [Resilient Microservices: Recovery Patterns (arxiv 2025)](https://arxiv.org/html/2512.16959v1) — MEDIUM confidence (academic, 2025). Confirms jitter + backoff + circuit breaker as standard.
- [DNS Failover Limitations (IBM)](https://www.ibm.com/think/topics/dns-failover) — HIGH confidence (official IBM). Confirms Windows DNS caching makes DNS-based failover unsuitable for LAN.
- [Split-Brain Prevention (StarWind)](https://www.starwindsoftware.com/blog/whats-split-brain-and-how-to-avoid-it/) — MEDIUM confidence (vendor blog, well-sourced). Heartbeat-based split-brain detection.
- cloud_sync.rs codebase — HIGH confidence (primary source). `RELAY_DOWN_THRESHOLD=3`, `RELAY_UP_THRESHOLD=2` pattern directly reused.
- rc-agent main.rs codebase — HIGH confidence (primary source). `reconnect_delay_for_attempt()` backoff, `CoreConfig.url`, WS reconnect loop structure.
- PROJECT.md v10.0 requirements — HIGH confidence (primary source).

---
*Feature research for: Connectivity & Redundancy (v10.0) — Racing Point eSports venue operations*
*Researched: 2026-03-20*
