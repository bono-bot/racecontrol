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

- [ ] **DEPLOY-01**: Staging HTTP server and webterm auto-start on James's machine boot via HKLM Run key or Task Scheduler
- [ ] **DEPLOY-02**: Post-deploy verification script checks binary size, polls /health, and confirms agent reconnection on /fleet/health
- [ ] **DEPLOY-03**: Deploy script enforces canary-first (Pod 8) with explicit human approval before fleet rollout

### Monitoring & Alerting

- [ ] **MON-01**: racecontrol emits structured JSON logs via tracing-subscriber with daily file rotation
- [ ] **MON-02**: rc-agent emits structured JSON logs via tracing-subscriber with daily file rotation
- [ ] **MON-03**: racecontrol triggers email alert when error rate exceeds N errors in M minutes (configurable threshold)
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
| DEPLOY-01 | Phase 53 | Pending |
| DEPLOY-02 | Phase 53 | Pending |
| DEPLOY-03 | Phase 53 | Pending |
| MON-01 | Phase 54 | Pending |
| MON-02 | Phase 54 | Pending |
| MON-03 | Phase 54 | Pending |
| MON-04 | Phase 55 | Pending |
| MON-05 | Phase 55 | Pending |
| MON-06 | Phase 56 | Pending |
| MON-07 | Phase 56 | Pending |

**Coverage:**
- v9.0 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after roadmap creation — all 19 requirements mapped to phases 51-56*
