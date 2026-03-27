# Multi-Model AI Audit Protocol v1.0

**Author:** James Vowles | **Date:** 2026-03-27 | **Status:** Proven (first run)
**Cost of first full audit:** $3.06 across 5 models + Opus review
**Findings:** 334 raw → 48 bugs Opus alone missed → 7 critical P1s no single model caught

---

## 1. Purpose

No single AI model — including Opus 4.6 — catches everything. Each model has different training data, biases, and blind spots. This protocol uses **model diversity** to maximize audit coverage at minimal cost.

**Proven results from first run (2026-03-27):**

| Model | Findings | Cost | Unique Strength |
|---|---|---|---|
| Gemini 2.5 Pro | 84 | $1.65 | Security checklists, credential scanning |
| DeepSeek V3 | 17 | $0.16 | Rust code patterns, Session 0 detection |
| Qwen3 235B | 139 | $0.05 | Exhaustive volume scanning |
| DeepSeek R1 | 46 | $0.43 | Reasoning about absence, state machine logic |
| MiMo v2 Pro | 48 | $0.77 | SRE/ops thinking, stuck states, idempotency |
| **Opus (reviewer)** | 10+8 | subscription | Cross-system architecture, domain knowledge |
| **TOTAL** | **334+18** | **$3.06** | |

---

## 2. Model Selection Philosophy

### The Diversity Matrix

Choose models that **differ on these axes:**

| Axis | Why It Matters | Example Split |
|---|---|---|
| **Training data origin** | Western vs Chinese training corpora see different patterns | Gemini (Google) vs DeepSeek (Chinese) vs Qwen (Alibaba) |
| **Architecture type** | Standard vs reasoning models think differently | DeepSeek V3 (standard) vs DeepSeek R1 (chain-of-thought) |
| **Context window** | Larger context = more cross-file awareness | MiMo 1M vs DeepSeek V3 163K |
| **Cost tier** | Cheap models for volume, expensive for depth | Qwen3 $0.05 total vs Gemini $1.65 total |

### Recommended 5-Model Stack (Total: ~$3-5)

| Slot | Model | OpenRouter ID | Role | Est. Cost |
|---|---|---|---|---|
| **Scanner** | Qwen3 235B | `qwen/qwen3-235b-a22b-2507` | Ultra-cheap exhaustive enumeration | $0.05-0.15 |
| **Code Expert** | DeepSeek V3 | `deepseek/deepseek-chat-v3-0324` | Deepest code pattern matching | $0.15-0.30 |
| **Reasoner** | DeepSeek R1 | `deepseek/deepseek-r1-0528` | Absence detection, logic bugs, state machines | $0.40-0.80 |
| **SRE** | MiMo v2 Pro | `xiaomi/mimo-v2-pro` | Operational gaps, stuck states, timeouts | $0.70-1.50 |
| **Security** | Gemini 2.5 Pro | `google/gemini-2.5-pro-preview-03-25` | Security checklists, credential scanning | $1.50-2.00 |
| **Reviewer** | Opus 4.6 | Claude Code subscription | Cross-system architecture, false positive filtering | $0 (subscription) |

### When to Swap Models

- **New model beats existing on benchmarks?** Swap in the same slot (e.g., replace Gemini with GPT-5 if cheaper/better)
- **Budget constrained (<$1)?** Run only Qwen3 + DeepSeek V3 + Opus review (~$0.35)
- **Security-focused audit?** Add GPT-4.1 ($2/8 per 1M) for Western security perspective
- **Performance audit?** Add a model with strong systems programming knowledge

---

## 3. Infrastructure Setup

### 3.1 OpenRouter Account

```
1. Create account at openrouter.ai
2. Add credits ($10 minimum, covers ~3 full audits)
3. Create MANAGEMENT key (for key provisioning)
4. Create API keys per audit run (with $ limits)
```

**Key management commands:**
```bash
# Check credits
curl -s https://openrouter.ai/api/v1/credits \
  -H "Authorization: Bearer $MANAGEMENT_KEY"

# Create API key with limit
curl -s -X POST https://openrouter.ai/api/v1/keys \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $MANAGEMENT_KEY" \
  -d '{"name":"audit-YYYY-MM-DD","limit":10}'

# Check key usage
curl -s https://openrouter.ai/api/v1/auth/key \
  -H "Authorization: Bearer $API_KEY"
```

### 3.2 Audit Scripts

Two scripts in `racecontrol/scripts/`:

| Script | Purpose | Usage |
|---|---|---|
| `multi-model-audit.js` | Run a single model audit | `OPENROUTER_KEY="..." MODEL="..." node scripts/multi-model-audit.js` |
| `cross-model-analysis.js` | Cross-reference all model results | `node scripts/cross-model-analysis.js` |

**multi-model-audit.js features:**
- Parameterized model via `MODEL` env var
- Auto-splits batches for models with smaller context windows
- Per-model pricing tracking
- 7 audit batches covering all system components
- Enhanced system prompt that explicitly asks for absence-based issues

### 3.3 Output Structure

```
audit/results/
├── gemini-audit-YYYY-MM-DD/
│   ├── 01-server-rust.md
│   ├── 02-agent-rust.md
│   ├── 03-sentry-watchdog-common.md
│   ├── 04-comms-link.md
│   ├── 05-audit-detection-healing.md
│   ├── 06-deploy-infra.md
│   ├── 07-standing-rules-crosssystem.md
│   └── FULL-AUDIT-REPORT.md
├── deepseek-v3-audit-YYYY-MM-DD/
├── qwen3-235b-audit-YYYY-MM-DD/
├── deepseek-r1-audit-YYYY-MM-DD/
├── mimo-v2-pro-audit-YYYY-MM-DD/
└── cross-model-report-YYYY-MM-DD/
    ├── CROSS-MODEL-REPORT.md      ← Human-readable
    └── findings.json               ← Machine-readable
```

---

## 4. Execution Protocol

### Phase 1: Run Models (15-30 min, ~$3)

```bash
# Set your API key
export OPENROUTER_KEY="sk-or-v1-..."

# Run all 5 models in parallel (separate terminals or background)
MODEL="qwen/qwen3-235b-a22b-2507" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-chat-v3-0324" node scripts/multi-model-audit.js &
MODEL="xiaomi/mimo-v2-pro" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js &
MODEL="google/gemini-2.5-pro-preview-03-25" node scripts/multi-model-audit.js &
wait
echo "All models complete"
```

**Expected timing:**
| Model | Time | Batches |
|---|---|---|
| Qwen3 235B | ~5 min | 7 (no splits) |
| DeepSeek V3 | ~8 min | 9 (2 splits) |
| MiMo v2 Pro | ~10 min | 7 (no splits) |
| DeepSeek R1 | ~15 min | 9 (2 splits, reasoning overhead) |
| Gemini 2.5 Pro | ~8 min | 7 (no splits) |

### Phase 2: Cross-Model Analysis (1 min)

```bash
node scripts/cross-model-analysis.js
```

**Output categories:**
- **Consensus (3+ models agree):** Highest confidence — fix immediately
- **Two models agree:** High confidence — verify and fix
- **Unique (1 model only):** Most valuable — needs Opus review to filter false positives

### Phase 3: Opus Review (manual, 15-30 min)

Read `CROSS-MODEL-REPORT.md` and triage:

1. **Consensus findings** → Accept all, create fix tasks
2. **Two-model findings** → Verify each against code, accept real ones
3. **Unique findings** → Most valuable but highest false positive rate (~30%)
   - For each unique finding, ask: "Is this a real bug or a misunderstanding of the architecture?"
   - Check the actual code at the referenced file:line
   - Mark as REAL, FALSE POSITIVE, or KNOWN/ACCEPTED

### Phase 4: Opus Deep Dive (manual, 30-60 min)

What no OpenRouter model can do — use domain knowledge to find:
- **Cross-system coordination bugs** (recovery systems fighting each other)
- **Missing code** (timeouts that should exist, sentinels never written)
- **State machine stuck states** (from operational incident history)
- **Serde silent drops** (deny_unknown_fields missing on boundary structs)

### Phase 5: Self-Audit (mechanical, 10 min)

Things ALL models (including Opus) structurally miss:
```bash
# Count unwrap/expect in production code
grep -rn '\.unwrap()' crates/*/src/ --include='*.rs' | grep -v test | wc -l
grep -rn '\.expect(' crates/*/src/ --include='*.rs' | grep -v test | wc -l

# Count untracked tokio::spawn
grep -rn 'tokio::spawn' crates/*/src/ --include='*.rs' | grep -v test | wc -l

# Find HTTP clients without timeout
grep -rn 'Client::new()' crates/*/src/ --include='*.rs' | grep -v test

# Find format! SQL
grep -rn 'format!.*SELECT\|format!.*INSERT\|format!.*UPDATE' crates/*/src/ --include='*.rs'

# Find integer overflow casts
grep -rn 'as u32\|as i32\|as usize' crates/*/src/ --include='*.rs' | grep -v test | wc -l

# Run cargo clippy (if available)
cargo clippy --all-targets 2>&1 | head -50

# Check dependencies
cargo audit 2>&1 | head -20
```

---

## 5. The 7 Audit Batches

Each model receives the same 7 batches:

| Batch | Scope | Key Files | Focus Areas |
|---|---|---|---|
| **01** | Racecontrol Server | `crates/racecontrol/src/*.rs` | Route auth, SQL injection, unwrap(), billing logic, game state |
| **02** | RC-Agent | `crates/rc-agent/src/*.rs` | Exec injection, process guard bypass, Session 0/1, Windows bugs |
| **03** | Sentry/Watchdog/Common | `crates/rc-sentry/`, `rc-watchdog/`, `rc-common/`, `rc-process-guard/` | Restart loops, MAINTENANCE_MODE, recovery coordination |
| **04** | Comms-Link | `comms-link/shared/`, `james/`, `bono/` | PSK auth, exec injection, chain orchestration, message loss |
| **05** | Audit/Healing Pipeline | `audit/lib/`, `audit/phases/`, `scripts/detectors/`, `scripts/healing/` | Race conditions, billing gate bypass, notification flooding |
| **06** | Deploy/Infra | `scripts/deploy/`, `Cargo.toml`, `.cargo/config.toml` | Pipeline integrity, credential leaks, binary verification |
| **07** | Standing Rules | `CLAUDE.md` (both repos) | Rule conflicts, coverage gaps, stale references |

### System Prompt

The system prompt (embedded in `multi-model-audit.js`) includes:
- Architecture overview
- 6 standard audit categories (security, code quality, reliability, integration, process, infrastructure)
- **Enhanced: 3 absence-based categories** (what's missing, stuck states, cross-system assumptions)
- Structured output format (SEVERITY, CATEGORY, FILE, LINE, FINDING, IMPACT, FIX)

---

## 6. Model Strengths Map

Use this to understand what each model excels at:

| Bug Class | Best Model | Why |
|---|---|---|
| Hardcoded credentials | Gemini | Security checklist training |
| Auth gaps on endpoints | Gemini, MiMo | Pattern matching on route definitions |
| SQL injection | Gemini, V3 | Code pattern detection |
| Missing DB transactions | Gemini, MiMo, V3 | Financial code pattern |
| Serde silent drops | **None** — Opus only | Requires cross-boundary knowledge |
| State machine stuck states | R1, MiMo | Reasoning about transitions |
| Absence-based bugs | R1 | Chain-of-thought examines "what should be here" |
| Recovery system conflicts | **None** — Opus only | Requires multi-system architecture knowledge |
| Windows Session 0/1 | V3 | OS-specific code pattern |
| Timing/race conditions | R1, V3 | Reasoning + code analysis |
| Process lifecycle bugs | R1 | Thread/spawn lifecycle reasoning |
| Operational completeness | MiMo | SRE-style "what breaks at 3am" thinking |
| Volume/exhaustive coverage | Qwen3 | Ultra-cheap, produces most findings |
| Integer overflow / casts | **None** — mechanical grep | All models miss numeric edge cases |
| Untracked spawned tasks | **None** — mechanical grep | Structural pattern, needs counting |

---

## 7. False Positive Patterns

Common false positives by model (filter these during Opus review):

| Model | Common False Positive | How to Spot |
|---|---|---|
| Gemini | "Rust edition 2024 is invalid" | Stale training data — 2024 edition exists since Rust 1.85 |
| Gemini | LAN-only endpoints flagged as "critical" | Context-blind — venue is a closed network |
| Qwen3 | Duplicate findings (same bug, different wording) | Check file:line overlap |
| All models | "ws:// should be wss://" on Tailscale connections | Tailscale already encrypts the tunnel |
| All models | "ALLOWED_BINARIES includes dangerous commands" | By design — required for fleet ops |
| R1 | Over-detailed reasoning that restates the obvious | Filter findings where FINDING = IMPACT |
| MiMo | "Missing health check endpoint" when one exists | Verify with `grep /health` |

---

## 8. When to Run This Protocol

| Trigger | Scope | Models |
|---|---|---|
| **Before major milestone ship** | Full 7-batch audit | All 5 models |
| **After security incident** | Batch 01 (server) + 04 (comms) + 06 (deploy) | Gemini + R1 |
| **New crate/service added** | Single batch for new code | V3 + R1 + MiMo |
| **Monthly maintenance** | Full audit | All 5 models |
| **After dependency update** | Batch 06 (deploy/infra) + `cargo audit` | Any 1 model + mechanical checks |
| **Quick pre-deploy check** | Batch for changed crate only | Qwen3 (cheapest) |

---

## 9. Cost Reference (as of 2026-03-27)

| Model | OpenRouter ID | $/1M In | $/1M Out | Context | Full Audit Cost |
|---|---|---|---|---|---|
| Qwen3 235B | `qwen/qwen3-235b-a22b-2507` | $0.07 | $0.10 | 262K | ~$0.05 |
| DeepSeek V3 | `deepseek/deepseek-chat-v3-0324` | $0.20 | $0.77 | 163K | ~$0.16 |
| DeepSeek R1 | `deepseek/deepseek-r1-0528` | $0.45 | $2.15 | 163K | ~$0.43 |
| MiMo v2 Pro | `xiaomi/mimo-v2-pro` | $1.00 | $3.00 | 1M | ~$0.77 |
| Gemini 2.5 Pro | `google/gemini-2.5-pro-preview-03-25` | $1.25 | $10.00 | 1M | ~$1.65 |
| **Full 5-model audit** | | | | | **~$3.06** |

**Compare:** Opus-only equivalent would cost ~$187 and still miss 48 bugs.

---

## 10. Updating This Protocol

### Adding New Models

1. Check OpenRouter for new models: `curl -s https://openrouter.ai/api/v1/models | jq '.data[] | select(.id | contains("KEYWORD"))'`
2. Verify pricing and context window
3. Add to `MODEL_CONFIG` in `multi-model-audit.js`
4. Run alongside existing models, compare unique findings
5. If new model finds >5 unique real bugs, add to the standard stack

### Updating Audit Batches

When new crates or services are added:
1. Add a new batch in `multi-model-audit.js` (follow existing pattern)
2. Update the system prompt with new architecture details
3. Update batch count in this document

### Model Retirement

If a model consistently produces <3 unique findings after 3 audit runs, consider replacing it with a newer alternative that covers the same diversity axis.

---

## 11. Bono-Specific Notes

### Bono's Infrastructure

Bono runs on VPS (srv1422716.hstgr.cloud) with **Perplexity MCP** as his primary external model access — NOT direct OpenRouter API. This means:

- Bono has `pplx_smart_query`, `pplx_claude_opus`, `pplx_gemini_pro_think`, `pplx_gpt54` etc. via MCP
- These call Perplexity's API, NOT OpenRouter directly
- The `multi-model-audit.js` script requires direct OpenRouter HTTPS access + Node.js

### Option A: James Runs Audits, Bono Reviews (RECOMMENDED)

James runs all 5 OpenRouter model audits from the on-site machine (already proven), pushes results to git. Bono pulls and reviews the cross-model report using his Perplexity MCP for any follow-up research.

```
James workflow:
1. cd ~/racingpoint/racecontrol
2. OPENROUTER_KEY="..." run 5 models in parallel (see Section 4)
3. node scripts/cross-model-analysis.js
4. git add audit/results/ && git commit && git push
5. Notify Bono via WS + INBOX.md

Bono workflow:
1. git pull racecontrol
2. Read audit/results/cross-model-report-YYYY-MM-DD/CROSS-MODEL-REPORT.md
3. Use Perplexity MCP (pplx_smart_query) for follow-up research on findings
4. Add Bono-perspective review as Opus to the findings
5. Push review to git, notify James
```

### Option B: Bono Runs Directly (if Node.js available on VPS)

If Bono's VPS has Node.js and outbound HTTPS to openrouter.ai:

```bash
cd ~/racecontrol

# Use James's OpenRouter key (shared securely, NOT committed to git)
export OPENROUTER_KEY="sk-or-v1-..."

# Run full audit
for model in "qwen/qwen3-235b-a22b-2507" "deepseek/deepseek-chat-v3-0324" \
             "xiaomi/mimo-v2-pro" "deepseek/deepseek-r1-0528" \
             "google/gemini-2.5-pro-preview-03-25"; do
  MODEL="$model" node scripts/multi-model-audit.js &
done
wait

# Cross-reference
node scripts/cross-model-analysis.js
```

**Prerequisites:** Node.js v18+, outbound HTTPS to openrouter.ai, OpenRouter API key.
**Key sharing:** Pass via comms-link WS (encrypted), NEVER commit to git or INBOX.md.

### Option C: Bono Uses Perplexity MCP Models

Bono can use his Perplexity MCP to run individual model queries against code snippets, but this is manual and doesn't scale to the full 7-batch protocol. Best used for **targeted follow-up** on specific findings, not the full audit.

### Coordination Protocol

- **James** owns the OpenRouter account and runs the 5-model automated audit
- **Bono** reviews cross-model findings from git + adds Opus domain review
- Both contribute domain knowledge for the final triage
- Results pushed to racecontrol repo for shared access
- **NEVER commit OpenRouter API keys to git** — share via WS or env vars only

---

## Appendix A: First Audit Results Summary (2026-03-27)

| Metric | Result |
|---|---|
| Total raw findings | 334 (5 models) + 18 (Opus) = 352 |
| Consensus (3+ models) | 6 |
| Two-model corroboration | 11 |
| Unique (1 model only) | 292 |
| Bugs Opus missed | 48 (7 P1, 26 P2, 15 P3) |
| Bugs all models missed (Opus-only) | 10 (5 P1, 5 P2) |
| Bugs ALL sources missed | Hardware, load behavior, customer chaos |
| Total cost | $3.06 |
| Total time | ~30 min (parallel) + 30 min Opus review |
| Most dangerous find (model) | Unauthenticated venue shutdown (MiMo) |
| Most dangerous find (Opus) | WOL_SENT sentinel never written |
| Best value model | DeepSeek R1 — 18 Opus-misses for $0.43 |
