# G5: Cross-language edges (Python→Rust, JS→Rust bridges) never emitted

## Gap

Graphify extracts each language in isolation. A Python script that shells out to a Rust binary, or a TypeScript frontend that POSTs to a Rust axum endpoint, produces ZERO edges in the graph linking the two sides — even though the data flow is real and the failure-domain is shared.

**Evidence from racecontrol:**
- `pwa/src/app/book/multiplayer/page.tsx` POSTs to `/customer/book-multiplayer` (Rust route)
- `scripts/seed-fleet-kb.js` (Node) shells out to `cargo run --bin seed-fleet-kb` (Rust)
- `audit/phases/tier1/phase*.sh` calls `curl http://localhost:11434/api/generate` (Ollama bridge)

None of these produce a cross-language edge. Each side looks disconnected in the per-language graph.

## Proposed fix

Bridge extractor that recognizes three categories:

1. **HTTP bridge** — client-side detects `fetch("/path")` / `axios.post("/path")` / `curl <url>` and emits `calls_route` edge to the matching Rust axum `.route("/path", method)` handler.
2. **Subprocess bridge** — Node/Python detects `spawn`/`execFile`/`subprocess.run` with a binary name matching a Rust `[[bin]]` target and emits `spawns_binary` edge.
3. **FFI bridge** — Python `ctypes.cdll` / Node `ffi-napi` detects the `.dll` / `.so` name and emits `calls_ffi` edge.

Scope: accept false positives (e.g. `"/path"` that coincidentally matches an unrelated route). Confidence tier: `AMBIGUOUS` (0.5), same as `cross_repo_label_match`.

## Impact

Closes ~3-5% of the "disconnected frontend/backend" gap. Enables queries like "everything that reaches `/customer/book-multiplayer`" to surface TS + Rust + Python callers together.

## References

Roadmap Tier 3 / P2.3. Related to G16 (directed edges — cross-language edges should be directed by construction).
