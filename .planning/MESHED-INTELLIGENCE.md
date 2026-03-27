# Meshed Intelligence: Self-Healing AI Fleet for Racing Point

**Status:** Design Phase
**Author:** James Vowles + Claude Opus 4.6
**Date:** 2026-03-27
**Goal:** Every node (8 pods + server) autonomously diagnoses and heals itself using the Unified Protocol + OpenRouter models. Solutions propagate across the mesh so no issue is debugged twice.

---

## 1. The Vision

**Today:** James (AI agent on .27) manually debugs pods and server. If Pod 3 crashes, James SSHes in, investigates, fixes, then manually applies the same fix to other pods. Knowledge lives in James's context window and LOGBOOK.

**Tomorrow:** Every pod and the server run the Unified Protocol autonomously. When Pod 3 encounters an issue, it diagnoses itself using 4 OpenRouter models, applies the fix, and broadcasts the solution. Pods 1-8 and the server receive the solution and pre-emptively apply it. Nobody prompts anything. Issues are resolved before the next customer notices.

```
         [Central Knowledge Base — Server .23]
              /    |    |    |    \
          gossip  gossip gossip gossip
            /      |      |      \
      [Pod 1] [Pod 2] [Pod 3] ... [Pod 8]
         \      |       |       /
          \     |       |      /
           [Peer-to-Peer Solution Gossip]
```

---

## 2. Budget Model

| Node | Daily Budget | Monthly Budget | Models Available |
|---|---|---|---|
| Each Pod (x8) | $10/day | ~$300/mo | 4 OpenRouter models |
| Server (.23) | $20/day | ~$600/mo | 4 OpenRouter models + fleet coordination |
| **Fleet Total** | **$100/day** | **~$3,000/mo** | |

**Budget justification:** If this saves 2 hours/day of manual debugging at $50/hr equivalent, it pays for itself in 1 day. The real value is **zero-downtime customer experience** — issues fixed between sessions, not during them.

**Per-incident cost breakdown:**
| Action | Cost | Budget Impact |
|---|---|---|
| Tier 1: Deterministic check (local) | $0 | None |
| Tier 2: Knowledge base lookup (local) | $0 | None |
| Tier 3: Single model diagnosis | $0.05-0.43 | Minimal |
| Tier 4: Full 4-model diagnosis | ~$3.01 | 3 per pod per day |
| Tier 5: Fleet-wide propagation | $0 (gossip) | None |

**At $10/day per pod:** Each pod can run ~3 full 4-model diagnostics OR ~200 lightweight single-model checks per day. Far more than needed.

---

## 3. Architecture

### 3.1 — Node Architecture (per pod and server)

Each node runs the same stack:

```
┌─────────────────────────────────────────┐
│             Node (Pod or Server)         │
│                                         │
│  ┌─────────────┐  ┌──────────────────┐  │
│  │ rc-agent    │  │ Diagnostic Engine │  │
│  │ (existing)  │←→│ (NEW)            │  │
│  │ port 8090   │  │ port 8095        │  │
│  └─────────────┘  └──────────────────┘  │
│         ↕                 ↕              │
│  ┌─────────────┐  ┌──────────────────┐  │
│  │ Local KB    │  │ Budget Manager   │  │
│  │ (SQLite)    │  │ (cost tracking)  │  │
│  └─────────────┘  └──────────────────┘  │
│         ↕                                │
│  ┌──────────────────────────────────┐   │
│  │ Mesh Gossip Layer               │   │
│  │ (solution sync, peer discovery) │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

### 3.2 — The Diagnostic Engine

**Runs locally on each node.** Watches for anomalies and self-heals.

**Detection triggers:**
- rc-agent health check fails
- Process crash detected (WerFault, exit code != 0)
- Game launch failure
- Display anomaly (edge_process_count mismatch)
- Performance degradation (FPS drop, latency spike)
- Error rate exceeds threshold (>5 errors/min)
- WebSocket disconnection (>30s)
- Sentinel file appears unexpectedly
- Process guard violation spike
- Scheduled periodic scan (every 5 minutes)

**Diagnosis flow (Unified Protocol Phase D, automated):**

```
Anomaly detected
    │
    ▼
Tier 1: Deterministic (local, $0)
    │── MAINTENANCE_MODE? → clear it
    │── Orphan processes? → kill them
    │── Stale sentinel? → remove it
    │── Known crash pattern? → restart service
    │
    ├── FIXED? → log solution → gossip to mesh → done
    │
    ▼
Tier 2: Knowledge Base (local, $0)
    │── Match in local KB? → apply known fix
    │── Match in fleet KB? → apply known fix
    │
    ├── FIXED? → log solution → update success stats → done
    │
    ▼
Tier 3: Single Model Diagnosis ($0.05-0.43)
    │── Cheapest model (Qwen3) analyzes symptoms + code
    │── Hypothesis generated → test → verify
    │
    ├── FIXED? → log solution → gossip to mesh → done
    │
    ▼
Tier 4: Full 4-Model Parallel Diagnosis ($3.01)
    │── All 4 models analyze in parallel
    │── Cross-reference: consensus → test → verify
    │
    ├── FIXED? → log solution → gossip to mesh → done
    │
    ▼
Tier 5: Escalate to Human
    │── WhatsApp Uday
    │── James/Bono manual investigation
```

### 3.3 — The 4 OpenRouter Models (per node)

Same stack as Unified Protocol D.10, but embedded in each node:

| Role | Model | OpenRouter ID | Strength | Cost/call |
|---|---|---|---|---|
| **Scanner** | Qwen3 235B | `qwen/qwen3-235b-a22b-2507` | Fast, cheap, volume screening | ~$0.05 |
| **Reasoner** | DeepSeek R1 | `deepseek/deepseek-r1-0528` | Logic bugs, absence detection | ~$0.43 |
| **SRE** | MiMo v2 Pro | `xiaomi/mimo-v2-pro` | Operational state, stuck states | ~$0.77 |
| **Security** | Gemini 2.5 Pro | `google/gemini-2.5-pro-preview-03-25` | Config errors, auth, credentials | ~$1.65 |

**Escalation:** Tier 3 uses Scanner only. Tier 4 uses all 4 in parallel.

### 3.4 — Local Knowledge Base (per node)

SQLite database on each node:

```sql
CREATE TABLE solutions (
    id TEXT PRIMARY KEY,               -- hash of problem_key
    problem_key TEXT NOT NULL,          -- normalized problem signature
    problem_hash TEXT NOT NULL,         -- hash of error + env fingerprint
    symptoms TEXT NOT NULL,             -- JSON: error message, stack trace, system state
    environment TEXT NOT NULL,          -- JSON: OS version, driver version, build_id, hardware
    root_cause TEXT NOT NULL,           -- confirmed root cause
    fix_action TEXT NOT NULL,           -- JSON: steps to apply fix
    fix_type TEXT NOT NULL,             -- 'deterministic' | 'config' | 'restart' | 'code_change' | 'manual'
    success_count INTEGER DEFAULT 1,   -- times this fix worked
    fail_count INTEGER DEFAULT 0,      -- times this fix failed
    confidence REAL DEFAULT 1.0,       -- success_count / (success_count + fail_count)
    cost_to_diagnose REAL DEFAULT 0,   -- $ spent finding this solution
    models_used TEXT,                   -- JSON: which models helped
    source_node TEXT NOT NULL,          -- which pod/server first solved this
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER DEFAULT 1,         -- increments on update
    ttl_days INTEGER DEFAULT 90,       -- auto-expire after 90 days without use
    tags TEXT                           -- JSON: ['game_launch', 'display', 'billing', etc.]
);

CREATE TABLE experiments (
    id TEXT PRIMARY KEY,
    problem_key TEXT NOT NULL,
    hypothesis TEXT NOT NULL,
    test_plan TEXT NOT NULL,
    result TEXT,                        -- 'confirmed' | 'eliminated' | 'inconclusive'
    cost REAL DEFAULT 0,
    node TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_solutions_hash ON solutions(problem_hash);
CREATE INDEX idx_solutions_key ON solutions(problem_key);
CREATE INDEX idx_experiments_key ON experiments(problem_key);
```

### 3.5 — Mesh Gossip Protocol

**How solutions propagate across the fleet:**

```
Pod 3 solves issue → broadcasts solution digest
    │
    ├──→ Server (.23) receives → adds to Fleet KB → broadcasts to all pods
    │
    ├──→ Pod 1 receives → checks if applicable → stores in local KB
    ├──→ Pod 2 receives → checks if applicable → stores in local KB
    ├──→ Pod 4-8 receive → same
    │
    └──→ If environment differs (different hardware, driver version):
         → store as "candidate" → verify locally before trusting
```

**Gossip messages (compact, over existing WebSocket):**

```json
// Solution announcement (pod → server → all)
{
  "type": "mesh:solution",
  "problem_hash": "a1b2c3d4",
  "problem_key": "game_launch:ac:ai_level_mismatch",
  "solution_version": 2,
  "confidence": 0.95,
  "fix_type": "config",
  "source_node": "pod_3",
  "environment_tags": ["win11", "rtx4070", "rc-agent:1c78dee7"],
  "cost_to_diagnose": 0.43,
  "summary": "AcLaunchParams.ai_level expects u32, kiosk sends string"
}

// Solution request (node needs full fix details)
{
  "type": "mesh:request_solution",
  "problem_hash": "a1b2c3d4",
  "requesting_node": "pod_7"
}

// Experiment announcement (avoid duplicate work)
{
  "type": "mesh:experiment",
  "problem_key": "game_launch:ac:ai_level_mismatch",
  "hypothesis": "serde type mismatch",
  "status": "testing",
  "node": "pod_3",
  "estimated_cost": 0.43
}

// Heartbeat with KB digest
{
  "type": "mesh:heartbeat",
  "node": "pod_3",
  "kb_size": 47,
  "kb_hash": "sha256:...",  // Bloom filter of problem_hashes
  "budget_remaining": 7.50,
  "last_diagnosis": "2026-03-27T18:30:00+05:30"
}
```

### 3.6 — Budget Manager (per node)

```json
{
  "node": "pod_3",
  "daily_budget": 10.00,
  "spent_today": 3.44,
  "remaining_today": 6.56,
  "reset_time": "00:00 IST",
  "monthly_budget": 300.00,
  "spent_this_month": 45.20,
  "rules": {
    "tier3_max_per_incident": 0.50,
    "tier4_max_per_incident": 5.00,
    "min_reserve": 2.00,
    "escalate_to_human_below": 1.00,
    "prefer_cached_solution_if_confidence_above": 0.80
  },
  "cost_saving_actions": [
    "Check KB before calling any model",
    "Check if another node is already diagnosing same issue",
    "Use cheapest model (Qwen3) first, escalate only if needed",
    "Skip models if KB match confidence > 80%",
    "Share experiments to prevent fleet-wide duplicate spending"
  ]
}
```

### 3.7 — Server Coordinator Role (.23)

The server is NOT a central brain. It's a **thin coordinator** that:

1. **Aggregates** — Collects solution digests from all pods
2. **Promotes** — Battle-tested solutions (>3 successes across >2 pods) get promoted to "fleet-verified" status
3. **Distributes** — Pushes fleet-verified solutions bundle to all pods (like a package update)
4. **Detects patterns** — "3 pods reporting same symptom in 5 min = systemic issue → alert Uday"
5. **Manages models** — Version pinning, fallback chains, model health checks
6. **Tracks budget** — Fleet-wide spend dashboard, per-node budget status

**Server does NOT:**
- Make diagnostic decisions for pods (each pod is autonomous)
- Block pod operations if server is down (Island Mode)
- Store raw telemetry (only solution digests and experiments)

---

## 4. Knowledge Propagation: Solve Once, Apply Everywhere

### 4.1 — Solution Lifecycle

```
1. DISCOVERED  — Pod 3 finds root cause + fix
2. LOCAL       — Stored in Pod 3's local KB (confidence: 1.0, success: 1)
3. ANNOUNCED   — Gossip message broadcast to server + peers
4. CANDIDATE   — Other pods receive, store as "candidate" (confidence: 0.5)
5. VERIFIED    — Pod 7 encounters same issue, applies candidate fix → success → confidence: 0.75
6. FLEET       — Server promotes after 3+ successes across 2+ pods → "fleet-verified"
7. HARDENED    — After 10+ successes, 0 failures → becomes Tier 1 deterministic check
8. EXPIRED     — After 90 days without use → TTL expires → archived
```

### 4.2 — Problem Signature Normalization

To match issues across pods with different hardware/drivers:

```
problem_key = normalize(
    error_type,         # "game_launch_fail" | "ws_disconnect" | "process_crash" | ...
    error_code,         # exit code, HTTP status, Windows error
    component,          # "rc-agent" | "rc-sentry" | "Edge" | "ConspitLink" | ...
    context_hash        # hash of relevant stack trace / error message (stripped of timestamps/PIDs)
)

environment_fingerprint = {
    os_version,         # "Windows 11 Pro 10.0.26200"
    gpu_driver,         # "NVIDIA 565.90"
    build_id,           # "1c78dee7"
    hardware_class,     # "ares_cpp_lite" | "apex" | ...
    pod_number          # for pod-specific quirks
}
```

**Match rules:**
- Same problem_key + same environment class → **direct apply** (confidence inherited)
- Same problem_key + different environment → **candidate** (needs local verification)
- Similar problem_key (partial match) → **suggestion** (human review or model re-analysis)

### 4.3 — Duplicate Work Prevention

Before any node spends money on diagnosis:

```
1. Check local KB → match found? → apply (cost: $0)
2. Check fleet KB → match found? → apply (cost: $0)
3. Check experiment ledger → another node already testing this? → WAIT
4. If no match and no active experiment → announce "mesh:experiment" → begin diagnosis
5. Other nodes seeing same symptom → DON'T diagnose, wait for first node's result
```

**The "first responder" rule:** Only the first node to encounter an issue spends model budget. All others wait for the result via gossip. If the first node fails, the next node picks up (round-robin by pod number).

---

## 5. Central Knowledge Hub

### 5.1 — Fleet Knowledge Base (on server .23)

Server maintains the master KB — superset of all pod KBs:

```sql
-- Same schema as local KB, plus:
CREATE TABLE fleet_solutions (
    -- ... (same as solutions table) ...
    promotion_status TEXT DEFAULT 'candidate',  -- 'candidate' | 'fleet_verified' | 'hardened'
    applied_on TEXT,           -- JSON: ["pod_1", "pod_3", "pod_7"]
    success_on TEXT,           -- JSON: ["pod_3", "pod_7"]
    fail_on TEXT,              -- JSON: []
    promotion_date TEXT,
    promoted_by TEXT            -- "auto:3_successes_2_pods" | "manual:james"
);

-- Incident log (central, never deleted)
CREATE TABLE incident_log (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    node TEXT NOT NULL,
    problem_key TEXT NOT NULL,
    severity TEXT NOT NULL,      -- 'P1' | 'P2' | 'P3'
    diagnosis_tier INTEGER,     -- 1-5
    cost REAL DEFAULT 0,
    resolution TEXT,            -- 'auto_fixed' | 'kb_match' | 'model_diagnosed' | 'escalated'
    time_to_resolve_ms INTEGER,
    customer_impact BOOLEAN DEFAULT FALSE,
    solution_id TEXT,           -- FK to fleet_solutions
    models_used TEXT
);
```

### 5.2 — Dashboard (admin panel)

New admin page at `:3201/mesh-intelligence`:

| Section | Data |
|---|---|
| **Fleet Health** | All 9 nodes, KB size, budget remaining, last diagnosis |
| **Solution Feed** | Real-time stream of solutions discovered/applied across fleet |
| **Budget Tracker** | Per-node daily/monthly spend, trending, projections |
| **Incident Timeline** | All issues detected, resolved, escalated — with MTTR |
| **Model Performance** | Which model finds what, false positive rate, cost per find |
| **Pattern Detector** | Systemic issues (3+ pods same symptom), correlations |
| **Knowledge Base Browser** | Search/filter all solutions, edit, promote, retire |

---

## 6. Fix-Deploy Anti-Regression Integration

### 6.0 — FIX-DEPLOY-PROTOCOL.md (FDP)

**The Meshed Intelligence system MUST enforce the Fix-Deploy Anti-Regression Protocol (FDP) during all automated deploys.** FDP was born from 3-round, 12-model OpenRouter audit ($5.59, 11.2M tokens) that identified 51 deploy-procedure findings and produced 16 concrete script fixes.

**FDP location:** `FIX-DEPLOY-PROTOCOL.md` at repo root.

**How Meshed Intelligence uses FDP:**

| Mesh Action | FDP Enforcement |
|-------------|-----------------|
| Node self-heals via deterministic fix | FDP Part 2 checklist: Session 1 verified, MAINTENANCE_MODE cleared, singleton processes |
| Node deploys new binary (OTA) | FDP Part 4: full 8-step deploy sequence incl. OTA_DEPLOYING sentinel, SHA256, bat sync |
| Solution gossips across fleet | FDP anti-regression: fix must be code-enforced (bat or startup), not manual-only |
| Model diagnoses issue | FDP Part 1: check against 10 known pain points before escalating to Tier 3+ |
| Fleet-wide propagation | FDP: include POS PC in all fleet operations, not just pods 1-8 |

**FDP 17-item pre-ship checklist integrated into Mesh Diagnostic Engine:**
Each node's Diagnostic Engine runs the FDP anti-regression checklist (Part 2) after applying any fix:
1. Session 1 context verified (PP-01)
2. Fix encoded in boot script, not manual (PP-02)
3. Bat file matches canonical version (PP-04)
4. MAINTENANCE_MODE cleared (PP-06)
5. Config fetch OK + allowlist non-empty (PP-07)
6. Singleton processes verified (PP-09)
7. LOGBOOK entry + gossip broadcast

**The 3-Round Audit Pattern for protocol evolution:**
When the fleet KB accumulates >50 solutions, run a 3-round multi-model audit:
- Round 1 (4 models): full scan → apply fixes
- Round 2 (4 different models): verify R1 fixes, find gaps → apply fixes
- Round 3 (4 code-specialized models): final verification → produce scorecard
Cost: ~$5-6 per cycle. Run monthly or after major incidents.

---

### 6.1 — Protocol Evolution Loop

The Unified Protocol itself evolves based on fleet intelligence:

```
Week 1: Protocol v3.0 deployed to all nodes
    │
    ├──→ Pods diagnose issues using Protocol's Phase D
    ├──→ Solutions accumulate in fleet KB
    ├──→ Patterns emerge (e.g., "ConspitLink crashes every Tuesday after Windows Update")
    │
Week 4: Fleet KB has 50+ solutions
    │
    ├──→ Server analyzes: which Phase D steps caught issues fastest?
    ├──→ Server analyzes: which models had best cost/accuracy ratio?
    ├──→ Server suggests Protocol updates:
    │       - "Add Tuesday post-update ConspitLink restart to Phase 0"
    │       - "Swap MiMo for cheaper model — same accuracy on our issue types"
    │       - "New Tier 1 deterministic check for the top 5 fleet-wide issues"
    │
Week 5: Protocol v3.1 deployed (model swap + new deterministic checks)
    │
    └──→ Cycle repeats — Protocol learns from the fleet
```

### 6.2 — Model Rotation

When the fleet KB shows diminishing returns from a model:

```
1. Fleet KB tracks: per-model unique finding rate
2. If model X finds <3 unique real issues in 30 days → CANDIDATE for swap
3. Server tests new model (from OpenRouter) on historical issues:
   - Feed it the same symptoms that model X missed
   - If new model catches them → SWAP
4. New model deployed to one pod first (canary)
5. If canary performance good after 7 days → fleet-wide rollout
6. Old model moved to fallback chain
```

---

## 7. Additional Features for Racing Point HQ

### 7.1 — Predictive Maintenance

Instead of waiting for failure, predict it:

| Signal | Prediction | Action |
|---|---|---|
| ConspitLink reconnection rate increasing | Wheelbase USB dying | Alert: "Pod 5 wheelbase may fail within 24h" |
| Edge process count trending down | Browser memory leak | Pre-emptive restart at next session gap |
| GPU temp consistently >80C | Thermal throttling imminent | Alert: "Check HVAC / clean GPU fan on Pod 3" |
| rc-agent restart count >2/day | Stability degrading | Schedule maintenance window |
| Disk space <10GB | Log rotation needed | Auto-cleanup old logs |
| Error rate spike across 3+ pods | Systemic issue incoming | Alert Uday before customers notice |

### 7.2 — Customer Experience Scoring

Each pod tracks a real-time experience score:

```
Experience Score = weighted average of:
  - Game launch success rate (30%)
  - Session completion rate (25%)
  - Display stability (no flicker/restart) (20%)
  - Hardware responsiveness (FFB, pedals) (15%)
  - Billing accuracy (10%)
```

**Score < 80%** → pod flagged for maintenance
**Score < 50%** → pod auto-removed from rotation + alert
**Fleet average** → displayed on admin dashboard

### 7.3 — Revenue Protection

Automatically detect and prevent revenue loss:

| Threat | Detection | Response |
|---|---|---|
| Billing session not started | Game running but no active billing | Auto-start billing or alert staff |
| Session ended but customer still racing | Game active after billing end | Grace period → auto-end game |
| Pod down during peak hours | Fleet health + booking calendar | Prioritize recovery of booked pods |
| Multiple pods down | >2 pods simultaneously | Emergency: auto-scale remaining pods' session capacity |
| Repeat customer issue | Same customer, same pod, same problem twice | Move customer to different pod + mark pod for deep diagnosis |

### 7.4 — Fleet Learning & Insights

Weekly automated report to Uday:

```
Racing Point Fleet Intelligence Report — Week of 2026-03-24

Issues detected:        47
Auto-resolved:          42 (89%)
Escalated to human:     5 (11%)
Customer impact:        2 (4%)
Average MTTR:           23 seconds (auto) / 4.2 minutes (escalated)

Top 3 recurring issues:
1. ConspitLink reconnection (12x) — USB Selective Suspend re-enabled by Windows Update
2. Edge memory leak (8x) — auto-restarted during session gaps
3. Game launch timeout (6x) — AC server slow to respond after idle

Budget spent:           $67.40 / $700 (9.6%)
Most valuable model:    DeepSeek R1 — found 3 absence bugs no other model caught
Least valuable model:   Gemini — all 4 findings were false positives this week

Knowledge base growth:  +12 solutions (total: 89)
Fleet-verified fixes:   +5 promoted
Hardened to Tier 1:     +2 (now deterministic, $0 to apply)

Recommendation:
- Schedule USB Selective Suspend enforcement in next Windows Update cycle
- Consider replacing Gemini with GPT-4.1 for next month (better cost/accuracy)
```

### 7.5 — Multi-Venue Readiness

When Racing Point opens a second venue:

```
Venue 1 (current)          Venue 2 (new)
    [8 pods + server]          [N pods + server]
           \                      /
            \                    /
         [Cloud Knowledge Base — Bono VPS]
            /                    \
           /                      \
    Solutions from Venue 1    Solutions from Venue 2
    propagate to Venue 2      propagate to Venue 1
```

**Day 1 at Venue 2:** The new venue starts with Venue 1's entire fleet KB — hundreds of pre-solved issues. Zero cold-start debugging. Every lesson learned at Venue 1 is immediately available.

### 7.6 — Competitive Intelligence

Track operational metrics vs industry benchmarks:

| Metric | Racing Point | Industry Avg | Status |
|---|---|---|---|
| Pod uptime | 99.2% | ~95% | Exceeding |
| MTTR (auto) | 23s | N/A (manual) | Unique advantage |
| Customer-facing incidents/week | 2 | ~10-15 | Exceeding |
| Issues auto-resolved | 89% | 0% (manual) | Unique advantage |

This becomes a **marketing asset**: "Racing Point: AI-powered, self-healing rigs."

### 7.7 — Autonomous Night Operations

Pods self-maintain during off-hours (midnight to 10am):

```
Midnight: Full fleet health check (Tier 1-2, free)
00:30:    Windows Update check → if pending, install + reboot + verify
01:00:    ConspitLink firmware check + restart
01:30:    Full 4-model diagnostic on each pod ($3/pod = $24 fleet)
02:00:    Apply any pending fleet-verified fixes
03:00:    Run full audit protocol (68-phase)
04:00:    Clear all logs > 7 days, compact databases
05:00:    Pre-flight check for morning opening
06:00:    Report to Uday: "Fleet ready. X issues found overnight, Y auto-resolved."
```

**No human needed.** Venue opens with a fully audited, self-healed fleet every morning.

---

## 8. Implementation Phases

| Phase | Milestone | Scope | Estimated Effort |
|---|---|---|---|
| **Phase 1** | Local Diagnostic Engine | rc-agent gets anomaly detection + Tier 1-3 auto-fix | 2-3 weeks |
| **Phase 2** | Local Knowledge Base | SQLite KB per node + solution logging | 1 week |
| **Phase 3** | OpenRouter Integration | 4-model diagnosis embedded in rc-agent | 1-2 weeks |
| **Phase 4** | Budget Manager | Per-node cost tracking + limits | 1 week |
| **Phase 5** | Mesh Gossip | Solution propagation via existing WebSocket | 2 weeks |
| **Phase 6** | Server Coordinator | Fleet KB, promotion, pattern detection | 2 weeks |
| **Phase 7** | Admin Dashboard | Mesh Intelligence page in admin panel | 1-2 weeks |
| **Phase 8** | Predictive Maintenance | ML-based failure prediction | 2-3 weeks |
| **Phase 9** | Customer Experience Scoring | Per-pod scoring + auto-rotation | 1 week |
| **Phase 10** | Night Operations | Autonomous overnight maintenance cycle | 1-2 weeks |
| **Phase 11** | Multi-Venue | Cloud KB sync between venues | 2-3 weeks |
| **Phase 12** | Fleet Intelligence Reports | Weekly automated reports to Uday | 1 week |

**Total estimated: 16-22 weeks (4-5 months)**

---

## 9. Technical Requirements

### Must Have (Phase 1-6)
- OpenRouter API key accessible from each pod (env var, never in code)
- SQLite on each pod + server
- WebSocket gossip messages added to existing rc-agent↔server WS
- Budget tracking module in rc-agent (Rust)
- Solution schema + propagation protocol

### Nice to Have (Phase 7-12)
- Admin dashboard page (Next.js)
- Predictive maintenance models (local, lightweight)
- Customer experience scoring
- Night operations scheduler
- Multi-venue cloud sync via Bono VPS

### Infrastructure
- OpenRouter account with sufficient credits (~$3,000/month)
- No new hardware — runs on existing pods + server
- No new network infrastructure — uses existing LAN + WebSocket

---

## 10. Risk Analysis

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| OpenRouter outage | Medium | High (no diagnosis) | Fallback to local Ollama (Tier 3) → human escalation |
| Bad fix propagated fleet-wide | Low | Critical | Canary promotion (3+ successes, 2+ pods before fleet-verified) |
| Budget exhaustion mid-day | Medium | Medium (degraded diagnosis) | $2 reserve + mechanical fallback + human escalation |
| Knowledge base poisoning (bad solution) | Low | High | Confidence scoring + auto-demotion on failure + human review |
| Model hallucination causes damage | Low | Critical | Fixes categorized: deterministic (auto-apply) vs code_change (human review) |
| Pod tries to fix itself and makes it worse | Low | High | Rollback on fix failure + MAINTENANCE_MODE if 3 failed fixes |

---

## 11. Success Metrics

| Metric | Target (Month 1) | Target (Month 6) | Target (Month 12) |
|---|---|---|---|
| Auto-resolution rate | 50% | 80% | 95% |
| Average MTTR (auto) | <2 min | <30s | <10s |
| Customer-facing incidents/week | <5 | <2 | <1 |
| Fleet KB size | 30 solutions | 150 solutions | 500+ solutions |
| Hardened to Tier 1 | 5 | 25 | 100+ |
| Human escalation rate | 50% | 20% | 5% |
| Budget utilization | 30% | 50% | 40% (efficiency improves) |
| Duplicate diagnosis rate | 20% | 5% | <1% |

---

## 12. The Endgame

After 12 months of Meshed Intelligence:

1. **The fleet debugs itself.** 95%+ of issues resolved without human intervention.
2. **Issues are solved once.** The fleet KB has 500+ solutions, each battle-tested across 8 pods.
3. **Morning opens are zero-effort.** Night operations handle all maintenance.
4. **New venues launch instantly.** Day 1 with full KB = zero cold-start problems.
5. **The protocol evolves itself.** Model rotation, new deterministic checks, and rule updates happen automatically based on fleet data.
6. **Uday gets a weekly report.** Not a task list — a confidence score that the venue is running optimally.

**Uday's goal — be with his daughter — is achieved. The venue runs itself.**
