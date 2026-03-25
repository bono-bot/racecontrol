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

- [ ] **CH-01**: Phase 07 allowlist check spot-verifies known-good process (svchost.exe) is present, not just count >= 100
- [ ] **CH-02**: Phase 25 menu check verifies at least one item has available=true
- [ ] **CH-03**: Phase 39 feature flags check verifies at least one flag with enabled=true exists
- [ ] **CH-04**: Phase 56 OpenAPI check spot-verifies 3-5 critical endpoints by name (app-health, flags, guard/whitelist)

### Config Validation (CV) — Verify credentials and config values are correct

- [ ] **CV-01**: Phase 02 config check validates ws_connect_timeout >= 600ms in racecontrol.toml
- [ ] **CV-02**: Phase 02 config check validates app_health monitoring URLs contain correct ports
- [ ] **CV-03**: Phase 30 WhatsApp check tests Evolution API live connection state
- [ ] **CV-04**: Phase 31 email check proactively verifies OAuth token expiry date

### Dashboard/UI (UI) — Verify user-facing pages render correctly

- [ ] **UI-01**: Phase 20 kiosk check verifies static file serving from pod perspective (_next/static/ returns 200)
- [ ] **UI-02**: Phase 26 game catalog check verifies kiosk game selection page renders expected game count
- [ ] **UI-03**: Phase 44 check verifies Next.js cameras page at :3200/cameras loads

### Cross-Service (XS) — Verify the dependency chain end-to-end

- [ ] **XS-01**: Phase 35+36 cloud sync check compares venue and cloud driver updated_at timestamps (< 5 min delta)
- [ ] **XS-02**: Phase 07+09 cross-check verifies allowlist background task ran recently when safe_mode inactive

### Structural Fixes (SF) — False PASSes and inoperative checks

- [ ] **SF-01**: Phase 19 display resolution check measures resolution via rc-agent exec (not hardcoded 1920x1080)
- [ ] **SF-02**: Phase 21 billing endpoint unreachable returns WARN during venue hours (not PASS)
- [ ] **SF-03**: Phase 53 watchdog check treats ps_count=0 as WARN (watchdog dead)

### Operational (OP) — Startup reliability

- [ ] **OP-01**: go2rtc warmup integrated into start-rcsentry-ai.bat (prevents NVR RTSP flood)

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
| CV-01 | Phase 202 | Pending |
| CV-02 | Phase 202 | Pending |
| CV-03 | Phase 202 | Pending |
| CV-04 | Phase 202 | Pending |
| SF-01 | Phase 202 | Pending |
| SF-02 | Phase 202 | Pending |
| SF-03 | Phase 202 | Pending |
| OP-01 | Phase 202 | Pending |
| WL-01 | Phase 203 | Pending |
| WL-02 | Phase 203 | Pending |
| WL-03 | Phase 203 | Pending |
| WL-04 | Phase 203 | Pending |
| CH-01 | Phase 203 | Pending |
| CH-02 | Phase 203 | Pending |
| CH-03 | Phase 203 | Pending |
| CH-04 | Phase 203 | Pending |
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
*Requirements defined: 2026-03-26*
*Last updated: 2026-03-26 — traceability added after roadmap creation*
