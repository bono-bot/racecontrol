# Phase 273 Plan 02: Safety Guardrails Summary

**One-liner:** Blast radius limiter (RAII FixGuard), per-action circuit breaker, and idempotency tracker integrated into tier engine pre-flight checks.

## Objective

Implement three safety guardrails (SAFE-01, SAFE-02, SAFE-03) to prevent cascading damage from the tier engine's autonomous fix application. All types centralized in `rc-common/src/safety.rs` with integration in `tier_engine.rs`.

## Tasks Completed

| Task | Description | Status |
|------|-------------|--------|
| 1 | BlastRadiusLimiter with RAII FixGuard (SAFE-01) | Done |
| 2 | PerActionCircuitBreaker (SAFE-02) | Done |
| 3 | IdempotencyTracker with TTL cleanup (SAFE-03) | Done |
| 4 | SafetyGuardrails combined facade | Done |
| 5 | Integration in tier_engine.rs run_supervised loop | Done |
| 6 | Unit tests (6 tests, all passing) | Done |
| 7 | cargo check -p rc-common -p rc-agent-crate | Done |

## Implementation Details

### SAFE-01: BlastRadiusLimiter
- `Mutex<HashMap<String, ActiveFix>>` with tight lock scopes (no lock across .await)
- Max 2 concurrent fixes globally, max 1 per action type
- RAII `FixGuard` auto-releases slot on drop
- Handles mutex poisoning gracefully (recovers inner data)
- Duplicate fix_id detection prevents double-application

### SAFE-02: PerActionCircuitBreaker
- Independent breaker per action type (vs existing single global CircuitBreaker)
- Default: 3 consecutive failures opens breaker, 300s cooldown
- Half-open state after cooldown allows retry
- Success resets the breaker for that action type only
- `snapshot()` method exposes all breaker states for diagnostics
- Integrated in run_supervised: records success/failure per action type after run_tiers

### SAFE-03: IdempotencyTracker
- `Mutex<HashMap<String, Instant>>` with configurable TTL (default 600s)
- Key = `{node_id}:{rule_version}:{incident_fingerprint}`
- `check_and_record()` is atomic: checks + records in single lock scope
- Auto-cleanup when map exceeds 500 entries
- `is_duplicate()` peek method for read-only checks

### SafetyGuardrails Facade
- `pre_check()` runs all three checks in order: circuit breaker -> idempotency -> blast radius
- Returns `Result<FixGuard, String>` — Ok means all checks passed, Err gives reason
- Single instantiation in `run_supervised()`, shared across all events

### tier_engine.rs Integration
- Safety guardrails instantiated once in `run_supervised()` alongside existing CircuitBreaker
- Pre-check runs after dedup but BEFORE `run_tiers()` is called
- Action type derived from DiagnosticTrigger discriminant
- Incident fingerprint uses existing `make_dedup_key()` for consistency
- Per-action circuit breaker feedback loop: success/failure recorded after run_tiers completes

## Key Files

| File | Action | Lines |
|------|--------|-------|
| `crates/rc-common/src/safety.rs` | Created | ~460 |
| `crates/rc-common/src/lib.rs` | Modified | +1 (module declaration) |
| `crates/rc-agent/src/tier_engine.rs` | Modified | +28 (integration) |

## Decisions Made

1. Used `std::sync::Mutex<HashMap>` instead of DashMap — avoids adding a new dependency, and all operations are synchronous with tight lock scopes
2. Kept existing single CircuitBreaker intact — the new PerActionCircuitBreaker runs in parallel as an additional safety layer, not a replacement
3. Used `std::mem::discriminant` for action type derivation from DiagnosticTrigger — stable across enum variants without needing Display impl

## Verification

- `cargo check -p rc-common`: PASS (0 errors)
- `cargo check -p rc-agent-crate`: PASS (0 errors, 23 pre-existing warnings)
- `cargo test -p rc-common -- safety`: PASS (6/6 tests)
- No `.unwrap()` in production code
- No locks held across `.await`
- All `tokio::spawn` in existing code already has lifecycle logging (not modified)

## Deviations from Plan

None — plan executed as specified.

## Commits

| Hash | Message |
|------|---------|
| `f310d4b0` | feat(273): safety guardrails — blast radius, circuit breaker, idempotency (SAFE-01..03) |
