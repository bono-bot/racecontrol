# Project Research Summary

**Project:** Racing Point Operations — v9.0 Tooling & Automation
**Domain:** Venue ops tooling — Claude Code skills, MCP servers, deployment automation, monitoring/alerting
**Researched:** 2026-03-20 IST
**Confidence:** HIGH

## Executive Summary

v9.0 Tooling & Automation is an operator-experience milestone, not a customer-facing one. The goal is to reduce the weekly manual overhead for James (developer) and Uday (operator) by codifying repetitive workflows into Claude Code skills, repairing and extending MCP integrations for Google Workspace access, automating the pod binary deploy pipeline, and introducing a lightweight monitoring stack that stays operational when the venue internet drops. The existing Rust/Axum/Next.js/SQLite stack is unchanged — all new tooling is additive, external, or session-side.

The recommended approach is strictly layered by deployment risk and time-to-value. Phase 1 delivers maximum ROI with minimum infrastructure change: a project CLAUDE.md (zero installs), four Claude Code skills (zero installs), and Grafana Alloy with axum-prometheus metrics (low-risk Cargo.toml additions). Phase 2 repairs the broken Gmail MCP and deploys the Prometheus+Grafana monitoring stack on-premises on the server. Phase 3 adds deployment automation via Fabric+OpenSSH as a pendrive-replacement, gated on successfully enabling OpenSSH on at least one pod — which has never been verified. Every phase is offline-capable by design: Prometheus+Grafana run on the venue LAN, and skills call existing local endpoints.

The critical risk for this milestone is repeating the v6.0 pattern: investing in fleet tooling (Salt, WinRM, OpenSSH) that fails silently on this network due to Windows Defender, Session 0/1 constraints, or non-domain authentication. The mitigation is explicit: every new tool must pass a five-question checklist (non-domain Windows, standard HTTP ports, Defender-safe, offline-capable, Windows 11 verified) before a line of implementation begins. Ansible and Fabric are the lowest-risk automation path if OpenSSH validation succeeds; they are explicitly deferred until that validation happens.

## Key Findings

### Recommended Stack

See `.planning/research/STACK.md` for full details and version rationale.

The tooling stack is additive — nothing in the existing Rust/Axum/SQLite/rc-agent architecture changes. For Claude Code skills, the SKILL.md format with `disable-model-invocation: true` on all operational skills (deploy, logbook write) is the correct pattern. For metrics, `axum-prometheus 0.10.0` + `metrics 0.24` provide a 3-line integration to expose a Prometheus `/metrics` endpoint from racecontrol. Grafana Alloy (not the EOL Grafana Agent) replaces the standalone `windows_exporter` pattern and scrapes both OS metrics and the racecontrol endpoint. Grafana Cloud free tier (10K series, 14-day retention) handles the Racing Point fleet's ~160 metric series comfortably; self-hosted Prometheus+Grafana on server .23 is the alternative if data sovereignty is required.

For MCP, `taylorwilsdon/google_workspace_mcp` fixes the broken Gmail OAuth and covers Gmail, Sheets, Calendar, and Drive in one server. The existing `racingpoint-mcp-gmail` server.js pattern is correct — only the OAuth token needs re-authorization. For deployment automation, Fabric (Python SSH) is the right-sized tool for 8 pods (vs. Ansible's complexity); both are gated on OpenSSH validation on pods.

**Core technologies:**
- `axum-prometheus 0.10.0` + `metrics 0.24`: racecontrol metrics endpoint — 3-line integration, no route changes
- Grafana Alloy (v1.x, Windows MSI): scrape OS + app metrics, forward to Grafana Cloud or local Prometheus — EOL Grafana Agent replacement
- `taylorwilsdon/google_workspace_mcp`: Gmail/Sheets/Calendar/Drive MCP — fixes broken OAuth, one OAuth credential
- SKILL.md format with `disable-model-invocation: true`: Claude Code skills for operational workflows — zero dependencies
- Fabric 3.2.x (Python SSH): pod deploy automation — 50 lines vs. Ansible's 200+ YAML — gated on OpenSSH validation
- `prometheus-mcp-server` (pab1it0): query Prometheus metrics from Claude — 18 read-only tools — add only after Prometheus is deployed

### Expected Features

See `.planning/research/FEATURES.md` for full prioritization matrix and ROI analysis.

**Must have (table stakes — v9.0 Phase 1):**
- Project CLAUDE.md with Racing Point topology, pod IPs, naming conventions — eliminates context re-establishment per session
- `/rp:deploy` + `/rp:deploy-server` skills — codify exact build+deploy sequence with `disable-model-invocation: true`
- `/rp:pod-status <pod>` skill — query any pod's rc-agent status with dynamic IP injection
- Google Workspace MCP (Gmail read + Sheets read) — fixes broken OAuth, unlocks business data access in Claude
- Structured JSON logging in racecontrol — one-line `tracing-subscriber` change, foundation for all monitoring
- Staging HTTP server auto-start (HKLM Run key) — prevents silent deploy failures after workstation reboot
- Deploy verification script — catches failed deploys before customer impact

**Should have (v9.0 Phase 2 — add after Phase 1 validated in daily ops):**
- `/rp:incident` skill — structured incident response using the 4-tier debug order
- Netdata on server + pods — hardware degradation detection, lightweight Windows MSI install
- Error rate alerting threshold — N errors in M minutes triggers email (in-racecontrol counter logic)
- `/rp:logbook` skill — auto-append incidents to LOGBOOK
- PostToolUse hook — auto-notify Bono on git commit (enforces standing rule)
- MCP-INVENTORY.md — track tool availability across James and Bono environments

**Defer (Phase 3 / v9.x+):**
- Ansible fleet management — blocked until WinRM/SSH validated on one pod
- rc-ops-mcp custom server — high-value but high build cost; validate MCP pattern with Google Workspace first
- WhatsApp P0 alerting — add after error rate thresholds proven reliable
- Weekly fleet uptime report — requires structured logging foundation
- Prometheus /metrics endpoint — add if Netdata auto-dashboards are insufficient

**Explicitly avoid:**
- Grafana+Prometheus full stack as MVP (Netdata covers this at lower cost and zero query language)
- ELK/OpenSearch (4GB+ RAM overhead for a `jq`-solvable problem)
- Docker on pods (gaming hardware, Session 1 GUI requirement)
- Cloud monitoring (Datadog/New Relic) — $100-500/month for an 8-pod venue

### Architecture Approach

See `.planning/research/ARCHITECTURE.md` for full component diagram and build order.

All v9.0 components are either purely additive or session-side. Net change to existing racecontrol Rust code: zero for Phase 1; three lines added (Cargo.toml + main.rs) for the metrics endpoint in Phase 2. Skills live in `.claude/skills/` (project-scoped, committed to repo). Hooks live in `~/.claude/settings.json` (personal, not committed). MCP servers are already configured on James's machine — Gmail needs OAuth repair only. The monitoring stack (Prometheus + Grafana) runs entirely on server .23 LAN-only; no metrics leave the venue except optionally to Grafana Cloud free tier. The key architectural constraint is that all monitoring must operate offline: Prometheus + Grafana on LAN, alerts via WhatsApp (Evolution API localhost:53622) first, email second.

**Major components:**
1. Claude Code Skills (`.claude/skills/`) — project-scoped operational macros; call existing deploy_pod.py + rc-agent :8090 + racecontrol API; no new infrastructure
2. Claude Code Hooks (`~/.claude/settings.json`) — PreToolUse guard (block rc-agent.exe on .27), PostToolUse logger (append to DEPLOY_LOG.md + INBOX.md for Bono)
3. Google Workspace MCP (`taylorwilsdon/google_workspace_mcp`) — replaces broken `racingpoint-mcp-gmail`; HTTP transport for Claude Code; OAuth repair one-time
4. Grafana Alloy on server + pods — scrapes `/metrics` from racecontrol + Windows OS metrics; forwards to Prometheus (local) or Grafana Cloud
5. Prometheus + Grafana on server .23 — LAN-only monitoring; Grafana JSON plugin reads `/api/v1/fleet/health` for operational state alongside OS metrics
6. Fabric (`fabfile.py`) — Python SSH task runner; wraps same deploy logic as install.bat v5 over SSH once OpenSSH is enabled on pods

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for full recovery strategies and integration gotchas.

1. **Repeating the v6.0 fleet-tool failure pattern** — Salt, WinRM, and OpenSSH have all failed on this network. Before evaluating any new fleet tool, answer five questions: (a) non-domain Windows support, (b) Defender-safe installer, (c) standard HTTP ports, (d) offline-tolerant, (e) Windows 11 verified. If any answer is no, don't proceed. Build on rc-agent remote_ops :8090 first.

2. **OAuth token expiry silently breaking Gmail automation** — Google OAuth refresh tokens expire in 7 days for test-mode apps. The broken `racingpoint-mcp-gmail` is the current example. Never make Gmail the sole alert path. Keep `send_email.js` as primary alert fallback. Publish the OAuth app to production mode to remove 7-day expiry. Add a daily health-check API call to detect token expiry before it silences critical alerts.

3. **Internet dependency for venue-critical operations** — Monitoring or automation that requires internet breaks during venue outages exactly when it matters most. All monitoring stays on LAN (Prometheus + Grafana on .23). Alerts go via WhatsApp (Evolution API localhost:53622, LAN-local) first. Follow the existing cloud_sync pattern: local-authoritative, cloud is a secondary sync.

4. **Windows Session 0 vs Session 1 breaking new services** — New agents installed as SYSTEM Windows Services run in Session 0 and cannot interact with the kiosk, lock screen, or game processes (Session 1). For any new process that needs GUI access: use HKLM Run key (`start-service.bat`) pattern. For background-only services (Prometheus, Grafana Alloy, windows_exporter): install as SYSTEM service but verify explicitly that all target operations work from Session 0 before deployment design.

5. **Alert fatigue from uncalibrated monitoring thresholds** — The existing system has production-calibrated thresholds (6s heartbeat, 10s idle, ALERT-02 rate limiting). A new monitoring layer that ignores these will generate 40+ false-positive alerts per day (normal AC launch CPU spikes, game-startup WS gaps). Start with zero alerts. Document the normal behavior baseline for one week. Add alerts one at a time, mirroring ALERT-02's 30-minute rate limit per alert type.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Context and Skills Foundation
**Rationale:** Highest ROI, zero new infrastructure, zero deployment risk. Everything in this phase is file authoring or one-line config changes. Delivers immediate daily-ops improvement for James in every Claude Code session before any infrastructure work begins.
**Delivers:** Project CLAUDE.md, four `/rp:*` skills (`/rp:deploy`, `/rp:deploy-server`, `/rp:pod-status`, `/rp:fleet-health`), staging server auto-start, deploy verification script
**Addresses:** Project CLAUDE.md (P1), all `/rp:*` skills (P1), staging HTTP server auto-start (P1), deploy verification script (P1)
**Avoids:** Over-engineering anti-pattern (zero new services), automation-requires-confirmation pitfall (all skills define fallback policy before marking complete)

### Phase 2: Metrics Instrumentation
**Rationale:** axum-prometheus is a 3-line Cargo.toml + main.rs change with zero breaking changes to existing routes. Must come before the Grafana stack (Phase 3) because custom `rp_*` metrics are what make dashboards meaningful beyond raw OS CPU graphs.
**Delivers:** `/metrics` endpoint on racecontrol :8080, custom gauges/counters for active sessions, pod WS state, game launches, billing credits, cloud sync lag
**Uses:** `axum-prometheus 0.10.0`, `metrics 0.24` — both compatible with existing axum 0.7
**Avoids:** "Dashboard without app metrics" anti-pattern documented in ARCHITECTURE.md

### Phase 3: MCP Repair and Google Workspace Integration
**Rationale:** Gmail OAuth re-authorization is a prerequisite for any email-based alerting to be reliable. Repair before building new monitoring that depends on email alerts. Google Workspace MCP is `pip install` + one-time OAuth flow — low complexity, high daily-ops value.
**Delivers:** Working Gmail read/send, Google Sheets read/write, Google Calendar read in Claude Code sessions. MCP-INVENTORY.md as James-Bono tool coordination artifact.
**Addresses:** Gmail MCP (P1), Google Sheets MCP (P1), Google Calendar MCP (P1), MCP config drift pitfall
**Avoids:** OAuth expiry silent failure (token health check + fallback to send_email.js required before phase is complete), MCP config drift (notify Bono via INBOX.md with config diff)

### Phase 4: On-Premises Monitoring Stack
**Rationale:** Grafana Alloy + Prometheus + Grafana on server .23 provides fleet-wide OS health and operational state visibility. LAN-only — works during internet outages. Must come after Phase 2 (app metrics instrumented) for meaningful dashboards.
**Delivers:** Grafana Alloy on server and all 8 pods, Prometheus on server, Grafana fleet-health dashboard (OS metrics + billing state + pod WS status), LAN-only
**Uses:** Grafana Alloy v1.x (Windows MSI), Prometheus 3.x, Grafana OSS 11.x — all Windows services on server .23
**Avoids:** Internet-dependency pitfall (everything on LAN), Session 0 pitfall (all background services, HTTP-only, no GUI needed), alert fatigue (one-week baseline calibration before any alert rule is enabled)

### Phase 5: Deployment Automation (Gated)
**Rationale:** Fabric+OpenSSH replaces the pendrive workflow. Explicitly gated on a validation spike confirming OpenSSH works on at least one pod. This has never been confirmed on this fleet. If validation fails, phase scope changes to rc-agent :8090 HTTP exec improvements only.
**Delivers:** `fabfile.py` deploy script, OpenSSH enabled on all 8 pods via rc-agent remote_ops exec, canary-first deploy enforcement (Pod 8 canary → human approval gate → fleet rollout)
**Uses:** Fabric 3.2.x, native Windows OpenSSH Server, Tailscale mesh for routing
**Avoids:** Fleet-tool-failure pitfall (five-question checklist passed, Pod 8 validated before fleet), over-engineering pitfall (Fabric is 50 lines vs Ansible 200+ YAML for 8 pods)

### Phase Ordering Rationale

- Phases 1-3 can be built in parallel — no hard dependencies between them. Phase 1 is pure file authoring. Phase 2 is a small Cargo.toml + Rust change. Phase 3 is MCP setup on James's workstation.
- Phase 4 (monitoring) depends on Phase 2 (metrics instrumented first). Deploying Grafana without app metrics produces OS CPU graphs with no correlation to Racing Point events — the anti-pattern documented in ARCHITECTURE.md.
- Phase 5 (deployment automation) is explicitly deferred and gated. It has the highest historical failure rate on this network. Run Phase 5 as a validation spike before any implementation work is scoped.
- The dependency graph from FEATURES.md confirms: structured JSON logging (Phase 2/4 foundation) must precede error rate alerting and log search features. Deploy skills (Phase 1) are independent and deliver value immediately.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (MCP):** `taylorwilsdon/google_workspace_mcp` HTTP transport mode was not hands-on tested in Claude Code (MEDIUM confidence). If HTTP transport fails, fall back to repairing `racingpoint-mcp-gmail` server.js OAuth directly. Research the fallback before committing to the community tool.
- **Phase 4 (Monitoring):** Grafana Alloy Windows service install on Windows Server .23 (not Windows 11) needs validation. Verify `winget` is available on the server. If not, use direct MSI download from Grafana releases page.
- **Phase 5 (Deployment Automation):** Full validation spike required before implementation. Confirm: (1) `Add-WindowsCapability OpenSSH.Server` succeeds on at least one pod. (2) SSH key auth works from James .27 to pod via Tailscale IP. (3) `scp` binary transfer completes in <60s for a 10MB binary. This is a hard go/no-go gate.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Skills):** SKILL.md format is official Anthropic documentation with HIGH confidence. No research needed.
- **Phase 2 (Metrics):** `axum-prometheus 0.10.0` docs.rs verified. Verify axum version compatibility with `cargo tree | grep axum` before adding dependency — no other research needed.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Claude Code official docs fetched live; axum-prometheus docs.rs verified; Grafana Alloy official docs. Single MEDIUM item: `taylorwilsdon/google_workspace_mcp` is a community project |
| Features | HIGH | Feature prioritization cross-referenced with existing MEMORY.md operational history. P1 features all have documented ROI from real operational pain points |
| Architecture | HIGH | Existing codebase read directly (deploy_pod.py, settings.json, rc-sentry/main.rs, racecontrol state.rs). Integration points verified against actual code |
| Pitfalls | HIGH | All critical pitfalls derived from documented failures: Salt v6.0 blocked, Gmail OAuth expired, WinRM failures, Session 0/1 split problem. Not hypothetical |

**Overall confidence:** HIGH

### Gaps to Address

- **OpenSSH pod status unknown:** MEMORY.md documents OpenSSH failure on the server (component store corrupted). Pod status is undocumented. Treat Phase 5 as blocked until a manual test confirms at least one pod can enable OpenSSH Server. If all pods fail, Phase 5 scope changes to rc-agent :8090 HTTP exec improvements only.
- **`google_workspace_mcp` HTTP transport verification:** HTTP transport for Claude Code (vs. stdio for Claude Desktop) was not hands-on tested. If HTTP transport fails, alternative is running the MCP as a stdio server in `.claude/settings.json` — same pattern as the existing `racingpoint-mcp-gmail`.
- **Grafana Alloy on Windows Server .23:** Server .23 may not have `winget` if it lacks Windows App Installer. Alternative: download Grafana Alloy MSI directly from the Grafana releases page. Verify before deployment planning.
- **Hardcoded JWT secret (existing P0):** PITFALLS.md flags `default_jwt_secret()` in config.rs as a P0 security issue. Before adding any new auth integration, this must be fixed. Treat as a prerequisite gate for Phase 3 (MCP with racecontrol API access).

## Sources

### Primary (HIGH confidence)
- Claude Code Skills official docs (`code.claude.com/docs/en/skills`) — SKILL.md format, frontmatter, invocation control, subagent forking
- Claude Code Hooks reference (`code.claude.com/docs/en/hooks`) — PreToolUse/PostToolUse, stdin JSON, exit codes
- `axum-prometheus` docs.rs — version 0.10.0, default metrics, main.rs integration pattern
- Grafana Alloy Windows docs (`grafana.com/docs/alloy/latest/set-up/install/windows/`) — MSI installer, `prometheus.exporter.windows` component
- Grafana Cloud pricing (`grafana.com/pricing/`) — 10K series, 14-day retention, 3 users free
- `windows_exporter` GitHub releases — v0.31.4 latest stable
- Tailscale SSH GitHub issue #14942 — SSH server not supported on Windows (confirmed open)
- Existing codebase read directly — `deploy/deploy_pod.py`, `~/.claude/settings.json`, `crates/rc-sentry/src/main.rs`, `crates/racecontrol/src/state.rs`
- MEMORY.md — WinRM failure, Salt v6.0 blocked, Gmail OAuth expired, OpenSSH server component store corruption
- CONCERNS.md (via PITFALLS.md) — hardcoded JWT secret P0, cloud sync fragility

### Secondary (MEDIUM confidence)
- `taylorwilsdon/google_workspace_mcp` GitHub — 287+ issues, active March 2026, HTTP transport for Claude Code
- `pab1it0/prometheus-mcp-server` GitHub — 18 read-only Prometheus tools, PromQL support
- Fabric official docs (`fabfile.org`) — v3.2.x, Python SSH tasks, Connection API
- Claude Code skills merged from slash commands (community article) — backward compatibility confirmed

### Tertiary (LOW confidence)
- Grafana Alloy on Windows Server (not Windows 11): documented for Windows 11, less community documentation for Windows Server variants — needs hands-on validation

---
*Research completed: 2026-03-20 IST*
*Ready for roadmap: yes*
