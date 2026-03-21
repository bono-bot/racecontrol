# Phase 112: RTSP Infrastructure & Camera Pipeline - Research

**Researched:** 2026-03-21
**Domain:** RTSP relay infrastructure, Rust RTSP client, stream health monitoring
**Confidence:** HIGH

## Summary

This phase establishes the foundational camera pipeline for v16.0 Security Camera AI. The core architecture is a two-layer design: (1) go2rtc as a standalone RTSP relay process on James (.27), proxying Dahua camera sub-streams so multiple consumers (people tracker, rc-sentry-ai, future face recognition) can read without starving the cameras, and (2) the rc-sentry-ai Rust crate that connects to go2rtc's relayed streams via the retina crate, extracts frames at 2-5 FPS, and exposes health/status via an Axum HTTP endpoint on port 8096.

The existing people tracker at `C:\Users\bono\racingpoint\people-tracker\` is a Python/FastAPI/YOLOv8 application that connects directly to camera RTSP streams via OpenCV. Migration means changing its `config.yaml` camera IPs to point at go2rtc's RTSP relay (localhost:8554) instead of the cameras directly. The NVR at .18 maintains its own independent connections and is unaffected.

**Primary recommendation:** Use go2rtc v1.9.13 as the RTSP relay (single binary, zero dependencies, Windows native), retina 0.4.19 for Rust-side RTSP frame extraction, and create rc-sentry-ai as a new workspace crate with Axum health endpoints.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None -- all implementation choices are at Claude's discretion for this infrastructure phase.

### Claude's Discretion
All implementation choices are at Claude's discretion. Key constraints from project context:
- Dahua cameras at entrance (.8) and reception (.15, .154) with RTSP subtype=1, auth admin/Admin@123
- NVR at .18 -- do not disrupt its recording
- Existing people tracker at :8095 (YOLOv8 + FastAPI) must continue working
- New service on :8096 on James (.27)
- Research recommends go2rtc or mediamtx for RTSP relay, retina crate for Rust RTSP
- RTX 4070 available but not needed for this phase (frame extraction only)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CAM-01 | RTSP relay service prevents Dahua stream starvation with auto-reconnect | go2rtc handles relay + reconnect natively; retina handles Rust-side reconnect |
| CAM-02 | Multi-camera stream management for entrance (.8) and reception (.15/.154) cameras | go2rtc.yaml streams section defines all 3 cameras; rc-sentry-ai manages per-camera tasks |
| CAM-03 | Stream health monitoring with auto-reconnect on failure | Axum health endpoint at :8096 reports per-camera status; retina session reconnect loop |
| CAM-04 | Integration with existing YOLOv8 people tracker at :8095 | People tracker config.yaml RTSP URLs change to go2rtc relay URLs |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| go2rtc | v1.9.13 | RTSP relay/proxy | Zero-dependency single binary, Dahua-tested, used by Frigate/Home Assistant, on-demand reconnect |
| retina | 0.4.19 | Rust RTSP client | Production-proven (Moonfire NVR), async/tokio native, H.264 depacketization, digest auth |
| axum | (workspace) | HTTP health endpoint | Already used across monorepo (racecontrol, rc-agent) |
| tokio | (workspace) | Async runtime | Already used across monorepo |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde + toml | (workspace) | Config parsing | rc-sentry-ai.toml configuration |
| tracing | (workspace) | Structured logging | All crate logging |
| reqwest | 0.12 | HTTP client for go2rtc API | Health-check go2rtc at :1984, get stream stats |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| go2rtc | mediamtx | mediamtx is heavier, more features we don't need; go2rtc is simpler, lighter, Dahua-specific URL support |
| retina | ffmpeg-next / gstreamer-rs | FFmpeg/GStreamer require native libs on Windows; retina is pure Rust, no external dependencies |
| retina | opencv (cv2) | Already used by people tracker (Python); but for Rust crate, retina is native and async |

**Installation:**

go2rtc: Download `go2rtc_win64.zip` from https://github.com/AlexxIT/go2rtc/releases/tag/v1.9.13, extract `go2rtc.exe` to `C:\RacingPoint\go2rtc\`.

Rust dependencies in rc-sentry-ai/Cargo.toml:
```toml
[dependencies]
retina = "0.4"
axum = "0.7"
tokio = { workspace = true }
serde = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
rc-common = { path = "../rc-common" }
anyhow = { workspace = true }
```

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-sentry-ai/
  src/
    main.rs           # Entry point, config load, spawn tasks
    config.rs         # TOML config structs (cameras, relay, health)
    relay.rs          # go2rtc process management (start, health-check, restart)
    stream.rs         # Per-camera retina RTSP session + frame extraction loop
    health.rs         # Axum routes: GET /health, GET /cameras
    frame.rs          # Frame buffer (latest frame per camera, Arc<RwLock<>>)
  Cargo.toml
C:\RacingPoint\go2rtc\
  go2rtc.exe          # RTSP relay binary
  go2rtc.yaml         # Stream configuration
C:\RacingPoint\rc-sentry-ai.toml  # Service configuration
```

### Pattern 1: go2rtc as a Managed Subprocess
**What:** rc-sentry-ai spawns and monitors go2rtc.exe as a child process, or go2rtc runs independently via a bat file / HKLM Run key.
**When to use:** Independent process is simpler and more reliable -- go2rtc has its own reconnect logic.
**Recommendation:** Run go2rtc independently (separate start-go2rtc.bat), rc-sentry-ai just checks its health via HTTP API at :1984.

```yaml
# C:\RacingPoint\go2rtc\go2rtc.yaml
streams:
  entrance: rtsp://admin:Admin@123@192.168.31.8/cam/realmonitor?channel=1&subtype=1
  reception: rtsp://admin:Admin@123@192.168.31.15/cam/realmonitor?channel=1&subtype=1
  reception_wide: rtsp://admin:Admin@123@192.168.31.154/cam/realmonitor?channel=1&subtype=1

api:
  listen: ":1984"

rtsp:
  listen: ":8554"
```

### Pattern 2: Per-Camera Async Task with Reconnect Loop
**What:** Each camera gets a `tokio::spawn` task that connects to go2rtc's RTSP output via retina, extracts frames in a loop, and reconnects on failure.
**When to use:** Always -- this is the standard pattern for multi-camera RTSP consumption.

```rust
// Pseudocode for stream.rs
async fn camera_loop(camera: CameraConfig, frame_buf: Arc<RwLock<Option<Vec<u8>>>>) {
    loop {
        match connect_and_stream(&camera, &frame_buf).await {
            Ok(()) => tracing::info!("Stream ended normally, reconnecting"),
            Err(e) => tracing::warn!("Stream error: {e}, reconnecting in 5s"),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn connect_and_stream(camera: &CameraConfig, frame_buf: &Arc<RwLock<Option<Vec<u8>>>>) -> anyhow::Result<()> {
    let url = Url::parse(&camera.relay_url)?;
    let session = retina::client::Session::describe(url, /* options */).await?;
    let session = session.setup(/* video stream index */).await?;
    let mut session = session.play(retina::client::PlayOptions::default()).await?;

    while let Some(item) = session.next().await {
        match item? {
            CodecItem::VideoFrame(frame) => {
                // Store raw H.264 NALUs or decode to JPEG
                let mut buf = frame_buf.write().await;
                *buf = Some(frame.data().to_vec());
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Pattern 3: Axum Health Endpoint
**What:** Axum server at :8096 exposes per-camera health and overall service status.
**When to use:** Required by CAM-03.

```rust
// GET /health
{
  "service": "rc-sentry-ai",
  "status": "ok",
  "uptime_secs": 3600,
  "cameras": {
    "entrance": { "status": "connected", "last_frame_secs_ago": 0.5, "frames_total": 7200 },
    "reception": { "status": "connected", "last_frame_secs_ago": 0.3, "frames_total": 7200 },
    "reception_wide": { "status": "reconnecting", "last_frame_secs_ago": 12.0, "frames_total": 6500 }
  },
  "relay": { "status": "healthy", "api_url": "http://127.0.0.1:1984" }
}
```

### Anti-Patterns to Avoid
- **Direct camera connections from multiple consumers:** Each Dahua camera has limited concurrent RTSP connections (~4-8). go2rtc relay solves this.
- **Blocking I/O in async context:** retina is async-native. Never use `std::thread::sleep` or blocking `cv2.VideoCapture` in the Rust crate.
- **Hardcoded camera credentials:** Use TOML config, consistent with monorepo pattern.
- **Single reconnect loop for all cameras:** Each camera must have an independent task so one camera going offline doesn't block others.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RTSP relay/proxy | Custom Rust RTSP server | go2rtc | RTSP relay is complex (NAT traversal, codec negotiation, keepalive). go2rtc handles all edge cases. |
| RTSP frame extraction | Raw socket RTSP parsing | retina crate | RTP depacketization, H.264 NAL reassembly, RTSP session management are deeply complex. |
| H.264 to JPEG decode | Custom decoder | ffmpeg CLI or go2rtc snapshot API | H.264 decode requires codec libraries. go2rtc provides `/api/frame.jpeg?src=entrance` for snapshots. |
| Process watchdog for go2rtc | Custom service manager | rc-process-guard or Windows Scheduled Task with restart-on-failure | Process supervision is already solved. |

**Key insight:** The RTSP protocol is deceptively complex -- interleaved TCP, UDP with NAT punch-through, SDP negotiation, codec-specific depacketization. go2rtc and retina handle years of edge-case fixes that a custom implementation would rediscover painfully.

## Common Pitfalls

### Pitfall 1: Dahua Stream Starvation
**What goes wrong:** Multiple direct RTSP connections exhaust the camera's connection limit (typically 4-8), causing streams to drop or refuse new connections.
**Why it happens:** NVR at .18 already holds connections; people tracker holds connections; adding rc-sentry-ai would exceed the limit.
**How to avoid:** go2rtc maintains ONE connection per camera and fans out to unlimited consumers via its RTSP server at :8554.
**Warning signs:** "Connection refused" or streams freezing after adding a new consumer.

### Pitfall 2: RTSP Auth with Special Characters
**What goes wrong:** The password `Admin@123` contains `@` which conflicts with URL parsing (`rtsp://user:pass@host`).
**Why it happens:** `@` in password is ambiguous with the `@` separator between credentials and host.
**How to avoid:** URL-encode the password: `Admin%40123`. go2rtc handles this in its YAML config natively. For retina, encode in the URL.
**Warning signs:** "Unauthorized" errors or connection failures despite correct credentials.

### Pitfall 3: retina VideoFrame is NOT Decoded Pixels
**What goes wrong:** Treating `VideoFrame.data()` as raw RGB/JPEG -- it contains H.264 NAL units (encoded bitstream).
**Why it happens:** retina does depacketization (RTP to NAL), not decoding (NAL to pixels).
**How to avoid:** For this phase, store raw NALs. For JPEG snapshots, use go2rtc's `/api/frame.jpeg` endpoint instead. Future phases (face detection) will decode via ort/ONNX Runtime.
**Warning signs:** Corrupted images, garbage pixel data.

### Pitfall 4: Windows Firewall Blocking go2rtc Ports
**What goes wrong:** go2rtc starts but nothing can connect to :8554 or :1984.
**Why it happens:** Windows Firewall blocks new listeners by default.
**How to avoid:** Add firewall rule: `netsh advfirewall firewall add rule name="go2rtc" dir=in action=allow program="C:\RacingPoint\go2rtc\go2rtc.exe" enable=yes`
**Warning signs:** Connection timeout when accessing go2rtc API or RTSP.

### Pitfall 5: People Tracker OpenCV RTSP URL Format
**What goes wrong:** People tracker fails to connect after switching to go2rtc relay.
**Why it happens:** OpenCV VideoCapture may need TCP transport hint for reliable relay connections.
**How to avoid:** Use `rtsp://127.0.0.1:8554/entrance` (no auth needed for local relay). If issues, append `?transport=tcp` hint.
**Warning signs:** Intermittent frame drops or connection failures after migration.

## Code Examples

### go2rtc Health Check from Rust
```rust
// Source: go2rtc API docs (https://github.com/AlexxIT/go2rtc)
async fn check_go2rtc_health(api_url: &str) -> anyhow::Result<bool> {
    let resp = reqwest::get(format!("{api_url}/api/streams")).await?;
    Ok(resp.status().is_success())
}
```

### retina RTSP Session Setup
```rust
// Source: retina docs (https://docs.rs/retina/0.4.19)
use retina::client::{SessionGroup, SetupOptions, Transport};
use url::Url;

async fn connect_rtsp(relay_url: &str) -> anyhow::Result<retina::client::Demuxed> {
    let url = Url::parse(relay_url)?;
    let session_group = SessionGroup::default();
    let mut session = retina::client::Session::describe(
        url,
        retina::client::SessionOptions::default()
            .session_group(session_group),
    ).await?;

    // Setup first video stream
    session.setup(0, SetupOptions::default().transport(Transport::Tcp)).await?;

    let session = session.play(retina::client::PlayOptions::default()).await?
        .demuxed()?;

    Ok(session)
}
```

### People Tracker Config Migration
```yaml
# BEFORE (config.yaml -- direct camera connections)
cameras:
  entrance:
    ip: "192.168.31.8"
    # ...builds RTSP URL: rtsp://admin:Admin@123@192.168.31.8/cam/realmonitor?channel=1&subtype=1

# AFTER (config.yaml -- via go2rtc relay on localhost)
cameras:
  entrance:
    ip: "127.0.0.1:8554/entrance"
    # ...builds RTSP URL: rtsp://127.0.0.1:8554/entrance (no auth needed)
```

Note: The people tracker's `CameraProcessor.__init__` constructs the RTSP URL as `rtsp://{username}:{password}@{ip}/cam/realmonitor?channel=1&subtype=1`. Migration requires either:
1. Changing the URL construction logic to support relay URLs (recommended), or
2. Overriding the RTSP URL directly in config (simpler, less invasive)

Best approach: Add an optional `rtsp_url` field to config.yaml that overrides the auto-constructed URL when present.

### rc-sentry-ai TOML Config
```toml
# C:\RacingPoint\rc-sentry-ai.toml
[service]
port = 8096
host = "0.0.0.0"

[relay]
api_url = "http://127.0.0.1:1984"
rtsp_base = "rtsp://127.0.0.1:8554"

[[cameras]]
name = "entrance"
stream_name = "entrance"  # matches go2rtc.yaml stream name
role = "entry_exit"
fps = 2

[[cameras]]
name = "reception"
stream_name = "reception"
role = "face_capture"
fps = 2

[[cameras]]
name = "reception_wide"
stream_name = "reception_wide"
role = "face_capture"
fps = 2
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct RTSP per-consumer | RTSP relay (go2rtc/mediamtx) | 2023-2024 | Eliminates stream starvation, enables multi-consumer |
| GStreamer/FFmpeg for RTSP in Rust | retina crate (pure Rust) | 2022+ | No native dependencies, async-native, Windows-friendly |
| RTSP-Simple-Server | MediaMTX (renamed) | 2023 | Same project, rebranded with expanded protocol support |

**Deprecated/outdated:**
- rtsp-simple-server: Renamed to mediamtx. Old name still referenced in some docs.
- VLC as RTSP relay: Works but not designed for production multi-stream relay.

## Open Questions

1. **retina + go2rtc relay interop**
   - What we know: retina connects to standard RTSP servers; go2rtc serves standard RTSP.
   - What's unclear: Whether retina handles go2rtc's on-demand stream startup gracefully (go2rtc may not start pulling from the camera until a consumer connects).
   - Recommendation: Test during Plan 01-02 with a simple connection test. Fallback: use go2rtc's `/api/frame.jpeg` HTTP endpoint for frame extraction instead of retina RTSP.

2. **Frame format for downstream phases**
   - What we know: retina gives H.264 NAL units, not decoded pixels. Phase 113 (Face Detection) needs pixel data for SCRFD/ONNX.
   - What's unclear: Whether to decode in this phase or defer to Phase 113.
   - Recommendation: This phase should only verify frame extraction works. Store raw NALs. Use go2rtc `/api/frame.jpeg` for the health endpoint snapshot. Phase 113 will add GPU decode.

3. **go2rtc process lifecycle on James**
   - What we know: James runs services via HKLM Run keys and bat files.
   - What's unclear: Whether go2rtc should be managed by rc-sentry-ai or run independently.
   - Recommendation: Independent bat file (`start-go2rtc.bat` via HKLM Run or Scheduled Task). rc-sentry-ai checks its health, does not manage its lifecycle. Keeps concerns separated.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p rc-sentry-ai` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-sentry-ai` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CAM-01 | go2rtc relay proxies streams without dropping | smoke (manual) | Manual: verify go2rtc streams via VLC | N/A |
| CAM-02 | All 3 cameras configured in go2rtc | unit | `cargo test -p rc-sentry-ai -- config` | No -- Wave 0 |
| CAM-03 | Health endpoint reports per-camera status | integration | `curl http://127.0.0.1:8096/health` | No -- Wave 0 |
| CAM-04 | People tracker works via relay | smoke (manual) | Manual: verify people tracker at :8095 still counts | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry-ai`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-sentry-ai`
- **Phase gate:** Full suite green + manual smoke tests (VLC stream, people tracker) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-sentry-ai/` -- entire crate does not exist yet, needs scaffold
- [ ] `crates/rc-sentry-ai/src/config.rs` -- config parsing with unit tests
- [ ] `crates/rc-sentry-ai/src/health.rs` -- health endpoint integration tests
- [ ] Workspace Cargo.toml -- needs `rc-sentry-ai` member added
- [ ] go2rtc binary -- needs download and placement at `C:\RacingPoint\go2rtc\`
- [ ] go2rtc.yaml -- needs creation with camera stream definitions

## Sources

### Primary (HIGH confidence)
- [go2rtc GitHub](https://github.com/AlexxIT/go2rtc) -- configuration, API, RTSP relay functionality, v1.9.13
- [retina crate docs.rs](https://docs.rs/retina/0.4.19) -- API structure, VideoFrame, Session, modules
- [retina GitHub](https://github.com/scottlamb/retina) -- examples, codec support, production usage
- [retina on lib.rs](https://lib.rs/crates/retina) -- version 0.4.19, released 2026-03-13

### Secondary (MEDIUM confidence)
- [Frigate go2rtc guide](https://docs.frigate.video/guides/configuring_go2rtc/) -- Dahua camera config examples
- [DahuaWiki RTSP](https://dahuawiki.com/Remote_Access/RTSP_via_VLC) -- RTSP URL format confirmation
- [go2rtc.com](https://go2rtc.com/) -- feature overview, protocol support

### Tertiary (LOW confidence)
- go2rtc vs mediamtx comparison -- community opinions from GitHub issues (subjective)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- go2rtc and retina are well-documented, actively maintained, verified on official sources
- Architecture: HIGH -- follows established monorepo patterns (Axum, tokio, TOML config), two-layer relay design is industry standard
- Pitfalls: HIGH -- stream starvation and auth encoding are well-documented Dahua issues; retina NAL vs pixel distinction verified in docs

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain, slow-moving libraries)
