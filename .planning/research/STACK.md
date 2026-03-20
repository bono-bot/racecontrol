# Stack Research

**Domain:** Tooling & Automation — Claude Code skills, MCP servers, deployment automation, and monitoring/alerting for a sim racing venue management system (Rust/Axum + Next.js + 8 Windows 11 pods)
**Researched:** 2026-03-20 IST
**Confidence:** HIGH (Claude Code official docs fetched live; library versions verified via docs.rs and GitHub releases; Grafana/Prometheus versions from official release pages)

---

> **Milestone scope:** v9.0 Tooling & Automation Research ONLY.
> Existing validated stack (Rust/Axum, Next.js 16, SQLite, Playwright, rc-agent, Ollama, Tailscale,
> Windows 11 pods, HKLM Run keys) is NOT re-researched.
> Focus: what gets ADDED for Claude Code skills, MCP servers, deployment automation, and monitoring.

---

## Recommended Stack

### 1. Claude Code Skills (Authoring)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| SKILL.md format (Agent Skills standard) | current | Reusable workflows invoked via `/skill-name` or auto-triggered | Official Claude Code extension point. Zero dependencies — markdown files only. Replaces `.claude/commands/` (still works but skills are the forward path). Supports YAML frontmatter for invocation control, argument injection, and subagent forking. |
| Project-scoped skills | n/a | Racing Point-specific workflows (deploy, pod-fix, billing-check) | Store in `.claude/skills/<name>/SKILL.md`. Checked into git. Available to every session in the repo. Shared with future team members without extra setup. |
| `disable-model-invocation: true` frontmatter | n/a | Prevent Claude from auto-triggering side-effect skills | Deployment and exec skills (deploy pod binary, restart rc-agent) MUST use this flag. Claude should not autonomously run deploys based on conversation context — only user-triggered via `/deploy`. |
| `context: fork` + `agent: Explore` | n/a | Run research/audit tasks in isolated subagent | Use for diagnostics that read many files without polluting the main conversation context. `/pod-audit`, `/check-billing-gaps` patterns. |

**Key skill patterns for Racing Point:**

- `/deploy` — build binary, deploy to Pod 8 canary, verify, roll to fleet. Use `disable-model-invocation: true`.
- `/pod-fix <pod_id>` — query pod health, attempt deterministic fix sequence (stale socket, game kill, restart rc-agent). Arguments via `$ARGUMENTS[0]`.
- `/billing-check` — summarize active sessions, unpaid credits, sync lag from cloud. Auto-triggered (no `disable-model-invocation`).
- `/release-notes` — query git log, format changelog. Safe to auto-trigger.

**Where skills live:**

```
racecontrol/
  .claude/
    skills/
      deploy/
        SKILL.md          # /deploy workflow with disable-model-invocation: true
        checklist.md      # Step-by-step deploy gates referenced from SKILL.md
      pod-fix/
        SKILL.md          # /pod-fix $ARGUMENTS[0] — deterministic heal sequence
      billing-check/
        SKILL.md          # auto-triggered billing summary
```

---

### 2. MCP Servers

#### 2a. Google Workspace MCP

| Technology | Version/Source | Purpose | Why Recommended |
|------------|----------------|---------|-----------------|
| `google_workspace_mcp` (taylorwilsdon) | latest (GitHub, active 2026) | Gmail read/send, Calendar, Drive, Sheets from Claude Code | Most feature-complete community implementation: 100+ tools, Gmail + Calendar + Drive + Sheets + Docs + Chat + Tasks. Single OAuth 2.0 credential setup. Active maintenance (287+ issues/PRs as of March 2026). Fixes the expired `mcp-gmail` OAuth that's currently broken. |

**Installation for Claude Code (not Claude Desktop):**

```bash
# Clone and run as HTTP MCP server
git clone https://github.com/taylorwilsdon/google_workspace_mcp
cd google_workspace_mcp
pip install -r requirements.txt

# Set credentials
export GOOGLE_OAUTH_CLIENT_ID=<from Google Cloud Console>
export GOOGLE_OAUTH_CLIENT_SECRET=<from Google Cloud Console>

# Run with HTTP transport for Claude Code
python server.py --transport streamable-http --port 8200
```

Add to `.claude/settings.json` (project-level MCP config):

```json
{
  "mcpServers": {
    "google-workspace": {
      "type": "http",
      "url": "http://localhost:8200/mcp"
    }
  }
}
```

**What to use it for at Racing Point:**
- Send booking confirmations via Gmail (replaces the broken `send_email.js` OAuth flow)
- Read calendar to check venue booking schedule from Claude
- Write session summaries to Google Sheets for Uday's reporting
- Re-authorizing fixes the current `mcp-gmail` "No access, refresh token" error

**Authentication note:** Desktop OAuth type (no redirect URI needed). One-time browser auth flow, then token is cached. Uday must authorize once from his Google account.

#### 2b. Prometheus MCP

| Technology | Version/Source | Purpose | Why Recommended |
|------------|----------------|---------|-----------------|
| `prometheus-mcp-server` (pab1it0) | latest (GitHub) | Query Prometheus metrics from Claude — ask "which pods have high CPU?" | 18 read-only MCP tools wrapping the Prometheus HTTP API: instant/range PromQL queries, metric/label discovery, target status, alert rules. Read-only makes it safe to expose. Claude can generate PromQL for you. |

**Use case:** Claude writes the PromQL query. You approve. It runs against local Prometheus. Surfaces pod CPU spikes, game crash correlations, billing anomalies directly in conversation.

```bash
pip install prometheus-mcp-server  # or npx -based install per repo instructions

# Point to local Prometheus
export PROMETHEUS_URL=http://192.168.31.23:9090
```

**When to add:** Only after Prometheus is deployed on the server (Phase 2 of v9.0 implementation). This MCP has no value without a running Prometheus instance.

---

### 3. Rust Instrumentation (axum-prometheus)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `axum-prometheus` | **0.10.0** | HTTP metrics middleware for racecontrol Axum server | Exposes `axum_http_requests_total`, `axum_http_requests_duration_seconds`, `axum_http_requests_pending` at `/metrics` endpoint in Prometheus text format. Built on `metrics.rs` + `metrics_exporter_prometheus` — same ecosystem the Rust community standardized on. 3-line integration. Does NOT require changing any existing route logic. |
| `metrics` | **0.24.x** (pulled by axum-prometheus) | Application-level custom metrics (billing counts, pod health, game launches) | Use `metrics::counter!`, `metrics::gauge!`, `metrics::histogram!` macros in racecontrol business logic. Example: `metrics::gauge!("rp_active_sessions", active_count as f64)`. Zero unsafe code, no C FFI. |

**Cargo.toml additions:**

```toml
[dependencies]
axum-prometheus = "0.10.0"
metrics = "0.24"
# metrics_exporter_prometheus pulled transitively by axum-prometheus
```

**Custom metrics to add in racecontrol:**

| Metric | Type | Where to instrument |
|--------|------|---------------------|
| `rp_active_sessions` | Gauge | billing.rs — updated on session start/stop |
| `rp_pod_websocket_connected` | Gauge (per pod) | ws handler — 1 when connected, 0 on disconnect |
| `rp_game_launches_total` | Counter | game_manager.rs — increment on each launch |
| `rp_game_launch_failures_total` | Counter | game_manager.rs — increment on timeout/error |
| `rp_billing_credits_collected_total` | Counter | billing.rs — increment on session end |
| `rp_cloud_sync_lag_seconds` | Histogram | cloud_sync.rs — time between push and ack |

**Integration in main.rs:**

```rust
use axum_prometheus::PrometheusMetricLayer;

let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
let app = Router::new()
    .route("/metrics", get(|| async move { metric_handle.render() }))
    // ... existing routes ...
    .layer(prometheus_layer);
```

---

### 4. Metrics Collection — Grafana Alloy on Windows

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Grafana Alloy | latest stable (v1.x) | Scrape racecontrol `/metrics` + Windows OS metrics on server and pods | Grafana Agent is EOL (November 2025). Alloy is the replacement — OTel-native, Windows MSI installer available, runs as Windows service. Embeds `prometheus.exporter.windows` component (wraps windows_exporter v0.31.4) so you don't need to install windows_exporter separately. Scrapes and forwards to Prometheus or Grafana Cloud. |
| `prometheus.exporter.windows` (embedded in Alloy) | v0.31.4 | Windows CPU, RAM, disk, network, process metrics from all pods | Embedded in Alloy — no separate binary needed. Exposes cpu, memory, net, logical_disk, process collectors. On pods: enables monitoring of rc-agent process specifically (is it running? memory usage?). |

**Install on server (.23) and each pod:**

```powershell
# Download MSI installer
winget install GrafanaLabs.Alloy
# OR: download alloy-installer-windows-amd64.exe from Grafana releases page

# Alloy config at C:\Program Files\GrafanaLabs\Alloy\config.alloy
```

**Alloy config for pods (scrape Windows metrics + racecontrol):**

```hcl
// On server (.23) — also scrapes racecontrol /metrics
prometheus.exporter.windows "local" {
  enabled_collectors = ["cpu", "memory", "net", "logical_disk", "process"]
}

prometheus.scrape "racecontrol_app" {
  targets = [{ __address__ = "localhost:8080" }]
  metrics_path = "/metrics"
  forward_to = [prometheus.remote_write.grafana_cloud.receiver]
}

prometheus.scrape "windows_self" {
  targets = prometheus.exporter.windows.local.targets
  forward_to = [prometheus.remote_write.grafana_cloud.receiver]
}

prometheus.remote_write "grafana_cloud" {
  endpoint {
    url = env("GRAFANA_CLOUD_METRICS_URL")
    basic_auth {
      username = env("GRAFANA_CLOUD_USER")
      password = env("GRAFANA_CLOUD_API_KEY")
    }
  }
}
```

**On pods:** Alloy only scrapes Windows metrics (no racecontrol endpoint on pods). Lightweight config.

---

### 5. Metrics Storage and Dashboards

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Grafana Cloud Free | n/a (SaaS) | Store metrics + host dashboards | Free tier: 10K active metric series, 14-day retention, 3 users. Racing Point has ~8 pods × ~20 metrics each = 160 series — well within free tier. Zero infra to maintain. Alternative: self-hosted Prometheus + Grafana on server .23 is viable but adds 2-3 services to manage on a machine that's already running racecontrol + kiosk. |
| Grafana dashboards (pre-built) | n/a | Windows fleet overview + app metrics | Import dashboard ID `24390` (Windows Exporter 2025) for OS metrics. Create custom Racing Point dashboard for `rp_*` app metrics. |

**Self-hosted alternative (if Grafana Cloud privacy is a concern):**

Install Prometheus on server .23 (single binary, no Docker needed):
- Prometheus 3.x scrapes Alloy remote_write endpoint (or Alloy scrapes and writes to local Prometheus)
- Grafana OSS 11.x on port 3000 for dashboards
- Adds ~300MB RAM usage to the server

Recommendation: Start with Grafana Cloud free. The 14-day retention is sufficient for operational monitoring (not long-term analytics). Switch to self-hosted only if Uday wants data sovereignty.

---

### 6. Deployment Automation (Replace Pendrive Workflow)

#### 6a. Tailscale + OpenSSH (prerequisite)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| OpenSSH Server (Windows built-in) | Windows 11 built-in (OpenSSH 9.x) | SSH access to pods via Tailscale mesh for binary deploy | Tailscale SSH server does NOT support Windows (confirmed GitHub issue #14942, open as of 2026). Alternative: enable Windows' native OpenSSH Server (`Add-WindowsCapability`) + Tailscale mesh VPN for routing. OpenSSH on pods = `scp` binary + `ssh cmd` remote execution. No WinRM, no Salt, no third-party tools. Note: MEMORY.md says `Add-WindowsCapability` failed on the server due to corrupted component store — pods may be different. Must verify per-pod. |

**Enable on each pod (once, via rc-agent remote_ops exec while rc-agent is running):**

```powershell
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Set-Service sshd -StartupType Automatic
Start-Service sshd
# Allow key auth, add James's key to C:\Users\ADMIN\.ssh\authorized_keys
```

#### 6b. Fabric (Python deployment tool)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `fabric` | **3.2.x** | Run deployment tasks on pods over SSH from James's machine | Fabric is the simplest SSH-based remote execution tool. Single Python script, no agent, no daemon, no YAML DSL. `Connection(host).run("cmd")` and `Connection(host).put(local, remote)` are the entire API. Replaces the pendrive workflow: `fab deploy --pod 8` builds the binary, scp's it to the pod, kills the old process, starts the new one. Works from WSL or Git Bash on James's Windows machine. Ansible is overkill for 8 pods with no config management needs — Fabric is the right size. |

```bash
pip install fabric
```

**Fabric deploy script pattern (`fabfile.py`):**

```python
from fabric import Connection, task

PODS = {
    1: "100.x.x.x",  # Tailscale IPs once SSH is enabled
    2: "100.x.x.x",
    # ...
    8: "100.x.x.x",
}

@task
def deploy(c, pod=8):
    """Deploy rc-agent.exe to a pod. Default: pod 8 (canary)."""
    pod = int(pod)
    host = PODS[pod]
    conn = Connection(host, user="ADMIN", connect_kwargs={"key_filename": "~/.ssh/id_ed25519"})

    # Kill old binary
    conn.run("taskkill /IM rc-agent.exe /F", warn=True)

    # Upload new binary
    conn.put("deploy-staging/rc-agent.exe", "C:/RacingPoint/rc-agent.exe")

    # Start new binary (Session 1 via scheduled task trigger or HKLM Run on next login)
    conn.run(r'reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" '
             r'/v RCAgent /t REG_SZ '
             r'/d "C:\RacingPoint\start-rcagent.bat" /f')

    print(f"Pod {pod} deployed. Verify: curl http://{host}:8090/health")
```

**Fallback (if OpenSSH install fails on pods):** Continue using rc-agent remote_ops (:8090) for exec commands, and the existing HTTP server + curl pipeline for binary delivery. The Fabric path is the upgrade target, not a hard requirement.

---

### 7. Alerting

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Grafana Cloud Alerting | free tier | Alert on pod disconnects, billing failures, high CPU | Already included in Grafana Cloud free tier. Alert rules are PromQL expressions evaluated against stored metrics. `rp_pod_websocket_connected == 0` for 5 minutes triggers an alert. Integrates with Gmail (or PagerDuty, but Gmail is free). No additional install — configure in Grafana UI. |
| Gmail alert channel | existing | Deliver alerts to Uday and James via email | Reuse existing Gmail OAuth setup. Grafana Cloud supports SMTP-based email alerts. Configure with Racing Point Gmail credentials. |

**Priority alert rules to create:**

| Alert | PromQL | Severity |
|-------|--------|----------|
| Pod WS disconnected | `rp_pod_websocket_connected == 0` for 5m | Warning |
| Pod offline (no Alloy scrape) | `up{job="pod_windows"} == 0` for 2m | Critical |
| Game launch failure spike | `rate(rp_game_launch_failures_total[5m]) > 0.1` | Warning |
| Active sessions with no telemetry | `rp_active_sessions > 0` and `rate(rp_game_launches_total[10m]) == 0` | Info |
| Server CPU high | `windows_cpu_time_total{mode="idle"} < 20` | Warning |

---

## Installation Summary

### Phase 1 — Claude Code Skills (no new installs)

```bash
# Create skill directories in the repo
mkdir -p .claude/skills/deploy
mkdir -p .claude/skills/pod-fix
mkdir -p .claude/skills/billing-check

# Write SKILL.md files (no npm/cargo installs)
```

### Phase 2 — Rust Instrumentation

```bash
# Add to Cargo.toml [dependencies]:
# axum-prometheus = "0.10.0"
# metrics = "0.24"

export PATH="$PATH:/c/Users/bono/.cargo/bin"
cargo build --release -p racecontrol
```

### Phase 3 — Metrics Stack

```bash
# Install Grafana Alloy on server (.23) via winget (run on server as ADMIN)
winget install GrafanaLabs.Alloy

# Sign up for Grafana Cloud free tier
# https://grafana.com/auth/sign-up/create-user

# Configure Alloy config.alloy with credentials
# Set GRAFANA_CLOUD_METRICS_URL, GRAFANA_CLOUD_USER, GRAFANA_CLOUD_API_KEY

# Install Alloy on pods (later, for OS metrics)
# Deploy via existing pendrive workflow or rc-agent remote_ops exec
```

### Phase 4 — Google Workspace MCP

```bash
# On James's machine
git clone https://github.com/taylorwilsdon/google_workspace_mcp
cd google_workspace_mcp
pip install -r requirements.txt

# Configure Google Cloud OAuth credentials (see Google Cloud Console)
# Add HTTP MCP server to .claude/settings.json
```

### Phase 5 — Deployment Automation (gated on OpenSSH success)

```bash
# On James's machine
pip install fabric

# Create fabfile.py in racecontrol repo
# Enable OpenSSH on pods one at a time via rc-agent remote_ops exec
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Grafana Cloud free tier | Self-hosted Prometheus + Grafana on server .23 | If Uday requires data never leaves local network. Server .23 has 64GB RAM — it can handle it. Add ~300MB RAM overhead. |
| Fabric (Python SSH tasks) | Ansible | If the pod count grows to 20+ or config drift becomes a problem. Ansible's declarative model shines at scale. At 8 pods with a homogeneous config, Fabric's imperative Python is simpler to debug and faster to write. |
| Fabric (Python SSH tasks) | SaltStack | Salt was evaluated and blocked (WSL2 portproxy issue, v6.0 paused). Do not revisit until BIOS AMD-V is fixed on the server. |
| `axum-prometheus` | OpenTelemetry SDK for Rust (`opentelemetry` crate) | If you need distributed traces across racecontrol → rc-agent → cloud. OTel traces are a v10+ concern. For now, metrics only. axum-prometheus is 3-line setup vs OTel's 50-line configuration. |
| `google_workspace_mcp` (taylorwilsdon) | `ngs/google-mcp-server` | Either works. taylorwilsdon has more GitHub stars and active issue tracker. ngs/google-mcp-server is TypeScript-based if Python dependency is a concern. |
| Grafana Alloy (embedded windows_exporter) | windows_exporter v0.31.4 standalone + Prometheus scrape | Use standalone windows_exporter if you self-host Prometheus on .23 and don't want Alloy. Standalone is simpler for pure Prometheus setup. Alloy is better if you use Grafana Cloud (it handles remote_write). |
| OpenSSH native (Windows built-in) | WinRM + pywinrm | WinRM failed previously on this network. OpenSSH uses standard port 22 + SSH keys — same tooling as Linux. pywinrm requires NTLM/Kerberos config. Not worth it. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `@google/model-context-protocol-workspace` (Google's official CLI MCP) | Requires Google Workspace Business tier. Racing Point uses personal Gmail/Google Workspace — the official enterprise CLI is not designed for small business OAuth flows. | `taylorwilsdon/google_workspace_mcp` — works with standard OAuth desktop credentials |
| SaltStack (v6.0 approach) | BLOCKED — WSL2 portproxy could not reliably forward HTTP to salt-api. ZMQ ports showed LISTENING but minion installer blocked by Windows Defender on pods. Re-investigate only after BIOS AMD-V is fixed and WSL2 is stable. | Fabric over SSH (simpler, no daemon) |
| Ansible for this fleet | Massive overkill for 8 homogeneous Windows pods. Ansible's Windows support requires WinRM (which failed previously) or experimental SSH. Adds Python + Ansible control node complexity with no DX advantage at this scale. | Fabric — Python SSH tasks, 50 lines vs Ansible's 200+ lines of YAML playbooks |
| Prometheus Agent Mode (standalone) | Prometheus Agent is a component of Prometheus server — requires running the full Prometheus binary in agent mode. More complexity than Alloy for the same job. Alloy supersedes it. | Grafana Alloy |
| Docker / containers on pods | Gaming pods are bare-metal Windows 11 for performance reasons. Container overhead on a gaming rig degrades FFB latency and frame pacing. Never containerize anything on the pods. | Native Windows services / HKLM Run keys (existing pattern) |
| `tracing-opentelemetry` crate for pods | rc-agent is a lean process; OTel instrumentation adds ~5MB to binary size and non-trivial startup overhead. The LLM-based self-diagnostics (qwen3:0.6b) already do root-cause analysis — OTel traces don't add enough value to justify the complexity at this stage. | `tracing` + `tracing-subscriber` already in rc-agent for local structured logs |
| Grafana Loki for log aggregation | Log shipping adds network overhead and Loki configuration complexity. Racing Point's logs are already structured via `tracing` — searchable locally with `grep` or the Claude Code `/pod-fix` skill. Add Loki only if log volume makes local search unmanageable. | Local `tracing` subscriber writing JSON logs to disk |
| Headless Grafana dashboards without first instrumenting the app | Deploying Grafana with only Windows OS metrics gives you CPU/RAM graphs with no correlation to Racing Point events. The `rp_*` custom Axum metrics are what make the dashboard useful. Instrument the app first. | Instrument racecontrol with axum-prometheus (Phase 2) before deploying Grafana (Phase 3) |

---

## Integration with Existing Stack

| Existing Component | Integration Point | Notes |
|--------------------|-------------------|-------|
| racecontrol Axum (:8080) | Add `axum-prometheus` middleware → exposes `/metrics` | Zero breaking changes. Add route and layer. Alloy scrapes `:8080/metrics`. |
| rc-agent on pods | Alloy on pods scrapes Windows process metrics for `rc-agent.exe` | No code change to rc-agent. Monitor via `windows_process` collector filtering by image name. |
| send_email.js Gmail auth | Replace with `google_workspace_mcp` Gmail tool | Fixes the expired OAuth. MCP handles token refresh. Keep `send_email.js` as fallback until MCP is stable. |
| HKLM Run key deploys | Fabric wraps the same commands in Python over SSH | Same deploy logic as `install.bat v5` — kill, copy binary, set Run key. Just automated instead of manual. |
| rc-agent remote_ops (:8090) | Keep as fallback exec channel | Fabric SSH is the primary deploy path. remote_ops stays as emergency exec when SSH is unavailable (e.g. OpenSSH not yet installed on a pod). |
| Tailscale mesh (pods + POS + spectator) | Route SSH traffic to pods via Tailscale IPs | Tailscale does NOT support SSH server on Windows — use it for routing to native OpenSSH Server. Tailscale provides the network; OpenSSH provides the shell access. |
| Ollama + qwen3:0.6b on pods | No change | LLM self-diagnostics stay. Prometheus metrics add a second layer of observability. They complement each other: Prometheus shows WHAT, LLM explains WHY. |
| Cloud sync (cloud_sync.rs) | Add `rp_cloud_sync_lag_seconds` histogram | Instrument `sync_push()` with metrics for push duration and success/failure counters. |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `axum-prometheus@0.10.0` | axum 0.7.x, tokio 1.x, Rust 1.80+ | racecontrol uses axum 0.7 — compatible. Check axum version in `Cargo.toml` before adding. |
| `metrics@0.24.x` | Pulled transitively by axum-prometheus | Pin to same minor series as axum-prometheus expects. Run `cargo tree | grep metrics` to verify no duplicate versions. |
| Grafana Alloy (latest) | Windows 10/11, Windows Server 2016+ | All pods are Windows 11. Compatible. Alloy runs as Windows service. |
| `windows_exporter@0.31.4` (embedded in Alloy) | Windows 10/11, 2019+ | Embedded in Alloy — no separate installation. Validates on startup; fails on unknown config keys. |
| `fabric@3.2.x` | Python 3.8+, OpenSSH client on James's machine | Python already on James's machine (used by webterm.py). OpenSSH client ships with Windows 11. |
| `google_workspace_mcp` | Python 3.10+, Claude Code (any version) | HTTP transport mode for Claude Code. Requires `OAUTHLIB_INSECURE_TRANSPORT=1` for local dev. |
| `prometheus-mcp-server` | Python 3.8+, Prometheus HTTP API v1 | Standard Prometheus API — compatible with any Prometheus 2.x/3.x or Grafana Cloud Prometheus endpoint. |

---

## Sources

- [Claude Code Skills official docs](https://code.claude.com/docs/en/skills) — SKILL.md format, frontmatter fields, invocation control, subagent forking — HIGH confidence (official Anthropic docs, fetched live)
- [Claude Code Extend Features overview](https://code.claude.com/docs/en/features-overview) — Skills vs MCP vs Hooks vs Subagents comparison table, context costs — HIGH confidence (official docs, fetched live)
- [axum-prometheus docs.rs](https://docs.rs/axum-prometheus) — version 0.10.0 current, default metrics, integration code — HIGH confidence (official crate registry, fetched live)
- [windows_exporter GitHub releases](https://github.com/prometheus-community/windows_exporter/releases) — v0.31.4 latest stable (March 20 2025) — HIGH confidence (official GitHub, fetched live)
- [Grafana Alloy Windows docs](https://grafana.com/docs/alloy/latest/set-up/install/windows/) — MSI installer, `prometheus.exporter.windows` component — HIGH confidence (official Grafana docs)
- [Grafana Cloud pricing/free tier](https://grafana.com/pricing/) — 10K series, 14-day retention, 3 users free — HIGH confidence (official Grafana pricing page)
- [taylorwilsdon/google_workspace_mcp GitHub](https://github.com/taylorwilsdon/google_workspace_mcp) — capabilities, Claude Code HTTP transport setup, auth requirements — MEDIUM-HIGH confidence (GitHub README, active project, 287+ issues)
- [pab1it0/prometheus-mcp-server GitHub](https://github.com/pab1it0/prometheus-mcp-server) — 18 read-only Prometheus tools, PromQL query support — MEDIUM confidence (GitHub repo, community project)
- [Tailscale SSH GitHub issue #14942](https://github.com/tailscale/tailscale/issues/14942) — Tailscale SSH server NOT supported on Windows (confirmed open issue) — HIGH confidence (official Tailscale GitHub)
- [WebSearch: Grafana Agent EOL announcement](https://grafana.com/blog/2024/04/09/grafana-agent-to-grafana-alloy-opentelemetry-collector-faq/) — Grafana Agent EOL November 2025, Alloy is successor — HIGH confidence (official Grafana blog)
- [WebSearch: Fabric deployment tool](https://www.fabfile.org/) — Python SSH task runner, v3.2.x current — MEDIUM confidence (official docs + community verification)
- [WebSearch: Claude Code skills merged from slash commands](https://medium.com/@joe.njenga/claude-code-merges-slash-commands-into-skills-dont-miss-your-update-8296f3989697) — backward-compat confirmed — MEDIUM confidence (community source, consistent with official docs)

---

*Stack research for: v9.0 Tooling & Automation — Claude Code skills, MCP servers, deployment automation, monitoring/alerting for Racing Point eSports*
*Researched: 2026-03-20 IST*
