# Requirements: Racing Point Operations — v9.0 Tooling & Automation

**Defined:** 2026-03-20
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v9.0 Requirements

Requirements for tooling & automation milestone. Each maps to roadmap phases.

### Claude Code Skills

- [x] **SKILL-01**: James's Claude Code sessions auto-load Racing Point project context (pod IPs, crate names, naming conventions, constraints) from a project-level CLAUDE.md
- [ ] **SKILL-02**: James can invoke `/rp:deploy` to build rc-agent and stage the binary for pod deployment, with `disable-model-invocation: true`
- [ ] **SKILL-03**: James can invoke `/rp:deploy-server` to build racecontrol, stop the old process, swap the binary, and verify :8080 comes back
- [ ] **SKILL-04**: James can invoke `/rp:pod-status <pod>` to query any pod's rc-agent status via dynamic IP injection
- [ ] **SKILL-05**: James can invoke `/rp:incident <description>` to get structured incident response following the 4-tier debug order

### MCP Servers

- [x] **MCP-01**: Claude Code can read Gmail messages via Google Workspace MCP using existing racingpoint-google OAuth
- [x] **MCP-02**: Claude Code can read and write Google Sheets via the same MCP server
- [x] **MCP-03**: Claude Code can read Google Calendar events via the same MCP server
- [x] **MCP-04**: Claude Code can query racecontrol REST API (/fleet/health, /sessions, /billing, /laps) via a custom rc-ops-mcp server running on James's machine

### Deployment Automation

- [x] **DEPLOY-01**: Staging HTTP server and webterm auto-start on James's machine boot via HKLM Run key or Task Scheduler
- [x] **DEPLOY-02**: Post-deploy verification script checks binary size, polls /health, and confirms agent reconnection on /fleet/health
- [x] **DEPLOY-03**: Deploy script enforces canary-first (Pod 8) with explicit human approval before fleet rollout

### Monitoring & Alerting

- [x] **MON-01**: racecontrol emits structured JSON logs via tracing-subscriber with daily file rotation
- [x] **MON-02**: rc-agent emits structured JSON logs via tracing-subscriber with daily file rotation
- [x] **MON-03**: racecontrol triggers email alert when error rate exceeds N errors in M minutes (configurable threshold)
- [ ] **MON-04**: Netdata agent installed on racecontrol server (.23) collecting system metrics (CPU, RAM, disk, network)
- [ ] **MON-05**: Netdata agent installed on all 8 pods collecting system metrics, deployed via rc-agent :8090 exec
- [ ] **MON-06**: WhatsApp notification sent to Uday for P0 severity events (all pods offline, billing crash) via existing racingpoint-whatsapp-bot
- [ ] **MON-07**: Weekly fleet uptime report emailed to Uday (total sessions, pod uptime %, revenue in credits, incidents)

## v9.x Future Requirements

Deferred to after v9.0 validation. Tracked but not in current roadmap.

### Skills & Hooks (Phase 2)

- **SKILL-06**: `/rp:logbook` skill appends timestamped IST incident entries to LOGBOOK
- **SKILL-07**: `/rp:fleet-health` skill summarizes all pod states from /fleet/health
- **SKILL-08**: `/rp:new-pod-config <pod>` skill generates rc-agent-pod{N}.toml from template
- **HOOK-01**: SessionStart hook re-injects Racing Point context after context compaction
- **HOOK-02**: PostToolUse hook auto-notifies Bono via comms-link INBOX.md after git commits

### Deployment (Gated)

- **DEPLOY-04**: Ansible fleet management after WinRM/SSH validated on Pod 8

### Monitoring (Extended)

- **MON-08**: Prometheus /metrics endpoint in racecontrol for custom dashboards
- **MON-09**: Grafana dashboards (only if Netdata auto-dashboards insufficient)

## Out of Scope

| Feature | Reason |
|---------|--------|
| SaltStack (v6.0) | Blocked at BIOS AMD-V, WSL2 portproxy failure. Do not re-invest. |
| ELK/OpenSearch | 4GB+ RAM overhead unjustified for 8 pods. `jq` on JSON files sufficient. |
| OpenTelemetry distributed tracing | Overkill for LAN venue. Structured logs with session_id/pod_id correlation sufficient. |
| Cloud monitoring (Datadog, New Relic) | $100-500/month unjustified. Netdata self-hosted is free and keeps data on-site. |
| Docker containers on pods | Session 1 GUI + USB hardware access incompatible with containers. |
| MCP for Dahua cameras | No vision pipeline for RTSP streams. Use NVR web dashboard. |
| MCP for pod Ollama instances | Pod LLMs are autonomous debuggers, not orchestrated from James. |
| One giant "Master" skill | Exceeds skill character budget. Separate focused skills instead. |
| Fully automated no-approval deploys | Bad deploy crashes all pods. Human approval gate before fleet rollout is intentional. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SKILL-01 | Phase 51 | Complete |
| SKILL-02 | Phase 51 | Complete |
| SKILL-03 | Phase 51 | Complete |
| SKILL-04 | Phase 51 | Complete |
| SKILL-05 | Phase 51 | Complete |
| MCP-01 | Phase 52 | Complete |
| MCP-02 | Phase 52 | Complete |
| MCP-03 | Phase 52 | Complete |
| MCP-04 | Phase 52 | Complete |
| DEPLOY-01 | Phase 53 | Complete |
| DEPLOY-02 | Phase 53 | Complete |
| DEPLOY-03 | Phase 53 | Complete |
| MON-01 | Phase 54 | Complete |
| MON-02 | Phase 54 | Complete |
| MON-03 | Phase 54 | Complete |
| MON-04 | Phase 55 | Pending |
| MON-05 | Phase 55 | Pending |
| MON-06 | Phase 56 | Pending |
| MON-07 | Phase 56 | Pending |

**Coverage:**
- v9.0 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0

---

## v10.0 Requirements — Conspit Link Full Capability Unlock

**Core Value:** When a session ends the wheelbase centers safely, and when the next session starts the wheel auto-loads the right profile without staff intervention.

### Safety & Session Lifecycle

- [x] **SAFE-01**: Wheelbase returns to center within 2 seconds of game session ending (no stuck rotation)
- [x] **SAFE-02**: Session-end sequence uses `fxm.reset` + `axis.idlespring` (NOT estop) for routine shutdown
- [x] **SAFE-03**: Force ramp-up is gradual (500ms minimum) when applying centering spring -- no snap-back
- [x] **SAFE-04**: Venue power capped at safe maximum via `axis.power` command
- [x] **SAFE-05**: ESTOP reserved for genuine emergencies only (separate code path from session end)
- [x] **SAFE-06**: ConspitLink gracefully closed (WM_CLOSE) before HID safety commands to avoid P-20 contention
- [x] **SAFE-07**: ConspitLink restarted after safety sequence completes, with JSON integrity verification

### ConspitLink Process Management

- [ ] **PROC-01**: Hardened watchdog with crash-count tracking and graceful restart (never taskkill /F)
- [ ] **PROC-02**: Post-restart config file verification (JSON parse check)
- [ ] **PROC-03**: Config file backup before any write operation
- [ ] **PROC-04**: Window minimization survives ConspitLink restarts

### Game Profile Switching

- [ ] **PROF-01**: `Global.json` placed at `C:\RacingPoint\` on each pod (fix runtime path)
- [ ] **PROF-02**: `GameToBaseConfig.json` mappings point to Racing Point venue presets
- [ ] **PROF-03**: rc-agent pre-loads correct preset BEFORE game launch (not after)
- [ ] **PROF-04**: Auto game detection verified working for AC, F1 25, ACC/AC EVO, AC Rally
- [ ] **PROF-05**: Fallback to default safe preset if game not recognized

### FFB Preset Tuning

- [ ] **FFB-01**: Venue-tuned `.Base` preset for Assetto Corsa (based on Yifei Ye pro preset)
- [ ] **FFB-02**: Venue-tuned `.Base` preset for F1 25 (custom -- no pro preset exists)
- [ ] **FFB-03**: Venue-tuned `.Base` preset for ACC / AC EVO (based on Yifei Ye pro preset)
- [ ] **FFB-04**: Venue-tuned `.Base` preset for AC Rally (custom -- no pro preset exists)
- [ ] **FFB-05**: Correct steering angle per game (e.g., 900 deg AC, 360 deg F1, 800 deg rally)
- [ ] **FFB-06**: All presets stored in version control under `.planning/presets/`

### Fleet Config Distribution

- [ ] **FLEET-01**: rc-agent can receive config push from racecontrol via WebSocket
- [ ] **FLEET-02**: Atomic file writes (write-to-temp, rename) for all config updates
- [ ] **FLEET-03**: Config checksum in heartbeat for drift detection
- [ ] **FLEET-04**: `Global.json` written to BOTH install dir and `C:\RacingPoint\` on push
- [ ] **FLEET-05**: Graceful ConspitLink stop -> write -> restart -> verify cycle on config push
- [ ] **FLEET-06**: Golden config directory in repo for version-controlled master configs

### Fleet Monitoring (v10.0)

- [ ] **CLMON-01**: rc-agent reports active ConspitLink preset per pod
- [ ] **CLMON-02**: rc-agent reports config file hashes per pod
- [ ] **CLMON-03**: rc-agent reports ConspitLink firmware version per pod
- [ ] **CLMON-04**: racecontrol dashboard shows fleet config status at a glance

### Telemetry & Display

- [ ] **TELE-01**: Wheel LCD dashboard showing RPM, speed, gear for all 4 venue games
- [ ] **TELE-02**: `GameSettingCenter.json` telemetry fields enabled for all venue games
- [ ] **TELE-03**: Shift light LEDs configured with Auto RPM for AC, ACC
- [ ] **TELE-04**: Shift light LEDs configured with manual RPM thresholds for F1 25, AC Rally
- [ ] **TELE-05**: RGB button lighting tied to telemetry (DRS, ABS, TC, flags) per game
- [ ] **TELE-06**: UDP port chain documented: game -> ConspitLink (20778)

### v10.0 Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SAFE-01 | Phase 57: Session-End Safety | Complete |
| SAFE-02 | Phase 57: Session-End Safety | Complete |
| SAFE-03 | Phase 57: Session-End Safety | Complete |
| SAFE-04 | Phase 57: Session-End Safety | Complete |
| SAFE-05 | Phase 57: Session-End Safety | Complete |
| SAFE-06 | Phase 57: Session-End Safety | Complete |
| SAFE-07 | Phase 57: Session-End Safety | Complete |
| PROC-01 | Phase 58: ConspitLink Process Hardening | Pending |
| PROC-02 | Phase 58: ConspitLink Process Hardening | Pending |
| PROC-03 | Phase 58: ConspitLink Process Hardening | Pending |
| PROC-04 | Phase 58: ConspitLink Process Hardening | Pending |
| PROF-01 | Phase 59: Auto-Switch Configuration | Pending |
| PROF-02 | Phase 59: Auto-Switch Configuration | Pending |
| PROF-03 | Phase 60: Pre-Launch Profile Loading | Pending |
| PROF-04 | Phase 59: Auto-Switch Configuration | Pending |
| PROF-05 | Phase 60: Pre-Launch Profile Loading | Pending |
| FFB-01 | Phase 61: FFB Preset Tuning | Pending |
| FFB-02 | Phase 61: FFB Preset Tuning | Pending |
| FFB-03 | Phase 61: FFB Preset Tuning | Pending |
| FFB-04 | Phase 61: FFB Preset Tuning | Pending |
| FFB-05 | Phase 61: FFB Preset Tuning | Pending |
| FFB-06 | Phase 61: FFB Preset Tuning | Pending |
| FLEET-01 | Phase 62: Fleet Config Distribution | Pending |
| FLEET-02 | Phase 62: Fleet Config Distribution | Pending |
| FLEET-03 | Phase 62: Fleet Config Distribution | Pending |
| FLEET-04 | Phase 62: Fleet Config Distribution | Pending |
| FLEET-05 | Phase 62: Fleet Config Distribution | Pending |
| FLEET-06 | Phase 62: Fleet Config Distribution | Pending |
| CLMON-01 | Phase 63: Fleet Monitoring | Pending |
| CLMON-02 | Phase 63: Fleet Monitoring | Pending |
| CLMON-03 | Phase 63: Fleet Monitoring | Pending |
| CLMON-04 | Phase 63: Fleet Monitoring | Pending |
| TELE-01 | Phase 64: Telemetry Dashboards | Pending |
| TELE-02 | Phase 64: Telemetry Dashboards | Pending |
| TELE-03 | Phase 65: Shift Lights & RGB Lighting | Pending |
| TELE-04 | Phase 65: Shift Lights & RGB Lighting | Pending |
| TELE-05 | Phase 65: Shift Lights & RGB Lighting | Pending |
| TELE-06 | Phase 64: Telemetry Dashboards | Pending |

**v10.0 Coverage:**
- v10.0 requirements: 38 total
- Mapped to phases: 38
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after v10.0 Conspit Link milestone added (phases 57-65)*
