# G15: `semantically_similar_to` edges never emitted

## Gap

Graphify gap rules list `semantically_similar_to` as an edge type, but the extractor never emits it. That's the hardest to fix because it needs embeddings, not AST.

## Proposed fix

Optional embeddings pipeline behind a feature flag (so default graphify stays pure-AST, fast, no external deps):

1. Add `graphify embed <repo>` subcommand that walks source files, chunks at function boundaries, encodes via `sentence-transformers/all-MiniLM-L6-v2` (or similar compact CPU-friendly model), writes `embeddings.parquet`.
2. Add `graphify similarity <repo> --threshold 0.85` that reads the parquet + emits `semantically_similar_to` edges where cosine ≥ threshold AND the two nodes are in different files.
3. Make both optional — default graph build unaffected unless user opts in.

## Impact

Closes ~5% of the "duplicate-semantics invisible" gap class. Useful for dedup analysis ("we have 3 functions doing the same thing in 3 crates"). Racing Point has at least 4 known instances (`load_config` in 4 places, `format_timestamp` in 3, etc) that pure-AST doesn't flag as related.

## Risks

- **Cost**: embeddings a 10k-node repo is ~30s on CPU, manageable.
- **Model lock-in**: picking a specific ST model bakes in assumptions. Mitigation: document the model + version in graph metadata.
- **False positives**: similar-ish but unrelated code gets linked. Mitigation: high threshold default (0.85+), tunable.

## References

Roadmap Tier 3 / P2.5. Highest-effort of the 7 gaps (~1.5 engineering days). Probably the last to land.
