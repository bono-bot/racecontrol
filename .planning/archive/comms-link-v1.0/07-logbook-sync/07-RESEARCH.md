# Phase 7: LOGBOOK Sync - Research

**Researched:** 2026-03-12
**Domain:** File synchronization over WebSocket with conflict detection
**Confidence:** HIGH

## Summary

Phase 7 implements bidirectional LOGBOOK.md synchronization between James and Bono over the existing WebSocket infrastructure. The file (~22KB, 270 lines) is polled every 5 seconds via SHA-256 hash comparison. On change, the full file content is sent as a `file_sync` WebSocket message. The receiving side writes atomically (temp file + rename) and acknowledges with `file_ack`. Conflict detection handles the rare case where both sides modify the file between sync cycles.

All required infrastructure already exists: `file_sync` and `file_ack` message types are defined in `shared/protocol.js`, `CommsClient.send()` queues during disconnection and replays on reconnect, and the DI + EventEmitter patterns are well-established across all prior phases. No external libraries are needed -- only Node.js built-in modules (`node:crypto`, `node:fs/promises`, `node:path`).

**Primary recommendation:** Build two classes -- `LogbookWatcher` (used by both sides, polls + hashes + sends) and `LogbookSyncer` (orchestrates watcher + incoming message handling + conflict detection). Wire into `james/watchdog-runner.js` and `bono/index.js` using the existing DI patterns.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Hardcoded path on James's side: `~/racingpoint/racecontrol/LOGBOOK.md`
- Two-way sync: both sides watch their local file and push changes
- No git commit on receive -- just write the file
- Poll every 5 seconds using file hash comparison (SHA-256)
- Send full file content every time (not diffs)
- Require `file_ack` before accepting the next sync
- Suppress file watcher during write to prevent echo loops (set 'writing' flag, write, wait one poll cycle, clear flag)
- Auto-merge when both sides appended new rows -- combine rows sorted by timestamp
- Flag as conflict if changes aren't purely appending new rows
- Both sides send `file_sync` with current hash on reconnect -- if hashes differ, conflict/merge logic kicks in
- File watcher keeps running while disconnected -- tracks latest hash so reconnect sync is immediate
- Use CommsClient's existing message queue for `file_sync` during disconnection -- replays on reconnect

### Claude's Discretion
- Bono's filesystem path for LOGBOOK.md
- Conflict notification level (log warning vs email)
- Whether to deduplicate queued file_sync messages (only send latest version on reconnect)
- Hash algorithm details (SHA-256 recommended but flexible)
- Exact auto-merge implementation for append detection

### Deferred Ideas (OUT OF SCOPE)
- Sync additional shared files beyond LOGBOOK.md (AS-01, v2)
- Bidirectional git sync (AS-02, v2)
- Conflict resolution UI for Uday -- just flag + .conflict file for now

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LS-01 | Watch LOGBOOK.md for changes using file hash comparison (not git-based) | SHA-256 via `node:crypto.createHash()`, poll with `setInterval`, compare hex digests |
| LS-02 | Sync full file content over WebSocket on change detection | `CommsClient.send('file_sync', { hash, content, timestamp })` -- protocol types already defined |
| LS-03 | Atomic writes on receiving side (write to temp file, then rename) | `fs.writeFile(path + '.tmp')` then `fs.rename()` -- verified working on Windows Node.js 22 |
| LS-04 | Conflict detection when both sides modified since last sync | Track `lastSyncedHash` per side; if incoming hash differs from expected, both modified |
| LS-05 | Both AIs always have current LOGBOOK.md within 30 seconds of a change | 5s poll interval + WebSocket latency + ack round-trip is well under 30s |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `node:crypto` | built-in (Node 22) | SHA-256 file hashing | Zero-dependency, `createHash('sha256').update(content).digest('hex')` |
| `node:fs/promises` | built-in (Node 22) | Async file read, write, rename | Promise-based API, `rename()` is atomic on same filesystem |
| `node:path` | built-in (Node 22) | Path manipulation for temp file + target | Cross-platform path joining |
| `ws` | ^8.19.0 | WebSocket transport | Already a project dependency, used by CommsClient |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `node:events` | built-in | EventEmitter for lifecycle events | LogbookWatcher emits `changed`, `synced`, `conflict` events |
| `node:test` | built-in | Test runner | 134 existing tests use this pattern |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| SHA-256 polling | `fs.watch()` / `chokidar` | `fs.watch` is unreliable on Windows (double-fires, misses events). Polling is deterministic and the 5s interval is a locked decision |
| Full file transfer | Diff/patch | Overkill for ~22KB file. Full content is simpler and always consistent (locked decision) |
| CRDTs | Append-only merge | Explicitly out of scope in REQUIREMENTS.md |

**Installation:**
```bash
# No new dependencies needed -- all built-in Node.js modules
```

## Architecture Patterns

### Recommended Project Structure
```
james/
  logbook-watcher.js      # LogbookWatcher class (polls, hashes, detects changes)
bono/
  (no new files -- LogbookWatcher is symmetric, import from james/)
shared/
  logbook-merge.js         # Pure functions: detectAppend(), mergeAppendOnly(), writeConflictFile()
  protocol.js              # Already has file_sync, file_ack types (no changes needed)
test/
  logbook-watcher.test.js  # Unit tests for LogbookWatcher
  logbook-merge.test.js    # Unit tests for merge/conflict logic
  logbook-sync.test.js     # Integration: watcher + incoming messages + ack flow
```

### Pattern 1: LogbookWatcher (Poll + Hash + Send)
**What:** A class that polls a file path every N ms, computes SHA-256, and emits `changed` when the hash differs from last known. Accepts DI for all I/O.
**When to use:** Both James and Bono instantiate this with their respective file paths.
**Example:**
```javascript
// Source: Project convention from ClaudeWatchdog, HeartbeatSender patterns
import { EventEmitter } from 'node:events';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

export class LogbookWatcher extends EventEmitter {
  #filePath;
  #pollMs;
  #interval = null;
  #lastHash = null;
  #writing = false;  // suppress echo during atomic write
  #readFileFn;       // DI for testing

  constructor({ filePath, pollMs = 5000, readFileFn = null }) {
    super();
    this.#filePath = filePath;
    this.#pollMs = pollMs;
    this.#readFileFn = readFileFn || ((p) => readFile(p, 'utf8'));
  }

  async #poll() {
    if (this.#writing) return;  // suppress during write
    try {
      const content = await this.#readFileFn(this.#filePath);
      const hash = createHash('sha256').update(content).digest('hex');
      if (this.#lastHash !== null && hash !== this.#lastHash) {
        this.emit('changed', { hash, content, timestamp: Date.now() });
      }
      this.#lastHash = hash;
    } catch (err) {
      // File not found or read error -- skip this cycle
      this.emit('error', err);
    }
  }

  get lastHash() { return this.#lastHash; }

  /** Call before writing received content to suppress echo detection */
  suppressNextCycle() { this.#writing = true; }

  /** Call after write + one poll cycle to re-enable detection */
  resumeDetection(newHash) {
    this.#lastHash = newHash;
    this.#writing = false;
  }

  start() {
    this.stop();
    this.#poll();  // immediate first poll
    this.#interval = setInterval(() => this.#poll(), this.#pollMs);
  }

  stop() {
    if (this.#interval !== null) {
      clearInterval(this.#interval);
      this.#interval = null;
    }
  }
}
```

### Pattern 2: Atomic Write with Echo Suppression
**What:** Write received content to a temp file, rename to target, update watcher's hash to prevent echo.
**When to use:** On receiving a `file_sync` message from the other side.
**Example:**
```javascript
// Source: Verified on Windows Node.js 22 (see research validation)
import { writeFile, rename } from 'node:fs/promises';

async function atomicWrite(targetPath, content) {
  const tmpPath = targetPath + '.tmp';
  await writeFile(tmpPath, content, 'utf8');
  await rename(tmpPath, targetPath);
}

// Usage in message handler:
async function handleFileSync(msg, watcher, sendAckFn) {
  const { hash, content, timestamp } = msg.payload;
  watcher.suppressNextCycle();
  await atomicWrite(watcher.filePath, content);
  watcher.resumeDetection(hash);
  sendAckFn({ hash, timestamp: Date.now() });
}
```

### Pattern 3: Append-Only Merge Detection
**What:** Determine if changes between two versions are purely new rows appended at the end of date sections.
**When to use:** When both sides have modified LOGBOOK.md since last sync (conflict scenario).
**Example:**
```javascript
// Source: LOGBOOK.md structure analysis
// Format: date header sections with | timestamp | author | commit | summary | rows

function detectAppend(baseContent, newContent) {
  const baseLines = baseContent.split('\n');
  const newLines = newContent.split('\n');

  // If new content starts with all of base content, it's a pure append
  if (newLines.length < baseLines.length) return false;

  for (let i = 0; i < baseLines.length; i++) {
    if (baseLines[i] !== newLines[i]) return false;
  }
  return true;  // All base lines match, new lines are appended
}
```

### Pattern 4: Reconnect Hash Exchange
**What:** On WebSocket reconnect, both sides send their current file hash. If they differ, trigger sync/merge.
**When to use:** On CommsClient `open` event (reconnection).
**Example:**
```javascript
// Source: CONTEXT.md locked decision + CommsClient 'open' event pattern
client.on('open', () => {
  // Send current hash for comparison
  client.send('file_sync', {
    hash: watcher.lastHash,
    content: currentContent,
    timestamp: Date.now(),
  });
});
```

### Pattern 5: DI Wiring (following wireRunner/wireBono pattern)
**What:** Export a wiring function that connects LogbookWatcher to CommsClient message routing.
**When to use:** In production entry points (`james/watchdog-runner.js`, `bono/index.js`).
**Example:**
```javascript
// Source: wireRunner() pattern from james/watchdog-runner.js
export function wireLogbook({ watcher, client, filePath, writeFn, sendAckFn }) {
  // Outbound: local change detected -> send to other side
  watcher.on('changed', ({ hash, content, timestamp }) => {
    client.send('file_sync', { hash, content, timestamp });
  });

  // Reconnect: exchange hashes
  client.on('open', async () => {
    const content = await readFile(filePath, 'utf8');
    const hash = createHash('sha256').update(content).digest('hex');
    client.send('file_sync', { hash, content, timestamp: Date.now() });
  });
}
```

### Anti-Patterns to Avoid
- **Using `fs.watch()` on Windows:** Unreliable -- fires duplicate events, misses events on some filesystems, no guarantee of filename in callback. Polling with hash comparison is the correct approach for this use case.
- **Echo loops:** Writing received content triggers the watcher to detect a "change" and re-send. Must use the `writing` flag suppression pattern from the locked decisions.
- **Sending diffs instead of full content:** Locked decision says full file. Diffs add complexity with no benefit for a 22KB file.
- **Git-based detection:** Explicitly out of scope. Git index.lock collisions are the reason this phase exists.
- **Blocking file reads in poll loop:** Use async `readFile` to avoid blocking the event loop during the 5-second poll interval.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File hashing | Custom hash function | `crypto.createHash('sha256')` | Cryptographic hash with zero deps, consistent across platforms |
| Atomic writes | Manual open/write/close/rename | `fs.writeFile` + `fs.rename` | Node.js `rename` is atomic on same filesystem, handles Windows NTFS correctly |
| Message envelope | Custom JSON format | `createMessage()` / `parseMessage()` from `shared/protocol.js` | Already handles version, id, timestamp, type, from, payload |
| Reconnect queuing | Custom offline buffer | `CommsClient.send()` queue | Already queues during disconnection and replays on reconnect (WS-05) |
| WebSocket transport | Raw TCP or HTTP polling | `CommsClient` + `createCommsServer` | Already handles PSK auth, state machine, backoff, keepalive |

**Key insight:** This phase is primarily wiring -- the transport, queuing, protocol, and message types all exist. The new code is file I/O (hash + read + atomic write) and conflict detection logic.

## Common Pitfalls

### Pitfall 1: Echo Loop (Write Triggers Re-send)
**What goes wrong:** Receiving a `file_sync` message writes the file, which the watcher detects as a change, sending it back to the sender -- infinite loop.
**Why it happens:** The poll-based watcher sees any file modification, including writes from sync.
**How to avoid:** Set `#writing = true` before writing, write atomically, update `#lastHash` to the new hash, then clear `#writing`. The watcher's next poll sees the new hash matches and does nothing.
**Warning signs:** Rapid `file_sync` messages bouncing back and forth in WebSocket logs.

### Pitfall 2: Stale Hash on Reconnect
**What goes wrong:** After reconnect, the hash sent is from before disconnection. If the file changed locally while disconnected, the reconnect sync sends stale content.
**Why it happens:** Watcher stops updating hash during disconnection if coded incorrectly.
**How to avoid:** Keep the watcher running during disconnection (locked decision). It tracks the latest hash even when offline. On reconnect, read the current file content and send it.
**Warning signs:** After reconnect, content reverts to pre-disconnection state.

### Pitfall 3: Queue Replay Sends Stale file_sync
**What goes wrong:** During disconnection, multiple local changes queue multiple `file_sync` messages. On reconnect, CommsClient replays all of them in order, but only the last one is current.
**Why it happens:** CommsClient queues all sends during disconnection without deduplication.
**How to avoid:** Deduplicate `file_sync` messages in the queue -- only keep the latest one. This is a Claude's Discretion item. **Recommendation:** Deduplicate by replacing any existing `file_sync` in the queue with the new one, rather than appending.
**Warning signs:** Multiple `file_sync` messages sent rapidly on reconnect, with intermediate (stale) content overwriting current state.

### Pitfall 4: Ack Timeout / Missing Ack
**What goes wrong:** Sender waits for `file_ack` before sending next sync, but ack never arrives (network issue, receiver error).
**Why it happens:** No timeout on ack wait.
**How to avoid:** Set a reasonable ack timeout (e.g., 30 seconds). If no ack received, clear the pending state so the next poll cycle can detect and re-send. Log a warning.
**Warning signs:** Watcher detects changes but never sends because it's stuck waiting for a previous ack.

### Pitfall 5: Race Between Poll and Incoming Write
**What goes wrong:** A poll reads the file while an atomic write (rename) is in progress, resulting in an error or partial read.
**Why it happens:** `readFile` and `rename` are not atomic with respect to each other.
**How to avoid:** The `#writing` flag prevents the poll from reading during a write operation. Since both operations are async and on the same event loop, the flag check in `#poll()` is sufficient -- no true race condition in single-threaded Node.js.
**Warning signs:** Occasional ENOENT errors during poll (file briefly doesn't exist between rename steps -- but Node.js `rename` is a single OS call, so this shouldn't happen).

### Pitfall 6: Windows Line Endings
**What goes wrong:** Git might convert line endings (LF vs CRLF), causing hash mismatches between James (Windows) and Bono (Linux) even when content is semantically identical.
**Why it happens:** Git's `core.autocrlf` setting or `.gitattributes` changes line endings.
**How to avoid:** Since we're syncing via WebSocket (not git), the content is transmitted as-is. The receiving side writes what it receives. Both sides will always have the same byte-for-byte content after sync. The only risk is if one side's editor normalizes line endings -- but since AIs append via code (not editors), this is unlikely. If needed, normalize to LF before hashing and transmitting.
**Warning signs:** Hashes never match even though content looks identical.

## Code Examples

### SHA-256 Hash Computation
```javascript
// Source: Verified on Node.js 22.14.0 (project's runtime)
import { createHash } from 'node:crypto';

function hashContent(content) {
  return createHash('sha256').update(content).digest('hex');
}
// Returns 64-character hex string
// Example: '6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72'
```

### Atomic Write on Windows
```javascript
// Source: Verified on Windows 11 + Node.js 22.14.0
import { writeFile, rename } from 'node:fs/promises';

async function atomicWrite(targetPath, content) {
  const tmpPath = targetPath + '.tmp';
  await writeFile(tmpPath, content, 'utf8');
  await rename(tmpPath, targetPath);  // Atomic on NTFS (same volume)
}
```

### file_sync Message Payload
```javascript
// Source: shared/protocol.js createMessage() pattern
// Outbound (on change detection):
client.send('file_sync', {
  hash: '6ae8a75...', // SHA-256 hex digest of file content
  content: '# RaceControl Logbook\n...',  // Full file content as UTF-8 string
  timestamp: 1710244320000,  // Date.now() when change was detected
});

// Inbound ack:
// { type: 'file_ack', payload: { hash: '6ae8a75...', timestamp: 1710244321000 } }
```

### Append Detection for Auto-Merge
```javascript
// Source: LOGBOOK.md structure analysis (270 lines, date-sectioned tables)

/**
 * Check if newContent is a pure append to baseContent.
 * Returns the appended lines if true, null if not a pure append.
 */
function getAppendedLines(baseContent, newContent) {
  // Trim trailing whitespace for comparison
  const baseNorm = baseContent.trimEnd();
  const newNorm = newContent.trimEnd();

  if (!newNorm.startsWith(baseNorm)) return null;

  const appended = newNorm.slice(baseNorm.length);
  if (appended.length === 0) return null;  // identical content

  return appended;
}

/**
 * Merge two append-only changes from a common base.
 * Both localAppend and remoteAppend are the new lines added by each side.
 */
function mergeAppends(baseContent, localAppend, remoteAppend) {
  // Combine appended lines, sorted by timestamp within each date section
  // Since both sides append chronologically, concatenation is usually correct
  return baseContent.trimEnd() + localAppend + remoteAppend;
}
```

### DI Test Pattern (following project convention)
```javascript
// Source: Project convention from watchdog.test.js, alerting.test.js
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

describe('LogbookWatcher', () => {
  it('emits changed when file hash differs from previous', async (t) => {
    t.mock.timers.enable({ apis: ['setInterval', 'setTimeout'] });

    let readCount = 0;
    const contents = ['content-v1', 'content-v2'];

    const watcher = new LogbookWatcher({
      filePath: '/fake/LOGBOOK.md',
      pollMs: 5000,
      readFileFn: async () => contents[Math.min(readCount++, contents.length - 1)],
    });

    const events = [];
    watcher.on('changed', (evt) => events.push(evt));
    watcher.start();

    await Promise.resolve();
    // First poll: sets initial hash, no event
    assert.equal(events.length, 0);

    t.mock.timers.tick(5000);
    await Promise.resolve();
    // Second poll: content changed, should emit
    assert.equal(events.length, 1);
    assert.ok(events[0].hash);
    assert.equal(events[0].content, 'content-v2');

    watcher.stop();
  });
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `fs.watchFile()` (stat polling) | `fs.watch()` (inotify/FSEvents) | Node 8+ | `fs.watch` is preferred but unreliable on Windows for this use case |
| `fs.watch()` | Manual polling + hash | Always for reliability-critical | Deterministic, no platform quirks, matches locked decision |
| `writeFileSync` + manual temp | `writeFile` + `rename` (async) | Node 12+ fs/promises | Non-blocking, still atomic |
| MD5 hashing | SHA-256 | Post-2020 | SHA-256 is standard for integrity checks, no collision risk |

**Deprecated/outdated:**
- `fs.watchFile()`: Still works but is stat-based polling internally with poor performance. Not what we want -- we're doing our own polling with hash comparison which is more reliable.

## Discretion Recommendations

### Bono's Filesystem Path
**Recommendation:** `~/comms-link/data/LOGBOOK.md` (or wherever Bono's comms-link repo is checked out + `/data/`). The path should be configurable via environment variable `LOGBOOK_PATH` with a sensible default. Since Bono's side is on a Linux VPS, standard Linux path conventions apply.

### Conflict Notification
**Recommendation:** Log warning + write `.conflict` file. Email is overkill for a near-impossible scenario (both AIs editing the same non-append section simultaneously). The `.conflict` file pattern is:
- Write `LOGBOOK.md.conflict` with the conflicting version
- Log `[LOGBOOK] CONFLICT: both sides modified non-append content`
- Keep the local version, don't overwrite

### Queue Deduplication
**Recommendation:** Yes, deduplicate. When queueing a `file_sync` message during disconnection, replace any existing `file_sync` in the queue rather than appending. This avoids the stale replay pitfall. Implementation: before pushing to queue, scan for existing `file_sync` entries and remove them. This requires a small modification to `CommsClient.send()` or a wrapper function.

**Alternative (simpler):** Don't modify CommsClient. Instead, on the receiving side, ignore `file_sync` messages whose hash matches the already-received content. This is idempotent and doesn't require changing existing code.

### Hash Algorithm
**Recommendation:** SHA-256 as specified. `crypto.createHash('sha256')` is fast enough for 22KB files (microseconds).

### Append Detection
**Recommendation:** Simple prefix comparison. If the new content starts with the entire old content (byte-for-byte), it's a pure append. This is accurate for LOGBOOK.md's structure where entries are only added at the end of date sections. For the merge case: concatenate both sides' appended portions. Since entries are timestamped, the merged result will be chronologically ordered within each section.

## Open Questions

1. **Bono's file path configuration**
   - What we know: James's path is hardcoded (`~/racingpoint/racecontrol/LOGBOOK.md`). Bono's path is at Claude's discretion.
   - What's unclear: Whether Bono's comms-link clone has a `data/` directory or if the path should point to Bono's racecontrol clone.
   - Recommendation: Use environment variable `LOGBOOK_PATH` with no default (require explicit configuration). Both sides need to set this.

2. **CommsClient queue deduplication scope**
   - What we know: CommsClient queues all messages during disconnection. Multiple `file_sync` messages could queue up.
   - What's unclear: Whether modifying CommsClient's queue logic is acceptable or if deduplication should be external.
   - Recommendation: Don't modify CommsClient. Use idempotent receiving (ignore if hash matches current) + send only once on reconnect via the `open` event handler (which reads the current file, not a stale queued version).

3. **Ack timeout behavior**
   - What we know: `file_ack` is required before accepting next sync (locked decision).
   - What's unclear: What happens if ack never arrives.
   - Recommendation: 30-second timeout. On timeout, clear pending state and log warning. Next poll cycle will detect the file hasn't changed (same hash) or has changed again (new hash triggers re-send).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node.js 22.14.0) |
| Config file | none -- invoked via `node --test test/*.test.js` |
| Quick run command | `node --test test/logbook-watcher.test.js test/logbook-merge.test.js test/logbook-sync.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LS-01 | Detect file changes via SHA-256 hash comparison | unit | `node --test test/logbook-watcher.test.js` | Wave 0 |
| LS-02 | Send full file content over WebSocket on change | unit+integration | `node --test test/logbook-sync.test.js` | Wave 0 |
| LS-03 | Atomic write on receiving side (temp + rename) | unit | `node --test test/logbook-watcher.test.js` | Wave 0 |
| LS-04 | Conflict detection when both sides modified | unit | `node --test test/logbook-merge.test.js` | Wave 0 |
| LS-05 | Identical content within 30 seconds | integration | `node --test test/logbook-sync.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/logbook-watcher.test.js test/logbook-merge.test.js test/logbook-sync.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/logbook-watcher.test.js` -- covers LS-01, LS-03 (polling, hashing, echo suppression, atomic write)
- [ ] `test/logbook-merge.test.js` -- covers LS-04 (append detection, auto-merge, conflict flagging)
- [ ] `test/logbook-sync.test.js` -- covers LS-02, LS-05 (end-to-end wiring, ack flow, reconnect sync)
- [ ] `james/logbook-watcher.js` -- LogbookWatcher class
- [ ] `shared/logbook-merge.js` -- Pure merge/conflict functions

## Sources

### Primary (HIGH confidence)
- Project codebase analysis: `shared/protocol.js` (line 12-13: `file_sync` and `file_ack` already defined)
- Project codebase analysis: `james/comms-client.js` (queue + replay mechanism, lines 121-135)
- Project codebase analysis: All existing test files (134 tests, `node:test` runner, DI patterns)
- Node.js 22 docs: `crypto.createHash()`, `fs/promises.writeFile()`, `fs/promises.rename()`
- Verified on local system: Atomic write via temp+rename works on Windows 11 NTFS with Node.js 22.14.0

### Secondary (MEDIUM confidence)
- LOGBOOK.md structure analysis: 270 lines, ~22KB, date-sectioned append-only tables
- Windows `fs.watch()` reliability: Known issues with duplicate events and missed events on NTFS -- polling is the correct approach

### Tertiary (LOW confidence)
- None -- all findings verified through code analysis or local testing

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all built-in Node.js modules, no external dependencies needed
- Architecture: HIGH -- follows established project patterns (DI, EventEmitter, wiring functions)
- Pitfalls: HIGH -- echo loop and stale hash are well-understood problems with known solutions
- Merge logic: MEDIUM -- append detection via prefix comparison is simple and correct for LOGBOOK.md's structure, but edge cases (e.g., trailing whitespace changes) may need attention

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no fast-moving dependencies)
