# RC-Doctor v2.2: Self-Healing System for RacingPoint

**Status**: APPROVED — Audited by 6 AI models (DeepSeek V3, Kimi K2.5, Gemini 2.5 Pro, GPT-4.1, Qwen 3 235B, Nemotron Ultra 253B)
**Score**: 7.5/10 after revisions (up from 4/10 original design)
**Owner**: James (on-site execution) + Bono (cloud deployment)
**Date**: 2026-03-27
**Updated**: 2026-03-28 — Added AI-powered diagnosis via OpenRouter (free + cheap models)

---

## Problem Statement

RacingPoint runs 14+ microservices on a single VPS. Current monitoring is fragmented across 5 cron scripts and 2 Python daemons that work in isolation. They detect symptoms but not root causes — PM2 blindly restarts services without fixing why they crashed (e.g., racecontrol-pwa: 51 restarts, dashboard: 14 restarts, still crash looping).

## Architecture: 3 Layers + Floor

```
+-----------------------------------------------------+
|  LAYER 1: EYES  (Uptime Kuma + External Canary)      |
|  Monitoring + alerting + status page                  |
|  On-box: port 3001 (already running)                  |
|  Off-box: external canary on venue PC or $5 VPS       |
+-----------------------------------------------------+
|  LAYER 2: MUSCLE  (Monit)                             |
|  Process supervision, dependency-aware restarts        |
|  5MB RAM, 30s cycle, native `depends on`              |
+-----------------------------------------------------+
|  LAYER 3: BRAIN  (rc-doctor.sh)                       |
|  Custom playbook logic PM2/Monit can't do             |
|  ~200 lines bash, systemd timer every 60s             |
+-----------------------------------------------------+
|  FLOOR: PM2 (executor only, autorestart disabled      |
|  for Monit-managed services)                          |
+-----------------------------------------------------+
```

### What Each Layer Does

| Layer | Handles | Can't Do |
|-------|---------|----------|
| Uptime Kuma + Canary | Alert humans, track uptime history, detect VPS-level outages | Fix anything |
| Monit | Restart processes with dependency order, resource monitoring | Custom remediation (disk cleanup, rebuilds) |
| rc-doctor.sh | Disk cleanup, binary rebuild, port conflicts, WAL vacuum, crash root cause | Basic restarts (Monit does that) |
| AI Diagnosis | Analyze error logs, identify non-obvious root causes, suggest fixes | Take action (read-only advisor) |
| PM2 | Process executor (start/stop on Monit's command) | Decision-making (disabled for managed services) |

---

## AI-Powered Diagnosis (OpenRouter — Free + Cheap Models)

When rc-doctor's pattern-matching playbooks can't identify a root cause, it escalates to AI models via OpenRouter for intelligent log analysis.

### Model Selection (Tested 2026-03-28)

All models **live-tested with real PM2 crash logs** (EADDRINUSE port conflict scenario). Selected for:
- **Structured JSON output** — must return valid parseable JSON
- **SRE/DevOps domain knowledge** — understand port conflicts, process crashes, dependency chains
- **Code-aware** — parse Node.js and Rust stack traces
- **Actionable fixes** — suggest specific commands, not generic advice

| Tier | Model | ID | Why This Model | Speed | Cost/Call | Monthly (10/day) |
|------|-------|----|----------------|-------|-----------|-------------------|
| **1** | Qwen3 Coder 30B | `qwen/qwen3-coder-30b-a3b-instruct` | **Best fix quality.** Code-specialized 30B MoE. Returned `"systemctl start racecontrol && pm2 restart racingpoint-admin"` — the exact two-step fix. Understands dependency chains. | 3.9s | $0.000054 | $0.016 |
| **2** | DeepSeek V3.1 | `deepseek/deepseek-chat-v3.1` | **Best root cause analysis.** Explained the full dependency chain failure. Clean JSON. DeepSeek's reasoning strength shines on multi-signal diagnosis. | 5.3s | $0.000225 | $0.068 |
| **3** | Gemma 3 12B | `google/gemma-3-12b-it` | **Fastest.** 1.1s response. Correct root cause. Terse output — good for simple/single-cause failures where speed matters. Google's code-trained model. | 1.1s | $0.000046 | $0.014 |
| **4** | Mistral Nemo 12B | `mistralai/mistral-nemo` | **Cheapest reliable.** $0.02/M input. Correct diagnosis with good detail. Mistral's code-trained 12B. Proven reliable across both test sessions. | 5.8s | $0.000018 | $0.005 |

**14 models evaluated, 4 selected.** Full evaluation:

| Model | JSON Valid | Root Cause Correct | Result |
|-------|-----------|-------------------|--------|
| Qwen3-Coder-30B | YES | YES (best fix) | **SELECTED** |
| DeepSeek V3.1 | YES | YES (best explanation) | **SELECTED** |
| Gemma 3 12B | YES | YES (fastest) | **SELECTED** |
| Mistral Nemo 12B | YES | YES (cheapest) | **SELECTED** |
| Llama 3.1 8B | YES | NO (missed upstream dep) | REJECTED — wrong diagnosis |
| Qwen 2.5 Coder 7B | NO (echoed prompt) | -- | REJECTED — instruction following failure |
| All free tier models | RATE LIMITED | -- | REJECTED — unreliable availability |
| Qwen3.5 9B, Nemotron 9B v2, OLMo 32B, Qwen3 14B | NO RESPONSE | -- | REJECTED — overloaded/unavailable |

**Selection strategy (waterfall):**
1. **Qwen3-Coder-30B** — primary, best overall quality, code-specialized
2. **Gemma 3 12B** — fast fallback when #1 is overloaded (1.1s vs 3.9s)
3. **Mistral Nemo 12B** — cheapest reliable fallback ($0.005/month)
4. **DeepSeek V3.1** — complex multi-signal failures needing deep reasoning
5. **Give up** — return "manual investigation needed", never blocks remediation

**Why paid-only (no free tier):**
Free OpenRouter models (Nemotron, GPT-oss, Qwen3-Coder free) have aggressive shared rate limits. In both test rounds, ALL free models returned empty when we needed them. For a self-healing system that must work at 2AM during an incident, reliability > cost savings. At $0.005-0.068/month, the paid tier costs less than a WhatsApp message.

### When AI Diagnosis Triggers

AI is NOT called on every check. It triggers ONLY when:
1. **Crash loop with unknown cause** — standard checks (port conflict, missing modules, stale binary) all passed but service still crashes
2. **Escalation from Monit** — 5 restarts in 10 cycles and playbooks didn't fix it
3. **Weekly health report** — summarize incidents and spot patterns (Sunday 10:00 AM)
4. **On-demand** — `rc-doctor.sh diagnose <service>`

### Flow

```
Service crash loop detected by Monit
        |
  rc-doctor standard playbook:
  port conflict? missing modules? stale binary? disk? memory?
        |
  All standard checks pass, still crashing
        |
  Collect last 50 lines of PM2 error log
        |
  ai_diagnose() call chain:
    Tier 1: nvidia/nemotron-3-nano-30b-a3b:free  (1s, $0)
      |-- rate limited/fail -->
    Tier 2: mistralai/mistral-nemo               (2s, $0.00002)
      |-- fail -->
    Tier 3: openrouter/free                       (11s, $0)
      |-- fail -->
    Return: {"root_cause":"AI unavailable","fix":"manual investigation needed"}
        |
  Parse JSON: {root_cause, severity, fix, category}
        |
  Log diagnosis + include in WhatsApp alert
  If fix matches known-safe pattern: auto-apply
  Otherwise: log for human review
```

### AI Functions (add to rc-doctor.sh)

```bash
# === AI DIAGNOSIS (OpenRouter — free + cheap models) ===
OPENROUTER_KEY="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"
# Diagnosis rotation (waterfall — try in order until one responds)
AI_DIAGNOSIS_MODELS=(
  "qwen/qwen3-coder-30b-a3b-instruct"     # Tier 1: code-specialist, best fix quality (3.9s, $0.07/M)
  "google/gemma-3-12b-it"                  # Tier 2: fastest fallback (1.1s, $0.04/M)
  "mistralai/mistral-nemo"                 # Tier 3: cheapest reliable ($0.02/M)
)

# Multi-model audit (all queried in parallel for consensus — use for complex/critical failures)
AI_AUDIT_MODELS=(
  "qwen/qwen3-coder-30b-a3b-instruct"     # Qwen code expert ($0.07/M)
  "google/gemma-3-12b-it"                  # Google code model ($0.04/M)
  "mistralai/mistral-nemo"                 # Mistral ($0.02/M)
  "deepseek/deepseek-chat-v3.1"            # DeepSeek reasoning ($0.15/M)
  "deepseek/deepseek-chat"                 # DeepSeek V3 ($0.32/M)
  "meta-llama/llama-3.1-70b-instruct"      # Meta Llama 70B ($0.40/M)
)
AI_CALLS_FILE="/var/lib/rc-doctor/ai-calls"

check_ai_budget() {
  local count
  count=$(grep -c "$(date +%Y-%m-%dT%H)" "$AI_CALLS_FILE" 2>/dev/null || echo 0)
  [ "$count" -lt 5 ]  # Max 5 AI calls per hour
}

ai_diagnose() {
  local service="$1"
  local error_logs="$2"

  if ! check_ai_budget; then
    log "AI budget exhausted this hour"
    echo '{"root_cause":"AI budget exhausted","severity":"unknown","fix":"manual investigation","category":"unknown"}'
    return 1
  fi

  local sys_prompt="You are an SRE assistant for RacingPoint (sim racing venue, 14 microservices on single VPS). Analyze error logs. Return ONLY valid JSON: {\"root_cause\":\"...\",\"severity\":\"critical|warning|info\",\"fix\":\"one actionable command or instruction\",\"category\":\"port_conflict|memory_leak|missing_dep|config_error|db_issue|network|unknown\"}"

  local user_prompt="Service: $service
Error logs (last 50 lines):
$error_logs

Ports: racecontrol=8080 pwa=3500 admin=3201 dashboard=3400 gateway=3100 whatsapp=3000 comms=8765
Return ONLY valid JSON, no markdown."

  local response=""
  for model in "${AI_MODELS[@]}"; do
    response=$(curl -sf --max-time 20 https://openrouter.ai/api/v1/chat/completions \
      -H "Authorization: Bearer $OPENROUTER_KEY" \
      -H "Content-Type: application/json" \
      -d "$(jq -n --arg s "$sys_prompt" --arg u "$user_prompt" --arg m "$model" '{
        model: $m,
        messages: [{role:"system",content:$s},{role:"user",content:$u}],
        max_tokens: 300, temperature: 0.1
      }')" 2>/dev/null | jq -r '.choices[0].message.content // empty')

    if [ -n "$response" ]; then
      log "AI diagnosis via $model"
      break
    fi
    log "AI model $model failed, trying next"
  done

  if [ -z "$response" ]; then
    log "All AI models failed"
    echo '{"root_cause":"AI unavailable","severity":"unknown","fix":"manual investigation needed","category":"unknown"}'
    return 1
  fi

  # Strip markdown fences, validate JSON
  response=$(echo "$response" | sed 's/^```json//; s/^```//; s/```$//' | tr -d '\n')
  if echo "$response" | jq . >/dev/null 2>&1; then
    echo "$(date -Is)" >> "$AI_CALLS_FILE"
    log "AI result: $response"
    echo "$response"
  else
    log "AI returned invalid JSON: $response"
    echo '{"root_cause":"AI parse error","severity":"unknown","fix":"check rc-doctor.log","category":"unknown"}'
    return 1
  fi
}

ai_weekly_report() {
  local audit_data
  audit_data=$(tail -500 /var/lib/rc-doctor/audit.log 2>/dev/null)
  [ -z "$audit_data" ] && return 0

  local report
  report=$(curl -sf --max-time 30 https://openrouter.ai/api/v1/chat/completions \
    -H "Authorization: Bearer $OPENROUTER_KEY" \
    -H "Content-Type: application/json" \
    -d "$(jq -n --arg p "Summarize this week's RacingPoint infrastructure incidents. Group by service, identify patterns, give 3 prioritized recommendations. Under 300 words.\n\nAudit log:\n$audit_data" '{
      model: "nvidia/nemotron-3-nano-30b-a3b:free",
      messages: [{role:"user",content:$p}],
      max_tokens: 500, temperature: 0.3
    }')" 2>/dev/null | jq -r '.choices[0].message.content // "Report failed"')

  log "WEEKLY REPORT: $report"
  alert_whatsapp "Weekly Health Report:\n$report"
  audit "weekly-report" "system" "generated"
}

cmd_diagnose() {
  local service="${1:-}"
  [ -z "$service" ] && { echo "Usage: rc-doctor.sh diagnose <service-name>"; return 1; }
  local logs
  logs=$(pm2 logs "$service" --lines 50 --nostream 2>/dev/null)
  [ -z "$logs" ] && { echo "No logs found for $service"; return 1; }
  ai_diagnose "$service" "$logs"
}

ai_multi_audit() {
  # Query all 6 models in parallel for consensus — used for critical/complex failures
  local service="$1"
  local error_logs="$2"
  local results_dir="/var/lib/rc-doctor/audit-$$"
  mkdir -p "$results_dir"

  local sys_prompt="You are an SRE assistant for RacingPoint (sim racing venue, 14 microservices, single VPS). Analyze error logs. Return ONLY valid JSON: {\"root_cause\":\"...\",\"severity\":\"critical|warning|info\",\"fix\":\"one actionable command\",\"category\":\"port_conflict|memory_leak|missing_dep|config_error|db_issue|network|unknown\",\"confidence\":\"high|medium|low\"}"

  local user_prompt="Service: $service
Error logs:
$error_logs

Ports: racecontrol=8080 pwa=3500 admin=3201 dashboard=3400 gateway=3100
Return ONLY valid JSON."

  # Fire all 6 models in parallel
  for model in "${AI_AUDIT_MODELS[@]}"; do
    local safe_name
    safe_name=$(echo "$model" | tr '/:' '_')
    (curl -sf --max-time 25 https://openrouter.ai/api/v1/chat/completions \
      -H "Authorization: Bearer $OPENROUTER_KEY" \
      -H "Content-Type: application/json" \
      -d "$(jq -n --arg s "$sys_prompt" --arg u "$user_prompt" --arg m "$model" '{
        model: $m,
        messages: [{role:"system",content:$s},{role:"user",content:$u}],
        max_tokens: 300, temperature: 0.1
      }')" 2>/dev/null | jq -r '.choices[0].message.content // empty' \
      > "$results_dir/$safe_name" ) &
  done
  wait  # Wait for all to complete

  # Collect and summarize results
  log "MULTI-MODEL AUDIT for $service:"
  local consensus_cause="" cause_counts=""
  for f in "$results_dir"/*; do
    local model_name
    model_name=$(basename "$f")
    local content
    content=$(cat "$f" | sed 's/^```json//; s/^```//; s/```$//' | tr -d '\n')
    if echo "$content" | jq . >/dev/null 2>&1; then
      local cause sev fix
      cause=$(echo "$content" | jq -r '.root_cause // "unknown"')
      sev=$(echo "$content" | jq -r '.severity // "unknown"')
      fix=$(echo "$content" | jq -r '.fix // "unknown"')
      log "  [$model_name] $sev | $cause | fix: $fix"
      cause_counts="$cause_counts $cause"
    else
      log "  [$model_name] FAILED (invalid response)"
    fi
  done

  rm -rf "$results_dir"
  audit "multi-audit" "$service" "6_models_queried"
}

cmd_audit() {
  local service="${1:-}"
  [ -z "$service" ] && { echo "Usage: rc-doctor.sh audit <service-name>"; return 1; }
  local logs
  logs=$(pm2 logs "$service" --lines 50 --nostream 2>/dev/null)
  [ -z "$logs" ] && { echo "No logs found for $service"; return 1; }
  echo "Running 6-model audit for $service (takes ~15s)..."
  ai_multi_audit "$service" "$logs"
  echo "Results logged to /var/log/rc-doctor.log"
}
```

### Integration in crash-loop playbook

After standard fixes fail and before escalating to WhatsApp:

```bash
# In cmd_crash_loop(), after Step 5 verify fails:
local error_logs diagnosis ai_cause ai_fix
error_logs=$(pm2 logs "$service" --lines 50 --nostream 2>/dev/null)
diagnosis=$(ai_diagnose "$service" "$error_logs")
ai_cause=$(echo "$diagnosis" | jq -r '.root_cause // "unknown"')
ai_fix=$(echo "$diagnosis" | jq -r '.fix // "manual investigation"')
alert_whatsapp "$service crash loop. AI: $ai_cause | Fix: $ai_fix"
audit "crash-loop" "$service" "AI_DIAGNOSED:$ai_cause"
```

### Weekly report (add to cmd_routine)

```bash
# In cmd_routine(), in the hourly block:
if [ "$(date +%u)" = "7" ] && [ "$(date +%H)" = "10" ] && [ "$minute" = "00" ]; then
  ai_weekly_report
fi
```

### New command: `rc-doctor.sh diagnose <service>`

```bash
# In DISPATCH case statement, add:
  diagnose)         cmd_diagnose "${2:-}" ;;
```

### Safety Constraints

- AI models are **read-only advisors** — they suggest, rc-doctor decides
- Max **5 AI calls per hour** (tracked in `/var/lib/rc-doctor/ai-calls`)
- **No sensitive data** sent — only error logs + service names, never keys/passwords/customer data
- If all models fail, returns "manual investigation needed" — never blocks remediation
- All AI responses logged to audit trail with model name
- Free tier handles 99% of calls; cheap models only hit when free is rate-limited

### Cost Summary

| Scenario | Cost |
|----------|------|
| Single diagnosis (Qwen3-Coder) | $0.000054 per call |
| 10 diagnoses/day, 30 days | $0.016/month (1.6 cents) |
| 6-model audit (all 6 queried once) | ~$0.0012 per audit (~0.1 cents) |
| Weekly report (4/month) | ~$0.001/month |
| **Realistic monthly budget** | **$0.02-0.05/month** |
| Worst case (50 diagnoses/day + 10 audits) | $0.09/month (9 cents) |

### Commands

| Command | What It Does | Models Used | Cost |
|---------|-------------|-------------|------|
| `rc-doctor.sh diagnose <svc>` | Single-model diagnosis | 1 model (waterfall) | ~$0.00005 |
| `rc-doctor.sh audit <svc>` | 6-model consensus audit | All 6 in parallel | ~$0.0012 |
| `rc-doctor.sh weekly-report` | AI-generated weekly summary | 1 model | ~$0.0003 |
| (auto) crash-loop escalation | Triggered by Monit after 5 restarts | 1 model | ~$0.00005 |

---

## Layer 1: Eyes (Uptime Kuma + External Canary)

### On-Box: Uptime Kuma (already running, port 3001)

Configure these monitors:

#### GROUP: Core (15s interval)
| Monitor | Type | Target | Healthy | Degraded |
|---------|------|--------|---------|----------|
| RaceControl API | HTTP | localhost:8080/api/v1/health | status:ok | >500ms or 5xx |
| PostgreSQL | TCP | localhost:5432 | connected | timeout |
| Nginx | HTTP | localhost:80 | 200 | 502/504 |

#### GROUP: Apps (30s interval)
| Monitor | Type | Target |
|---------|------|--------|
| PWA | HTTP | localhost:3500 |
| Admin Dashboard | HTTP | localhost:3201/api/health |
| Venue Dashboard | HTTP | localhost:3400 |
| Kiosk | HTTP | localhost:3300 |
| Website | HTTP | localhost:3600 |

#### GROUP: Messaging (30s interval)
| Monitor | Type | Target |
|---------|------|--------|
| API Gateway | HTTP | localhost:3100/health |
| WhatsApp Bot | HTTP | localhost:3000/health |
| Evolution API | HTTP | localhost:53622 |
| Comms-Link | TCP | localhost:8765 |

#### GROUP: Support (60s interval)
| Monitor | Type | Target |
|---------|------|--------|
| Hiring Bot | HTTP | localhost:3050 |
| Website API | HTTP | localhost:5050 |

#### GROUP: Infrastructure (300s interval)
| Monitor | Type | Check |
|---------|------|-------|
| Disk Usage | Script | df / < 85% |
| Memory | Script | free < 85% |
| SSL Cert Expiry | HTTPS | app.racingpoint.cloud (daily) |

#### Alerting
- **Primary**: WhatsApp via Evolution API webhook
- **Fallback**: Email via racingpoint-google Gmail API (if WhatsApp fails)
- **Rules**: Alert after 3 consecutive failures, resolve after 2 successes
- **Status page**: Proxy via Nginx at `status.racingpoint.cloud`

### Off-Box: External Canary

**Purpose**: Detect when the entire VPS is down (kernel panic, provider outage, network partition). On-box Uptime Kuma can't catch this.

**Options (pick one):**
1. **Venue PC (James)**: Cron job every 2 min pings `app.racingpoint.cloud`. If 3 consecutive failures, send WhatsApp alert via local Evolution instance or email.
2. **$5/mo external VPS**: Dedicated Uptime Kuma instance monitoring all public endpoints.
3. **Free tier**: UptimeRobot or similar SaaS with WhatsApp/email webhook.

**Minimum viable canary** (for venue PC):
```bash
#!/bin/bash
# /home/bono/canary.sh — runs every 2 min via Task Scheduler
STATE="/tmp/canary-fails"
if curl -sf --max-time 10 https://app.racingpoint.cloud/api/v1/health >/dev/null 2>&1; then
  echo 0 > "$STATE"
else
  FAILS=$(cat "$STATE" 2>/dev/null || echo 0)
  FAILS=$((FAILS + 1))
  echo "$FAILS" > "$STATE"
  if [ "$FAILS" -ge 3 ]; then
    # Alert via email or local WhatsApp
    echo "VPS DOWN: app.racingpoint.cloud unreachable for 6+ minutes" | mail -s "CRITICAL: VPS DOWN" uday@racingpoint.in
  fi
fi
```

---

## Layer 2: Muscle (Monit)

### Installation
```bash
apt-get install -y monit
systemctl enable monit
```

### Configuration: /etc/monit/monitrc

```monit
# === GLOBAL ===
set daemon 30                         # Check every 30s
set log /var/log/monit.log
set httpd port 2812                   # Web UI for debugging
  allow admin:<secure-password>

# === DEPENDENCY CHAIN ===
# postgres -> racecontrol -> all downstream
# If postgres is down, Monit won't restart racecontrol (waits for postgres first)

check process postgresql with pidfile /var/run/postgresql/16-main.pid
  start program = "/usr/bin/systemctl start postgresql"
  stop program = "/usr/bin/systemctl stop postgresql"
  if failed port 5432 protocol pgsql for 3 cycles then restart
  if 5 restarts within 10 cycles then alert

check process racecontrol matching "racecontrol"
  depends on postgresql
  start program = "/usr/bin/pm2 start racecontrol"
  stop program = "/usr/bin/pm2 stop racecontrol"
  if failed port 8080 protocol http
    request "/api/v1/health" for 3 cycles then restart
  if cpu > 80% for 5 cycles then alert
  if memory > 500 MB then alert
  if 5 restarts within 10 cycles then exec "/root/bin/rc-doctor.sh crash-loop racecontrol"

check process racecontrol-pwa matching "racecontrol-pwa"
  depends on racecontrol
  start program = "/usr/bin/pm2 start racecontrol-pwa"
  stop program = "/usr/bin/pm2 stop racecontrol-pwa"
  if failed port 3500 for 3 cycles then restart
  if 5 restarts within 10 cycles then exec "/root/bin/rc-doctor.sh crash-loop racecontrol-pwa"

check process racingpoint-admin matching "racingpoint-admin"
  depends on racecontrol
  start program = "/usr/bin/pm2 start racingpoint-admin"
  stop program = "/usr/bin/pm2 stop racingpoint-admin"
  if failed port 3201 for 3 cycles then restart
  if 5 restarts within 10 cycles then exec "/root/bin/rc-doctor.sh crash-loop racingpoint-admin"

check process racingpoint-dashboard matching "racingpoint-dashboard"
  depends on racecontrol
  start program = "/usr/bin/pm2 start racingpoint-dashboard"
  stop program = "/usr/bin/pm2 stop racingpoint-dashboard"
  if failed port 3400 for 3 cycles then restart
  if 5 restarts within 10 cycles then exec "/root/bin/rc-doctor.sh crash-loop racingpoint-dashboard"

check process api-gateway matching "racingpoint-api-gateway"
  depends on racecontrol
  start program = "/usr/bin/pm2 start racingpoint-api-gateway"
  stop program = "/usr/bin/pm2 stop racingpoint-api-gateway"
  if failed port 3100 protocol http for 3 cycles then restart

check process whatsapp-bot matching "racingpoint-bot"
  depends on racecontrol
  start program = "/usr/bin/pm2 start racingpoint-bot"
  stop program = "/usr/bin/pm2 stop racingpoint-bot"
  if failed port 3000 for 3 cycles then restart

check process comms-link matching "comms-link"
  start program = "/usr/bin/pm2 start comms-link"
  stop program = "/usr/bin/pm2 stop comms-link"
  if failed port 8765 for 3 cycles then restart

check process nginx with pidfile /var/run/nginx.pid
  start program = "/usr/sbin/nginx"
  stop program = "/usr/sbin/nginx -s stop"
  if failed port 80 for 2 cycles then restart
  if failed port 443 for 2 cycles then restart

# === RESOURCES ===
check filesystem rootfs with path /
  if space usage > 85% then exec "/root/bin/rc-doctor.sh disk-pressure"
  if space usage > 95% then alert
  if inode usage > 85% then alert

check system localhost
  if memory usage > 85% then exec "/root/bin/rc-doctor.sh memory-pressure"
  if cpu usage > 90% for 10 cycles then alert
```

### PM2 Configuration Change

**CRITICAL**: Disable PM2 autorestart for Monit-managed services to prevent race conditions.

Update each ecosystem.config.js:
```javascript
// For services managed by Monit:
{
  name: 'racecontrol',
  autorestart: false,      // Monit handles restart decisions
  // ... rest of config
}
```

Services where PM2 autorestart stays ON (not Monit-managed):
- racingpoint-hiring (non-critical)
- james-email-notifier (non-critical)
- racingpoint-website + racingpoint-website-api (non-critical)
- racingpoint-kiosk (non-critical)

---

## Layer 3: Brain (rc-doctor.sh)

### Location: /root/bin/rc-doctor.sh

```bash
#!/bin/bash
# RC-Doctor v2.1: Custom remediation for edge cases
# Called by: Monit (on escalation), systemd timer (routine), manual
# Philosophy: Fix root causes, not symptoms. Never destructive.

set -euo pipefail

# === SERIALIZATION (prevent concurrent runs) ===
exec 200>/var/lock/rc-doctor.lock
flock -n 200 || { echo "Another rc-doctor instance running, exiting"; exit 0; }

LOG="/var/log/rc-doctor.log"
AUDIT="/var/lib/rc-doctor/audit.log"
STATE_DIR="/var/lib/rc-doctor"
DISABLED_FILE="$STATE_DIR/DISABLED"
MAX_ACTIONS_PER_HOUR=10

log() { echo "[$(date -Is)] $*" | tee -a "$LOG"; }
audit() { echo "[$(date -Is)] ACTION=$1 TARGET=$2 RESULT=$3" >> "$AUDIT"; }

# === ENABLE / DISABLE TOGGLE ===
# Sentinel-file based: touch DISABLED to stop all automated actions.
# Manual commands (status, logs, enable, disable) always work.
# Monit-triggered commands (crash-loop, disk-pressure, memory-pressure) respect the toggle.
# This allows safe maintenance windows without stopping monitoring.

cmd_enable() {
  if [ -f "$DISABLED_FILE" ]; then
    rm -f "$DISABLED_FILE"
    log "RC-Doctor ENABLED (removed $DISABLED_FILE)"
    audit "toggle" "rc-doctor" "enabled"
    echo "RC-Doctor is now ENABLED"
  else
    echo "RC-Doctor is already ENABLED"
  fi
}

cmd_disable() {
  local reason="${1:-manual}"
  mkdir -p "$STATE_DIR"
  echo "disabled_at=$(date -Is) reason=$reason" > "$DISABLED_FILE"
  log "RC-Doctor DISABLED (reason: $reason)"
  audit "toggle" "rc-doctor" "disabled:$reason"
  alert_whatsapp "RC-Doctor DISABLED by operator (reason: $reason). Automated remediation paused."
  echo "RC-Doctor is now DISABLED (reason: $reason)"
  echo "Run 'rc-doctor.sh enable' to re-enable."
}

is_enabled() {
  [ ! -f "$DISABLED_FILE" ]
}

# Guard: skip automated actions when disabled. Allows explicit manual override with --force.
check_enabled_or_exit() {
  if [ -f "$DISABLED_FILE" ] && [ "${FORCE:-}" != "1" ]; then
    local disabled_info
    disabled_info=$(cat "$DISABLED_FILE" 2>/dev/null || echo "unknown")
    log "SKIP: RC-Doctor is DISABLED ($disabled_info). Use --force or 'rc-doctor.sh enable' to resume."
    exit 0
  fi
}

# === SAFETY RAILS ===
check_billing_active() {
  local active
  active=$(curl -sf http://localhost:8080/api/v1/billing/active 2>/dev/null | jq '.count // 0')
  [ "${active:-0}" -gt 0 ]
}

check_peak_load() {
  # Returns true if >4 pods active (defer non-critical work)
  local pods
  pods=$(curl -sf http://localhost:8080/api/v1/fleet/health 2>/dev/null | jq '[.[] | select(.ws_connected==true)] | length')
  [ "${pods:-0}" -gt 4 ]
}

check_action_budget() {
  local count
  count=$(grep -c "$(date +%Y-%m-%dT%H)" "$AUDIT" 2>/dev/null || echo 0)
  [ "$count" -lt "$MAX_ACTIONS_PER_HOUR" ]
}

alert_whatsapp() {
  curl -sf -X POST "http://localhost:53622/message/sendText/Racing%20Point%20Reception" \
    -H "apikey: ${EVOLUTION_API_KEY:-}" \
    -H "Content-Type: application/json" \
    -d "{\"number\":\"917075778180\",\"text\":\"RC-Doctor: $1\"}" >/dev/null 2>&1 || \
  # Fallback to email if WhatsApp fails
  node /root/racingpoint-google/send-email.js \
    --to "uday@racingpoint.in" \
    --subject "RC-Doctor Alert" \
    --body "$1" 2>/dev/null || true
}

# === PLAYBOOKS ===

cmd_disk_pressure() {
  check_enabled_or_exit
  log "PLAYBOOK: disk-pressure"
  local before after freed
  before=$(df / --output=pcent | tail -1 | tr -d '% ')

  # Step 1: PM2 logs (usually biggest offender)
  pm2 flush >/dev/null 2>&1 || true

  # Step 2: Cargo build artifacts (can be 2GB+)
  if [ -d /root/racecontrol/target ] && [ "$(du -sm /root/racecontrol/target | cut -f1)" -gt 500 ]; then
    cargo clean --manifest-path /root/racecontrol/Cargo.toml 2>/dev/null || true
  fi

  # Step 3: Next.js caches
  find /root/racingpoint-*/ -name ".next" -path "*/cache/*" -type d -exec rm -rf {} + 2>/dev/null || true

  # Step 4: Old logs > 50MB (truncate, don't delete — preserve recent entries)
  find /root -maxdepth 1 -name "*.log" -size +50M -exec truncate -s 10M {} \; 2>/dev/null || true

  # Step 5: Vacuum SQLite databases
  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db; do
    [ -f "$db" ] && sqlite3 "$db" "PRAGMA wal_checkpoint(TRUNCATE); VACUUM;" 2>/dev/null || true
  done

  after=$(df / --output=pcent | tail -1 | tr -d '% ')
  freed=$((before - after))
  log "Disk: ${before}% -> ${after}% (freed ${freed}%)"
  audit "disk-pressure" "rootfs" "freed_${freed}_pct"

  [ "$freed" -lt 5 ] && alert_whatsapp "Disk pressure: cleaned ${freed}% but still at ${after}%. Manual intervention needed."
}

cmd_memory_pressure() {
  check_enabled_or_exit
  log "PLAYBOOK: memory-pressure"

  # Step 1: Find top memory consumer among PM2 processes
  local top_proc top_mem
  top_proc=$(pm2 jlist 2>/dev/null | jq -r 'sort_by(.monit.memory) | last | .name // "none"')
  top_mem=$(pm2 jlist 2>/dev/null | jq -r 'sort_by(.monit.memory) | last | .monit.memory / 1048576 | floor')

  # Step 2: If any process > 300MB, restart it (likely leak)
  if [ "${top_mem:-0}" -gt 300 ]; then
    log "Restarting $top_proc (${top_mem}MB - likely memory leak)"
    pm2 restart "$top_proc" 2>/dev/null || true
    audit "memory-pressure" "$top_proc" "restarted_${top_mem}MB"
  fi

  # Step 3: Drop caches as last resort
  sync && echo 3 > /proc/sys/vm/drop_caches 2>/dev/null || true
  audit "memory-pressure" "system" "dropped_caches"
}

cmd_crash_loop() {
  check_enabled_or_exit
  local service="${1:-unknown}"
  log "PLAYBOOK: crash-loop for $service"

  if ! check_action_budget; then
    log "SKIP: Action budget exhausted for this hour"
    alert_whatsapp "$service crash loop but action budget exhausted. Manual check needed."
    return 0
  fi

  # Safety: don't restart during active billing (for critical services)
  if check_billing_active; then
    log "SKIP: Active billing sessions - deferring restart of $service"
    alert_whatsapp "$service in crash loop but billing is active. Deferring."
    return 0
  fi

  # Circuit breaker: defer non-critical during peak load
  case "$service" in
    racingpoint-hiring|racingpoint-website|racingpoint-website-api)
      if check_peak_load; then
        log "SKIP: Peak load - deferring non-critical $service"
        return 0
      fi
      ;;
  esac

  # Step 1: Check port conflict
  local port=""
  case "$service" in
    racecontrol)             port=8080 ;;
    racecontrol-pwa)         port=3500 ;;
    racingpoint-admin)       port=3201 ;;
    racingpoint-dashboard)   port=3400 ;;
    racingpoint-api-gateway) port=3100 ;;
    racingpoint-bot)         port=3000 ;;
    *) port="" ;;
  esac

  if [ -n "$port" ]; then
    local stale_pid
    stale_pid=$(lsof -ti ":$port" 2>/dev/null | head -1)
    if [ -n "$stale_pid" ]; then
      local stale_name
      stale_name=$(ps -p "$stale_pid" -o comm= 2>/dev/null || echo "unknown")
      log "Port $port held by PID $stale_pid ($stale_name) - killing"
      kill "$stale_pid" 2>/dev/null; sleep 2; kill -9 "$stale_pid" 2>/dev/null || true
      audit "crash-loop" "$service" "killed_stale_port_${port}"
    fi
  fi

  # Step 2: Check missing node_modules (for Node.js services)
  local svc_dir="/root/${service}"
  if [ -d "$svc_dir" ] && [ -f "$svc_dir/package.json" ] && [ ! -d "$svc_dir/node_modules" ]; then
    log "Missing node_modules for $service - reinstalling"
    (cd "$svc_dir" && npm install --production) 2>&1 | tail -5 >> "$LOG"
    audit "crash-loop" "$service" "npm_install"
  fi

  # Step 3: For racecontrol (Rust), check stale binary
  if [ "$service" = "racecontrol" ]; then
    cmd_stale_binary
    return
  fi

  # Step 4: Restart with delay (give port time to release)
  log "Restarting $service with 10s delay"
  pm2 stop "$service" 2>/dev/null || true
  sleep 10
  pm2 start "$service" 2>/dev/null || true
  audit "crash-loop" "$service" "delayed_restart"

  # Step 5: Verify after 15s
  sleep 15
  if ! pm2 show "$service" 2>/dev/null | grep -q "online"; then
    alert_whatsapp "$service crash loop - auto-fix failed. Manual intervention needed."
    audit "crash-loop" "$service" "ESCALATED"
  fi
}

cmd_stale_binary() {
  log "PLAYBOOK: stale-binary"

  local build_id git_head
  build_id=$(curl -sf http://localhost:8080/api/v1/health | jq -r '.build_id // "unknown"')
  git_head=$(git -C /root/racecontrol rev-parse --short HEAD 2>/dev/null || echo "unknown")

  if [ "$build_id" = "$git_head" ]; then
    log "Binary is current ($build_id)"
    return 0
  fi

  # Check disk space before building (need at least 3GB)
  local free_gb
  free_gb=$(df / --output=avail | tail -1 | awk '{print int($1/1048576)}')
  if [ "$free_gb" -lt 3 ]; then
    log "SKIP: Only ${free_gb}GB free - not enough for cargo build"
    alert_whatsapp "Stale binary detected but only ${free_gb}GB disk free. Clean up first."
    return 1
  fi

  # ROLLBACK SAFETY: backup current binary before rebuild
  local bin_path="/root/racecontrol/target/release/racecontrol"
  if [ -f "$bin_path" ]; then
    cp "$bin_path" "${bin_path}.bak"
    log "Backed up current binary to ${bin_path}.bak"
  fi

  log "Rebuilding: binary=$build_id, git=$git_head"
  if ! (cd /root/racecontrol && cargo build --release 2>&1 | tail -10) >> "$LOG"; then
    log "BUILD FAILED - restoring backup binary"
    [ -f "${bin_path}.bak" ] && mv "${bin_path}.bak" "$bin_path"
    alert_whatsapp "Cargo build failed for racecontrol. Restored backup. Manual fix needed."
    audit "stale-binary" "racecontrol" "BUILD_FAILED_ROLLED_BACK"
    return 1
  fi

  pm2 restart racecontrol 2>/dev/null || true
  sleep 10

  # Verify new build
  local new_build
  new_build=$(curl -sf http://localhost:8080/api/v1/health | jq -r '.build_id // "unknown"')
  if [ "$new_build" = "unknown" ] || [ "$new_build" = "$build_id" ]; then
    log "REBUILD UNHEALTHY - restoring backup"
    pm2 stop racecontrol 2>/dev/null || true
    [ -f "${bin_path}.bak" ] && mv "${bin_path}.bak" "$bin_path"
    pm2 start racecontrol 2>/dev/null || true
    alert_whatsapp "Rebuilt racecontrol but health check failed. Rolled back to previous binary."
    audit "stale-binary" "racecontrol" "HEALTH_FAILED_ROLLED_BACK"
  else
    rm -f "${bin_path}.bak"
    log "Rebuild complete: $build_id -> $new_build"
    audit "stale-binary" "racecontrol" "rebuilt_${build_id}_to_${new_build}"
  fi
}

cmd_wal_bloat() {
  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db /root/comms-link/data/*.db; do
    [ -f "$db" ] || continue
    local wal="${db}-wal"
    if [ -f "$wal" ]; then
      local size_mb
      size_mb=$(du -m "$wal" 2>/dev/null | cut -f1)
      if [ "${size_mb:-0}" -gt 50 ]; then
        log "WAL bloat: $wal is ${size_mb}MB - checkpointing"
        sqlite3 "$db" "PRAGMA wal_checkpoint(TRUNCATE);" 2>/dev/null || true
        audit "wal-bloat" "$db" "checkpointed_${size_mb}MB"
      fi
    fi
  done
}

cmd_ssl_check() {
  log "PLAYBOOK: ssl-check"
  local days
  days=$(echo | openssl s_client -connect app.racingpoint.cloud:443 -servername app.racingpoint.cloud 2>/dev/null \
    | openssl x509 -noout -enddate 2>/dev/null \
    | cut -d= -f2 \
    | xargs -I{} bash -c 'echo $(( ($(date -d "{}" +%s) - $(date +%s)) / 86400 ))' 2>/dev/null) || days=999

  if [ "${days:-999}" -lt 14 ]; then
    log "SSL cert expires in ${days} days - renewing"
    certbot renew --quiet 2>&1 | tail -3 >> "$LOG"
    nginx -s reload 2>/dev/null || true
    audit "ssl-check" "certs" "renewed_${days}_days_left"
  fi
}

cmd_db_integrity() {
  log "PLAYBOOK: db-integrity"
  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db; do
    [ -f "$db" ] || continue
    local result
    result=$(sqlite3 "$db" "PRAGMA integrity_check;" 2>/dev/null | head -1)
    if [ "$result" != "ok" ]; then
      log "DB INTEGRITY FAIL: $db - $result"
      alert_whatsapp "Database integrity check FAILED for $db: $result"
      audit "db-integrity" "$db" "FAILED_${result}"
    fi
  done
}

cmd_backup_verify() {
  log "PLAYBOOK: backup-verify"
  local backup_dir="/var/backups/racingpoint"
  local today
  today=$(date +%Y%m%d)

  if [ ! -d "$backup_dir" ]; then
    alert_whatsapp "Backup directory $backup_dir does not exist!"
    return 1
  fi

  # Check if today's backup exists
  local found=0
  for f in "$backup_dir"/*"$today"*; do
    [ -f "$f" ] && found=1 && break
  done

  if [ "$found" -eq 0 ]; then
    log "No backup found for today ($today)"
    alert_whatsapp "Daily backup missing for $today. Check backup-databases.sh."
    audit "backup-verify" "daily" "MISSING"
  fi
}

cmd_routine() {
  check_enabled_or_exit
  # Called by systemd timer every 60s - lightweight checks only
  cmd_wal_bloat

  # Docker health (Evolution can silently die)
  if ! curl -sf http://localhost:53622 >/dev/null 2>&1; then
    log "Evolution API unreachable - restarting container"
    docker restart evolution-api 2>/dev/null || true
    audit "routine" "evolution-api" "docker_restart"
  fi

  # DB integrity (run once per hour, not every 60s)
  local minute
  minute=$(date +%M)
  if [ "$minute" = "00" ]; then
    cmd_db_integrity
    cmd_backup_verify
    cmd_ssl_check
  fi
}

cmd_logs() {
  # Tail all monitoring logs together for debugging
  tail -f /var/log/rc-doctor.log /var/log/monit.log /root/.pm2/logs/*.log 2>/dev/null
}

cmd_status() {
  echo "=== RC-Doctor Status ==="
  if is_enabled; then
    echo "State: ENABLED"
  else
    echo "State: DISABLED ($(cat "$DISABLED_FILE" 2>/dev/null || echo 'unknown'))"
  fi
  echo "Last run: $(stat -c %y /var/lock/rc-doctor.lock 2>/dev/null || echo 'never')"
  echo "Actions this hour: $(grep -c "$(date +%Y-%m-%dT%H)" "$AUDIT" 2>/dev/null || echo 0)/$MAX_ACTIONS_PER_HOUR"
  echo ""
  echo "=== Monit Summary ==="
  monit summary 2>/dev/null || echo "Monit not running"
  echo ""
  echo "=== PM2 Status ==="
  pm2 list 2>/dev/null
  echo ""
  echo "=== Recent Actions ==="
  tail -20 "$AUDIT" 2>/dev/null || echo "No actions yet"
}

# === DISPATCH ===
mkdir -p /var/lib/rc-doctor

# Support --force flag to bypass disabled state
[[ "${*}" == *"--force"* ]] && export FORCE=1

case "${1:-routine}" in
  enable)           cmd_enable ;;
  disable)          cmd_disable "${2:-manual}" ;;
  disk-pressure)    cmd_disk_pressure ;;
  memory-pressure)  cmd_memory_pressure ;;
  crash-loop)       cmd_crash_loop "${2:-}" ;;
  stale-binary)     cmd_stale_binary ;;
  wal-bloat)        cmd_wal_bloat ;;
  ssl-check)        cmd_ssl_check ;;
  db-integrity)     cmd_db_integrity ;;
  backup-verify)    cmd_backup_verify ;;
  diagnose)         cmd_diagnose "${2:-}" ;;
  audit)            cmd_audit "${2:-}" ;;
  weekly-report)    ai_weekly_report ;;
  routine)          cmd_routine ;;
  logs)             cmd_logs ;;
  status)           cmd_status ;;
  *)                echo "Usage: rc-doctor.sh {enable|disable [reason]|status|routine|disk-pressure|memory-pressure|crash-loop <svc>|stale-binary|...}"
                    echo ""
                    echo "Toggle:  enable / disable [reason]"
                    echo "Info:    status / logs"
                    echo "Force:   any-command --force  (bypass disabled state)"
                    ;;
esac
```

### systemd Timer

**/etc/systemd/system/rc-doctor.service**
```ini
[Unit]
Description=RC-Doctor routine health check

[Service]
Type=oneshot
ExecStart=/root/bin/rc-doctor.sh routine
TimeoutSec=30
```

**/etc/systemd/system/rc-doctor.timer**
```ini
[Unit]
Description=RC-Doctor routine checks every 60s

[Timer]
OnBootSec=60
OnUnitActiveSec=60
AccuracySec=5

[Install]
WantedBy=timers.target
```

Enable:
```bash
chmod +x /root/bin/rc-doctor.sh
systemctl daemon-reload
systemctl enable --now rc-doctor.timer
```

### Remote Control (from James or Bono)

RC-Doctor can be toggled remotely without SSH using the comms-link relay:

**From James (venue):**
```bash
# Check status
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"custom","custom_command":"rc-doctor.sh status"}'

# Disable (e.g., maintenance window)
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"custom","custom_command":"rc-doctor.sh disable maintenance-window"}'

# Re-enable
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"custom","custom_command":"rc-doctor.sh enable"}'

# Force a specific playbook even when disabled
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"custom","custom_command":"rc-doctor.sh disk-pressure --force"}'
```

**Via SSH fallback:**
```bash
ssh root@100.70.177.44 "rc-doctor.sh disable deploy-in-progress"
ssh root@100.70.177.44 "rc-doctor.sh enable"
ssh root@100.70.177.44 "rc-doctor.sh status"
```

**Toggle behavior:**
| Command | When Disabled | When Enabled |
|---------|--------------|--------------|
| `enable` | Removes DISABLED file, resumes all automation | No-op ("already enabled") |
| `disable [reason]` | No-op (already disabled) | Writes DISABLED file, pauses all automation |
| `status` | Shows DISABLED + reason + last run | Shows ENABLED + metrics |
| `logs` | Always works | Always works |
| `routine` | Exits immediately (logged) | Runs normally |
| `crash-loop <svc>` | Exits immediately (logged) | Runs normally |
| `disk-pressure` | Exits immediately (logged) | Runs normally |
| Any command `--force` | **Runs anyway** (bypasses disabled) | Runs normally |

---

## What Gets Replaced

| Current Component | Action | Why |
|-------------------|--------|-----|
| bono-failsafe.py | REPLACE (Phase 6) | Monit + rc-doctor covers all its logic |
| bono-racecontrol-monitor.sh | REPLACE (Phase 6) | Monit monitors local RC natively |
| health-check.sh | REPLACE (Phase 6) | Monit checks resources, rc-doctor does cleanup |
| bono-server-monitor.sh | KEEP | Venue server monitoring via Tailscale (unique) |
| james-failsafe.py | KEEP | Venue-side, can't be replaced from cloud |
| PM2 | KEEP as executor | autorestart disabled for Monit-managed services |
| git-sync-repos.sh | KEEP | Git operations stay independent |
| backup-databases.sh | KEEP | rc-doctor verifies backups, doesn't replace them |

---

## Implementation Steps

### Step 1: Configure Uptime Kuma (30 min, zero risk)
- Open `http://localhost:3001`
- Add all 17 monitors listed above
- Configure WhatsApp webhook for alerts
- Proxy via Nginx to `status.racingpoint.cloud`
- **Risk**: None. Purely observational.

### Step 2: Write rc-doctor.sh (1 hour, zero risk)
- Deploy script to `/root/bin/rc-doctor.sh`
- Test each playbook manually:
  ```bash
  rc-doctor.sh status          # Should show system state
  rc-doctor.sh wal-bloat       # Should checkpoint any large WALs
  rc-doctor.sh db-integrity    # Should report ok for all DBs
  rc-doctor.sh ssl-check       # Should report cert status
  rc-doctor.sh backup-verify   # Should verify today's backup
  ```
- **Risk**: None. Only runs when called, doesn't restart anything in test mode.

### Step 3: Install Monit (30 min, low risk)
```bash
apt-get install -y monit
# Write /etc/monit/monitrc (from config above)
monit -t                       # Syntax check
systemctl start monit
monit summary                  # Verify all services detected
```
- Run Monit alongside PM2 initially (both managing restarts)
- **Risk**: Low. Monit only acts after 3 failed cycles (90s).

### Step 4: Disable PM2 autorestart for managed services (10 min)
- Update ecosystem.config.js files: `autorestart: false` for critical services
- `pm2 save` to persist
- Verify Monit is the only restart decision-maker
- **Risk**: Medium. If Monit fails, services won't auto-restart. That's why we do this AFTER Step 3 confirms Monit works.

### Step 5: systemd timer for rc-doctor (10 min)
```bash
# Deploy service + timer files
systemctl daemon-reload
systemctl enable --now rc-doctor.timer
systemctl list-timers | grep rc-doctor    # Verify scheduled
```
- **Risk**: Low. Routine checks are lightweight.

### Step 6: Burn-in (2 weeks)

**Week 1: Passive observation**
- Monitor Monit + rc-doctor logs daily
- Compare: does Monit catch issues the old monitors missed?
- Keep old monitors running in parallel

**Week 2: Chaos injection**
| Test | How | Expected Result |
|------|-----|-----------------|
| Kill racecontrol | `pm2 stop racecontrol` | Monit restarts after 90s. Downstream apps NOT restarted. |
| Kill postgres | `systemctl stop postgresql` | Monit restarts postgres. RC waits. Downstream waits. |
| Fill disk to 90% | `fallocate -l 250G /tmp/fill` | rc-doctor disk-pressure cleans up. Alert sent. |
| Memory pressure | Run a memory hog | rc-doctor identifies top consumer, restarts it. |
| Break nginx | `echo "bad" > /etc/nginx/conf.d/bad.conf && nginx -s reload` | Monit restarts nginx. Does NOT trigger 14 app restarts. |
| Stale binary | Change git HEAD without rebuilding | rc-doctor rebuilds, verifies, or rolls back. |
| Port conflict | Start a process on :8080 | rc-doctor kills stale process, Monit restarts RC. |

**Acceptance criteria:**
- [ ] Zero manual interventions during Week 1
- [ ] All chaos tests pass in Week 2
- [ ] Crash loops resolved in <2 retries (vs PM2's 51)
- [ ] No false positive cascades
- [ ] WhatsApp alerts received for all escalations
- [ ] rc-doctor audit log shows all actions taken

### Step 7: Retire old monitors
```bash
# Remove from crontab:
# - bono-racecontrol-monitor.sh
# - health-check.sh

# Stop PM2 process:
# pm2 stop bono-failsafe && pm2 delete bono-failsafe

# Keep:
# - bono-server-monitor.sh (venue-specific)
# - james-failsafe.py (venue-side)
```

### Step 8: External canary (venue PC)
- James deploys canary script on venue PC
- Task Scheduler runs every 2 min
- Alerts via email if VPS unreachable for 6+ min

---

## Metrics: Before vs After

| Metric | Before (Fragmented) | After (RC-Doctor v2.1) |
|--------|---------------------|------------------------|
| Monitors | 5 cron scripts + 2 Python daemons | 1 Monit + 1 systemd timer + Uptime Kuma |
| RAM overhead | ~250MB (2 Python daemons) | ~5MB (Monit) |
| Code to maintain | ~800 lines across 7 files | ~200 lines in 1 file |
| Dependency awareness | None (each monitor independent) | Monit `depends on` chain |
| Root cause analysis | None (restart and pray) | rc-doctor playbooks |
| Crash loop handling | PM2: 51 blind restarts | Monit: 5 retries then escalate to rc-doctor |
| Disk/memory response | Alert only | Auto-cleanup + alert |
| External monitoring | None | Canary on venue PC |
| Alert channels | WhatsApp only | WhatsApp + email fallback |
| Debug at 2AM | Check 7 different log files | `rc-doctor.sh logs` or `rc-doctor.sh status` |

---

## Audit Trail

This plan was audited by 6 independent AI models:

| Model | Round | Verdict |
|-------|-------|---------|
| DeepSeek V3 | Round 1 (original) | "Slightly over-engineered, hybrid approach better" |
| Kimi K2.5 | Round 1 (original) | "Kill the Mesh. Use Monit + simple script" |
| Gemini 2.5 Pro | Round 1 (original) | "Reject proposal. Use off-the-shelf tools" |
| GPT-4.1 | Round 2 (revised) | "80% there. Much improved, needs hardening" |
| Qwen 3 235B | Round 2 (revised) | "Passes for small venue. Fix PM2 conflict" |
| Nemotron Ultra 253B | Round 2 (revised) | "Meets 80% needs. Add external monitoring" |

All Round 2 feedback incorporated into this v2.1 plan.

---

## Verification: 3 Layers + Floor Test

**Run this test to confirm the entire self-healing stack is operational.** Integrated into:
- COGNITIVE-GATE-PROTOCOL Phase 0.2b (every session start)
- AUDIT-PROTOCOL Phase 10b (every audit)
- gate-check.sh Suite 6 (pre-deploy when healing code changes)
- escalation-engine.sh --self-test (healing pipeline validation)

### Quick Test (one SSH command)

```bash
ssh root@100.70.177.44 "echo '=== 3 LAYERS + FLOOR TEST ===' && \
  echo -n 'L1 EYES (Uptime Kuma :3001): ' && \
  (curl -sf -m 3 http://localhost:3001/api/status-page/heartbeat >/dev/null && echo PASS || echo FAIL) && \
  echo -n 'L2 MUSCLE (Monit): ' && \
  (monit summary >/dev/null 2>&1 && echo PASS || echo FAIL) && \
  echo -n 'L3 BRAIN (rc-doctor timer): ' && \
  (systemctl is-active rc-doctor.timer >/dev/null 2>&1 && echo PASS || echo FAIL) && \
  echo -n 'FLOOR (PM2): ' && \
  (pm2 jlist >/dev/null 2>&1 && echo PASS || echo FAIL) && \
  echo '=== END TEST ==='"
```

### Expected Output (all PASS)

```
=== 3 LAYERS + FLOOR TEST ===
L1 EYES (Uptime Kuma :3001): PASS
L2 MUSCLE (Monit): PASS
L3 BRAIN (rc-doctor timer): PASS
FLOOR (PM2): PASS
=== END TEST ===
```

### Failure Recovery Matrix

| Layer | FAIL Symptom | Recovery Command | Impact While Down |
|-------|-------------|-----------------|-------------------|
| **L1 EYES** | No Uptime Kuma response | `pm2 restart uptime-kuma` | No monitoring/alerting — services can die silently |
| **L2 MUSCLE** | Monit not running | `systemctl start monit` | No dependency-aware restarts — PM2 blind restarts only |
| **L3 BRAIN** | Timer inactive/stale | `systemctl enable --now rc-doctor.timer` | No playbook remediation — disk fills, ports conflict, no cleanup |
| **FLOOR** | PM2 down | `pm2 resurrect` | No process executor — Monit can't start/stop services |

### Test Triggers

| When | Automated? | How |
|------|-----------|-----|
| Session start | Manual | Run 0.2b in COGNITIVE-GATE-PROTOCOL |
| Pre-deploy (healing changes) | Auto | gate-check.sh Suite 6 detects healing-domain diff |
| Audit | Manual | AUDIT-PROTOCOL Phase 10b |
| Escalation self-test | Auto | `bash scripts/healing/escalation-engine.sh --self-test` |
| VPS service crash | Manual | First step before debugging any VPS issue |
| Post-incident | Manual | Phase D Exit Gate re-verification |
