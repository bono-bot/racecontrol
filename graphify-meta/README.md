# graphify-meta — map-of-maps for the RacingPoint ecosystem

Produces an inter-module knowledge graph by aggregating per-module `graphify-out/graph.json`
files and discovering cross-module coupling across 5 channels.

## What it does

Each of the racingpoint-* repos has its own `graphify-out/graph.json` (produced by running
`/graphify` in that repo). This tooling reads all of them and synthesises a **map of maps**
showing how modules relate.

5 edge channels:

| Channel | How detected | Confidence |
|---|---|---|
| `api-call` | Grep for `/api/...` URL literals + fetch/axios/fetchApi verbs | HIGH |
| `shared-ws-variant` | Shared WS protocol-enum variants (`AgentMessage::X`) | HIGH |
| `shared-db-table` | Same SQL table read/written in both modules | HIGH |
| `label-overlap` | Shared top-30 function/file names (after 2-tier stoplist) | LOW |
| `contains` | Synthetic parent → slice edges (e.g. `racecontrol → rc.kiosk`) | N/A |

## Quick commands

```bash
# Regenerate everything (5-stage pipeline, ~1.7s, no API cost)
node graphify-meta/rebuild-all.mjs

# Regression test (38 assertions — run after changing any script)
node graphify-meta/smoke-test.mjs

# Rebuild individual stage
node graphify-meta/slice-submodules.mjs   # 8 sub-slices of racecontrol
node graphify-meta/api-edges.mjs          # HTTP URL coupling
node graphify-meta/db-edges.mjs           # SQL/Prisma coupling
node graphify-meta/ws-edges.mjs           # WS enum variant coupling
node graphify-meta/build-meta.mjs         # assemble + emit viz
```

## Output artifacts (gitignored — regenerable)

| File | Contents |
|---|---|
| `meta-graph.json` | Machine-readable: modules + edges |
| `meta.html` | Interactive vis-network viz (double-click node to open slice graph) |
| `META_REPORT.md` | Summary: module stats, top edges, warnings |
| `api-edges.json` | Forward HTTP edges (consumer → racecontrol) + back-edges |
| `db-edges.json` | Shared-SQL-table edges (per-module table inventory) |
| `ws-edges.json` | Shared WS-enum-variant edges |
| `slices.json` | 8 sub-slice manifest (ROOT-relative paths) |
| `SLICE_GAP_SUMMARY.json` | Gap-rule counts per slice |
| `../graphify-out-<id>/graph.{json,html}` | Per-slice sub-graphs (inside racecontrol repo) |

## Auto-rebuild

`.git/hooks/post-commit` runs `rebuild-all.mjs` when:
- `graphify-out/graph.json` is newer than `graphify-meta/meta-graph.json` (backend rebuilt), OR
- Any `graphify-meta/*.mjs` or `scan-module.py` was in the commit.

Non-blocking — hook failure doesn't fail the commit.

Install on a fresh clone: `bash scripts/graphify-post/install-hook.sh`

## Module registry

Edit `build-meta.mjs` MODULES array to add/remove modules. Each entry:
```js
{ id: 'my-module', label: 'My Module', color: '#4E79A7', purpose: 'What it does' }
```

Modules resolve `graph.json` at `<ROOT>/<id>/graphify-out/graph.json` unless overridden
by `graph_path_override` (used by the 8 sub-slices of racecontrol).

## Gap detection

`META_REPORT.md` surfaces two warnings automatically:

1. **Low-density graphs** — modules with `< 0.5 nodes/file AND < 20 nodes` OR `< 10 nodes total`.
   Usually means graphify auto-skipped due to small corpus. Fix: `/graphify <module> --mode deep`.
2. **Missing graph.html** — module has `graph.json` but no `graph.html`. Usually means the
   graph is too large for graphify's viz (> 5000 nodes). Fix: lightweight slicing.

## Sessions

See `HANDOFF.md` for full context, `HANDOFF-GAPS.md` for the remaining-work checklist.
