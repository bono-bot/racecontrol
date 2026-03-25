# Project Research Summary

**Project:** v25.0 — Debug-First-Time-Right
**Domain:** Rust verification framework — observable state machines, boot resilience, silent failure elimination
**Researched:** 2026-03-26 IST
**Confidence:** HIGH

## Executive Summary

v25.0 is a quality infrastructure retrofit, not a greenfield feature. The goal is systematic elimination of the 11 multi-attempt debugging incidents documented in this codebase, which averaged 2.4 fix attempts each and stemmed from 7 recurring root cause categories: proxy verification, silent failures, incomplete root cause analysis, manual fixes without code enforcement, boot-time transient failures, context/semantic mismatches, and missing first-run verification on guards. The recommended approach is additive — new modules layer onto the existing 4-crate structure (rc-common, rc-agent, racecontrol, rc-sentry) using primitives already in the workspace. Only one new Cargo dependency is justified (`notify 8.2.0` for sentinel file watching via OS-level `ReadDirectoryChangesW`); all other patterns are built from `tracing 0.1`, `tokio::sync::watch`, `tokio::time::interval`, `thiserror 2`, and `eprintln!`.

The architecture concentrates on two complementary mechanisms: making failures visible at the moment they occur (not after downstream symptoms appear) and enforcing correctness patterns as code rather than process guidelines. The `VerificationChain` type in rc-common wraps existing parse/transform paths at the call site so intermediate failures become observable without touching the final health endpoint. The `ObservableState` pattern fires an `eprintln!` plus `tracing::warn!` before any sentinel file is written or config fallback is activated — ensuring degraded states are emitted even before the tracing subscriber is initialized. The `boot_resilience.rs` module formalizes the periodic re-fetch pattern already applied to the process guard allowlist (commit `821c3031`) and extends it to feature flags and any future startup-fetched resource.

The primary risk for this milestone is scope creep in the wrong direction: building a verification framework only for new code while leaving the 6 known silent failure categories in existing code untouched. Research is unambiguous that v25.0 must include a retroactive sweep of existing failure sites — specifically the 7 `unwrap_or("http://127.0.0.1:8080")` silent fallbacks in `main.rs`, the missing WhatsApp alert on `MAINTENANCE_MODE` writes, and the `spawn().is_ok()` false confirmation in rc-sentry `restart_service()`. A framework that only instruments new paths will not reduce the documented incident average.

---

## Key Findings

### Recommended Stack

The stack for v25.0 is almost entirely the existing workspace. Zero new runtime infrastructure is needed. The philosophy: "most of v25.0 is patterns and macros built on existing deps, not new crates." The workspace already has `tracing 0.1`, `tokio 1`, `thiserror 2`, `serde_json 1`, and `anyhow 1` — all are used directly.

The single justified addition is `notify = "8.2"` added to workspace deps. This replaces silent polling loops (`loop { sleep(1s); if path.exists() }`) with OS-level `ReadDirectoryChangesW` events for sentinel file detection. It is the only new crate that cannot be replicated in 15 lines with existing primitives.

**Core technologies:**

- `tracing 0.1` (workspace) — chain-of-verification spans, state transition events, structured warn/error fields; the primary tool for 4 of 7 v25.0 goals
- `tokio::sync::watch` (tokio 1 workspace) — observable state values with current-value semantics; prefer over `broadcast` for state (not events)
- `tokio::time::interval` (tokio 1 workspace) — periodic re-fetch loops for all startup-fetched resources; same primitive as existing allowlist re-fetch
- `thiserror 2` (workspace) — typed `VerificationError` enum variants per pipeline stage (InputParseError, TransformError, DecisionError, ActionError)
- `eprintln!` (std) — mandatory for all errors before `tracing_subscriber::init()`; the only output path that works before logging is initialized
- `notify 8.2.0` (new) — filesystem event watcher for sentinel files on Windows via `ReadDirectoryChangesW`; zero-CPU idle cost vs polling; justified because no existing primitive achieves event-driven detection

**What NOT to add:** OpenTelemetry stack, Prometheus/metrics crates, state machine crates for existing state machines (`statig` is correct for NEW state machines only), `tokio-retry` crate (overkill vs 15-line custom pattern), `anyhow::Context` for chain verification (unstructured; use `thiserror` variants for machine-parseable stage names).

### Expected Features

Features map directly to the 7 root cause categories from the 11-incident retrospective.

**Must have — P1 (directly prevents documented incident patterns):**

- `MAINTENANCE_MODE` write alert — WhatsApp alert within 30s of sentinel write; prevents 1.5-hour silent pod death (3 pods dark)
- Config fallback observability — `warn!` on every `unwrap_or("http://127.0.0.1:8080")` site; 6 known silent fallbacks confirmed in `main.rs`; prevents dev URL in production
- Empty allowlist on boot alert — `error!` before logging init when process guard enabled but allowlist empty; prevents 28,749 false violations/day recurrence
- Sentinel file creation alerts via `fleet_alert.rs` — `GRACEFUL_RELAUNCH`, `OTA_DEPLOYING`, `MAINTENANCE_MODE` all visible in fleet health dashboard
- Cause Elimination template enforcement — bash `fix_log.sh` helper that prompts for 5-step process before LOGBOOK entry; zero Rust changes required
- Domain-matched verification gate — pre-ship checklist that classifies change type (display/network/parse/billing/config) and requires the corresponding verification domain; addresses 8+ proxy verification incidents

**Should have — P2 (closes structural gaps):**

- Chain-of-verification helpers — `VerifyStep` trait + `info_span!` per step for the 4 critical chains (UDP→billing, config→URL, curl→stdout→parse, allowlist→enforcement)
- Startup enforcement bat scanner — compare deployed bat hash on all 8 pods against canonical in `self_heal.rs`; closed-loop (detect + deploy canonical + verify), not report-only
- Boot resilience for feature flags — periodic re-fetch to match the `821c3031` allowlist pattern
- Bat drift detector in fleet audit — extends existing v23.0 `audit.sh` phase framework
- First-scan validation for guards — log first 10 violations when a guard flips from disabled to enabled; require operator acknowledgment before enforcement proceeds

**Defer to v26+ — P3 (quality measurement, deferred):**

- Fix attempt counter per incident — requires structured LOGBOOK adoption first
- Boot resilience scorecard in dashboard — requires `startup_log` enrichment with fetch outcomes
- Automated parse-chain smoke tests — HIGH complexity; add alongside each chain as fixed, not upfront

**Anti-features (do not build):**

- Universal "super health" endpoint — creates a new proxy to replace old proxies; the root problem
- Real-time debugging dashboard with live state streams — incidents had enough data in existing logs; the failure was process discipline, not data availability
- LLM-gated fix declarations — adds external dependency to every debugging session; structured template achieves same outcome with zero dependencies
- Global debug mode flag — use `RUST_LOG=rc_agent::billing_guard=debug` per-target instead

### Architecture Approach

v25.0 adds 4 new modules and modifies 11 existing modules across the 4-crate structure. No new crate is warranted. The build order is imposed by Rust type dependencies: rc-common must stabilize before consumers compile. Server-side changes (racecontrol) are lower risk than pod-side session logic (rc-agent) and build after.

The critical architectural constraint: verification results travel over the **existing AgentMessage WebSocket channel** and the **existing RecoveryLogger JSONL path**. No new protocol message variants unless no existing variant fits AND both sides are upgraded atomically — rolling pod deploys mean mismatched versions silently drop unknown variants.

The hot-path/cold-path distinction is non-negotiable: billing start, game launch, session end, and WS message handling are hot paths — verification must be async fire-and-forget to a ring buffer, never blocking. Config load, allowlist fetch, and periodic health checks are cold paths — synchronous verification is acceptable.

**Major components (new):**

1. `rc-common/verification.rs` — `VerificationChain`, `VerificationStep`, `Verdict` types; ~100 LOC; consumed by all three executables
2. `rc-agent/observable_state.rs` — `StateTransitionKind` enum + `emit_transition()`; ~80 LOC; fires `eprintln!` + `tracing::warn!` before every sentinel write
3. `rc-agent/boot_resilience.rs` — generic `spawn_periodic_refetch()` scaffold with lifecycle logging (start / first-success / exit); ~60 LOC; formalizes the `821c3031` pattern
4. `racecontrol/verification_gate.rs` — pre-ship domain-matched gate runner; ~150 LOC; wired into `gate-check.sh` Suite 0

**Modified modules (additive only — no existing behavior removed):**

- `rc-agent`: `startup_log.rs`, `event_loop.rs`, `pre_flight.rs`, `self_monitor.rs`, `process_guard.rs`, `feature_flags.rs`
- `racecontrol`: `pod_monitor.rs`, `pod_healer.rs`, `config.rs`, `fleet_health.rs`
- `rc-sentry`: `watchdog.rs` — append to `RecoveryLogger` on Suspect AND Crashed transitions (currently Crashed-only)

### Critical Pitfalls

1. **Chain checks the endpoint, not the chain** — All 8 proxy verification incidents used health endpoint probes while intermediate parse steps failed silently (e.g. `"200"` with surrounding quotes failing `u32::parse()` — 4 deploy cycles declared PASS). Prevention: `VerificationChain` must instrument the specific link that failed in each prior incident, not the final endpoint. A failing parse must be visible as `ParseStep::Failed("value='\"200\"'")` without reproducing the full chain from outside.

2. **Observable transitions that log but don't alert** — `MAINTENANCE_MODE` was written with no WhatsApp alert; process guard loaded empty allowlist and logged at DEBUG; neither notified staff. Prevention: degraded-state transitions MUST write to WhatsApp via Evolution API OR fleet health dashboard. A passive log line at any level is structurally insufficient — the operator was not watching, and nobody was alerted.

3. **Boot resilience that fetches once and assumes success** — Process guard fetched empty allowlist when server was transiently down at boot, then never re-fetched; 28,749 false violations/day for 2+ days. Prevention: every startup-fetched resource needs a periodic background re-fetch loop AND an observable state transition when self-heal occurs. Boot retry and periodic re-fetch are complementary, not alternatives.

4. **Pre-ship verification that checks the wrong domain** — Blanking screen deployed 4 times with "health OK, build_id matches" as PASS while the visual output was broken for every customer. Next.js deploys declared healthy from server-side curl while `_next/static/` returned 404. Prevention: domain-matched gate must be a blocking checklist. Visual change = visual check from venue, cannot be substituted by any terminal-accessible probe.

5. **Bat auditing that reports without fixing** — ConspitLink flickering was fixed 3 times manually same day; each time the fix regressed because `start-rcagent.bat` was not updated. Prevention: bat audit must be closed-loop — detect divergence, deploy canonical bat from git, verify enforcement lines present. Additionally, 4 Windows-specific silent traps exist in bat files (UTF-8 BOM from Write tool, parentheses in if/else, `/dev/null` instead of `nul`, `timeout` in non-interactive context) — auditor must detect "present but broken" enforcement lines, not just missing ones.

6. **Silent failure sweep that only covers new code** — Building the framework without retroactively fixing the 6 known silent failures leaves the same incident categories open for the next audit cycle. The 6 known failures: `spawn().is_ok()` in rc-sentry `restart_service()`, empty allowlist DEBUG log, config parse before tracing init, MAINTENANCE_MODE 30-min auto-clear gap, feature flag DB unreachable with no alert, SSL/TOML corruption with no detection. Prevention: v25.0 phases must include an explicit retroactive fix pass, not just framework construction.

7. **Verification framework adding latency to the billing hot path** — Synchronous verification steps on billing start delay the session and change timing relative to UDP telemetry; the PlayableSignal gate (v24.0) makes this timing-sensitive. Prevention: all verification on billing/game launch/WS paths must be async fire-and-forget. Measure `billing_start_latency_ms` before and after merging the framework; must be within 5ms of baseline.

---

## Implications for Roadmap

Based on combined research, 6 phases are warranted. Build order is imposed by Rust type dependencies (rc-common must stabilize before consumers compile) and operational risk (server-side racecontrol changes are lower risk than pod-side rc-agent session logic).

### Phase 1: Verification Chain Foundation

**Rationale:** All other phases depend on the `VerificationChain` type in rc-common. This must stabilize first so rc-agent and racecontrol can consume it without type churn. The hot-path/cold-path async distinction must be specified in the framework design here — getting it wrong breaks billing latency fleet-wide and requires a rewrite. Also includes the retroactive audit for `tracing::error!` calls before `tracing_subscriber::init()` — the pre-tracing logging gap is a prerequisite for boot resilience phases to be meaningful.
**Delivers:** `rc-common/verification.rs` (VerificationChain, VerificationStep, Verdict), pre-tracing error buffer pattern in `main.rs`, async fire-and-forget pattern spec for hot paths, retroactive audit of pre-tracing error paths
**Addresses:** Chain-of-verification helpers (P1), logging initialization gap (Pitfall 6), billing hot path risk (Pitfall 7)
**Avoids:** Proxy verification failure mode — framework data model must represent each step as a distinct trackable unit before any code path is wrapped
**Research flag:** Standard patterns — well-defined from codebase analysis; skip `/gsd:research-phase`

### Phase 2: Observable State Transitions

**Rationale:** The highest-impact silent failure eliminations are all in this phase (MAINTENANCE_MODE, config fallback, empty allowlist, sentinel fleet alerts). These are LOW complexity changes with HIGH incident reduction per effort. They must come before boot resilience because observable state transitions are what make boot self-heal events visible when they occur. The retroactive sweep of all existing sentinel write sites belongs here.
**Delivers:** `rc-agent/observable_state.rs`, `emit_transition()` wired into all 7 sentinel write sites, WhatsApp alert on MAINTENANCE_MODE write, `warn!` on all 6 `unwrap_or("http://127.0.0.1:8080")` sites in `main.rs`, `fleet_alert.rs` integration for sentinel events, retroactive sweep of known silent failures in existing code
**Addresses:** MAINTENANCE_MODE alert (P1), config fallback observability (P1), sentinel file creation alerts (P1), empty allowlist on boot alert (P1), silent failure sweep (Pitfall 8)
**Avoids:** Alert-only vs log-only distinction (Pitfall 2) — must wire to WhatsApp/fleet dashboard, not just tracing; alert storm rate limiting required (max 1 alert per sentinel type per pod per 5 minutes)
**Research flag:** Standard patterns — existing `fleet_alert.rs` channel and WhatsApp integration are well-understood; skip `/gsd:research-phase`

### Phase 3: Boot Resilience Formalization

**Rationale:** The periodic re-fetch scaffold for feature flags follows the `process_guard.rs` reference pattern exactly (commit `821c3031`). Sequenced after Phase 2 so that boot self-heal events ("allowlist recovered after boot failure — 47 entries loaded") have `emit_transition()` available to make them visible. This phase formalizes the pattern as a reusable module and applies it to all remaining startup-fetched resources.
**Delivers:** `rc-agent/boot_resilience.rs` with `spawn_periodic_refetch()`, feature flags periodic re-fetch (60s), recovery event emitted when re-fetch succeeds after boot failure, retroactive sweep of all `load_or_default()` callsites for missing periodic re-fetch
**Addresses:** Boot resilience for feature flags (P2), boot-time transient failure category, retroactive sweep of `load_or_default()` sites (Pitfall 3)
**Avoids:** "Fetch once and assume success" anti-pattern — boot-time retry and periodic re-fetch are complementary; allowlist hysteresis (apply new allowlist only if it has >= 80% of previous entry count) prevents flip-flop from transient server errors
**Research flag:** Standard patterns — reference implementation is commit `821c3031` in this repo; skip `/gsd:research-phase`

### Phase 4: Pre-Ship Verification Gate

**Rationale:** The domain-matched verification gate addresses the single most recurring failure mode in this codebase (8+ proxy verification incidents). It is operational tooling — no Rust compile dependency — and can be developed in parallel with Waves 2-3. Sequenced before bat auditing because the bat deploy step in Phase 5 is itself a deploy that should pass through the gate.
**Delivers:** Domain-mapped verification table (display/network/parse/billing/config) as a blocking checklist in `gate-check.sh` Suite 0, `racecontrol/verification_gate.rs` for server-side gate runner, integration with existing GSD execute-phase output template
**Addresses:** Domain-matched verification gate (P1), proxy verification root cause category
**Avoids:** Terminal-accessible verification for visual changes (Pitfall 4) — gate must explicitly require visual confirmation for display-affecting deploys; "verified" from SSH terminal for a visual change must be a gate failure
**Research flag:** Standard patterns — change taxonomy maps to existing standing rule categories in CLAUDE.md; skip `/gsd:research-phase`

### Phase 5: Startup Bat Auditing

**Rationale:** Bat enforcement is the deepest recurring regression pattern in this codebase (ConspitLink fixed 3 times same day; power plan, USB suspend, process kill list all regressed on subsequent deploys). Sequenced after Phase 4 so the bat deploy step itself goes through the domain-matched gate.
**Delivers:** `audit/startup/` tier in the v23.0 audit runner, bat hash comparison against canonical in `self_heal.rs`, auto-deploy of canonical bat to divergent pods (4-concurrent cap, 500ms stagger), post-deploy `findstr` verification of enforcement lines, 4-trap detection (BOM, parentheses, `/dev/null`, `timeout`)
**Addresses:** Startup enforcement bat scanner (P2), bat drift detector in audit (P2), first-scan validation for guards (P2)
**Avoids:** Report-without-fixing anti-pattern (Pitfall 5) — audit must detect AND deploy AND verify; "present but broken" enforcement lines must be distinguished from missing lines (Pitfall 9)
**Research flag:** Standard patterns — v23.0 audit framework, rc-sentry /files endpoint, and canonical bat in `self_heal.rs` all exist; skip `/gsd:research-phase`

### Phase 6: Cause Elimination Process Enforcement

**Rationale:** Process enforcement is the lowest-complexity, highest-discipline-leverage change in v25.0. It closes the loop on the "incomplete root cause analysis" category (3+ incidents where a plausible artifact was fixed without testing other hypotheses). Sequenced last so the template is validated against real Phase 1-5 fix work before being enforced on all core binary commits.
**Delivers:** `fix_log.sh` bash helper with 5-step Cause Elimination template, pre-commit hook that checks for template section in commits touching rc-agent/racecontrol/rc-sentry, `LOGBOOK.md` structured format adoption, emergency bypass field (`emergency: true` with reason, logged for post-incident review)
**Addresses:** Cause Elimination template enforcement (P1), structured fix log (P2), incomplete root cause analysis category
**Avoids:** Stopping at "found a crash dump" (Pitfall 10) — template must list ALL hypotheses considered and eliminated, not just the confirmed cause; template must not block urgent hotfixes (emergency bypass exists)
**Research flag:** Standard patterns — pure process tooling; skip `/gsd:research-phase`

### Phase Ordering Rationale

- **Wave 1 before Wave 2 (Rust):** `VerificationChain` types in rc-common must compile before rc-agent and racecontrol consume them. Rust type dependencies impose this constraint absolutely — no workaround.
- **Observable state (Phase 2) before boot resilience (Phase 3):** Boot self-heal events ("allowlist recovered after boot failure") need `emit_transition()` to be visible. Building boot resilience without observability recreates the silent recovery pattern.
- **Pre-ship gate (Phase 4) before bat auditing (Phase 5):** The bat deploy step in Phase 5 is itself a deploy and must pass through the Phase 4 domain-matched gate. Sequencing ensures the gate is available.
- **Cause Elimination (Phase 6) last:** Template is validated against real Phase 1-5 fix work before being enforced. Enforcing it before the framework exists adds overhead without the corresponding tooling benefit.
- **Phases 4-6 parallelize with Waves 2-3:** No Rust compile dependency — bat scripts, gate-check.sh integration, and fix_log.sh can be developed alongside rc-agent/racecontrol work.

### Research Flags

Phases likely needing deeper research during planning: **None.** All 6 phases build on primitives verified directly in the codebase (existing module code, existing patterns, documented incidents). No phase requires external API research or niche domain knowledge.

Phases with standard patterns (skip `/gsd:research-phase`):
- **Phase 1:** VerificationChain derived from codebase analysis; tracing/tokio patterns are HIGH confidence from existing workspace deps
- **Phase 2:** Alert channels (`fleet_alert.rs`, WhatsApp Evolution API) are existing and tested infrastructure
- **Phase 3:** Reference implementation is commit `821c3031` in this repo
- **Phase 4:** Change taxonomy maps exactly to standing rule categories already in CLAUDE.md
- **Phase 5:** v23.0 audit framework, rc-sentry /files, and canonical bat in `self_heal.rs` all exist
- **Phase 6:** Pure process tooling — bash script + commit hook

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Workspace Cargo.toml confirmed directly; `notify 8.2.0` verified on docs.rs with Windows platform support (`windows-sys ^0.60`); all other patterns use existing workspace deps with zero new additions |
| Features | HIGH | Features derived from 11 documented incidents in this exact codebase — not generic research. 7 root cause categories from direct post-mortem. Code inspection confirmed 7 `unwrap_or` fallbacks, empty allowlist DEBUG log, MAINTENANCE_MODE write-without-alert |
| Architecture | HIGH | Based on direct codebase analysis of all 4 crates: app_state.rs, event_loop.rs, self_monitor.rs, process_guard.rs, feature_flags.rs, protocol.rs, recovery.rs. Component boundaries, data flow, and build order derived from actual source |
| Pitfalls | HIGH | Every pitfall is sourced from documented production incidents in CLAUDE.md and PROJECT.md. 4 bat syntax traps confirmed from multiple failed deploys. Billing hot path latency risk confirmed from PlayableSignal billing gate timing in v24.0. No hypothetical pitfalls |

**Overall confidence: HIGH**

### Gaps to Address

- **`notify` crate in non-interactive service context:** `notify 8.2.0` uses `ReadDirectoryChangesW` which is confirmed Windows-supported, but behavior when rc-agent runs via HKLM Run (no desktop session, no message pump) needs validation. If it fails in this context, the fallback is a 500ms polling loop — acceptable latency for sentinel detection. Validate during Phase 1 implementation with `cargo test` on Pod 8.

- **`VerificationChain` allocation on billing hot path:** Architecture specifies async fire-and-forget to a ring buffer for hot paths, but the exact ring buffer mechanism needs implementation validation. The existing `RecoveryEventStore` in recovery.rs is a candidate — verify it can accept `VerificationChain` records without blocking the billing path. Measure `billing_start_latency_ms` before and after during Phase 1.

- **WhatsApp alert storm rate limiting:** Observable state transitions could fire rapidly during a crash storm (MAINTENANCE_MODE written repeatedly). The existing `app_health_monitor.rs` alert channel has rate limiting — verify the same rate limiting is applied to new sentinel alerts. Address during Phase 2 wiring. Max 1 alert per sentinel type per pod per 5 minutes is the recommended threshold.

- **Canonical bat comparison method:** `self_heal.rs` embeds canonical bat content, but hash vs line-by-line diff needs a decision before Phase 5. Hash is faster but hides WHICH enforcement lines are missing. Line-by-line diff by category (process kills / power settings / singleton guards / sentinel clears) is recommended — the bat audit value is in the diff, not just pass/fail.

---

## Sources

### Primary (HIGH confidence)

- `crates/rc-agent/src/main.rs` — confirmed 6x `unwrap_or("http://127.0.0.1:8080")` and 1x `unwrap_or("127.0.0.1")` silent fallbacks
- `crates/rc-agent/src/startup_log.rs` — phase-based startup log, `AtomicBool LOG_INITIALIZED`, crash detection pattern
- `crates/rc-agent/src/self_heal.rs` — canonical bat content embedded in binary (bat drift detector baseline)
- `crates/racecontrol/src/process_guard.rs` — "empty allowlist is almost certainly a config loading failure" comment with no enforcement; `821c3031` periodic re-fetch reference
- `crates/racecontrol/src/fleet_alert.rs` — existing WhatsApp alert channel confirmed for sentinel alert reuse
- `crates/rc-common/src/recovery.rs` — RecoveryLogger, RecoveryAuthority, JSONL path
- `crates/rc-common/src/protocol.rs` — AgentMessage / CoreToAgentMessage protocol; new variant introduction constraint
- `workspace Cargo.toml` — existing dep versions, confirmed absence of `notify`
- `.planning/PROJECT.md` — v25.0 milestone spec, 7 root cause categories, 11 incidents, avg 2.4 attempts
- `CLAUDE.md` standing rules — 12 debugging rules each tracing to a specific incident; `eprintln!` rule; bat syntax traps; domain-matched verification rule

### Secondary (MEDIUM confidence)

- `docs.rs/notify/8.2.0/notify/` — Windows support confirmed (`windows-sys ^0.60`), `RecommendedWatcher` API, `tokio::sync::mpsc` bridge pattern
- `docs.rs/statig/0.4.1/statig/` — `before_transition`/`after_transition` hooks; not recommended for existing state machines
- MEMORY.md — 2026-03-24/25 audit records: MAINTENANCE_MODE incident (Pods 5/6/7, 1.5h dark), empty allowlist incident (2+ hours, 28,749 violations/day), SSH banner TOML corruption

### Tertiary (informational)

- PITFALLS-v17.1-watchdog-ai.md — `spawn().is_ok()` false confirmation, MAINTENANCE_MODE silent block; directly applicable incident records
- PITFALLS.md (v23.0 audit runner) — cmd.exe quoting traps, SSH banner corruption, curl quote stripping (applicable to bat auditing and verification tooling)

---

*Research completed: 2026-03-26 IST*
*Ready for roadmap: yes*
