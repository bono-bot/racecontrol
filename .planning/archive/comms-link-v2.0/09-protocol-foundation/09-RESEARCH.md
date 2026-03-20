# Phase 9: Protocol Foundation - Research

**Researched:** 2026-03-20
**Domain:** ACK tracking, durable message queue, reliable delivery for 2-node WebSocket system
**Confidence:** HIGH

## Summary

Phase 9 builds two standalone modules -- `shared/ack-tracker.js` and `shared/message-queue.js` -- plus protocol.js additions for `msg_ack`. These are pure library code with no side effects, not wired into the daemon yet (Phase 11 does wiring). The codebase is Node 22.14.0, ESM (`"type": "module"`), zero deps beyond `ws@^8.19.0`, 23 test files with 222+ tests using `node:test`. All classes use dependency injection (injectable functions via constructor) and EventEmitter for cross-component notifications.

The key unresolved decision -- SQLite vs JSON WAL -- is resolved by this research: **use a JSON Lines WAL file** (append-only, one JSON object per line). This is the correct choice for a queue of hundreds of messages on a 2-node system. Zero native dependencies, no compilation toolchain needed on either Windows or Linux, NTFS-safe (appends don't corrupt existing data on crash), and consistent with the existing codebase's pure-JS philosophy. SQLite would require `better-sqlite3` C++ compilation on both platforms for marginal benefit at this scale.

The ACK tracker follows the standard TCP-like pattern: monotonic integer sequence numbers (never timestamps), per-sender counters, exponential backoff retry (3 max), and a strict data/control message split to prevent ACK storms. The dedup cache uses a Map with 1000-entry / 1hr TTL eviction. Both modules are fully testable via DI -- no real timers, no real filesystem in unit tests.

**Primary recommendation:** Build ack-tracker.js and message-queue.js as standalone modules with full DI, following the exact patterns established by ProcessSupervisor (constructor options object, injectable functions, EventEmitter events, public `poll()` for testing). Lock JSON Lines WAL for the queue. Target 30+ new tests.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| REL-01 | Sender assigns monotonic sequence number to each data message | AckTracker: integer counter starting at 0, incremented per tracked message. Never use Date.now() (Pitfall 20: 15.6ms Windows timer resolution). |
| REL-02 | Receiver sends msg_ack with received sequence number within 1 second | New `msg_ack` message type in protocol.js. AckTracker on receiver side auto-emits ACK for data message types. |
| REL-03 | Sender retries unACKed messages with exponential backoff (3 retries max) | AckTracker retry logic: 1s base, 2x multiplier, 3 max retries, `timeout` event on exhaustion. Reuse EscalatingCooldown pattern from watchdog.js. |
| REL-04 | On reconnect, sender replays from last ACKed sequence number | AckTracker.getPendingMessages() returns all unACKed messages ordered by sequence. Called by daemon on reconnect (wiring is Phase 11, but API must support it). |
| REL-05 | Receiver deduplicates via seen-message cache (last 1000 IDs, 1hr TTL) | DeduplicatorCache class: Map-based, 1000 max entries, 1hr TTL, injectable `nowFn`. Standalone or embedded in ack-tracker.js. |
| REL-06 | Control messages (heartbeat, msg_ack) never require ACKs | `CONTROL_TYPES` Set in protocol.js. AckTracker.track() rejects control types. Receive handler gates on message type before ACK logic. Prevents Pitfall 19 (ACK storms). |
| TQ-01 | Messages persisted to file-backed WAL before sending | MessageQueue.enqueue() appends JSON line to `.wal` file, then returns. WAL write happens before WebSocket send. |
| TQ-02 | ACKed messages removed from WAL (compaction) | MessageQueue.acknowledge(id) marks entry as ACKed. Compaction rewrites WAL excluding ACKed entries when ratio exceeds threshold (e.g., 50% ACKed). |
| TQ-03 | On crash and restart, unACKed messages loaded from WAL and resent | MessageQueue constructor reads WAL on init, filters out ACKed entries, populates in-memory queue. getPending() returns all unACKed for replay. |
| TQ-04 | WAL writes are atomic (no partial/corrupt entries on crash) | JSON Lines append-only pattern: partial last line detected on read by try/catch JSON.parse per line. Corrupt last line is discarded (only the message being written at crash time is lost, not the entire queue). |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:test | built-in (Node 22.14.0) | Test runner | Already used for all 222+ existing tests, no additional deps |
| node:assert/strict | built-in | Assertions | Project convention from all 23 test files |
| node:events (EventEmitter) | built-in | Cross-component notifications | Every existing component uses this pattern |
| node:crypto (randomUUID) | built-in | Message IDs | Already used in protocol.js createMessage() |
| node:fs/promises | built-in | WAL file I/O | Already used in logbook-watcher.js with DI |
| ws | ^8.19.0 | WebSocket transport | Only external dependency, unchanged |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| node:path | built-in | WAL file path construction | MessageQueue storePath resolution |
| node:os | built-in | tmpdir for test isolation | Test fixtures need temp directories |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JSON Lines WAL | better-sqlite3 ^12.8.0 | SQLite gives queryable history but requires C++ build tools on Windows + Linux. Overkill for queue of hundreds. |
| JSON Lines WAL | node:sqlite (experimental) | Prints ExperimentalWarning, API unstable, same native dep issue |
| Custom retry timers | EscalatingCooldown from watchdog.js | EscalatingCooldown is for process restart (ready/fired model). ACK retry needs per-message independent timers. Similar concept but different shape -- build purpose-specific. |

**Installation:**
```bash
# No new dependencies needed. Everything is Node.js built-in + existing ws.
```

**Version verification:** Node 22.14.0 confirmed on James's machine. `ws@^8.19.0` already in package.json.

## Architecture Patterns

### Recommended Project Structure
```
shared/
  ack-tracker.js          # NEW: ACK tracking + retry + dedup cache
  message-queue.js        # NEW: WAL-backed durable message queue
  protocol.js             # MODIFIED: add msg_ack type + CONTROL_TYPES set
test/
  ack-tracker.test.js     # NEW: 15-20 tests
  message-queue.test.js   # NEW: 15-20 tests
  protocol.test.js        # EXISTING: add tests for new types
```

### Pattern 1: Dependency Injection Constructor (Mandatory -- Project Convention)
**What:** All new classes accept an options object with injectable functions for external dependencies.
**When to use:** Every new class. No exceptions.
**Example from codebase:**
```javascript
// Source: james/process-supervisor.js (Phase 10, shipped)
class ProcessSupervisor {
  constructor({ healthCheckFn, killFn, spawnFn, pollMs, failThreshold, nowFn, pidFilePath }) { ... }
  async poll() { ... }  // Public for testing
}

// Source: james/watchdog.js (v1.0, shipped)
class ClaudeWatchdog extends EventEmitter {
  constructor({ detectFn, killFn, spawnFn, findExeFn, cooldown }) { ... }
}
```

**Apply to AckTracker:**
```javascript
class AckTracker extends EventEmitter {
  constructor({ sendFn, nowFn = Date.now, timeoutMs = 10000, maxRetries = 3 })
  // sendFn: injected -- how to send a message (allows test mock)
  // nowFn: injected -- time source (allows deterministic tests)
}
```

**Apply to MessageQueue:**
```javascript
class MessageQueue {
  constructor({ storePath, appendFileFn, readFileFn, writeFileFn, nowFn, maxSize = 1000 })
  // All filesystem ops injectable for testing
}
```

### Pattern 2: EventEmitter for State Changes (Project Convention)
**What:** Components emit named events; wiring functions connect them.
**Example events:**
- AckTracker: `'retry'` (messageId, attempt), `'timeout'` (messageId), `'ack'` (messageId)
- MessageQueue: `'enqueue'` (message), `'ack'` (messageId), `'compact'` (removedCount)

### Pattern 3: Object.freeze Enums (Project Convention)
**What:** Constants defined as frozen objects.
**Apply to:**
```javascript
// In protocol.js
export const CONTROL_TYPES = Object.freeze(new Set([
  MessageType.heartbeat,
  MessageType.heartbeat_ack,
  MessageType.msg_ack,
  MessageType.echo,
  MessageType.echo_reply,
]));

export function isControlMessage(type) {
  return CONTROL_TYPES.has(type);
}
```

### Pattern 4: Atomic File Write (Existing Pattern)
**What:** Write to temp file, rename to target.
**Source:** `atomicWrite()` in `james/logbook-watcher.js` -- uses DI for both writeFile and rename functions.
**Apply to:** MessageQueue WAL compaction (not append -- appends are inherently safe as partial last lines).

### Anti-Patterns to Avoid
- **Date.now() as sequence number:** Windows timer resolution is 15.6ms. Two messages in the same tick get identical sequences. Use integer counter starting at 0.
- **ACK-ing control messages:** Creates infinite ACK storm (Pitfall 19). Gate on `isControlMessage()` before any ACK logic.
- **writeFileSync for entire queue state:** Not atomic on crash. Use append-only WAL instead.
- **Global mutable sequence counter:** Each AckTracker instance must have its own counter. James and Bono each have independent sequence spaces.
- **Persisting sequence counter to WAL:** The sequence counter is per-session. On reconnect, replay from WAL (which has the actual messages). Don't conflate sequence numbers with WAL persistence.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Timer management for retry | Custom setInterval polling | Per-message setTimeout with clear-on-ACK | Each message has independent retry timing; polling wastes cycles checking all messages every tick |
| UUID generation | Custom ID scheme | `crypto.randomUUID()` | Already used in protocol.js, RFC 4122 compliant, collision-proof |
| JSON Lines parsing | Custom line parser | `content.split('\n').filter(Boolean).map(l => JSON.parse(l))` | JSON Lines is one-JSON-per-line, no special parser needed |
| Exponential backoff math | Custom calculation | `Math.min(baseMs * 2 ** attempt, maxMs)` | Same formula used in CommsClient reconnect (lines 164-167) |
| File locking | flock/advisory locks | Append-only WAL + single-writer design | Only one process writes the WAL (the daemon). No concurrent writers = no locking needed. |

**Key insight:** The entire queue and ACK system is standard messaging infrastructure at trivial scale (hundreds of messages, 2 nodes). Every component maps to a well-understood pattern. The value is in correct implementation of simple patterns, not in clever architecture.

## Common Pitfalls

### Pitfall 1: ACK Storm (Pitfall 19 -- CRITICAL)
**What goes wrong:** ACKs trigger further ACKs, creating infinite ping-pong at wire speed.
**Why it happens:** No distinction between data and control messages in the ACK logic.
**How to avoid:** Define `CONTROL_TYPES` Set in protocol.js. First line of receive handler: `if (isControlMessage(msg.type)) return;` before any ACK logic. `msg_ack` is a control type and MUST be in this set.
**Warning signs:** Messages-per-second spikes above normal rate. CPU pegged on JSON parse/serialize.

### Pitfall 2: Timestamp-as-Sequence (Pitfall 20 -- CRITICAL)
**What goes wrong:** Two messages sent in the same 15.6ms Windows tick get identical sequence numbers. Dedup cache treats them as the same message. One is silently dropped.
**Why it happens:** `Date.now()` resolution on Windows is 15.6ms (the OS timer tick).
**How to avoid:** Use `this.#seq++` (integer counter starting at 0). Never use timestamps as ordering primitives.
**Warning signs:** Messages occasionally "vanishing" -- sent but not processed on receiver.

### Pitfall 3: WAL Corruption on Crash (Pitfall 21 -- MODERATE)
**What goes wrong:** Process crashes mid-write to WAL file. Last line is partial JSON.
**Why it happens:** `fs.appendFile` is not atomic -- OS may flush partial data.
**How to avoid:** On WAL read, parse each line individually with try/catch. Discard any line that fails JSON.parse (it was the in-flight write at crash time). Only the last message is at risk, not the entire queue.
**Warning signs:** WAL read fails on startup. Queue starts empty unexpectedly.

### Pitfall 4: NTFS File Locking During Compaction (Pitfall 25 -- MODERATE)
**What goes wrong:** WAL compaction (rewrite without ACKed entries) uses write-tmp-then-rename. Windows Defender holds a lock on the original file, rename fails with EBUSY.
**How to avoid:** (a) Exclude the comms-link data directory from Defender real-time scanning. (b) Retry rename 3 times with 100ms delay. (c) Use `.wal` extension (not `.json`) to avoid Windows Search Indexer. (d) Use the existing `atomicWrite()` pattern from logbook-watcher.js which already handles DI for rename.
**Warning signs:** EBUSY errors in logs during compaction.

### Pitfall 5: Stale Retry Timers After ACK
**What goes wrong:** Message is ACKed, but the retry timer is not cleared. Timer fires, re-sends a message that was already acknowledged. Receiver processes it again.
**Why it happens:** ACK handler clears the message from the pending map but forgets to `clearTimeout` the associated timer.
**How to avoid:** Store timer reference alongside the message in the pending map. On acknowledge(), clear both the map entry AND the timer. Test this explicitly: track -> acknowledge -> assert no retry event fires.
**Warning signs:** Duplicate messages arriving at receiver after ACK was already sent.

### Pitfall 6: Memory Leak in Dedup Cache
**What goes wrong:** Dedup cache grows unbounded because TTL eviction never runs. After days of uptime, memory usage climbs.
**Why it happens:** TTL is checked on lookup but no background cleanup. Entries that are never looked up again stay forever.
**How to avoid:** Periodic cleanup sweep (every 60s) that iterates the Map and deletes expired entries. OR evict on insert when size exceeds 1000 (LRU-style). The 1000-entry cap is the safety net; the 1hr TTL is for correctness.
**Warning signs:** `process.memoryUsage().heapUsed` increasing steadily over days.

## Code Examples

Verified patterns from the existing codebase:

### Protocol Envelope Creation (Existing)
```javascript
// Source: shared/protocol.js (lines 35-44)
export function createMessage(type, from, payload = {}) {
  return JSON.stringify({
    v: PROTOCOL_VERSION,
    type,
    from,
    ts: Date.now(),
    id: randomUUID(),
    payload,
  });
}
```

### AckTracker API Shape (Recommended)
```javascript
// Based on existing DI patterns (watchdog.js, process-supervisor.js)
import { EventEmitter } from 'node:events';

export class AckTracker extends EventEmitter {
  #pending = new Map();      // messageId -> { rawMessage, seq, attempt, timer }
  #seq = 0;                  // Monotonic integer counter
  #sendFn;
  #nowFn;
  #timeoutMs;
  #maxRetries;

  constructor({ sendFn, nowFn = Date.now, timeoutMs = 10000, maxRetries = 3 }) {
    super();
    this.#sendFn = sendFn;
    this.#nowFn = nowFn;
    this.#timeoutMs = timeoutMs;
    this.#maxRetries = maxRetries;
  }

  /** Assign sequence number and begin tracking. Returns the sequence number. */
  track(messageId, rawMessage) { ... }

  /** Mark message as delivered, clear retry timer. */
  acknowledge(messageId) { ... }

  /** Get all pending (unACKed) messages for reconnect replay. */
  getPendingMessages() { ... }

  /** Clear all pending on intentional disconnect. */
  reset() { ... }

  get pendingCount() { return this.#pending.size; }
  get currentSeq() { return this.#seq; }
}
```

### MessageQueue API Shape (Recommended)
```javascript
// Based on existing patterns + WAL design from ARCHITECTURE.md
export class MessageQueue {
  #entries = [];             // In-memory queue
  #walPath;
  #appendFileFn;
  #readFileFn;
  #writeFileFn;
  #nowFn;
  #maxSize;

  constructor({ storePath, appendFileFn, readFileFn, writeFileFn, nowFn = Date.now, maxSize = 1000 }) {
    this.#walPath = storePath;
    this.#appendFileFn = appendFileFn;
    this.#readFileFn = readFileFn;
    this.#writeFileFn = writeFileFn;
    this.#nowFn = nowFn;
    this.#maxSize = maxSize;
  }

  /** Load WAL from disk. Call once on startup. */
  async load() { ... }

  /** Append message to WAL and in-memory queue. Returns entry ID. */
  async enqueue(message) { ... }

  /** Mark message as ACKed. Removes from in-memory queue. */
  acknowledge(messageId) { ... }

  /** Get all unACKed messages (for reconnect replay). */
  getPending() { ... }

  /** Rewrite WAL without ACKed entries. */
  async compact() { ... }

  get size() { return this.#entries.filter(e => !e.acked).length; }
}
```

### JSON Lines WAL Format (Recommended)
```
{"id":"abc-123","ts":1710900000000,"type":"task_request","payload":{...},"acked":false}
{"id":"def-456","ts":1710900001000,"type":"exec_request","payload":{...},"acked":false}
{"id":"abc-123","acked":true}
```
Each line is a complete JSON object. Append-only: enqueue appends a message line, acknowledge appends an `{"id":"...","acked":true}` line. Compaction rewrites the file excluding ACKed entries. Partial last line on crash is safely discarded during load.

### Test Pattern (Existing Convention)
```javascript
// Source: test/process-supervisor.test.js (Phase 10 pattern)
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

function makeSupervisor(overrides = {}) {
  return new ProcessSupervisor({
    healthCheckFn: overrides.healthCheckFn ?? (async () => true),
    killFn: overrides.killFn ?? noopKill,
    spawnFn: overrides.spawnFn ?? noopSpawn,
    pollMs: overrides.pollMs ?? 100,
    nowFn: overrides.nowFn ?? Date.now,
    ...overrides,
  });
}
```

Apply the same pattern for `makeAckTracker(overrides)` and `makeQueue(overrides)`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| INBOX.md appendFileSync | WAL-backed MessageQueue | This phase | Crash-resilient, no git race conditions |
| Fire-and-forget messages | ACK-tracked with retry | This phase | Delivery confirmation for data messages |
| In-memory queue only (CommsClient) | In-memory + WAL persistence | This phase | Survives process crashes |
| No deduplication | Seen-message cache (1000 IDs, 1hr TTL) | This phase | Safe reconnect replay without double-processing |

**Deprecated/outdated:**
- `INBOX.md` programmatic reads: Demoted to audit log only in Phase 11 (TQ-05). This phase builds the replacement but does not wire it in.
- `Date.now()` for ordering: Must not be used as sequence numbers. Only for timestamps (metadata).

## Open Questions

1. **Sequence number persistence across clean shutdown**
   - What we know: Per-session sequence numbers reset on reconnect. WAL handles crash recovery (replay all unACKed).
   - What's unclear: Should we persist the high-water sequence number so the receiver's dedup cache doesn't have stale entries after a clean restart?
   - Recommendation: No. On clean shutdown, all messages should be ACKed (WAL empty). On reconnect after clean restart, start sequences from 0. The dedup cache clears on restart too. Only crash recovery needs the WAL.

2. **WAL compaction trigger**
   - What we know: Compaction rewrites WAL without ACKed entries.
   - What's unclear: When to trigger? After every N ACKs? On a timer? On startup only?
   - Recommendation: Compact on startup (always clean state) + when ACKed ratio exceeds 50% of total entries. Keep it simple.

3. **AckTracker timer strategy: setTimeout per message vs polling**
   - What we know: Each tracked message needs independent retry timing.
   - What's unclear: Is per-message setTimeout expensive with 10+ concurrent tracked messages?
   - Recommendation: Use per-message setTimeout. At this scale (max ~10 concurrent), timer overhead is negligible. Polling would add unnecessary complexity and latency. The `clearTimeout` on ACK prevents timer accumulation.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node 22.14.0) |
| Config file | None (uses `node --test test/*.test.js` from package.json) |
| Quick run command | `node --test test/ack-tracker.test.js test/message-queue.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REL-01 | Monotonic sequence number assignment | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| REL-02 | msg_ack sent on receipt of data message | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| REL-03 | Exponential backoff retry, 3 max | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| REL-04 | Reconnect replay of unACKed messages | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| REL-05 | Dedup cache (1000 IDs, 1hr TTL) | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| REL-06 | Control messages excluded from ACK | unit | `node --test test/ack-tracker.test.js -x` | No -- Wave 0 |
| TQ-01 | WAL persistence before send | unit | `node --test test/message-queue.test.js -x` | No -- Wave 0 |
| TQ-02 | Compaction removes ACKed entries | unit | `node --test test/message-queue.test.js -x` | No -- Wave 0 |
| TQ-03 | Crash recovery loads unACKed from WAL | unit | `node --test test/message-queue.test.js -x` | No -- Wave 0 |
| TQ-04 | Partial last line handled gracefully | unit | `node --test test/message-queue.test.js -x` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/ack-tracker.test.js test/message-queue.test.js test/protocol.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/ack-tracker.test.js` -- covers REL-01 through REL-06
- [ ] `test/message-queue.test.js` -- covers TQ-01 through TQ-04
- [ ] `test/protocol.test.js` -- extend with msg_ack type + isControlMessage() tests
- [ ] No framework install needed -- node:test is built-in

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis -- all comms-link v1.0 source files (shared/protocol.js, james/comms-client.js, james/watchdog.js, james/process-supervisor.js, james/logbook-watcher.js, test/*.test.js)
- Node.js 22.14.0 built-in APIs -- fs/promises, events, crypto, test
- Project research -- .planning/research/ARCHITECTURE.md, FEATURES.md, PITFALLS.md, SUMMARY.md (all HIGH confidence)
- Existing test suite -- 23 test files, 222+ tests confirming DI and EventEmitter conventions

### Secondary (MEDIUM confidence)
- NATS ACK patterns -- at-least-once delivery, sequence-based replay
- TCP ACK protocol -- exponential backoff, duplicate detection, sequence numbers
- npm CLI issue #9021 -- NTFS EBUSY evidence for file rename operations

### Tertiary (LOW confidence)
- None -- all findings are verified against codebase or primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all Node.js built-ins verified on Node 22.14.0
- Architecture: HIGH -- patterns directly derived from codebase analysis of 6+ existing components
- Pitfalls: HIGH -- all critical pitfalls have real-world evidence (Windows timer resolution, NTFS locking, v1.0 production incidents)

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, no fast-moving dependencies)
