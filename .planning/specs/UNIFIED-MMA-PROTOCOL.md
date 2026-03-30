# Unified MMA Protocol v1.0

**Author:** Bono + Uday (MMA-researched: 10 models, 2 iterations)
**Date:** 2026-03-31
**Status:** SPEC — approved by Uday
**Affects:** v31.0 Phase 268, tier_engine.rs, openrouter.rs, knowledge_base.rs
**Supersedes:** MMA-FIRST-PROTOCOL.md (Q1-Q4 gate remains, this defines what happens INSIDE Q3)

---

## Summary

The Unified MMA Protocol combines the **Unified Protocol** (P1-P6 principles: diagnostic harness, adversarial evaluation, deterministic verification) with **MMA** (Multi-Model Audit: 5 models per iteration, N iterations, consensus) into a 4-step convergence engine.

When Q3 authorizes an MMA call, the system runs 4 sequential steps. Each step uses 5 models per iteration, minimum 2 iterations, until 3/5 majority consensus forms. Always optimized for Cold Path — get it right the first time, every time.

---

## Core Principles

1. **Never re-diagnose what's already solved** (Q1 KB gate handles this)
2. **3/5 majority = consensus** — minority opinions preserved for backtracking
3. **Each step has its own 10-model pool** — shuffled per iteration, stratified by role
4. **Always Cold Path** — thoroughness over speed, every time
5. **Deterministic verification cannot be bypassed** — AI consensus is not proof
6. **Any Step 4 failure triggers full backtrack** — even 1 failed check

---

## The 4-Step Protocol

```
Issue authorized by Q3
    |
    Step 1: DIAGNOSE ──→ consensus on ALL problems
    |   (5 models × N iterations, min 2)
    |
    Step 2: PLAN ──→ consensus on fix plans
    |   (5 models × N iterations, min 2)
    |   Input: Step 1 consensus
    |
    Step 3: EXECUTE ──→ consensus on best solution, applied
    |   (5 models × N iterations, min 2)
    |   Input: Step 2 consensus
    |
    Step 4: VERIFY ──→ deterministic checks + 1 model sanity
    |   ANY failure → backtrack to Step 1
    |   ALL pass → store in KB, gossip to fleet
    |
    Max 3 backtracks → human escalation (WhatsApp)
```

---

## Step 1: DIAGNOSE

**Goal:** Identify ALL problems with evidence and confidence scores.

### Iteration Flow
1. Select 5 models from Step 1's 10-model pool (stratified shuffle)
2. Send diagnostic prompt with issue context + context bundle
3. Collect 5 responses, extract findings
4. Build consensus: findings with 3/5+ agreement = confirmed
5. Iteration 2: shuffle pool, send confirmed findings + "what did we miss?"
6. Continue until convergence (iteration N adds <2 new findings vs N-1)

### Prompt Template
```
CONTEXT:
[Fleet context + issue trigger + context bundle + pod state]

TASK — STEP 1: DIAGNOSE
You are diagnosing a live issue on a Racing Point sim racing pod fleet.
List ALL possible root causes for this issue.

For EACH root cause, provide:
1. description: What is the problem?
2. severity: critical / high / medium / low
3. confidence: 0.0-1.0 (how certain are you?)
4. evidence: What log lines, metrics, or observations support this?
5. assumptions: What are you assuming to be true?
6. disproof: What would DISPROVE this hypothesis?

DO NOT suggest "restart" as a root cause.
If restarting fixes it, explain WHY restarting fixes it.

Output ONLY valid JSON array of findings.
```

### Convergence Criteria
- **Converged:** Iteration N produces <2 semantically new findings vs N-1
- **Semantic dedup:** Compare by problem category + affected component, not exact string
- **Max iterations:** 4 per step
- **Minimum:** 2 iterations

### Model Pool (10 models, biased toward reasoners)
Selected based on MMA research consensus (10 models, 2 iterations):

| Slot | Role | Models (choose based on domain) |
|------|------|------|
| 1 | Reasoner (required) | DeepSeek R1 0528, GPT-5.4 Nano, Kimi K2.5 |
| 2 | Code Expert (required) | DeepSeek V3.2, Grok Code Fast, Qwen3 Coder |
| 3 | SRE/Ops (required) | MiMo v2 Pro, Nemotron 3 Super, MiMo v2 Flash |
| 4 | Domain Specialist | Varies by issue domain (see Domain Roster below) |
| 5 | Generalist/Wildcard | Qwen3 235B, Gemini 2.5 Flash, Mistral Medium |
| 6-10 | Pool reserves | Filled from domain roster + remaining models |

**Stratified shuffle rule:** Each iteration of 5 MUST include ≥1 reasoner + ≥1 code expert + ≥1 SRE. Remaining 2 slots randomized from pool.

---

## Step 2: PLAN

**Goal:** Design fix plans for every confirmed problem from Step 1.

### Prompt Template
```
CONTEXT:
[Fleet context + Step 1 consensus (JSON)]

CONFIRMED PROBLEMS (from Step 1 consensus):
[majority_findings array]

DISSENTING OPINIONS (minority views — consider if majority is wrong):
[dissenting_opinions array]

TASK — STEP 2: PLAN
For EACH confirmed problem, design a fix plan.

For EACH plan, provide:
1. problem_id: Which problem this fixes (from Step 1)
2. actions: Ordered list of specific steps to apply the fix
3. fix_type: "deterministic" | "config" | "code_change" | "hardware"
4. risk_analysis: What could go wrong if we apply this fix?
5. rollback_strategy: How to undo this fix if it makes things worse
6. verification_steps: How to confirm the fix worked (deterministic checks)
7. side_effects: What else might this change affect?
8. estimated_duration: How long to apply (seconds)

For fix_type "code_change" or "hardware": mark requires_human = true.
NEVER auto-apply code changes or hardware modifications.

Output ONLY valid JSON array of plans.
```

### Model Pool (10 models, biased toward architects)
| Slot | Role | Models |
|------|------|--------|
| 1 | Architect (required) | Gemini 2.5 Pro, GPT-5.4 Nano, Mistral Large |
| 2 | SRE/Ops (required) | MiMo v2 Pro, Nemotron 3 Super |
| 3 | Code Expert (required) | DeepSeek V3.2, Grok Code Fast |
| 4-5 | Domain + Generalist | Varies |
| 6-10 | Pool reserves | Remaining models |

---

## Step 3: EXECUTE

**Goal:** Select and apply the best solution from the consensus plans.

### Prompt Template
```
CONTEXT:
[Fleet context + Step 1 consensus + Step 2 consensus]

FIX PLANS (from Step 2 consensus):
[majority plans array]

TASK — STEP 3: EXECUTE
Review these fix plans and select the BEST solution for each problem.

For EACH selected solution, provide:
1. problem_id: Which problem this fixes
2. selected_plan_index: Which plan from Step 2 you chose
3. implementation: The exact command, config change, or code to apply
4. execution_order: Priority order (fix critical issues first)
5. expected_outcome: What should change after applying this fix
6. confidence: 0.0-1.0 (how confident are you this will work?)

Prefer:
- deterministic fixes over config changes
- config changes over code changes
- Smallest reversible change that solves the problem

Output ONLY valid JSON array of executions.
```

### Model Pool (10 models, biased toward coders)
| Slot | Role | Models |
|------|------|--------|
| 1 | Code Expert (required) | Grok Code Fast, DeepSeek V3.2, Qwen3 Coder |
| 2 | Code Expert 2 (required) | Mercury Coder, GPT-5.1 Codex Mini |
| 3 | SRE/Ops (required) | MiMo v2 Flash, Nemotron 3 Super |
| 4-5 | Fast/Cheap models | Qwen3 235B 2507, Gemini 2.5 Flash |
| 6-10 | Pool reserves | Remaining models |

**Cost optimization (Gemini Iter 2 insight):** Step 3 prioritizes speed + code quality over deep reasoning. Use cheaper/faster models here. Save expensive reasoning models (R1, Gemini Pro, GPT-5.4) for Steps 1 and 4.

---

## Step 4: VERIFY

**Goal:** Deterministic proof that the fix actually worked. AI consensus is NOT proof.

### Verification Flow
1. **Deterministic checks first** (Ralph Wiggum — P6, cannot lie):
   - Process alive? (`tasklist /FI "IMAGENAME eq {name}"`)
   - Port open? (`netstat -an | findstr {port}`)
   - Health endpoint correct? (`curl /health` → check build_id, status)
   - Edge process count > 0? (if blanking/display issue)
   - Original symptom reproduced? (re-trigger the original diagnostic)
   - Custom checks from Step 2 `verification_steps[]`

2. **1 cheap model sanity check** (~$0.01):
   - Different model from any used in Steps 1-3 (P2 adversarial principle)
   - "Given this fix and these verification results, does this make logical sense?"
   - Grade on 4-criterion rubric (P3):
     - Root Cause Accuracy (35%): Did we fix the actual cause?
     - Fix Completeness (25%): Does it handle all variants?
     - Verification Evidence (25%): Is there concrete proof?
     - Side Effect Safety (15%): Could it break anything else?
   - Score ≥ 4.0 → PASS. Score 3.0-3.9 → flag. Score < 3.0 → FAIL.

3. **Result routing:**
   - ALL pass → store permanent fix in KB (stable hash), gossip to fleet
   - ANY failure → **backtrack to Step 1** with failure evidence appended

### Backtracking Rules
- **Any single failed check = full backtrack to Step 1**
- Each backtrack uses DIFFERENT models (fresh perspective — P2)
- Step 4 failure evidence is appended to Step 1 prompt as additional context
- **Max 3 backtracks** → human escalation via WhatsApp
- Backtrack prompt includes: "Previous fix attempt failed. Failure evidence: [...]"

---

## Domain Roster

Model selection per issue domain. Based on MMA research consensus (10 models, 2 iterations, 8/10+ agreement).

### Rust/Backend (rc-agent, racecontrol, Axum)
| Priority | Model | Why |
|----------|-------|-----|
| Primary | DeepSeek R1 0528 | Best reasoner for Rust ownership/async bugs |
| Primary | DeepSeek V3.2 | Strong systems code, memory safety |
| Primary | Qwen3 Coder | Rust syntax/idiom specialist |
| Primary | GPT-5.4 Nano | Good Rust reasoning at low cost |
| Primary | Grok Code Fast | Fast, strong Rust benchmarks |
| Secondary | Llama 4 Maverick | Broad systems knowledge |
| Secondary | Nemotron 3 Super | Enterprise Rust patterns |
| Secondary | Mistral Medium 3.1 | Balanced reasoning |
| Secondary | Mercury Coder | Code generation |
| Secondary | Kimi K2.5 | Adversarial edge cases |

### Node.js/Next.js Frontend (dashboard, admin, kiosk, PWA)
| Priority | Model | Why |
|----------|-------|-----|
| Primary | Grok 4.1 Fast | Fastest JS/TS specialist |
| Primary | GPT-5 Mini | Strong framework knowledge |
| Primary | Gemini 2.5 Pro | 1M context for monorepos |
| Primary | Mistral Large 2512 | Web dev breadth |
| Primary | Qwen3 235B 2507 | Async debugging |
| Secondary | DeepSeek V3.1 | Full-stack logic |
| Secondary | Seed 2.0 Mini | Component generation |
| Secondary | Kimi K2.5 | Edge case detection |
| Secondary | ERNIE 4.5 | Alternative perspective |
| Secondary | Llama 4 Maverick | React patterns |

### Windows OS (Session 0/1, registry, services, drivers)
| Priority | Model | Why |
|----------|-------|-----|
| Primary | GPT-5.4 Nano | Best Windows internals knowledge |
| Primary | DeepSeek R1 0528 | OS-level reasoning |
| Primary | Nemotron 3 Super | Enterprise Windows |
| Primary | MiMo v2 Pro | System administration |
| Primary | ERNIE 4.5 | Enterprise integration |
| Secondary | Qwen3 235B 2507 | Broad knowledge |
| Secondary | GLM 4.7 | Driver analysis |
| Secondary | Kimi K2.5 | Log analysis |
| Secondary | Grok 4.1 Fast | Fast iteration |
| Secondary | Mistral Medium 3.1 | Balanced |

### Network/WebSocket
| Priority | Model | Why |
|----------|-------|-----|
| Primary | DeepSeek V3.2 | Protocol analysis |
| Primary | Qwen3 235B 2507 | Connection state machines |
| Primary | MiMo v2 Pro | Distributed systems SRE |
| Primary | Gemini 2.5 Flash | Fast network logic |
| Primary | Kimi K2.5 | Real-time comms |
| Secondary | Nemotron 3 Super | Network topology |
| Secondary | Mistral Medium 3.1 | Protocol logic |
| Secondary | Llama 4 Maverick | Distributed nets |
| Secondary | DeepSeek R1 0528 | Deep reasoning |
| Secondary | GPT-5 Mini | Broad knowledge |

### Security (auth, credentials, injection, permissions)
| Priority | Model | Why |
|----------|-------|-----|
| Primary | Gemini 2.5 Pro | Proven credential scanner (84 findings) |
| Primary | GPT-5.4 Nano | Threat modeling |
| Primary | DeepSeek R1 0528 | Adversarial reasoning |
| Primary | MiMo v2 Pro | Vulnerability detection |
| Primary | Kimi K2.5 | Security architecture |
| Secondary | ERNIE 4.5 | CVE databases |
| Secondary | Grok 4.1 Fast | Adversarial training |
| Secondary | Mistral Large 2512 | Broad security |
| Secondary | Nemotron 3 Super | Enterprise hardening |
| Secondary | Qwen3 235B 2507 | Volume scanning |

### Hardware/GPU/Thermal
| Priority | Model | Why |
|----------|-------|-----|
| Primary | Gemini 2.5 Pro | Sensor/telemetry analysis |
| Primary | DeepSeek V3.2 | Low-level driver knowledge |
| Primary | Qwen3 235B 2507 | Broad hardware knowledge |
| Primary | Nemotron 3 Super | Enterprise hardware |
| Primary | GLM 4.7 | Driver analysis |
| Secondary | MiMo v2 Flash | Fast sensor interpretation |
| Secondary | ERNIE 4.5 | Hardware integration |
| Secondary | GPT-5 Mini | Broad knowledge |
| Secondary | Kimi K2.5 | Edge cases |
| Secondary | Llama 4 Maverick | Performance tuning |

---

## Consensus Schema

### StepConsensus (passed between steps)

```json
{
  "protocol_version": "1.0",
  "step": "DIAGNOSE",
  "step_number": 1,
  "iterations_completed": 2,
  "domain": "rust_backend",
  "majority_findings": [
    {
      "id": "P001",
      "description": "Thread deadlock in scheduler async task",
      "severity": "critical",
      "confidence": 0.92,
      "evidence": [
        {"type": "log", "content": "task_scheduler: timeout waiting for lock"},
        {"type": "metric", "content": "cpu_usage: 100% on core 3"}
      ],
      "assumptions": [
        "scheduler uses tokio::spawn for task dispatch",
        "lock is std::sync::Mutex, not tokio::sync::Mutex"
      ],
      "verification_steps": [
        "check tasklist /V for hung rc-agent threads",
        "check port 8090 responding to /health"
      ],
      "models_agreed": ["R1", "V3.2", "Grok", "Kimi"],
      "agreement_score": 0.8
    }
  ],
  "dissenting_opinions": [
    {
      "model": "MiMo",
      "finding": "Not a deadlock but memory exhaustion from leaked handles",
      "confidence": 0.65,
      "vote_count": 1
    }
  ],
  "models_used": ["R1", "V3.2", "MiMo", "Grok", "Kimi",
                   "GPT54N", "Qwen3", "Nemotron", "Gemini", "Mistral"],
  "total_cost": 0.45,
  "converged_at_iteration": 2,
  "timestamp": "2026-03-31T11:30:00Z"
}
```

### Consensus Rules

| Threshold | Definition | Action |
|-----------|-----------|--------|
| 5/5 unanimous | All models agree | Highest confidence — proceed |
| 4/5 strong | 4 agree, 1 dissents | High confidence — proceed, preserve dissent |
| 3/5 majority | 3 agree, 2 dissent | Minimum consensus — proceed, both dissents preserved |
| 2/5 or less | No majority | NOT consensus — run another iteration. If max iterations hit, escalate to human |

---

## Cost Model

### Per-Model Cost (diagnostic prompt ~2K tokens in, ~1K tokens out)

| Model | Cost/call (est.) | Category |
|-------|-----------------|----------|
| Qwen3 235B 2507 | $0.003 | Budget |
| Nemotron 3 Super | $0.005 | Budget |
| MiMo v2 Flash | $0.004 | Budget |
| Grok 4.1 Fast | $0.006 | Budget |
| Mistral Medium 3.1 | $0.008 | Budget |
| DeepSeek V3.2 | $0.008 | Mid |
| GPT-5.4 Nano | $0.012 | Mid |
| Mercury Coder | $0.010 | Mid |
| Grok Code Fast | $0.015 | Mid |
| ERNIE 4.5 | $0.015 | Mid |
| DeepSeek R1 0528 | $0.028 | Premium |
| Kimi K2.5 | $0.030 | Premium |
| GLM 4.7 | $0.020 | Premium |
| MiMo v2 Pro | $0.050 | Premium |
| Gemini 2.5 Pro | $0.110 | Expensive |
| Mistral Large 2512 | $0.025 | Mid |
| GPT-5 Mini | $0.020 | Mid |
| Llama 4 Maverick | $0.008 | Budget |

### Per-Incident Cost Estimate

| Scenario | Steps | Iterations | Models/iter | Est. Cost |
|----------|-------|-----------|-------------|-----------|
| **Quick resolve** (2 iter/step, no backtrack) | 4 | 2 each = 8 | 5 | $0.80-$2.00 |
| **Standard** (3 iter avg, no backtrack) | 4 | 3 each = 12 | 5 | $1.20-$3.00 |
| **Complex** (4 iter max, 1 backtrack) | 4×2 = 8 | 3 avg = 24 | 5 | $2.40-$6.00 |
| **Worst case** (4 iter, 3 backtracks) | 4×4 = 16 | 4 avg = 64 | 5 | $6.40-$16.00 |

### Training Period Budget (30 days)

| Day | New issues | KB hit rate | MMA calls | Daily cost |
|-----|-----------|-------------|-----------|------------|
| Day 1 | ~20 | 0% | 20 | $16-$40 |
| Day 7 | ~20 | 60% | 8 | $6-$16 |
| Day 14 | ~20 | 80% | 4 | $3-$8 |
| Day 30 | ~20 | 95% | 1 | $0.80-$2 |
| **30-day total** | | | | **$150-$400** |

Previous estimate was $800-$1,200. This protocol is **3-5x cheaper** because:
1. Actual per-model costs are much lower than estimated ($0.003-$0.05 vs $0.86)
2. Step 3 uses cheap/fast models
3. Step 4 is mostly deterministic ($0)

---

## Implementation Checklist

### Phase 268 Requirements (updated)

- [ ] **MP-01**: 5-model roster via OpenRouter — role-based prompts per step
- [ ] **MP-02**: Step 4 adversarial evaluator — DIFFERENT model from Steps 1-3, 4-criterion rubric
- [ ] **MP-03**: Domain roster mapping — issue type → 10-model pool per step
- [ ] **MP-04**: Cost guard — budget pre-check before each step
- [ ] **MP-05**: StepConsensus schema — structured JSON between steps with severity, assumptions, verification_steps
- [ ] **MP-06**: 3/5 majority consensus logic with semantic dedup
- [ ] **MP-07**: Training mode — Q3 always yes, all issues get full 4-step protocol
- [ ] **MP-08**: Convergence engine — iteration termination when <2 new findings
- [ ] **MP-09**: Stratified shuffle — ≥1 reasoner + ≥1 code expert + ≥1 SRE per iteration
- [ ] **MP-10**: Step 4 deterministic verification (Ralph Wiggum P6)
- [ ] **MP-11**: Backtracking — Step 4 fail → Step 1 with failure evidence, max 3 backtracks
- [ ] **MP-12**: Different models per backtrack (fresh perspective)
- [ ] **MP-13**: Per-model confidence_scores in consensus (not just aggregate)
- [ ] **MP-14**: Max 4 iterations per step, then human escalation
- [ ] **MP-15**: Step-level model pools (diagnose=reasoners, plan=architects, execute=coders)
- [ ] **MP-16**: Prompt templates per step (diagnose/plan/execute — different purposes)
- [ ] **MP-17**: Q1-Q4 gate integration — full protocol fires when Q3 authorizes

### Key Files to Modify

| File | Changes |
|------|---------|
| `tier_engine.rs` | Replace `tier4_multi_model()` with 4-step protocol engine |
| `openrouter.rs` | Add domain roster, step-specific model selection, consensus builder |
| `knowledge_base.rs` | Store StepConsensus alongside Solution, assumption tracking |
| `config.rs` | Domain roster config in `[mma]` section |
| `budget_tracker.rs` | Per-step budget tracking, per-incident cost accumulation |

---

## Decision Tree (Complete)

```
Issue Detected on Pod N
    |
    Q1: KB lookup (exact hash → stable hash)
    |   Permanent fix (≥ 0.9) → APPLY → DONE
    |   Workaround → APPLY → Q4 background
    |   Miss → Q2
    |
    Q2: Fleet experiment open?
    |   Yes → WAIT 120s → recheck KB
    |   No → Q3
    |
    Q3: Invoke MMA? (training=always, production=novel/billing)
    |   No → deterministic only → DONE
    |   Yes ↓
    |
    ┌─────────────────────────────────────────┐
    │  UNIFIED MMA PROTOCOL (4 STEPS)         │
    │                                         │
    │  Step 1: DIAGNOSE (5×N models, min 2)   │
    │    → consensus on ALL problems          │
    │                                         │
    │  Step 2: PLAN (5×N models, min 2)       │
    │    → consensus on fix plans             │
    │                                         │
    │  Step 3: EXECUTE (5×N models, min 2)    │
    │    → consensus + apply best solution    │
    │                                         │
    │  Step 4: VERIFY (deterministic + 1 AI)  │
    │    ├─ ALL PASS → KB store + gossip      │
    │    └─ ANY FAIL → backtrack to Step 1    │
    │       (max 3 backtracks → human)        │
    └─────────────────────────────────────────┘
    |
    Q4: Workaround recurred 3+? → background permanent fix search
```

---

## MMA Research Provenance

This spec was designed using the Unified MMA Protocol methodology itself:

**Iteration 1 (5 models):** DeepSeek R1 0528, DeepSeek V3.2, MiMo v2 Flash, Grok 4.1 Fast, Kimi K2.5
**Iteration 2 (5 different models):** Gemini 2.5 Pro, GPT-5.4 Nano, Qwen3 235B, Nemotron 3 Super, Mistral Medium 3.1

**Consensus results:**
- 12 original gaps → all resolved with 8/10+ agreement
- 3 new gaps discovered and integrated (assumptions, per-model confidence, cost tiering)
- Total research cost: ~$0.50-$1.00

---

*Spec created: 2026-03-31 by Bono + Uday*
*MMA research: 10 models, 2 iterations, consensus-driven*
*Training period: 2026-03-31 to 2026-04-29 (30 days)*
