# Phase 7: LOGBOOK Sync - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Synchronize LOGBOOK.md between James and Bono over WebSocket with conflict detection. Both sides watch their local copy, push full file content on change, and write received updates atomically. Auto-merge append-only changes; flag non-append edits as conflicts.

</domain>

<decisions>
## Implementation Decisions

### File Location & Path
- Hardcoded path on James's side: `~/racingpoint/racecontrol/LOGBOOK.md`
- Bono's path is at Claude's discretion -- James sends file content, Bono's server code decides where to write
- Two-way sync: both sides watch their local file and push changes
- No git commit on receive -- just write the file. The receiving AI commits as part of its normal workflow

### Change Detection & Sync Trigger
- Poll every 5 seconds using file hash comparison (SHA-256)
- Send full file content every time (not diffs) -- LOGBOOK.md is small (~15KB), full file is simpler and always consistent
- Require `file_ack` before accepting the next sync -- confirms delivery and write
- Suppress file watcher during write to prevent echo loops (set 'writing' flag, write, wait one poll cycle, clear flag)

### Conflict Handling
- Auto-merge when both sides appended new rows -- combine rows sorted by timestamp (LOGBOOK is append-only tables)
- Flag as conflict if changes aren't purely appending new rows (e.g., existing line edits, reformatting)
- Conflict notification approach at Claude's discretion (log warning + write .conflict file vs email alert)

### Offline & Reconnect Behavior
- Both sides send `file_sync` with current hash on reconnect -- if hashes differ, conflict/merge logic kicks in
- File watcher keeps running while disconnected -- tracks latest hash so reconnect sync is immediate
- Use CommsClient's existing message queue for `file_sync` during disconnection -- replays on reconnect

### Claude's Discretion
- Bono's filesystem path for LOGBOOK.md
- Conflict notification level (log warning vs email)
- Whether to deduplicate queued file_sync messages (only send latest version on reconnect)
- Hash algorithm details (SHA-256 recommended but flexible)
- Exact auto-merge implementation for append detection

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CommsClient.send('file_sync', payload)`: Already queues during disconnection and replays on reconnect (Phase 2, WS-05)
- `protocol.js`: Already defines `file_sync` and `file_ack` message types -- just need to use them
- `createMessage()` / `parseMessage()`: Standard envelope for all WebSocket messages
- `send_email.js`: Available for conflict notification if needed (execFile pattern from Phase 5)

### Established Patterns
- ESM modules with Object.freeze enums and private class fields (#field)
- DI via constructor options for testability (collectFn, sendFn, nowFn patterns)
- EventEmitter for lifecycle events
- node:test built-in test runner (134 tests across 13 files)
- Fire-and-forget for non-critical operations (email, alerts)

### Integration Points
- `james/index.js` or `james/watchdog-runner.js`: Wire LogbookWatcher to CommsClient
- `bono/index.js`: Wire LogbookReceiver to WebSocket message handler (alongside heartbeat and recovery)
- `CommsClient 'open' event`: Trigger reconnect sync exchange
- `shared/protocol.js`: file_sync and file_ack types already defined

</code_context>

<specifics>
## Specific Ideas

- LOGBOOK.md is an append-only chronological table organized by date sections with `| timestamp | author | commit | summary |` rows
- Currently ~270 lines / ~15KB -- grows daily but will stay manageable
- Both AIs append entries as part of their normal commit workflow
- Conflicts are near-impossible with the append-only structure, but the detection exists as a safety net
- The `file_ack` pattern mirrors HTTP request/response -- sender knows the other side has it

</specifics>

<deferred>
## Deferred Ideas

- Sync additional shared files beyond LOGBOOK.md (AS-01, v2)
- Bidirectional git sync (AS-02, v2)
- Conflict resolution UI for Uday -- just flag + .conflict file for now

</deferred>

---

*Phase: 07-logbook-sync*
*Context gathered: 2026-03-12*
