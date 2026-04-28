# G16: Graph is undirected — caller/callee direction lost at extraction time

## Gap

Graphify emits `calls` edges but doesn't preserve direction. Graph.json has `{source, target}` but the extractor treats them symmetrically. Downstream `graphify query` can't answer "who calls X" vs "what does X call" — both are `neighbors(X)`.

## Proposed fix

1. Extractor: ensure `{source, target}` always encodes caller→callee, not arbitrary order. Document this invariant in the graph.json schema.
2. Renderer: add arrowheads for directed edge types (`calls`, `reads_field`, `writes_field` once G3 lands, `spawns_binary` once G5 lands, etc). Use distinct styling for undirected types (`cross_repo_label_match`, `semantically_similar_to`).
3. CLI: add `graphify query --incoming <sym>` / `--outgoing <sym>` subcommands that respect direction.

## Impact

Closes the "hollow neighbour query" gap. Meaningful for debug-with-context workflows — "who calls `end_billing_session`?" is a different question from "what does `end_billing_session` call?" and both are useful separately.

## Test plan

- Fixture: `a() { b(); }`. Expected: one edge source=a, target=b.
- Query: `query --incoming b` returns `[a]`. `query --outgoing a` returns `[b]`.
- Schema: graph.json includes `"directed": true` at edge-set level OR per-edge `"directed": true|false`.

## References

Roadmap Tier 3 / P2.6. Smallest of the 7 (~0.5 engineering days). Probably first to land; required by G3/G5/G6 to be meaningful.
