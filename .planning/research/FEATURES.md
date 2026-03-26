# Feature Research

**Domain:** Autonomous Bug Detection & Self-Healing — Scheduled, Unattended Fleet Operations
**Project:** Autonomous Detection Milestone — adding scheduled execution, config drift, log anomaly, cascade verification, self-healing escalation, and pipeline tests to existing Racing Point eSports infrastructure
**Researched:** 2026-03-26
**Confidence:** HIGH (based on direct inspection of existing codebase, foundation scripts, and audit protocol)

---

## Context: What Already Exists

This milestone adds to an already-substantial foundation. Understanding the boundary between "already built" and "to be built" is critical to scoping correctly.

| Component | What It Does | Status |
|-----------|-------------|--------|
| `audit.sh` | 60-phase fleet audit, parallel engine, delta tracking, auto-fix whitelist (3 fixes), Bono+WhatsApp notifications | SHIPPED (v23.0) |
| `auto-detect.sh` (James) | 6-step pipeline: Audit → Quality Gate → E2E → Cascade → Standing Rules → Report | FOUNDATION — exists, not scheduled |
| `bono-auto-detect.sh` (Bono) | Failover checks: venue server, cloud, fleet, Next.js apps, git sync. Delegates to James when alive. | FOUNDATION — exists, not scheduled |
| `audit/lib/fixes.sh` | Auto-fix engine: kill sentinels, kill orphan PS, restart rc-agent (whitelist-only) | SHIPPED (v23.0) |
| `audit/lib/notify.sh` | Dual-channel: Bono WS + INBOX.md + WhatsApp to Uday | SHIPPED (v23.0) |
| `audit/lib/delta.sh` | Delta tracking: compares current run to last run across 6 categories | SHIPPED (v23.0) |
| `audit/lib/suppress.sh` | suppress.json with per-pattern expiry for known-quiet items | SHIPPED (v23.0) |
| `comms-link test/run-all.sh` | 4-suite quality gate: contract, integration, syntax, security | SHIPPED (v18.2) |
| `relay/exec/run` + `relay/chain/run` | Bono exec relay — single commands and chained command sequences | SHIPPED (v18.0) |
| `fleet/health` + `fleet/exec` | Server-side fleet status and remote exec via rc-agent | SHIPPED |
| `auto-fix: is_pod_idle()` billing gate | Never interrupt billing sessions before auto-fix | SHIPPED (v23.0) |
| rc-sentry `/files` endpoint | Read files from pod filesystem remotely | SHIPPED |

### What the Foundation Scripts Currently Lack

`auto-detect.sh` exists but runs only when manually triggered. It covers:
- Audit protocol results (pass/fail counts from last run)
- Quality gate (comms-link test suite)
- E2E health (server, relay, exec round-trip, chain round-trip, Next.js apps)
- Cascade check (build drift, pod consistency, cloud-venue match, comms-link hash)
- Standing rules (unpushed commits, relay health)

It does NOT yet cover:
- Scheduled/autonomous execution (no Task Scheduler entry exists)
- Config drift detection (TOML values, bat content, env vars across 8 pods + server + cloud)
- Log anomaly detection (ERROR/PANIC scanning, crash loop detection, rate-based anomalies)
- Expanded cascade verification (schema sync, feature flag sync across environments)
- Self-healing escalation ladder (retry → Wake-on-LAN → cloud failover → Uday alert)
- Integration test suite for the autonomous pipeline itself

---

## Feature Landscape

### Table Stakes (Must Exist or Autonomy is Theater)

These features are the minimum for the system to be genuinely autonomous rather than a manually-triggered wrapper.

| Feature | Why Expected | Complexity | Dependencies on Existing |
|---------|--------------|------------|--------------------------|
| **Scheduled execution (Task Scheduler + cron)** | "Autonomous" detection that requires a human to trigger it is not autonomous. A schedule is the prerequisite for everything else. Without it, the pipeline is a convenience wrapper, not a system. | LOW | `auto-detect.sh` (James Task Scheduler entry), `bono-auto-detect.sh` (Bono cron 0 21 * * *). Both scripts exist — need registration. |
| **Config drift detection: TOML value validation** | `racecontrol.toml` has drifted silently before (SSH banner corruption, stale ws_connect_timeout=200ms caught in v23.0 audit). Detecting this autonomously prevents multi-hour silent failures. Key fields: `ws_connect_timeout`, `app_health` URLs, `process_guard.enabled`, `feature_flags` sync interval. | MEDIUM | Uses `rc-sentry /files` endpoint (exists) to read TOML from pods and server. Compare against expected values from canonical config. |
| **Config drift detection: bat file content** | Manual fixes regress on reboot because bat files are not kept in sync. The v25.0 FEATURES.md identified "bat drift detector" as P2. For an autonomous system, detecting bat drift is table stakes — if the bat is wrong, every reboot undoes every fix. | MEDIUM | `self_heal.rs` embeds canonical bat content (exists). `rc-sentry /files` can read deployed bat (exists). Diff comparison is the new piece. |
| **Log anomaly scanning: ERROR/PANIC detection** | The existing audit protocol checks service health but does NOT read application logs for error patterns. A crash loop (rc-agent restarting 5x in 10 min) is invisible to fleet health endpoints if recovery succeeds. Log scanning catches what health checks miss. | MEDIUM | Logs at `C:\RacingPoint\*.jsonl` on pods. `fleet/exec` to read via rc-sentry. New: grep-based anomaly extraction logic. |
| **Crash loop detection** | rc-agent restart storm (3 restarts in 10 min) triggers MAINTENANCE_MODE silently. The audit must detect this pattern in logs before MAINTENANCE_MODE is written, enabling early intervention. Rate: >=2 ERROR lines in last 15 min = anomaly. >=3 restart events in 10 min = crash loop candidate. | MEDIUM | Uses log anomaly scanning (above). Reads `MAINTENANCE_MODE` sentinel file existence via rc-sentry (already detectable via health endpoint `maintenance_mode_active` field). |
| **Self-healing escalation ladder** | The existing auto-fix engine has 3 approved fixes (kill sentinels, orphan PS, rc-agent restart). The milestone calls for: retry → Wake-on-LAN → failover → Uday alert. Without the escalation ladder, a pod that doesn't recover after one restart just sits unaddressed until Uday notices. | HIGH | WoL: existing MAC addresses in CLAUDE.md. Failover: `pm2 start racecontrol` on Bono (already in `bono-auto-detect.sh`). Uday WhatsApp: `notify.sh` (exists). New: retry counter, WoL trigger via `wakeonlan` or `etherwake`, escalation state tracking. |
| **Bono failover activation on James DOWN** | When James machine is offline, Bono must run its own detection independently. `bono-auto-detect.sh` already implements this logic but is not scheduled. Scheduling is required for the failover to actually fire. | LOW | Script exists. Needs cron registration on Bono VPS at `0 21 * * *` (2:30 AM IST). |
| **Pipeline result persistence** | Each autonomous run must write a structured result (JSON + Markdown) to a persistent location so that the next run can compare against it (delta tracking) and so Uday can review history. | LOW | `audit/lib/delta.sh` and `audit/lib/results.sh` already implement this pattern. Autonomous pipeline results should follow the same schema. |

### Differentiators (Not Required, but Meaningfully Better)

| Feature | Value Proposition | Complexity | Notes |
|---------|-----------------|------------|-------|
| **Config drift: env var validation** | `NEXT_PUBLIC_WS_URL` defaulted to `ws://localhost:8080` for multiple sessions before discovery. Autonomously checking that all Next.js `NEXT_PUBLIC_*` env vars in `.env.production.local` match expected venue values catches this class of bug before a session where customers see broken data. | MEDIUM | Reads `.env.production.local` from server via `fleet/exec`. Compare each key against expected values (defined in canonical config). |
| **Feature flag sync validation** | v22.0 added feature flags with WS sync across pods. A desync (pod running old flag value after server update) is invisible to health checks. Autonomously comparing pod-cached flag values against server truth catches sync failures before they affect billing. | MEDIUM | Requires new endpoint on rc-agent: `GET /api/v1/flags/summary` returning current cached values. Compare against server's flag values. Adds 1 endpoint, 1 comparison check. |
| **Schema sync validation (cloud-venue DB)** | Cloud and venue databases have drifted before (ALTER TABLE coverage gaps). Autonomously checking that both DBs have the same table columns catches schema drift before cloud sync breaks silently. | MEDIUM | New: run `PRAGMA table_info(billing_sessions)` on both DBs via exec, compare column names. Detectable without direct DB access if server exposes a schema-hash endpoint. |
| **Log anomaly: rate-based alerting** | Beyond crash loops, rate-based anomalies (e.g. 100+ violations/24h from process guard, which was the empty-allowlist indicator) provide early warning. Threshold: if `violation_count_24h > 50` on any pod, flag for review. | LOW | Already queryable via `GET /api/v1/guard/status` endpoint on server. Just needs to be checked in the pipeline with a threshold comparison. |
| **Suppression with confidence scoring** | The existing `suppress.json` suppresses known-quiet patterns. A differentiator is applying confidence scoring: patterns suppressed >30 days should trigger "is this still valid?" review rather than silent continuation. This prevents suppressions that outlive the underlying fix. | LOW | Extend `suppress.sh`. Add `last_validated` field. When a suppression entry is >30 days old, include it in the report as "suppression aging — validate". |
| **Pipeline self-test (integration tests)** | The pipeline itself is untested. An autonomous system that might have its own bugs is not trustworthy. A test suite that exercises each detection function against known-good and known-bad scenarios validates that the detection logic works before trusting the clean reports. | HIGH | New: `audit/test/test-auto-detect.sh` — mock server responses, mock log files with injected anomalies, verify correct classification. This is what turns the pipeline from "probably works" to "verified works". |
| **Idempotent run guard** | If two instances of `auto-detect.sh` run concurrently (e.g. cron fires while a manual run is in progress), they can interfere — particularly during auto-fix steps. A lock file (`/tmp/auto-detect.lock`) prevents concurrent runs. | LOW | Standard bash lock pattern: `flock -n /tmp/auto-detect.lock` or `mkdir /tmp/auto-detect.lock`. Trivially added, prevents a class of race conditions. |
| **Escalation cooldown tracking** | If Uday receives 5 WhatsApp messages in 30 minutes about the same pod, it erodes trust and creates alert fatigue. A cooldown file (per-pod, per-alert-type) ensures the same alert is not sent more than once per 4-hour window. | LOW | Extend `notify.sh` with per-alert cooldown state. Write `$RESULT_DIR/cooldown/$pod_N_type.ts` on alert send. Check timestamp before sending. |

### Anti-Features (Do Not Build)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|--------------|-----------------|-------------|
| **Autonomous binary deployment** | "If the system detects a stale build, just deploy the new binary automatically" | Binary deployment is a 7-step sequence with rollback dependency. An autonomous deploy that fails mid-swap leaves pods in MAINTENANCE_MODE. The billing gate (`is_pod_idle()`) is necessary but not sufficient — an automated deploy during a late-night session could still interrupt unexpectedly. | Keep deployment human-triggered. Autonomous system flags build drift and pages Uday/James, but never initiates a deploy. |
| **LLM-gated fix decisions** | "Use Ollama to decide which fix to apply" | Adds latency (Ollama inference is 2-5s per query), adds an external dependency to the critical detection path, and the LLM cannot verify whether a fix was actually safe in context. The safe-action whitelist is more reliable than LLM judgment for a system that runs at 2:30 AM. | Keep whitelist-only fixes. Extend the whitelist via deliberate human decision, not autonomous LLM judgment. |
| **Real-time anomaly streaming** | "Push anomalies to a WebSocket dashboard as they're detected" | Creates a permanent WebSocket channel that must be maintained, monitored for its own health, and secured. The autonomous pipeline runs once per day — streaming is overkill for a daily batch job. | Daily report + threshold-based WhatsApp alert. Streaming only makes sense when human operators are watching in real time. |
| **Predictive failure modeling** | "Use historical data to predict which pod will fail next" | Requires weeks of clean training data, a model that needs retraining as the fleet changes, and produces probabilistic outputs that are hard to act on. The fleet is only 8 pods. | Trend detection on delta data (3 consecutive WARNs → escalate) is sufficient and deterministic. |
| **Auto-remediation of TOML config drift** | "If racecontrol.toml has wrong values, auto-correct them" | TOML modification on a running server requires (1) stopping the server, (2) editing the config, (3) restarting. This is a 7-step process with rollback implications. An autonomous wrong-value correction could overwrite intentional config changes made by a human. | Report config drift with exact diff. Human applies fix. Auto-remediation only for sentinel files and process kills (existing whitelist). |
| **Pod-level log streaming** | "Stream all pod logs to a central log aggregator" | 8 pods × log volume = significant network overhead. The pods' `rc-agent.exe` already writes JSONL locally. Central aggregation requires a log ingestion service, storage, and retention policy — significant ongoing maintenance cost. | Point-in-time log reads during anomaly detection (existing rc-sentry `/files` endpoint). Only read logs when an anomaly is suspected, not continuously. |

---

## Feature Dependencies

```
[Scheduled execution — James Task Scheduler]
    └──enables──> [Config drift detection]
    └──enables──> [Log anomaly scanning]
    └──enables──> [Self-healing escalation ladder]
    └──enables──> [Pipeline result persistence]

[Scheduled execution — Bono cron]
    └──enables──> [Bono failover activation]
                      └──uses──> [venue server health check] (exists)
                      └──uses──> [pm2 failover] (exists in bono-auto-detect.sh)

[Log anomaly scanning: ERROR/PANIC detection]
    └──enables──> [Crash loop detection]
                      └──informs──> [Self-healing escalation ladder]
                      └──uses──> [MAINTENANCE_MODE sentinel check] (exists via fleet health)

[Self-healing escalation ladder]
    └──requires──> [Retry counter with persistence] (new)
    └──requires──> [WoL trigger] (new — wakeonlan tool)
    └──uses──> [cloud failover] (exists in bono-auto-detect.sh)
    └──uses──> [WhatsApp notify.sh] (exists)
    └──requires──> [Escalation cooldown tracking] (Differentiator)

[Config drift detection: bat content]
    └──uses──> [self_heal.rs canonical bat] (exists)
    └──uses──> [rc-sentry /files endpoint] (exists)
    └──enhances──> [Startup enforcement bat scanner from v25.0]

[Feature flag sync validation]
    └──requires──> [GET /api/v1/flags/summary on rc-agent] (new endpoint — 1 file change)
    └──uses──> [GET /api/v1/feature-flags on server] (exists)

[Pipeline self-test]
    └──uses──> [all detection functions] (depends on all above being stable first)
    └──requires──> [mock infrastructure fixtures] (new)

[Idempotent run guard]
    └──used by──> [Scheduled execution]
    └──prevents conflict with──> [Manual trigger]

[Suppression with confidence scoring]
    └──extends──> [suppress.sh] (exists)
    └──reads──> [suppress.json last_validated field] (new field)
```

### Dependency Notes

- **Scheduled execution is the critical path.** Every table stakes feature depends on it. Build and register the schedule first, dry-run for 48 hours before enabling live fixes.
- **Log anomaly scanning requires exec access to pod logs.** This uses `fleet/exec` → `rc-sentry /files` to read JSONL logs remotely. This path is verified working from the cascade check in `auto-detect.sh`. The new piece is the anomaly classification logic (grep patterns + rate thresholds).
- **Self-healing escalation ladder requires WoL to be tested before autonomous activation.** WoL is network-dependent (broadcast or unicast to MAC). All 8 pod MAC addresses are known (CLAUDE.md). WoL must be verified manually on one pod before adding it to the autonomous path.
- **Feature flag sync validation requires a new rc-agent endpoint.** This is the only new Rust code in the feature set. Keep it minimal: a GET handler returning a JSON map of flag name → current cached value. No other changes to rc-agent.
- **Pipeline self-test should be the last feature built.** It tests the pipeline as a whole. Build it after the detection functions are stable.

---

## MVP Definition

"MVP" for an autonomous system: the minimum set that runs reliably every night without human trigger and catches the categories of bugs that have burned the most time.

### Launch With (v1 — Core Autonomy)

- [ ] **Scheduled execution registered (James + Bono)** — without this, nothing is autonomous. James Task Scheduler daily at 02:30 IST, Bono cron daily at 02:30 IST (21:00 UTC). `--mode standard` to stay within 15-minute window.
- [ ] **Config drift detection: TOML key validation** — checks `ws_connect_timeout >= 600ms`, `app_health` URLs correct for admin/kiosk, `process_guard.enabled` value on all pods. These three fields have caused the most silent failures.
- [ ] **Config drift detection: bat file hash check** — compares deployed `start-rcagent.bat` on all pods against canonical. Flags divergence without auto-correcting. One of the most common regression causes.
- [ ] **Log anomaly scanning: ERROR/PANIC rate** — reads last 200 lines of `rc-agent-*.jsonl` on each pod via rc-sentry, counts ERROR/PANIC/CRITICAL entries in the last hour. Threshold: >5 in 1 hour = WARN, >20 = FAIL.
- [ ] **Crash loop detection** — checks `MAINTENANCE_MODE` sentinel file existence on each pod as part of the anomaly pass. Already available in fleet health `maintenance_mode_active` field — just needs to be checked and escalated.
- [ ] **Idempotent run guard** — flock on `/tmp/auto-detect.lock`. Prevents concurrent runs from cron and manual triggers interfering.
- [ ] **Escalation cooldown tracking** — per-pod, per-alert-type cooldown state. Prevents WhatsApp spam to Uday. Required before scheduling fires every night.

### Add After Validation (v1.x — Escalation Depth)

- [ ] **Self-healing escalation ladder: WoL tier** — after verifying retry doesn't recover a pod, send WoL magic packet. Add only after manual WoL test confirms it works for at least 2 pods.
- [ ] **Log anomaly scanning: crash loop pattern** — beyond rate counting, match specific patterns: `"restarting self"` appearing 3x within 10 min in JSONL timestamps. Requires timestamp parsing in bash.
- [ ] **Process guard violation rate check** — `GET /api/v1/guard/status` already returns `violation_count_24h`. Add threshold check: >50 = flag for empty-allowlist investigation.
- [ ] **Suppression confidence scoring** — add `last_validated` to suppress.json entries. Flag suppressions older than 30 days in the report.

### Future Consideration (v2+ — Verification Depth)

- [ ] **Feature flag sync validation** — requires new rc-agent endpoint. High value but requires Rust change + rebuild + fleet deploy. Defer until after v1 is stable.
- [ ] **Schema sync validation (cloud-venue DB)** — low-risk bash check but requires careful output parsing. Defer until after v1 is stable.
- [ ] **Env var validation for Next.js apps** — reads `.env.production.local` and checks NEXT_PUBLIC_ values. Trivial check but needs verification that the file path is predictable.
- [ ] **Pipeline self-test suite** — tests the detectors with injected anomalies. Build when the feature set is stable.

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|-------------------|--------------------| ---------|
| Scheduled execution (James Task Scheduler) | HIGH — nothing is autonomous without it | LOW | P1 |
| Scheduled execution (Bono cron) | HIGH — failover requires it | LOW | P1 |
| Idempotent run guard | HIGH — prevents concurrent run corruption | LOW | P1 |
| Escalation cooldown tracking | HIGH — prevents alert fatigue killing trust | LOW | P1 |
| Config drift: TOML key validation | HIGH — ws_connect_timeout + app_health drift caused incidents | MEDIUM | P1 |
| Config drift: bat file hash check | HIGH — single biggest regression cause | MEDIUM | P1 |
| Log anomaly: ERROR/PANIC rate | HIGH — catches crash storms health endpoints miss | MEDIUM | P1 |
| Crash loop detection (MAINTENANCE_MODE check) | HIGH — 3 pods silent for 1.5h incident | LOW | P1 |
| Self-healing escalation: WoL tier | MEDIUM — recovery without human needed for late night | HIGH | P2 |
| Log anomaly: crash loop pattern in JSONL | MEDIUM — more precise than sentinel file check | MEDIUM | P2 |
| Process guard violation rate check | MEDIUM — empty allowlist early warning | LOW | P2 |
| Suppression confidence scoring | MEDIUM — prevents stale suppressions masking bugs | LOW | P2 |
| Feature flag sync validation | HIGH — but requires Rust change + rebuild | HIGH | P3 |
| Schema sync validation | MEDIUM — SQLite drift has burned time before | MEDIUM | P3 |
| Env var validation (Next.js) | MEDIUM — NEXT_PUBLIC_ defaulting to localhost burned sessions | LOW | P3 |
| Pipeline self-test suite | HIGH long-term — verifies detectors work | HIGH | P3 |

**Priority key:**
- P1: Must have for launch — directly enables reliable autonomy or prevents trust-breaking failures (alert spam, concurrent corruption)
- P2: Should have — adds detection depth and recovery capability
- P3: Valuable when P1+P2 are stable and verified

---

## Incident-to-Feature Mapping

Each new feature traces to a specific incident where autonomous detection would have caught it earlier:

| Incident | Detection Gap | New Feature That Catches It |
|----------|-------------|----------------------------|
| `ws_connect_timeout=200ms` → all pods falling behind threshold | Audit fixed in v23.0 but could have caught it autonomously 48h earlier | Config drift: TOML ws_connect_timeout validation |
| `app_health` URLs wrong (admin port, kiosk basePath) | Audit caught, but ran manually | Config drift: TOML app_health URL validation |
| Bat file missing 8 bloatware kills → ConspitLink multiplied | No automated check existed | Config drift: bat file hash check |
| MAINTENANCE_MODE blocked 3 pods for 1.5h silently | Fleet health showed pods down but no sentinel-specific alert | Crash loop detection: MAINTENANCE_MODE existence check |
| Empty allowlist + 28,749 false violations/day for 2 days | `violation_count_24h: 100` visible in API, never checked | Process guard violation rate check |
| rc-agent crash storm → MAINTENANCE_MODE re-entry | Log showed restart events but nothing read the logs | Log anomaly: ERROR rate + crash loop pattern |
| WoL needed when pod offline during off-hours | Manual WoL required Uday to be present | Self-healing escalation: WoL tier |
| Uday WhatsApp spam during multi-pod incident | Same alert fired for same pod 4x in 30 min | Escalation cooldown tracking |

---

## Sources

- `scripts/auto-detect.sh` — direct inspection: confirmed 6 steps, identified gaps (no schedule, no config drift, no log scanning, no WoL, no escalation ladder)
- `scripts/bono-auto-detect.sh` — direct inspection: confirmed failover logic exists but is not scheduled on Bono VPS
- `scripts/AUTONOMOUS-DETECTION.md` — architecture doc for the foundation scripts
- `audit/lib/` — confirmed: fixes.sh (3 approved fixes), notify.sh (dual channel), delta.sh (6 categories), suppress.sh (with expiry)
- `CLAUDE.md` standing rules — 12 debugging rules naming specific incidents, each mapping to a detection gap
- `.planning/PROJECT.md` (v23.0, v23.1, v25.0 sections) — milestone context for what is shipped vs targeted
- `MEMORY.md` (shipped milestones, open issues) — pod MAC addresses, WoL prerequisite data, fleet IP table, pod incident history

---

*Feature research for: Autonomous Bug Detection & Self-Healing*
*Researched: 2026-03-26 IST*
