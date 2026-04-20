# graphify-post — gap detection & repair for graphify output

Permanent (git-tracked) post-processor for `graphify-out/`. Runs against
whatever graph graphify produces and:

1. **validate.mjs** — detects 24 gaps, emits `GAP_REPORT.md` + `.json`. Non-destructive.
2. **repair.mjs** — applies auto-fix rules to a COPY of `graph.json` → `graph.repaired.json` + side-by-side `*.repaired.html`. Never overwrites.
3. **gap-rules.mjs** — declarative rule catalogue. Add / tune rules here.

Install not required — runs with plain Node (no deps).

## Usage

```bash
# from racecontrol repo root
node scripts/graphify-post/validate.mjs                    # reads graphify-out/graph.json
node scripts/graphify-post/repair.mjs                      # writes graphify-out/graph.repaired.json

# explicit directory
node scripts/graphify-post/validate.mjs path/to/graphify-out
```

## Wiring into the /graphify flow

Graphify's SKILL.md Step 9 finishes with a success message. Run validate.mjs
after that step — or add a post-commit hook:

```bash
# .git/hooks/post-commit
if [ -f graphify-out/graph.json ]; then
  node scripts/graphify-post/validate.mjs graphify-out > /dev/null 2>&1 && \
    echo "graphify-post: GAP_REPORT.md updated"
fi
```

## Rule catalogue (24 rules)

| ID | Severity | Fixability | Title |
|----|---|---|---|
| G1 | P0 | warn | GET()/run() name-collision hub nodes (523 cross-community edges on one `GET()` node) |
| G2 | P1 | auto | Label-duplicate pollution (97× `page.tsx`, 88× `.new()`) — renamed with path suffix |
| G3 | P0 | manual | Struct-field access edges invisible (AcLaunchParams deg=1) — needs AST-level re-extract |
| G4 | P1 | manual | Unit tests disconnected from functions under test |
| G5 | P1 | manual | Cross-language edges (Python → Rust) missing |
| G6 | P1 | manual | External-boundary edges (spawns/binds/reads_file/writes_sentinel) missing |
| G7 | P2 | warn | Weak community modularity (c12 spread across 15+ partner communities) |
| G8 | P2 | manual | Single-file dominance (57% of c12 from one file) |
| G9 | P3 | manual | Semantic mis-clustering (bot_coordinator in AC-launcher community) |
| G10 | P3 | auto* | Absolute Windows paths baked into node IDs (*detect-only, auto-rewrite risky) |
| G11 | P1 | auto | Community HTML legend dead — fixed by setting `group = comm_<id>` on every node |
| G12 | P1 | warn | Community IDs re-shuffle on every rebuild (77.8% drift between hour-apart snapshots) |
| G13 | P1 | auto | AMBIGUOUS confidence tier unused — retag INFERRED<0.6 as AMBIGUOUS |
| G14 | P2 | manual | 5 schema-declared relations (implements/cites/etc) never emitted |
| G15 | P2 | manual | `semantically_similar_to` never emitted |
| G16 | P2 | manual | Graph is undirected — caller/callee direction lost |
| G17 | P2 | auto | 43 edges use banned `confidence_score=0.5` default — retag AMBIGUOUS |
| G18 | P3 | auto | `_src`/`_tgt` duplicate `source`/`target` on every edge — stripped |
| G19 | P2 | manual | Hyperedges exist but HTML renderer ignores them |
| G20 | P3 | manual | Provenance metadata (source_url/captured_at/author) 0/N globally |
| G21 | P3 | warn | `source_location` null on 206 nodes |
| G22 | P1 | warn | Hub dominance ratio > 2 (c12: one node has 640 edges vs 278 members) |
| G23 | P3 | warn | Edge `weight` field effectively constant (>99% = 1) |
| G24 | P3 | auto | Rationale nodes have sentence-length labels — truncated to phrase + ellipsis |

## Severity scale

- **P0** — semantic correctness broken (e.g. collisions); acting on the graph gives wrong answers
- **P1** — major blind spot or UI broken
- **P2** — missing capability the schema promised
- **P3** — cosmetic / dead field / bloat

## Fixability

- **auto** — safe post-hoc patch, applied by `repair.mjs`, writes to side-by-side copies
- **warn** — surface in report; no safe auto-fix
- **manual** — requires either re-extraction with `--mode deep` or graphify-tool changes

## What this does NOT fix

- Any gap classed `manual` — those need either `/graphify <path> --mode deep` re-extraction, or upstream graphify changes. This tool documents + surfaces them, does not pretend to fix them.
- Community ID non-permanence (G12) — mitigation would be content-hash-derived labels, which requires modifying graphify itself.
- Name-collision splitting (G1) — original chunks would be needed to reconstruct per-file nodes.

## Adding a new rule

Edit `gap-rules.mjs`. Each rule exports `{ id, title, severity, fixability, source, detect(g), apply?(g) }`. `detect` returns `null` for no gap or `{ hit: true, ... }` with evidence. `apply` (optional, for fixability='auto') mutates the graph in place.

## Permanence

These scripts live in git at `racecontrol/scripts/graphify-post/`. They survive `pip install --upgrade graphifyy` (which would wipe any site-packages patch). They run against whatever graphify produces — no coupling to internal graphify APIs.
