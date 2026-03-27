# Requirements: v26.0 Meshed Intelligence — Self-Healing AI Fleet

**Defined:** 2026-03-27
**Core Value:** Every node diagnoses itself, solutions propagate fleet-wide, no issue debugged twice
**Owner split:** James (Phases 217-221, 224-226) | Bono (Phases 222-223, 227-228)

## Diagnostic Engine (James — Phase 217)
- [x] **DIAG-01**: Each node detects anomalies automatically (health fail, crash, game fail, display mismatch, error spike, WS disconnect, sentinel, violation spike)
- [ ] **DIAG-02**: Tier 1 deterministic fixes applied without AI (clear MAINTENANCE_MODE, kill orphans, restart service)
- [ ] **DIAG-03**: Tier 2 KB lookup matches problem signatures against local + fleet KB before models
- [ ] **DIAG-04**: Tier 3 single-model diagnosis (Qwen3, ~$0.05) when KB has no match
- [ ] **DIAG-05**: Tier 4 full 4-model parallel diagnosis (R1+V3+MiMo+Gemini, ~$3) when Tier 3 fails
- [ ] **DIAG-06**: Tier 5 human escalation via WhatsApp when all automated tiers fail
- [x] **DIAG-07**: Engine runs on 5-min scheduled scan + anomaly trigger

## Knowledge Base (James — Phase 218)
- [ ] **KB-01**: Local SQLite KB: solutions with problem_key, symptoms, root_cause, fix_action, confidence, cost, source_node
- [ ] **KB-02**: Experiments table prevents duplicate work across fleet
- [ ] **KB-03**: Problem signature normalization (error_type + error_code + component + context_hash)
- [ ] **KB-04**: Environment fingerprinting (OS, GPU driver, build_id, hardware_class)
- [ ] **KB-05**: Confidence scoring: success / (success + fail), auto-demotion on failure
- [ ] **KB-06**: TTL expiration: 90 days unused → auto-archive

## OpenRouter Integration (James — Phase 219)
- [ ] **API-01**: Rust HTTP client calls OpenRouter with model-specific prompts + structured parsing
- [ ] **API-02**: 4 models: Qwen3 (scanner), DeepSeek R1 (reasoner), DeepSeek V3 (code), Gemini 2.5 Pro (security)
- [ ] **API-03**: Model registry: version pinning, fallback chain, quarterly review flag
- [ ] **API-04**: Role-specific system prompts (Reasoner, Code Expert, SRE, Security)
- [ ] **API-05**: Response parsing: root_cause, confidence, fix_action, risk_level

## Budget Manager (James — Phase 220)
- [ ] **BUDGET-01**: Per-node daily tracking ($10/pod, $20/server) with midnight IST reset
- [ ] **BUDGET-02**: Per-incident cost tracking
- [ ] **BUDGET-03**: Hard ceiling: block model calls when daily budget exhausted
- [ ] **BUDGET-04**: Graceful degradation: ceiling → mechanical fallback, never blocks ops
- [ ] **BUDGET-05**: Monthly tracking with configurable alerts + hard stop
- [ ] **BUDGET-06**: Budget status on health endpoint for dashboard

## Mesh Gossip Protocol (James — Phase 221)
- [ ] **MESH-01**: Solution announcement via existing WS (compact digest)
- [ ] **MESH-02**: Solution request: fetch full details from any peer
- [ ] **MESH-03**: Experiment announcement: prevent duplicate fleet-wide diagnosis
- [ ] **MESH-04**: Heartbeat with KB bloom filter for sync detection
- [ ] **MESH-05**: First-responder rule: only first node spends model budget
- [ ] **MESH-06**: Environment-aware propagation: same env → apply, different → candidate

## Server Coordinator (Bono — Phase 222)
- [ ] **COORD-01**: Fleet KB aggregation from all pods
- [ ] **COORD-02**: Canary promotion: 3+ successes across 2+ pods → fleet-verified
- [ ] **COORD-03**: Hardening: 10+ successes, 0 failures → Tier 1 deterministic
- [ ] **COORD-04**: Pattern detection: 3+ pods same symptom in 5 min → systemic alert
- [ ] **COORD-05**: Solution demotion: confidence <0.5 → back to candidate
- [ ] **COORD-06**: Fleet-wide experiment dedup

## Admin Dashboard (Bono — Phase 223)
- [ ] **DASH-01**: Mesh Intelligence page at :3201/mesh-intelligence
- [ ] **DASH-02**: Real-time solution feed
- [ ] **DASH-03**: Budget tracker per node with charts
- [ ] **DASH-04**: Model performance metrics
- [ ] **DASH-05**: Knowledge Base browser with search/filter/promote/retire

## Predictive Maintenance (James — Phase 224)
- [ ] **PRED-01**: ConspitLink reconnection rate trending → USB alert
- [ ] **PRED-02**: Edge process count trending → memory leak restart
- [ ] **PRED-03**: GPU temp >80C → thermal alert
- [ ] **PRED-04**: rc-agent restart >2/day → stability alert
- [ ] **PRED-05**: Disk <10GB → auto-cleanup
- [ ] **PRED-06**: Error spike 3+ pods → systemic alert

## Customer Experience Scoring (James — Phase 225)
- [ ] **CX-01**: Per-pod score: launch success (30%), session completion (25%), display (20%), hardware (15%), billing (10%)
- [ ] **CX-02**: Score <80% → flagged for maintenance
- [ ] **CX-03**: Score <50% → auto-removed from rotation + alert
- [ ] **CX-04**: Fleet average on dashboard

## Night Operations (James — Phase 226)
- [ ] **NIGHT-01**: Midnight maintenance: health → diagnose → fix → audit → report
- [ ] **NIGHT-02**: Windows Update check + install + reboot + verify off-hours
- [ ] **NIGHT-03**: Apply pending fleet-verified fixes
- [ ] **NIGHT-04**: Morning report to Uday: "Fleet ready"

## Multi-Venue Cloud KB (Bono — Phase 227)
- [ ] **CLOUD-01**: Fleet KB syncs to Bono VPS cloud DB
- [ ] **CLOUD-02**: New venue pulls entire KB on day 1
- [ ] **CLOUD-03**: Cross-venue solution propagation

## Fleet Intelligence Reports (Bono — Phase 228)
- [ ] **REPORT-01**: Weekly report: issues, auto-resolved, escalated, MTTR, budget, KB growth
- [ ] **REPORT-02**: Per-model performance tracking
- [ ] **REPORT-03**: Recommendations: model swaps, new Tier 1 checks, patterns
- [ ] **REPORT-04**: Delivered via WhatsApp to Uday

## Traceability

| Phase | Owner | Requirements | Count |
|---|---|---|---|
| 229 | James | DIAG-01 to DIAG-07 | 7 |
| 230 | James | KB-01 to KB-06 | 6 |
| 231 | James | API-01 to API-05 | 5 |
| 232 | James | BUDGET-01 to BUDGET-06 | 6 |
| 233 | James | MESH-01 to MESH-06 | 6 |
| 234 | Bono | COORD-01 to COORD-06 | 6 |
| 235 | Bono | DASH-01 to DASH-05 | 5 |
| 236 | James | PRED-01 to PRED-06 | 6 |
| 237 | James | CX-01 to CX-04 | 4 |
| 238 | James | NIGHT-01 to NIGHT-04 | 4 |
| 239 | Bono | CLOUD-01 to CLOUD-03 | 3 |
| 240 | Bono | REPORT-01 to REPORT-04 | 4 |
| **Total** | | | **62** |
