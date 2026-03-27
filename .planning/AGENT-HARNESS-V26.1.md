# v26.1 — Agent Harness Integration

**Status:** Design Phase | **Author:** Bono + Claude Opus 4.6 | **Date:** 2026-03-27
**Depends on:** v26.0 Meshed Intelligence (Phases 222-233)
**Goal:** Apply Anthropic's 6 agent harness principles to the Meshed Intelligence fleet.

## 6 Principles Applied

| # | Principle | Implementation |
|---|-----------|---------------|
| 1 | Structured Agent Harness | Code-enforced state machine: DETECT→HYPOTHESIZE(≥3)→TEST(≥1)→EVALUATE→VALIDATE→PROMOTE |
| 2 | No Self-Approval | Evaluator MUST be different model than diagnostician (adversarial tension) |
| 3 | Graded Evaluation | 4-criterion rubric: Root Cause Accuracy 35%, Fix Completeness 25%, Verification Evidence 25%, Side Effect Safety 15% |
| 4 | Context Anxiety | Compaction + hard reset at 80% context for small-window models. Opus 4.6 uses compaction only. |
| 5 | Evolve with Model | model-assumptions.toml registry — 8 constraints with removal conditions. Quarterly audit. |
| 6 | Ralph Wiggum Loop | Deterministic checks (ProcessAlive, HttpStatus, PortOpen) that AI cannot override. Max 5 loops. |

## Phase Plan

| Phase | Name | Owner | Days |
|-------|------|-------|------|
| 240 | Diagnostic Harness State Machine | James | 2 |
| 241 | Adversarial Evaluator Role | James | 1 |
| 242 | Graded Evaluation Rubric | Bono | 1 |
| 243 | Ralph Wiggum Validation Loop | James | 2 |
| 244 | Context Management for Pod Engines | James | 1 |
| 245 | Model Assumption Audit | Bono | 0.5 |
| 246 | Unified Protocol Integration | Bono | 0.5 |

## Success Criteria

1. No self-approved solutions in fleet KB (different evaluator model required)
2. Ralph Wiggum compliance — no promotion without deterministic validation
3. Every solution graded on 4-criterion rubric
4. No diagnostic session truncated by context overflow
5. All harness constraints documented in model-assumptions.toml
6. Protocol phases are code-gated, not just documented

## Shared Types (in rc-common/src/mesh_types.rs)

- SolutionGrade: 4-criterion weighted rubric with evaluator_model tracking
- DiagnosticSession: harness state machine with phase progression
- DiagnosticPhase: Detect→Hypothesize→Test→Evaluate→Validate→Promote→Escalated
- Hypothesis: per-hypothesis tracking with status (Untested/Testing/Confirmed/Eliminated)
- ValidationCheck: deterministic check for Ralph Wiggum loop

See UNIFIED-PROTOCOL.md v3.1 for full integration into lifecycle phases.
