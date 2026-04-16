# Knowledge Graph Setup — James (On-Site)

## What This Is

A unified knowledge graph of the entire RacingPoint ecosystem — code structure + fix history.
Prevents re-investigating solved bugs and provides architectural visibility for future builds.

**Built by Bono (2026-04-16). Commits: `eb4f146a`, `23f0b171`, `959bb875`.**

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

## Current GSD Context (for graph-informed work)

### Active milestones with incomplete phases:

**v46.0 — Game Launch Diagnostics** (3 incomplete plans)
- Phase 363: deploy deferred
- Graph areas: `ac_launcher.rs`, `game_state.rs`, `launch_verifier.rs`

**v47.0 — Admin Dashboard Venue-Ready** (25 incomplete plans)
- Phases 345-360: backend resilience, cafe proxy, contract tests, UI hardening
- Graph areas: `racingpoint-admin/`, routing, cafe integration
- Blocker: Phase 343 Plans 01+02+04 must ship before Phase 347

**v48.0 — Codebase Architecture** (14 incomplete phases)
- P0: AC launch rewrite, multi-game, lap recording, billing arcade, multiplayer
- P2: event bus, codebase decomposition, fix tooling
- Graph areas: `billing.rs` (god node), `routes.rs`, `ws_handler.rs`

**v49.0 — Unified RaceControl Operations** (6 incomplete)
- Phase 383: deploy pipeline (cloud done, venue pending)
- Phase 384: lap recording (awaiting venue AC launch test)
- Phase 389: game launch completion
- Phase 392: unified readiness review
- Phase 392.1: P0 zero-laps fix

### Where the graph helps most:
- **v48.0 Phase 380 (Codebase Decomposition)**: `GRAPH_REPORT.md` god nodes show which files need splitting — `billing.rs`, `routes.rs` are structural bottlenecks
- **v48.0 Phase 379 (Event Bus)**: community clusters show natural boundaries for event domains
- **v49.0 Phase 389 (Game Launch)**: query "game launch" surfaces all 30+ past launch fixes
- **v47.0 Phase 346 (Cafe Proxy)**: ecosystem graph shows admin↔racecontrol↔dashboard connections
