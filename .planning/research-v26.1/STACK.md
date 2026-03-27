# Stack Research

**Domain:** Event-driven mesh architecture — sim racing venue (v24.0 Meshed Intelligence)
**Researched:** 2026-03-27
**Confidence:** MEDIUM-HIGH (versions verified via web search; Windows-specific behaviors flagged where uncertain)

---

## What This Covers

Stack additions only. Existing capabilities (Rust agents, Node.js/Next.js, SQLite, PostgreSQL, go2rtc, Tailscale, bash tooling) are already validated and not re-researched here.

---

## Core Technologies

### Event Backbone

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| NATS Server | 2.12.x (latest: v2.12.6, 2026-03-24) | Message broker + JetStream persistence | Single binary, runs as Windows service via sc.exe, JetStream built-in (no separate install), 3-model council consensus. Note: v2.12.6 has a regression in clustered JetStream consumer updates — use single-node mode (not affected). |
| async-nats | 0.46.0 | Rust async NATS client for pod agents | Official nats-io crate, full JetStream API, tokio-native async, 0.x but API-stable. MSRV ~1.79 (from 0.44 line, verify for 0.46). |
| natscli | Latest (match nats-server) | CLI for stream/consumer management, debugging | Essential ops tool — create streams, replay events, tail subjects. Windows binary available. |

### mDNS / Peer Discovery

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| mdns-sd | 0.13.11 (2025-07-08) | mDNS-SD service discovery — publish and query | Pure Rust, actively maintained, supports both responder and querier roles, cross-platform including Windows. See Windows caveat below. |
| swarm-discovery | 0.4.1 | Swarm-level peer discovery with bandwidth bounding | Wraps mDNS with bounded-bandwidth protocol — ideal for 8-pod swarm where we don't want O(n^2) query storms. Built by Roland Kuhn (Akka author). |

**Windows mDNS caveat (MEDIUM confidence):** Windows 11 mDNS has known intermittent failures (Windows randomly stops listing mDNS devices — no consistent root cause found). The mdns-sd crate itself tests against Avahi/macOS/iOS but not explicitly Windows. Mitigation: treat mDNS as best-effort discovery; fall back to static IP table (192.168.31.x) for pod addressing. Never make mDNS the only discovery path on Windows.

### ONNX / Camera AI Inference

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| ort | 2.0.0-rc.12 (2026-03-05) | ONNX Runtime Rust bindings for camera AI inference | Wraps Microsoft ONNX Runtime 1.24, GPU acceleration via CUDA/DirectML on RTX 4070, Windows native. Still RC but production-used. Better than `tract` for GPU-accelerated camera inference. |
| tract-onnx | 0.22.x | Pure-Rust CPU-only ONNX fallback | No dynamic library dependency — use for lightweight models (occupancy, anomaly) where GPU not required. ~85% ONNX backend coverage. |

**Recommendation:** Use `ort` for camera person-detection models (YOLOv8-class, runs on RTX 4070 via DirectML). Use `tract-onnx` for the fusion service's confidence scoring and simple threshold models where GPU unavailable.

### Predictive Analytics / Lightweight ML

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| linfa | 0.8.1 (3 months old as of 2026-03) | Classical ML: regression, clustering, SVMs | Rust-native scikit-learn equivalent. Use for: pod failure prediction (classification), demand forecasting (linear regression), anomaly thresholds. No Python runtime needed. |
| burn | 0.13.x | Deep learning / neural nets in Rust | For more complex sequence models if linfa insufficient. Supports ONNX import. Windows native. Use only if linfa is insufficient — adds significant compile-time overhead. |

**Recommendation for v24.0:** Start with `linfa` + hand-crafted heuristics. Reserve `burn` for v25.0 if temporal patterns require sequence modeling. Do NOT pull in Python/PyTorch for inference — adds a runtime dependency on every pod.

---

## Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1.x (latest stable) | Async runtime | Already likely in rc-agent; async-nats requires tokio. Pin to same major as existing codebase. |
| serde / serde_json | 1.x | Event envelope serialization | All NATS message payloads as JSON envelopes. Already in use. |
| uuid | 1.x | Event IDs for sourcing | Every event needs a stable UUID for deduplication and replay. |
| chrono | 0.4.x | Timestamps in IST/UTC | Event timestamps — store UTC, display IST. Already likely in use. |
| tokio-stream | 0.1.x | Stream adapters for JetStream consumers | Convert JetStream pull consumers to async Stream interface. |
| futures | 0.3.x | Combinators for multi-stream fusion | Time-windowed join requires select! and merge streams. |
| tracing | 0.1.x | Structured event logging | Use for observability of NATS subscriber handlers — correlate with event IDs. |
| dashmap | 6.x | Concurrent hashmaps for blackboard state | AI agent blackboard pattern — concurrent write from multiple event handlers. |
| rusqlite | 0.31.x | SQLite spooling in degraded mode | Already on pods; use for event spool when NATS unreachable. |

---

## Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| nats-server (CLI) | Local dev server with JetStream | `nats-server -js -sd ./jetstream-data` — single command local cluster |
| natscli (`nats` binary) | Stream creation, consumer management, tail/replay | `nats stream add`, `nats sub`, `nats pub`. Must match server version. |
| nats-top | Real-time NATS server monitoring | Shows connection counts, message rates |
| Netron | ONNX model visualization | Inspect YOLOv8 / custom ONNX models before deployment |

---

## Installation

### NATS Server (Windows Service on .23 server)

```bash
# Download nats-server v2.12.x Windows amd64 from:
# https://github.com/nats-io/nats-server/releases

# Install as Windows service (run as admin)
sc.exe create nats-server binPath= "C:\nats\nats-server.exe -js -sd C:\nats\data -c C:\nats\nats.conf"
sc.exe start nats-server

# Configure JetStream in nats.conf:
# jetstream {
#   store_dir: "C:\\nats\\data"
# }
```

### Rust Cargo.toml additions

```toml
[dependencies]
async-nats     = "0.46"
mdns-sd        = "0.13"
swarm-discovery = "0.4"
ort            = { version = "2.0.0-rc.12", features = ["directml"] }  # GPU on Windows
tract-onnx     = "0.22"                                                  # CPU fallback
linfa          = "0.8"
linfa-linear   = "0.8"
linfa-trees    = "0.8"
tokio-stream   = "0.1"
dashmap        = "6"
uuid           = { version = "1", features = ["v4"] }
```

### natscli (ops tooling)

```bash
# Download nats CLI from:
# https://github.com/nats-io/natscli/releases
# Add to PATH on server and dev machine
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| async-nats 0.46 | nats crate (sync) | Never — sync nats crate is deprecated, async-nats is the official replacement |
| NATS 2.12 | Kafka | Only at 100+ node clusters with multi-team consumers — overkill here |
| NATS 2.12 | Eclipse Zenoh | If venue expands to peer-to-peer mesh without server infrastructure. Revisit in v25.0. |
| mdns-sd + swarm-discovery | libp2p-mdns | If building a general P2P network. libp2p is massive dependency for 8-pod LAN. |
| ort (DirectML) | NVIDIA CUDA direct | CUDA requires driver lockstep and adds CUDA runtime dependency. DirectML is Windows-native, runs on RTX 4070, zero extra install. |
| linfa | Python scikit-learn sidecar | Only if models require frequent retraining from live data. For v24.0 inference-only, Rust-native is correct. |
| tract-onnx | candle (Hugging Face) | If transformer-class models needed. candle is excellent but heavier. Out of scope for v24.0. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `nats` crate (sync) | Officially deprecated in favor of async-nats | `async-nats` 0.46 |
| `onnxruntime-ng` | Unmaintained wrapper, stuck at ORT 1.8 | `ort` 2.0-rc.12 |
| `libp2p` for mDNS | ~50 transitive dependencies for what we need is 3 lines of mdns-sd | `mdns-sd` + `swarm-discovery` |
| NATS Streaming Server (STAN) | Deprecated by NATS team in 2023, superseded by JetStream | NATS + JetStream (`async-nats` jetstream module) |
| Redis for event spool | Another service to install on Windows pods — operational overhead | SQLite (already on every pod) |
| Python inference on pods | Python runtime + venv management on 8 Windows pods is ops nightmare | `ort` or `tract-onnx` Rust crates |
| Docker on pods | Pods are Windows 11 gaming machines — Docker adds overhead, VT-x issues documented | Native Windows services (existing pattern) |
| Eclipse Zenoh | Not ruled out long-term but less battle-tested tooling than NATS; 3-model council ruled it out for v24 | NATS JetStream |

---

## Stack Patterns by Variant

**For pod agents (rc-agent, rc-sentry additions):**
- Use `async-nats` with tokio runtime (already present)
- Subscribe to `pod.<id>.>` subjects for lateral awareness
- Publish health/state events to `events.pod.<id>.health`
- mDNS via `swarm-discovery` for leader election fallback

**For camera fusion service (new service, runs on server .23):**
- Rust binary using `ort` with DirectML backend
- Subscribes to `events.camera.>` from NATS
- Publishes fused events to `events.fusion.occupancy`
- Can be Node.js if Rust ONNX setup proves complex — Node.js has `onnxruntime-node` as fallback

**For degraded mode (NATS unreachable):**
- `rusqlite` append-only spool table on each pod
- Resume/replay on reconnect via JetStream `StartSequence` or `StartTime` consumer options

**For predictive analytics (runs on server or Bono VPS):**
- `linfa` for classical models: linear regression (demand), isolation forest (anomaly), decision tree (failure prediction)
- Models trained offline from historical SQLite/PostgreSQL data, serialized as ONNX or native linfa model files
- Loaded at startup, inference in-process

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| async-nats 0.46 | NATS Server 2.10+ | Use 2.12.x server for latest JetStream features. Server must be same major or newer than client expectations. |
| async-nats 0.46 | tokio 1.x | Requires tokio, no sync support. Full tokio feature needed (`features = ["full"]` or at minimum `rt-multi-thread`, `net`). |
| ort 2.0-rc.12 | ONNX Runtime 1.24 | Bundles ORT dynamically or statically. Use `features = ["directml"]` for Windows GPU. Requires ONNX Runtime DLL unless static feature used. |
| linfa 0.8.x | ndarray 0.15.x | linfa-* subcrates must all be same 0.8.x version to avoid ndarray version conflicts. Pin all to `= "0.8"` not `"^0.8"`. |
| swarm-discovery 0.4 | mdns-sd 0.13 | swarm-discovery depends on mdns-sd internally — do not add mdns-sd separately unless you need direct low-level access; it will be pulled transitively. Verify Cargo.lock for version alignment. |
| rustc 1.93.1 (venue) | async-nats 0.46 | MSRV for 0.44 line is ~1.79 — 1.93.1 is well above this. No upgrade needed. |

---

## Integration Points With Existing Codebase

| Existing Component | Integration Approach |
|-------------------|---------------------|
| rc-agent (pod Rust binary) | Add `async-nats` dependency, spawn a NATS publisher task alongside existing WebSocket health loop. NATS publishes are fire-and-forget; WebSocket health probe continues as fallback. |
| rc-sentry (pod AI binary) | Subscribe to `events.pod.>` for lateral awareness. Publish recovery events to NATS. |
| racecontrol (server) | Add NATS subscriber for aggregation — replaces/augments current WebSocket hub. Existing WS clients remain unchanged. |
| comms-link (James/Bono) | Route AI coordination messages through NATS `agents.>` subject hierarchy instead of WS-only. |
| SQLite (pods) | Add `events_spool` table for offline buffering. Schema: `(id TEXT, subject TEXT, payload BLOB, created_at INTEGER, synced INTEGER)`. |

---

## Sources

- [NATS Server releases — GitHub](https://github.com/nats-io/nats-server/releases) — v2.12.6 confirmed latest (2026-03-24), Windows binary available, MEDIUM confidence
- [NATS Windows Service docs](https://docs.nats.io/running-a-nats-service/introduction/windows_srv) — sc.exe install pattern confirmed, HIGH confidence
- [async-nats — crates.io](https://crates.io/crates/async-nats) — v0.46.0 confirmed latest, HIGH confidence
- [async-nats docs.rs](https://docs.rs/async-nats/0.46.0/async_nats/) — JetStream API documented, HIGH confidence
- [swarm-discovery — crates.io](https://crates.io/crates/swarm-discovery) — v0.4.1 confirmed latest (~2 months ago), MEDIUM confidence
- [mdns-sd — crates.io](https://crates.io/crates/mdns-sd) — v0.13.11 confirmed latest (2025-07-08), MEDIUM confidence
- [mdns-sd GitHub issues](https://github.com/keepsimple1/mdns-sd/issues/360) — Active development confirmed 2025, MEDIUM confidence
- [Windows 11 mDNS issues](https://github.com/Hierosoft/mdnscheckup) — Intermittent failures documented, HIGH confidence for caveat
- [ort crate — docs.rs](https://docs.rs/ort) — v2.0.0-rc.12 (2026-03-05), wraps ORT 1.24, MEDIUM confidence (RC status)
- [ort — pykeio GitHub](https://github.com/pykeio/ort) — DirectML backend for Windows confirmed, MEDIUM confidence
- [tract-onnx — crates.io](https://crates.io/crates/tract-onnx) — v0.22.x confirmed, pure Rust, MEDIUM confidence
- [linfa — crates.io](https://crates.io/crates/linfa) — v0.8.1 confirmed, 3 months old, HIGH confidence for stability
- [NATS by Example — Rust](https://natsbyexample.com/examples/jetstream/pull-consumer/rust) — JetStream consumer patterns verified, HIGH confidence

---

*Stack research for: v24.0 Meshed Intelligence — event-driven mesh, Racing Point eSports*
*Researched: 2026-03-27*
