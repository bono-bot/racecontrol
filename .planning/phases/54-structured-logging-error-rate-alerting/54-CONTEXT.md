# Phase 54: Structured Logging + Error Rate Alerting - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

racecontrol and rc-agent emit structured JSON logs with daily file rotation so incidents can be investigated with jq; racecontrol watches its own error rate and emails James and Uday when it exceeds a configurable threshold.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation decisions delegated to Claude. The following are sensible defaults:

**JSON Log Format:**
- Switch both racecontrol and rc-agent from text to JSON format via `tracing_subscriber::fmt::layer().json()`
- Fields: timestamp, level, message, target (module path), span context
- rc-agent logs should include `pod_id` field (from config) for fleet-wide log aggregation
- racecontrol already has `rolling::daily` — keep it, just switch format
- rc-agent has `rolling::never` — switch to `rolling::daily` to match racecontrol
- Log file naming: `racecontrol-YYYY-MM-DD.jsonl` and `rc-agent-YYYY-MM-DD.jsonl`
- Keep stdout layer as human-readable text (JSON is for files only)

**Error Rate Thresholds:**
- Default: 5 errors in 1 minute triggers email alert
- Configurable via `racecontrol.toml`: `error_rate_threshold` and `error_rate_window_secs`
- Only `tracing::error!()` level counts (not warn)
- Use existing `state.email_alerter` with `should_send` rate limiting (already handles 30-min cooldown)
- Recipients: james@racingpoint.in and usingh@racingpoint.in (existing pattern)

**Log Retention:**
- Keep 30 days of logs
- Startup cleanup: delete `.jsonl` files older than 30 days from `logs/` directory
- Simple approach: check file modification time on startup, no background task needed

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Current Logging Implementation
- `crates/racecontrol/src/main.rs` — Current tracing_subscriber setup with rolling::daily + text format
- `crates/rc-agent/src/main.rs` — Current tracing_subscriber setup with rolling::never + text format
- `crates/racecontrol/Cargo.toml` — tracing-appender already in dependencies
- `crates/rc-agent/Cargo.toml` — tracing-appender already in dependencies

### Existing Alert System
- `crates/racecontrol/src/main.rs` — `state.email_alerter` with should_send rate limiting
- `.planning/research/FEATURES.md` §Monitoring — structured logging as monitoring foundation

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tracing_appender::rolling::daily` already in use for racecontrol — just need format change
- `state.email_alerter` already handles rate-limited email sending via send_email.js
- `tracing_subscriber` JSON feature likely needs enabling in Cargo.toml

### Established Patterns
- Dual-layer tracing: stdout (human-readable) + file (currently text, switching to JSON)
- Non-blocking file writer via `tracing_appender::non_blocking()`
- Email alerts via shell-out to `send_email.js` (PROJECT.md constraint: no SMTP crate)

### Integration Points
- `racecontrol.toml` — add error_rate_threshold and error_rate_window_secs config
- `rc-agent.toml` — pod_id already in config, needs to be injected into JSON log layer
- `logs/` directory on both server and pods

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Key constraint: use existing tracing-subscriber + tracing-appender stack, don't add new logging libraries.

</specifics>

<deferred>
## Deferred Ideas

- Prometheus /metrics endpoint (MON-08) — v9.x future
- Structured log search via MCP — depends on this phase, future consideration
- Log aggregation across pods — Netdata (Phase 55) may cover this

</deferred>

---

*Phase: 54-structured-logging-error-rate-alerting*
*Context gathered: 2026-03-20*
