# Phase 212: Detection Expansion - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Expand the auto-detect pipeline with 6 new detection modules: config drift, bat file regression, log anomalies, crash loops, feature flag desync, and schema gaps. Each module is a standalone bash script sourced into auto-detect.sh's detection step. Every detection traces to a documented historical Racing Point incident.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Detection scripts follow existing audit/phases/ pattern (bash + jq, sourced into auto-detect.sh).

Key technical constraints:
- Config drift (DET-01) must SCP racecontrol.toml from pods (SSH banner corruption standing rule applies — never pipe SSH output into config)
- Bat drift (DET-02) uses sha256sum checksum comparison against canonical repo version
- Log anomaly (DET-03) uses pattern-based triggers (ERROR/PANIC line count in last hour) — rate-based thresholds deferred (need 7-day calibration)
- Crash loop (DET-04) reads JSONL restart timestamps, not process count
- Flag desync (DET-05) requires querying /api/v1/flags on each pod and comparing enabled sets
- Schema gap (DET-06/07) checks for ALTER TABLE migrations matching CREATE TABLE columns
- All modules output findings in the same JSON format as existing audit phases (category, severity, pod_ip, message)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- scripts/auto-detect.sh — 6-step pipeline, detection modules sourced in step 2
- audit/lib/core.sh — venue_state_detect(), finding formatting, JSON output helpers
- audit/lib/fixes.sh — APPROVED_FIXES whitelist, is_pod_idle() billing gate
- audit/phases/tier*/ — 60 existing detection scripts as pattern reference
- scripts/bono-auto-detect.sh — Bono fallback inherits same detection modules

### Established Patterns
- Detection scripts: bash + jq, output JSON findings array, exit 0/1 based on P1 presence
- Pod iteration: `for pod_ip in 192.168.31.{89,33,28,88,86,87,38,91}; do`
- Config access: SCP from pod, validate first line (`head -1 | grep -q '^\['`), parse with grep/awk
- Fleet health: `curl -s http://192.168.31.23:8080/api/v1/fleet/health`

### Integration Points
- Detection modules sourced by auto-detect.sh step 2 (detection)
- Findings consumed by step 3 (auto-fix engine) via APPROVED_FIXES whitelist
- New config-drift findings may feed into Phase 213 (self-healing) auto-fix entries

</code_context>

<specifics>
## Specific Ideas

- Config drift should check specific keys known to cause incidents: ws_connect_timeout (must be >=600ms), app_health URLs (admin :3201, kiosk basePath), process_guard.enabled
- Bat drift should store canonical checksums in a reference file, not compute dynamically
- Log anomaly thresholds: >10 ERROR/PANIC in 1h = WARN, >50 = FAIL
- Crash loop: >3 restarts in 30min from JSONL timestamps

</specifics>

<deferred>
## Deferred Ideas

- Rate-based anomaly thresholds (need 7-day calibration window) — future phase
- Config drift via Rust API endpoint GET /api/v1/config/health-params — requires upstream work

</deferred>
