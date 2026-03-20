# Pitfalls Research

**Domain:** Adding Claude Code skills, MCP servers, deployment automation, and monitoring to a Windows fleet venue management system (Racing Point v9.0)
**Researched:** 2026-03-20
**Confidence:** HIGH — pitfalls derived from documented past failures (Salt v6.0 blocked, Gmail OAuth expired, WinRM/OpenSSH failures, over-engineering scraps), codebase analysis (CONCERNS.md), and environment constraints (Windows 11 fleet, LAN-only, single operator).

---

## Critical Pitfalls

### Pitfall 1: Assuming Fleet Tooling Will Work on This Network (Salt/WinRM/Ansible History)

**What goes wrong:**
A fleet automation tool that works perfectly in a corporate or cloud environment is deployed to Racing Point's Windows 11 LAN. Installation appears to succeed. Then: the tool cannot reach pods because Windows Defender blocks the agent installer, the ZMQ ports (Salt 4505/4506) are LISTENING but no minion connects, WinRM returns 401/5 authentication failures, or OpenSSH's Windows component store is corrupted and `Add-WindowsCapability` silently fails. The entire deployment week is wasted. This happened three times: Salt (WSL2 portproxy couldn't forward ZMQ), WinRM (failed on this network previously), OpenSSH (Server component store corrupted on the server).

**Why it happens:**
This network has compounding constraints that break fleet tools: (1) No domain controller — pods use local Windows accounts with no Kerberos. (2) Windows Defender on pods is active and blocks new service installers without admin approval. (3) rc-agent is in Session 1 (GUI session) because of the lock screen requirement, but most remote management daemons run as SYSTEM in Session 0 — creating the Session 0/1 split. (4) Firewall rules require admin to open ports, and batch file CRLF bugs silently break netsh rules. (5) WSL2 portproxy has documented limitations with ZMQ multicast.

**How to avoid:**
Before committing to any new fleet tool for v9.0, answer five questions: (a) Does it work on non-domain Windows machines? (b) Does its agent installer pass Windows Defender without manual exclusion? (c) Does it use standard HTTP/HTTPS ports (80/443/8080) rather than custom ports? (d) Does it tolerate a 5-minute internet outage mid-operation? (e) Is there a working reference deployment on Windows 11 21H2+ without a domain controller? If any answer is no, it will fail. The existing rc-agent remote_ops on port 8090 (WebSocket exec) is already deployed and working — prefer building on that foundation over adopting a new fleet daemon.

**Warning signs:**
- Tool documentation says "supports Windows" but examples show AD/domain setups
- Agent requires `Add-WindowsCapability` or `DISM` to install
- Tool uses ZMQ, SSH, or WinRM as transport (all failed before)
- First pod test works but identical steps fail on second pod (inconsistent Defender state)

**Phase to address:** Phase 1 (Deployment Automation) — verify the five questions before any implementation begins. Add a "Prior Failures" section to the phase plan citing Salt, WinRM, OpenSSH.

---

### Pitfall 2: OAuth Token Expiry Silently Breaking Automation

**What goes wrong:**
An automation workflow is built on Gmail MCP or Google Workspace API for alerting, status emails, or log delivery. The integration works on day 1. Six weeks later, the refresh token silently expires (Google OAuth tokens for non-production apps have 7-day expiry in test mode; production apps can be 6 months). The automation sends no alerts. Uday doesn't notice because the venue is running fine. Three weeks after that, a pod crashes at 2 AM — no alert fires, Uday sees the issue at 9 AM when customers arrive. The Gmail MCP is documented as broken in MEMORY.md as of 2026-03-20: "Gmail OAuth tokens expired — `getAuthClient` fails with 'No access, refresh token'."

**Why it happens:**
Google OAuth refresh tokens are not permanent. For apps in "testing" mode (not published to Google Workspace Marketplace), tokens expire after 7 days and must be re-authorized. Even published apps expire after 6 months of inactivity. Claude Code Gmail MCP uses stored credentials — when the token expires, all MCP tool calls silently fail with an auth error, but the MCP server keeps running and reporting no errors at the process level.

**How to avoid:**
Never build a critical alerting path on OAuth-dependent integrations without a fallback. The existing `send_email.js` shell-out with stored credentials is the current working email method — keep it as the primary alert path. For any new MCP-based integration, add a daily health check that attempts a low-stakes API call (e.g., list one calendar event) and alerts via the fallback channel if it fails. For Google Workspace: publish the app to production mode (removes 7-day expiry), or use a service account with domain-wide delegation (no token expiry). Set a calendar reminder to re-authorize tokens before expiry.

**Warning signs:**
- MCP server process is running but API calls return `getAuthClient` or "No access, refresh token" errors
- Alert emails stop arriving but venue operations appear normal (silent failure)
- Last successful alert email was more than 7 days ago
- `node send_email.js` returns 200 but Gmail MCP returns auth error

**Phase to address:** Phase 2 (MCP Servers) — every MCP integration must have a token health check and fallback path before being marked complete.

---

### Pitfall 3: Internet Dependency for Venue-Critical Operations

**What goes wrong:**
A new monitoring or automation feature is built assuming stable internet: Prometheus metrics pushed to a cloud endpoint, alert webhooks to Slack/PagerDuty, Claude API calls for AI-assisted debugging, cloud-sync-dependent workflows for billing decisions. The venue's internet connection drops (it does — "flaky internet" is a documented constraint). During the outage: all monitoring is blind, alerts cannot fire, billing workflows stall because cloud_sync fails, and AI debugging fallback to cloud Claude API fails. Uday is left managing 8 pods with no tooling for 20-30 minutes.

**Why it happens:**
The impulse when adding monitoring is to send metrics/events to a cloud service (Grafana Cloud, Datadog, etc.) because they provide dashboards without self-hosting. But Racing Point's operations are venue-local — the 8 pods, the server, and James workstation are all on 192.168.31.x. None of them need internet to function. Building critical tooling on internet-dependent paths turns a network blip into an operational outage.

**How to avoid:**
Follow the existing cloud_sync pattern: local operations are authoritative and run first; cloud is a secondary sync that gracefully degrades. For monitoring: run Prometheus + Grafana locally on the server (.23) and export metrics over LAN. For alerting: send alerts via WhatsApp (Evolution API on localhost:53622 — LAN-only, works offline) first, email second. For AI debugging: Ollama on James workstation (.27) is already the primary path, Claude API is the fallback. New tooling must follow this same offline-first hierarchy: LAN-local → offline-capable → cloud-dependent.

**Warning signs:**
- Feature works perfectly during development but fails during demo when 4G hotspot is used
- Monitoring dashboard shows gaps in metrics during known internet outage periods
- Alert fires 20 minutes late (cloud webhook queued and delivered after reconnect)
- Billing session hangs because it awaits a cloud_sync confirmation

**Phase to address:** Phase 1 (Deployment Automation) and Phase 4 (Monitoring) — architecture review must verify offline behavior before implementation begins.

---

### Pitfall 4: Over-Engineering the Fleet Management Layer

**What goes wrong:**
v6.0 is the canonical example: Salt fleet management (SaltStack master on WSL2, salt-minions on 8 pods + server) was designed, researched, and planned before any pod was tested. The planned architecture required WSL2 AMD-V virtualization on the server. The server's BIOS doesn't support AMD-V for WSL2. v6.0 is now "BLOCKED" and has been parked. Six weeks of architectural planning delivered nothing operational. The existing rc-agent remote_ops port 8090 was already deployed and working.

**Why it happens:**
Fleet management is a solved problem at scale (1000+ servers). It's also massively over-engineered for a fleet of 8 Windows PCs that rarely change. The temptation is to adopt "production-grade" infrastructure tooling because it exists and is well-documented. But Racing Point's fleet is not a cloud server farm — pods rarely need batch commands, binary deploys happen once a month, and the primary channel (rc-agent WS exec) already works for 90% of cases.

**How to avoid:**
Before evaluating any new fleet tool for v9.0, ask: "What does the existing rc-agent remote_ops + pendrive workflow not cover?" The answer is small: (a) deploying a new rc-agent binary when the old one is crashed, (b) bulk config changes across all 8 pods simultaneously. Address those two gaps with the simplest possible mechanism (a one-click deploy HTTP server on James workstation + rc-agent's self-restart sentinel) rather than introducing a new fleet daemon. New tooling must solve a documented operational problem, not a hypothetical scale problem.

**Warning signs:**
- Tool evaluation takes more than one day before any pod is tested
- Architecture requires a component (WSL2 AMD-V, domain controller, SSH server) that hasn't been verified on the target hardware
- The tool solves a problem that currently requires ~5 minutes of manual work (not worth the automation overhead)
- The plan has more than 3 deployment phases before producing any operational value

**Phase to address:** Phase 1 (Deployment Automation) — scope must be bounded to the documented operational gaps, not a general fleet management overhaul.

---

### Pitfall 5: Claude Code Skills That Require Human Confirmation in the Critical Path

**What goes wrong:**
A Claude Code skill or automation hook is added to handle pod crashes or billing issues. The skill works correctly in testing. In production, the skill hits an edge case (e.g., billing session stuck in a state it doesn't recognize) and prompts Uday for confirmation: "Should I force-end this session? (y/n)". Uday is with a customer at the reception desk. He doesn't see the prompt for 15 minutes. The pod is locked and the customer is waiting. The automation that was supposed to reduce manual intervention has created a worse experience than no automation at all.

**Why it happens:**
Claude Code and LLM-based automation have a natural tendency to ask for confirmation before destructive or irreversible actions. This is appropriate for development tasks but wrong for operational automation where speed and autonomy are the value proposition. Racing Point has one operator (Uday) who cannot monitor a terminal. If automation can't make a decision, Uday must, and he may not be available.

**How to avoid:**
Every automation action must have a pre-determined policy for the uncertain case: "If I cannot determine the correct action, do X by default." For pod automation: the safest default is always "engage lock screen and alert Uday via WhatsApp." Never leave the system in a state requiring human input before proceeding. For Claude Code skills: define the decision boundary explicitly in the skill's prompt — what it is authorized to do without asking, and what should trigger a WhatsApp alert to Uday instead. The existing 4-tier debug order (deterministic → memory → Ollama → Claude API) is the correct pattern: escalate, don't block.

**Warning signs:**
- Skill prompt includes "ask the user whether to..." for any operational action
- Testing shows the skill works for the 3 common cases but "will ask" for edge cases
- The skill's error handling is "return the error and wait for instructions"
- Uday has to check a terminal to see if automation completed

**Phase to address:** Phase 3 (Claude Code Skills) — every skill must define its fallback policy (alert + safe-state) before implementation.

---

### Pitfall 6: Windows Session 0 vs Session 1 Breaking New Service Deployments

**What goes wrong:**
A new monitoring agent, MCP server, or automation service is installed as a Windows Service (SYSTEM account, Session 0). The service starts successfully. It cannot display UI elements, cannot interact with the kiosk or lock screen (which run in Session 1), cannot read GPU metrics from the user session, and cannot interact with game processes (which have user-session security context). On the server, services that need to talk to the kiosk frontend over localhost see connection refused because the kiosk is listening on a user-session socket. This exact problem caused the watchdog to restart rc-agent in Session 0 on pod crashes — it shows as blank screen until next reboot.

**Why it happens:**
Windows Vista+ separates services (Session 0) from interactive user processes (Session 1+). Any service running as SYSTEM cannot directly interact with the desktop session. For Racing Point: the kiosk, rc-agent, and the lock screen all require Session 1 because they are GUI applications or need to interact with game processes. New services that need to bridge both sessions require explicit `CreateProcessAsUser` or `WTSQueryUserToken` calls, which are complex and error-prone.

**How to avoid:**
For new agents or services that need to interact with the RC system: prefer HTTP endpoints over Windows Service architecture. The existing pattern (HKLM Run key → starts in Session 1 at login, `start-rcagent.bat`) is the proven approach for processes that need GUI access. For background monitoring that doesn't need GUI (Prometheus node exporter, log shipper), install as a SYSTEM service but scope it to HTTP-only operations against LAN endpoints. Never install a new Windows Service as SYSTEM without explicitly testing that it can access everything it needs from Session 0.

**Warning signs:**
- Service reports "started" but produces no output / collects no metrics
- Log files show "Access denied" or "Handle is invalid" for operations that work fine when run as the logged-in user
- Service can HTTP-GET a LAN endpoint but cannot write to a local file in the user profile
- New service needs to interact with the lock screen HTML, kiosk browser, or game process — these require Session 1

**Phase to address:** Phase 1 (Deployment Automation) and Phase 4 (Monitoring) — session context must be specified for every new process before deployment design.

---

### Pitfall 7: MCP Server Config Drift Breaking James-Bono Coordination

**What goes wrong:**
A new MCP server is configured in Claude Code on James workstation (.27) — for example, a Google Workspace MCP or a custom venue MCP that reads racecontrol SQLite. The MCP works from James's Claude Code session. A month later, a new MCP is added. The `claude_desktop_config.json` (or equivalent MCP config) is updated on James's machine but never synced to Bono's VPS environment. Bono attempts a task that should use the Google Drive MCP and falls back to a worse path because the MCP is not available in his environment. James assumes Bono has the same tools; Bono assumes James has the same tools. Neither verifies, and both produce incorrect automation that silently fails.

**Why it happens:**
Claude Code MCP configuration is per-machine. James (.27, Windows) and Bono (VPS, Linux) have different environments and different MCP availability. When new MCPs are added to one environment, there is no automatic sync to the other. The comms-link INBOX.md is used for operational messages but not for infrastructure changes. Over time, the tool sets diverge.

**How to avoid:**
Maintain a `.planning/codebase/MCP-INVENTORY.md` file in the racecontrol repo that lists every MCP server, which environment it runs in (James/Bono/both), its purpose, its config location, and its last verified date. When adding a new MCP, update this file and notify the partner AI via INBOX.md with the config diff. For any task that involves both James and Bono, check MCP-INVENTORY.md first to confirm tool availability on both sides.

**Warning signs:**
- Bono completes a task using a different (worse) path that James knows has an MCP shortcut
- James references a tool capability that Bono's session doesn't have
- `claude mcp list` output differs between James and Bono environments
- New MCP was added without a corresponding INBOX.md message to the partner AI

**Phase to address:** Phase 2 (MCP Servers) — MCP-INVENTORY.md must be created and populated as the first deliverable, before any new MCP is installed.

---

### Pitfall 8: Monitoring That Alerts Too Much (Alert Fatigue) or Too Little (Silent Failures)

**What goes wrong:**
A Prometheus + Grafana monitoring stack is set up. Alerts are configured for pod heartbeat timeout (6s), WebSocket disconnect, billing session anomaly, CPU over 80%. Within a week, Uday receives 40+ WhatsApp alerts per day: pods disconnect momentarily during game startup (normal), CPU spikes during AC loading (normal), heartbeat gaps during Windows update reboots (normal). Uday starts ignoring all alerts. Three weeks later, a genuine billing failure produces an alert that goes unnoticed because the notification channel is muted.

**Why it happens:**
The existing system already has hardcoded thresholds tuned for the Racing Point environment (6s heartbeat, 10s idle, 5 CLOSE_WAIT strikes before restart). These thresholds were calibrated through 50+ phases of production operation. A new monitoring layer that doesn't respect these calibrations will generate false positives from normal events. The rate-limited alert system (ALERT-02) exists specifically because unchecked alerting caused this problem before.

**How to avoid:**
Before configuring any alert, document the normal behavior baseline: What is the expected heartbeat gap during AC launch? (up to 30s.) What is normal CPU during F1 25 session? (60-80%.) What is acceptable WebSocket disconnect frequency? (1-2 per day.) Alert only when observed values exceed the documented normal range for more than the calibrated duration. Start with zero alerts and add them one at a time after observing the baseline for one week. Mirror the existing ALERT-02 rate-limiting: maximum one alert per type per 30 minutes.

**Warning signs:**
- More than 10 alerts fire in the first 24 hours after monitoring is enabled
- Alerts fire during events that are known-normal (game startup, pod reboot, steam update)
- Uday mutes the alert channel or starts not responding to alerts
- Alert thresholds were copied from a generic template rather than calibrated against Racing Point's logs

**Phase to address:** Phase 4 (Monitoring) — baseline calibration week must precede any alert threshold configuration.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using Gmail MCP as the sole alert channel | Simple to set up | Silent failure when OAuth expires; no fallback for critical pod alerts | Never for critical alerts — always add WhatsApp (Evolution API) as primary |
| Installing new agents as Windows Services without Session 1 testing | Clean install, auto-start | Agent runs but can't access GUI or game processes; breaks lock screen integration | Only if agent is purely network/HTTP with no UI interaction |
| Adding Claude Code skills that prompt for confirmation | Safer in development | Blocks operations when Uday is unavailable; worse than no automation | Only for non-time-sensitive admin tasks (never for pod crash recovery) |
| Deploying fleet tool to all 8 pods simultaneously | Faster deployment | One failure blocks all pods; no canary | Never — always deploy Pod 8 canary first, verify, then roll out |
| Hardcoding alert webhook URLs in automation scripts | Simple | URL changes break all alerts silently; no single source of truth | Never — put webhook URLs in racecontrol.toml config |
| Building monitoring on cloud endpoints | Avoids self-hosting | Monitoring blind during internet outage; exactly when monitoring matters most | Only for non-critical dashboards that don't affect operations |
| Storing MCP credentials alongside the racecontrol repo | Convenient | Credentials committed to git history; API keys exposed | Never — always use environment variables or separate secrets file outside repo |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Google Workspace MCP | Assume OAuth tokens are permanent | Tokens expire; add daily health-check API call + fallback to send_email.js |
| Prometheus node exporter on Windows | Install as SYSTEM service and assume it reads everything | Verify Session 0 can access all target metrics; GPU/game metrics need Session 1 context |
| Ansible/fleet tool on Windows pods | Assume WinRM is available | WinRM has failed before; use rc-agent HTTP port 8090 as the transport layer |
| Claude Code custom skills | Write skills that call external APIs directly | Route all external calls through racecontrol API or existing rc-agent endpoints to respect config and auth |
| SQLite monitoring queries | Run monitoring queries against live racecontrol.db | SQLite WAL mode allows concurrent reads but heavy monitoring queries add latency to billing operations; use a read replica or periodic export |
| MCP for racecontrol SQLite | Allow MCP to write to the database | MCP access must be read-only; all writes go through racecontrol API to maintain billing integrity |
| Grafana on the server (.23) | Install via Linux package manager | Server runs Windows; use the Windows binary or the Docker-based version (if Docker is available); verify RAM usage against 64GB budget |
| WhatsApp via Evolution API | Assume the API instance stays connected | Evolution API WhatsApp instance requires periodic QR re-scan; add a health check endpoint probe |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Monitoring queries on racecontrol.db during active sessions | Billing latency increases; `compute_session_cost()` takes 200ms+ | Use SQLite WAL mode (already configured?) and limit monitoring to off-peak reads | During peak hours (6-10 PM) with 8 active sessions |
| Prometheus scraping every pod every 15s via HTTP | Network traffic doubles; rc-agent HTTP handler slows | Scrape interval 60s minimum; use rc-agent WebSocket push events rather than HTTP polling | Immediately — rc-agent HTTP handler is not designed for high-frequency scrapes |
| Claude API calls from automation triggered by pod events | API latency (500ms-3s) blocks event processing loop | Rate-limit Claude calls; use Ollama on .27 for real-time decisions, Claude API only for async analysis | During any pod flap (rapid connect/disconnect) — flood of events triggers flood of API calls |
| Full log shipping from 8 pods + server | Disk I/O on server hits 100%; log files consume all free space | Ship only ERROR/WARN logs; rotate daily; cap at 100MB per pod | Within 2 weeks of enabling DEBUG-level log shipping |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| MCP server with read/write access to racecontrol.db | Billing manipulation, driver wallet modification via Claude | MCP access must be read-only SQLite or go through racecontrol API with auth token |
| Storing API keys in Claude Code skill prompts | Keys exposed in conversation history, logs, and any context window that includes the skill | Use environment variables or racecontrol.toml config; never inline keys in prompts |
| Enabling remote exec via new automation tool without auth | Any process on the LAN can run arbitrary commands on pods | Require rc-agent token auth for all exec endpoints; never open unauthenticated exec on any port |
| Hardcoded JWT default secret remaining unfixed (existing P0) | Token forgery, unauthorized billing session creation | Before adding any new auth integration, fix `default_jwt_secret()` in config.rs — this is a prerequisite |
| New MCP server running with James's Windows credentials | MCP process has full filesystem access including `C:\RacingPoint\racecontrol.toml` (contains API keys) | Run MCP servers as a limited user account; scope filesystem access to minimum needed paths |

---

## "Looks Done But Isn't" Checklist

- [ ] **Gmail MCP working:** Verify by sending a test email *and* checking token expiry date — a successful send today does not mean the token is valid in 7 days
- [ ] **Fleet automation deployed:** Verify by deploying a real binary update to Pod 8 and confirming the pod restores to Running state within 2 minutes — not just that the deploy script exits 0
- [ ] **Monitoring alerting:** Verify by killing rc-agent on Pod 8 and confirming a WhatsApp alert arrives within 2 minutes — not just that Grafana shows a red panel
- [ ] **Claude Code skill works offline:** Verify by disabling internet on James workstation and running the skill — if it fails, add Ollama fallback before marking complete
- [ ] **MCP credential rotation tested:** Simulate token expiry by revoking the OAuth token and confirm the fallback path (send_email.js) fires before marking the MCP integration complete
- [ ] **New service session context verified:** Run the service, then check with `Get-Process -IncludeUserName` that it's running in Session 1 (if GUI needed) or confirm it doesn't need Session 1 access (if background only)
- [ ] **Alert thresholds calibrated:** Confirm zero false-positive alerts fired during one full business day before enabling production alerting

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Fleet tool failure (blocked by Defender/WinRM/etc.) | HIGH | Abandon the tool; fall back to pendrive install.bat v5 + rc-agent remote_ops :8090 for the specific gap being addressed |
| Gmail OAuth token expired | LOW | Re-authorize via `node send_email.js --reauth`; update token in credentials file; set 30-day calendar reminder for next re-auth |
| Cloud-dependent automation fails during internet outage | MEDIUM | Add offline-first fallback path; switch alert delivery to WhatsApp (Evolution API, LAN-local); document the outage handling policy |
| Alert fatigue (too many alerts) | LOW | Disable alert channel; audit all alert rules; raise thresholds or add minimum-duration filters; re-enable one alert at a time |
| MCP credential leak | HIGH | Rotate all API keys immediately; audit racecontrol.toml for any keys that appeared in MCP context; update `rc-agent.toml` on all pods via pendrive |
| Over-engineered solution (like v6.0) | HIGH | Stop. Document why it failed in PROJECT.md. Identify the minimal working alternative. Ship that instead. |
| New Windows Service stuck in Session 0 | MEDIUM | Convert to HKLM Run key (`start-service.bat`) to launch in Session 1 at login; accept the "no crash restart" limitation as the cost of Session 1 |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Fleet tool blocked by Defender/WinRM/Session constraints | Phase 1: Deployment Automation | Five-question checklist answered before any tool evaluation; no new fleet daemon unless it passes all five |
| OAuth token expiry breaks automation | Phase 2: MCP Servers | Token health check fires before all critical workflows; fallback to send_email.js tested by simulating expiry |
| Internet dependency for venue-critical ops | Phase 1 + Phase 4 | Offline test: disable internet on .27, verify all automation still functions on LAN |
| Over-engineering (v6.0 redux) | Phase 1: Deployment Automation | Phase 1 scope bounded to documented gaps only; any new fleet daemon proposal requires prior failure review |
| Automation requiring Uday confirmation | Phase 3: Claude Code Skills | Every skill has a defined fallback policy; zero skills with blocking prompts in operational paths |
| Session 0 vs Session 1 breakage | Phase 1 + Phase 4 | `Get-Process -IncludeUserName` check for every new process; Session 1 requirement documented before deployment design |
| MCP config drift (James vs Bono) | Phase 2: MCP Servers | MCP-INVENTORY.md created as first deliverable; Bono notified via INBOX.md on every config change |
| Alert fatigue | Phase 4: Monitoring | Baseline calibration documented before any alert is enabled; max 1 alert per type per 30 minutes (mirror ALERT-02) |

---

## Sources

- MEMORY.md — Documents Salt v6.0 blocked (BIOS AMD-V), Gmail OAuth expired, WinRM failure history, OpenSSH component store corruption, Session 0/1 split problem
- PROJECT.md — v6.0 "Paused Milestone" entry; remote deploy scrapped approaches list (Salt, OpenSSH, WinRM)
- CONCERNS.md — Hardcoded JWT secret (P0 #1), cloud sync fragility, error silencing patterns, race conditions in pod state
- INTEGRATIONS.md — Gmail/Google Workspace dependency, Ollama LAN-only constraint, Evolution API localhost:53622 (LAN-local), cloud_sync 30s poll pattern
- codebase STACK.md — Static CRT build, HKLM Run key for Session 1, existing rc-agent remote_ops :8090
- Real operational history: v6.0 blocked March 2026, Gmail MCP broken March 2026, pendrive as fallback after all remote deploy approaches failed

---
*Pitfalls research for: Tooling, automation, MCP servers, and monitoring additions to Racing Point Windows fleet venue system (v9.0)*
*Researched: 2026-03-20*
