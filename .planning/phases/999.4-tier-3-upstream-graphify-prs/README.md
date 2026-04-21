# 999.4 — Tier 3 upstream graphify PRs

Status: **deferred** (each PR is ~1 day engineering + indefinite maintainer review). Backlog opened 2026-04-22.

## Context

Graphify v0.4.25 (pipx) extracts AST-level edges reasonably well but is structurally incomplete in seven areas that matter for Racing Point. Roadmap item P2 (`project_graphify_utilization_roadmap.md` Tier 3) covers these as *upstream* changes because `scripts/graphify-post/` can only compensate for a slice of the gap (e.g. cross-repo label matching — the `unify.mjs` approach — which post-processes but doesn't read additional extraction).

Fork is at https://github.com/james-racingpoint/graphify (done 2026-04-22 P2.1).

## The 7 gaps

Each has its own draft issue markdown in this dir: `g3-struct-field.md`, `g5-cross-language.md`, etc.

| ID  | Gap                                    | Effort | Blocker for        |
|-----|----------------------------------------|--------|--------------------|
| G3  | struct-field edges                     | ~1 day | data-carrier nodes |
| G5  | cross-language edges (Py→Rust bridges) | ~1 day | ecosystem view     |
| G6  | external-boundary edges (syscalls)     | ~1 day | deploy-graph       |
| G15 | semantic-similarity edges              | ~1.5 day | fuzzy-dedup     |
| G16 | directed edges (caller/callee)         | ~0.5 day | path queries    |
| G19 | hyperedge renderer                     | ~1 day | n-ary relations    |
| G20 | provenance metadata                    | ~0.5 day | audit trails    |

Total: ~6 engineering days across 7 PRs. Each standalone; order doesn't matter.

## Retire when

All 7 merged in `safishamsi/graphify` ≥ 0.5.x; `pipx upgrade graphifyy` picks them up; `unify.mjs` + post-processors simplify accordingly.

## 6-month timer

Opened 2026-04-22. If still unpromoted by 2026-10-22, review and either:
- close as "not pursuing upstream — forked patches live on fork only"
- promote to an active phase and schedule work

## Next actions (user-controlled)

1. Review the 7 draft issue bodies in this dir.
2. Decide: post on upstream (social cost, maintainer engagement), post on fork (no social cost, tracks locally), or neither.
3. Pick ≥1 to actually implement and PR upstream when quarter-capacity allows.
