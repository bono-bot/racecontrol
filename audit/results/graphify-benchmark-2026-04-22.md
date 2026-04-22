# Graphify Benchmark — 2026-04-22

Fixture: `scripts/graphify-benchmark/fixture/` — hand-authored known-answer Rust.
graphify version: warning: skill is from graphify 0.4.20, package is 0.4.25. Run 'graphify install' to update.
Emitted: 17 nodes, 26 edges.

## Overall coverage: **85.7%** (18 / 21)

Of the 18 matched edges, **7** had reversed source/target direction (G16 gap). Content caught, direction lost.

| Category | Expected | Matched | By graphify | By extras | Dir-reversed | Coverage |
|---|---|---|---|---|---|---|
| A_function_calls | 9 | 9 | 9 | 0 | 7 | 100.0% |
| B_struct_fields | 7 | 5 | 0 | 5 | 0 | 71.4% |
| C_external_boundary | 2 | 1 | 0 | 1 | 0 | 50.0% |
| D_dynamic_dispatch | 2 | 2 | 0 | 2 | 0 | 100.0% |
| E_macro_expansion | 1 | 1 | 0 | 1 | 0 | 100.0% |

Attribution: 9 edges from pure-AST graphify, 9 from Phase 5 post-processors (static-extras + cross-process).

## Missing edges (per category)

### B_struct_fields

> Struct-field reads and writes. G3 gap — pure AST misses these.

- `main` --[writes_field]-> `Config.count`
- `main` --[writes_field]-> `Config.enabled`

### C_external_boundary

> Syscalls / subprocess / file I/O. G6 gap — needs syscall-pattern extractor.

- `spawns_subprocess` --[spawns_process]-> `node`

## Interpretation

- Overall **86%** is the measured coverage on the fixture. This is NOT the same as coverage on racecontrol — fixture is intentionally shaped to expose gap categories.
- Category A (function calls) is what pure-AST graphify is built for; expect ≥90%.
- Categories B/C/D/E are the known gaps (G3/G6/G16/G21). Low coverage here is EXPECTED until upstream PRs / P5.1 / P5.2 land.
- To measure racecontrol-scale coverage, expand the fixture + reference or author a second reference for a real-code slice.

## Replay

```bash
node scripts/graphify-benchmark/bench.mjs
```
