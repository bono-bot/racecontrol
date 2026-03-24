# Phase 188: James Watchdog + rc-watchdog Grace Window - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Two deliverables: (1) Replace james_watchdog.ps1 with Rust-based AI watchdog in rc-watchdog crate using shared ollama.rs from rc-common, monitoring comms-link/go2rtc/rc-sentry-ai/Ollama with graduated response. (2) Add 30-second grace window to rc-watchdog's pod service that reads sentry-restart-breadcrumb.txt before acting, plus spawn verification after session1 launch.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key constraints from ROADMAP success criteria:
- james_watchdog.ps1 deleted from deploy-staging; rc-watchdog (Rust binary) monitors comms-link, go2rtc, rc-sentry-ai, Ollama
- Graduated response: count 1 wait → count 2 restart → count 3 Ollama diagnosis → count 4+ WhatsApp alert
- Health-poll verification: 500ms intervals for 10s after restart (same pattern as rc-sentry spawn verify)
- sentry-restart-breadcrumb.txt less than 30s old → rc-watchdog skips restart
- ollama.rs needs to be accessible from rc-watchdog (may already be in rc-common or needs moving)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-watchdog/` — existing Rust binary, Windows Service, already monitors some services
- `crates/rc-watchdog/src/session.rs` — WTSQueryUserToken + CreateProcessAsUser (Session 1 spawn)
- `crates/rc-sentry/src/ollama.rs` — Tier 3 LLM query (may need to move to rc-common or be duplicated)
- `crates/rc-common/src/recovery.rs` — RecoveryAuthority::JamesMonitor
- `deploy-staging/james_watchdog.ps1` — existing PowerShell watchdog to replace

### Established Patterns
- rc-watchdog uses tokio (unlike rc-sentry which is pure std)
- Service monitoring via health endpoint polling
- Graduated response already proven in james_monitor.rs pattern (count-based)

### Integration Points
- ollama.rs query to James .27:11434 for Tier 3 diagnosis
- WhatsApp alert via POST to server .23:8080/api/v1/fleet/alert
- sentry-restart-breadcrumb.txt written by rc-sentry after restart actions
- james_watchdog.ps1 removal from deploy-staging + Task Scheduler cleanup

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
