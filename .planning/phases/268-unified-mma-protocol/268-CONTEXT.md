# Phase 268: Unified MMA Protocol - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

A reusable 5-model MMA protocol library that any survival layer can invoke for diagnosis, with cost guardrails, structured findings, and fallback to deterministic rules when OpenRouter is unreachable. Implements the Unified MMA Protocol which incorporates all 4 layers of Unified Protocol v3.1.

Requirements: MP-01 through MP-09.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from research:
- OpenRouter client: implement the `OpenRouterDiagnose` trait from rc-common (defined in Phase 267)
- 5-model roster: Qwen3 235B (Scanner), DeepSeek R1 (Reasoner), DeepSeek V3 (Code Expert), Devstral 2 (SRE/Ops), Gemini 2.5 Pro (Security)
- Fact-checker role: one model cross-references findings against standing rules
- Dual reasoning mode: both thinking and non-thinking variants in same session
- Cost guard: check budget_state.json before launching, abort if insufficient
- Budget persistence: write to budget_state.json after every OpenRouter call
- Deterministic fallback: rule engine for when OpenRouter is unreachable (>3 failures)
- Per-pod child API keys: provisioned via management key API at deploy time
- Training flag: sessions tagged training=true during 30-day window
- Model validation gate: >90% agreement benchmark before switching to cheaper models
- This module lives in rc-common or a new shared crate — must be importable by rc-watchdog, rc-agent, racecontrol, and rc-guardian
- OpenRouter management key: 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 (for key provisioning only)
- Child key for MMA: environment variable OPENROUTER_KEY on each machine

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- crates/rc-agent/src/openrouter.rs — existing 5-model OpenRouter client (async, uses reqwest)
- crates/rc-agent/src/budget_tracker.rs — existing per-node cost tracking
- crates/rc-agent/src/tier_engine.rs — existing 5-tier decision tree
- crates/rc-common/src/survival_types.rs — OpenRouterDiagnose trait, DiagnosisContext, DiagnosisResult, DiagnosisFinding

### Established Patterns
- reqwest for HTTP calls to OpenRouter
- serde for JSON serialization of prompts and responses
- tracing for structured logging
- Budget tracking in SQLite (existing MI v26.0)

### Integration Points
- rc-watchdog needs blocking/sync MMA calls (no tokio runtime in main loop)
- rc-agent already has async OpenRouter calls
- racecontrol needs async MMA for fleet-wide diagnosis
- rc-guardian (new Linux binary) needs async MMA

</code_context>

<specifics>
## Specific Ideas

- The protocol must work in BOTH sync (rc-watchdog) and async (everything else) contexts
- Use the existing openrouter.rs as a reference but extract to rc-common
- Budget state must persist to JSON file, not just SQLite (for rc-watchdog which has no SQLite)
- The 5-model roster should be configurable via TOML/env, not hardcoded

</specifics>

<deferred>
## Deferred Ideas

- N-iteration convergence (FUT-01) — requires training data
- Night-ops integration — Phase 270+ concern

</deferred>
