---
phase: 182-cross-milestone-integration
plan: "01"
status: complete
started: 2026-03-25
completed: 2026-03-25
---

# Phase 182-01 Summary: Cross-Milestone Integration

## What was done

Updated all active milestones to reference v22.0 capabilities, eliminating duplicate infrastructure and ensuring future phases use the unified OTA pipeline, feature flags, and config push.

### XMIL-01: v6.0 Salt Fleet Management
- Already DEPRECATED (2026-03-25) — confirmed fully superseded by v22.0
- Config push (CP-01-06) replaces Salt's config distribution
- OTA pipeline (OTA-01-10) replaces Salt's deploy orchestration
- Comms-link relay covers remaining remote exec

### XMIL-02: v10.0 Phase 62 (Fleet Config Distribution)
- Marked as SUPERSEDED in ROADMAP.md with strikethrough
- v22.0 Config Push (CP-01 to CP-06) provides: WebSocket-based push, schema validation, per-pod queuing, offline delivery via sequence-number ack, audit logging

### XMIL-03: v13.0 Multi-Game Launcher
- Added v22.0 integration note to milestone description
- Future game additions should use runtime feature flags (FF-01) for per-pod game enablement
- Cargo feature gates (CF-01) for telemetry modules
- AC EVO (Phase 86) already uses compile-time flag — future games use runtime flags

### XMIL-04: v15.0 Phase 111 (Code Signing + Canary)
- Updated phase description to reference OTA-10 (SHA256 binary identity) and OTA-02 (canary Pod 8)
- No duplicate canary infrastructure — use v22.0 wave-based rollout
- Added gate-check.sh requirement

### XMIL-05: v17.0 Phase 127 (CI/CD Pipeline)
- Updated phase description to reference OTA-08 (deploy state machine)
- Cloud and local deploy share the same pipeline architecture
- Added gate-check.sh requirement

### XMIL-06: Standing Rules Gate Dependency
- Added global note to Phase Numbering section in ROADMAP.md
- All future phases must run `bash test/gate-check.sh --pre-deploy` before shipping
- Enforced automatically by OTA pipeline for binary deploys

## Requirements covered
All 6: XMIL-01, XMIL-02, XMIL-03, XMIL-04, XMIL-05, XMIL-06

## Files modified
- `.planning/ROADMAP.md` — Phase 62 superseded, Phase 111/127 updated, v13.0 integration note, Phase 182 marked complete, gate-check.sh global dependency
- `.planning/REQUIREMENTS.md` — XMIL-01 through XMIL-06 marked complete
