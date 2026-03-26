# Pitfalls Research

**Domain:** Autonomous bug detection and self-healing for an existing fleet management system
**Researched:** 2026-03-26
**Confidence:** HIGH — all pitfalls drawn from documented past incidents in this exact codebase (CLAUDE.md standing rules, PROJECT.md, MEMORY.md audit records). No hypothetical pitfalls.

---

## Critical Pitfalls

### Pitfall 1: Recovery Systems Fighting Each Other (Infinite Restart Loop)

**What goes wrong:**
Multiple independent recovery systems — self_monitor, rc-sentry watchdog, server pod_monitor, WoL, and now auto-detect.sh — each see a pod as "down" and independently act to bring it back up. One system restarts rc-agent, which triggers the MAINTENANCE_MODE sentinel, which blocks the restart, which causes the system to look "down" again, which triggers WoL, which boots a pod that immediately re-enters MAINTENANCE_MODE.

**Why it happens:**
Each subsystem was built independently with no shared state about why a pod is offline. They see the same symptom (pod not responding) but have no coordination layer. Auto-detect.sh adding a 7th actor into this mix without coordination is the classic "one more system with no awareness of the others" mistake. This exact loop (self_monitor + rc-sentry + WoL) took 45 minutes to diagnose in a documented incident.

**How to avoid:**
- Auto-detect.sh must read OTA_DEPLOYING, MAINTENANCE_MODE, and GRACEFUL_RELAUNCH sentinels before triggering any fix action
- Any fix that restarts a service must check all existing recovery actors' state first (is rc-sentry already handling it? is WoL already triggered?)
- Introduce a shared lock file: `C:\RacingPoint\AUTO_DETECT_ACTIVE` — prevents rc-sentry and pod_monitor from acting while auto-detect is mid-repair
- Recovery intent must be logged to RecoveryIntentStore so any actor can see what others are doing

**Warning signs:**
- Pod cycling between connected and disconnected at regular intervals (every 10-30s)
- MAINTENANCE_MODE appears on multiple pods simultaneously after an auto-detect run
- Fleet health shows rapid state changes without any manual intervention
- rc-sentry logs show "restarting" for a pod that auto-detect just tried to heal

**Phase to address:**
Phase 1 (auto-detect.sh core pipeline) — sentinel-awareness must be in the first version, not added later.

---

### Pitfall 2: Alert Fatigue — Too Many WhatsApp Messages to Uday

**What goes wrong:**
Auto-detect runs nightly, detects 3-5 non-critical drift items (expected log rotation, minor config deltas, the 6 QUIET venue-closed findings from every v23.0 audit run), and sends a WhatsApp message for each. After a week, Uday ignores all messages. When a real critical failure arrives, it is also ignored.

**Why it happens:**
The audit framework generates findings for everything including items already marked QUIET or INFO. If the escalation threshold is "anything that didn't auto-fix," non-critical residuals generate noise every night. The existing v23.0 audit already generates 6 QUIET findings at every run as a baseline.

**How to avoid:**
- WhatsApp escalation ONLY for: (a) items that were attempted but auto-fix failed to verify, (b) items classified CRITICAL with no available auto-fix, (c) cascade failures (3+ pods affected simultaneously)
- Never send WhatsApp for QUIET/venue-closed findings — they are always expected
- Collapse multiple findings into one digest: "3 issues found, 2 auto-fixed, 1 needs attention: [item]"
- Track "same finding N nights in a row" — first occurrence: auto-fix attempt + notify; subsequent occurrences: notify at most once per week until resolved
- Gate: if 0 unfixed critical items, send NO message (stay silent on a clean run)

**Warning signs:**
- Uday stops responding to WhatsApp messages that were previously answered
- Daily message count from auto-detect exceeds 2
- Messages arriving during business hours (08:00-23:00 IST)
- Every message body contains "no action required" but a message was still sent

**Phase to address:**
Phase 1 (escalation logic) — silence conditions must be correct before the first scheduled run.

---

### Pitfall 3: Config Drift False Positives from Intentional Per-Pod Differences

**What goes wrong:**
Config drift detection compares expected vs actual values, but some values are intentionally different between pods (pod_number, pod_ip, tailscale IP, pod-specific feature flags). The checker flags these as drift, generating 8 findings per run (one per pod), and either overwhelms logs or trains the team to ignore all drift alerts.

**Why it happens:**
Config drift checkers typically diff against a single "golden config" — but the golden config has no per-pod overrides. The Racing Point system has pod_number, IP, and MAC fields that legitimately differ across all 8 pods. Additionally, OTA canary pods may intentionally run a different binary version.

**How to avoid:**
- Define a drift schema separating shared keys (ws_connect_timeout, app_health URLs) from per-pod keys (pod_number, ip, mac)
- Shared keys: any deviation from expected is drift
- Per-pod keys: deviation from that pod's expected value is drift; deviation from other pods is expected and not flagged
- OTA sentinel check: if OTA_DEPLOYING exists for a pod, skip binary version drift checks for that pod
- suppress.json with expiry for known-acceptable differences (document the reason + expiry date)

**Warning signs:**
- Drift report shows the same 8 findings every run with pod-specific values (pod_number: 1, pod_number: 2, etc.)
- suppress.json accumulates entries for pod_number, pod_ip, or mac_address
- Delta tracking shows 0 new findings after the first run because everything was suppressed

**Phase to address:**
Phase 2 (cascade engine and config drift detection).

---

### Pitfall 4: Log Anomaly Detection Sensitivity Miscalibration

**What goes wrong:**
Tuned too sensitively: flags every WARN log as an anomaly. The racecontrol JSONL logs contain hundreds of expected warnings during normal operation (WS reconnect on pod boot, telemetry timeout when no game is running, billing idle threshold hits). Alternatively, tuned too loosely: misses the rc-agent process guard empty-allowlist pattern that generated 28,749 violations/day for 2 days without detection.

**Why it happens:**
Log anomaly detection without a baseline is pattern matching against noise. Without knowing the "normal warn rate" per source, anything above zero looks like an anomaly. The 28,749/day incident went undetected because no monitoring existed — the same gap will appear again if the anomaly detector is too conservative.

**How to avoid:**
- Baseline first: run 7 days of silent observation, compute p95 warn/error rate per source (rc-agent, racecontrol, rc-sentry)
- Anomaly threshold: rate meaningfully above p95, not any non-zero rate
- For launch: focus on specific high-value patterns rather than rate-based detection: `MAINTENANCE_MODE written`, `empty allowlist loaded`, `violation_count spiking beyond 100/hr`, `spawn() succeeded but health check failed`
- Pattern-based triggers are more reliable than rate-based for this system in its first iteration

**Warning signs:**
- Log anomaly fires every night on the same WARN messages (false positive loop established)
- Log anomaly never fires even after manually injecting known-bad state into a test pod
- Anomaly log is larger than the audit log it's meant to summarize

**Phase to address:**
Phase 2 (expanded auto-fix engine, log anomaly detection).

---

### Pitfall 5: Cascade Fixes Targeting the Symptom, Not the Cause

**What goes wrong:**
Cascade engine detects that Pod 3 build_id differs from Pods 1, 2, 4-8. Auto-fix redeploys Pod 3. But the real situation: Pod 3 is mid-OTA from a scheduled canary window and the binary swap is in progress. The auto-fix interrupts the OTA, starts a concurrent download to `rc-agent-new.exe`, corrupts the partial file (two writers), and Pod 3 enters MAINTENANCE_MODE. This exact failure mode (OTA_DEPLOYING not checked) is documented in the OTA standing rules.

**Why it happens:**
Cascade engine sees "pod differs from fleet" and treats alignment as always safe. It has no model of "is this pod intentionally in a different state right now?" The system has well-defined sentinel files for exactly this purpose but they are not automatically checked by new actors.

**How to avoid:**
- Before any cascade fix: check OTA_DEPLOYING sentinel, RecoveryIntentStore entry, recent deploy timestamps in LOGBOOK.md
- Pod 8 (canary by convention) is explicitly excluded from cascade homogenization — it is allowed to differ
- Cascade fixes run in dry-run mode by default: log what would be fixed, require explicit `--apply` flag
- 5-minute observation window before acting: if the difference resolves itself, no fix needed
- The auto-detect pipeline's cascade step outputs a proposed fix list; the apply step is separate and gated

**Warning signs:**
- Cascade fix triggered during a known deploy window
- Pod 8 is constantly being "corrected" to match the rest of the fleet
- Auto-fix log shows the same pod being fixed 2+ nights in a row (symptom returns each time)

**Phase to address:**
Phase 2 (cascade engine) — dry-run-by-default must be in from the start.

---

### Pitfall 6: Scheduled Tasks Running During Venue Operations

**What goes wrong:**
Daily 2:30 AM IST run includes audit phases that check pod connectivity. Auto-detect sends WoL to wake a pod for health verification. Pod boots, finds no billing session, idles until morning. Staff arrive to find all 8 pods already powered on from the 2:30 AM check — power wasted, session timers exposed.

Alternatively: venue has a late event (midnight racing league). Auto-detect runs at 2:30 AM while billing sessions are active. The idle gate check is based on fleet health aggregate — misses the active session on Pod 3 because Pod 3 is checked last. Auto-detect applies a restart-based fix to Pod 3 mid-race.

**Why it happens:**
Fixed schedule ignores the venue's operational calendar. Racing venues have irregular hours — late nights on weekends, early close on weekdays, private event bookings. The billing session check must cover ALL pods individually, not the aggregate fleet status.

**How to avoid:**
- `has_active_billing_session()` checked against EACH pod individually before ANY pod-touching action
- WoL actions are OFF by default in auto-detect — pods are not woken for health checks
- Venue hours config: simple `venue_open_until: "01:00"` — skip pod-touching phases if within 2 hours of close
- If any billing session is active anywhere in the fleet, run server-only checks only; skip all pod-touching phases
- Bono-side cron checks James-side AUDIT_RUNNING sentinel before starting its own run

**Warning signs:**
- Pods showing uptime of 4-6 hours when staff expect them to be powered off at venue open
- Billing session started during an auto-detect run (check billing DB timestamps vs audit log timestamps)
- Auto-detect log timestamps overlap with known late-night events

**Phase to address:**
Phase 1 (scheduling and idle gate) — billing session check per-pod must gate ALL pod actions, not just the aggregate.

---

### Pitfall 7: Dual-AI Race Conditions (James and Bono Acting Simultaneously)

**What goes wrong:**
James auto-detect.sh runs at 2:30 AM IST. Bono bono-auto-detect.sh runs at 2:30 AM IST (cron). Both detect build drift on Pod 5. Both trigger deploy chains. Two concurrent downloads write to `C:\RacingPoint\rc-agent-new.exe` simultaneously — file is corrupted or one deploy wins the rename race while the other fails silently, leaving Pod 5 with a truncated binary and no rollback.

Second scenario: Bono failover activates because James's machine has a 15-minute network blip. Bono starts a cascade fix. James comes back online, auto-detect.sh also starts. Two independent fix sets applied to overlapping pods.

**Why it happens:**
Two autonomous agents with the same goal will collide on shared resources unless they coordinate. The comms-link relay is the correct coordination channel but only if both agents check it before acting — the current foundation (bono-auto-detect.sh) does not yet have this coordination protocol.

**How to avoid:**
- Global mutex via comms-link relay: before starting any fix, post AUTO_DETECT_LOCK to the relay; if the lock already exists from the other agent, abort and log
- Bono failover activation requires confirmed offline status, not just "no response for N minutes" — Tailscale `node status` must show James as offline
- Stagger schedules: James at 2:30 AM IST, Bono at 3:30 AM IST; if James's run posts completion to INBOX.md before 3:30 AM, Bono skips its run
- Write `AUTO_DETECT_ACTIVE` sentinel with agent identity (james/bono) and timestamp; both agents check before starting

**Warning signs:**
- Both James and Bono logs show deploy actions for the same pod within the same 10-minute window
- Pod enters MAINTENANCE_MODE shortly after an auto-detect run (truncated binary from concurrent write)
- comms-link relay shows two AUTO_DETECT sessions simultaneously active

**Phase to address:**
Phase 3 (Bono failover) — coordination protocol must be defined and implemented before Bono failover is enabled.

---

### Pitfall 8: MAINTENANCE_MODE Permanently Blocking Auto-Fix Cycles

**What goes wrong:**
Auto-detect triggers an auto-fix that restarts rc-agent on a pod. The restart fails (server is briefly processing another chain, network blip, 30-second window). rc-agent counts 3 failed restarts in 10 minutes and writes MAINTENANCE_MODE. Auto-detect sees the pod as failed on next check, tries another fix, but MAINTENANCE_MODE blocks all restarts. Auto-detect reports "unfixed" every night indefinitely until a human manually clears the sentinel.

**Why it happens:**
MAINTENANCE_MODE was designed as a human-intervention gate to prevent restart storms. Auto-detect is a new actor that can trigger the conditions that write MAINTENANCE_MODE without a human in the loop to clear it. v17.1 added 30-minute auto-clear but this must be verified active before relying on it.

**How to avoid:**
- Verify v17.1 30-minute auto-clear is active before shipping auto-detect
- Auto-detect must track whether IT caused MAINTENANCE_MODE (its fix triggered the restart threshold)
- If auto-detect caused MAINTENANCE_MODE: wait 35 minutes for auto-clear + verify, then retry once, then escalate to Uday
- Never retry a failed fix more than once in the same run cycle without waiting for MAINTENANCE_MODE to clear
- MAINTENANCE_MODE written during auto-detect's own fix attempt = immediate WhatsApp to Uday (auto-detect failed, human needed)

**Warning signs:**
- Pod stuck in MAINTENANCE_MODE with timestamp matching auto-detect run time
- Auto-detect reports "unfixed" for the same pod multiple consecutive nights
- MAINTENANCE_MODE timestamp is 2:31-2:38 AM IST (within auto-detect window)

**Phase to address:**
Phase 1 (auto-fix engine) — MAINTENANCE_MODE awareness required before first fix attempt.

---

### Pitfall 9: Process Guard Empty Allowlist Window After Auto-Deploy

**What goes wrong:**
Auto-detect deploys a new rc-agent binary to a pod. Pod restarts rc-agent. At restart, rc-agent fetches the process guard allowlist from the server — but the server is briefly busy responding to the deploy chain, returns a timeout, rc-agent falls back to the empty default allowlist. Process guard enabled + empty allowlist = 28,749 false violations per day. If auto-detect deploys 4 pods in parallel, all 4 have the empty allowlist window simultaneously, creating a burst on the server.

**Why it happens:**
The periodic re-fetch (every 300 seconds) eventually corrects this, but the 0-300 second window after an auto-deploy is vulnerable. This is a known incident pattern in this codebase — it occurred because all 8 pods booted while the server was briefly down during a restart.

**How to avoid:**
- After any rc-agent deploy, verification step must include: `GET /api/v1/guard/whitelist/pod-{N}` response must be non-empty
- Include a 30-second delay after pod restart before running post-deploy verification (gives periodic re-fetch time to run)
- If allowlist is empty post-deploy, trigger manual re-fetch via exec before marking deploy complete
- Never deploy more than 2 pods concurrently to avoid overwhelming the server during allowlist fetch burst

**Warning signs:**
- `violation_count_24h` spikes immediately after an auto-deploy run
- All violations on a recently-deployed pod involve known-good processes (svchost.exe, rc-agent.exe)
- Allowlist endpoint returns 0 entries for a pod that just restarted

**Phase to address:**
Phase 1 (auto-fix engine) — post-deploy verification must include allowlist check before marking pod fix as complete.

---

### Pitfall 10: SSH Output Corrupting Config Files via Auto-Fix

**What goes wrong:**
The cascade engine detects that `racecontrol.toml` on the server has a stale `ws_connect_timeout` value. Auto-fix SSH-es to the server and writes the corrected line by redirecting SSH output. The SSH banner (post-quantum warning, MOTD, OpenSSH version line) is captured along with the command output and prepended to the config file. TOML parser rejects the file from line 1. `load_or_default()` silently falls back to empty defaults. Process guard runs with 0 allowed entries for hours with no visible error — because the parse error occurs before logging is initialized.

**Why it happens:**
This exact incident occurred on 2026-03-24 (documented in CLAUDE.md standing rules). Auto-fix scripts written to correct config drift may repeat this mistake if the prohibition on SSH output piping is not enforced as a hard constraint in the fix action framework.

**How to avoid:**
- Config fixes MUST use the dedicated ConfigPush WebSocket channel (CP-01) or the `/api/v1/config` REST endpoint — never SSH output redirect
- SCP for file transfers only; exec endpoint for operational commands only; API for config changes only
- After any config fix, validate file integrity: `head -1 racecontrol.toml | grep -q '^\[' || alert "config corrupted"`
- Add a "config fix type" field to the approved-fixes whitelist — only `api_call` and `scp_file` types are permitted, never `ssh_exec_redirect`

**Warning signs:**
- racecontrol.toml has non-TOML content on its first line after an auto-detect run
- Server starts returning empty/default responses for config-dependent features (process guard count drops to 0)
- No tracing errors visible despite config-dependent features behaving incorrectly (parse error before logging init)

**Phase to address:**
Phase 2 (config drift detection and auto-fix) — config update path enforced as API-only from the first implementation.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| No idle gate on first implementation | Simpler code, faster to ship | First scheduled run interrupts an active billing session | Never in production |
| Single WhatsApp per finding (no digest) | Easier notification logic | Alert fatigue in week 1 — Uday ignores all messages including critical ones | Never after the first run |
| Hardcode 2:30 AM with no venue hours awareness | Fast to implement | Conflicts with late venue events, no recovery if event runs late | MVP only with documented risk and manual override |
| No dry-run mode for cascade fixes | Faster implementation | First cascade run applies fixes to wrong pods | Never — dry-run is always the default first mode |
| Trust `spawn().is_ok()` for fix verification | One line of code | Silent non-start; pods appear fixed but aren't — documented incident where all 3 launch methods returned Ok but all silently failed | Never |
| Skip coordination check between James and Bono | Simpler individual scripts | First time both run simultaneously corrupts a binary | Acceptable only in detect-only (no-fix) mode |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| comms-link relay exec | Fire-and-forget exec assuming it ran | Use `/relay/exec/run` (synchronous) which returns the result; verify the result content, not just HTTP 200 |
| WhatsApp Evolution API | Call directly from James auto-detect.sh via venue tunnel | Route through Bono VPS Evolution API (standing rule: marketing/alerts via Bono, not venue tunnel) |
| rc-sentry exec endpoint | Use to write config files | SCP for file transfers only; exec is for operational commands only |
| Windows Task Scheduler | Assume interactive session is available | Non-interactive context: no GUI, no user profile, absolute paths everywhere, no `timeout` command (use `ping -n N 127.0.0.1`) |
| OTA pipeline | Auto-detect triggers during OTA_DEPLOYING window | Always read `C:\RacingPoint\OTA_DEPLOYING` before any pod-touching action |
| Bono cron + James task | Same schedule, no coordination | Stagger by 60 minutes minimum; share run completion status via comms-link INBOX.md |
| audit.sh integration | Call with `--auto-fix` always | Call in detect-only mode first; apply fixes selectively from the approved-fixes whitelist |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full 60-phase audit on every nightly run | 8-minute run blocks all other scheduled operations | Use `--mode quick` for nightly; full mode only on weekend maintenance windows | When full mode is added to the default nightly schedule |
| Parallel deploys to all 8 pods simultaneously | Server overload during allowlist fetch burst, corrupted concurrent binary writes | Max 2 concurrent pod deploys with 30-second stagger | First time 5+ pods need the same fix in one run |
| Log anomaly scanning full JSONL history | Scan time grows linearly with log age (30+ days of rolling files) | Scan only last 24h of JSONL using timestamp filter on `racecontrol-*.jsonl` filename pattern | After 30 days of log accumulation |
| Bono VPS Tailscale check via SSH for all pods | 10s timeout per pod, 8 pods sequential = 80s minimum | Use fleet health API for all-pod status in one call; SSH only for pods unreachable via HTTP | At 8 pods with intermittent Tailscale connectivity |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Auto-fix executes shell commands derived from audit output | A crafted pod response could inject commands into the fix step | All auto-fix actions are whitelist-only from approved-fixes.json; audit output is data, never eval'd as commands |
| COMMS_PSK passed as environment variable | PSK visible in process list via `ps aux` or Windows task manager | Read PSK from a chmod-600 file; never pass via command-line argument |
| Auto-detect commits and pushes config changes to git unattended | Could push corrupted state after a failed partial fix | Limit auto-commits to audit report files only; never auto-push binary, config, or toml changes |
| WhatsApp messages include internal IPs and build hashes | Leaks infrastructure details to anyone with phone access | Sanitize messages: "Pod 5 needs attention" not "Pod 5 (192.168.31.86) build 4bdcc6e9 failed allowlist check" |

---

## "Looks Done But Isn't" Checklist

- [ ] **Idle gate per-pod:** Verify `has_active_billing_session()` is checked on ALL pods individually, not the fleet aggregate — an active session on Pod 3 is still active even if Pod 1's health is checked first
- [ ] **Sentinel reads before fix:** Confirm auto-detect reads OTA_DEPLOYING, MAINTENANCE_MODE, and GRACEFUL_RELAUNCH before any fix — test by manually placing sentinel, running auto-detect in dry-run, and checking that the fix is skipped
- [ ] **Cascade dry-run default:** First production run must use `--dry-run` — verify log shows "would fix" not "fixed" and that no actual changes were applied
- [ ] **WhatsApp silence on clean run:** Trigger a run with no issues; confirm no WhatsApp message is sent to Uday
- [ ] **Bono coordination:** Manually run James auto-detect.sh and Bono bono-auto-detect.sh simultaneously; confirm only one writes AUTO_DETECT_ACTIVE and the other aborts cleanly
- [ ] **Post-deploy allowlist:** After auto-deploying rc-agent to Pod 8 canary, check `violation_count_24h` after 5 minutes — must be at baseline (not spiking from empty allowlist)
- [ ] **Venue hours gate:** Insert an active billing session into the DB, trigger auto-detect, confirm all pod-touching phases are skipped and only server-side checks run

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Recovery systems fighting (infinite loop) | HIGH | Kill all recovery actors manually, clear all sentinels (MAINTENANCE_MODE, GRACEFUL_RELAUNCH, AUTO_DETECT_ACTIVE), restart one pod at a time with manual monitoring |
| Alert fatigue (Uday ignoring messages) | HIGH (trust erosion, slow to recover) | Silence all auto-detect WhatsApp for 72 hours, manually review all previous messages to identify which were noise, reduce threshold to critical-only |
| Config corrupted by SSH banner | MEDIUM | SCP correct config from repo to server, verify first line is valid TOML (`head -1 | grep '^\['`), restart racecontrol.exe, verify build_id + config values at API level |
| Cascade fix broke a pod mid-OTA | MEDIUM | Restore from `rc-agent-prev.exe` (preserved 72h per standing rule), clear OTA_DEPLOYING sentinel, restart rc-agent via rc-sentry exec, verify build_id |
| MAINTENANCE_MODE from auto-detect's own restart | LOW | Wait 35 minutes for v17.1 auto-clear; if not clearing, manually delete `C:\RacingPoint\MAINTENANCE_MODE` via rc-sentry exec, then restart rc-agent |
| Dual-AI race condition corrupts binary | HIGH | Restore from `rc-agent-prev.exe` on affected pod, clear AUTO_DETECT_ACTIVE on both agents, fix schedule stagger, add coordination protocol before re-enabling Bono failover |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Recovery systems fighting | Phase 1: Core pipeline with sentinel reads | Manually set MAINTENANCE_MODE on a pod; run auto-detect in dry-run; confirm pod is skipped with sentinel logged |
| Alert fatigue | Phase 1: Escalation logic with silence conditions | Trigger clean run with no issues; confirm no WhatsApp sent |
| Config drift false positives | Phase 2: Drift schema with per-pod key exclusions | Run on live fleet; confirm pod_number, pod_ip, mac not flagged as drift |
| Log anomaly miscalibration | Phase 2: Pattern-based anomaly triggers | Inject known-bad pattern (empty allowlist); confirm anomaly fires. Run normal night; confirm expected WARNs do not fire |
| Cascade wrong target | Phase 2: Cascade engine with dry-run default | Run cascade in dry-run on live fleet; review log for any Pod 8 canary being "corrected" |
| Scheduled task conflicts | Phase 1: Idle gate (per-pod) and venue hours gate | Insert active billing session in DB; run auto-detect; confirm pod phases skipped |
| Dual-AI race conditions | Phase 3: Bono failover with coordination protocol | Run James + Bono simultaneously in dry-run; confirm only one writes AUTO_DETECT_ACTIVE |
| MAINTENANCE_MODE blocking | Phase 1: Auto-fix engine with MAINTENANCE_MODE awareness | Trigger MAINTENANCE_MODE on a pod; run auto-detect fix; confirm it waits rather than retrying immediately |
| Process guard empty allowlist | Phase 1: Post-deploy verification checklist | Deploy to Pod 8 canary via auto-detect; confirm allowlist non-empty in verification step output |
| SSH banner config corruption | Phase 2: Config fixes via API only, no SSH write path | Attempt a config drift fix in test; trace code path; confirm no SSH exec redirect is used |

---

## Sources

- `CLAUDE.md` standing rules — Cross-Process Recovery Awareness, SSH banner corruption incident (2026-03-24), MAINTENANCE_MODE permanent blocker, process guard empty allowlist incident (28,749 false violations/day), `spawn().is_ok()` silent failure pattern, OTA sentinel protocol
- `MEMORY.md` — v23.0 audit protocol QUIET findings baseline, rc-sentry restart bug, pod healer flicker (PowerShell variable strip), Self_monitor + WoL + rc-sentry infinite loop (45-minute diagnosis)
- `PROJECT.md` — v26.0 milestone constraints, foundation already built (auto-detect.sh b54e4585, bono-auto-detect.sh deployed, chains.json templates), known constraint: Bono cron must not conflict with bono-server-monitor and bono-racecontrol-monitor

---
*Pitfalls research for: Autonomous bug detection and self-healing (v26.0)*
*Researched: 2026-03-26 IST*
