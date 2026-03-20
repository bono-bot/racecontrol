# Phase 11: Reliable Delivery Wiring - Research

**Researched:** 2026-03-20
**Domain:** WebSocket message routing with ACK tracking, durable queue integration, and bidirectional task request/response
**Confidence:** HIGH

## Summary

Phase 11 wires the already-built AckTracker (Phase 9 Plan 1) and MessageQueue (Phase 9 Plan 2) into the live WebSocket message flow on both James and Bono sides. This is an integration phase, not a library-building phase -- the foundation modules exist with 46 passing tests. The work is: (1) James sends data messages through AckTracker instead of fire-and-forget, (2) Bono auto-sends msg_ack for tracked message types, (3) MessageQueue replaces appendFileSync/appendFile for INBOX.md writes, (4) both sides can initiate structured task_request with correlation IDs and configurable timeout, and (5) INBOX.md becomes a write-only human-readable audit log.

The critical constraint is **deploy order**: Bono must be updated first because sending msg_ack for messages that don't expect ACKs is harmless (the receiver simply ignores unknown message types), but James sending messages that expect ACKs to an old Bono that doesn't send them would trigger unnecessary retries and timeout events. The codebase already handles unknown message types gracefully -- they are silently dropped in parseMessage consumers.

**Primary recommendation:** Wire AckTracker + MessageQueue into james/index.js and bono/index.js using the existing DI and EventEmitter patterns. Add a DeduplicatorCache on the receiver side. Keep the wiring additive -- existing message handlers remain unchanged, new handlers are added alongside them.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TQ-05 | INBOX.md is demoted to human-readable audit log only (never read programmatically) | Replace appendFileSync(inboxPath, ...) in james/index.js lines 131-136, 146-151 with MessageQueue.enqueue(). Replace appendFile(inboxPath, ...) in bono/index.js lines 49-55. Keep INBOX.md append as audit-only side effect. |
| BDR-01 | Both James and Bono can initiate structured task requests with correlation IDs | Extend existing task_request/task_response handling in both sides. Add correlation ID (taskId) to all task messages. Wire AckTracker to track task_request messages for reliable delivery. |
| BDR-02 | Task responses are routed back to originator via reply_to correlation | Use taskId as correlation key. Bono's wireBono() already sends task_response with taskId; James's handler already sends task_response with taskId. Wire AckTracker to confirm delivery of responses. |
| BDR-03 | Unanswered task requests time out after configurable period (default 5 minutes) | AckTracker already has timeoutMs + maxRetries with exponential backoff. Configure appropriately: 5-minute total window. AckTracker emits 'timeout' event when all retries exhausted -- wire this to log/alert. |
</phase_requirements>

## Standard Stack

### Core (already installed -- no new dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ws | ^8.19.0 | WebSocket transport | Already in use, unchanged |
| node:events | built-in | EventEmitter for component communication | Existing pattern in all modules |
| node:crypto | built-in | randomUUID for message IDs | Already used in protocol.js |
| node:fs | built-in | appendFile for audit log writes | Already used for INBOX.md |
| node:fs/promises | built-in | Async file ops for MessageQueue WAL | Already used by MessageQueue DI |

### Project Modules (already built in Phase 9)
| Module | Purpose | API Surface |
|--------|---------|-------------|
| shared/ack-tracker.js | Track ACKs, retry with backoff, reconnect replay | AckTracker: track(), acknowledge(), getPendingMessages(), reset() |
| shared/ack-tracker.js | Deduplicate received messages | DeduplicatorCache: record(), isDuplicate(), cleanup() |
| shared/message-queue.js | Durable WAL-backed message queue | MessageQueue: enqueue(), acknowledge(), compact(), load(), getPending() |
| shared/protocol.js | Message types and envelope | msg_ack, CONTROL_TYPES, isControlMessage(), createMessage(), parseMessage() |

**Installation:** None needed -- zero new dependencies.

## Architecture Patterns

### Current Message Flow (v1.0 -- fire-and-forget)
```
James sends task_request → Bono receives → Bono sends task_response → done
                           (no delivery confirmation, no retry, appendFileSync to INBOX.md)
```

### Target Message Flow (Phase 11 -- reliable delivery)
```
James sends task_request → AckTracker.track(id, raw, 'task_request')
                         → sendFn(raw) over WebSocket
                         → Bono receives → DeduplicatorCache.isDuplicate(id)?
                           NO → process message + auto-send msg_ack {ackId: id}
                           YES → discard + still send msg_ack
                         → James receives msg_ack → AckTracker.acknowledge(id)

If no ACK within timeout → AckTracker retries (exponential backoff, 3 max)
If all retries fail → AckTracker emits 'timeout' → log error, optionally alert
```

### Integration Points (what gets modified)

**james/index.js modifications:**
1. Import AckTracker, DeduplicatorCache, MessageQueue, isControlMessage
2. Instantiate AckTracker with sendFn wrapping client's raw WebSocket send
3. Instantiate DeduplicatorCache for incoming message dedup
4. Instantiate MessageQueue with WAL path for durable storage
5. Add msg_ack handler: `ackTracker.acknowledge(msg.payload.ackId)`
6. Wrap outgoing task_request/task_response/message sends through ackTracker.track()
7. Add dedup guard at top of message handler: skip if isDuplicate(msg.id)
8. Replace appendFileSync(inboxPath, ...) with messageQueue.enqueue() + audit log append
9. On reconnect ('open' event): replay ackTracker.getPendingMessages()
10. Add HTTP relay routes: GET /relay/queue/peek, POST /relay/queue/ack

**bono/index.js modifications to wireBono():**
1. Accept ackTracker and deduplicator as new deps (DI pattern)
2. Auto-send msg_ack for non-control incoming messages that have an id
3. Add dedup guard at top of message handler
4. Replace appendFile(inboxPath, ...) with queue enqueue + audit log append
5. Wire ackTracker.acknowledge() for msg_ack received from James

**Key constraint -- CommsClient.send() API:**
CommsClient.send(type, payload) creates the message envelope internally (calls createMessage). But AckTracker needs the raw message string (for resend) AND the message ID (for tracking). This means either:
- Option A: AckTracker wraps above client.send() -- but can't get the ID back since createMessage is called inside send()
- Option B: Call createMessage() directly, pass raw string to AckTracker.track(), and use a lower-level ws.send() for actual transmission
- Option C: Add a sendRaw(rawString) method to CommsClient that bypasses createMessage

**Recommendation: Option C** -- add a `sendRaw(rawMessage)` method to CommsClient. The existing `send(type, payload)` remains for backward compatibility. AckTracker's sendFn uses sendRaw. This is the cleanest integration because:
- createMessage() is called once by the caller, not hidden inside CommsClient
- The raw message string is available for both tracking and resending
- The existing offline queue in CommsClient still works for sendRaw (queue the raw string)
- No changes to existing callers of client.send()

### Recommended Project Structure (no new files)
```
shared/
  ack-tracker.js        # EXISTS (Phase 9) -- AckTracker + DeduplicatorCache
  message-queue.js      # EXISTS (Phase 9) -- MessageQueue with WAL
  protocol.js           # EXISTS -- msg_ack, CONTROL_TYPES (no changes needed)
james/
  index.js              # MODIFY -- wire AckTracker, MessageQueue, dedup
  comms-client.js       # MODIFY -- add sendRaw() method
bono/
  index.js              # MODIFY -- wire ACK auto-send, dedup in wireBono()
test/
  reliable-delivery.test.js  # NEW -- integration tests for ACK wiring
```

### Pattern: DI Wiring (mandatory -- follows wireBono/wireRunner convention)
```javascript
// In bono/index.js wireBono():
export function wireBono({ wss, monitor, alertManager, accumulator, scheduler,
                           inboxPath, ackTracker, deduplicator, messageQueue }) {
  // New deps are optional for backward compatibility
  // Existing tests pass without ackTracker/deduplicator/messageQueue

  wss.on('message', (msg, ws) => {
    // Dedup guard (new)
    if (deduplicator && msg.id && deduplicator.isDuplicate(msg.id)) {
      // Still send ACK for deduped messages (sender needs confirmation)
      if (ackTracker && !isControlMessage(msg.type)) {
        ws.send(createMessage('msg_ack', 'bono', { ackId: msg.id }));
      }
      return;
    }
    if (deduplicator && msg.id) deduplicator.record(msg.id);

    // Auto-ACK for non-control messages (new)
    if (!isControlMessage(msg.type) && msg.id) {
      ws.send(createMessage('msg_ack', 'bono', { ackId: msg.id }));
    }

    // ... existing handlers unchanged ...
  });
}
```

### Pattern: AckTracker Wiring on James Side
```javascript
// In james/index.js:
import { AckTracker, DeduplicatorCache } from '../shared/ack-tracker.js';
import { MessageQueue } from '../shared/message-queue.js';
import { isControlMessage } from '../shared/protocol.js';

const deduplicator = new DeduplicatorCache();
const messageQueue = new MessageQueue({
  storePath: walPath,
  appendFileFn: (p, d) => appendFile(p, d, 'utf8'),
  readFileFn: (p) => readFile(p, 'utf8'),
  writeFileFn: (p, d) => writeFile(p, d, 'utf8'),
});

// AckTracker sendFn uses client.sendRaw() (new method)
const ackTracker = new AckTracker({
  sendFn: (raw) => client.sendRaw(raw),
  timeoutMs: 10000,
  maxRetries: 3,
});

// Handle incoming msg_ack
client.on('message', (msg) => {
  if (msg.type === 'msg_ack') {
    ackTracker.acknowledge(msg.payload.ackId);
    return;
  }

  // Dedup guard
  if (msg.id && deduplicator.isDuplicate(msg.id)) return;
  if (msg.id) deduplicator.record(msg.id);

  // ... existing handlers ...
});

// On reconnect: replay pending messages
client.on('open', () => {
  for (const raw of ackTracker.getPendingMessages()) {
    client.sendRaw(raw);
  }
});
```

### Pattern: Task Request with Timeout (BDR-03)
```javascript
// AckTracker already handles retry + timeout via exponential backoff.
// For 5-minute total timeout with 3 retries:
//   timeoutMs = 60000 (1 min), maxRetries = 3
//   Retry schedule: 60s, 120s, 240s = 420s total (~7 min)
//   OR: timeoutMs = 30000, maxRetries = 3 → 30s, 60s, 120s = 210s (~3.5 min)
//
// Better approach: Use AckTracker for delivery confirmation (fast, ~10s timeout),
// and a SEPARATE task-level timeout for response (5 min).
// Delivery ACK confirms the message arrived.
// Task timeout is application-level: "did they respond to my request?"

// Task timeout is separate from delivery ACK:
function sendTaskRequest(client, ackTracker, payload, timeoutMs = 300000) {
  const taskId = payload.taskId || randomUUID();
  const raw = createMessage('task_request', 'james', { ...payload, taskId });
  const parsed = JSON.parse(raw);

  ackTracker.track(parsed.id, raw, 'task_request');  // delivery tracking

  // Application-level task timeout (5 min default)
  const timer = setTimeout(() => {
    pendingTasks.delete(taskId);
    console.warn(`[TASK] Request ${taskId} timed out after ${timeoutMs}ms`);
  }, timeoutMs);

  pendingTasks.set(taskId, { timer, resolve: null });
  return taskId;
}

// When task_response arrives:
if (msg.type === 'task_response') {
  const pending = pendingTasks.get(msg.payload.taskId);
  if (pending) {
    clearTimeout(pending.timer);
    pendingTasks.delete(msg.payload.taskId);
    // Process response
  }
}
```

### Anti-Patterns to Avoid
- **ACK-ing ACKs:** msg_ack is in CONTROL_TYPES -- AckTracker.track() throws if you try to track a control message. This prevents ACK storms by design (Phase 9 decision).
- **Double message creation:** Don't call createMessage() inside client.send() AND also in the caller. Use sendRaw() for tracked messages.
- **Breaking existing tests:** wireBono() new params must be optional. All 307 existing tests must pass without modification.
- **Mixing delivery ACK with task response:** msg_ack confirms the WebSocket message arrived. task_response is the application-level answer. These are separate concerns with separate timeouts.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Retry with backoff | Custom setTimeout chains | AckTracker (Phase 9) | Already built, tested (19 tests), handles edge cases |
| Message deduplication | Manual Set/Map tracking | DeduplicatorCache (Phase 9) | TTL + size eviction already implemented |
| Durable queue | appendFileSync to INBOX.md | MessageQueue WAL (Phase 9) | Crash recovery, compaction, proper enqueue/ack semantics |
| Correlation ID generation | Custom counter | randomUUID from node:crypto | Already used throughout protocol.js |

**Key insight:** Phase 11 should import and wire -- not reimplement. Every reliability primitive already exists from Phase 9 with full test coverage.

## Common Pitfalls

### Pitfall 1: CommsClient.send() Hides Message ID
**What goes wrong:** AckTracker needs the message ID to track, but CommsClient.send() generates the ID internally via createMessage() and doesn't return it.
**Why it happens:** v1.0 fire-and-forget design didn't need to expose IDs.
**How to avoid:** Add sendRaw(rawString) to CommsClient. Caller creates message with createMessage(), extracts ID, passes raw string to both AckTracker.track() and client.sendRaw().
**Warning signs:** If you find yourself parsing the return value of send() to get the ID, the API is wrong.

### Pitfall 2: Reconnect Replay Duplicates with Queue Flush
**What goes wrong:** CommsClient already has an offline queue (#queue) that flushes on reconnect. AckTracker also has getPendingMessages() for replay. Both fire on 'open' event, causing duplicate sends.
**Why it happens:** Two independent replay mechanisms competing.
**How to avoid:** For tracked messages (non-control types), use ONLY AckTracker replay. The CommsClient offline queue should only hold messages sent via the old send() path (control messages like heartbeat). Tracked messages go through AckTracker, which handles its own replay.
**Warning signs:** Same message ID appearing twice in Bono's logs on reconnect.

### Pitfall 3: Backward Compatibility During Rolling Deploy
**What goes wrong:** James sends messages expecting ACKs, but Bono hasn't been updated yet and doesn't send msg_ack. AckTracker retries 3 times then fires timeout for every message.
**Why it happens:** Both sides need coordinated changes.
**How to avoid:** Deploy Bono first (sending msg_ack is harmless if James isn't tracking yet). Then deploy James. This is already the documented deploy order in STATE.md.
**Warning signs:** Sudden spike of 'timeout' events from AckTracker after James deploy.

### Pitfall 4: INBOX.md Still Being Read Programmatically
**What goes wrong:** Some code path still reads INBOX.md expecting structured data, but it's now an audit log with inconsistent format.
**Why it happens:** Forgot to audit all INBOX.md read paths.
**How to avoid:** Search entire codebase for any readFile/readFileSync of INBOX_PATH. After Phase 11, INBOX.md should only be written to (append), never read by code. The MessageQueue WAL replaces programmatic reading.
**Warning signs:** Grep for `readFile.*inbox` or `readFileSync.*inbox` in the codebase.

### Pitfall 5: AckTracker sendFn Closure Captures Stale WebSocket
**What goes wrong:** AckTracker's sendFn captures a reference to the WebSocket that may be closed/reconnected.
**Why it happens:** CommsClient replaces the internal #ws reference on reconnect, but the sendFn closure may point to the old one.
**How to avoid:** sendFn should call client.sendRaw() (which internally uses the current #ws), not directly reference ws.send(). The indirection through CommsClient ensures the current connection is always used.
**Warning signs:** Retries silently failing with "WebSocket is not open" errors.

## Code Examples

### CommsClient.sendRaw() Addition
```javascript
// In james/comms-client.js -- add alongside existing send():
/**
 * Send a pre-serialized raw message string.
 * Used by AckTracker for retry/replay (message already created via createMessage).
 * @param {string} rawMessage - JSON string from createMessage()
 * @returns {boolean} true if sent, false if queued
 */
sendRaw(rawMessage) {
  if (this.sm.state === State.CONNECTED && this.#ws?.readyState === WebSocket.OPEN) {
    this.#ws.send(rawMessage);
    return true;
  }

  if (this.#queue.length >= this.#maxQueueSize) {
    this.#queue.shift();
  }
  this.#queue.push(rawMessage);
  return false;
}
```

### INBOX.md Audit Log Write (replacing programmatic writes)
```javascript
// Audit log helper -- write-only, human-readable, never read by code
function appendAuditLog(inboxPath, msg) {
  if (!inboxPath) return;
  try {
    const ts = new Date(msg.ts).toISOString();
    const entry = `\n## ${ts} -- from ${msg.from}\n**Type:** ${msg.type}\n${JSON.stringify(msg.payload, null, 2)}\n`;
    appendFileSync(inboxPath, entry, 'utf8');
  } catch (err) {
    console.error(`[AUDIT] Failed to write: ${err.message}`);
  }
}
```

### Wiring AckTracker Timeout to Logging
```javascript
ackTracker.on('timeout', (messageId) => {
  console.error(`[ACK] Message ${messageId} timed out after all retries`);
  // Optionally: persist to queue for next session, or alert
});

ackTracker.on('retry', ({ messageId, attempt }) => {
  console.warn(`[ACK] Retrying message ${messageId} (attempt ${attempt})`);
});
```

## State of the Art

| Old Approach (v1.0) | New Approach (Phase 11) | Impact |
|---------------------|------------------------|--------|
| appendFileSync to INBOX.md | MessageQueue.enqueue() + audit log | Crash-safe, no git races, proper ACK semantics |
| Fire-and-forget client.send() | AckTracker.track() + client.sendRaw() | Delivery confirmation, auto-retry, reconnect replay |
| No dedup on receiver | DeduplicatorCache on both sides | Safe reconnect replay without double-processing |
| task_request with immediate ack | task_request with delivery ACK + application timeout | Two-layer reliability: transport + application |
| INBOX.md as IPC mechanism | INBOX.md as human-readable audit log | Clean separation of machine-readable (WAL) from human-readable (audit) |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node 22.14.0) |
| Config file | none (convention: test/*.test.js) |
| Quick run command | `node --test test/reliable-delivery.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TQ-05 | INBOX.md never read programmatically, only appended as audit | unit | `node --test test/reliable-delivery.test.js` | Wave 0 |
| BDR-01 | Both sides initiate task_request with correlation ID + ACK tracking | unit | `node --test test/reliable-delivery.test.js` | Wave 0 |
| BDR-02 | task_response routed back via taskId correlation | unit | `node --test test/reliable-delivery.test.js` | Wave 0 |
| BDR-03 | Unanswered task requests time out after configurable period | unit | `node --test test/reliable-delivery.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/reliable-delivery.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (307+ tests) before /gsd:verify-work

### Wave 0 Gaps
- [ ] `test/reliable-delivery.test.js` -- covers TQ-05, BDR-01, BDR-02, BDR-03
- [ ] May need `test/comms-client.test.js` updates if sendRaw() is added to CommsClient

## Open Questions

1. **CommsClient offline queue vs AckTracker replay**
   - What we know: Both have replay mechanisms. They must not conflict.
   - What's unclear: Should tracked messages bypass CommsClient's #queue entirely?
   - Recommendation: sendRaw() should still use the offline queue (for when WS is down), but on reconnect, flush the offline queue FIRST, then replay AckTracker pending messages. AckTracker entries that were already in the offline queue will be deduped by the receiver's DeduplicatorCache.

2. **MessageQueue role in Phase 11**
   - What we know: MessageQueue was built for durable persistence. INBOX.md is being demoted.
   - What's unclear: Does Phase 11 use MessageQueue as the primary storage for incoming messages (replacing INBOX.md reads), or is it used for outgoing message durability?
   - Recommendation: Use MessageQueue for INCOMING messages on James side (replaces INBOX.md as machine-readable store). Claude Code reads from queue via HTTP /relay/queue/peek. Outgoing message durability is handled by AckTracker + CommsClient offline queue. This gives a clean separation: MessageQueue = inbox, AckTracker = outbox.

3. **WAL path configuration**
   - What we know: MessageQueue needs a storePath.
   - Recommendation: Use environment variable QUEUE_WAL_PATH with default `./data/message-queue.wal`. Create data/ directory on startup if missing.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis: shared/ack-tracker.js (207 lines), shared/message-queue.js (180 lines), shared/protocol.js (88 lines)
- Direct codebase analysis: james/index.js (277 lines), bono/index.js (372 lines), james/comms-client.js (186 lines)
- Phase 9 summaries: 09-01-SUMMARY.md (AckTracker, 19 tests), 09-02-SUMMARY.md (MessageQueue, 20 tests)
- Test suite: 25 test files, 307 tests all passing (verified 2026-03-20)
- Architecture research: .planning/research/ARCHITECTURE.md (full component map and data flows)

### Secondary (MEDIUM confidence)
- Project research summary: .planning/research/SUMMARY.md (pitfall analysis, build order rationale)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all modules already built and tested
- Architecture: HIGH -- integration points identified by reading actual source code, not speculation
- Pitfalls: HIGH -- based on direct analysis of CommsClient internals and existing replay mechanisms

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable -- no external dependency changes expected)
