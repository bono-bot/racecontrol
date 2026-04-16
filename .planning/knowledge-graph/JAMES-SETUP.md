# Knowledge Graph + Operational Framework — James (On-Site)

## What This Is

A unified knowledge graph of the entire RacingPoint ecosystem — code structure + fix history —
integrated with the full operational framework: CGP, GSD, CLD, MMA, deploy pipeline, security gates.

**Built by Bono (2026-04-16). Commits: `eb4f146a`, `23f0b171`, `959bb875`.**

---

## Part 0: Operational Framework (READ FIRST)

The knowledge graph is ONE tool in a complete operational system. Everything below must
be in place for it to be useful. This section maps the E2E workflow.

### CGP v4.3 — Cognitive Gate Protocol (MANDATORY)

**Location:** `COGNITIVE-GATE-PROTOCOL.md` at repo root + CLAUDE.md standing rules section.

5 hard gates enforced by hooks — cannot skip:

| Gate | When | What | Graph integration |
|---|---|---|---|
| **H1** | Before action tools | PROBLEM + PLAN block | Graph query goes in PLAN step |
| **H2** | Completion claims | Fix and verify in SEPARATE messages | Never claim graph-done without rebuild |
| **H3** | Before "done/fixed" | Behavior + raw output + WHERE + not tested | Graph query is evidence, not proxy |
| **H4** | Before "all/everywhere" | Grep + per-target list | Graph communities ≠ target enumeration |
| **H5** | User correction | Root cause + structural fix + G9 count | Update fix-history after correction |

**Backlog gate:** WIP >= 3 blocks new features. COMMITTED ≠ SHIPPED.

**Session metrics (report at end):** `Claims: N | Corrections: N | FCR: N% | G9s: N`

### CLD v1.0 — Closed-Loop Debug (PRIMARY METHOD)

**Location:** `docs/CLOSED-LOOP-DEBUG.md`

Every investigation starts AND ends at the layer closest to the user:

1. **OPEN** — Reproduce the EXACT symptom (screenshot/curl/tasklist, NOT health check)
2. **DESCEND** — 6 layers: Smoke → Function → Boundary → Infra → Data → Code
3. **FIX** — Smallest change at the root cause layer
4. **CLOSE** — Re-run the EXACT same test from Step 1
5. **SWEEP** — Verify ALL deploy targets (venue + cloud + pods)

**Graph integration:** Before Step 2, run `query-fixes.py` with the symptom. If a past
fix exists, go directly to verifying whether it's still applied — skip re-investigation.

### 4-Tier Debug Order

| Tier | Method | Graph role |
|---|---|---|
| 1 | **Deterministic** — stale sockets, cleanup, WerFault | `query-fixes.py` finds past deterministic fixes |
| 2 | **Memory** — LOGBOOK + commit history | fix-history.json IS this tier, automated |
| 3 | **Local Ollama** — qwen2.5:3b at .27:11434 | Graph nodes inform the prompt |
| 4 | **Cloud Claude** — escalate (last resort) | Full graph context available |

### Cause Elimination Process

1. **Reproduce & Document** — screenshot/error/exact steps
2. **Hypothesize** — list ALL possible causes (not just first)
3. **Test & Eliminate** — one by one with evidence
4. **Fix & Verify** — reproduce original trigger post-fix
5. **Log** — LOGBOOK.md entry + run `extract-fix-history.py` to update graph

### Reference Docs (check before investigating from scratch)

| Doc | Location | What |
|---|---|---|
| ERROR-CATALOG.md | `docs/ERROR-CATALOG.md` | 35 known errors indexed by symptom |
| DIAGNOSTIC-PLAYBOOK.md | `docs/DIAGNOSTIC-PLAYBOOK.md` | Single entry point for all diagnostics |
| LOG-LOCATIONS.md | `docs/LOG-LOCATIONS.md` | Every log file on every machine |
| SERVICE-REFERENCE.md | `docs/SERVICE-REFERENCE.md` | Per-binary deep dive |
| DATA-FLOW-DIAGRAMS.md | `docs/DATA-FLOW-DIAGRAMS.md` | 9 flow diagrams |
| API.md | `docs/API.md` | ~403 endpoints across 7 auth tiers |
| ARCHITECTURE.md | `docs/ARCHITECTURE.md` | System overview, crates, topology |

### GSD Workflow

Standard phase lifecycle:
```
/gsd-discuss-phase → /gsd-plan-phase → /gsd-execute-phase → /gsd-verify-work → /gsd-ship
```

**Subagent gates (MANDATORY per phase type):**

| Phase Type | Required Agent | Artifact |
|---|---|---|
| Any frontend | `gsd-ui-researcher` + `gsd-ui-auditor` | UI-SPEC.md + UI-REVIEW.md |
| Multi-phase milestone (3+) | `gsd-integration-checker` | Integration check |
| Business logic | `gsd-nyquist-auditor` | Test coverage |
| New milestone | `gsd-codebase-mapper` | Refresh codebase/ |

### MMA Protocol v3.0 — Multi-Model Audit

**When required:** Before milestone ship, after security incident, new crate/service,
cross-system bridge deploy.

**Location:** `.planning/specs/UNIFIED-MMA-PROTOCOL.md`

4 steps: DIAGNOSE (5 models) → PLAN (5 models) → EXECUTE (smallest fix) → VERIFY (3 adversarial)

Budget: $5/session via OpenRouter. Key recovery: auto-provisions new key on 401.

### Security Gates

| Gate | What | When |
|---|---|---|
| SEC-GATE-01 | `node comms-link/test/security-check.js` — 31 assertions | Before any deploy |
| Pre-commit hooks | Block credentials, .unwrap(), `any` | Every commit |
| SEC-GATE-02 | Staged changes credential scan | Every commit (auto) |

### Deploy Pipeline (MANDATORY for every phase)

**Deploy Manifest Protocol (DMP):** Run `bash scripts/deploy/deploy-audit.sh <old_hash> <new_hash>`

| Step | rc-agent (pods) | racecontrol (server) | Frontends |
|---|---|---|---|
| 1 | `cargo build --release --bin rc-agent` | `cargo build --release --bin racecontrol` | `npm run build` |
| 2 | Stage to deploy-staging/ | Stage to deploy-staging/ | tar + SCP |
| 3 | Download via HTTP :18889 | SSH download or SCP | Extract on target |
| 4 | Kill → RCWatchdog restarts | SSH kill → schtasks start | Restart schtask |
| 5 | Verify build_id on /health | Verify build_id on /health | Verify API proxy |
| 6 | Test EXACT behavior | Test EXACT behavior | Test from browser |
| 7 | **Cloud parity** | **Cloud parity** | **Cloud parity** |

**CRITICAL:** Rebuild ALL 3 frontends (kiosk, web, admin) after ANY server deploy.

**Visual verification:** Any change affecting screens MUST be visually confirmed.

### Comms Protocol

| Channel | Method | When |
|---|---|---|
| INBOX.md | `git add INBOX.md && git commit && git push` | Every commit/task |
| WS message | comms-link `send-message.js` | Alongside git push |
| Email | james@racingpoint.in / bono@racingpoint.in | Escalation |

**Auto-push + notify:** `git push` → WS message → INBOX.md entry. All three. Every commit.

### Standing Rules Quick Reference

**Code quality:** No `.unwrap()` in Rust. No `any` in TypeScript. Cascade updates recursive.
Static CRT. `.bat` files = clean ASCII + CRLF + goto labels (no parentheses).

**Deploy:** DEPLOY PARITY (local + cloud). Test before upload (Pod 8 canary first).
Never run pod binaries on James's PC. SP game launch = direct acs.exe (no bat).
rc-agent MUST run in Session 1 (interactive desktop) — Session 0 blocks ALL GUI operations.
Server deploy: `deploy-server.sh` v3.0 (8 steps with auto-rollback). See CLAUDE.md "Server deploy" section.

**Cross-process updates:** Changing a feature? Update ALL: rc-agent, racecontrol, PWA, Admin,
Gateway, Dashboard. ALL environments: venue (.23), cloud (Bono VPS), James (.27).
Deploy to one and forget the other = schema divergence.

**Process:** Milestone = update ARCHITECTURE.md + memory. Fix commits need G4 NOT TESTED.
Refactor second (tests first). Route uniqueness (same-commit delete old).

**Verification:** Verify EXACT behavior, not proxies. "Removed" = removed from EVERY machine.
Audit what CUSTOMER sees. Never dismiss anomalies. Fix during audit, don't catalog.

### Ultimate Rule — 4 Verification Layers Before Ship

```bash
# 1. Quality Gate — automated tests
cargo test + run-all.sh

# 2. E2E — live round-trip verification
curl endpoints, verify actual behavior

# 3. Standing Rules — compliance check
All rules in this document

# 4. Multi-Model AI Audit — cross-model consensus
node scripts/multi-model-audit.js (for milestones)
```

### Memory System

| Location | Count | What |
|---|---|---|
| Bono memory | 51 files | `/root/.claude/projects/-root/memory/` |
| James memory | 163 files | `C:\Users\bono\.claude\projects\C--Users-bono\memory\` |
| Partner sync | Hooks | `partner-memory-read.js` loads partner memory at session start |

**Rule:** Check memory FIRST before claiming "no context." NEVER say "I don't know" without
checking MEMORY.md, LOGBOOK, git log, and ERROR-CATALOG.

## What's Available

| Layer | Scope | Stats |
|-------|-------|-------|
| Code graph (racecontrol) | Single repo | 9,268 nodes, 22,676 edges, 344 communities |
| Ecosystem graph | 12 repos | 10,914 nodes, 27,126 edges, 469 communities |
| Fix-history overlay | 1,135 commits | 3,683 nodes annotated, 41 symptom categories |

## Setup Steps (Windows — Git Bash)

### 1. Pull Latest

```bash
cd ~/racingpoint/racecontrol
git pull origin main
```

### 2. Install Graphify

```bash
# Check Python version (need 3.10+)
python3 --version

# Install via pip (Windows doesn't have pipx by default)
pip install graphifyy

# OR if pip blocked:
python3 -m pip install --user graphifyy

# Register as Claude Code skill
graphify install --platform claude
```

### 3. Build the Code Graph (racecontrol only — ~30 seconds)

```bash
cd ~/racingpoint/racecontrol
graphify update .
```

Output appears in `graphify-out/` (symlinked to `.planning/knowledge-graph/`).

### 4. Build the Fix-History Overlay (~60 seconds)

```bash
python3 scripts/knowledge-graph/extract-fix-history.py
```

This creates `.planning/knowledge-graph/fix-history.json` (gitignored — local only).

### 5. (Optional) Build the Ecosystem Graph

To graph ALL repos (not just racecontrol):

```bash
# Create staging area with all repos
mkdir -p /tmp/racingpoint-ecosystem
for dir in ~/racingpoint/racecontrol ~/racingpoint/comms-link ~/racingpoint/racingpoint-admin ~/racingpoint/racingpoint-api-gateway ~/racingpoint/racingpoint-dashboard ~/racingpoint/racingpoint-discord-bot ~/racingpoint/racingpoint-whatsapp-bot; do
  name=$(basename "$dir")
  find "$dir" -type f \( -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.py" \) \
    ! -path "*/node_modules/*" ! -path "*/.next/*" ! -path "*/target/*" ! -path "*/.git/*" \
    -exec cp --parents {} /tmp/racingpoint-ecosystem/ \; 2>/dev/null
done

# Build
cd /tmp/racingpoint-ecosystem
graphify update .

# Copy to racecontrol planning
cp graphify-out/graph.json ~/racingpoint/racecontrol/.planning/knowledge-graph/ecosystem-graph.json
cp graphify-out/GRAPH_REPORT.md ~/racingpoint/racecontrol/.planning/knowledge-graph/ECOSYSTEM-REPORT.md
```

### 6. Install Git Hooks (auto-rebuild on commit)

```bash
cd ~/racingpoint/racecontrol
graphify hook install
```

### 7. Session Startup Hook

Copy the knowledge-graph lookup hook to your Claude Code hooks:

```bash
# The hook file is at: ~/.claude/hooks/knowledge-graph-lookup.js
# Copy from the repo's Bono version and adjust paths:
```

Create `C:\Users\bono\.claude\hooks\knowledge-graph-lookup.js` with the content from
`/root/.claude/hooks/knowledge-graph-lookup.js` (on Bono VPS), replacing:
- `/root/racecontrol` → `C:/Users/bono/racingpoint/racecontrol`

Then add to `C:\Users\bono\.claude\settings.json` under `UserPromptSubmit` hooks:
```json
{
  "type": "command",
  "command": "node \"C:\\Users\\bono\\.claude\\hooks\\knowledge-graph-lookup.js\"",
  "timeout": 6
}
```

## How to Use

### Query by symptom (most useful)
```bash
python3 scripts/knowledge-graph/query-fixes.py "pod keeps restarting"
python3 scripts/knowledge-graph/query-fixes.py "billing refund wrong amount"
python3 scripts/knowledge-graph/query-fixes.py "blanking screen not working"
python3 scripts/knowledge-graph/query-fixes.py "game launch stuck"
```

### Show most-fixed code (hotspots)
```bash
python3 scripts/knowledge-graph/query-fixes.py --hotspots
```

### List all symptom keys
```bash
python3 scripts/knowledge-graph/query-fixes.py --symptoms
```

### Query code structure
```bash
graphify query "how does billing connect to game launch"
graphify explain "restart_service()"
graphify path "handle_crash()" "enter_maintenance_mode()"
```

### Cross-repo query (ecosystem graph)
```bash
graphify query "what connects whatsapp to billing" --graph .planning/knowledge-graph/ecosystem-graph.json
```

## Sync Protocol

| What | Who builds | How often | Shared via |
|------|-----------|-----------|------------|
| graph.json (racecontrol) | Each AI locally | Auto on commit (git hook) | Not shared — local |
| ecosystem-graph.json | Each AI locally | After `git pull` with new repos | Not shared — local |
| fix-history.json | Each AI locally | After batch of fix commits | Not shared — local |
| GRAPH_REPORT.md | Auto (graphify) | On commit | Git (committed) |
| ECOSYSTEM-REPORT.md | Bono | Periodically | Git (committed) |
| extract-fix-history.py | Shared | On code changes | Git (committed) |
| query-fixes.py | Shared | On code changes | Git (committed) |

Both AIs build their own graphs locally because:
- Graphs reflect local working tree (including uncommitted changes)
- 16MB JSON files are too large for git
- Each machine's graph should match its own repo state

## Integration with GSD

When starting a new GSD phase:
1. Run `graphify query "what handles <area>"` to understand the code landscape
2. Run `python3 scripts/knowledge-graph/query-fixes.py "<area>"` to find past issues
3. Check `GRAPH_REPORT.md` god nodes to identify structural bottlenecks
4. Check `ECOSYSTEM-REPORT.md` for cross-repo dependencies

When investigating a bug:
1. The session startup hook auto-surfaces past fixes (if keywords match)
2. Run `query-fixes.py` with the symptom for detailed results
3. Check blast radius in query output to know what else might break

## Complete GSD Context — All Incomplete Work

### v39.0 — Session Trace ID (1 incomplete plan)

| Phase | Plan | What | Graph Query | Deps |
|---|---|---|---|---|
| 310 | 310-02 | Dashboard events + GET /sessions/{id}/trace | `graphify query "session trace dashboard"` | None |

### v46.0 — Game Launch Diagnostics (3 incomplete plans, Phase 362 shipped)

| Phase | Plan | What | Graph Query | Deps |
|---|---|---|---|---|
| 363 | deploy | Billing 5s grace window + lap audit | `query-fixes.py "billing session end lap"` | Deploy pending |
| 364 | plans | Launch reliability (details TBD) | `query-fixes.py "game launch stuck"` | Phase 363 |
| 365 | plans | AI behavior validation via MMA | `graphify query "ai_behavior mma"` | Phase 364 |
| 366 | COMPLETE | Fleet intelligence (composite health 0-100) | — | — |
| 367 | COMPLETE | Staff tools (suspect lap triage) | — | — |

### v47.0 — Admin Dashboard Venue-Ready (25 incomplete plans across 7 phases)

**Blocker: Phase 343 Plans 01+02+04 must ship before Phase 347**

| Phase | Plans Left | What | Graph Query | Deps |
|---|---|---|---|---|
| **345** | 3 plans | Backend resilience: rc proxy env, admin.db lazy-load, halt-on-missing-secrets | `graphify query "admin proxy resilience"` | Phase 344 |
| **346** | 3 plans | Cafe proxy rewrite: schema-diff, drop migration, identity consolidation | `graphify query "cafe menu proxy admin"` (ecosystem graph) | Phase 345 |
| **350** | 3 plans | Contract tests: cafe/pricing/coupon, staff PIN, 46-page smoke test | `graphify query "contract test billing"` | Phases 346, 347 |
| **354** | 1 plan | UI hardening: skeleton gaps (4), replace alert() with toast (15), empty states | `graphify query "admin page skeleton"` | Phase 345 |
| **355** | 2 plans | Venue-ready readiness review: VERIFICATION.md + milestone close | — | All above |
| **356** | 3 plans | Business rules SSOT: schema migration, refactor billing.rs/routes.rs consumers, admin /settings page | `graphify query "business rules pricing"` | Phase 346 |
| **357** | 3 plans | Pricing tier admin: CRUD endpoints, admin /pricing/tiers page, remove kiosk hardcoded strings | `query-fixes.py "pricing tier"` | Phase 356 |
| **358** | 3 plans | Cafe promos admin: list view, create/edit modal, broadcast integration | `graphify query "cafe promo broadcast"` (ecosystem) | Phase 356 |
| **359** | 2 plans | Wallet bonus tiers: CRUD endpoints, admin page | `graphify query "wallet bonus tier"` | Phase 356 |
| **360** | 2 plans | Topup presets: admin editor page, contract test | `graphify query "wallet topup preset"` | Phase 359 |

### v48.0 — Codebase Architecture: Department-Driven Event Mesh (14 incomplete phases)

All 14 phases are code-committed (2026-04-13) but NOT deployed. Deploy is Phase 383.

**P0 — Must ship (revenue-affecting):**

| Phase | What | Graph Query | Key Files |
|---|---|---|---|
| **369** | AC Launch Rewrite — VMS-parity, separate staff/PWA paths | `query-fixes.py "game launch AC"` | `ac_launcher.rs`, `launch_verifier.rs` |
| **370** | Multi-Game Launch — F1 25, iRacing, LMU; SimLauncher trait | `graphify query "SimLauncher game adapter"` | `sim_adapters/`, `game_state.rs` |
| **371** | Lap Recording — All 4 games E2E; leaderboard within 10s | `query-fixes.py "lap recording zero laps"` | `telemetry_handler.rs`, `laps` table |
| **372** | Billing Arcade Model — Coin-first, per-minute, crash-pause | `query-fixes.py "billing arcade pricing"` | `billing.rs`, `billing_fsm.rs`, `billing_timer.rs` |
| **373** | Multiplayer — Simultaneous launch, atomic billing, continuous laps | `query-fixes.py "multiplayer group session"` | `billing_multiplayer.rs`, `ac_server.rs` |

**P1 — Should ship (customer experience):**

| Phase | What | Graph Query | Key Files |
|---|---|---|---|
| **374** | PWA Self-Service Launch — PIN generation, pin-grid on pod | `graphify query "customer pin launch"` | `kiosk/`, `terminal_pin` |
| **375** | Wallet Types — Cash vs promotional credits, refund enforcement | `graphify query "wallet credit promotional"` | `wallet_transactions`, `billing.rs` |
| **376** | Cafe Integration — Cafe wallet debit, combo deals with racing | `graphify query "cafe wallet combo"` (ecosystem) | `cafe_items`, `billing.rs` |
| **377** | Customer Experience — Multi-game stats/PBs, <15s launch | `graphify query "personal best leaderboard"` | `personal_bests`, `track_records` |
| **378** | Marketing Engine — Low-utilization detection, WhatsApp deals | `graphify query "marketing whatsapp push"` (ecosystem) | WhatsApp bot + racecontrol |

**P2 — Foundation (technical debt):**

| Phase | What | Graph Query | Key Files |
|---|---|---|---|
| **379** | Event Bus Foundation — DomainEvent enum, mesh broadcast | `graphify query "domain event broadcast mesh"` | `ws/mod.rs`, `mesh_gossip` |
| **380** | Codebase Decomposition — routes.rs 55 modules, billing.rs split | `GRAPH_REPORT.md` god nodes | `routes.rs` (169 fixes!), `billing.rs` |
| **381** | Fix Tooling — Blast-radius tool, insertion:deletion hook | Knowledge graph IS this tool | `scripts/knowledge-graph/` |
| **382** | Foundation & CI — Feature registry, dead code removal, CODEOWNERS | `graphify query "feature flag registry"` | `FEATURE-REGISTRY.toml` |

### v49.0 — Unified RaceControl Operations (6 incomplete of 10 phases)

| Phase | Status | What | Graph Query | Deps |
|---|---|---|---|---|
| **383** | Cloud done, venue pending | Deploy pipeline: ship v48+v46 | `query-fixes.py "deploy binary swap"` | None (first) |
| **384** | Awaiting venue test | Lap recording wiring (~70 lines) | `query-fixes.py "lap telemetry adapter"` | Phase 383 |
| **385** | COMPLETE | Architecture: billing.rs + db splits | — | Phase 384 |
| **386** | COMPLETE | Autonomous pricing engine | — | Phase 384 |
| **387** | COMPLETE | Customer opt-in/opt-out preferences | — | Phase 384 |
| **388** | COMPLETE | Autonomous marketing triggers | — | Phase 387 |
| **389** | Incomplete | Game launch completion | `query-fixes.py "game launch adapter"` | Phases 383, 384 |
| **390** | COMPLETE | Spectator displays + cloud access | — | Phase 384 |
| **391** | COMPLETE | Digital staff operations | — | Phase 384 |
| **392** | Not started | Unified readiness review (E2E) | Full graph + ecosystem graph | All above + v47 |
| **392.1** | Layer 1 deployed | P0 zero-laps 3-layer fix + FK-PRAGMA | `query-fixes.py "zero laps sentry"` | Phase 384 |

### Backlog (parked — promote with /gsd-review-backlog)

| Phase | What | Graph Query |
|---|---|---|
| 999.1 | Deployment infrastructure improvements | `query-fixes.py "deploy"` |
| 999.2 | Infrastructure hardening items | `graphify query "infrastructure hardening"` |
| 999.3 | Network/maintenance improvements | `graphify query "network tailscale"` |

### Summary: 50+ incomplete items

| Milestone | Phases | Plans Left | Priority | Blocked By |
|---|---|---|---|---|
| v39.0 | 1 | 1 plan | Low | None |
| v46.0 | 3 | 3 plans | Medium | Deploy (Phase 383) |
| v47.0 | 10 | 25 plans | Medium | Phase 343 → Phase 347 chain |
| v48.0 | 14 | All TBD | HIGH (code-committed) | Deploy (Phase 383) |
| v49.0 | 6 | Plans TBD | HIGHEST (P0 laps) | Phase 383 → 384 chain |
| Backlog | 3 | All TBD | Low | N/A |

**Critical path:** Phase 383 (deploy) → Phase 384 (laps) → everything else unlocks.

### How the graph accelerates each milestone:

| Milestone | Graph value |
|---|---|
| **v46.0** | `query-fixes.py "game launch"` shows 30+ past launch fixes — don't re-investigate |
| **v47.0** | Ecosystem graph shows admin↔racecontrol↔dashboard API boundaries — prevents proxy bugs |
| **v48.0** | `GRAPH_REPORT.md` god nodes = which files to split in Phase 380. Community clusters = natural event bus boundaries for Phase 379 |
| **v49.0** | `query-fixes.py "zero laps telemetry"` shows exact P0 fix history. Blast radius for Phase 384 wiring |
