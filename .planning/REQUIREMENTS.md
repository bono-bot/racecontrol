# Requirements: v31.0 Autonomous Survival System

**Defined:** 2026-03-30
**Core Value:** No single system failure can kill the healing brain — 3 independent survival layers with Unified MMA Protocol

## Survival Foundation (SF)

- [x] **SF-01**: HEAL_IN_PROGRESS sentinel file with JSON payload (layer, started_at, action, ttl_secs) checked by all 5 recovery systems before acting
- [x] **SF-02**: Server-arbitrated heal lease — component requests lease from server for specific pod, server grants with TTL, healer renews while working, expired lease frees pod for other healers
- [x] **SF-03**: Structured action_id logging — every cross-layer operation (diagnosis, fix, rollback, escalation) shares a correlation ID for tracing
- [x] **SF-04**: Survival types in rc-common — SurvivalReport, HealLease, BinaryManifest, DiagnosisContext structs + OpenRouter client trait (trait only, no reqwest)
- [x] **SF-05**: Recovery coordination protocol — existing rc-sentry, RCWatchdog, self_monitor, pod_monitor, WoL all check HEAL_IN_PROGRESS + OTA_DEPLOYING before acting

## Smart Watchdog — Layer 1 (SW)

- [ ] **SW-01**: Binary SHA256 validation — rc-watchdog reads release-manifest.toml, computes SHA256 of rc-agent.exe, blocks launch if mismatch
- [ ] **SW-02**: PE header validation — rc-watchdog checks DOS_MAGIC, PE_MAGIC, COFF_MACHINE_X86_64, TimeDateStamp via goblin crate before launching rc-agent
- [ ] **SW-03**: Automatic rollback to rc-agent-prev.exe if new binary fails health poll within 30s of launch
- [ ] **SW-04**: Rollback depth tracking in rollback-state.json — max depth 3, then escalate to Layer 2 ("both binaries bad")
- [ ] **SW-05**: Startup health poll loop — 3 attempts at 10s intervals before declaring healthy or escalating to MMA
- [ ] **SW-06**: Direct HTTP survival reporting to server /api/v1/pods/{id}/survival-report bypassing dead rc-agent
- [ ] **SW-07**: MAINTENANCE_MODE auto-clear on confirmed clean binary + clean health poll after validated startup
- [ ] **SW-08**: Unified MMA Protocol diagnosis when restart loop detected (>2 fails in 10 min) — 5 top-tier models via OpenRouter
- [ ] **SW-09**: Dedicated async runtime thread in watchdog for OpenRouter calls — never block the main service poll loop
- [ ] **SW-10**: OpenRouter fallback to deterministic rule engine when API unreachable (>3 consecutive failures)
- [ ] **SW-11**: Manifest-driven health expectations — watchdog validates expected build_id from manifest, not just HTTP 200
- [ ] **SW-12**: Budget persistence to disk — daily spend stored in budget_state.json, survives reboots, enforces $10/day cap
- [ ] **SW-13**: Post-rollback WhatsApp alert to staff via server relay
- [ ] **SW-14**: MMA_DIAGNOSING sentinel with TTL (separate from MAINTENANCE_MODE) — prevents concurrent diagnosis

## Server Fleet Healer — Layer 2 (FH)

- [ ] **FH-01**: SSH diagnostic runner — server SSHes into dark pods, runs predefined diagnostic scripts, returns structured JSON
- [ ] **FH-02**: SSH diagnostic fingerprinting — map known output patterns (tasklist, netstat, Event Log) to symptom JSON schema
- [ ] **FH-03**: Fleet-pattern detection — same failure on 3+ pods within 5 min triggers single MMA session (not 8 parallel)
- [ ] **FH-04**: Repair confidence gate — only dispatch autonomous fix if confidence >= 0.8 AND fix_type is deterministic or config (never code_change)
- [ ] **FH-05**: Autonomous Tier 1 fix dispatch via SSH — apply deterministic fixes from fleet KB remotely
- [ ] **FH-06**: Post-fix behavioral verification — poll /health for build_id match AND /debug for edge_process_count > 0
- [ ] **FH-07**: Canary rollout — fix applied to Pod 8 first, wait for verification, then gradual (3 pods, then remaining)
- [ ] **FH-08**: Pod isolation before risky repair — write MAINTENANCE_MODE via rc-sentry before repair, clear after verification
- [ ] **FH-09**: Repair audit trail — every SSH command + response logged to incident_log table with action_id
- [ ] **FH-10**: Layer 1 report ingestion — survival_coordinator receives and stores watchdog survival reports
- [ ] **FH-11**: Billing safety check — never restart or repair a pod with an active billing session (check has_active_billing_session())
- [ ] **FH-12**: New server endpoint POST /api/v1/pods/{id}/survival-report for watchdog direct reporting

## External Guardian — Layer 3 (EG)

- [ ] **EG-01**: Server health polling from Bono VPS every 60s via HTTP to /api/v1/health
- [ ] **EG-02**: Dead-man detection — 3 consecutive missed polls (3 min) declares server dead
- [ ] **EG-03**: Server restart via Tailscale SSH — schtasks /Run /TN StartRCTemp after dead-man trigger
- [ ] **EG-04**: Billing safety check — check /api/v1/fleet/health for active_billing_sessions before restart
- [ ] **EG-05**: WhatsApp escalation when restart fails or is unsafe (active sessions during peak hours)
- [ ] **EG-06**: Status distinction — dead (connection refused) vs busy (HTTP 200 but slow) vs unreachable (timeout)
- [ ] **EG-07**: Graduated restart — soft (schtasks) first, hard (taskkill + start) if soft fails, report-only if hard fails
- [ ] **EG-08**: Guardian heartbeat to James via comms-link every 6h or on any triggered event
- [ ] **EG-09**: GUARDIAN_ACTING coordination — shared state via comms-link WS prevents James and Bono guardians from acting simultaneously
- [ ] **EG-10**: New rc-guardian crate — standalone binary for Bono VPS (Linux target), deployed via pm2/systemd

## Unified MMA Protocol (MP) — 4-Step Convergence Engine

**Spec:** `.planning/specs/UNIFIED-MMA-PROTOCOL.md`
**Designed via MMA:** 10 models, 2 iterations, consensus-driven (2026-03-31)

### 4-Step Protocol Core
- [ ] **MP-01**: Step 1 DIAGNOSE — 5 models × N iterations (min 2), consensus on all problems with evidence, assumptions, disproof criteria
- [ ] **MP-02**: Step 2 PLAN — 5 models × N iterations (min 2), consensus on fix plans with risk analysis, rollback strategy, verification steps
- [ ] **MP-03**: Step 3 EXECUTE — 5 models × N iterations (min 2), consensus on best solution, apply fix
- [ ] **MP-04**: Step 4 VERIFY — deterministic checks (P6 Ralph Wiggum) + 1 adversarial model sanity check (P2, different model from Steps 1-3)
- [ ] **MP-05**: Backtracking — ANY Step 4 failure triggers full backtrack to Step 1 with failure evidence appended. Max 3 backtracks → human escalation
- [ ] **MP-06**: Different models per backtrack (fresh perspective, P2 adversarial principle)

### Consensus Engine
- [ ] **MP-07**: 3/5 majority consensus logic — findings with <3/5 agreement rejected or re-iterated
- [ ] **MP-08**: StepConsensus JSON schema — majority findings, dissenting opinions (capped 2-3), problem IDs, assumptions[], verification_steps[], per-model confidence_scores[]
- [ ] **MP-09**: Convergence detection — iteration N adds <2 semantically new findings vs N-1 (semantic dedup, not string match)
- [ ] **MP-10**: Max 4 iterations per step, then human escalation via WhatsApp

### Model Management
- [ ] **MP-11**: Domain roster mapping — 6 domains (Rust, Frontend, Windows, Network, Security, Hardware) → 10-model pool each with primary/secondary tiers
- [ ] **MP-12**: Step-level model pools — Step 1 biased toward reasoners, Step 2 toward architects, Step 3 toward coders, Step 4 uses 1 cheap model
- [ ] **MP-13**: Stratified shuffle — each iteration of 5 MUST include ≥1 reasoner + ≥1 code expert + ≥1 SRE. Remaining 2 randomized from pool
- [ ] **MP-14**: 10 unique paid models per step, shuffled/randomized between iterations within that step

### Cost & Budget
- [ ] **MP-15**: Cost guard — budget pre-check before each step, abort if insufficient
- [ ] **MP-16**: Training mode — Q3 always yes, all issues get full 4-step protocol (2026-03-31 to 2026-04-29)
- [ ] **MP-17**: Cost-tiered model strategy — expensive models (R1, Gemini Pro) reserved for Steps 1+4, cheap/fast models for Step 3

### Integration
- [ ] **MP-18**: Q1-Q4 gate integration — full 4-step protocol fires when Q3 authorizes MMA
- [ ] **MP-19**: Step-specific prompt templates (diagnose/plan/execute — fundamentally different prompts)
- [ ] **MP-20**: 4-criterion rubric scoring in Step 4 (Root Cause Accuracy 35%, Fix Completeness 25%, Verification Evidence 25%, Side Effect Safety 15%)

## v31.x Requirements (Deferred)

### Future (after training period)

- **FUT-01**: N-iteration convergence — run MMA until 3 consecutive rounds with 0 new P1/P2 findings
- **FUT-02**: Night-ops autonomous maintenance window (02:00-05:00 IST) — full fleet SSH sweep + Tier 1 apply
- **FUT-03**: Graduated repair scope — single pod → pod class (hardware_class) → fleet
- **FUT-04**: API key lifecycle management — provisioning, rotation, revocation in deploy pipeline
- **FUT-05**: Predictive repair trigger — trending metric threshold crosses pre-emptive action
- **FUT-06**: Cross-region KB sync — Guardian triggers KB sync after server restart
- **FUT-07**: Binary age monitoring — alert if binary > 7 days without re-deploy

## Out of Scope

| Feature | Reason |
|---------|--------|
| Replace rc-sentry entirely | Layer 1 extends rc-watchdog, not rc-sentry. Sentry's 6 endpoints remain for fallback ops |
| Guardian diagnosing pods directly | Guardian watches server only. Layer 2 watches pods. Separation of concerns |
| Custom model fine-tuning | Training period collects data; fine-tuning is a separate future initiative |
| Light mode/mobile UI for survival dashboard | Backend-only milestone; dashboard integration deferred |
| Replacing OpenRouter with self-hosted models | 30-day training uses cloud; self-hosting is post-training decision |
| Code signing with real certificate | v15.0 AntiCheat scope; not duplicated here |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SF-01 | Phase 267 | Complete |
| SF-02 | Phase 267 | Complete |
| SF-03 | Phase 267 | Complete |
| SF-04 | Phase 267 | Complete |
| SF-05 | Phase 267 | Complete |
| MP-01 | Phase 268 | Pending |
| MP-02 | Phase 268 | Pending |
| MP-03 | Phase 268 | Pending |
| MP-04 | Phase 268 | Pending |
| MP-05 | Phase 268 | Pending |
| MP-06 | Phase 268 | Pending |
| MP-07 | Phase 268 | Pending |
| MP-08 | Phase 268 | Pending |
| MP-09 | Phase 268 | Pending |
| MP-10 | Phase 268 | Pending |
| MP-11 | Phase 268 | Pending |
| MP-12 | Phase 268 | Pending |
| MP-13 | Phase 268 | Pending |
| MP-14 | Phase 268 | Pending |
| MP-15 | Phase 268 | Pending |
| MP-16 | Phase 268 | Pending |
| MP-17 | Phase 268 | Pending |
| MP-18 | Phase 268 | Pending |
| MP-19 | Phase 268 | Pending |
| MP-20 | Phase 268 | Pending |
| SW-01 | Phase 269 | Pending |
| SW-02 | Phase 269 | Pending |
| SW-03 | Phase 269 | Pending |
| SW-04 | Phase 269 | Pending |
| SW-05 | Phase 269 | Pending |
| SW-06 | Phase 269 | Pending |
| SW-07 | Phase 269 | Pending |
| SW-08 | Phase 269 | Pending |
| SW-09 | Phase 269 | Pending |
| SW-10 | Phase 269 | Pending |
| SW-11 | Phase 269 | Pending |
| SW-12 | Phase 269 | Pending |
| SW-13 | Phase 269 | Pending |
| SW-14 | Phase 269 | Pending |
| FH-01 | Phase 270 | Pending |
| FH-02 | Phase 270 | Pending |
| FH-03 | Phase 270 | Pending |
| FH-04 | Phase 270 | Pending |
| FH-05 | Phase 270 | Pending |
| FH-06 | Phase 270 | Pending |
| FH-07 | Phase 270 | Pending |
| FH-08 | Phase 270 | Pending |
| FH-09 | Phase 270 | Pending |
| FH-10 | Phase 270 | Pending |
| FH-11 | Phase 270 | Pending |
| FH-12 | Phase 270 | Pending |
| EG-01 | Phase 271 | Pending |
| EG-02 | Phase 271 | Pending |
| EG-03 | Phase 271 | Pending |
| EG-04 | Phase 271 | Pending |
| EG-05 | Phase 271 | Pending |
| EG-06 | Phase 271 | Pending |
| EG-07 | Phase 271 | Pending |
| EG-08 | Phase 271 | Pending |
| EG-09 | Phase 271 | Pending |
| EG-10 | Phase 271 | Pending |

**Coverage:**
- v31.0 requirements: 56 total (SF:5 + MP:20 + SW:14 + FH:12 + EG:10)
- Mapped to phases: 56 (267:5 + 268:20 + 269:14 + 270:12 + 271:10)
- Unmapped: 0

---
*Requirements defined: 2026-03-30*
*Last updated: 2026-03-31 — MP requirements expanded from 9 to 20 after MMA research (10 models, 2 iterations)*
