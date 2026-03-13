# Feature Research

**Domain:** Pod management reliability & connection hardening for a venue sim racing system
**Researched:** 2026-03-13
**Confidence:** HIGH (based on direct codebase inspection + archived Phase 5 research)

## Feature Landscape

### Table Stakes (Users Expect These)

These are features an operator of an 8-pod venue assumes work. Missing any of them means
the system cannot be trusted as the operational backbone of the venue.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| WebSocket stays connected during game launch | Staff see "Disconnected" flash in kiosk today — this is the primary reported pain point. Any supervision system must maintain reliable server-agent channels. | MEDIUM | Root cause: game launch consumes CPU/RAM briefly, stressing the WS connection. Fix: tune keepalive timers, suppress reconnect noise in kiosk. |
| Escalating restart cooldown per pod | Fixed cooldowns are table stakes for any process supervisor (systemd, supervisord, PM2 all do this). A flat 120s means a broken pod retries identically forever. | MEDIUM | 30s → 2m → 10m → 30m. Must be per-pod, not global. Resets on confirmed recovery. EscalatingBackoff struct in rc-common. |
| Post-restart health verification | Issuing a restart command and assuming success is insufficient. Every production supervisor (Kubernetes readiness probes, systemd ExecStartPost checks) verifies the service is actually healthy after restart. | MEDIUM | Check: process running (tasklist via pod-agent), WebSocket reconnected (state.agent_senders), lock screen port 18923 responsive. Timeout: 60s with checks at 5s/15s/30s/60s. |
| Email alert for persistent pod failures | Any monitored system alerts its operator when automation has given up. Without this, Uday has no way to know a pod needs physical intervention unless he watches the dashboard. | LOW | Shell-out to existing send_email.js (OAuth2 already configured). Rate-limited: 1 per pod per 30min, 1 venue-wide per 5min. |
| Coordinated restart between monitor and healer tiers | Two independent supervisors (pod_monitor + pod_healer) issuing concurrent restarts is a footgun present in any multi-tier supervision system. Coordination via shared state is standard practice. | MEDIUM | Share EscalatingBackoff in AppState. pod_monitor owns restarts; pod_healer focuses on diagnostics/cleanup. |
| Config validation at startup | rc-agent silently failing due to a missing or malformed config field is a deployment blocker. Every reliable daemon fails fast on bad config rather than discovering the problem mid-operation. | LOW | Validate all required fields at startup in rc-agent config.rs. Return descriptive error and exit code 1 if invalid — never start partially configured. |
| Clean process lifecycle during deploy | Deploying a new binary while the old process holds file locks or ports causes silent failures that are hard to diagnose. Idempotent kill-before-replace is table stakes for any deployment system. | LOW | Explicit kill with verification before binary replacement. Ordering: kill → wait 2s → verify dead (tasklist) → replace binary → start. |
| Consistent binary behavior across all 8 pods | A binary that works on Pod 8 but fails on Pod 3 due to environment differences (CWD, DLL, config path) is the most common deployment failure mode today. | LOW | Static CRT build already in place. Config validation at startup catches missing fields. Deploy to one pod first, verify, then roll to all 8. |
| Idempotent pod-agent commands with clear status | A remote exec endpoint that returns HTTP 200 regardless of whether the command succeeded is dangerous. Callers need actionable exit codes. | LOW | pod-agent /exec must return non-zero or include failure in body when command fails. Callers (pod_monitor, deployment scripts) must check and log the result. |
| Kiosk shows stable connection state during operations | The "Disconnected" flash during game launch is the visible symptom. A staffed kiosk that flickers connectivity undermines confidence in the system. | MEDIUM | Debounce WebSocket disconnect events in kiosk UI. Only show "Disconnected" after 5–10s of confirmed absence, not on first drop. |

### Differentiators (Competitive Advantage)

These go beyond the minimum. For a bespoke venue management system, these are what
make it operationally excellent rather than merely functional.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Aggregated multi-pod email alerts | If a network switch reboots and all 8 pods go offline, one consolidated email listing all affected pods is far more useful than 8 separate alerts. Reduces alert fatigue. | MEDIUM | Venue-wide 5-minute window. If 2+ pods go down simultaneously, aggregate into single email. Requires cross-pod state tracking in EmailAlerter. |
| Partial recovery classification | Distinguishing "process running, WebSocket connected, but GUI in Session 0" from "process not running" enables smarter responses. Session 0 recoveries are acceptable; full failures need manual intervention. | MEDIUM | Post-restart verification returns a typed result: FullRecovery / PartialRecovery(reason) / Failed. Logs and email content reflect the distinction. |
| Activity log continuity across restarts | Today, restarting rc-agent loses the in-memory activity log context. Persisting recent events to a small local file means the log survives pod restarts and provides richer debugging context. | HIGH | Scope risk: this touches rc-agent's activity_log.rs substantially. Likely a future milestone feature, not this one. |
| Deployment dry-run mode | Before committing to a full 8-pod deploy, a dry-run that validates config files and reports what would change reduces the risk of a botched rollout. | MEDIUM | A --dry-run flag for the deploy script that checks binary compatibility, config validity, and process state without changing anything. |
| Self-healing rate limit tuning via config | The cooldown steps (30s/2m/10m/30m) being configurable in racecontrol.toml means Uday can tune behavior for a busy Saturday vs a quiet Tuesday without a code change. | LOW | WatchdogConfig struct fields. Already planned in WD-06 requirement. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Restart rc-agent on every WebSocket disconnect | Seems like the obvious fix for "connection dropped" | WebSocket drops are transient and self-heal within seconds. Restarting rc-agent on every drop creates a restart storm when the network hiccups, interrupting active billing sessions and game play. | Tune WebSocket keepalive/ping intervals. Use reconnect logic with short backoff on the rc-agent side. Only restart when heartbeat goes stale (6s+ dead, current threshold is correct). |
| Real-time process monitoring with sub-second polling | Seems like better visibility | Polling pod-agent /exec at sub-second intervals to check process state generates constant network traffic and pod-agent load, and finds problems no faster than the existing 10s heartbeat cycle. | UDP heartbeat at 6s staleness detection is already faster than needed. Sub-second polling adds cost with no reliability benefit. |
| Automatic binary rollback on failed deploy | Instinctively appealing for reliability | Adds significant deployment complexity (storing previous binary, detecting "failure", triggering rollback), and the current failure modes (binary won't start, config mismatch) are better caught by config validation and test-on-Pod-8-first discipline rather than automated rollback. | Deploy to Pod 8 first. If it fails, fix the binary/config before rolling to other pods. Manual rollback via pendrive is fast enough for an 8-pod venue. |
| SNMP or Prometheus metrics scraping | Looks professional and matches enterprise patterns | Adds a new protocol, new dependencies, and a metrics infrastructure to maintain. The venue has 8 pods, not 800 servers. The existing activity log + dashboard + email alerts cover all actionable signals. | Structured tracing (already using tracing crate) to files. Dashboard aggregates state. Email for actionable alerts. This is sufficient for the scale. |
| Watchdog that restarts during active billing | Seems safer to restart a misbehaving pod immediately | Interrupts a paying customer's session mid-game. The current billing guard (check active billing before restart) is correct and must be preserved. | Let the pod finish its session. If the pod is in a truly broken state during billing, surface it in the dashboard as AssistanceNeeded for staff to handle manually. |

## Feature Dependencies

```
[Escalating Backoff struct (rc-common)]
    └──required by──> [pod_monitor escalating cooldown]
    └──required by──> [pod_healer escalating cooldown]
    └──required by──> [Shared backoff state in AppState]

[Shared backoff state in AppState]
    └──required by──> [Coordinated restart (monitor + healer)]

[Post-restart health verification]
    └──requires──> [pod-agent /exec idempotency + clear status]
    └──triggers──> [Email alert on verification failure]

[Email alert module]
    └──requires──> [send_email.js on rc-core host (server .23)]
    └──requires──> [Node.js installed on server .23]
    └──enhanced by──> [Aggregated multi-pod alerts]
    └──enhanced by──> [Rate limiting (per-pod + venue-wide)]

[Config validation at startup]
    └──enables──> [Consistent deploy behavior across 8 pods]

[WebSocket connection resilience]
    └──enables──> [Kiosk stable connection state]

[Clean process lifecycle]
    └──required by──> [Consistent deploy behavior across 8 pods]
```

### Dependency Notes

- **Escalating Backoff requires nothing new:** It is a pure struct in rc-common with no external deps. It is the correct starting point for any wave.
- **Email alerts require Node.js on server .23:** This is an open question from the archived research. Must be verified before implementing email_alerts.rs. If Node.js is absent, install it — do not architect around it.
- **Post-restart verification depends on idempotent pod-agent commands:** If /exec returns ambiguous status, verification cannot be trusted. Fix pod-agent status reporting before wiring up verification logic.
- **Kiosk stability depends on WebSocket resilience:** The debounce fix is a frontend change in the Next.js kiosk. It is independent of Rust changes and can be done in parallel.
- **Aggregated alerts conflict with simple per-pod rate limiting:** You cannot have both a per-pod 30min cooldown AND venue-wide 5min aggregation independently — they must share state in EmailAlerter. Design EmailAlerter to hold both per-pod last_sent and a global last_venue_email timestamp.

## MVP Definition

### Launch With (v1 — this milestone)

Minimum set to eliminate the primary pain points: crash loops, silent failures, and deployment inconsistency.

- [ ] EscalatingBackoff struct in rc-common with unit tests — foundational, everything else depends on it
- [ ] pod_monitor uses EscalatingBackoff (30s → 2m → 10m → 30m) — eliminates crash loop restart spam
- [ ] pod_healer uses same shared EscalatingBackoff — eliminates duplicate restarts
- [ ] Post-restart verification task (process + WebSocket + lock screen, 60s window) — catches silent startup failures
- [ ] Email alert on verification failure or max escalation, rate-limited — Uday gets notified without watching dashboard
- [ ] Config validation at rc-agent startup, fail-fast on bad config — eliminates silent config mismatch deploys
- [ ] WebSocket keepalive tuning + kiosk disconnect debounce — eliminates "Disconnected" flash during game launch

### Add After Validation (v1.x)

- [ ] Aggregated multi-pod email alerts — add when the first real network-wide outage triggers duplicate emails
- [ ] Partial recovery classification (Session 0 vs full failure) — add when Session 0 restarts generate false alerts
- [ ] Deployment dry-run mode — add if rollout errors recur after v1

### Future Consideration (v2+)

- [ ] Activity log persistence across restarts — touches rc-agent core significantly; defer to its own milestone
- [ ] Configurable cooldown step tuning in racecontrol.toml — low priority, defaults will be correct for this venue

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| EscalatingBackoff struct | HIGH | LOW | P1 |
| pod_monitor escalating cooldown | HIGH | LOW | P1 |
| pod_healer shared backoff | HIGH | LOW | P1 |
| Config validation at startup | HIGH | LOW | P1 |
| Post-restart health verification | HIGH | MEDIUM | P1 |
| WebSocket keepalive + kiosk debounce | HIGH | MEDIUM | P1 |
| Email alerts (rate-limited) | HIGH | MEDIUM | P1 |
| Idempotent pod-agent commands | MEDIUM | LOW | P1 |
| Clean process lifecycle for deploys | MEDIUM | LOW | P1 |
| Aggregated multi-pod email | MEDIUM | MEDIUM | P2 |
| Partial recovery classification | MEDIUM | LOW | P2 |
| Deployment dry-run | LOW | MEDIUM | P3 |
| Activity log persistence | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for this milestone
- P2: Should have, add when P1s are verified working
- P3: Nice to have, future milestone

## Competitor Feature Analysis

This is a bespoke system, not a commercial product. The relevant comparison class is
general-purpose process supervisors that inform what "table stakes" looks like in the domain.

| Feature | systemd / supervisord (Linux) | PM2 (Node.js) | Our Approach |
|---------|-------------------------------|----------------|--------------|
| Escalating restart backoff | systemd: StartLimitBurst + StartLimitIntervalSec. supervisord: startretries + backoff. Both support escalation. | PM2: --exp-backoff-restart-delay (exponential). | EscalatingBackoff struct with fixed step table (30s/2m/10m/30m). Step table is easier to reason about and tune than exponential formula. |
| Post-restart health check | systemd: Type=notify + WatchdogSec + NotifyAccess. supervisord: no built-in health check. | PM2: no built-in health probe beyond process alive. | Custom verification task: process + WebSocket + lock screen. More specific than generic process-alive check because rc-agent has application-level health signals. |
| Alert on failure | systemd: OnFailure= unit. supervisord: eventlistener + mailer. PM2: pm2-notify or webhook. | PM2: pm2-slack, pm2-telegram integrations. | Shell-out to existing send_email.js. Reuses established Gmail OAuth2 credentials. No new infrastructure. |
| Multi-tier supervision | Not standard; usually single supervisor per service. | Not standard. | 3-tier (watchdog.bat + pod-agent → pod_monitor → pod_healer). Already implemented; this milestone coordinates the tiers rather than adding more. |
| Billing-aware restart guard | Not applicable to general process supervisors. | Not applicable. | Custom guard: never restart pod with active billing session. Venue-specific differentiator. |

## Sources

- **Codebase (HIGH confidence):** pod_monitor.rs, pod_healer.rs, udp_heartbeat.rs, watchdog.bat, pod-agent/src/main.rs, config.rs, send_email.js — direct inspection
- **Archived research (HIGH confidence):** .planning/archive/hud-safety/phases/05-watchdog-hardening/05-RESEARCH.md — thorough Phase 5 research with code examples and pitfall analysis
- **PROJECT.md (HIGH confidence):** Active requirements list, constraints, out-of-scope items
- **MEMORY.md (HIGH confidence):** Session 0 issue, deployment rules, network map, pod architecture
- **systemd docs (MEDIUM confidence):** Restart policy and health check patterns used for table stakes comparison
- **supervisord docs (MEDIUM confidence):** Industry standard process supervisor used for comparison baseline

---
*Feature research for: RaceControl Reliability & Connection Hardening*
*Researched: 2026-03-13*
