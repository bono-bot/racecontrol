# Requirements: v23.1 Audit Protocol v5.0 — Cross-Service Validation & Gap Closure

**Defined:** 2026-03-26
**Core Value:** Every user-visible system breakage is detected by the audit — no false PASSes

## v1 Requirements

### Wrong Layer (WL) — Check the consuming service, not just infrastructure

- [ ] **WL-01**: Phase 09 self-monitor check verifies liveness beyond uptime proxy (log recency or dedicated health field)
- [ ] **WL-02**: Phase 10 AI healer check test-queries Ollama model qwen2.5:3b for a parseable response, not just /api/tags
- [ ] **WL-03**: Phase 15 preflight check queries rc-agent preflight subsystem status, not just overall health=ok
- [ ] **WL-04**: Phase 44 face detection check verifies face-audit.jsonl has entries within last 10 minutes

### Count vs Health (CH) — Verify items work, not just that they exist

- [x] **CH-01**: Phase 07 allowlist check spot-verifies known-good process (svchost.exe) is present, not just count >= 100
- [x] **CH-02**: Phase 25 menu check verifies at least one item has available=true
- [x] **CH-03**: Phase 39 feature flags check verifies at least one flag with enabled=true exists
- [x] **CH-04**: Phase 56 OpenAPI check spot-verifies 3-5 critical endpoints by name (app-health, flags, guard/whitelist)

### Config Validation (CV) — Verify credentials and config values are correct

- [x] **CV-01**: Phase 02 config check validates ws_connect_timeout >= 600ms in racecontrol.toml
- [x] **CV-02**: Phase 02 config check validates app_health monitoring URLs contain correct ports
- [x] **CV-03**: Phase 30 WhatsApp check tests Evolution API live connection state
- [x] **CV-04**: Phase 31 email check proactively verifies OAuth token expiry date

### Dashboard/UI (UI) — Verify user-facing pages render correctly

- [ ] **UI-01**: Phase 20 kiosk check verifies static file serving from pod perspective (_next/static/ returns 200)
- [ ] **UI-02**: Phase 26 game catalog check verifies kiosk game selection page renders expected game count
- [ ] **UI-03**: Phase 44 check verifies Next.js cameras page at :3200/cameras loads

### Cross-Service (XS) — Verify the dependency chain end-to-end

- [ ] **XS-01**: Phase 35+36 cloud sync check compares venue and cloud driver updated_at timestamps (< 5 min delta)
- [ ] **XS-02**: Phase 07+09 cross-check verifies allowlist background task ran recently when safe_mode inactive

### Structural Fixes (SF) — False PASSes and inoperative checks

- [x] **SF-01**: Phase 19 display resolution check measures resolution via rc-agent exec (not hardcoded 1920x1080)
- [x] **SF-02**: Phase 21 billing endpoint unreachable returns WARN during venue hours (not PASS)
- [x] **SF-03**: Phase 53 watchdog check treats ps_count=0 as WARN (watchdog dead)

### Operational (OP) — Startup reliability

- [x] **OP-01**: go2rtc warmup integrated into start-rcsentry-ai.bat (prevents NVR RTSP flood)

## v2 Requirements

- **DV-01**: rc-sentry-ai staggered camera startup (requires ONNX build env)
- **DV-02**: Phase 38+46 relay E2E cross-check verifies VPS comms-link WS state
- **DV-03**: Phase 39 runtime flag behavioral verification on rc-agent

## Out of Scope

| Feature | Reason |
|---------|--------|
| Rewriting audit in Rust/Python | Pure bash+jq by design |
| Adding phases beyond 60 | Fix existing phases only |
| rc-sentry-ai Rust code changes | ONNX build env unavailable |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CV-01 | Phase 202 | Complete |
| CV-02 | Phase 202 | Complete |
| CV-03 | Phase 202 | Complete |
| CV-04 | Phase 202 | Complete |
| SF-01 | Phase 202 | Complete |
| SF-02 | Phase 202 | Complete |
| SF-03 | Phase 202 | Complete |
| OP-01 | Phase 202 | Complete |
| WL-01 | Phase 203 | Pending |
| WL-02 | Phase 203 | Pending |
| WL-03 | Phase 203 | Pending |
| WL-04 | Phase 203 | Pending |
| CH-01 | Phase 203 | Complete |
| CH-02 | Phase 203 | Complete |
| CH-03 | Phase 203 | Complete |
| CH-04 | Phase 203 | Complete |
| XS-01 | Phase 204 | Pending |
| XS-02 | Phase 204 | Pending |
| UI-01 | Phase 204 | Pending |
| UI-02 | Phase 204 | Pending |
| UI-03 | Phase 204 | Pending |

**Coverage:**
- v1 requirements: 22 total
- Mapped to phases: 22/22
- Unmapped: 0

---

# Requirements: v26.0 Autonomous Bug Detection & Self-Healing

**Defined:** 2026-03-26
**Core Value:** Fully autonomous infrastructure health — detect, fix, cascade, and notify without human intervention

## v1 Requirements

### Scheduling & Execution (SCHED)

- [x] **SCHED-01**: James auto-detect runs daily at 2:30 AM IST via Windows Task Scheduler without human trigger
- [x] **SCHED-02**: Bono auto-detect runs daily at 2:35 AM IST via cron (5-min offset prevents race condition)
- [x] **SCHED-03**: Run guard prevents overlapping auto-detect executions (PID file lock)
- [x] **SCHED-04**: Escalation cooldown prevents repeated WhatsApp alerts for same issue within 6 hours
- [x] **SCHED-05**: Venue-state-aware timing — full mode during closed hours, quick mode if triggered during open hours

### Detection Expansion (DET)

- [x] **DET-01**: Config drift detection compares running racecontrol.toml values against canonical expected values
- [x] **DET-02**: Bat file drift detection compares pod start-rcagent.bat checksums against repo canonical version
- [x] **DET-03**: Log anomaly detection scans pod JSONL logs for ERROR/PANIC rate exceeding threshold (>10/hour open, >2/hour closed)
- [x] **DET-04**: Crash loop detection flags pods with >3 rc-agent restarts in 30 minutes
- [x] **DET-05**: Feature flag sync check verifies all 8 pods have identical enabled flag set
- [x] **DET-06**: Schema drift detection compares cloud and venue DB table schemas for column mismatches
- [x] **DET-07**: Cascade module (cascade.sh) sources into auto-detect.sh, shares env (BUGS_FOUND, LOG_FILE)

### Self-Healing & Escalation (HEAL)

- [x] **HEAL-01**: Expanded auto-fix whitelist adds: WoL for powered-off pods, MAINTENANCE_MODE auto-clear after 30 min, stale bat file replacement
- [x] **HEAL-02**: 5-tier escalation ladder: retry (2x) → restart (schtasks) → WoL → cloud failover → Uday WhatsApp
- [x] **HEAL-03**: Each escalation tier checks sentinel files (OTA_DEPLOYING, MAINTENANCE_MODE) before acting
- [x] **HEAL-04**: WhatsApp silence conditions: no alert for QUIET findings, max 1 alert per issue per 6 hours, venue-closed findings deferred to morning
- [x] **HEAL-05**: Post-fix verification — every auto-fix is followed by re-check to confirm resolution
- [x] **HEAL-06**: Fixes follow Audit Protocol debugging methodology: Cause Elimination Process (document symptom → hypothesize → test & eliminate → fix confirmed cause → verify) — not blind whitelist matching
- [x] **HEAL-07**: Live-sync model — fixes apply immediately on detection and confirmed diagnosis (not batched to end of run), each fix pushed to affected system the moment it's verified
- [x] **HEAL-08**: Global toggle `auto_fix_enabled` in auto-detect config (toggle via relay command, admin API, or TOML edit) — when OFF, system detects and reports only, never applies fixes

### Bono Coordination (COORD)

- [x] **COORD-01**: AUTO_DETECT_ACTIVE mutex via relay — prevents James and Bono from fixing simultaneously
- [x] **COORD-02**: Bono failover requires confirmed Tailscale offline status (not just timeout) before activating
- [x] **COORD-03**: Delegation protocol — Bono checks James alive first, delegates if so, only runs independently when James confirmed down
- [x] **COORD-04**: After James recovery, Bono deactivates cloud failover and syncs findings

### Self-Improving Intelligence (LEARN)

- [x] **LEARN-01**: Detection pattern tracker logs findings across runs to suggestions.jsonl (bug type, frequency, pod, fix applied, success)
- [x] **LEARN-02**: Suggestion engine analyzes patterns and generates improvement proposals with evidence + confidence score
- [x] **LEARN-03**: Suggestions auto-categorized: "new audit check", "threshold tune", "new auto-fix candidate", "standing rule gap", "cascade coverage gap", "self-patch"
- [x] **LEARN-04**: Trend analysis flags statistical outliers (e.g. "Pod 3 has 4x more sentinel clears than fleet average")
- [x] **LEARN-05**: Approved suggestions sync to standing-rules-registry.json, suppress.json, or APPROVED_FIXES and pushed to both AIs
- [x] **LEARN-06**: Suggestion inbox viewable via API endpoint or Markdown report
- [x] **LEARN-07**: Self-patch loop — the system can modify its own v26.0 scripts (auto-detect.sh, cascade.sh, fixes.sh, detectors) to improve detection accuracy, fix coverage, or threshold tuning, then commit + push + notify
- [x] **LEARN-08**: Self-patch follows same Cause Elimination methodology — change is diagnosed (why is detection wrong?), patched, verified (re-run detects correctly), and logged. Reverts automatically if verification fails
- [x] **LEARN-09**: Self-patch toggle `self_patch_enabled` — independent of `auto_fix_enabled` (can detect+fix infrastructure without self-modifying, or vice versa)

### Pipeline Testing (TEST)

- [x] **TEST-01**: Integration test suite for auto-detect.sh validates each of the 6 steps independently
- [x] **TEST-02**: Injected anomaly fixtures test each detector (fake config drift, fake log anomaly, fake build mismatch)
- [x] **TEST-03**: Escalation ladder test verifies tier progression with mocked pod responses
- [x] **TEST-04**: Bono coordination test verifies mutex acquisition and delegation protocol

## v2 Requirements

- **DET-08**: Real-time continuous health polling (30s interval) — separate from nightly audit
- **LEARN-10**: Ollama-powered suggestion reasoning (Tier 3 AI analysis of patterns)
- **HEAL-09**: Auto-rollback on failed deploys (binary swap to -prev.exe)
- **DET-09**: Network partition detection (pods reachable via LAN but not WS)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Autonomous binary deployment | Too risky without human gate — OTA pipeline handles this separately |
| Real-time log streaming | Excessive bandwidth; query-on-demand fits 8-min budget |
| Per-pod compile-time variants | Single-binary-tier policy (v22.0 standing rule) |
| Direct TOML value overwrites | Config drift is detected and reported, not silently corrected — human reviews proposed changes |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SCHED-01 | Phase 211 | Complete |
| SCHED-02 | Phase 211 | Complete |
| SCHED-03 | Phase 211 | Complete |
| SCHED-04 | Phase 211 | Complete |
| SCHED-05 | Phase 211 | Complete |
| DET-01 | Phase 212 | Complete |
| DET-02 | Phase 212 | Complete |
| DET-03 | Phase 212 | Complete |
| DET-04 | Phase 212 | Complete |
| DET-05 | Phase 212 | Complete |
| DET-06 | Phase 212 | Complete |
| DET-07 | Phase 212 | Complete |
| HEAL-01 | Phase 213 | Complete |
| HEAL-02 | Phase 213 | Complete |
| HEAL-03 | Phase 213 | Complete |
| HEAL-04 | Phase 213 | Complete |
| HEAL-05 | Phase 213 | Complete |
| HEAL-06 | Phase 213 | Complete |
| HEAL-07 | Phase 213 | Complete |
| HEAL-08 | Phase 213 | Complete |
| COORD-01 | Phase 214 | Complete |
| COORD-02 | Phase 214 | Complete |
| COORD-03 | Phase 214 | Complete |
| COORD-04 | Phase 214 | Complete |
| LEARN-01 | Phase 215 | Complete |
| LEARN-02 | Phase 215 | Complete |
| LEARN-03 | Phase 215 | Complete |
| LEARN-04 | Phase 215 | Complete |
| LEARN-05 | Phase 215 | Complete |
| LEARN-06 | Phase 215 | Complete |
| LEARN-07 | Phase 215 | Complete |
| LEARN-08 | Phase 215 | Complete |
| LEARN-09 | Phase 215 | Complete |
| TEST-01 | Phase 216 | Complete |
| TEST-02 | Phase 216 | Complete |
| TEST-03 | Phase 216 | Complete |
| TEST-04 | Phase 216 | Complete |

**Coverage:**
- v1 requirements: 37 total
- Mapped to phases: 37/37
- Unmapped: 0

---
*Requirements defined: 2026-03-26*
*Last updated: 2026-03-26 after adding HEAL-06/07/08 (Audit Protocol methodology, live-sync, toggle) and LEARN-07/08/09 (self-patch loop)*
