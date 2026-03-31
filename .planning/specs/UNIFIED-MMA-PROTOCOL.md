# Unified MMA Protocol v3.0

**Author:** Bono + Uday + James (MMA-researched: 10 models, 2 iterations + 26-gap meta-audit)
**Date:** 2026-03-31
**Version:** v3.0 (merges MMA-First Protocol infrastructure + v2.0 reasoning engine into single complete spec)
**Status:** SPEC — approved by Uday
**Affects:** v31.0 Phases 268-272, tier_engine.rs, openrouter.rs, knowledge_base.rs, mma_engine.rs, budget_tracker.rs
**Supersedes:** MMA-FIRST-PROTOCOL.md (fully absorbed), Unified MMA Protocol v1.0, v2.0

---

## Summary

The Unified MMA Protocol v3.0 is a complete autonomous diagnostic system for the RacingPoint venue fleet. It combines:

- **Infrastructure layer** (from MMA-First Protocol): Q1-Q4 decision gate, context bundles, KB schema, fix type routing, fleet learning, permanent fix pipeline
- **Reasoning engine** (from Unified MMA Protocol v2.0): 4-step convergence (DIAGNOSE→PLAN→EXECUTE→VERIFY), multi-model consensus, domain rosters, backtracking
- **Hardening** (from v2.0 Amendments 1-3): config safety gates, 26-gap meta-audit fixes (MMA-01 to MMA-21), cloud↔venue sync

When an issue is detected, the system asks 4 questions (Q1-Q4). If Q3 authorizes an MMA call, the 4-step convergence engine runs. Results are stored in KB, propagated to fleet, and upgraded from workarounds to permanent fixes over time.

**Core Principle:** Never re-diagnose what's already been solved. Never accept a workaround as a final answer.

---

## Core Principles

1. **Never re-diagnose what's already solved** (Q1 KB gate handles this)
2. **3/5 majority = consensus** — minority opinions preserved for backtracking
3. **Each step has its own 10-model pool** — shuffled per iteration, stratified by role
4. **Always Cold Path** — thoroughness over speed, every time
5. **Deterministic verification cannot be bypassed** — AI consensus is not proof
6. **Any Step 4 failure triggers backtrack** — partial retry first, then full
7. **Every issue gets a permanent root cause fix** — workarounds are temporary

---

## Part 1: Q1-Q4 Decision Gate (Infrastructure)

Every time an issue is detected, the system asks 4 questions in order:

### Q1: "Has this EXACT problem been solved before?"

- KB lookup by `problem_hash` (exact match first, then stable hash without build_id)
- If **permanent fix** found with confidence >= 0.9 → APPLY → DONE
- If **workaround** found → APPLY immediately (customer unblocked) → falls to Q4
- If no match → Q2

### Q2: "Is someone ALREADY diagnosing this?"

- Check `experiments` table for open experiment with same `problem_key`
- Check mesh gossip for `MeshExperimentBroadcast` from another pod
- If YES → WAIT (max 120s) → check KB again
- If NO → Q3

### Q3: "Is this novel and worth an MMA call?"

**Training mode (Day 1-30):** Always YES for any unresolved issue. MMA is Tier 1.

**Production mode (Day 31+):**
- Billing active on this pod? → YES (revenue justifies it)
- First time seeing this `problem_key`? → YES
- Same issue 3+ times in 24h with no KB solution? → YES
- All NO → deterministic fix only, log, move on

### Q4: "Can MMA find a PERMANENT solution so this never recurs?"

- Triggered AFTER Q1 applies a workaround — runs in **background** (customer already unblocked)
- Fires when ALL of:
  - KB solution was just applied (Q1 hit)
  - Solution `fix_permanence == "workaround"` (not permanent)
  - `recurrence_count >= 3` for this `problem_hash`
  - No permanent fix attempt in the last 7 days
  - Budget allows it
- MMA prompt asks: "This workaround has been applied N times. WHY does this keep happening? Find the ROOT CAUSE and a PERMANENT fix."
- Result replaces the workaround in KB; workaround demoted to "fallback"

### Training Mode Config

```toml
# racecontrol.toml
[mma]
training_mode = true
training_start = "2026-03-31"
training_end = "2026-04-29"
daily_budget_pod = 15.0       # Raised from $10 during training
daily_budget_server = 25.0    # Raised from $20 during training
daily_budget_pos = 8.0        # Raised from $5 during training
```

After `training_end`, system auto-flips `training_mode = false`. No manual intervention needed.

### Training Mode Special Behavior

During training mode, after Q2 (no fleet experiment):
- **Instant safe fix available?** (kill WerFault, clear stale sentinel) → DO IT (free, <100ms) → **still fall through to MMA**
- This ensures the KB gets a root cause diagnosis even for issues with known deterministic workarounds

### Decision Tree (Complete)

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
    │  4-STEP CONVERGENCE ENGINE (Part 2)     │
    │  Step 1: DIAGNOSE → Step 2: PLAN →      │
    │  Step 3: EXECUTE → Step 4: VERIFY       │
    │  (see Part 2 below)                     │
    └─────────────────────────────────────────┘
    |
    Q4: Workaround recurred 3+ times?
        MMA finds root cause → replace workaround in KB
        CodeChange? → send to James
        Hardware? → alert staff
        Can't find root cause? → retry in 7 days
```

---

## Part 2: Context Bundles (Infrastructure)

Before every MMA call, collect a context bundle specific to the issue type. This replaces generic trigger dumps.

| Issue Type | Context Collected |
|-----------|------------------|
| Game crash mid-session | Exit code, crash dump path, last 20 Event Viewer entries, GPU temp, track/car/game, session duration, last telemetry packet, process tree at death |
| Game launch fail | Game Doctor 12-point result, race.ini content, installed games list, disk space, process list, CM state |
| Display/blanking | Edge process count, screen resolution, NVIDIA Surround state, lock screen state, taskbar visibility |
| WS disconnect | Server reachability (ping + HTTP), network adapter state, last successful msg timestamp, reconnect count |
| Process crash | WerFault dump path, process name, parent PID, crash frequency (last hour), memory pressure, handle count |
| Health check fail | Last health response, port binding (netstat), CPU/memory, disk I/O |
| Thermal/hardware | All predictive_maintenance.rs metrics, GPU temp history, fan RPM, disk SMART, power plan |

### MMA-19: Domain-Specific Prompting

All MMA prompts include a domain context header:
```
DOMAIN: Sim racing venue management (8 gaming PCs, Rust/Axum server, Windows pods,
Conspit wheelbases, AC/F1 25/LMU/Forza/iRacing, USB HID billing, Edge kiosk)
KNOWN FAILURE PATTERNS: [top 5 from KB by frequency]
```

---

## Part 3: 4-Step Convergence Engine (Reasoning)

When Q3 authorizes an MMA call, the system runs 4 sequential steps. Each step uses 5 models per iteration, minimum 2 iterations, until 3/5 majority consensus forms.

```
Step 1: DIAGNOSE ──→ consensus on ALL problems
    (5 models × N iterations, min 2)
Step 2: PLAN ──→ consensus on fix plans
    (5 models × N iterations, min 2)
Step 3: EXECUTE ──→ consensus on best solution, applied
    (5 models × N iterations, min 2)
Step 4: VERIFY ──→ deterministic checks + 3-model adversarial
    ANY failure → partial retry (Steps 3-4), then full backtrack to Step 1
    ALL pass → store in KB, gossip to fleet
Max 3 full backtracks → multi-channel escalation → SAFE_MODE
```

### Universal MMA System Prompt (all steps)

All 5 models in every step receive this system prompt:

```
CRITICAL INSTRUCTION: You must provide:
1. ROOT CAUSE — not symptoms, not "restart fixed it", the ACTUAL cause
2. PERMANENT FIX — a fix that prevents this issue from recurring
3. VERIFICATION — how to confirm the fix worked
4. PREVENTION — what should change so this never happens again

DO NOT suggest "restart the service" as a root cause.
"Restart" is a WORKAROUND, not a solution.
If restarting fixes it, explain WHY restarting fixes it
(stale state? memory leak? file lock? corrupted cache?)
and what should be done to prevent the stale state.
```

### Step 1: DIAGNOSE

**Goal:** Identify ALL problems with evidence and confidence scores.

**Iteration Flow:**
1. Select 5 models from Step 1's 10-model pool (stratified shuffle)
2. Send diagnostic prompt with issue context + context bundle
3. Collect 5 responses, extract findings
4. Build consensus: findings with 3/5+ agreement = confirmed
5. Iteration 2: shuffle pool, send confirmed findings + "what did we miss?"
6. Continue until convergence (iteration N adds <2 new findings vs N-1)

**Prompt Template:**
```
CONTEXT:
[Fleet context + issue trigger + context bundle + pod state]

TASK — STEP 1: DIAGNOSE
You are diagnosing a live issue on a Racing Point sim racing pod fleet.
Show your reasoning step by step.
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

**Convergence Criteria:**
- Converged: Iteration N produces <2 semantically new findings vs N-1
- Semantic dedup: Compare by problem category + affected component
- Max iterations: 4 per step, Minimum: 2 iterations

**Model Pool (10 models, biased toward reasoners):**

| Slot | Role | Models |
|------|------|--------|
| 1 | Reasoner (required) | DeepSeek R1 0528, GPT-5.4 Nano, Kimi K2.5 |
| 2 | Code Expert (required) | DeepSeek V3.2, Grok Code Fast, Qwen3 Coder |
| 3 | SRE/Ops (required) | MiMo v2 Pro, Nemotron 3 Super, MiMo v2 Flash |
| 4 | Domain Specialist | Varies by issue domain (see Domain Roster) |
| 5 | Generalist/Wildcard | Qwen3 235B, Gemini 2.5 Flash, Mistral Medium |
| 6-10 | Pool reserves | Filled from domain roster + remaining models |

**MMA-05: Vendor Diversity:** Each 5-model iteration MUST include ≥1 reasoner + ≥1 code expert + ≥1 SRE. Max 2 models per vendor family. Min 3 vendor families per step.

### Step 2: PLAN

**Goal:** Design fix plans for every confirmed problem from Step 1.

**Prompt Template:**
```
CONFIRMED PROBLEMS (from Step 1 consensus):
[majority_findings array]

DISSENTING OPINIONS (minority views — consider if majority is wrong):
[dissenting_opinions array]

TASK — STEP 2: PLAN
For EACH confirmed problem, design a fix plan.

For EACH plan, provide:
1. problem_id: Which problem this fixes
2. actions: Ordered list of specific steps
3. fix_type: "deterministic" | "config" | "code_change" | "hardware"
4. risk_analysis: What could go wrong?
5. rollback_strategy: How to undo this fix
6. verification_steps: How to confirm it worked
7. side_effects: What else might change?

For fix_type "code_change" or "hardware": mark requires_human = true.
Output ONLY valid JSON array of plans.
```

**Model Pool:** Biased toward architects (Gemini 2.5 Pro, GPT-5.4 Nano, Mistral Large).

### Step 3: EXECUTE

**Goal:** Select and apply the best solution from consensus plans.

**Prompt Template:**
```
CONTEXT:
[Fleet context + Step 1 consensus + Step 2 consensus]

FIX PLANS (from Step 2 consensus):
[majority plans array]

TASK — STEP 3: EXECUTE
Review these fix plans and select the BEST solution for each problem.
Show your reasoning step by step.

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

**Model Pool (biased toward coders — cost optimization):**

| Slot | Role | Models |
|------|------|--------|
| 1 | Code Expert (required) | Grok Code Fast, DeepSeek V3.2, Qwen3 Coder |
| 2 | Code Expert 2 (required) | Mercury Coder, GPT-5.1 Codex Mini |
| 3 | SRE/Ops (required) | MiMo v2 Flash, Nemotron 3 Super |
| 4-5 | Fast/Cheap models | Qwen3 235B 2507, Gemini 2.5 Flash |
| 6-10 | Pool reserves | Remaining models |

**Cost optimization (Gemini Iter 2 insight):** Step 3 prioritizes speed + code quality over deep reasoning. Use cheaper/faster models here. Save expensive reasoning models (R1, Gemini Pro, GPT-5.4) for Steps 1 and 4.

**MMA-16: Step Timeouts:** 60s per model call, 5min per step. Model timeout → skip, proceed with 4. Step timeout → backtrack.

### Step 4: VERIFY

**Goal:** Deterministic proof that the fix actually worked. AI consensus is NOT proof.

**Verification Flow:**
1. **Deterministic checks first** (Ralph Wiggum P6 — cannot lie):
   - Process alive? Port open? Health endpoint correct?
   - Edge process count > 0? (if blanking issue)
   - Original symptom reproduced?
   - Custom checks from Step 2 verification_steps[]
   - **MMA-08: Semantic config validation** — URLs resolve, values reasonable, API keys valid

2. **3-model diverse adversarial verification** (MMA-07, upgraded from single model):
   - 3 models from different vendor families, none used in Steps 1-3
   - 2/3 majority = PASS. All 3 FAIL = FAIL. Mixed = FLAG.
   - 60s timeout per model (MMA-16). Parallel execution.
   - Grade on 4-criterion rubric:
     - Root Cause Accuracy (35%): Did we fix the actual cause?
     - Fix Completeness (25%): Does it handle all variants?
     - Verification Evidence (25%): Is there concrete proof?
     - Side Effect Safety (15%): Could it break anything else?
   - Score ≥ 4.0 → PASS. Score 3.0-3.9 → FLAG. Score < 3.0 → FAIL.

3. **Result routing:**
   - ALL pass → store permanent fix in KB (stable hash), gossip to fleet
   - ANY failure → partial retry Steps 3-4 first (MMA-05), then full backtrack to Step 1

### Backtracking Rules

- **Partial backtrack first (MMA-05):** Step 4 failure retries Steps 3-4 once with different models before full restart. Saves ~60% cost.
- **Full backtrack:** Returns to Step 1 with failure evidence appended.
- Each backtrack uses DIFFERENT models (fresh perspective — P2)
- **Max 3 full backtracks** → multi-channel escalation (MMA-03)
- **MMA-13: Evidence Schema** for backtrack data: original error, model responses, confidence, timestamps, cumulative cost.

---

## Part 4: KB Schema + Two-Tier Hash (Infrastructure)

### Schema Changes

```sql
ALTER TABLE solutions ADD COLUMN fix_permanence TEXT DEFAULT 'workaround';
-- Values: 'workaround', 'permanent', 'pending_permanent', 'fallback'

ALTER TABLE solutions ADD COLUMN recurrence_count INTEGER DEFAULT 0;
-- Incremented every time Q1 applies this solution

ALTER TABLE solutions ADD COLUMN permanent_fix_id TEXT;
-- Links a workaround to its permanent replacement

ALTER TABLE solutions ADD COLUMN last_recurrence TEXT;
-- ISO 8601 timestamp of last Q1 application

ALTER TABLE solutions ADD COLUMN permanent_attempt_at TEXT;
-- When Q4 was last invoked (prevents re-invoke within 7-day cooldown)
```

### Two-Tier Hash Lookup

```rust
// Exact hash (version-specific): problem_key + build_id + hardware_class
fn compute_exact_hash(key: &str, env: &EnvironmentFingerprint) -> String;

// Stable hash (cross-version): problem_key + hardware_class only
fn compute_stable_hash(key: &str, env: &EnvironmentFingerprint) -> String;

// Lookup order: exact first, then stable
fn lookup_two_tier(exact_hash: &str, stable_hash: &str) -> Option<Solution>;
```

Most solutions are version-independent. Only binary-specific bugs need exact hash.

---

## Part 5: FixType Routing (Infrastructure)

### Structured Diagnosis Output

```rust
struct MmaDiagnosis {
    root_cause: String,
    immediate_fix: String,
    permanent_fix: String,
    fix_type: FixType,
    verification: String,
    prevention: Option<String>,
    confidence: f64,
    requires_human: bool,
}

enum FixType {
    Deterministic,  // Auto-apply: kill process, clear file, change setting
    Config,         // Auto-apply: config change in toml/ini/registry
    CodeChange,     // NEVER auto-apply -- send to James
    Hardware,       // Physical intervention -- alert staff via WhatsApp
}
```

### Fix Type Routing

| fix_type | Action | Auto-apply? |
|----------|--------|-------------|
| Deterministic | Execute fix, verify, store in KB, gossip | YES |
| Config | Apply config change, verify, store, gossip | YES |
| CodeChange | Store diagnosis, send to James via comms-link | NO — human |
| Hardware | Alert staff via WhatsApp, store in KB | NO — physical |

---

## Part 6: Fleet Learning (Infrastructure)

### Fleet Dedup (Q2)

Before calling OpenRouter, broadcast to fleet:
```
MeshExperimentBroadcast {
    problem_key: "game_crash_acs.exe",
    hypothesis: "diagnosing",
    node: "pod_3",
    estimated_cost: 4.30
}
```
Any other pod with the same `problem_key` waits instead of launching its own diagnosis. One pod pays, all pods learn.

### Solution Gossip

After Step 4 VERIFY passes, broadcast the solution to all pods:
```
MeshSolutionAnnounce {
    problem_key: "game_crash_acs.exe",
    fix_action: "clear stale lock file",
    fix_permanence: "permanent",
    confidence: 0.85,
    source_pod: "pod_3",
    verified: true
}
```

### Solution Propagation Confidence

| Source | KB Confidence | Auto-apply? |
|--------|--------------|-------------|
| Single pod, single model (Qwen3) | 0.60 | No |
| Single pod, 5-model consensus | 0.85 | Yes (if same env fingerprint) |
| 2+ pods, same fix worked | 0.95 | Yes (fleet-verified) |
| 5+ pods, zero failures | 0.99 | Yes (hardened knowledge) |

### Model Reputation Scoring (MMA-09)

Track model accuracy across MMA runs. Models with <30% accuracy after 5+ runs get WARNING logged and consideration for roster removal. Correct majority models recorded on Step 4 PASS. Monthly pool rotation (MMA-15) swaps 2 secondary models based on performance.

---

## Part 7: Q4 Permanent Fix Pipeline (Infrastructure)

```
Q4 prompt to MMA:

CONTEXT:
- Problem: {problem_key} on {pod_id}
- Workaround applied {recurrence_count} times: "{fix_action}"
- Workaround works every time (confidence: {confidence})
- First seen: {created_at}
- Recurrence rate: {times_per_day}/day across {affected_pods} pods
- Full context bundle from latest occurrence

TASK:
This issue keeps recurring despite the workaround working.
1. WHY does "{fix_action}" fix it? What state was corrupted?
2. WHAT causes that corruption in the first place?
3. HOW to prevent the corruption from occurring?
4. Provide a PERMANENT FIX that eliminates recurrence.
```

---

## Part 8: Domain Rosters

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

### Node.js/Next.js Frontend
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

### Security
| Priority | Model | Why |
|----------|-------|-----|
| Primary | Gemini 2.5 Pro | Proven credential scanner |
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

## Part 9: Consensus Schema

### StepConsensus (passed between steps)

```json
{
  "protocol_version": "3.0",
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
      "evidence": [...],
      "assumptions": [...],
      "verification_steps": [...],
      "models_agreed": ["R1", "V3.2", "Grok", "Kimi"],
      "agreement_score": 0.8
    }
  ],
  "dissenting_opinions": [...],
  "models_used": [...],
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
| 2/5 or less | No majority | NOT consensus — run another iteration |

**MMA-07: Minority Opinion Review:** If the same minority opinion appears in 3+ consecutive runs, promote it to Step 1 context for investigation.

---

## Part 10: Cost Model

### Per-Incident Cost

| Scenario | Steps | Est. Cost |
|----------|-------|-----------|
| Quick resolve (2 iter, no backtrack) | 4 | $0.80-$2.00 |
| Standard (3 iter, no backtrack) | 4 | $1.20-$3.00 |
| Complex (4 iter, 1 backtrack) | 8 | $2.40-$6.00 |
| Worst case (4 iter, 3 backtracks) | 16 | $6.40-$16.00 |

### Training Period Budget (30 days)

| Day | KB hit rate | MMA calls | Daily cost |
|-----|-------------|-----------|------------|
| Day 1 | 0% | 20 | $16-$40 |
| Day 7 | 60% | 8 | $6-$16 |
| Day 14 | 80% | 4 | $3-$8 |
| Day 30 | 95% | 1 | $0.80-$2 |
| **30-day total** | | | **$150-$400** |

**Note:** MMA-First Protocol estimated $800-$1,200 for 30 days. The 4-step convergence engine is 3-5x cheaper because: (1) actual per-model costs are $0.003-$0.05 vs estimated $0.86, (2) Step 3 uses cheap/fast models, (3) Step 4 is mostly deterministic ($0). The $150-$400 range is the validated estimate.

### Per-Model Cost Table

| Model | Cost/call (est.) | Category |
|-------|-----------------|----------|
| Qwen3 235B 2507 | $0.003 | Budget |
| Nemotron 3 Super | $0.005 | Budget |
| MiMo v2 Flash | $0.004 | Budget |
| Grok 4.1 Fast | $0.006 | Budget |
| Mistral Medium 3.1 | $0.008 | Budget |
| Llama 4 Maverick | $0.008 | Budget |
| DeepSeek V3.2 | $0.008 | Mid |
| GPT-5.4 Nano | $0.012 | Mid |
| Mercury Coder | $0.010 | Mid |
| Grok Code Fast | $0.015 | Mid |
| ERNIE 4.5 | $0.015 | Mid |
| GPT-5 Mini | $0.020 | Mid |
| GLM 4.7 | $0.020 | Premium |
| Mistral Large 2512 | $0.025 | Mid |
| DeepSeek R1 0528 | $0.028 | Premium |
| Kimi K2.5 | $0.030 | Premium |
| MiMo v2 Pro | $0.050 | Premium |
| Gemini 2.5 Pro | $0.110 | Expensive |

**Manual mode (MMA-10):** $5/session cap unless Uday approves.

### Production Mode Budget (Day 31+)

| | Daily | Monthly |
|--|-------|---------|
| Per pod | $1-$3 | $30-$90 |
| Fleet (8 pods) | $8-$24 | $240-$720 |
| Server + POS | $2-$5 | $60-$150 |
| **Total** | **$10-$29** | **$300-$870** |

### ROI

One peak-hour customer waiting 15 min for a broken pod = Rs.225 lost revenue + reputation damage. The entire 30-day training budget ($150-$400) is recovered if it prevents ~400 minutes of cumulative downtime across all pods (~50 minutes per pod over 30 days).

---

## Part 11: Operational Rules (Amendments 1-3)

### MMA-01: Bootstrap Independence
MMA config reads from env vars first (`OPENROUTER_KEY`, `MMA_DAILY_BUDGET`), then `mma.toml`, then hardcoded defaults. Never depends on `racecontrol.toml`.

### MMA-02: Dual Execution Mode
- **Automated** (primary): `mma_engine.rs` on rc-agent pods.
- **Structured-Manual** (break-glass): Bono/Claude calling OpenRouter. Must log every call, enforce 3/5 consensus, track cost.

### MMA-06: Pre-Flight Infrastructure Probing

Before ANY MMA execution (automated or manual), run pre-flight probes:
1. **OpenRouter API reachable?** `curl -s -o /dev/null -w "%{http_code}" https://openrouter.ai/api/v1/models -H "Authorization: Bearer $key"` → expect 200
2. **Comms-link WebSocket alive?** `ws://localhost:8765` ping → expect pong within 5s
3. **RaceControl health?** `curl -s localhost:8080/api/v1/health` → expect `{"status":"ok"}`

If any probe fails: log the failure, proceed with degraded mode (skip unavailable channels). Never assume infrastructure is healthy. If OpenRouter fails pre-flight, trigger MMA-14 fallback immediately (don't waste the first model call discovering it).

### MMA-11: Daily Self-Health-Check

MMA engine runs a synthetic self-test daily (cron or startup):
- Send a known-answer diagnostic to 1 cheap model: `"What is 2+2? Answer as JSON: {answer: N}"`
- Verify: response parses as JSON AND answer == 4
- If fails: log ERROR, flag MMA as degraded, fall back to deterministic-only
- Cost: ~$0.001/day

Catches: API key expiry, OpenRouter outages, model deprecation, response format changes — all before a real incident needs MMA and fails silently.

### MMA-03: Multi-Channel Escalation
After max backtracks: WhatsApp + email + comms-link. If all fail after 5min → SAFE_MODE sentinel (no automated fixes).

### Config Safety Gates (Amendment 1)
- **GAP-1:** Server panics if config file exists but fails to parse. No silent Config::default().
- **GAP-2:** Test validates all TOML sections match Config struct fields.
- **GAP-3:** Guardian `deny_unknown_fields` + fail-fast on corrupt config.

### MMA-05: Vendor Diversity
Max 2 models per vendor family per step. Min 3 different vendors. Families: DeepSeek, Meta, Google, Moonshot, Mistral, Qwen, xAI, Nvidia, OpenAI.

### MMA-12: Chain-of-Thought Mandate
All prompts include "Show your reasoning step by step." Responses without reasoning weighted 0.5x.

### MMA-14: Multi-Provider Fallback
If OpenRouter returns 5xx for 3 consecutive calls: try Anthropic → Google → local Ollama. Reduce to 3-model consensus in degraded mode.

### MMA-17: Input Sanitization
Strip ANSI codes, truncate 2000 chars, remove `sk-`, `Bearer`, `password=`, redact `/root/` paths before inserting into prompts.

### MMA-18: Model Provenance
Log model ID, step number, prompt hash, finish_reason, token count, latency_ms, cost for every call.

### MMA-20: Cascade Update Rule
Any protocol change must cascade to ALL consumers. See cascade checklist in implementation section.

### MMA-21: Cloud ↔ Venue Sync
Both environments follow the same protocol. CLAUDE.md standing rules (cloud) must match `mma_engine.rs` constants (venue). See sync table below.

| Component | Cloud (CLAUDE.md) | Venue (mma_engine.rs) |
|-----------|-------------------|----------------------|
| Consensus threshold | "3/5 majority" | CONSENSUS_RATIO |
| Max backtracks | "max 3" | MAX_BACKTRACKS = 3 |
| Step timeout | "60s/5min" | timeout(60s) |
| Vendor diversity | "≥3 vendors" | select_adversarial_models() |
| Budget cap | "$5/session" | budget_tracker daily limits |

---

## Part 12: New Triggers

| Trigger | When | Context |
|---------|------|---------|
| GameMidSessionCrash | Game exits with non-zero during billing | Exit code, crash dump, telemetry at death |
| PostSessionAnalysis | After billing session ends | Session quality metrics, lightweight analysis |
| PreShiftAudit | Morning health check | All pods, full MMA |
| DeployVerification | After binary deploy | Post-deploy MMA validation |

---

## Part 13: Implementation Checklist

### Detailed Task Checklist

**tier_engine.rs:**
- [ ] New `should_invoke_mma()` gate implementing Q1-Q4 protocol
- [ ] Training mode: skip deterministic-only tier, go straight to MMA after KB miss
- [ ] Q4 background task: after Q1 workaround, spawn async permanent fix search
- [ ] Q4 trigger: recurrence_count >= 3 AND fix_permanence == "workaround"

**knowledge_base.rs:**
- [ ] Add columns: fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
- [ ] Two-tier hash: compute_exact_hash + compute_stable_hash
- [ ] lookup_two_tier(): exact first, then stable fallback
- [ ] Increment recurrence_count on every Q1 hit
- [ ] Link workaround to permanent fix via permanent_fix_id

**diagnostic_engine.rs:**
- [ ] Per-trigger-type context collectors (see Context Bundles table)
- [ ] Context bundle passed to MMA instead of generic trigger dump
- [ ] GameMidSessionCrash trigger (exit code, crash dump, telemetry at death)
- [ ] PostSessionAnalysis trigger (session quality metrics)
- [ ] PreShiftAudit trigger (morning health check, all pods)
- [ ] DeployVerification trigger (post-deploy MMA validation)

**budget_tracker.rs:**
- [ ] Read training mode config for budget limits ($15/$25/$8)
- [ ] Higher limits during training period, auto-revert after training_end

**mma_engine.rs (DONE):**
- [x] 4-step convergence engine with partial backtracking
- [x] 3-model diverse Step 4 verification
- [x] Model reputation scoring
- [x] Multi-channel escalation + SAFE_MODE
- [x] Persistent state machine (JSON checkpoints)

### Key Files

| File | Changes |
|------|---------|
| `tier_engine.rs` | Q1-Q4 gate, training mode, should_invoke_mma() |
| `mma_engine.rs` | 4-step convergence engine (already implemented: partial backtrack, 3-model verify, reputation, escalation, state persistence) |
| `openrouter.rs` | Domain roster, model selection, consensus builder |
| `knowledge_base.rs` | KB schema, two-tier hash, fix_permanence, recurrence tracking |
| `budget_tracker.rs` | Training mode limits, per-step cost tracking |
| `diagnostic_engine.rs` | Context bundles, new triggers |
| `config.rs` | MmaConfig with fail-fast parse |

### Cascade Checklist (MMA-20)

| Consumer | Owner | Status |
|----------|-------|--------|
| Bono CLAUDE.md | Bono | Standing rules synced |
| James CLAUDE.md | Bono (via sync-rules.sh) | Pending |
| mma_engine.rs | James (venue deploy) | Code pulled, needs rebuild |
| openrouter.rs | James | Needs roster update |
| rc-doctor.sh | Bono | Needs vendor diversity |
| Memory | Bono | Updated |

---

## Provenance

**v1.0 (2026-03-31):** 4-step convergence engine. 10 models, 2 iterations, consensus-driven.
**v2.0 (2026-03-31):** Added 26-gap meta-audit fixes (MMA-01 to MMA-21), config safety gates, Wave 2 engine upgrades.
**v3.0 (2026-03-31):** Merged MMA-First Protocol infrastructure (Q1-Q4, context bundles, KB schema, FixType routing, fleet learning, Q4 pipeline) into single complete spec. MMA-FIRST-PROTOCOL.md fully absorbed.

*Total MMA model calls across all versions: ~37 calls (~$1.00-$1.50)*
*Training period: 2026-03-31 to 2026-04-29 (30 days)*
