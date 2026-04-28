# G19: Hyperedges (n-ary relations) ignored by HTML renderer

## Gap

Current graphify HTML viz assumes binary edges (source, target). Relations that are inherently n-ary — e.g. "these 5 functions all write to the same global", "this config struct is the shared ground truth for 8 handlers" — have no rendering. The underlying graph schema could support them, but vis-network-based viz would treat them as a cluster of binary edges or drop them silently.

## Proposed fix

Pick one of:

1. **Rendering approach**: switch viz backend from vis-network to a renderer that supports hyperedges natively (e.g. GraphViz with multi-tailed edges, d3-force with hypergraph plugins). Bigger lift but cleanest.
2. **Expansion approach**: keep vis-network; emit hyperedges as a virtual node + N binary edges. Simpler, but visually noisy.

## Impact

Modest (~2%). Hyperedges are less frequent than binary edges in Rust/TS/Python codebases. The clearest use case: expanding `cross_repo_label_match` from current 2-way matches to N-way matches (if a label appears in 5 repos, current graphify emits C(5,2)=10 edges — a hyperedge would emit 1).

## Risks

- Existing users' graphs look different. Mitigation: opt-in renderer flag.
- Hypergraph layouts are harder to read. Mitigation: include toggle between hypergraph/binary-expansion.

## References

Roadmap Tier 3 / P2.7. Lowest-priority of the 7 because the gap is rendering, not extraction (a renderer gap doesn't block analysis — grep-the-graph.json still works).
