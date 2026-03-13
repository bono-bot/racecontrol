# Phase 1: State Wiring & Config Hardening - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire EscalatingBackoff and EmailAlerter into AppState (already implemented, needs initialization); rc-agent validates critical config fields and fails fast with a branded lock screen error on bad config; pod-agent /exec returns honest HTTP status codes with exit code + stderr; deploy process overwrites stale config without backup. This is integration and hardening — not new design.

</domain>

<decisions>
## Implementation Decisions

### Config failure behavior
- rc-agent shows a branded error on the lock screen when config is invalid ("Configuration Error — contact staff") — no file paths or system details exposed
- Validate critical fields only: server URL (valid URL), pod number (1-8), billing rates (must be > 0), game paths. Optional fields use defaults.
- Validate field values, not just presence — catches typos like rate_30min = 0 that would give free sessions
- Fail immediately on invalid config — no retry delay. Watchdog/HKLM Run key handles restarts, so retry in the agent is redundant.

### Deploy config cleanup
- Overwrite old config without backup — source of truth is deploy-staging on James (.27), not what's on the pod
- One shared config template with only `pod_number` as the per-pod field — everything else identical across all 8 pods
- Deploy process: delete old racecontrol.toml → write new one → start rc-agent

### pod-agent error reporting
- /exec returns JSON with { success, exit_code, stdout, stderr } — enough for James to diagnose without SSHing into the pod
- Proper HTTP status codes: 200 success, 400 bad request (missing cmd field), 500 command execution failure
- 30s default timeout with override via { cmd: "...", timeout: 60 } in request body — prevents hung commands blocking the endpoint
- pod-agent binds to LAN only (192.168.31.x, not 0.0.0.0) — no auth needed, router NAT blocks external access. Note: pods DO have internet access for online games (iRacing, LMU).

### AppState wiring
- Pre-populate pod_backoffs entries for all 8 pods at rc-core startup — pod_monitor never encounters a missing entry
- Send a test email on first boot only (flag file after first success) — verifies Gmail OAuth works without spamming Uday's inbox on every restart
- Backoff step durations (30s→2m→10m→30m) hardcoded — no config file tuning for now
- No network ping check at startup — pods check in via WebSocket when they come online, "offline" is the default state

### Claude's Discretion
- Exact lock screen error message wording and styling
- Which config fields count as "optional" vs "critical" beyond the explicitly listed ones
- How pod_backoffs entries are keyed (pod_id string vs pod_number)
- Error log format for config validation failures

</decisions>

<specifics>
## Specific Ideas

- User wants config to be portable for future multi-venue expansion — one template that works anywhere, pod_number is the only per-pod difference
- Pods have internet access (iRacing, LMU online play) — security model should account for LAN exposure even though NAT blocks external access
- Existing deploy-staging workflow on James (.27:9998) is the source of truth for config generation

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EscalatingBackoff` (rc-common/src/watchdog.rs): Already implemented with tests, Vec<Duration> steps with clamping
- `EmailAlerter` (rc-core/src/email_alerts.rs): Already implemented with dual rate limits (per-pod 30min, venue-wide 5min)
- `AppState` (rc-core/src/state.rs): Already has `pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>` and `email_alerter: RwLock<EmailAlerter>` fields
- `load_config()` (rc-agent/src/main.rs:1249): Currently loads from rc-agent.toml or /etc/racecontrol/rc-agent.toml — needs validation logic added

### Established Patterns
- Config loading: TOML deserialization via `toml::from_str` — validation would be a post-parse step
- pod-agent: Node.js HTTP server on port 8090, /exec endpoint currently returns 200 for everything
- Deploy: HTTP download from James (.27:9998) via pod-agent /exec, or pendrive install.bat
- Lock screen: Edge browser showing branded HTML — can display error states

### Integration Points
- rc-agent main.rs: load_config() → add validation step → fail with branded lock screen error
- pod-agent /exec handler: change response format + HTTP status codes
- rc-core state.rs AppState::new(): pre-populate pod_backoffs from pod list
- rc-core startup: first-boot email test with flag file check
- deploy-staging scripts: update to delete old config before writing new

</code_context>

<deferred>
## Deferred Ideas

- Multi-venue support — config portability for new venues, venue_id field, centralized config management. Future project, not this phase.

</deferred>

---

*Phase: 01-state-wiring-config-hardening*
*Context gathered: 2026-03-13*
