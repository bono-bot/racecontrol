# G3: Rust extractor missing struct-field edges (reads_field / writes_field)

## Gap

Current Rust AST extractor emits edges for function calls (`a.foo()`, `foo()`) but **not** struct-field reads/writes (`a.bar`, `a.bar.baz`). This leaves data-carrier structs with `degree=1` (only their constructor is edged in) even though dozens of sites touch their fields.

**Evidence from racecontrol:** `AcLaunchParams` struct has 12 fields read across 6 call sites — graph shows `degree=1`.

## Proposed fix

Extend `syn::visit::Visit` (or equivalent walker) to handle `Expr::Field` (`a.field`) and `ExprMethodCall` receivers. Emit:
- `reads_field` edge when the field appears in a read context (right-hand side, argument, return)
- `writes_field` edge when the field appears in an assignment LHS

Both edges carry the enclosing function node and the owning struct's field node.

## Impact

Closes ~5% of the "graph undirected / hollow struct" gap class. Would promote G3 from `manual` to automatic in the gap rule set. Improves `community` detection for struct-heavy crates (racecontrol has ~400 struct types).

## Test plan

1. Small fixture: 1 struct with 3 fields, 2 functions reading them, 1 writing one.
2. Expected: 2× `reads_field` edges + 1× `writes_field` edge.
3. Regression: existing `calls` edges unchanged.

## References

- Racing Point eSports graphify utilization roadmap (`project_graphify_utilization_roadmap.md`) Tier 3 / P2.2
- Related: G16 (directed edges) — landing G3 without G16 produces undirected `reads_field` which is less useful
