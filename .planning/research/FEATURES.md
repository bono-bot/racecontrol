# Feature Research — Tooling & Automation (v9.0)

**Domain:** Venue operations tooling — Claude Code skills, MCP servers, deployment automation, monitoring/alerting
**Researched:** 2026-03-20
**Confidence:** HIGH (Claude Code official docs verified, MCP ecosystem cross-referenced, Ansible/monitoring confirmed via multiple sources)

---

## Context: What Already Exists vs What This Milestone Adds

Racing Point already has working operations infrastructure. This milestone improves the *developer and operator experience* for James and Uday — not the customer experience. The test is simple: does Uday need fewer manual interventions per week?

### What Already Exists (Do NOT Duplicate)

| System | What It Does | Gap It Leaves |
|--------|-------------|---------------|
| rc-agent remote_ops :8090 | HTTP exec endpoint for one-off pod commands | Manual: James must curl each pod individually, no fleet-wide orchestration |
| Pendrive install.bat v5 | Deploy rc-agent to pods | Physical: requires James to walk to each pod, not automatable remotely |
| Email alerts via send_email.js | Critical failure notifications | Reactive: fires after failure, no proactive trend visibility |
| Fleet health dashboard | Real-time pod WS status in kiosk panel | No alerting, no history, no metrics, no SMS/push |
| Claude Code (James's workstation) | AI-assisted coding | No venue-specific skills, no Racing Point context baked in, no Google Workspace integration |
| Google Workspace (Gmail, Sheets, Calendar) | Business operations | No MCP integration — AI cannot read/write bookings, reports, or customer data |
| tracing crate (already in Cargo.toml) | Debug logging | Unstructured text logs, no persistent storage, no search, no alerting thresholds |

---

## Feature Landscape

### Category 1: Claude Code Skills

Features for extending Claude Code with Racing Point-specific context and workflows.

#### Table Stakes — Claude Code Skills

Features where absence means Claude wastes tokens re-establishing context every session, or James cannot invoke common workflows reliably.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Project CLAUDE.md with Racing Point context | Without it Claude asks about the stack, pod IPs, crate names every session. CLAUDE.md is auto-loaded at session start. | LOW | Already partially in MEMORY.md on James's machine; needs a project-level `.claude/CLAUDE.md` in the racecontrol repo that loads the Racing Point topology, pod IPs, binary naming conventions, and constraint rules. One-time authoring effort. |
| `/gsd:new-milestone` skill (already exists via GSD) | James uses GSD for project planning. Without project-level skills the GSD commands must be typed in full. | LOW | GSD skills already installed globally. Confirm they appear when working in `/racecontrol`. No new work if global skills resolve correctly. |
| `/rp:deploy` skill — build + copy binary to staging | James runs `cargo build --release` then copies to `deploy-staging/` manually. A skill codifies the exact sequence so it never differs between deploys. | LOW | Skill content: `cargo build --release --bin rc-agent`, size check, copy to `C:\Users\bono\racingpoint\deploy-staging\rc-agent.exe`. 15 lines of SKILL.md with `disable-model-invocation: true`. |
| `/rp:deploy-server` skill — build + replace racecontrol binary | Same need as above for the server binary. Kill old process, swap binary, verify port 8080 comes back. | LOW | Pattern: stop → delete old → copy new → start → health check. Must wait for `/health` on :8080 before declaring success. |
| `/rp:pod-status <pod_number>` skill — query pod state | James checks pod health via curl manually. A skill that calls `http://192.168.31.{ip}:8090/status` and summarizes the response saves time during incidents. | LOW | Use dynamic context injection: `!curl http://192.168.31.{ip}:8090/status`. Maps pod number to IP from the pod-IP table in the skill. |
| `/rp:incident <description>` skill — structured incident response | Without this, James pastes raw logs and asks Claude to diagnose ad hoc. A skill that knows the debugging playbook, crate structure, and common fix patterns gives focused answers. | MEDIUM | Skill loads: debugging-playbook context + asks Claude to follow the 4-tier debug order (deterministic → memory → local LLM → cloud). Reduces "what crate owns this?" back-and-forth. |

#### Differentiators — Claude Code Skills

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `/rp:logbook` skill — append incident/fix to LOGBOOK | LOGBOOK is updated manually today. A skill that appends a timestamped IST entry from the current session prevents forgetting to document fixes. | LOW | Reads current LOGBOOK, appends entry in correct format, writes back. `disable-model-invocation: true` — only James should trigger this deliberately. |
| `/rp:new-pod-config <pod_number>` skill | Generating rc-agent-pod{N}.toml files by hand from the template is error-prone. A skill that reads the pod-IP table and generates the correct TOML cuts a common source of deploy mistakes. | LOW | Skill template + pod IP map. Outputs rc-agent-pod{N}.toml to `deploy-staging/`. |
| `/rp:fleet-health` skill — summarize all pod states | Calls `/fleet/health` on racecontrol server and formats the response as a human-readable summary. Good for quick stand-up or sharing state with Uday. | LOW | Dynamic context injection: `!curl http://192.168.31.23:8080/api/v1/fleet/health`. Claude formats as Markdown table. |
| Session-level context injection hook (SessionStart) | After compaction Claude loses Racing Point context. A hook that re-injects pod IPs, binary naming rules, and current milestone from CLAUDE.md prevents "which crate is racecontrol again?" after every compact. | LOW | `SessionStart` with `compact` matcher, echoes key context lines. Built on top of project CLAUDE.md existing content. |
| `PostToolUse` git commit hook — auto-notify Bono | Standing rule: every commit must be followed by notifying Bono via comms-link INBOX.md. A hook that fires after `Bash(git commit *)` and appends to INBOX.md makes the rule automatic. | MEDIUM | Hook: PostToolUse, matcher: `Bash`, command checks if `git commit` was the command then runs comms-link append + push. Will need to parse commit hash from git output. |

#### Anti-Features — Claude Code Skills

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| One giant "Racing Point Master" skill | Single entry point that does everything | Context budget: a 2000-line skill eats the entire skill character budget, leaving nothing for other skills. Claude ignores skills beyond the budget. | Separate focused skills per workflow: `/rp:deploy`, `/rp:incident`, `/rp:pod-status`. Each under 500 lines. |
| `user-invocable: false` on all operational skills | "Claude should just know when to use them" | Claude auto-invoking `/rp:deploy` mid-conversation would be catastrophic. Deploy skills must be opt-in only. | `disable-model-invocation: true` on any skill with side effects (deploy, logbook write, pod exec). Allow model invocation only on read-only skills like `/rp:pod-status`. |
| Skills that shell out to rc-agent :8090 directly from Claude Code | "Claude should be able to push commands to pods" | Security: Claude Code running on James's machine could send arbitrary exec commands to pods if not carefully scoped. | Scope skills to read-only queries against rc-agent or racecontrol API. Destructive commands require James to manually confirm + paste. |

---

### Category 2: MCP Servers

Features for connecting Claude Code to external systems (Google Workspace, monitoring) via Model Context Protocol.

#### Table Stakes — MCP Servers

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Google Workspace MCP (Gmail read) | James troubleshoots issues that generate alert emails. Today he must open Gmail in browser and copy-paste. With MCP, Claude can read the alerts in-context. | LOW | Use `taylorwilsdon/google_workspace_mcp` — covers Gmail, Sheets, Calendar, Drive, Tasks in one server. OAuth via existing `racingpoint-google` repo OAuth. Single `npx` command install. |
| Google Sheets MCP (read/write) | Booking records, revenue reports, and customer data live in Sheets. Without MCP, James must copy-paste sheet data into Claude manually. | LOW | Same MCP server as Gmail. Requires `spreadsheets` scope in OAuth consent. High immediate ROI: Claude can analyze revenue trends or booking patterns without copy-paste. |
| Google Calendar MCP (read) | Uday books venue reservations and group events in Calendar. Claude cannot help optimize the schedule or detect conflicts without calendar access. | LOW | Same MCP server. Requires `calendar.readonly` scope. Useful for: "Do we have any group bookings this week that affect which pods I can test on?" |
| Filesystem MCP for deploy-staging | Claude currently uses File/Write tools to edit files. MCP filesystem server on `C:\Users\bono\racingpoint\` gives structured access to the deploy-staging directory, configs, and log files without giving Claude full system access. | LOW | `@modelcontextprotocol/server-filesystem` — standard MCP server, ships with Claude Code. Scope to `racingpoint/` directory only to limit blast radius. |

#### Differentiators — MCP Servers

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Custom Racing Point MCP server (rc-ops-mcp) | A purpose-built MCP server wrapping racecontrol's REST API (/fleet/health, /sessions, /billing, /laps) exposes venue state directly to Claude. Claude can answer "which pods are active right now?" without James manually curling. | HIGH | Node.js MCP server, ~300 lines. Wraps 8–10 racecontrol endpoints. Runs on James's machine, talks to server :8080. This is the highest-ROI custom tool but also the highest build cost. Defer to Phase 2 if timeline is tight. |
| Gmail MCP (send) for alert composition | When Claude drafts incident reports or Uday summaries, it could send the email directly via Gmail MCP instead of James copy-pasting to browser. | LOW | Same `google_workspace_mcp`, requires `gmail.send` scope in addition to `gmail.readonly`. Risk: Claude could send email autonomously. Use `disable-model-invocation: true` on any skill that triggers send. |
| Google Tasks MCP for venue operations checklist | Uday's daily opening/closing checklist lives in paper or mental memory. Connecting Google Tasks via MCP lets Claude help track daily ops without changing Uday's workflow tool. | MEDIUM | Same MCP server has Tasks integration. Value is lower than Gmail/Sheets — defer to after Gmail/Sheets validated. |
| Structured log search via MCP (future) | If racecontrol writes structured JSON logs, an MCP server wrapping a log search endpoint would let Claude query "show me last 10 errors on Pod 3" without tailing log files manually. | HIGH | Requires first building structured logging (Category 4). Don't build this MCP before the logging foundation exists. Dependency: structured logging must land first. |

#### Anti-Features — MCP Servers

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Remote MCP servers (Google Cloud managed) | "Google's managed MCP removes hosting burden" | Racing Point's Gmail OAuth is already configured locally in `racingpoint-google` repo. Migrating to Google Cloud managed MCP requires re-authorizing OAuth, setting up Cloud credentials, and paying for API calls. For a single venue with 1 AI user, local MCP is simpler and free. | Run `google_workspace_mcp` locally on James's machine. One `npm install` + OAuth token. No cloud account needed. |
| MCP for Dahua security cameras | "Claude could watch the venue cameras" | The Dahua NVR API is RTSP/HTTP and non-standard. Building an MCP wrapper around 13 camera feeds would be a significant project and Claude has no visual analysis pipeline for RTSP streams without additional tooling. | Use camera NVR's built-in web dashboard for camera review. MCP adds no value here without a vision pipeline. |
| MCP for Ollama on pods | "Claude could query each pod's local LLM" | Each pod has qwen3:0.6b for self-debugging only. Querying pod LLMs from James's machine adds network complexity and the pod LLMs have no knowledge beyond rc-agent debugging. | Use Ollama on James's machine (RTX 4070) directly for any Claude-side LLM needs. Pod LLMs are autonomous — not orchestrated. |

---

### Category 3: Deployment Automation

Features for replacing or augmenting the current pendrive workflow.

#### Table Stakes — Deployment Automation

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| HTTP-based deploy trigger via rc-agent :8090 | Already partially exists: rc-agent has `remote_ops` exec endpoint. A deploy script that uses this endpoint to: (1) send the new binary via HTTP to deploy-staging HTTP server on :27, (2) exec install steps remotely, provides pendrive-free deploys for all pods where rc-agent is running. | MEDIUM | This is the `current method` already documented in MEMORY.md. The gap is: it requires rc-agent to already be running. On fresh installs or crashed-agent scenarios, pendrive remains required. Automate what's possible now, keep pendrive for recovery. |
| Staging HTTP server auto-start on boot | `python webterm.py` on :9999 and HTTP server on :27 for binary serving are currently started manually. If James's machine restarts, deploys fail silently because the HTTP server is down. | LOW | Windows HKLM Run key or Task Scheduler to start both Python servers at login. 30-minute task. Prevents silent deploy failures. |
| Deploy verification script (post-deploy health check) | Already identified in v7.0 research. After binary swap: poll `/health`, check binary size changed, verify `/fleet/health` shows agents reconnected. Without this, a failed deploy is discovered when a customer session crashes. | LOW | Shell script, already partially designed in v7.0 FEATURES.md. Move to `scripts/deploy-verify.sh`. Reuses existing API endpoints. |
| Canary-first deploy enforcement (Pod 8 → all) | Pod 8 is the established canary convention. Without enforcement, a rushed deploy skips canary and hits all 8 pods simultaneously. One bad binary crashes all pods during operating hours. | LOW | Deploy script structure: `deploy-pod.sh 8` → verify → prompt "deploy to all? [y/N]" → `deploy-all.sh`. Not fully automated — the human approval gate before fleet deploy is intentional. |

#### Differentiators — Deployment Automation

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Ansible for fleet-wide deploys (when WinRM/SSH resolves) | Ansible + WinRM or OpenSSH would enable James to deploy to all 8 pods from a single YAML playbook on his machine, replacing the HTTP+curl pipeline entirely. | HIGH | **Blocked**: WinRM failed on this network previously (MEMORY.md: "WinRM: Failed on this network previously"). OpenSSH: server component store corrupted on the racecontrol server; status unknown on pods. Before investing in Ansible, validate WinRM or OpenSSH on ONE pod. Estimated: 2–4 hours validation + 4–8 hours playbook authoring. Not worth starting until connectivity is confirmed. |
| Chocolatey-based dependency management for pods | Pods currently have no package manager. If a pod needs a dependency (VC runtime, specific driver), it must be manually installed. Chocolatey + Ansible would enable `choco install` via playbook. | HIGH | Depends on Ansible connectivity working first. P3 — this is infrastructure polish, not immediate venue ops need. |
| Git-triggered deploy pipeline (CI on push to main) | On every push to main, automatically build + deploy to staging. Reduces the build-copy-verify sequence from 5 manual steps to one `git push`. | HIGH | Requires a CI runner with network access to venue (currently no external access from CI to 192.168.31.x subnet). GitHub Actions runners cannot reach the venue LAN. Would need Tailscale tunnel to CI runner or self-hosted GitHub Actions runner on James's machine. Significant infrastructure before any ROI. Defer. |
| rc-agent update endpoint (binary self-replace) | An `/update` HTTP endpoint on rc-agent that downloads the new binary from deploy-staging, stops itself, replaces its own binary, and restarts. Eliminates the HTTP exec sequence for routine rc-agent updates. | HIGH | In Rust on Windows, a process cannot replace its own running binary directly. Must write to temp path + use scheduled task or watchdog to swap on next start. Complex; risk of self-update leaving pods in broken state. Only pursue if fleet grows beyond 8 pods where HTTP exec becomes unwieldy. |

#### Anti-Features — Deployment Automation

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| SaltStack (v6.0, paused) | Mature fleet management, declarative state | Already blocked: WSL2 portproxy couldn't forward HTTP to salt-api. AMD-V BIOS issue on server. Known-broken after significant investment. | rc-agent :8090 HTTP exec + future Ansible with WinRM/SSH when connectivity is resolved. Do not re-invest in Salt until the BIOS/WSL2 blocker is resolved. |
| Fully automated no-approval deploys | "One command deploys to all 8 pods simultaneously" | During operating hours, a bad deploy crashes all pods simultaneously. A 5-second human approval gate before fleet-wide deploy is a worthwhile operational safety. | Canary-first with explicit human approval before fleet rollout. Automate the mechanics, not the decision. |
| Docker containers on pods | Containerize rc-agent for consistent deploys | Pods run Windows GUI applications and game launchers that require Session 1 access and direct hardware interaction (USB wheelbases, GPU). Docker on Windows adds Hyper-V virtualization overhead and cannot run GUI processes in Session 1. Complete non-starter for this workload. | Static Rust binary with static CRT (.cargo/config.toml already in place). No runtime dependencies, no containers needed. |

---

### Category 4: Monitoring and Alerting

Features for observability beyond the current email-on-failure approach.

#### Table Stakes — Monitoring and Alerting

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Structured JSON logging in racecontrol and rc-agent | Current tracing output is human-readable text. Structured JSON logs (via `tracing-subscriber` with JSON format) are machine-parseable, searchable, and compatible with any log aggregator. This is the foundation for all other monitoring features. | LOW | Add `tracing-subscriber` with `fmt::layer().json()`. Already a dependency in the Rust workspace. One-line config change. All existing `tracing::info!()` and `tracing::warn!()` calls emit valid JSON immediately. Write logs to file via `tracing_appender` for persistence. |
| Log rotation and retention policy | Without rotation, log files on the racecontrol server grow unbounded. 8 pods sending telemetry will produce 1–5GB of logs per month. | LOW | `tracing_appender::rolling::daily()` — creates date-stamped log files. Add a cleanup task (PowerShell scheduled task or Rust startup check) to delete logs older than 30 days. One-time setup. |
| Error rate alerting threshold (not just on-crash) | Current ALERT-01 fires on crash. But 50 errors in 10 minutes before a crash is more actionable than the crash itself. A rate-based threshold catches degradation before customer impact. | MEDIUM | In racecontrol's existing alert logic: add a counter per error type. If N errors in M minutes, send email alert. Uses existing `send_email.js` — no new alert channel needed. Pattern is deterministic; no external monitoring dependency. |
| Pod-level health metrics export | Per-pod CPU, memory, and process liveness from rc-agent. Currently all rc-agent state is in the WebSocket protocol — it is not exported to any time-series store. | MEDIUM | Two options: (a) rc-agent exposes a `/metrics` Prometheus text endpoint (40 lines of Rust), or (b) rc-agent sends metric samples to racecontrol which aggregates. Option (a) is simpler and standard. |

#### Differentiators — Monitoring and Alerting

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Netdata agents on pods (Windows) | Netdata has a native Windows agent that installs as a Windows service, collects 800+ metrics at per-second resolution, and uses <5% CPU. No configuration needed for basic CPU/memory/disk/network. For 8 gaming PCs under load, per-second GPU and CPU metrics during sessions would reveal hardware degradation before it causes game crashes. | MEDIUM | Netdata MSI installer supports silent deploy (`msiexec /quiet`). Deployable via rc-agent :8090 exec. Each pod reports to a Netdata parent on James's machine or the server. No cloud account required — cloud is optional. Estimated: 2–4 hours to deploy and wire up. |
| Netdata on racecontrol server | Server health (RAM usage on 64GB machine, network throughput, disk I/O for SQLite) gives early warning of resource exhaustion. SQLite write performance degrades before it fails — disk I/O metrics catch this. | LOW | Same Netdata agent, same silent deploy. Lower complexity since the server has an easier deployment path than pods. |
| Prometheus `/metrics` endpoint in racecontrol | Standard Prometheus exposition format from racecontrol would allow scraping by any compatible tool (Grafana, Netdata, Prometheus itself). Expose: active sessions count, billing total, WS connected pods count, alert send count, last deploy timestamp. | MEDIUM | `prometheus` crate or manual text format. 8–10 application-level gauges/counters. Does not require running a full Prometheus server — expose the endpoint, let whatever scraper the operator chooses consume it. |
| WhatsApp/SMS alerting for Uday | Email alerts require Uday to check email. For a single-operator venue where Uday may be on-site or nearby, a WhatsApp message for critical failures (all pods offline, billing system down) would be actionable in seconds. | MEDIUM | Racing Point already has `racingpoint-whatsapp-bot`. Route ALERT-01 equivalent through WhatsApp bot for P0 severity events (all pods disconnected, billing failure). Email stays for P1/P2. Avoids adding Twilio or new dependencies. |
| Fleet uptime report (weekly email to Uday) | A weekly summary of: total sessions, average session duration, pod uptime %, revenue (in credits), any incidents — gives Uday a business health view without logging into the dashboard. | MEDIUM | Rust scheduled task (tokio timer, runs Sunday midnight IST). Aggregates from existing `sessions`, `billing`, and `laps` tables. Sends via existing `send_email.js`. No new tools needed — pure application logic in racecontrol. |
| Grafana + Prometheus full stack | The "proper" monitoring stack: Prometheus scrapes racecontrol + Netdata, Grafana visualizes dashboards. Would provide rich visualization, alerting rules, and historical trend analysis. | HIGH | For 8 pods and 1 operator, the Grafana stack (Grafana + Prometheus + Loki for logs) is 3 services to maintain. Overhead outweighs benefit at this scale. Netdata alone provides equivalent real-time visibility with zero query language and auto-discovered dashboards. Use Grafana only if Uday wants to build custom dashboards for customer-facing use. |

#### Anti-Features — Monitoring and Alerting

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full ELK/OpenSearch stack | "Enterprise-grade log search and analytics" | Elasticsearch requires 4–8GB RAM minimum. The racecontrol server has 64GB, but ELK for 8 pods' logs is significant operational overhead for a single operator. Logstash + Kibana adds complexity with no proportional benefit at this scale. | Structured JSON logs in files + Netdata for metrics. If log search becomes needed, use `grep` + `jq` on JSON log files. For a venue with 8 pods, `jq` on daily log files is more practical than ELK. |
| OpenTelemetry distributed tracing | "Trace requests from kiosk through racecontrol to rc-agent" | Distributed tracing requires instrumenting all three services (kiosk Next.js, racecontrol Rust, rc-agent Rust) with OTel SDKs, running a collector, and a trace backend (Jaeger or Zipkin). For a local-LAN venue system without microservices at scale, the per-request latency being traced is not the problem. Billing and game launch failures are the problems — these are better addressed by structured error logging than distributed tracing. | Use `tracing` spans within each service for debugging. Structured JSON logs with correlation IDs (session_id, pod_id) provide equivalent traceability without a collector. |
| Real-time alerting on every error | "Notify Uday on every warning-level log event" | At busy times, 8 pods generate hundreds of debug/warning events per minute. Alert fatigue makes real alerts invisible. | Severity-tiered alerting: P0 (all pods offline, billing crash) → WhatsApp. P1 (single pod crash, WS drop) → email. P2 (warnings, recoverable errors) → weekly digest only. |
| Cloud monitoring (Datadog, New Relic) | "Production-grade monitoring platform" | Monthly cost for 8 hosts + application metrics would be $100–500/month. For a venue with 8 pods and 1 operator, this is unjustifiable when Netdata (free, self-hosted) covers the same need. Additionally, sending venue metrics to external cloud services exposes business data (session counts, revenue trends). | Netdata self-hosted (free) + structured logs on-premises. All data stays on-site. |

---

## Feature Dependencies

```
[Structured JSON logging]
    required-by --> [Error rate alerting threshold]
    required-by --> [Structured log search via MCP (future)]
    enables --> [Log rotation and retention]
    foundation-for --> [All monitoring features]

[Project CLAUDE.md]
    required-by --> [All /rp:* skills]
    required-by --> [Session context injection hook]
    informs --> [/rp:incident skill (loads playbook context)]

[/rp:deploy skill]
    requires --> [Staging HTTP server auto-start]
    requires --> [Deploy verification script]
    uses --> [rc-agent :8090 exec endpoint (existing)]

[google_workspace_mcp install]
    required-by --> [Gmail read MCP]
    required-by --> [Google Sheets MCP]
    required-by --> [Google Calendar MCP]
    uses --> [racingpoint-google OAuth (existing)]

[rc-ops-mcp (custom)]
    requires --> [racecontrol API running :8080]
    requires --> [/fleet/health, /sessions, /billing endpoints (existing)]
    optional-enhances --> [/rp:fleet-health skill (replaces !curl with MCP call)]

[Netdata on pods]
    requires --> [rc-agent :8090 exec (to deploy MSI silently)]
    requires --> [Netdata parent on server or James's machine]
    feeds --> [Pod-level health metrics (CPU, RAM, disk)]

[Prometheus /metrics endpoint in racecontrol]
    requires --> [racecontrol running (existing)]
    optional-feeds --> [Grafana (if adopted)]
    optional-feeds --> [Netdata scraper (Netdata can scrape Prometheus endpoints)]

[WhatsApp alerting]
    requires --> [racingpoint-whatsapp-bot (existing)]
    requires --> [ALERT-01 severity classification (new logic in racecontrol)]

[Ansible fleet management]
    requires --> [WinRM or OpenSSH validation on one pod (BLOCKED)]
    required-by --> [Chocolatey dependency management]
    replaces --> [rc-agent :8090 HTTP exec (for deploy workflows)]
```

### Dependency Notes

- **Structured logging is the foundation.** All meaningful monitoring features depend on it. It has the lowest cost (one-line Rust config change) and the highest leverage. Build it first.
- **Google Workspace MCP reuses existing OAuth.** The `racingpoint-google` repo already has OAuth configured. MCP installation is `npm install` + pointing at existing token files. No new Google Cloud project needed.
- **Ansible is blocked until WinRM/SSH is validated.** Do not architect around Ansible until one pod successfully accepts a WinRM or OpenSSH connection from James's machine. The prior failure (MEMORY.md) makes this high-risk.
- **rc-ops-mcp (custom MCP server) is high-value but high-cost.** It requires building a new Node.js server and maintaining it alongside racecontrol API changes. Defer until Google Workspace MCP and skills are in place and the ROI pattern is validated.

---

## MVP Definition

v9.0 "Tooling & Automation" is useful when Uday spends less time waiting on James for ops tasks and James spends less time re-establishing context each Claude Code session.

### Launch With (v9.0 MVP — Phase 1)

High ROI, low effort. These can be completed in a single day's work.

- [ ] **Project CLAUDE.md** — Load Racing Point topology, pod IPs, naming conventions, constraints into project context. Eliminates context re-establishment per session.
- [ ] **`/rp:deploy` and `/rp:deploy-server` skills** — Codify the exact build + deploy sequence. No more "what was the deploy sequence again?" `disable-model-invocation: true` on both.
- [ ] **`/rp:pod-status` skill** — Query any pod's rc-agent status with dynamic IP injection. Single-line incident triage.
- [ ] **Google Workspace MCP (`google_workspace_mcp`)** — Install, configure OAuth, verify Gmail read + Sheets read work in Claude Code session.
- [ ] **Structured JSON logging in racecontrol** — Add `tracing-subscriber` JSON format + `tracing_appender` daily rotation. Foundation for all monitoring.
- [ ] **Staging HTTP server auto-start** — HKLM Run key for `webterm.py` + HTTP server. Prevents silent deploy failures after James's machine reboots.

### Add After Validation (v9.x — Phase 2)

Deploy and verify Phase 1 is used in daily ops, then add:

- [ ] **`/rp:incident` skill** — Structured incident response leveraging debugging playbook. Add after `/rp:pod-status` has proven useful for quick queries.
- [ ] **Netdata on racecontrol server** — System-level metrics on the server. Lower complexity than pods; validates Netdata setup before rolling to pods.
- [ ] **Netdata on all 8 pods** — Silent MSI deploy via rc-agent :8090. Add after server Netdata is verified.
- [ ] **Error rate alerting threshold** — In-racecontrol counter logic: N errors in M minutes triggers email. Add after JSON logging is in place and log patterns are understood.
- [ ] **`/rp:logbook` skill** — Append incidents to LOGBOOK automatically. Add after the core skills are tested and trusted.
- [ ] **PostToolUse hook for Bono comms-link** — Auto-notify Bono on git commit. Add after other hooks are validated and the pattern is stable.

### Future Consideration (v9.x+ — Phase 3)

- [ ] **Ansible fleet management** — Only pursue after WinRM/SSH is validated on one pod. Do not build until connectivity is confirmed.
- [ ] **rc-ops-mcp (custom MCP server)** — High-value, high-cost. Build after Google Workspace MCP validates the pattern.
- [ ] **WhatsApp alerting for P0 events** — Add after error rate thresholds prove reliable (avoid alert fatigue from false positives).
- [ ] **Weekly fleet uptime report** — Add after structured logging provides the data foundation. Low urgency.
- [ ] **Prometheus /metrics endpoint** — Add if Uday wants to build custom dashboards or Netdata's auto-dashboards are insufficient.

---

## Feature Prioritization Matrix

| Feature | Operator Value | Implementation Cost | Priority |
|---------|----------------|---------------------|----------|
| Project CLAUDE.md | HIGH — eliminates context re-establishment | LOW — authoring only | P1 |
| `/rp:deploy` + `/rp:deploy-server` skills | HIGH — codifies critical workflow | LOW — 2 SKILL.md files | P1 |
| Google Workspace MCP (Gmail + Sheets) | HIGH — unlocks business data access | LOW — npm install + OAuth | P1 |
| Structured JSON logging | HIGH — foundation for all monitoring | LOW — one-line Rust config | P1 |
| Staging HTTP server auto-start | HIGH — prevents silent deploy failures | LOW — HKLM Run key | P1 |
| `/rp:pod-status` skill | MEDIUM — useful for incidents | LOW — 15 lines of SKILL.md | P1 |
| Deploy verification script | HIGH — catches failed deploys before customer impact | LOW — shell script | P1 |
| Netdata on server | MEDIUM — server health visibility | LOW — MSI install | P2 |
| Netdata on pods | HIGH — hardware degradation detection | MEDIUM — fleet deploy | P2 |
| Error rate alerting threshold | HIGH — proactive failure detection | MEDIUM — Rust logic | P2 |
| `/rp:incident` skill | MEDIUM — structured incident response | MEDIUM — playbook authoring | P2 |
| PostToolUse Bono notification hook | MEDIUM — enforces standing rule automatically | MEDIUM — hook scripting | P2 |
| WhatsApp P0 alerting | MEDIUM — faster Uday notification | MEDIUM — whatsapp-bot routing | P3 |
| Ansible fleet management | HIGH (when it works) — fleet deploy automation | HIGH + BLOCKED | P3 |
| rc-ops-mcp custom server | HIGH — full venue state in Claude | HIGH — new service to maintain | P3 |
| Grafana + Prometheus stack | LOW — Netdata covers this at lower cost | HIGH — 3 services to run | Avoid |
| ELK/OpenSearch | LOW — `jq` on JSON files is sufficient | HIGH — 4GB+ RAM overhead | Avoid |

**Priority key:** P1 = v9.0 MVP Phase 1, P2 = Phase 2 after validation, P3 = Phase 3 future, Avoid = explicitly not recommended

---

## ROI Analysis for Single-Operator Venue (Uday)

This is a single-operator venue where Uday's time is the primary constraint. Features are ranked by time-saved-per-week:

| Feature | Estimated Weekly Time Saved | For Whom |
|---------|----------------------------|----------|
| Project CLAUDE.md + skills | 30–60 min (context overhead eliminated) | James |
| Google Workspace MCP | 20–40 min (no copy-paste from browser) | James |
| Structured logging + error rate alerts | 30–60 min (fewer blind investigations) | James + Uday |
| Netdata on pods | 60–120 min (hardware issues caught before crash) | James |
| Deploy verification script | 15–30 min (confident deploy completion) | James |
| Ansible fleet management | 60–90 min/deploy (eliminates pendrive) | James |
| Weekly uptime report | 20–30 min (Uday's manual reporting eliminated) | Uday |

**Highest ROI pair:** Project CLAUDE.md + Google Workspace MCP. Together they take < 4 hours to set up and immediately improve every Claude Code session. Start here.

---

## Sources

- [Claude Code Skills Official Docs](https://code.claude.com/docs/en/skills) — HIGH confidence (official docs, verified 2026-03-20)
- [Claude Code Hooks Official Docs](https://code.claude.com/docs/en/hooks-guide) — HIGH confidence (official docs, verified 2026-03-20)
- [google_workspace_mcp GitHub](https://github.com/taylorwilsdon/google_workspace_mcp) — MEDIUM confidence (community project, active as of 2026)
- [Google Cloud MCP Support Announcement](https://cloud.google.com/blog/products/ai-machine-learning/announcing-official-mcp-support-for-google-services) — HIGH confidence (official Google announcement)
- [Netdata Native Windows Agent](https://www.netdata.cloud/blog/netdata-native-windows-agent/) — HIGH confidence (official Netdata docs)
- [Ansible Windows Management Docs](https://docs.ansible.com/projects/ansible/latest/os_guide/intro_windows.html) — HIGH confidence (official Ansible docs)
- [tokio-rs/tracing-opentelemetry GitHub](https://github.com/tokio-rs/tracing-opentelemetry) — HIGH confidence (official tokio ecosystem)
- Claude Code Hooks merged into Skills (v2.1.3): [Medium article](https://medium.com/@joe.njenga/claude-code-merges-slash-commands-into-skills-dont-miss-your-update-8296f3989697) — MEDIUM confidence
- MEMORY.md (WinRM failure, pendrive workflow, existing infrastructure) — HIGH confidence (primary source)
- PROJECT.md v9.0 requirements — HIGH confidence (primary source)

---
*Feature research for: Tooling & Automation (v9.0) — Racing Point eSports venue operations*
*Researched: 2026-03-20*
