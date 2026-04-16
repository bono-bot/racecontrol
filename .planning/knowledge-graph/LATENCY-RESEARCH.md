# Latency Optimization Research — RaceControl Stack

**Researched:** 2026-04-16
**Domain:** Full-stack latency reduction (Rust/Axum server, WebSocket pods, SQLite, Next.js frontends)
**Confidence:** HIGH (codebase-verified + official docs)

## Summary

RaceControl's latency profile spans four boundaries: (1) frontend HTTP to server, (2) server processing + DB queries, (3) server WS message to pod agent, (4) pod agent process spawn on Windows. The system already has several solid foundations in place — WAL mode with `synchronous=NORMAL`, 10-connection pool, TraceLayer with latency logging. The biggest gains will come from areas currently un-optimized: HTTP response compression (not enabled), SQLite tuning (missing `mmap_size`, `cache_size`, `temp_store`), and frontend data fetching patterns (no SWR/caching layer). Process spawn latency on Windows is largely fixed-cost and hard to improve without architecture changes.

**Primary recommendation:** Enable tower-http `CompressionLayer` (1 line, ~60-80% response size reduction for JSON), add SQLite `mmap_size` + `cache_size` + `temp_store` PRAGMAs (3 lines, ~20-40% read speedup), and add SWR to frontend API calls (library + pattern change, eliminates perceived latency for repeat queries).

---

## 1. Rust/Axum Server Performance

### 1A. HTTP Response Compression (HIGH PRIORITY)

| Property | Value |
|----------|-------|
| What | `tower_http::compression::CompressionLayer` |
| Type | Cargo feature flag + 1 line of code |
| Current state | **NOT ENABLED** — tower-http 0.6 is in Cargo.toml with `features = ["cors", "fs", "trace"]` but `compression-*` features are absent |
| Expected improvement | 60-80% reduction in JSON response body size; ~10-30ms savings on large payloads (fleet health, leaderboard, catalog) over LAN; larger savings for cloud/mobile clients |
| Complexity | LOW — add feature flags + one `.layer()` call |
| Risk | Negligible. Compression skips WebSocket upgrades and small bodies automatically. CPU cost is trivial for an 8-pod venue. |

**Implementation:**

```toml
# Cargo.toml — add compression features to tower-http
tower-http = { version = "0.6", features = ["cors", "fs", "trace", "compression-gzip", "compression-br"] }
```

```rust
// main.rs — add after CorsLayer, before TraceLayer
use tower_http::compression::CompressionLayer;

.layer(CompressionLayer::new())  // auto-selects gzip/brotli based on Accept-Encoding
```

[VERIFIED: tower-http 0.6.8 docs — CompressionLayer supports gzip, brotli, deflate, zstd]
[VERIFIED: codebase grep — no CompressionLayer in current code]

### 1B. Request ID / Timing Headers

| Property | Value |
|----------|-------|
| What | `tower_http::request_id::SetRequestIdLayer` + `PropagateRequestIdLayer` |
| Type | Tower middleware |
| Current state | Correlation ID is generated in TraceLayer span but NOT returned in response headers |
| Expected improvement | No latency reduction directly — enables debugging WHERE latency occurs |
| Complexity | LOW |

Already partially implemented: the `TraceLayer` generates `correlation_id` and logs `latency_ms`. Consider adding `X-Request-Id` and `Server-Timing` response headers to surface latency to frontend devtools.

[VERIFIED: codebase — main.rs lines 413-437 show TraceLayer with correlation_id and latency logging]

### 1C. Axum Version

| Property | Value |
|----------|-------|
| Current | axum 0.8 |
| Latest | axum 0.8.9 [VERIFIED: `cargo search axum`] |
| Impact | Patch versions contain bug fixes but no major perf changes |
| Action | Update to 0.8.9 for latest fixes. Axum 0.9 is in development but not released. |

### 1D. tower_governor Rate Limiting

Already present (`tower_governor = "0.8"`, `governor = "0.10"`). Rate limiting has minimal latency impact on a venue-scale system. No changes needed.

[VERIFIED: Cargo.toml]

---

## 2. SQLite Performance

### 2A. Current Configuration (VERIFIED)

```rust
// db/mod.rs — current PRAGMA settings
PRAGMA foreign_keys=ON     // per-connection via after_connect
PRAGMA journal_mode=WAL    // per-connection + global verification
PRAGMA busy_timeout=5000   // 5s retry on SQLITE_BUSY
PRAGMA synchronous=NORMAL  // relaxed fsync (WAL-safe)
```

Pool: `max_connections(10)`, `max_lifetime(300s)`.

[VERIFIED: db/mod.rs lines 43-54]

### 2B. Missing PRAGMAs (MEDIUM PRIORITY)

| PRAGMA | Recommended Value | Effect | Confidence |
|--------|-------------------|--------|------------|
| `cache_size` | `-32000` (32MB) | Keeps 32MB of DB pages in process memory. Default is ~2MB. Reduces disk reads for repeated queries (fleet health, billing, leaderboard). Server has 64GB RAM — 32MB is negligible. | HIGH [CITED: sqlite.org/pragma.html, cj.rs/blog/sqlite-pragma-cheatsheet] |
| `mmap_size` | `268435456` (256MB) | Memory-maps DB file for reads. Bypasses read() syscalls entirely. The racecontrol.db is likely <100MB, so the entire DB fits in mmap. Reduces read latency by 10-40% for random access patterns. | HIGH [CITED: sqlite.org/pragma.html, oldmoe.blog/2024/02/03/turn-on-mmap-support] |
| `temp_store` | `memory` | Stores temp tables/indices in RAM instead of disk. Helps complex JOINs and ORDER BY. | HIGH [CITED: sqlite.org/pragma.html] |

**Implementation — add to after_connect hook:**

```rust
.after_connect(|conn, _meta| Box::pin(async move {
    sqlx::query("PRAGMA foreign_keys=ON").execute(&mut *conn).await?;
    sqlx::query("PRAGMA journal_mode=WAL").execute(&mut *conn).await?;
    sqlx::query("PRAGMA busy_timeout=5000").execute(&mut *conn).await?;
    sqlx::query("PRAGMA synchronous=NORMAL").execute(&mut *conn).await?;
    // NEW: Performance PRAGMAs
    sqlx::query("PRAGMA cache_size=-32000").execute(&mut *conn).await?;   // 32MB page cache
    sqlx::query("PRAGMA mmap_size=268435456").execute(&mut *conn).await?; // 256MB mmap
    sqlx::query("PRAGMA temp_store=memory").execute(&mut *conn).await?;   // temp tables in RAM
    Ok(())
}))
```

**Risk:** `mmap_size` on Windows has been stable since SQLite 3.26. The 64GB-RAM server can easily afford 256MB. No data integrity risk — mmap is read-only in WAL mode.

### 2C. sqlx Statement Cache

sqlx caches prepared statements per connection with a default capacity of 100. The RaceControl codebase uses `sqlx::query()` / `sqlx::query_as()` throughout, which means statements are cached by default. At venue scale (8 pods, ~50 unique queries), the default 100-capacity is sufficient.

[VERIFIED: sqlx docs — SqliteConnectOptions::statement_cache_capacity defaults to 100]

**No change needed** unless profiling shows cache eviction.

### 2D. Connection Pool Sizing

Current: `max_connections(10)`. For 8 pods + dashboard + POS + admin + cloud sync, this is adequate. SQLite serializes writes anyway — more connections only help concurrent reads. The `max_lifetime(300s)` is reasonable to prevent connection staleness.

[VERIFIED: db/mod.rs lines 43-45]

---

## 3. WebSocket Optimization

### 3A. Binary Protocol (MessagePack)

| Property | Value |
|----------|-------|
| What | Replace JSON serialization with MessagePack for WS messages |
| Crate | `rmp-serde` (0.15.x) — pure Rust MessagePack via serde |
| Expected improvement | 2-3x faster serialization, 30-50% smaller messages [CITED: aeshirey.github.io benchmark, rust serialization benchmarks] |
| Complexity | HIGH — requires changes to BOTH server and agent WS handlers, plus all message type serialization |
| Risk | Debugging harder (binary vs readable JSON). Breaks any external WS consumers. Dashboard WS also affected. |

**Recommendation: DO NOT implement.** The WS messages between server and 8 pods are small JSON objects (<1KB). At venue scale on a LAN (sub-1ms network latency), the difference between JSON and MessagePack serialization is microseconds. The debugging cost outweighs the gain. JSON's human-readability is critical for the breadcrumb/diagnostic system.

If latency profiling later shows serialization is a bottleneck (unlikely), consider it then.

### 3B. WebSocket Compression (permessage-deflate)

| Property | Value |
|----------|-------|
| What | WS frame-level compression via `permessage-deflate` extension |
| Crate | `tokio-tungstenite` supports it via `tungstenite` config |
| Expected improvement | 50-70% WS message size reduction |
| Complexity | MEDIUM — config change on both client and server WS setup |
| Risk | CPU overhead per frame. Not worth it on LAN with small messages. |

**Recommendation: SKIP for LAN.** Only consider for cloud WS connections (Bono relay). LAN WS messages are already sub-1KB and arrive in <1ms.

### 3C. Heartbeat Tuning

Current heartbeat interval should be checked — if it's too aggressive, it adds unnecessary WS traffic. At 8 pods, even aggressive heartbeats are negligible. No change needed unless profiling shows heartbeat interference.

---

## 4. Windows Process Launch Optimization

### 4A. Current Launch Flow (VERIFIED)

The AC launch path (ac_launcher.rs) has significant inherent latency:

1. **Pre-launch checks** (~2-8s): Kill orphan processes (up to 3 retry rounds with 2s sleeps), disk space check, sysinfo refresh
2. **Config bootstrap** (~100ms): Verify/create AC config files
3. **Kill existing AC** (~0-8s): `taskkill /IM acs.exe /F` + poll for exit (max 5s + 3s retry)
4. **Write config files** (~10ms): race.ini, assists.ini, apps preset, controls.ini FFB reset
5. **Spawn game** (~100-500ms): `launch-ac.bat` or direct `acs.exe` spawn
6. **AC loading** (~10-30s): Game's own startup (loading track, cars, shaders)

Steps 1-5 total ~3-15s. Step 6 is game-internal and cannot be reduced.

[VERIFIED: ac_launcher.rs lines 437-700+]

### 4B. Optimization: Parallel Pre-Launch Checks

The `pre_launch_checks()` function in game_process.rs runs sequentially:
1. Sentinel file checks (instant)
2. Orphan process scan + kill (2-8s with sysinfo + sleep)
3. Disk space check (~100ms with sysinfo)

The orphan kill loop has hardcoded `sleep(Duration::from_secs(2))` between attempts. This is conservative — for the common case (no orphans), the full scan takes ~200ms. The 2s sleep only triggers when orphans exist.

**Optimization opportunity:** The `sysinfo::System::new()` + `refresh_processes()` call is expensive (~150-300ms on Windows). If pre-launch checks run frequently, consider caching the System instance.

[VERIFIED: game_process.rs lines 82-133]

### 4C. Optimization: Skip Redundant taskkill

If the server knows no game is running on the pod (GameTracker state is Idle), the agent could skip the "kill existing AC" step entirely. Currently, `launch_ac()` always runs `taskkill /IM acs.exe /F` + waits for exit, even on a clean pod.

**Expected improvement:** Save 1-3s on clean launches (most common case).
**Complexity:** LOW — add a `skip_kill` flag to launch params based on GameTracker state.
**Risk:** LOW — the orphan check in `pre_launch_checks()` still catches stale processes.

### 4D. Optimization: Pre-warm Config Files

The `bootstrap_ac_config()` + `write_race_ini()` + `write_assists_ini()` sequence writes files on every launch. These could be pre-generated when the user selects their configuration in the kiosk, sent alongside the launch command, and written in parallel with the kill step.

**Expected improvement:** ~50-100ms (minor — file writes are fast on SSD).
**Complexity:** MEDIUM — requires kiosk-to-agent config pre-push protocol.
**Recommendation:** Not worth the complexity for ~100ms savings.

### 4E. Steam Pre-Warming (CONDITIONAL)

For Steam-based games, `steam_checks.rs` verifies Steam is running before launch. If Steam isn't running, it starts it — adding 5-15s. For AC specifically (the primary game), Steam is typically already running because the pods boot with Steam autostart.

**Recommendation:** Ensure Steam is in autostart on all pods (operational, not code). Already handled by pod setup. No code change needed.

[VERIFIED: steam_checks.rs exists and handles Steam readiness]

---

## 5. Frontend Optimization (Next.js)

### 5A. SWR for API Data Fetching (HIGH PRIORITY)

| Property | Value |
|----------|-------|
| What | `swr` npm package — React hooks for data fetching with stale-while-revalidate |
| Version | swr@2.x (latest stable) [ASSUMED — need to verify exact version] |
| Expected improvement | Eliminates perceived latency for repeat data fetches. First load unchanged, subsequent loads instant from cache with background revalidation. |
| Complexity | MEDIUM — replace `fetch()` calls with `useSWR()` hooks across kiosk/dashboard/POS |
| Risk | LOW — SWR is from Vercel (Next.js team), production-proven |

**Key API calls that benefit from SWR:**

| Endpoint | Current Pattern | SWR Benefit |
|----------|----------------|-------------|
| `/api/v1/fleet/health` | Polled every N seconds | Instant render from cache, background refresh |
| `/api/v1/games/catalog` | Fetched on page load | Cached across navigation |
| `/api/v1/customer/search` | Fetched per keystroke | Debounced + cached results |
| `/api/v1/billing/active` | Polled for live timers | Instant initial render |

**Implementation pattern:**

```typescript
import useSWR from 'swr';

const fetcher = (url: string) => fetch(url).then(r => r.json());

function FleetHealth() {
  const { data, error, isLoading } = useSWR('/api/v1/fleet/health', fetcher, {
    refreshInterval: 5000,  // auto-refresh every 5s
    revalidateOnFocus: true,
    dedupingInterval: 2000,  // dedup rapid calls
  });
  // data available instantly on re-mount from cache
}
```

[CITED: nextjs.org/docs, swr docs]

### 5B. Response Compression (from server side)

Covered in Section 1A — enabling `CompressionLayer` on the server benefits all frontend clients automatically. Next.js proxy (`/api/rc/*` in dashboard) will pass through compressed responses if the browser sends `Accept-Encoding: gzip`.

### 5C. Next.js Bundle Optimization

These are generally already handled by Next.js 16 defaults:
- Code splitting: automatic per-page
- Tree shaking: automatic in production builds
- Image optimization: `next/image` with lazy loading
- Font optimization: `next/font` (if used)

**Quick win:** Verify that `next.config.ts` has `output: 'standalone'` (already in use for deploys). No additional config needed.

### 5D. API Proxy Latency

The dashboard uses `/api/rc/[...path]` proxy to RaceControl. This adds a hop:

```
Browser -> Next.js :3400 -> RaceControl :8080
```

For latency-critical dashboard operations, consider direct API calls to `:8080` (CORS is already configured for LAN origins). This eliminates one proxy hop (~5-15ms).

**Risk:** Requires CORS setup (already done) and exposing the API port to browsers (already done — kiosk hits :8080 directly).

---

## 6. Monitoring and Profiling

### 6A. Current Instrumentation (VERIFIED)

The server already has solid latency tracking via `TraceLayer`:

```rust
.on_response(|response, latency: Duration, _span| {
    tracing::info!(
        target: "admin_api",
        status = response.status().as_u16(),
        latency_ms = latency.as_millis() as u64,
        "request_completed"
    );
})
```

This logs `latency_ms` for every HTTP request. Combined with the JSONL rolling log, you can grep for slow requests:

```bash
# Find requests slower than 100ms
grep '"latency_ms":' racecontrol-*.jsonl | jq 'select(.latency_ms > 100)'
```

[VERIFIED: main.rs lines 430-437]

### 6B. Missing: Structured Latency Histogram

| Property | Value |
|----------|-------|
| What | `axum-prometheus` crate — exposes Prometheus histograms per route |
| Version | 0.8.x [ASSUMED] |
| Expected improvement | No latency reduction — but provides p50/p95/p99 breakdown by endpoint |
| Complexity | LOW — add crate + one `.layer()` call + `/metrics` endpoint |
| Risk | LOW — read-only metrics collection |

**Alternative: simple in-process histograms.** For an 8-pod venue, a full Prometheus stack may be overkill. A simpler approach:

```rust
// Track latency histograms in-memory, expose via /api/v1/debug/latency
struct LatencyTracker {
    histograms: DashMap<String, Vec<Duration>>,
}
```

**Recommendation:** Start with the existing TraceLayer logs. Add Prometheus only if you need ongoing dashboards.

### 6C. tokio-console (Development Only)

| Property | Value |
|----------|-------|
| What | `tokio-console` — real-time async task inspector |
| Crate | `console-subscriber` (add to dev-dependencies) |
| Purpose | Visualize task scheduling, poll times, waker behavior |
| When to use | During development to find tasks that block the runtime |
| Complexity | LOW for dev, NOT for production (requires `--cfg tokio_unstable`) |

[CITED: tokio.rs/tokio/topics/tracing-next-steps]

### 6D. Flamegraph Profiling

| Property | Value |
|----------|-------|
| What | `cargo-flamegraph` — generates CPU flame graphs |
| Install | `cargo install flamegraph` |
| Purpose | Find hot code paths, identify where CPU time goes |
| When to use | One-time profiling sessions, not ongoing monitoring |

[CITED: markaicode.com/profiling-applications-2025]

### 6E. Endpoint-Level Timing (Quick Win)

Add `Server-Timing` headers to responses so browser DevTools shows server processing time:

```rust
// Custom middleware
async fn server_timing(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let mut resp = next.run(req).await;
    let elapsed = start.elapsed();
    resp.headers_mut().insert(
        "Server-Timing",
        HeaderValue::from_str(&format!("server;dur={}", elapsed.as_millis())).unwrap(),
    );
    resp
}
```

This is visible in Chrome DevTools Network tab under "Timing" — zero setup on the frontend side.

---

## 7. Architecture-Level Optimizations

### 7A. WebSocket Command ACK (ALREADY IMPLEMENTED)

The server already has a `CommandAck` mechanism (Phase 312, WSCMD-01):

```
Agent sends CommandAck immediately on receiving LaunchGame
Server waits up to 5s for ACK before returning to API caller
```

This means the API response is gated on WS delivery confirmation, not on game launch completion. Good design — no change needed.

[VERIFIED: ws_handler.rs line 456 — "Send CommandAck immediately to confirm receipt"]

### 7B. Concurrent Session Guard (HTTP 409)

Already returns HTTP 409 for duplicate launches (Phase 366). This prevents wasted latency from redundant launch attempts.

[VERIFIED: CLAUDE.md — "POST /games/launch: returns HTTP 409"]

### 7C. Connection Keep-Alive

HTTP/1.1 keep-alive is the default for Axum/Hyper. No change needed. For the LAN environment, connections persist across requests, eliminating TCP handshake latency.

### 7D. Pre-Flight Cache on Agent

The agent could cache `sysinfo::System` and refresh it periodically (every 30s) instead of creating a new instance on each launch. `System::new()` + `refresh_processes()` takes ~150-300ms on Windows.

**Expected improvement:** Save ~200ms per launch for the common path.
**Complexity:** LOW — store System in agent state, refresh on timer.
**Risk:** Process list may be up to 30s stale — acceptable for orphan detection.

---

## Priority-Ordered Action Plan

| Priority | Action | Expected Gain | Effort | Type |
|----------|--------|---------------|--------|------|
| P1 | Enable `CompressionLayer` | 60-80% smaller API responses | 2 lines | Config change |
| P2 | Add SQLite PRAGMAs (cache_size, mmap_size, temp_store) | 20-40% faster reads | 3 lines | Config change |
| P3 | Add SWR to frontend API calls | Eliminates perceived latency on repeat loads | Medium | npm + code |
| P4 | Skip taskkill when pod is clean | Save 1-3s per clean launch | Low | Code change |
| P5 | Cache sysinfo::System in agent | Save ~200ms per launch | Low | Code change |
| P6 | Add `Server-Timing` header | Enables browser DevTools timing | Low | Code change |
| P7 | Direct API calls from dashboard (skip proxy) | Save ~5-15ms per request | Low | Config change |

### Don't Do (Not Worth the Complexity)

| Idea | Why Skip |
|------|----------|
| MessagePack WS protocol | Microsecond gains on LAN, lose JSON debuggability |
| WS permessage-deflate | Sub-1KB messages on LAN, CPU overhead not justified |
| Process pool / pre-warming | Game process is spawned once per session, not a hot path |
| Prometheus / Grafana | Overkill for 8-pod venue; existing TraceLayer logs suffice |
| HTTP/2 or HTTP/3 | LAN latency is <1ms; protocol overhead savings are negligible |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | swr@2.x is latest stable | 5A | Low — any 2.x works |
| A2 | mmap_size works reliably on Windows with sqlx 0.8 | 2B | Medium — test on server first |
| A3 | axum-prometheus is 0.8.x | 6B | Low — informational only, not recommended |

---

## Sources

### Primary (HIGH confidence)
- Codebase verification: `db/mod.rs`, `main.rs`, `ac_launcher.rs`, `game_process.rs`, `ws_handler.rs`, `Cargo.toml`
- [tower-http CompressionLayer docs](https://docs.rs/tower-http/latest/tower_http/compression/struct.CompressionLayer.html)
- [SQLite PRAGMA reference](https://sqlite.org/pragma.html)
- [sqlx SqliteConnectOptions docs](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html)
- [sqlx Statement Caching](https://deepwiki.com/launchbadge/sqlx/4.5-statement-caching)

### Secondary (MEDIUM confidence)
- [SQLite PRAGMA Cheatsheet](https://cj.rs/blog/sqlite-pragma-cheatsheet-for-performance-and-consistency/)
- [SQLite mmap support](https://oldmoe.blog/2024/02/03/turn-on-mmap-support-for-your-sqlite-connections/)
- [Rust serialization benchmarks](https://github.com/djkoloski/rust_serialization_benchmark)
- [Process spawning performance in Rust](https://kobzol.github.io/rust/2024/01/28/process-spawning-performance-in-rust.html)
- [Profiling Rust Applications 2025](https://markaicode.com/profiling-applications-2025/)
- [Next.js SWR docs](https://nextjs.org/docs/app/guides/how-revalidation-works)
- [SWR by Vercel](https://peerlist.io/jagss/articles/understanding-react-swr-how-it-works-and-why-its-awesome)

### Crate Versions (VERIFIED via cargo search)
- axum: 0.8.9 (current in project: 0.8)
- tower-http: 0.6.8 (current in project: 0.6)
- sqlx: 0.8 (current in project: 0.8)
