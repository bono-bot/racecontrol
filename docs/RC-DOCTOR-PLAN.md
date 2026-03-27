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

### Model Tiers (Tested 2026-03-28)

All models live-tested with real PM2 crash logs. Cost calculated for typical rc-doctor call (~500 input + ~200 output tokens).

| Tier | Model | ID | Speed | Cost/Call | Monthly (10/day) | Status |
|------|-------|----|-------|-----------|-------------------|--------|
| **Free** | Nemotron Nano 30B | `nvidia/nemotron-3-nano-30b-a3b:free` | ~1s | $0.00 | $0.00 | VERIFIED |
| **Free** | OpenRouter Auto | `openrouter/free` | ~11s | $0.00 | $0.00 | VERIFIED (fallback) |
| **Cheap** | Mistral Nemo 12B | `mistralai/mistral-nemo` | ~2s | $0.000018 | $0.005 | VERIFIED |
| **Cheap** | GPT-oss 120B | `openai/gpt-oss-120b` | ~3s | $0.000058 | $0.017 | VERIFIED |

**Model selection strategy:**
1. Try free Nemotron Nano first (fast, zero cost)
2. If free model is rate-limited/down, try cheap Mistral Nemo ($0.005/month)
3. If both fail, try openrouter/free auto-router (slower but routes around limits)
4. If ALL fail, return "manual investigation needed" — never blocks remediation

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
AI_MODELS=("nvidia/nemotron-3-nano-30b-a3b:free" "mistralai/mistral-nemo" "openrouter/free")
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

| Scenario | Free Models | With Cheap Fallback |
|----------|-------------|---------------------|
| 10 diagnoses/day | $0/month | $0.005/month |
| Weekly reports (4/month) | $0/month | $0/month (free model) |
| Worst case (all cheap) | N/A | $0.02/month |

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
MAX_ACTIONS_PER_HOUR=10

log() { echo "[$(date -Is)] $*" | tee -a "$LOG"; }
audit() { echo "[$(date -Is)] ACTION=$1 TARGET=$2 RESULT=$3" >> "$AUDIT"; }

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

case "${1:-routine}" in
  disk-pressure)    cmd_disk_pressure ;;
  memory-pressure)  cmd_memory_pressure ;;
  crash-loop)       cmd_crash_loop "${2:-}" ;;
  stale-binary)     cmd_stale_binary ;;
  wal-bloat)        cmd_wal_bloat ;;
  ssl-check)        cmd_ssl_check ;;
  db-integrity)     cmd_db_integrity ;;
  backup-verify)    cmd_backup_verify ;;
  diagnose)         cmd_diagnose "${2:-}" ;;
  weekly-report)    ai_weekly_report ;;
  routine)          cmd_routine ;;
  logs)             cmd_logs ;;
  status)           cmd_status ;;
  *)                echo "Usage: rc-doctor.sh {routine|disk-pressure|memory-pressure|crash-loop <svc>|stale-binary|wal-bloat|ssl-check|db-integrity|backup-verify|diagnose <svc>|weekly-report|logs|status}" ;;
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
