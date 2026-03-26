# Phase 213: Self-Healing & Escalation - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Graduated auto-fix engine for the auto-detect pipeline. Detected issues trigger fix attempts progressing through retry, restart, WoL, cloud failover, human escalation. Sentinel-aware (OTA_DEPLOYING, MAINTENANCE_MODE block fixes). Every fix is verified within 60s. Toggle-controlled (auto_fix_enabled). Follows Audit Protocol methodology (APPROVED_FIXES whitelist, is_pod_idle billing gate).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase. Healing engine follows existing patterns in audit/lib/fixes.sh and the escalation tiers documented in REQUIREMENTS.md.

Key constraints:
- Escalation tiers: retry, restart, WoL, cloud failover, WhatsApp to Uday (no tier skipped)
- Sentinel checks (OTA_DEPLOYING, MAINTENANCE_MODE) before EVERY tier action
- WhatsApp only on: (a) all automated tiers exhausted, (b) 3+ pods affected simultaneously
- Every fix followed by re-check within 60s showing "fix applied" + "verification: PASS/FAIL"
- auto_fix_enabled toggle in pipeline config -- false = detect only, no fixes
- HEAL-06: uses Audit Protocol methodology (APPROVED_FIXES whitelist from fixes.sh)
- HEAL-07: live-sync -- fixes applied immediately on detection (not batched)
- HEAL-08: auto_fix_enabled toggle (default=true, readable from config without pipeline restart)
- WhatsApp via Evolution API on Bono VPS (standing rule: promotions/deals go via Bono, not venue tunnel)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- audit/lib/fixes.sh -- APPROVED_FIXES whitelist, apply_approved_fix(), is_pod_idle() billing gate
- audit/lib/notify.sh -- WhatsApp notification via Evolution API, dual-channel Bono messaging
- scripts/auto-detect.sh -- step 5 already calls run_auto_fixes() from fixes.sh
- scripts/cascade.sh -- findings.json output consumed by fix engine
- audit/lib/core.sh -- safe_remote_exec(), venue_state_detect()

### Established Patterns
- Escalation: rc-watchdog.exe already implements tiered recovery (10 services)
- WoL: WoL MAC addresses available in CLAUDE.md network map
- Sentinel files: OTA_DEPLOYING and MAINTENANCE_MODE at C:\RacingPoint\ on pods
- Fix logging: LOGBOOK.md append pattern, JSON fix trail in audit/results/

### Integration Points
- Healing engine sourced by auto-detect.sh step 5 (after detection, before notification)
- Reads findings.json from cascade.sh (step 4 output)
- auto_fix_enabled toggle read from auto-detect.sh config or env var
- WhatsApp escalation calls notify.sh functions

</code_context>

<specifics>
## Specific Ideas

- Graduated tiers should share a common escalation loop with tier progression state
- Each tier should be a function (attempt_retry, attempt_restart, attempt_wol, attempt_cloud_failover, escalate_human)
- Fix verification should re-run the specific detector that found the issue (not full pipeline)

</specifics>

<deferred>
## Deferred Ideas

- WoL manual test on 2 pods required before WoL tier is enabled in APPROVED_FIXES
- Self-patch loop (Phase 215) -- detection of own code improvements

</deferred>
