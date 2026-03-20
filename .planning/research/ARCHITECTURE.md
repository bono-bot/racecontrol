# Architecture Research

**Domain:** Tooling, automation, and monitoring integration for a Rust/Axum + Next.js sim racing venue management system
**Researched:** 2026-03-20 IST
**Confidence:** HIGH (Claude Code official docs verified, existing codebase read directly, Prometheus/windows_exporter official docs)

---

> **Milestone scope:** This file covers v9.0 Tooling & Automation ONLY — Claude Code skills, MCP servers,
> deployment automation, and monitoring/alerting integrations with the existing racecontrol architecture.
> Existing stack (Rust/Axum, SQLite, rc-agent, WebSocket, rc-sentry) is not re-researched.
> Focus: what connects where, what is new, what is modified, and the deployment topology.

---

## Standard Architecture

### Existing System Topology (Current State)

```
James Workstation (.27)          Server (.23)                 Pods (.89/.33/.28 etc.)
+-------------------------+     +---------------------------+  +---------------------+
|  Claude Code            |     |  racecontrol :8080         |  |  rc-agent :8090     |
|  ~/.claude/settings.json|     |  kiosk :3300               |  |  rc-sentry :8091    |
|  ~/.claude/skills/      |     |  admin :3200               |  |  rc-watchdog svc    |
|  racecontrol repo       |     |  SQLite racecontrol.db     |  |  Ollama qwen3:0.6b  |
|                         |     |  rc-sentry :8091           |  |                     |
|  deploy-staging/ :9998  +---> |  bono_relay :8099          |  |  WebSocket → :8080  |
|  webterm.py :9999       |     |  cloud_sync → VPS          |  |                     |
+-------------------------+     +---------------------------+  +---------------------+
         |                               |                              |
         |                       Tailscale mesh                         |
         +---------------------+ 100.x.x.x +---------------------------+
                                       |
                               Bono VPS :8080
                               app.racingpoint.cloud
```

### Proposed v9.0 Additions (Where Each Tool Lives)

```
James Workstation (.27)                  Server (.23)
+-------------------------------------+  +----------------------------------+
| Claude Code (extended)              |  | racecontrol :8080                |
|                                     |  |   + /hooks/pre-tool-use endpoint  |
| .claude/skills/                     |  |   (optional: HTTP hook target)    |
|   deploy-pod/SKILL.md         NEW   |  |                                  |
|   fleet-status/SKILL.md       NEW   |  | windows_exporter :9182    NEW    |
|   pod-logs/SKILL.md           NEW   |  |   exposes CPU/RAM/disk metrics   |
|   pod-heal/SKILL.md           NEW   |  |                                  |
|   racecontrol-context/SKILL.md NEW  |  | Prometheus :9090          NEW    |
|                                     |  |   scrapes .23:9182 + pods:9182  |
| ~/.claude/settings.json             |  |                                  |
|   hooks:                            |  | Grafana :3000             NEW    |
|     PreToolUse: deploy guard  NEW   |  |   dashboards for fleet health    |
|     PostToolUse: log actions  NEW   |  |                                  |
|                                     |  +----------------------------------+
| ~/.claude/settings.json             |
|   mcpServers:                       |       Pods (.89/.33/.28 etc.)
|     racingpoint-gmail (EXISTS)      |  +----------------------------------+
|     racingpoint-drive (EXISTS)      |  | windows_exporter :9182    NEW    |
|     context7 (add)           NEW    |  |   exposes CPU/RAM/GPU metrics    |
|                                     |  +----------------------------------+
| deploy/ scripts                     |
|   deploy_pod.py (EXISTS)            |
|   ansible/ playbooks         NEW    |
|   ansible/inventory.ini      NEW    |
+-------------------------------------+
```

---

## Component Responsibilities

| Component | Location | Responsibility | New / Existing |
|-----------|----------|----------------|----------------|
| Claude Code skills | James ~/.claude/skills/ + .claude/skills/ | Project-scoped and personal automation macros invoked by James as `/skill-name` | NEW |
| Claude Code hooks | ~/.claude/settings.json | Pre/post-tool guards — block unsafe deploys, log actions, auto-notify Bono | NEW |
| MCP racingpoint-gmail | James ~/.claude/settings.json | Gmail read/send via James's OAuth | EXISTS (broken OAuth) |
| MCP racingpoint-drive | James ~/.claude/settings.json | Google Drive read/write | EXISTS |
| MCP context7 | James ~/.claude/settings.json | Library documentation lookups during coding | NEW |
| windows_exporter | Each Windows host (server + pods) | Expose CPU, RAM, disk, network metrics on :9182 for Prometheus scraping | NEW |
| Prometheus | Server (.23) | Time-series metrics store, scrapes all hosts every 30s | NEW |
| Grafana | Server (.23) | Dashboard — fleet health, billing activity, pod uptime | NEW |
| Ansible control | James (.27) WSL2 or Git Bash | Fleet config push via WinRM — deploy binaries, configs, registry keys | NEW |
| deploy_pod.py | James (.27) | Existing per-pod deploy orchestrator — extend, don't replace | EXISTS |
| racecontrol AppState | Server (.23) | Fleet health already exposed via /api/v1/fleet/health — Grafana reads this | EXISTS |

---

## Integration Points with Existing Architecture

### 1. Claude Code Skills — Integration with racecontrol

Skills are Markdown files in `.claude/skills/` (project-scoped) that James invokes as slash commands. They do NOT modify any Rust code. They call existing infrastructure via Bash tool calls.

**Integration pathway:** Skill → Bash tool → existing CLI (deploy_pod.py / curl / rc-sentry :8091)

```
/deploy-pod pod-8
    |
    v
.claude/skills/deploy-pod/SKILL.md
    |
    Claude reads skill instructions
    |
    Bash: python deploy/deploy_pod.py 8
         |
         v
    deploy_pod.py → rc-sentry :8091/exec OR rc-agent :8090
         |
         v
    Pod 8 binary swapped, rc-agent restarted
```

**Which existing modules skills touch:**
- `deploy/deploy_pod.py` — called directly via Bash; no modification needed
- `rc-sentry :8091` — called via curl in deploy scripts; no modification needed
- `racecontrol /api/v1/fleet/health` — queried for status; no modification needed

**New project skills to create** in `.claude/skills/` (project-scoped, committed to repo):

| Skill | Purpose | Invocation |
|-------|---------|------------|
| `deploy-pod` | Deploy rc-agent to one or all pods via deploy_pod.py | `/deploy-pod [pod-N\|all]` |
| `fleet-status` | Query /api/v1/fleet/health and summarize pod states | `/fleet-status` |
| `pod-logs` | Fetch recent rc-agent logs from a pod via rc-sentry :8091/exec | `/pod-logs pod-N` |
| `pod-heal` | Force-restart rc-agent on a pod via rc-sentry :8091 or rc-agent :8090 | `/pod-heal pod-N` |
| `racecontrol-context` | Load architecture context for coding sessions | auto-invoked |

**New personal skills to create** in `~/.claude/skills/` (James only):

| Skill | Purpose |
|-------|---------|
| `rp-deploy-server` | Build + deploy racecontrol.exe to server (.23) |
| `rp-build-rc-agent` | Cross-compile rc-agent + stage at :9998 |

### 2. Claude Code Hooks — Integration Points

Hooks fire at Claude Code lifecycle events and can block or log actions. They run as Bash scripts receiving JSON on stdin.

**Two hooks to add to `~/.claude/settings.json`:**

**Hook A — PreToolUse deploy guard:**
Intercepts any Bash command containing `rc-agent.exe` or binary-swap keywords on James's machine (preventing accidental local execution of pod binaries).

```
PreToolUse (matcher: "Bash")
    |
    bash script reads tool_input.command via jq
    |
    if command matches rc-agent.exe execution pattern on .27 → deny + warn
    if command matches safe deploy pattern → allow
```

**Hook B — PostToolUse action logger:**
After any deploy Bash command succeeds, logs the action to a local file and optionally appends to comms-link INBOX.md for Bono.

```
PostToolUse (matcher: "Bash")
    |
    bash script reads tool_input.command + output
    |
    if command was deploy_pod.py → append to .planning/DEPLOY_LOG.md
```

**What hooks do NOT modify:** No changes to racecontrol, rc-agent, or rc-sentry. Hooks are purely Claude Code session-side.

### 3. MCP Servers — Integration Points

MCP servers expose tools to Claude Code via stdio. Two already exist; one needs repair and one is new.

**racingpoint-gmail (EXISTING — OAuth broken):**
- Location: `C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js`
- Issue: Google OAuth refresh token expired (logged in MEMORY.md as open issue)
- Fix: Re-authorize via Google OAuth flow; update `GOOGLE_REFRESH_TOKEN` in `~/.claude/settings.json`
- No architectural change needed — the server.js pattern is correct

**racingpoint-drive (EXISTING — working):**
- Location: `C:\Users\bono\racingpoint\racingpoint-mcp-drive\server.js`
- Status: Working; no changes needed

**context7 (NEW):**
- Purpose: Live library documentation lookups during coding sessions
- Configured in `~/.claude/settings.json` under `mcpServers`
- No integration with racecontrol code; purely a James workstation tool

**Integration with racecontrol code:** None. MCP servers are James-side tools that feed Claude context. They do not connect to racecontrol's API or any pod.

### 4. Monitoring Stack — Integration Points

The monitoring stack is purely additive. It reads existing data; no changes to racecontrol or rc-agent.

**Data sources already exposed:**
- `GET /api/v1/fleet/health` — pod WS status, rc-agent version, last heartbeat
- `GET /api/v1/pods` — pod states, active sessions, game status
- `GET /api/v1/billing/active` — active billing sessions
- rc-agent UDP heartbeat (interpreted by racecontrol)

**New data source: windows_exporter**
- Install on server (.23) and all 8 pods as a Windows service
- Exposes: CPU%, RAM MB used, disk GB free, network bytes/sec, process count
- Default port: 9182, path: /metrics (Prometheus text format)
- Installation: `windows_exporter-0.x-amd64.msi /quiet` via rc-sentry or pendrive

**Prometheus on server (.23):**
- Runs as Windows service on server
- Scrapes windows_exporter on all 9 hosts (server + 8 pods) every 30s
- Also scrapes racecontrol custom metrics endpoint (optional)

**Grafana on server (.23):**
- Connects to Prometheus as data source
- Dashboards: fleet health (pod online/offline), resource usage per pod, billing activity
- Not publicly exposed — LAN-only on :3000

**Data flow:**

```
Pod windows_exporter :9182 --+
Pod windows_exporter :9182 --+
Pod windows_exporter :9182 --+--> Prometheus (.23:9090) --> Grafana (.23:3000)
         ...                  |         |
Server windows_exporter :9182 +    /api/v1/fleet/health
                                  (polled by Grafana JSON plugin)
```

**What monitoring does NOT require changing:**
- No changes to racecontrol Rust code
- No changes to rc-agent
- No changes to SQLite schema
- No new API endpoints required (fleet/health already sufficient)

**Optional enhancement:** Add a Prometheus metrics endpoint to racecontrol (`GET /metrics`) using the `prometheus` crate. This would expose billing session counts, active pods, lap counts per hour. This is enhancement, not required for the monitoring stack to function.

### 5. Deployment Automation — Integration Points

The current pendrive-based deployment has two layers that already exist:
- `deploy/deploy_pod.py` — Python script that orchestrates per-pod deploys via rc-sentry :8091
- `deploy-staging/` on James with HTTP server :9998 for binary hosting
- `racecontrol /api/v1/pods/{pod_id}/exec` — fallback if rc-agent :8090 unreachable

**Ansible complement (new layer for config management):**
- Ansible control node: James workstation (.27) using Git Bash + WSL2
- Target: all 8 pods + server via WinRM (HTTP 5985)
- Ansible does NOT replace deploy_pod.py — it handles configuration that doesn't exist yet:
  - Install windows_exporter on all pods (one-time)
  - Push registry keys, firewall rules, HKLM Run keys
  - Enforce `C:\RacingPoint\` directory structure

**WinRM requirement:** WinRM must be enabled on each pod. This is the same blocker that defeated Ansible in the past. However, rc-sentry :8091 can run the WinRM enable command one-time: `POST :8091/exec {"cmd": "powershell -ExecutionPolicy Bypass -Command \"Enable-PSRemoting -Force\""}`. This bootstraps WinRM without physical access.

**Ansible does NOT replace:**
- deploy_pod.py (Python — more flexible for binary swap with verification)
- rc-sentry :8091 (always-on backup exec)
- HKLM Run key + Session 1 pattern (Ansible can SET the key, not replace the pattern)

---

## Recommended Project Structure (New Files)

```
racecontrol/
├── .claude/
│   └── skills/                    NEW — project-scoped skills
│       ├── deploy-pod/
│       │   └── SKILL.md           NEW
│       ├── fleet-status/
│       │   └── SKILL.md           NEW
│       ├── pod-logs/
│       │   └── SKILL.md           NEW
│       ├── pod-heal/
│       │   └── SKILL.md           NEW
│       └── racecontrol-context/
│           └── SKILL.md           NEW
│
├── deploy/
│   ├── deploy_pod.py              EXISTS — extend, don't replace
│   ├── ansible/                   NEW — fleet config management
│   │   ├── inventory.ini          NEW — pod IPs + WinRM creds
│   │   ├── install-exporter.yml   NEW — windows_exporter on all pods
│   │   ├── configure-winrm.yml    NEW — bootstrap WinRM via rc-sentry
│   │   └── roles/
│   │       └── windows-baseline/  NEW — firewall, registry, dirs
│   └── monitoring/                NEW
│       ├── prometheus.yml         NEW — scrape config for all 9 hosts
│       └── grafana/
│           └── fleet-dashboard.json  NEW — import into Grafana
│
└── crates/
    └── racecontrol/
        └── src/
            └── metrics.rs         OPTIONAL NEW — Prometheus /metrics endpoint
```

**Personal (James ~/.claude/) — not committed to repo:**

```
~/.claude/
├── settings.json                  EXISTS — add context7 MCP, new hooks
└── skills/
    ├── rp-deploy-server/
    │   └── SKILL.md               NEW
    └── rp-build-rc-agent/
        └── SKILL.md               NEW
```

---

## Architectural Patterns

### Pattern 1: Skill as Deploy Orchestrator

**What:** A SKILL.md file that wraps deploy_pod.py with argument validation, dry-run option, and Bono notification. The skill itself contains instructions; execution happens via existing Python script.

**When to use:** For any repeated operational task with more than 2 steps that James performs at the Claude Code prompt.

**Trade-offs:** Skills add no new infrastructure risk. The underlying script is the same deploy_pod.py that already works. The skill just ensures Claude runs the right sequence with the right arguments.

**SKILL.md pattern:**
```yaml
---
name: deploy-pod
description: Deploy rc-agent binary to a pod or all pods. Use when user says "deploy to pod N", "push new binary", or "update pods".
disable-model-invocation: true
allowed-tools: Bash(python *)
---

Deploy rc-agent to the specified pod using deploy_pod.py.

1. Confirm pod number or "all" from $ARGUMENTS
2. Run: python deploy/deploy_pod.py $ARGUMENTS
3. Watch output for VERIFY_OK or FAILED
4. If FAILED: report the error and do NOT retry automatically

Working directory must be the racecontrol repo root.
```

### Pattern 2: HTTP Hook for Deploy Guarding

**What:** A PreToolUse hook that reads incoming Bash commands and blocks any attempt to run `rc-agent.exe` directly on James's machine. Prevents the documented failure mode (running pod binary on .27 crashes the workstation).

**When to use:** Any tool-use that has a known unsafe pattern on James's workstation.

**Trade-offs:** Low cost — hook is a 20-line bash script. Fires on every Bash call but exits early (< 1ms) unless the command matches.

**Hook pattern:**
```bash
#!/bin/bash
# .claude/hooks/block-pod-binary.sh
COMMAND=$(jq -r '.tool_input.command // ""' < /dev/stdin)
if echo "$COMMAND" | grep -qiE '(rc-agent\.exe|pod-agent\.exe)' && \
   ! echo "$COMMAND" | grep -q 'deploy'; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: "rc-agent.exe must not run on .27 — use deploy_pod.py to deploy to pods"
    }
  }'
else
  exit 0
fi
```

### Pattern 3: Prometheus + racecontrol fleet/health JSON

**What:** Grafana connects to racecontrol's existing REST API as a JSON data source alongside Prometheus. Windows metrics (CPU, RAM) come from Prometheus/windows_exporter; operational state (ws_connected, billing active, pod game state) comes directly from `/api/v1/fleet/health`.

**When to use:** When the metric you need is application state (not OS metrics). Avoids adding a `metrics.rs` endpoint to racecontrol unless strictly needed.

**Trade-offs:** Grafana JSON plugin requires additional configuration vs native Prometheus. For a v9.0 MVP, start with Prometheus for OS metrics and use Grafana's JSON plugin for fleet/health. Only add `metrics.rs` if the JSON plugin is insufficient.

**Data flow:**
```
Grafana dashboard
    |-- Panel A: Pod CPU (source: Prometheus, metric: windows_cpu_time_total)
    |-- Panel B: Pod RAM (source: Prometheus, metric: windows_os_physical_memory_free_bytes)
    |-- Panel C: Fleet status (source: JSON plugin → racecontrol /api/v1/fleet/health)
    |-- Panel D: Active billing (source: JSON plugin → racecontrol /api/v1/billing/active)
```

### Pattern 4: Ansible via rc-sentry Bootstrap

**What:** Use rc-sentry :8091 (already deployed on all pods) to enable WinRM on each pod in a single pass. Once WinRM is active, Ansible can manage pods directly from James's workstation.

**When to use:** First-time Ansible setup. This is the bootstrap-only pattern — rc-sentry enables WinRM, then Ansible takes over for ongoing config management.

**Trade-offs:** WinRM previously failed on this network. However, the previous attempts were for OpenSSH (server component store corrupted) and SaltStack (WSL2 portproxy issue). WinRM to Windows from Windows via LAN is simpler — no WSL2 relay, no minion process, HTTP 5985 on LAN. The blocker was configuration, not protocol.

**Bootstrap sequence:**
```bash
# Step 1: Enable WinRM on all pods via rc-sentry
for pod_ip in 192.168.31.89 .33 .28 .88 .86 .87 .38 .91; do
  curl -s -X POST http://$pod_ip:8091/exec \
    -d '{"cmd": "powershell -ExecutionPolicy Bypass -Command \"Enable-PSRemoting -Force; winrm set winrm/config/client/auth @{Basic=\"\"true\"\"}\""}'
done

# Step 2: Test Ansible connectivity
ansible all -i deploy/ansible/inventory.ini -m win_ping

# Step 3: Run playbooks
ansible-playbook deploy/ansible/install-exporter.yml
```

---

## Data Flow Changes

### Monitoring Data Flow (New)

```
Before v9.0:
  racecontrol /api/v1/fleet/health → browser dashboard (:3200)

After v9.0:
  windows_exporter :9182 on pods/server
      |
      v
  Prometheus :9090 on server (.23)  <-- scrapes every 30s
      |
      v
  Grafana :3000 on server (.23)
      |
      +-- Panel: OS metrics (from Prometheus)
      +-- Panel: Fleet state (from racecontrol /fleet/health JSON)
      +-- Panel: Billing (from racecontrol /billing/active JSON)
```

### Claude Code Skill Data Flow (New)

```
Before v9.0:
  James types: python deploy/deploy_pod.py 8
  (remembers syntax, arguments, verification steps)

After v9.0:
  James types: /deploy-pod 8
      |
      v
  Claude reads .claude/skills/deploy-pod/SKILL.md
      |
      v
  Bash: python deploy/deploy_pod.py 8
      |
      v
  Claude reads output, reports result or escalates failure
      |
      v
  PostToolUse hook appends to DEPLOY_LOG.md
```

### MCP Server Data Flow (Repaired)

```
Before v9.0:
  racingpoint-gmail broken (OAuth expired)

After v9.0:
  Claude Code session
      |
      v
  racingpoint-gmail MCP tool call
      |
      v
  C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js
  (running as stdio MCP server, OAuth refreshed)
      |
      v
  Gmail API → send or read emails
```

---

## Deployment Topology Summary

| Component | Runs On | How Deployed | Manages |
|-----------|---------|--------------|---------|
| Claude Code skills | James .27 | Committed to repo (.claude/skills/) | James's sessions only |
| Claude Code hooks | James .27 | ~/.claude/settings.json | James's sessions only |
| racingpoint-gmail MCP | James .27 | ~/.claude/settings.json (already) | Re-auth only |
| context7 MCP | James .27 | ~/.claude/settings.json | Docs lookups |
| windows_exporter | All 9 Windows hosts | MSI installer via rc-sentry :8091 or Ansible | OS metrics |
| Prometheus | Server .23 | Windows service (manual install or Ansible) | Metrics collection |
| Grafana | Server .23 | Windows service (manual install or Ansible) | Dashboards |
| Ansible control | James .27 (Git Bash) | pip install ansible + pywinrm | Pod config push |
| Ansible targets | All 8 pods (.23 is separate) | WinRM :5985 (bootstrapped via rc-sentry) | Receive Ansible |

**Nothing new runs on the cloud VPS (app.racingpoint.cloud).** All monitoring is on-premises. Grafana is LAN-only.

---

## Build Order

The following dependency graph determines what must be built before what. Later items depend on earlier.

```
1. Claude Code skills (no deps — create SKILL.md files, test immediately)
   .claude/skills/fleet-status/SKILL.md
   .claude/skills/deploy-pod/SKILL.md
   .claude/skills/pod-logs/SKILL.md
   .claude/skills/pod-heal/SKILL.md

2. Claude Code hooks (depends on: knowing which Bash patterns to guard)
   ~/.claude/settings.json → add PreToolUse block-pod-binary hook
   ~/.claude/settings.json → add PostToolUse deploy-logger hook

3. MCP OAuth repair (depends on: Gmail OAuth re-authorization)
   Update GOOGLE_REFRESH_TOKEN in ~/.claude/settings.json
   Test: ask Claude to send a test email via racingpoint-gmail

4. Monitoring foundation (depends on: rc-sentry on all pods confirmed)
   Install windows_exporter on server .23 and all 8 pods via rc-sentry
   Install Prometheus on server .23
   Configure prometheus.yml scrape_configs for all 9 hosts
   Verify Prometheus targets page shows all 9 hosts as UP

5. Grafana dashboards (depends on: Prometheus UP with data)
   Install Grafana on server .23
   Add Prometheus data source
   Add racecontrol JSON data source (fleet/health endpoint)
   Import fleet-dashboard.json

6. Ansible bootstrap (depends on: rc-sentry confirmed on all pods)
   pip install ansible pywinrm on James's Git Bash
   Write deploy/ansible/inventory.ini
   Bootstrap WinRM via rc-sentry curl loop
   ansible all -m win_ping to confirm
   Playbook: install-exporter.yml (replaces Step 4 manual install for future pods)

7. racecontrol-context skill (depends on: codebase stable enough to document)
   .claude/skills/racecontrol-context/SKILL.md with module map
```

**Critical path:** Steps 1-3 are independent and can be built in parallel. Step 4 (monitoring) is the only one with hardware prerequisites (rc-sentry must be on all pods — confirmed deployed in v8.0). Step 6 (Ansible) can be done in parallel with Steps 4-5.

---

## Anti-Patterns

### Anti-Pattern 1: Monitoring as the racecontrol "business metrics"

**What people do:** Add a full Prometheus `/metrics` endpoint to racecontrol as the first monitoring step, exposing billing counts, lap counts, session durations as custom Prometheus metrics.

**Why it's wrong:** It requires modifying production Rust code to add a metrics dependency, testing the endpoint, and deploying a new binary — all before seeing any monitoring value. The existing `/api/v1/fleet/health` and `/billing/active` endpoints already expose the same data via JSON.

**Do this instead:** Use Grafana's JSON data source plugin to poll existing REST endpoints. Add custom Prometheus metrics to racecontrol only if time-series aggregation is needed for a specific dashboard (e.g., "laps per hour over 7 days"). This is a Phase 2 concern, not MVP.

### Anti-Pattern 2: Replacing deploy_pod.py with Ansible

**What people do:** Move the entire binary-swap deploy process into Ansible playbooks, replacing deploy_pod.py with `win_copy` + `win_service` tasks.

**Why it's wrong:** Ansible's `win_copy` over WinRM is slow (100+ seconds for a 10MB binary). The self-swap pattern in deploy_pod.py (download via curl on the pod itself from James's HTTP server :9998) is faster because the binary moves over LAN at gigabit speed. Ansible's strength is idempotent config management (registry keys, firewall rules, service definitions) — not binary deployment.

**Do this instead:** Keep deploy_pod.py for binary swaps. Use Ansible for everything that doesn't change per-deploy: install windows_exporter once, set HKLM Run keys once, configure firewall rules once. "Ansible for config, deploy_pod.py for code" is the boundary.

### Anti-Pattern 3: Project-scoped skills that contain sensitive data

**What people do:** Put pod IPs, server credentials, or API keys directly in `.claude/skills/` SKILL.md files, then commit to the repo.

**Why it's wrong:** `.claude/skills/` is committed to the racecontrol repo on GitHub (public org). Anything in SKILL.md is readable by anyone with repo access.

**Do this instead:** Skills reference environment variables or call existing scripts that read from `racecontrol.toml` (never committed). Pod IPs are acceptable (already in MEMORY.md which is private). Credentials stay in `~/.claude/settings.json` (personal, not committed) or in `racecontrol.toml`.

### Anti-Pattern 4: Adding monitoring to the cloud VPS

**What people do:** Run Prometheus and Grafana on app.racingpoint.cloud (Bono's VPS) and have pods send metrics over the internet.

**Why it's wrong:** Pods are on a private LAN (192.168.31.x). Sending metrics over the internet adds latency, requires VPN/Tailscale to reach each pod's exporter port, and adds a cloud infrastructure dependency to venue-local reliability monitoring. If the internet drops (which happens at the venue), monitoring breaks exactly when you need it most.

**Do this instead:** Prometheus and Grafana live on the server (.23) on the venue LAN. All metrics traffic stays on 192.168.31.x. James and Uday access Grafana via the LAN or via Tailscale SSH tunnel from outside. Bono gets alerts via the existing email_alerts.rs mechanism in racecontrol — no change needed.

---

## Integration Summary Table

| v9.0 Feature | Touches Existing Code? | Touches Existing Config? | New Files | Deployment Target |
|-------------|------------------------|--------------------------|-----------|-------------------|
| Claude Code skills | No | No (project .claude/) | 5 SKILL.md files | James .27 |
| Claude Code hooks | No | Yes (settings.json) | 2 bash hook scripts | James .27 |
| Gmail MCP repair | No | Yes (settings.json) | No | James .27 |
| context7 MCP | No | Yes (settings.json) | No | James .27 |
| windows_exporter | No | No | prometheus.yml | Server + all 8 pods |
| Prometheus | No | No | prometheus.yml | Server .23 |
| Grafana | No | No | fleet-dashboard.json | Server .23 |
| Ansible bootstrap | No | No | inventory.ini + playbooks | James .27 (control) |
| Ansible WinRM | No | Yes (pod Windows config) | configure-winrm.yml | All 8 pods |

**Net change to existing racecontrol Rust code: zero.** All v9.0 components are additive or side-car.

---

## Sources

- [Claude Code Skills Official Docs](https://code.claude.com/docs/en/skills) — SKILL.md format, frontmatter fields, invocation control, supporting files — HIGH confidence
- [Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks) — hook types, PreToolUse/PostToolUse, stdin JSON format, exit codes — HIGH confidence
- [windows_exporter GitHub](https://github.com/prometheus-community/windows_exporter) — port 9182, MSI install, collector list — HIGH confidence
- [Grafana windows_exporter dashboard](https://grafana.com/grafana/dashboards/13466-windows-exporter-for-prometheus-dashboard/) — confirmed dashboard available for import — HIGH confidence
- [Ansible Windows WinRM docs](https://docs.ansible.com/projects/ansible/latest/os_guide/windows_winrm.html) — WinRM setup, pywinrm requirement — HIGH confidence
- [Google Workspace MCP (taylorwilsdon)](https://github.com/taylorwilsdon/google_workspace_mcp) — OAuth flow, transport modes, Claude Code integration — MEDIUM confidence
- Existing codebase (read directly 2026-03-20): `crates/racecontrol/src/ai.rs`, `crates/racecontrol/src/deploy.rs`, `crates/racecontrol/src/bono_relay.rs`, `crates/racecontrol/src/state.rs`, `crates/rc-sentry/src/main.rs`, `deploy/deploy_pod.py`, `racecontrol.toml`, `~/.claude/settings.json` — HIGH confidence

---
*Architecture research for: v9.0 Tooling & Automation — Claude Code skills, MCP servers, deployment automation, monitoring/alerting*
*Researched: 2026-03-20 IST*
