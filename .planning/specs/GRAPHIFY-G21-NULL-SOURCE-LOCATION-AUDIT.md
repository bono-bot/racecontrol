# G21 null source_location — audit finding (2026-04-22)

## Hypothesis (from roadmap)

> G21 — source_location null on 206 racecontrol nodes. Audit which extractor path produces null — likely macros or generated code.

## Finding

Hypothesis **retracted**. The 206 null-source_location nodes are NOT macro/generated code. They are **meta-corpus / spine-doc** nodes injected by graphify-meta — manually-authored documentation anchors describing architecture paths, not actual source symbols.

### Classification of the 206

- 100% have `degree: 0` (no incoming or outgoing edges)
- Labels are plain-English descriptions: "Game Launch E2E Map (spine doc)", "kiosk staff page.tsx", "RaceControl.py AC plugin"
- None match Rust path syntax (`::`), method-access syntax (`.x`), or function-call syntax (`fn()`)
- None appear in the actual source tree — they're references to files/concepts, not to the files themselves

### Root cause

`scripts/graphify-meta/` (parallel sub-module to graphify-post) injects these for the *meta graph* view — the 30,000-foot "here's the architecture" narrative that sits alongside the source-extracted graph. graphify-meta isn't bound by AST extraction rules and legitimately has no source_location for its conceptual anchors.

## Recommendation

**No code change.** The 206 are behaving as designed. Two alternatives:

1. **Filter in unify.mjs** — exclude `source_location: null && degree: 0` nodes from cross-repo matching (they won't match textually anyway, but this formalizes the rule). Cost: ~5 lines.
2. **Populate source_location with spec-file paths** — point "Game Launch E2E Map" to `.planning/specs/GAME-LAUNCH-E2E.md` or similar. Cost: ~1 hour for a grep+map pass.

Both are optional polish. The 206 are not "broken" nodes; they are correctly-null-by-design meta anchors.

## Impact on roadmap

Closes P3.1. Originally estimated as "2-4 hours" assuming macro/generated-code root cause. Actual fix scope: 0 hours (audit only) or ~5 lines if filter chosen.

G22 (hub dominance, one node with 640 edges) is a separate and real issue; it should NOT be retracted on the same grounds.

## Commands to reproduce

```bash
cd C:/Users/bono/racingpoint/racecontrol
node -e "
const g = JSON.parse(require('fs').readFileSync('graphify-out/graph.json'));
const nulls = g.nodes.filter(n => !n.source_location);
console.log('null:', nulls.length, '/', g.nodes.length);
console.log('degree-0:', nulls.filter(n => (n.outgoing||[]).length + (n.incoming||[]).length === 0).length);
"
```
