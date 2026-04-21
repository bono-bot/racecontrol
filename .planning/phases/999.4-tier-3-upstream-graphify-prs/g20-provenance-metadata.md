# G20: Provenance metadata (source_url, captured_at, author) 0/N globally

## Gap

Graphify extracts structural info (labels, edges, communities) but drops all provenance. For audit / compliance / triage workflows it's useful to know:
- `source_url` — the git URL the graph came from
- `captured_at` — when extraction ran (for staleness detection)
- `author` — who last touched a given function (from git blame)
- `commit` — what HEAD the graph reflects

Currently these fields are `null` for all 10,000+ nodes in `racecontrol/graphify-out/graph.json`.

## Proposed fix

At extraction time, for each node:
1. Read the file's `git log -1 --format='%H %ae %at' <file>` → populate `commit`, `author`, `committed_at`.
2. Read the repo's `git remote get-url origin` → populate `source_url`.
3. Record extraction time as `captured_at` (ISO UTC).

Add these as new fields on every node in graph.json. Keep them optional so older graphs parse; extractor populates by default.

## Impact

Closes the "where did this come from" gap class. Enables queries like "who owns this community?" and staleness checks ("nodes last touched >6mo ago"). Small (~0.5 engineering days); low-risk.

## Risks

- `git log -1 --format` per-file is slow on big repos. Mitigation: batch-walk via `git log --name-only` once + index in memory.
- Some repos aren't git repos. Mitigation: gracefully skip git-derived fields.

## References

Roadmap Tier 3 / P2.8. Lowest-effort, highest-reversibility of the 7. Good candidate for the first real PR upstream.
