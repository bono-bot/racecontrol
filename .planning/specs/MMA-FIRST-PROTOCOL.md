# MMA-First Protocol: OpenRouter as Tier 1

**Author:** Bono (designed with Uday)
**Date:** 2026-03-30
**Status:** SPEC — awaiting James review
**Affects:** v31.0 Phases 268-272, tier_engine.rs, knowledge_base.rs, budget_tracker.rs, openrouter.rs

---

## Summary

Redesign the diagnostic tier ordering so that OpenRouter MMA (5-model parallel diagnosis) is **Tier 1** during a 30-day training period starting today (2026-03-30). After training, the fleet KB handles most issues autonomously; MMA only fires for novel problems.

The goal: every issue gets a **permanent root cause fix**, not a band-aid. The KB becomes a self-built operations manual written from real incidents, not documentation.

---

## Core Principle

**Never re-diagnose what's already been solved. Never accept a workaround as a final answer.**

---

## The 4-Question Protocol

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

---

## Training Mode Specification

### Config

```toml
# racecontrol.toml
[mma]
training_mode = true
training_start = "2026-03-30"
training_end = "2026-04-29"
daily_budget_pod = 15.0       # Raised from $10 during training
daily_budget_server = 25.0    # Raised from $20 during training
daily_budget_pos = 8.0        # Raised from $5 during training
```

After `training_end`, system auto-flips `training_mode = false`. No manual intervention needed.

### Training Mode Tier Order

```
Issue Detected
    |
    +-- Q1: KB hit (permanent fix, confidence >= 0.9)?
    |       YES --> APPLY --> DONE (no MMA needed)
    |       KB hit (workaround)?
    |       YES --> APPLY (customer unblocked) --> Q4 in background
    |       NO  --> continue
    |
    +-- Q2: Fleet experiment in progress for same problem_key?
    |       YES --> WAIT 120s --> check KB again
    |       NO  --> continue
    |
    +-- Instant safe fix available? (kill WerFault, clear stale sentinel)
    |       YES --> DO IT (free, <100ms) -- still fall through to MMA
    |
    +-- INVOKE MMA 5-MODEL DIAGNOSIS  <-- THIS IS TIER 1
            |
            +-- Collect full context bundle (2s max)
            +-- Broadcast MeshExperimentBroadcast to fleet (dedup)
            +-- Call all 5 models in parallel
            +-- Require: root_cause + permanent_fix + verification
            +-- Apply fix if fix_type is Deterministic or Config
            +-- Verify fix worked
            +-- Store in KB with full provenance
            +-- Gossip solution to fleet via MeshSolutionAnnounce
```

### Production Mode Tier Order (Day 31+)

```
Issue Detected
    |
    +-- Q1: KB lookup (exact hash --> stable hash)
    |       Permanent fix --> APPLY --> DONE
    |       Workaround --> APPLY --> Q4 background
    |
    +-- Q2: Fleet experiment? --> WAIT
    |
    +-- Instant deterministic fix --> APPLY
    |
    +-- Q3: Is this novel? (no KB match at all)
    |       YES --> MMA 5-model diagnosis
    |       NO  --> log, move on
    |
    +-- Q4: Background permanent fix for recurring workarounds
```

---

## MMA Response Requirements

All 5 models must return structured responses. The system prompt must include:

```
CRITICAL INSTRUCTION: You must provide:
1. ROOT CAUSE -- not symptoms, not "restart fixed it", the ACTUAL cause
2. PERMANENT FIX -- a fix that prevents this issue from recurring
3. VERIFICATION -- how to confirm the fix worked
4. PREVENTION -- what should change so this never happens again

DO NOT suggest "restart the service" as a root cause.
"Restart" is a WORKAROUND, not a solution.
If restarting fixes it, explain WHY restarting fixes it
(stale state? memory leak? file lock? corrupted cache?)
and what should be done to prevent the stale state.
```

### Structured Diagnosis Output

```rust
struct MmaDiagnosis {
    root_cause: String,          // WHY it happened
    immediate_fix: String,       // What to do NOW (may be a workaround)
    permanent_fix: String,       // What prevents recurrence
    fix_type: FixType,           // Determines auto-apply vs escalate
    verification: String,        // How to confirm the fix worked
    prevention: Option<String>,  // Standing rule, config, or code change
    confidence: f64,             // Model consensus confidence
    requires_human: bool,        // true for hardware, code_change
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
| CodeChange | Store diagnosis, send to James via comms-link | NO -- human |
| Hardware | Alert staff via WhatsApp, store in KB | NO -- physical |

---

## Context Bundles

Before every MMA call, collect a context bundle specific to the issue type. This replaces the current generic `{:?}` trigger dump.

| Issue Type | Context Collected |
|-----------|------------------|
| Game crash mid-session | Exit code, crash dump path, last 20 Event Viewer entries (Application + System), GPU temp (nvidia-smi), track/car/game, session duration, last telemetry packet, process tree at death |
| Game launch fail | Game Doctor 12-point result, race.ini content, installed games list, disk space, process list, CM state |
| Display/blanking | Edge process count, screen resolution, NVIDIA Surround state, lock screen state, taskbar visibility |
| WS disconnect | Server reachability (ping + HTTP), network adapter state, last successful msg timestamp, reconnect count |
| Process crash | WerFault dump path, process name, parent PID, crash frequency (last hour), memory pressure, handle count |
| Health check fail | Last health response, port binding (netstat), CPU/memory, disk I/O |
| Thermal/hardware | All predictive_maintenance.rs metrics, GPU temp history, fan RPM, disk SMART, power plan |

---

## KB Schema Changes

New columns for the `solutions` table:

```sql
ALTER TABLE solutions ADD COLUMN fix_permanence TEXT DEFAULT 'workaround';
-- Values: 'workaround', 'permanent', 'pending_permanent', 'fallback'

ALTER TABLE solutions ADD COLUMN recurrence_count INTEGER DEFAULT 0;
-- Incremented every time Q1 applies this solution
-- Tracks "issue came back despite fix working" (not same as success_count)

ALTER TABLE solutions ADD COLUMN permanent_fix_id TEXT;
-- Links a workaround to its permanent replacement
-- When permanent fix exists, lookup returns THAT instead

ALTER TABLE solutions ADD COLUMN last_recurrence TEXT;
-- ISO 8601 timestamp of last Q1 application
-- Used to calculate recurrence rate

ALTER TABLE solutions ADD COLUMN permanent_attempt_at TEXT;
-- When Q4 was last invoked for this problem
-- Prevents re-invoking Q4 within cooldown (7 days)
```

### Two-Tier Hash Lookup

Current `compute_problem_hash` includes `build_id` in the hash. This means every deploy invalidates the KB. Change to two-tier:

```rust
// Exact hash (version-specific): problem_key + build_id + hardware_class
fn compute_exact_hash(key: &str, env: &EnvironmentFingerprint) -> String;

// Stable hash (cross-version): problem_key + hardware_class only
fn compute_stable_hash(key: &str, env: &EnvironmentFingerprint) -> String;

// Lookup order: exact first, then stable
fn lookup_two_tier(exact_hash: &str, stable_hash: &str) -> Option<Solution>;
```

Most solutions are version-independent (corrupted track, GPU thermal, orphan process). Only binary-specific bugs need the exact hash.

---

## Fleet Learning Lifecycle

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

### Solution Propagation Confidence

| Source | KB Confidence | Auto-apply? |
|--------|--------------|-------------|
| Single pod, single model (Qwen3) | 0.60 | No |
| Single pod, 5-model consensus | 0.85 | Yes (if same env fingerprint) |
| 2+ pods, same fix worked | 0.95 | Yes (fleet-verified) |
| 5+ pods, zero failures | 0.99 | Yes (hardened knowledge) |

### Q4: Workaround-to-Permanent Pipeline

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

## Budget

### Training Period (2026-03-30 to 2026-04-29)

| Estimate | Day 1 | Day 7 | Day 14 | Day 30 |
|----------|-------|-------|--------|--------|
| KB solutions | 0 | ~50 | ~80 | ~120 |
| KB hit rate | 0% | 60% | 80% | 95% |
| New issues/day | ~20 | ~8 | ~4 | ~1 |
| Daily fleet cost | ~$86 | ~$34 | ~$17 | ~$4 |

**30-day total estimate: $800-$1,200 (~Rs.67,000-100,000)**

### Production Mode (Day 31+)

| | Daily | Monthly |
|--|-------|---------|
| Per pod | $1-$3 | $30-$90 |
| Fleet (8 pods) | $8-$24 | $240-$720 |
| Server + POS | $2-$5 | $60-$150 |
| **Total** | **$10-$29** | **$300-$870** |

### ROI

One peak-hour customer waiting 15 min for a broken pod = Rs.225 lost revenue + reputation damage. The entire 30-day training budget is recovered if it prevents ~400 minutes of cumulative downtime across all pods (~50 minutes per pod over 30 days).

---

## Implementation Checklist (for James)

### Phase 268 Changes (Unified MMA Protocol)

- [ ] Upgrade MP-07: training mode = MMA as Tier 1 (not just model selection)
- [ ] Add `training_mode`, `training_start`, `training_end` to racecontrol.toml
- [ ] Auto-flip logic: if today > training_end, training_mode = false
- [ ] Raise budget caps during training ($15/$25/$8)
- [ ] MMA system prompt: require root_cause + permanent_fix + verification
- [ ] Structured MmaDiagnosis response with FixType enum
- [ ] Fix type routing: auto-apply Deterministic/Config, escalate CodeChange/Hardware

### tier_engine.rs Changes

- [ ] New `should_invoke_mma()` gate implementing Q1-Q4 protocol
- [ ] Training mode: skip deterministic-only tier, go straight to MMA after KB miss
- [ ] Q4 background task: after Q1 workaround, spawn async permanent fix search
- [ ] Q4 trigger: recurrence_count >= 3 AND fix_permanence == "workaround"

### knowledge_base.rs Changes

- [ ] Add columns: fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
- [ ] Two-tier hash: compute_exact_hash + compute_stable_hash
- [ ] lookup_two_tier(): exact first, then stable fallback
- [ ] Increment recurrence_count on every Q1 hit
- [ ] Link workaround to permanent fix via permanent_fix_id

### Context Bundles (new module or extend diagnostic_engine.rs)

- [ ] Per-trigger-type context collectors (see Context Bundles table above)
- [ ] Context bundle passed to MMA instead of generic trigger dump

### budget_tracker.rs Changes

- [ ] Read training mode config for budget limits
- [ ] Higher limits during training period

### New Triggers (diagnostic_engine.rs)

- [ ] GameMidSessionCrash (exit code, crash dump, telemetry state at death)
- [ ] PostSessionAnalysis (session quality metrics, lightweight Qwen3 call)
- [ ] PreShiftAudit (morning health check, all pods, full MMA)
- [ ] DeployVerification (post-deploy MMA validation)

---

## Decision Tree Summary

```
Issue Detected on Pod N
    |
    +-- Q1: KNOWN?
    |   +-- KB permanent fix (>= 0.9) --> APPLY --> DONE
    |   +-- KB workaround --> APPLY (instant) --> Q4 (background)
    |   +-- KB miss --> Q2
    |
    +-- Q2: IN PROGRESS?
    |   +-- Fleet experiment open --> WAIT 120s --> recheck KB
    |   +-- No --> Q3
    |
    +-- Q3: DIAGNOSE (MMA)
    |   +-- Training mode: ALWAYS full 5-model
    |   +-- Production mode: only for novel issues
    |   +-- Result --> apply/escalate --> store in KB --> gossip
    |
    +-- Q4: MAKE PERMANENT (background, async)
        +-- Workaround recurred 3+ times?
        +-- MMA finds root cause --> replace workaround in KB
        +-- CodeChange? --> send to James
        +-- Hardware? --> alert staff
        +-- Can't find root cause? --> retry in 7 days
```

---

*Spec created: 2026-03-30 by Bono + Uday*
*Training period: 2026-03-30 to 2026-04-29 (30 days)*
