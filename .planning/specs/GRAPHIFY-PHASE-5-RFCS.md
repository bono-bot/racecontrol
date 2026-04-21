# Graphify Phase 5 RFCs — beyond the pure-AST ceiling

Status: **stub / RFC-only** (2026-04-22). Full implementation of any one of these is a 2-5 day engineering effort; they are written as scoped design notes so the user can pick which (if any) to promote. All are *beyond* what the 7 upstream graphify PRs (999.4) can close — they require an extractor *alongside* graphify, not extensions to graphify itself.

Estimated ceiling lift if all 5 shipped: +14%. In practice only a subset make sense; pick from highest-leverage down.

---

## RFC P5.1 — Runtime-trace extractor

### Problem

Dynamic dispatch invisibility. `rc-agent` spawns games via `Box<dyn GameLauncher>`; graphify sees the trait but not which concrete impl fires at runtime. Same class of gap: callback registration, trait objects, reflection, runtime plugin loading.

### Sketch

- Instrument `rc-agent` / `racecontrol` with `tokio::tracing` spans on trait-object dispatch points (`GameLauncher::launch`, `AudioProvider::play`, etc).
- Emit span data to a JSONL sink.
- Offline: parse spans, reconstruct a *runtime call graph* with `dispatched_to` edges.
- Feed into `graphify-out-unified/graph.json` as an additional edge tier (`confidence_tier: RUNTIME_OBSERVED`, confidence 1.0).

### Effort

3-5 days. Requires instrumentation PRs across rc-agent + racecontrol + a span-parser crate.

### Risks

- Runtime cost of always-on tracing. Mitigation: sample 1:100 by default.
- Span taxonomy drift: if future code doesn't emit spans at dispatch points, graph coverage silently drops. Mitigation: CI check that flags missing spans on new trait objects.

---

## RFC P5.2 — Cross-process flow extractor

### Problem

WebSocket messages, IPC, UDP packet types, file sentinels — these edges exist in Racing Point architecture but graphify has no extractor for them. `rc-sentry` → `rc-agent` via HTTP on :8091, `rc-agent` → UDP :20778 for F1-25 telemetry, `rc-watchdog` → sentinel file `C:\RacingPoint\MAINTENANCE_MODE`. None show up as edges.

### Sketch

- Parse `shared/protocol.js` (WebSocket message types) for each repo.
- Parse `*.rs`/`*.ts` for HTTP route registrations + HTTP client call sites.
- Parse bash/PowerShell for file-sentinel reads/writes.
- Emit a separate `cross-process.graph.json` with:
  - `ws_message` edges (source handler → target message type → consumer handler)
  - `http_endpoint` edges (client → route)
  - `udp_port` edges (sender → port → listener)
  - `sentinel_file` edges (writer → path → reader)
- Unify with the main graph in `unify.mjs`.

### Effort

2-3 days.

### Risks

- Parser brittleness: handwritten regex for WS types will drift. Mitigation: derive from shared protocol schemas where available.

---

## RFC P5.3 — LSP-based symbol resolution

### Problem

Overloaded names (`load()` in Rust, TS, Python, SQL) all cluster together in graphify's label-based unification. Semantic quality suffers.

### Sketch

Stand up `rust-analyzer` / `typescript-language-server` as secondary extractors for key repos. Resolve symbols to `<crate>::<path>::<name>` form. Emit as canonical IDs in the graph.

### Effort

3-5 days. LSP client libraries exist in Node (`vscode-languageclient`) and Rust (`lsp-types`).

### Risks

- LSP is heavyweight; ramp-up time per extraction is high.
- Language coverage uneven (Rust + TS have good LSPs; bash/PowerShell don't).

---

## RFC P5.4 — Cross-rebuild benchmarking CLI

### Problem

Roadmap estimates utilization as "~55% → ~60%" etc — hand-wavy percentages with no ground truth. Need actual measurement.

### Sketch

- Author a small reference graph (~50 nodes, 80 edges) of a known codebase slice by hand, counting every edge a perfect extractor would emit.
- Run graphify on the same slice.
- Compute coverage: `edges_emitted / edges_expected` per edge type.
- Report per-rule coverage + overall.
- Run quarterly; track drift.

### Effort

2 days instrumentation + 1 day to hand-author the first reference graph.

### Risks

- Low. The reference graph is small and bounded.

---

## RFC P5.5 — Incremental/delta extraction

### Problem

Today, each graphify rebuild re-parses the whole repo. Racecontrol = ~2230 files; rebuild is ~40s. Slows down post-commit hook and invites skipping extraction.

### Sketch

- Track `file → sha256` index in `graphify-out/.graphify/file-hashes.json`.
- On update: only re-extract files whose sha changed.
- Merge deltas into the existing graph.json + recompute communities only if edge set changed ≥ N%.

### Effort

2 days.

### Risks

- Communities are global; "only recompute if ≥N% changed" is a heuristic. Mitigation: always recompute when user passes `--full`.

---

## Priority ordering (opinion)

If picking 1: **P5.4 (benchmarking)** — without measurement, every other Phase 5 investment is a guess.
If picking 2: add **P5.5 (incremental)** — biggest day-to-day quality-of-life improvement.
If picking 3: add **P5.2 (cross-process)** — highest coverage lift of the remaining three.

P5.1 and P5.3 require buy-in beyond graphify (instrumenting main codebase or standing up LSPs) and should only fire if P5.4 shows a ≥5%-addressable gap in their class.
