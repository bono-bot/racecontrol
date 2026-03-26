# Phase 213: Self-Healing & Escalation - Research

**Researched:** 2026-03-26
**Domain:** Bash shell -- graduated auto-fix engine, escalation tiers, sentinel-aware gating, post-fix verification
**Confidence:** HIGH (all findings from direct code inspection of the live codebase)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation choices are at Claude discretion -- pure infrastructure phase.
Healing engine follows existing patterns in audit/lib/fixes.sh and REQUIREMENTS.md.

Key constraints:
- Escalation tiers: retry, restart, WoL, cloud failover, WhatsApp to Uday (no tier skipped)
- Sentinel checks (OTA_DEPLOYING, MAINTENANCE_MODE) before EVERY tier action
- WhatsApp only on: (a) all automated tiers exhausted, (b) 3+ pods affected simultaneously
- Every fix followed by re-check within 60s showing fix applied + verification PASS/FAIL
- auto_fix_enabled toggle in pipeline config -- false = detect only, no fixes
- HEAL-06: uses Audit Protocol methodology (APPROVED_FIXES whitelist from fixes.sh)
- HEAL-07: live-sync -- fixes applied immediately on detection (not batched)
- HEAL-08: auto_fix_enabled toggle (default=true, readable from config without pipeline restart)
- WhatsApp via Evolution API on Bono VPS (standing rule: comms go via Bono not venue tunnel)

### Claude Discretion
All implementation choices (file structure, function signatures, state tracking) are at Claude discretion.

### Deferred Ideas (OUT OF SCOPE)
- WoL manual test on 2 pods required before WoL tier is enabled in APPROVED_FIXES
- Self-patch loop (Phase 215) -- detection of own code improvements
</user_constraints>

---
<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HEAL-01 | Expanded APPROVED_FIXES: WoL for powered-off pods, MAINTENANCE_MODE auto-clear after 30 min, stale bat replacement | New fix functions in fixes.sh; WoL uses MAC table from CLAUDE.md; bat replacement uses :9998 staging server |
| HEAL-02 | 5-tier escalation ladder: retry (2x) -> restart -> WoL -> cloud failover -> WhatsApp | escalation-engine.sh with escalate_pod() orchestrating tier functions |
| HEAL-03 | Each tier checks sentinel files before acting | check_pod_sentinels() already in fixes.sh; _sentinel_gate() wraps it per tier |
| HEAL-04 | WhatsApp silence: no alert for QUIET findings, 6h cooldown, venue-closed deferred | Builds on _is_cooldown_active()/_record_alert() from auto-detect.sh |
| HEAL-05 | Post-fix verification -- every fix followed by re-check within 60s | verify_fix() polls every 10s up to 60s; emits verification:PASS/FAIL |
| HEAL-06 | Fixes follow Cause Elimination methodology: symptom -> hypothesis -> test -> fix -> verify | Each fix function documents its hypothesis; no blind whitelist matching |
| HEAL-07 | Live-sync model -- fixes on detection, not batched | Detectors call attempt_heal() after _emit_finding(); sourced via cascade.sh |
| HEAL-08 | Global toggle auto_fix_enabled readable from config without restart | Config at audit/results/auto-detect-config.json; read at call time not startup |
</phase_requirements>

---

## Summary

Phase 213 extends the existing auto-detect pipeline with a graduated healing engine. The detection
layer (Phase 212) already produces findings.json via _emit_finding(). Phase 213 adds an escalation
engine that consumes those findings in-flight (live-sync, HEAL-07) and applies fixes in tier order:
retry -> restart -> WoL -> cloud failover -> WhatsApp to Uday.

The existing codebase already has the scaffolding: fixes.sh has is_pod_idle(), check_pod_sentinels(),
emit_fix(), and three approved fixes. auto-detect.sh has cooldown infrastructure. notify.sh has
all three channels including _notify_whatsapp_uday(). The work for Phase 213 is: escalation tier
loop, three new fix functions, post-fix verifier, QUIET-aware WhatsApp gate, auto_fix_enabled toggle.

**Primary recommendation:** Implement as scripts/healing/escalation-engine.sh sourced by cascade.sh
and auto-detect.sh. Each detector calls attempt_heal() immediately after emitting a finding.
The auto_fix_enabled toggle is read from audit/results/auto-detect-config.json at each fix attempt.

---

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| bash | 5.x (Git Bash) | Healing engine scripts | All audit infrastructure is bash+jq |
| jq | 1.6+ | JSON state tracking, fix records | Used throughout audit/ and scripts/ |
| curl | bundled | HTTP to rc-agent :8090, relay :8766 | Standard for all remote ops |
| safe_remote_exec() | core.sh | Remote pod commands | Handles tempfile, curl, cmd quoting |
| emit_fix() | core.sh | Fix audit trail | Already writes fixes.jsonl |
| check_pod_sentinels() | fixes.sh | Sentinel gate | Checks OTA_DEPLOYING + MAINTENANCE_MODE |
| is_pod_idle() | fixes.sh | Billing gate | Returns 1 on any API error (fail-safe) |
| _notify_whatsapp_uday() | notify.sh | Uday escalation | Bono relay + Evolution API |
| _is_cooldown_active() | auto-detect.sh | 6h dedup | Keyed per pod_ip:issue_type |

### WoL Implementation Notes
WoL magic packets are sent via safe_remote_exec to server .23 (always on) using PowerShell UDP.

Confirmed MAC addresses from CLAUDE.md network map:
- Pod 1 (.89): 30-56-0F-05-45-88
- Pod 2 (.33): 30-56-0F-05-46-53
- Pod 3 (.28): 30-56-0F-05-44-B3
- Pod 4 (.88): 30-56-0F-05-45-25
- Pod 5 (.86): 30-56-0F-05-44-B7
- Pod 6 (.87): 30-56-0F-05-45-6E
- Pod 7 (.38): 30-56-0F-05-44-B4
- Pod 8 (.91): 30-56-0F-05-46-C5

WoL tier MUST default to WOL_ENABLED=false until manual test on 2 pods (STATE.md pending todo).

### No New Dependencies
Zero new tools required. Everything is bash, jq, curl, and existing lib functions.

---## Architecture Patterns

### Recommended File Structure

```
scripts/
  healing/
    escalation-engine.sh   # New: 5-tier escalation + attempt_heal() entry point
  detectors/
    detect-*.sh             # Existing (Phase 212); each calls attempt_heal() after _emit_finding()
  auto-detect.sh            # Modified: source escalation-engine.sh
  cascade.sh                # Modified: source escalation-engine.sh
audit/
  lib/
    fixes.sh                # Modified: add wol_pod(), clear_old_maintenance_mode(), replace_stale_bat()
    core.sh                 # Unchanged
    notify.sh               # Unchanged
  results/
    auto-detect-config.json # New: runtime toggle config
```

### Pattern 1: 5-Tier Escalation Loop (HEAL-02/03)

Each tier is a function. Loop calls them in order, stops on success, sentinel-checks before each.

```bash
# scripts/healing/escalation-engine.sh
escalate_pod() {
  local pod_ip="$1" issue_type="$2"

  # HEAL-08: read toggle at call time
  if ! _auto_fix_enabled; then
    log INFO "[HEAL] auto_fix_enabled=false -- detect only"
    return 0
  fi

  # Billing gate
  if ! is_pod_idle "$pod_ip"; then
    emit_fix "heal" "$pod_ip" "SKIP_BILLING_ACTIVE" "$issue_type" "skipped"
    return 0
  fi

  local tier_result="UNRESOLVED"

  # Tier 1: Retry (2x health check)
  _sentinel_gate "$pod_ip" "tier1_retry" && {
    tier_result=$(attempt_retry "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 2: Restart (schtasks via rc-sentry)
  _sentinel_gate "$pod_ip" "tier2_restart" && {
    tier_result=$(attempt_restart "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 3: WoL (disabled until manual test)
  if [[ "${WOL_ENABLED:-false}" == "true" ]]; then
    _sentinel_gate "$pod_ip" "tier3_wol" && {
      tier_result=$(attempt_wol "$pod_ip" "$issue_type")
    }
    [[ "$tier_result" == "RESOLVED" ]] && return 0
  fi

  # Tier 4: Cloud failover
  _sentinel_gate "$pod_ip" "tier4_cloud_failover" && {
    tier_result=$(attempt_cloud_failover "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 5: WhatsApp to Uday
  emit_fix "heal" "$pod_ip" "tier5_escalate_human" "$issue_type" "ESCALATED"
  escalate_human "$pod_ip" "$issue_type"
}
```

### Pattern 2: Sentinel Gate (HEAL-03)

```bash
_sentinel_gate() {
  local pod_ip="$1" tier_name="$2"
  if ! check_pod_sentinels "$pod_ip"; then
    log INFO "[HEAL] sentinel block: $pod_ip at $tier_name"
    emit_fix "heal" "$pod_ip" "SENTINEL_BLOCK_${tier_name}" "sentinel_active" "blocked"
    return 1
  fi
  return 0
}
```

### Pattern 3: Post-Fix Verification Poll 60s (HEAL-05)

```bash
verify_fix() {
  local pod_ip="$1" issue_type="$2"
  local verify_fn="_verify_${issue_type}"
  local deadline=$(( $(date +%s) + 60 ))
  while [[ $(date +%s) -lt $deadline ]]; do
    if [[ $(type -t "$verify_fn") == "function" ]]; then
      if "$verify_fn" "$pod_ip"; then
        emit_fix "heal_verify" "$pod_ip" "verify_${issue_type}" "fix_applied" "verification:PASS"
        return 0
      fi
    fi
    sleep 10
  done
  emit_fix "heal_verify" "$pod_ip" "verify_${issue_type}" "fix_applied" "verification:FAIL"
  return 1
}
```

### Pattern 4: Live-Sync Integration in Detectors (HEAL-07)

```bash
# In each detector file after each _emit_finding call:
_emit_finding "crash_loop" "P1" "$pod_ip" "crash loop: N restarts in 30min"
# HEAL-07: live-sync -- attempt heal immediately
if [[ $(type -t attempt_heal) == "function" ]]; then
  attempt_heal "$pod_ip" "crash_loop"
fi
```

### Pattern 5: auto_fix_enabled Toggle (HEAL-08)

Config file at audit/results/auto-detect-config.json. Read at each fix call. No restart needed.

```bash
_auto_fix_enabled() {
  local config_file="$REPO_ROOT/audit/results/auto-detect-config.json"
  [[ "${NO_FIX:-false}" == "true" ]] && return 1  # cli flag overrides config
  [[ ! -f "$config_file" ]] && return 0             # missing = enabled (fail-safe)
  local val
  val=$(jq -r '.auto_fix_enabled // true' "$config_file" 2>/dev/null || echo "true")
  [[ "$val" == "true" ]]
}
```

### Pattern 6: WhatsApp Silence Conditions (HEAL-04)

```bash
escalate_human() {
  local pod_ip="$1" issue_type="$2" severity="${3:-P1}"
  [[ "$severity" == "QUIET" ]] && return 0  # HEAL-04: no WhatsApp for QUIET
  local ist_hour venue_state
  ist_hour=$(TZ=Asia/Kolkata date +%H | sed 's/^0*//')
  venue_state=$(venue_state_detect 2>/dev/null || echo "closed")
  # HEAL-04: defer closed-hour findings to morning
  if [[ "$venue_state" == "closed" ]] && [[ "${ist_hour:-0}" -lt 7 ]]; then return 0; fi
  # HEAL-04: 6h cooldown per pod+issue
  _is_cooldown_active "$pod_ip" "$issue_type" && return 0
  _record_alert "$pod_ip" "$issue_type"
  local msg="Racing Point ALERT: Pod ${pod_ip} -- ${issue_type}. All recovery tiers exhausted. $(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')"
  UDAY_WHATSAPP="${UDAY_WHATSAPP:-}" _notify_whatsapp_uday "$msg"
}
```

### Pattern 7: HEAL-01 New Fix Functions

Three new entries added to APPROVED_FIXES array in fixes.sh:

1. **wol_pod** -- WoL magic packet via server .23 PowerShell. Guarded by WOL_ENABLED=false.
2. **clear_old_maintenance_mode** -- clears MAINTENANCE_MODE only when >30 min old AND venue closed.
3. **replace_stale_bat** -- downloads canonical start-rcagent.bat from James :9998 to pod + verifies checksum.

### Anti-Patterns to Avoid

- **Batching fixes**: Fixes must fire on detection (HEAL-07). Never collect all findings and fix at end.
- **Skipping sentinel gate**: Every tier must call _sentinel_gate(). OTA_DEPLOYING blocks rc-agent restart.
- **Blind whitelist matching**: Each fix function must document hypothesis (HEAL-06). Clear MAINTENANCE_MODE FIRST if that is root cause.
- **Hardcoded WOL_ENABLED=true**: Must stay false until manual test on 2 pods (STATE.md pending).
- **WhatsApp on QUIET findings**: Filter severity before calling _notify_whatsapp_uday.
- **WoL when machine is on**: Check ping before sending magic packet. Skip WoL if ping succeeds.

---

## Architecture Patterns

### Recommended File Structure

```
scripts/
  healing/
    escalation-engine.sh   # New: 5-tier escalation + attempt_heal() entry point
  detectors/
    detect-*.sh             # Existing (Phase 212); each calls attempt_heal() after _emit_finding()
  auto-detect.sh            # Modified: source escalation-engine.sh
  cascade.sh                # Modified: source escalation-engine.sh
audit/
  lib/
    fixes.sh                # Modified: add wol_pod(), clear_old_maintenance_mode(), replace_stale_bat()
    core.sh                 # Unchanged
    notify.sh               # Unchanged
  results/
    auto-detect-config.json # New: runtime toggle config
```

### Pattern 1: 5-Tier Escalation Loop (HEAL-02/03)

Each tier is a function. Loop calls them in order, stops on success, sentinel-checks before each.

```bash
# scripts/healing/escalation-engine.sh
escalate_pod() {
  local pod_ip="$1" issue_type="$2"

  # HEAL-08: read toggle at call time
  if ! _auto_fix_enabled; then
    log INFO "[HEAL] auto_fix_enabled=false -- detect only"
    return 0
  fi

  # Billing gate
  if ! is_pod_idle "$pod_ip"; then
    emit_fix "heal" "$pod_ip" "SKIP_BILLING_ACTIVE" "$issue_type" "skipped"
    return 0
  fi

  local tier_result="UNRESOLVED"

  # Tier 1: Retry (2x health check)
  _sentinel_gate "$pod_ip" "tier1_retry" && {
    tier_result=$(attempt_retry "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 2: Restart (schtasks via rc-sentry)
  _sentinel_gate "$pod_ip" "tier2_restart" && {
    tier_result=$(attempt_restart "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 3: WoL (disabled until manual test)
  if [[ "${WOL_ENABLED:-false}" == "true" ]]; then
    _sentinel_gate "$pod_ip" "tier3_wol" && {
      tier_result=$(attempt_wol "$pod_ip" "$issue_type")
    }
    [[ "$tier_result" == "RESOLVED" ]] && return 0
  fi

  # Tier 4: Cloud failover
  _sentinel_gate "$pod_ip" "tier4_cloud_failover" && {
    tier_result=$(attempt_cloud_failover "$pod_ip" "$issue_type")
  }
  [[ "$tier_result" == "RESOLVED" ]] && return 0

  # Tier 5: WhatsApp to Uday
  emit_fix "heal" "$pod_ip" "tier5_escalate_human" "$issue_type" "ESCALATED"
  escalate_human "$pod_ip" "$issue_type"
}
```

### Pattern 2: Sentinel Gate (HEAL-03)

```bash
_sentinel_gate() {
  local pod_ip="$1" tier_name="$2"
  if ! check_pod_sentinels "$pod_ip"; then
    log INFO "[HEAL] sentinel block: $pod_ip at $tier_name"
    emit_fix "heal" "$pod_ip" "SENTINEL_BLOCK_${tier_name}" "sentinel_active" "blocked"
    return 1
  fi
  return 0
}
```

### Pattern 3: Post-Fix Verification Poll 60s (HEAL-05)

```bash
verify_fix() {
  local pod_ip="$1" issue_type="$2"
  local verify_fn="_verify_${issue_type}"
  local deadline=$(( $(date +%s) + 60 ))
  while [[ $(date +%s) -lt $deadline ]]; do
    if [[ $(type -t "$verify_fn") == "function" ]]; then
      if "$verify_fn" "$pod_ip"; then
        emit_fix "heal_verify" "$pod_ip" "verify_${issue_type}" "fix_applied" "verification:PASS"
        return 0
      fi
    fi
    sleep 10
  done
  emit_fix "heal_verify" "$pod_ip" "verify_${issue_type}" "fix_applied" "verification:FAIL"
  return 1
}
```

### Pattern 4: Live-Sync Integration in Detectors (HEAL-07)

```bash
# In each detector file after each _emit_finding call:
_emit_finding "crash_loop" "P1" "$pod_ip" "crash loop: N restarts in 30min"
# HEAL-07: live-sync -- attempt heal immediately
if [[ $(type -t attempt_heal) == "function" ]]; then
  attempt_heal "$pod_ip" "crash_loop"
fi
```

### Pattern 5: auto_fix_enabled Toggle (HEAL-08)

Config file at audit/results/auto-detect-config.json. Read at each fix call. No restart needed.

```bash
_auto_fix_enabled() {
  local config_file="$REPO_ROOT/audit/results/auto-detect-config.json"
  [[ "${NO_FIX:-false}" == "true" ]] && return 1  # cli flag overrides config
  [[ ! -f "$config_file" ]] && return 0             # missing = enabled (fail-safe)
  local val
  val=$(jq -r '.auto_fix_enabled // true' "$config_file" 2>/dev/null || echo "true")
  [[ "$val" == "true" ]]
}
```

### Pattern 6: WhatsApp Silence Conditions (HEAL-04)

```bash
escalate_human() {
  local pod_ip="$1" issue_type="$2" severity="${3:-P1}"
  [[ "$severity" == "QUIET" ]] && return 0  # HEAL-04: no WhatsApp for QUIET
  local ist_hour venue_state
  ist_hour=$(TZ=Asia/Kolkata date +%H | sed 's/^0*//')
  venue_state=$(venue_state_detect 2>/dev/null || echo "closed")
  # HEAL-04: defer closed-hour findings to morning
  if [[ "$venue_state" == "closed" ]] && [[ "${ist_hour:-0}" -lt 7 ]]; then return 0; fi
  # HEAL-04: 6h cooldown per pod+issue
  _is_cooldown_active "$pod_ip" "$issue_type" && return 0
  _record_alert "$pod_ip" "$issue_type"
  local msg="Racing Point ALERT: Pod ${pod_ip} -- ${issue_type}. All recovery tiers exhausted. $(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')"
  UDAY_WHATSAPP="${UDAY_WHATSAPP:-}" _notify_whatsapp_uday "$msg"
}
```

### Pattern 7: HEAL-01 New Fix Functions

Three new entries added to APPROVED_FIXES array in fixes.sh:

1. **wol_pod** -- WoL magic packet via server .23 PowerShell. Guarded by WOL_ENABLED=false.
2. **clear_old_maintenance_mode** -- clears MAINTENANCE_MODE only when >30 min old AND venue closed.
3. **replace_stale_bat** -- downloads canonical start-rcagent.bat from James :9998 to pod + verifies checksum.

### Anti-Patterns to Avoid

- **Batching fixes**: Fixes must fire on detection (HEAL-07). Never collect all findings and fix at end.
- **Skipping sentinel gate**: Every tier must call _sentinel_gate(). OTA_DEPLOYING blocks rc-agent restart.
- **Blind whitelist matching**: Each fix function must document hypothesis (HEAL-06). Clear MAINTENANCE_MODE FIRST if that is root cause.
- **Hardcoded WOL_ENABLED=true**: Must stay false until manual test on 2 pods (STATE.md pending).
- **WhatsApp on QUIET findings**: Filter severity before calling _notify_whatsapp_uday.
- **WoL when machine is on**: Check ping before sending magic packet. Skip WoL if ping succeeds.

---

## Don't Hand-Roll

| Problem | Use Instead | Why |
|---------|-------------|-----|
| Remote pod command | safe_remote_exec() from core.sh | Tempfile, cmd quoting, timeout |
| Fix audit trail | emit_fix() from core.sh | Already writes fixes.jsonl |
| Billing gate | is_pod_idle() from fixes.sh | Returns 1 on API error (fail-safe) |
| Sentinel check | check_pod_sentinels() from fixes.sh | Both OTA + MAINTENANCE_MODE |
| WhatsApp send | _notify_whatsapp_uday() from notify.sh | Tempfile safety, relay, non-fatal |
| Cooldown dedup | _is_cooldown_active()/_record_alert() from auto-detect.sh | Per pod+issue, 6h window |
| IST timestamp | ist_now() from core.sh | UTC mismatch caused past bugs |

---

## Common Pitfalls

### Pitfall 1: Escalation Tier Skipping on Sentinel Block
**What goes wrong:** Sentinel block skips ALL tiers including human escalation. Uday never notified.
**How to avoid:** Sentinel block is a SKIP. If ALL tiers blocked and pod still offline, that itself should escalate.
**Warning signs:** fixes.jsonl shows many SENTINEL_BLOCK entries for same pod with no resolution.

### Pitfall 2: verify_fix() -- 60s Is a Ceiling Not a Wait
**How to avoid:** Poll every 10s up to 60s. Most fixes resolve in 5-15s.

### Pitfall 3: WoL for Machines That Are Already On
**How to avoid:** Check ping before sending magic packet. Skip WoL if ping succeeds.

### Pitfall 4: Stale FLEET_HEALTH_CACHE
**How to avoid:** Reset _FLEET_HEALTH_CACHE to empty string at start of each escalate_pod() call.

### Pitfall 5: MAINTENANCE_MODE Age Check on Windows Paths
**How to avoid:** Use safe_remote_exec with forfiles: forfiles /P C:\RacingPoint /M MAINTENANCE_MODE /C "cmd /c echo @fdate @ftime"

### Pitfall 6: replace_stale_bat Requires HTTP Server on Port 9998
**How to avoid:** Check port 9998 responds. Fail gracefully with after_state=bat_staging_server_offline.

### Pitfall 7: Cloud Failover When Bono Already Active
**How to avoid:** Check _is_cooldown_active fleet cloud_failover before activating.

---

## Code Examples

### Tier 1 Retry



### Tier 2 Restart



---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual fix via SSH | run_auto_fixes() in fixes.sh | v23.0 Phase 189-193 | Fixes run unattended |
| Fixes batched at end | Live-sync on detection (HEAL-07) | Phase 213 new | Issues fixed before audit completes |
| Single-tier restart | 5-tier escalation ladder | Phase 213 new | Graduated response |
| --no-fix flag only | auto_fix_enabled config file | Phase 213 new | Toggle without restart |
| WhatsApp for any issue | WhatsApp only when tiers exhausted | Phase 213 new | Reduced alert fatigue |

---

## Open Questions

1. **WoL packet sender on server .23 Windows**
   - What we know: server .23 always on LAN; PowerShell UDP socket can send magic packets
   - Recommendation: Implement wol_pod() via safe_remote_exec to server .23 with PowerShell UDP. Test on 2 pods before enabling WOL_ENABLED=true.

2. **Detector-to-verify function binding**
   - What we know: 6 issue types (crash_loop, bat_drift, config_drift, log_anomaly, flag_desync, schema_gap)
   - Recommendation: Lightweight per-type check. Full detector re-run is 15s -- too slow.

3. **MAINTENANCE_MODE 30-min auto-clear scope**
   - Recommendation: Auto-clear only when (a) sentinel >30 min old AND (b) venue_state=closed. During open hours do not auto-clear.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Bash unit tests (comms-link test/run-all.sh) + manual E2E |
| Config file | audit/lib/fixes.sh (APPROVED_FIXES), audit/results/auto-detect-config.json |
| Quick run | bash scripts/healing/escalation-engine.sh --self-test |
| Full suite | COMMS_PSK="..." bash test/run-all.sh |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| HEAL-01 | APPROVED_FIXES includes 3 new functions | unit | bash source+array check | Wave 0 |
| HEAL-02 | escalate_pod() calls 5 tiers in order | unit mock | bash scripts/healing/escalation-engine.sh --self-test | Wave 0 |
| HEAL-03 | Sentinel active = tier skipped + logged | unit | included in --self-test | Wave 0 |
| HEAL-04 | QUIET findings produce no WhatsApp call | unit | included in --self-test | Wave 0 |
| HEAL-05 | verify_fix emits PASS/FAIL within 60s | unit | included in --self-test | Wave 0 |
| HEAL-06 | Each fix function has documented hypothesis | code review | grep Hypothesis audit/lib/fixes.sh | Wave 0 |
| HEAL-07 | attempt_heal() called after _emit_finding() | code review | grep attempt_heal scripts/detectors/ | Wave 0 |
| HEAL-08 | auto_fix_enabled=false stops all fixes | unit | included in --self-test with mock config | Wave 0 |

### Sampling Rate
- **Per task commit:** bash scripts/healing/escalation-engine.sh --self-test
- **Per wave merge:** COMMS_PSK="..." bash test/run-all.sh
- **Phase gate:** Full suite green before /gsd:verify-work

### Wave 0 Gaps
- [ ] scripts/healing/escalation-engine.sh -- new file; needs --self-test mode with mocked functions (no live pod contact)

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: audit/lib/fixes.sh -- APPROVED_FIXES, is_pod_idle, check_pod_sentinels, emit_fix
- Direct code inspection: audit/lib/notify.sh -- _notify_whatsapp_uday, Bono dual-channel
- Direct code inspection: scripts/auto-detect.sh -- cooldown infrastructure, NO_FIX flag, 6-step pipeline
- Direct code inspection: scripts/cascade.sh -- _emit_finding, DETECTOR_FINDINGS, run_all_detectors
- Direct code inspection: audit/lib/core.sh -- safe_remote_exec, emit_fix, ist_now, venue_state_detect
- Direct code inspection: scripts/detectors/*.sh -- all 6 detectors confirmed shipped by Phase 212
- .planning/STATE.md -- WoL pending manual test, Phase 211 cooldown architecture decisions
- .planning/REQUIREMENTS.md -- HEAL-01 through HEAL-08 text verified verbatim

### Secondary (MEDIUM confidence)
- CLAUDE.md -- MAC address table, pod IPs confirmed
- scripts/AUTONOMOUS-DETECTION.md -- architecture diagram, --no-fix flag behavior

### Tertiary (LOW confidence)
- None -- all findings from direct code inspection

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all tools verified in existing codebase
- Architecture: HIGH -- patterns derived from fixes.sh, notify.sh, auto-detect.sh
- Pitfalls: HIGH -- derived from CLAUDE.md standing rules

**Research date:** 2026-03-26
**Valid until:** 2026-04-26 (stable bash infrastructure, 30-day window)
