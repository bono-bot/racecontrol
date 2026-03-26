# Phase 207: Boot Resilience - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Apply spawn_periodic_refetch() from boot_resilience.rs to feature flags and any other startup-fetched resources that lack periodic re-fetch. Document the architectural rule. Add first-scan validation for process guard enable transitions. This phase wires existing library code (Phase 205) into rc-agent consumers — no new rc-common types.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from requirements:
- Feature flags use spawn_periodic_refetch() with 5-minute interval (BOOT-02)
- Disk cache fallback emits "fallback-to-cache" event, self-heal emits "self_healed" event
- CLAUDE.md standing rules gains boot resilience architectural rule with resource checklist (BOOT-03)
- First-scan validation: >50% violation rate stays in report_only, requires GUARD_CONFIRMED fleet exec (BOOT-04)
- Feature flags currently loaded via WS FlagSync + disk cache (feature_flags.rs) — periodic re-fetch adds HTTP poll as fallback
- Process guard already has periodic allowlist re-fetch (commit 821c3031) — this is the model for feature flags

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rc-common/src/boot_resilience.rs` — spawn_periodic_refetch() with lifecycle logging (Phase 205)
- `rc-agent/src/feature_flags.rs` — FeatureFlags struct with load_from_cache(), apply_sync(), persist_to_disk()
- `rc-agent/src/process_guard.rs` — already has OBS-03 empty allowlist auto-response, needs BOOT-04 first-scan threshold
- `rc-agent/src/main.rs` — feature flags initialized as Arc<RwLock<FeatureFlags>>, passed to event loop

### Established Patterns
- Process guard allowlist re-fetch pattern (commit 821c3031) — 5-min tokio interval in process_guard.rs
- Feature flags propagation: WS FlagSync → apply_sync() → persist_to_disk() → write_sentry_flags()
- eprintln! for pre-tracing-init errors, tracing::warn! for post-init state transitions

### Integration Points
- rc-agent main.rs — spawn the periodic re-fetch task for feature flags after tracing init
- rc-agent process_guard.rs — add GUARD_CONFIRMED fleet exec handler + first-scan threshold logic
- CLAUDE.md — add standing rule to Boot Resilience / Debugging section

</code_context>

<specifics>
## Specific Ideas

- Feature flag periodic re-fetch should HTTP GET from the server's flag endpoint (same URL used by FlagSync WS), parse response, and call apply_sync() on the shared Arc<RwLock<FeatureFlags>>
- GUARD_CONFIRMED should be a fleet exec command that flips a guard_confirmed: AtomicBool, allowing process guard to escalate from report_only to kill_and_report

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
