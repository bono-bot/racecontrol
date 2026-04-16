# Knowledge Graph — Symptom-Indexed Fix Registry

Code structure graph (Graphify) + fix-history overlay = fast symptom → fix lookup.

## Quick Start

```bash
# Rebuild code graph (after code changes)
graphify update .

# Rebuild fix history overlay (after new fix commits)
python3 scripts/knowledge-graph/extract-fix-history.py

# Query by symptom
python3 scripts/knowledge-graph/query-fixes.py "pod keeps restarting"
python3 scripts/knowledge-graph/query-fixes.py "billing refund wrong"
python3 scripts/knowledge-graph/query-fixes.py "screens 1024x768"

# Show most-fixed code hotspots
python3 scripts/knowledge-graph/query-fixes.py --hotspots

# List all symptom keys
python3 scripts/knowledge-graph/query-fixes.py --symptoms

# Lookup by exact symptom key
python3 scripts/knowledge-graph/query-fixes.py --symptom crash-loop
```

## Architecture

```
Layer 1: CODE STRUCTURE (Graphify)
  9,237 nodes | 22,595 edges | 339 communities
  Built from: Tree-sitter AST parsing of 1,020 source files
  Output: graph.json (12MB), GRAPH_REPORT.md (88KB)

Layer 2: FIX HISTORY (git + LOGBOOK + ERROR-CATALOG)
  1,134 fix commits | 314 LOGBOOK entries | 35 error catalog entries
  4,599 graph nodes annotated with symptoms + fix commits
  41 symptom categories for reverse lookup
  Output: fix-history.json (9MB)

Layer 3: QUERY (query-fixes.py)
  Natural language → symptom matching → graph node lookup
  TF-IDF ranking (specific nodes rank higher than generic)
  Shows: relevant nodes, blast radius, keyword-matched commits
```

## File Locations

| File | Tracked | Purpose |
|------|---------|---------|
| `GRAPH_REPORT.md` | Yes | God nodes, communities, knowledge gaps |
| `graph.json` | No (gitignored) | Full code graph — rebuild with `graphify update .` |
| `fix-history.json` | No (gitignored) | Fix overlay — rebuild with `extract-fix-history.py` |
| `cache/` | No (gitignored) | SHA256 hashes for incremental updates |

## Setup (James / new machine)

```bash
# 1. Install Graphify
pipx install graphifyy
graphify install --platform claude

# 2. Build code graph
cd ~/racingpoint/racecontrol
graphify update .

# 3. Build fix history
python3 scripts/knowledge-graph/extract-fix-history.py

# 4. (Optional) Install git hooks for auto-rebuild
graphify hook install
```

## Symptom Categories

restart, crash-loop, maintenance-mode, watchdog, websocket, auth-failure,
blanking-screen, flicker, display-resolution, billing, wallet, refund,
pricing, game-launch, assetto-corsa, game-config, telemetry, ffb-wheelbase,
deploy, binary, binary-swap, bat-file, stale-build, config, allowlist,
service-key, feature-flag, remote-access, pod-specific, fleet-wide, cloud,
comms-link, session-0, orphan-process, zombie-process, timeout,
port-conflict, kiosk, edge-browser, overlay, session-end
