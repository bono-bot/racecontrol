# Phase 13: Observability - Research

**Researched:** 2026-03-20
**Domain:** In-process metrics, heartbeat enrichment, HTTP metrics endpoint, email fallback validation
**Confidence:** HIGH

## Summary

Phase 13 adds observability to the comms-link so Bono has full visibility into James's operational state. This is the final "polish" phase before graceful degradation (Phase 14). The work divides cleanly into four independent deliverables matching the four requirements: (1) enrich the existing heartbeat payload with pod status, queue depth, and deployment state; (2) accumulate in-process metrics counters (uptime, reconnect count, ACK latency, queue depth); (3) expose those metrics via a JSON HTTP endpoint on James's existing relay server (port 8766); and (4) validate the email fallback path end-to-end in production.

All four deliverables build on existing infrastructure with minimal new code. The heartbeat sender already collects CPU, memory, uptime, and Claude process status -- extending its payload is a data-gathering exercise. The HTTP relay server on port 8766 already serves 10+ routes -- adding GET /relay/metrics is trivial. The metrics accumulation is a new in-process module but follows the exact same DI + EventEmitter pattern as HealthAccumulator on Bono's side. The email fallback validation is a one-shot operational task using the existing send_email.js script.

**Primary recommendation:** Create a single new `james/metrics-collector.js` module that accumulates counters, inject it into HeartbeatSender for payload enrichment, and expose it via a new GET /relay/metrics route on the existing relay server. Email validation is a manual operational step with a simple smoke test.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| OBS-01 | Heartbeat payload extended with pod status, queue depth, and deployment state | HeartbeatSender already uses injectable collectFn; extend system-metrics.js to fetch pod status from rc-core :8080/api/v1 and read queue depth from MessageQueue.size and AckTracker.pendingCount |
| OBS-02 | Metrics counters accumulated in-process: uptime, reconnect count, ACK latency, queue depth | New MetricsCollector class following HealthAccumulator pattern (DI, snapshot, EventEmitter); listens to AckTracker 'ack'/'timeout'/'retry' events and CommsClient 'state' events |
| OBS-03 | Metrics exported as structured JSON via HTTP endpoint for Bono to consume | Add GET /relay/metrics route to existing relay server in james/index.js; returns MetricsCollector.snapshot() as JSON |
| OBS-04 | Email fallback path validated end-to-end (send + receive confirmed) | Use existing send_email.js with known recipients; write a smoke test that invokes it and confirms non-error exit; manual receipt verification |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:http | Node 22.14.0 built-in | Metrics HTTP endpoint | Already used for relay server on port 8766 |
| node:os | Node 22.14.0 built-in | System metrics (CPU, memory, uptime) | Already used in system-metrics.js |
| node:child_process | Node 22.14.0 built-in | Claude detection, email sending | Already used in system-metrics.js and watchdog-runner.js |
| node:events | Node 22.14.0 built-in | EventEmitter for metrics events | Already used throughout codebase |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ws | ^8.19.0 | WebSocket transport (existing) | Already installed, no changes needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom JSON metrics | prom-client / OpenTelemetry | Massively over-engineered for 1 consumer (Bono). Out of Scope per REQUIREMENTS.md |
| Custom HTTP endpoint | Express | 15 lines of node:http vs adding a dependency. Existing relay uses node:http already |

**Installation:**
```bash
# No new packages needed -- all built-in Node.js modules
```

## Architecture Patterns

### Recommended Project Structure
```
james/
  metrics-collector.js    # NEW -- in-process metrics accumulator
  system-metrics.js       # MODIFY -- add pod status + deployment state to collectMetrics()
  heartbeat-sender.js     # MODIFY -- pass MetricsCollector snapshot into heartbeat payload
  index.js                # MODIFY -- add GET /relay/metrics route, wire MetricsCollector
shared/
  protocol.js             # NO CHANGE -- heartbeat is a control message, payload is freeform
test/
  metrics-collector.test.js  # NEW
  system-metrics.test.js     # MODIFY -- add tests for pod status + deployment state
  metrics-endpoint.test.js   # NEW -- HTTP endpoint test
  email-fallback.test.js     # NEW -- email smoke test
```

### Pattern 1: MetricsCollector (In-Process Accumulator)
**What:** A class that accumulates operational counters over time, with a snapshot() method that returns the current state as a plain object. Follows the exact pattern of HealthAccumulator on Bono's side.
**When to use:** When you need to track counters that span multiple events and report them on demand.
**Example:**
```javascript
// Source: Modeled after bono/health-accumulator.js
export class MetricsCollector {
  #startTime;
  #reconnectCount = 0;
  #ackLatencies = [];    // Rolling window of last N latencies
  #nowFn;

  constructor({ nowFn = Date.now } = {}) {
    this.#nowFn = nowFn;
    this.#startTime = this.#nowFn();
  }

  recordReconnect() { this.#reconnectCount++; }
  recordAckLatency(ms) { /* push to rolling window, trim to last 100 */ }

  snapshot() {
    return {
      uptimeMs: this.#nowFn() - this.#startTime,
      reconnectCount: this.#reconnectCount,
      ackLatencyAvgMs: /* average of rolling window */,
      ackLatencyP99Ms: /* 99th percentile */,
      ts: this.#nowFn(),
    };
  }
}
```

### Pattern 2: Heartbeat Payload Enrichment
**What:** Extend the existing collectMetrics() function to fetch additional data sources (pod status, queue depth, deployment state) and merge them into the heartbeat payload.
**When to use:** OBS-01 -- enriching the heartbeat that already fires every 15 seconds.
**Example:**
```javascript
// system-metrics.js -- extended collectMetrics()
export async function collectMetrics({ queueSizeFn, ackPendingFn, metricsSnapshotFn, podStatusFn } = {}) {
  // Existing: cpu, memoryUsed, memoryTotal, uptime, claudeRunning
  const base = { cpu, memoryUsed, memoryTotal, uptime, claudeRunning };

  // New: queue depth
  base.queueDepth = queueSizeFn?.() ?? 0;
  base.ackPending = ackPendingFn?.() ?? 0;

  // New: pod status from rc-core (fire-and-forget, don't block heartbeat)
  base.podStatus = podStatusFn ? await podStatusFn().catch(() => null) : null;

  // New: metrics snapshot
  if (metricsSnapshotFn) Object.assign(base, metricsSnapshotFn());

  return base;
}
```

### Pattern 3: HTTP Metrics Endpoint
**What:** Add GET /relay/metrics to the existing relay server in james/index.js. Returns a JSON snapshot of all current metrics.
**When to use:** OBS-03 -- Bono queries this on demand (not pushed).
**Example:**
```javascript
// In james/index.js relay server handler
if (req.method === 'GET' && req.url === '/relay/metrics') {
  const snapshot = metricsCollector.snapshot();
  snapshot.queueDepth = messageQueue.size;
  snapshot.ackPending = ackTracker.pendingCount;
  snapshot.wsState = client.state;
  jsonResponse(res, 200, snapshot);
  return;
}
```

### Anti-Patterns to Avoid
- **Blocking heartbeat on pod status fetch:** The rc-core HTTP call to fetch pod status may time out. Use a cached value that's refreshed independently on a 30s interval, not inline in the heartbeat path. A failed fetch should return stale data, not block the heartbeat.
- **Unbounded metric arrays:** Never accumulate all ACK latencies forever. Use a rolling window (last 100 values) or exponential moving average.
- **Adding new message types for metrics:** The heartbeat message type already exists and is a control message. Enriching its payload is the right approach -- do NOT create a separate "metrics_push" message type. Bono already receives heartbeat payloads via HeartbeatMonitor.lastPayload.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pod status monitoring | Custom pod polling loop | HTTP GET to rc-core :8080/api/v1/pods (existing API) | rc-core already aggregates pod state from all rc-agents |
| Email sending | SMTP client | Existing send_email.js via execFile | Already handles Google OAuth, tested in daily summary |
| Percentile calculation | Statistics library | Simple sort + index for P99 on 100-element array | 3 lines of code for a fixed-size window |

**Key insight:** This phase adds no new external dependencies. Every data source already exists (rc-core API, MessageQueue.size, AckTracker.pendingCount, send_email.js). The work is gathering and exposing, not building.

## Common Pitfalls

### Pitfall 1: Heartbeat Blocked by Slow HTTP Call
**What goes wrong:** collectMetrics() calls rc-core :8080 for pod status. If rc-core is down or slow, the heartbeat stops firing, Bono thinks James is dead.
**Why it happens:** Making a synchronous HTTP call inside the 15s heartbeat interval timer.
**How to avoid:** Fetch pod status on a separate 30s timer, cache the result. collectMetrics() reads from cache, never blocks.
**Warning signs:** Bono's HeartbeatMonitor fires james_down events while James is actually healthy.

### Pitfall 2: Email Validation Confusion with OAuth
**What goes wrong:** send_email.js requires valid Google OAuth tokens. Gmail MCP OAuth is broken (expired per MEMORY.md). The send_email.js used by daily-summary may use a different auth path.
**Why it happens:** Multiple email sending paths exist in the codebase.
**How to avoid:** Test with the exact same send_email.js path and environment that the daily summary uses. On James: SEND_EMAIL_PATH env var. Verify tokens are valid before assuming E2E test can work.
**Warning signs:** "getAuthClient" errors, "No access, refresh token" in stderr.

### Pitfall 3: Stale Metrics After Reconnect
**What goes wrong:** MetricsCollector accumulates counters from daemon start. After a reconnect, the reconnect count increments but ACK latency window may be empty (no recent ACKs during disconnection).
**Why it happens:** Rolling window empties during disconnection period.
**How to avoid:** Return explicit "no data" / null for metrics that have no recent samples, rather than 0 which implies "zero latency."

### Pitfall 4: Pod Status API Shape Unknown
**What goes wrong:** Assuming rc-core's pod status API returns a specific shape without verifying.
**Why it happens:** The rc-core API is in the racecontrol repo, not comms-link.
**How to avoid:** During implementation, make a test call to `http://localhost:8080/api/v1/pods` (or equivalent) and log the response shape. Design the heartbeat payload field to pass through whatever rc-core returns, not transform it.

## Code Examples

### Current Heartbeat Payload (as-is)
```javascript
// Source: james/system-metrics.js collectMetrics()
{
  cpu: 12.5,           // CPU usage percentage
  memoryUsed: 45.2,    // Memory usage percentage
  memoryTotal: 34359738368, // Total RAM bytes
  uptime: 86400,       // OS uptime seconds
  claudeRunning: true  // Claude Code process detected
}
```

### Target Heartbeat Payload (OBS-01)
```javascript
{
  // Existing fields
  cpu: 12.5,
  memoryUsed: 45.2,
  memoryTotal: 34359738368,
  uptime: 86400,
  claudeRunning: true,
  // NEW: Queue depth (OBS-01)
  queueDepth: 3,         // MessageQueue pending count
  ackPending: 1,         // AckTracker unACKed messages
  // NEW: Pod status (OBS-01)
  podStatus: null,       // Cached rc-core response or null if unavailable
  // NEW: Deployment state (OBS-01)
  deployState: {
    version: '2.0.0',    // from package.json
    nodeVersion: '22.14.0',
    daemonPid: 12345,
    startedAt: 1710900000000,
  },
  // NEW: Accumulated metrics (OBS-02)
  reconnectCount: 2,
  ackLatencyAvgMs: 45,
  daemonUptimeMs: 3600000,
}
```

### MetricsCollector Wiring in index.js
```javascript
// james/index.js -- wire MetricsCollector into existing components
import { MetricsCollector } from './metrics-collector.js';

const metricsCollector = new MetricsCollector();

// Wire to AckTracker events
ackTracker.on('ack', (messageId) => {
  // Calculate latency from track time to ack time
  metricsCollector.recordAckLatency(/* delta */);
});

// Wire to CommsClient state changes
client.on('state', (evt) => {
  if (evt.state === 'CONNECTED' && evt.previous === 'RECONNECTING') {
    metricsCollector.recordReconnect();
  }
});

// Inject into HeartbeatSender's collectFn
const heartbeat = new HeartbeatSender(client, {
  collectFn: () => collectMetrics({
    queueSizeFn: () => messageQueue.size,
    ackPendingFn: () => ackTracker.pendingCount,
    metricsSnapshotFn: () => metricsCollector.snapshot(),
  }),
});
```

### Email Fallback Smoke Test
```javascript
// test/email-fallback.test.js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';

test('OBS-04: send_email.js exits without error', { skip: !process.env.SEND_EMAIL_PATH }, (t, done) => {
  const emailPath = process.env.SEND_EMAIL_PATH;
  execFile('node', [
    emailPath,
    '--to', 'bono@racingpoint.in',
    '--subject', '[COMMS-LINK] E2E Fallback Test',
    '--body', `Email fallback validated at ${new Date().toISOString()}`,
  ], { timeout: 30000 }, (err, stdout, stderr) => {
    // Non-zero exit or error means email path is broken
    assert.ifError(err);
    done();
  });
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Heartbeat with basic system metrics only | Enriched heartbeat with operational state | Phase 13 (now) | Bono sees full picture without querying |
| No in-process metrics | MetricsCollector accumulator | Phase 13 (now) | Trend data available for debugging |
| No metrics endpoint | GET /relay/metrics on port 8766 | Phase 13 (now) | Bono can query on demand |
| Email fallback assumed working | E2E validated | Phase 13 (now) | Confidence in fallback path |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node 22.14.0) |
| Config file | none -- `node --test test/*.test.js` |
| Quick run command | `node --test test/metrics-collector.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OBS-01 | Heartbeat includes pod status, queue depth, deployment state | unit | `node --test test/system-metrics.test.js -x` | Exists (needs extension) |
| OBS-02 | MetricsCollector accumulates uptime, reconnect, ACK latency, queue depth | unit | `node --test test/metrics-collector.test.js -x` | Wave 0 |
| OBS-03 | GET /relay/metrics returns structured JSON | integration | `node --test test/metrics-endpoint.test.js -x` | Wave 0 |
| OBS-04 | Email send_email.js exits without error | smoke | `node --test test/email-fallback.test.js -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/metrics-collector.test.js test/system-metrics.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (374+ tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/metrics-collector.test.js` -- covers OBS-02 (MetricsCollector unit tests)
- [ ] `test/metrics-endpoint.test.js` -- covers OBS-03 (HTTP endpoint integration test)
- [ ] `test/email-fallback.test.js` -- covers OBS-04 (email smoke test, skip-if-no-env)

## Open Questions

1. **rc-core Pod Status API Shape**
   - What we know: rc-core runs on :8080, has `/api/v1` prefix, handles sync_push/sync_action
   - What's unclear: Exact endpoint and response shape for pod status listing
   - Recommendation: During implementation, test-call `http://localhost:8080/api/v1/pods` (or similar) and design the heartbeat field as a passthrough of whatever rc-core returns. If endpoint doesn't exist, omit podStatus from heartbeat (null) rather than blocking the phase.

2. **send_email.js CLI Arguments**
   - What we know: Used by daily-summary.js with `--to`, `--subject`, `--body` args; lives at SEND_EMAIL_PATH
   - What's unclear: Whether the Gmail OAuth tokens on James's machine are currently valid (MEMORY.md says "Gmail OAuth tokens expired")
   - Recommendation: Attempt the E2E send. If OAuth is broken, document it as a known blocker for OBS-04 and mark the requirement as "validated infrastructure exists, auth issue separate." The email path code is correct; the auth renewal is an ops task, not a code task.

3. **Deployment State Content**
   - What we know: Version from package.json, node version from process.version, PID from process.pid
   - What's unclear: Whether "deployment state" should include git commit hash, last deploy timestamp, or binary versions
   - Recommendation: Start minimal -- package.json version + node version + daemon PID + start timestamp. Extend later if Bono needs more.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis -- all james/, bono/, shared/ source files (read in full)
- james/system-metrics.js -- current heartbeat payload shape (5 fields)
- james/heartbeat-sender.js -- DI pattern with injectable collectFn
- shared/ack-tracker.js -- pendingCount getter and event emissions (ack, timeout, retry)
- shared/message-queue.js -- size getter for queue depth
- bono/health-accumulator.js -- reference pattern for in-process metrics accumulation
- bono/heartbeat-monitor.js -- lastPayload getter shows Bono already stores heartbeat data
- bono/comms-server.js -- existing HTTP relay routes + jsonResponse helper
- james/index.js -- relay server with 10+ routes, relay port 8766
- .planning/REQUIREMENTS.md -- OBS-01..04 definitions
- .planning/research/SUMMARY.md -- Phase 5 (now Phase 13) architecture notes

### Secondary (MEDIUM confidence)
- MEMORY.md -- Gmail OAuth tokens expired (affects OBS-04 email validation)
- package.json -- Node 22.14.0, ws ^8.19.0, no other dependencies

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all Node.js built-ins already in use
- Architecture: HIGH -- follows exact patterns from bono/health-accumulator.js and existing relay server
- Pitfalls: HIGH -- based on direct codebase analysis (heartbeat blocking risk, OAuth state from MEMORY.md)
- Email validation: MEDIUM -- depends on OAuth token state which may be broken

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, minimal external dependency risk)
