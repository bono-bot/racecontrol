# Phase 145: go2rtc Infrastructure - Research

**Researched:** 2026-03-22
**Domain:** go2rtc configuration — Dahua NVR RTSP streams + CORS + WebRTC coexistence
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation choices are at Claude's discretion.

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key decisions to make during execution:
- Whether to use NVR RTSP URLs (rtsp://192.168.31.18/cam/realmonitor?channel=N&subtype=1) or direct camera IPs
- go2rtc CORS config: `origin: "*"` under `[api]` section
- Stream naming convention: ch1-ch13 mapping to NVR channels 1-13
- Whether H.264 transcoding is needed (via ffmpeg: prefix) or native H.265 relay works

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFRA-01 | All 13 NVR cameras are registered in go2rtc with RTSP sub-stream URLs | go2rtc 1.9.13 is installed; Dahua NVR RTSP URL pattern confirmed; ch1-ch13 naming strategy documented |
| INFRA-02 | go2rtc CORS is configured and verified for cross-port WebRTC access from :8096 and :3200 | `origin: "*"` in `[api]` section confirmed as the only supported CORS value; verification curl command documented |
</phase_requirements>

---

## Summary

go2rtc 1.9.13 is already installed at `C:\RacingPoint\go2rtc\go2rtc.exe`. The current `go2rtc.yaml` has 3 direct-IP cameras (entrance, reception, reception_wide) plus 3 H.264 transcoded variants. This phase extends that config to cover all 13 NVR channels using the NVR RTSP passthrough URL format (`rtsp://192.168.31.18/cam/realmonitor?channel=N&subtype=1`), adds CORS via `origin: "*"` in the `api:` section, and verifies that snapshot fetching (SnapshotCache in rc-sentry-ai) and WebRTC can coexist on the same NVR channel.

The coexistence concern is resolved by go2rtc's fundamental design: it opens **one RTSP connection per stream** to the NVR and fans out to any number of consumers (WebRTC sessions, MJPEG, snapshot). The existing SnapshotCache fetches JPEG snapshots directly from the NVR HTTP API (`/cgi-bin/snapshot.cgi?channel=N`) — it does NOT go through go2rtc RTSP at all. These are independent NVR connections using different protocols (HTTP vs RTSP), so they cannot conflict at the go2rtc level. The only resource to watch is Dahua NVR's per-channel RTSP connection limit (typically 4-6 simultaneous RTSP sessions per channel), which is not an issue with go2rtc holding one session per channel and snapshot using HTTP.

H.264 vs H.265: the sub-stream cameras transmit H.265 natively. Chrome 136+ added H.265 WebRTC support but it is not universally reliable. The existing pattern of using `ffmpeg:` prefix for H.264 transcoding is proven in this project. For the 13-channel registration, the **recommended approach** is to register both native H.265 streams (for RTSP relay and any Safari/edge use) and H.264 transcoded variants (for WebRTC browser consumption). The planner should evaluate whether to add h264 variants for all 13 or only for the cameras that will actively serve WebRTC (this phase only needs WebRTC test to open — a single channel suffices for the test).

**Primary recommendation:** Add 13 NVR streams to go2rtc.yaml using NVR RTSP passthrough URLs (ch1–ch13), add `origin: "*"` to the `api:` section, restart go2rtc, and verify with `curl -X OPTIONS` and a manual WebRTC session via go2rtc web UI.

---

## Standard Stack

### Core
| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| go2rtc | 1.9.13 (installed) | RTSP-to-WebRTC relay, stream mux | Already deployed, proven with 3 cameras |
| Dahua NVR RTSP | Protocol standard | Camera stream source | NVR handles camera failover, single IP endpoint |
| go2rtc.yaml | YAML | go2rtc configuration | Only supported config format |

### Supporting
| Component | Purpose | When to Use |
|-----------|---------|-------------|
| ffmpeg (bundled in go2rtc) | H.265 → H.264 transcode | When WebRTC session in Chrome fails due to codec — use `ffmpeg:` prefix |
| go2rtc web UI (:1984) | Manual WebRTC test | Verify stream opens; no extra tool needed |
| curl | CORS verification, OPTIONS preflight check | CI/manual verification step |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| NVR RTSP passthrough | Direct camera IPs (.8, .15, .154, etc.) | Direct IPs require knowing each camera IP; NVR handles failover; NVR approach is consistent for all 13 channels |
| `origin: "*"` CORS | Specific origins (:8096, :3200) | go2rtc only supports `"*"` — no per-origin allow-list supported |
| H.264 transcode for all 13 | Transcode only tested channels | Transcoding all 13 simultaneously costs CPU; only needed for WebRTC consumers; omit until dashboard phase |

**Installation:** go2rtc already installed. No new packages needed.

---

## Architecture Patterns

### Existing go2rtc.yaml Structure (Current)

```yaml
streams:
  entrance:            rtsp://admin:Admin%40123@192.168.31.8/cam/realmonitor?channel=1&subtype=1
  reception:           rtsp://admin:Admin%40123@192.168.31.15/cam/realmonitor?channel=1&subtype=1
  reception_wide:      rtsp://admin:Admin%40123@192.168.31.154/cam/realmonitor?channel=1&subtype=1
  entrance_h264:       ffmpeg:rtsp://admin:Admin%40123@192.168.31.8/cam/realmonitor?channel=1&subtype=1#video=h264
  reception_h264:      ffmpeg:rtsp://admin:Admin%40123@192.168.31.15/cam/realmonitor?channel=1&subtype=1#video=h264
  reception_wide_h264: ffmpeg:rtsp://admin:Admin%40123@192.168.31.154/cam/realmonitor?channel=1&subtype=1#video=h264

api:
  listen: ":1984"

rtsp:
  listen: ":8554"
```

### Pattern 1: NVR Channel Registration (ch1–ch13)

**What:** Register all 13 cameras as named streams using the NVR as the RTSP source rather than direct camera IPs.

**When to use:** Always — NVR passthrough URL is the canonical approach for Dahua NVRs.

**Dahua NVR RTSP URL format:**
```
rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=N&subtype=1
```

- `channel=N` — NVR channel number (1-13)
- `subtype=1` — sub-stream (lower resolution, lower bitrate, suitable for dashboard)
- `subtype=0` — main stream (full resolution, NOT for dashboard — too heavy)
- Password `Admin@123` must be percent-encoded: `@` → `%40`

**Example streams section for all 13 channels:**
```yaml
streams:
  # NVR channels via sub-stream (H.265 native)
  ch1:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=1&subtype=1
  ch2:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=2&subtype=1
  ch3:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=3&subtype=1
  ch4:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=4&subtype=1
  ch5:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=5&subtype=1
  ch6:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=6&subtype=1
  ch7:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=7&subtype=1
  ch8:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=8&subtype=1
  ch9:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=9&subtype=1
  ch10: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=10&subtype=1
  ch11: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=11&subtype=1
  ch12: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=12&subtype=1
  ch13: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=13&subtype=1
```

Existing direct-IP streams (`entrance`, `reception`, `reception_wide`) and their h264 variants remain in the file — they are used by the AI detection pipeline and must NOT be removed.

### Pattern 2: CORS Configuration

**What:** Add `origin: "*"` under the `api:` section in go2rtc.yaml.

**When to use:** Required for browser-side WebRTC from :8096 and :3200 to reach go2rtc on :1984.

**Example:**
```yaml
api:
  listen: ":1984"
  origin: "*"
```

Source: [go2rtc HTTP API docs](https://go2rtc.org/internal/api/) — confirmed `origin: "*"` is the only supported CORS value. Per-origin allowlists are NOT supported.

### Pattern 3: WebRTC Connection (Browser-side)

**What:** WebSocket-based WebRTC signaling via go2rtc API.

**Endpoint format:**
```
ws://192.168.31.27:1984/api/ws?src=ch1
```

**JavaScript connection pattern:**
```javascript
// Source: go2rtc source webrtc.html
const ws = new WebSocket(`ws://192.168.31.27:1984/api/ws?src=ch1`);
// Exchange SDP offer/answer via JSON messages:
// { "type": "webrtc/offer", "value": "<sdp>" }
// { "type": "webrtc/answer", "value": "<sdp>" }
// { "type": "webrtc/candidate", "value": "<ice-candidate>" }
```

### Pattern 4: go2rtc Stream Muxing (Coexistence)

**What:** go2rtc holds one RTSP connection per named stream to the NVR. Multiple consumers (WebRTC sessions, RTSP relay) share that single connection — go2rtc muxes internally.

**Key insight for coexistence:** SnapshotCache in rc-sentry-ai fetches JPEG via NVR HTTP API (`/cgi-bin/snapshot.cgi?channel=N`) — this is a separate HTTP connection to the NVR, completely independent of go2rtc's RTSP sessions. No conflict possible at the go2rtc layer.

**NVR connection budget (13 channels):**
- go2rtc: 1 RTSP session per channel = 13 sessions total (one per channel, only while a consumer is connected; lazy — opens on first request)
- SnapshotCache: up to 13 concurrent HTTP GET requests in rotation
- Dahua NVR typically supports 8+ concurrent RTSP sessions per channel — 13 total across 13 different channels is well within limits

### Anti-Patterns to Avoid

- **Do NOT use main stream (subtype=0)** for dashboard — bandwidth is 2-4x higher, will saturate LAN
- **Do NOT remove existing streams** — `entrance_h264`, `reception_h264`, `reception_wide_h264` feed the AI detection pipeline
- **Do NOT put NVR credentials in browser-visible RTSP URLs** — go2rtc is the relay; browser only sees `ws://go2rtc:1984/api/ws?src=ch1`, credentials never reach the browser
- **Do NOT transcode all 13 channels to H.264 in this phase** — only needed for WebRTC consumers; dashboard streaming is phase 147

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RTSP→WebRTC bridge | Custom signaling server | go2rtc (already installed) | ICE, STUN, SDP negotiation, codec handling — hundreds of edge cases |
| CORS header injection | Proxy or middleware | `origin: "*"` in go2rtc.yaml | One-line config; go2rtc handles all OPTIONS preflights |
| Multi-client stream fan-out | Custom muxer | go2rtc built-in muxing | go2rtc already opens one RTSP session per stream and fans out |
| H.265→H.264 transcode | Custom ffmpeg pipeline | `ffmpeg:` prefix in go2rtc stream source | go2rtc handles ffmpeg process management and output routing |

---

## Common Pitfalls

### Pitfall 1: `@` Not Percent-Encoded in Password

**What goes wrong:** RTSP URL parsed incorrectly; `@` in `Admin@123` is treated as host separator.

**Why it happens:** RTSP URLs follow RFC 3986 — `@` in userinfo must be `%40`.

**How to avoid:** Always write `Admin%40123` in go2rtc.yaml RTSP URLs.

**Warning signs:** go2rtc logs `connection refused` or `invalid host` for NVR streams.

### Pitfall 2: go2rtc Opens RTSP Lazily — No Consumer = No Connection

**What goes wrong:** After adding 13 streams, testing "is go2rtc connected?" without opening a stream consumer shows nothing in go2rtc UI.

**Why it happens:** go2rtc connects to sources on-demand when the first consumer attaches. Idle streams have no RTSP session.

**How to avoid:** Open go2rtc web UI at http://192.168.31.27:1984 → click stream name → verify video appears. Or use the WebRTC test page.

**Warning signs:** Stream shows as "registered" in config but no active connection in the go2rtc streams list — this is normal until a client connects.

### Pitfall 3: H.265 WebRTC Fails in Chrome < 136

**What goes wrong:** WebRTC session opens but no video; browser console shows codec negotiation failure.

**Why it happens:** Chrome < 136 does not support H.265 in WebRTC. Dahua 4MP cameras use H.265 on sub-streams.

**How to avoid:** For the WebRTC coexistence test in this phase, use the `ffmpeg:` prefix to register an H.264 transcoded variant of one channel (e.g., `ch1_h264`). Then test WebRTC on `ch1_h264`.

**Warning signs:** WebRTC `RTCPeerConnection` SDP answer shows no matching video codec; video element stays black.

### Pitfall 4: Restarting go2rtc Drops AI Detection Pipeline Temporarily

**What goes wrong:** go2rtc restart drops RTSP sessions for `entrance_h264`, `reception_h264`, `reception_wide_h264`, briefly interrupting face detection.

**Why it happens:** rc-sentry-ai stream.rs reconnects when go2rtc RTSP relay goes offline, but there is a reconnect delay.

**How to avoid:** Edit go2rtc.yaml, restart go2rtc during low-activity time (not during operating hours). rc-sentry-ai auto-reconnects within ~5 seconds.

**Warning signs:** rc-sentry-ai log shows `RTSP connection lost` then `RTSP reconnected` — expected during this phase.

### Pitfall 5: NVR Channel Numbers vs Camera Numbers

**What goes wrong:** ch1 in go2rtc maps to the wrong physical camera.

**Why it happens:** NVR channel numbering depends on cable port order, not camera IP or name.

**How to avoid:** After adding all 13, open go2rtc web UI and visually verify each stream shows the expected area. Record ch → physical location mapping in config comments.

**Warning signs:** Camera tile shows wrong room/area in dashboard.

### Pitfall 6: CORS `OPTIONS` Preflight — Only `*` Supported

**What goes wrong:** Attempt to set `origin: "http://192.168.31.27:8096"` (specific origin) in go2rtc.yaml.

**Why it happens:** go2rtc's CORS implementation only supports `"*"` — specific origin strings are not processed.

**How to avoid:** Use `origin: "*"`. Verify with `curl -X OPTIONS http://192.168.31.27:1984/api/ws -v` and check for `Access-Control-Allow-Origin: *` in response headers.

---

## Code Examples

### Complete Updated go2rtc.yaml

```yaml
streams:
  # --- Existing AI detection streams (DO NOT REMOVE) ---
  entrance:            rtsp://admin:Admin%40123@192.168.31.8/cam/realmonitor?channel=1&subtype=1
  reception:           rtsp://admin:Admin%40123@192.168.31.15/cam/realmonitor?channel=1&subtype=1
  reception_wide:      rtsp://admin:Admin%40123@192.168.31.154/cam/realmonitor?channel=1&subtype=1
  entrance_h264:       ffmpeg:rtsp://admin:Admin%40123@192.168.31.8/cam/realmonitor?channel=1&subtype=1#video=h264
  reception_h264:      ffmpeg:rtsp://admin:Admin%40123@192.168.31.15/cam/realmonitor?channel=1&subtype=1#video=h264
  reception_wide_h264: ffmpeg:rtsp://admin:Admin%40123@192.168.31.154/cam/realmonitor?channel=1&subtype=1#video=h264

  # --- NVR channels ch1-ch13 via sub-stream ---
  ch1:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=1&subtype=1
  ch2:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=2&subtype=1
  ch3:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=3&subtype=1
  ch4:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=4&subtype=1
  ch5:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=5&subtype=1
  ch6:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=6&subtype=1
  ch7:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=7&subtype=1
  ch8:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=8&subtype=1
  ch9:  rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=9&subtype=1
  ch10: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=10&subtype=1
  ch11: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=11&subtype=1
  ch12: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=12&subtype=1
  ch13: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=13&subtype=1

  # --- H.264 transcoded for WebRTC testing (add one to verify coexistence) ---
  ch1_h264: ffmpeg:rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=1&subtype=1#video=h264

api:
  listen: ":1984"
  origin: "*"

rtsp:
  listen: ":8554"
```

### CORS Verification Command

```bash
# [James .27] Verify CORS headers present
curl -X OPTIONS http://192.168.31.27:1984/api/ws -v 2>&1 | grep -i "access-control"
# Expected output: Access-Control-Allow-Origin: *
```

### WebRTC Stream Reachability Test

```bash
# [James .27] Check stream is registered and accessible
curl -s http://192.168.31.27:1984/api/streams | python -m json.tool | grep -i "ch"
# Lists all registered stream names

# Manual: Open http://192.168.31.27:1984 in browser
# Click ch1 or ch1_h264 → verify WebRTC video appears
```

### Snapshot + WebRTC Coexistence Verification

```bash
# [James .27] Trigger snapshot fetch while WebRTC session is open in browser
# 1. Open http://192.168.31.27:1984 → start WebRTC on ch1_h264
# 2. While video is playing, run:
curl -s "http://192.168.31.27:8096/api/v1/cameras/nvr/1/snapshot" -o /dev/null -w "%{http_code}"
# Expected: 200 (snapshot served from SnapshotCache regardless of WebRTC session)
```

### go2rtc Restart (Windows)

```powershell
# [James .27] Stop, apply config, restart go2rtc
# go2rtc runs as a process — check how it's started first
Get-Process go2rtc -ErrorAction SilentlyContinue | Stop-Process
Start-Process "C:\RacingPoint\go2rtc\go2rtc.exe" -ArgumentList "-config C:\RacingPoint\go2rtc\go2rtc.yaml" -WindowStyle Hidden
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Direct camera IPs per stream | NVR passthrough URL (channel=N) | Single auth endpoint, NVR handles failover |
| No CORS | `origin: "*"` in api section | Browser-side WebRTC from any port now works |
| 3 cameras only | 13 channels via ch1-ch13 | Full venue coverage in dashboard |
| H.265 native relay only | H.265 native + H.264 transcoded variant | WebRTC works in all Chrome versions |

---

## Open Questions

1. **How is go2rtc currently started on James's machine?**
   - What we know: go2rtc.exe is at `C:\RacingPoint\go2rtc\`, running on port 1984
   - What's unclear: Is it started via Task Scheduler, HKLM Run key, or manual? The planner needs to know how to restart it safely.
   - Recommendation: Task in plan to check startup method before restarting.

2. **Which NVR channel numbers map to which physical cameras?**
   - What we know: 13 Dahua cameras, NVR at .18, channels 1-13 exist
   - What's unclear: Which channel is entrance, which are pods, etc.
   - Recommendation: After enabling all 13 streams, visually verify each in go2rtc web UI and document in yaml comments. Not a blocker for INFRA-01/02.

3. **Does the Dahua NVR at .18 have any per-channel RTSP connection limits configured?**
   - What we know: Default Dahua limits are typically 4-6 simultaneous RTSP sessions per channel
   - What's unclear: Whether admin has set a lower limit
   - Recommendation: Test with go2rtc opening 1 session per channel (13 total across 13 channels) — this is far within typical limits.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | None (infrastructure-only phase — config file edit + live verification) |
| Config file | `C:\RacingPoint\go2rtc\go2rtc.yaml` |
| Quick run command | `curl -s http://192.168.31.27:1984/api/streams` |
| Full suite command | Manual verification: CORS check + WebRTC open + snapshot 200 |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INFRA-01 | All 13 NVR channels reachable via go2rtc stream names ch1-ch13 | smoke | `curl -s http://192.168.31.27:1984/api/streams \| python -m json.tool \| grep ch` | ❌ Wave 0 |
| INFRA-01 | WebRTC test session opens on at least one channel | manual | Open http://192.168.31.27:1984 in browser, click ch1_h264, verify video | N/A — manual |
| INFRA-02 | CORS headers present on go2rtc API | smoke | `curl -X OPTIONS http://192.168.31.27:1984/api/ws -v 2>&1 \| grep -i access-control` | ❌ Wave 0 |
| INFRA-02 | Snapshot + WebRTC coexist | manual | WebRTC playing + curl snapshot endpoint returns 200 | N/A — manual |

### Sampling Rate
- **Per task commit:** `curl -s http://192.168.31.27:1984/api/streams | python -m json.tool`
- **Per wave merge:** Full manual checklist (CORS + WebRTC + snapshot)
- **Phase gate:** All 4 test rows green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Verify go2rtc startup method before any restart task
- [ ] No test framework to install — pure config + curl verification

---

## Sources

### Primary (HIGH confidence)
- go2rtc v1.9.13 installed binary — version verified directly
- [go2rtc HTTP API docs](https://go2rtc.org/internal/api/) — `origin: "*"` CORS config confirmed
- [go2rtc WebRTC docs](https://go2rtc.org/internal/webrtc/) — WebSocket endpoint format `ws://:1984/api/ws?src=`
- `C:\RacingPoint\go2rtc\go2rtc.yaml` — existing config read directly
- `C:\RacingPoint\rc-sentry-ai.toml` — current camera config read directly
- `crates/rc-sentry-ai/src/mjpeg.rs` — SnapshotCache implementation read directly (HTTP, not RTSP)

### Secondary (MEDIUM confidence)
- [Frigate go2rtc guide](https://docs.frigate.video/guides/configuring_go2rtc/) — confirms one RTSP connection per stream, multi-client fan-out
- [Dahua RTSP format](https://dahuatech.zendesk.com/hc/en-gb/articles/16320900884754) — `channel=N&subtype=1` URL pattern confirmed
- go2rtc GitHub issues — H.265 WebRTC browser compatibility (Chrome 136+ required for H.265; ffmpeg transcode needed for broader compat)

### Tertiary (LOW confidence)
- Community reports on NVR RTSP connection limits (4-6 per channel typical for Dahua) — not verified against specific NVR model

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — go2rtc 1.9.13 installed, existing config verified from disk
- Architecture: HIGH — CORS config verified from official docs, coexistence logic verified from source code
- Pitfalls: HIGH — percent-encoding and lazy-connect from direct go2rtc knowledge; H.265 from community + official issues
- Open questions: MEDIUM — startup method and channel mapping need task-time investigation

**Research date:** 2026-03-22 IST
**Valid until:** 2026-04-22 (go2rtc stable, Dahua RTSP protocol stable)
