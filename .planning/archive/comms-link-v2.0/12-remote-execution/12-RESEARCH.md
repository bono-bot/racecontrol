# Phase 12: Remote Execution - Research

**Researched:** 2026-03-20
**Domain:** Remote command execution over WebSocket with approval flow, shell injection prevention, environment sanitization
**Confidence:** HIGH

## Summary

Phase 12 adds the headline v2.0 feature: either AI (James or Bono) can request the other to execute a command on its machine, with results returned reliably. The implementation builds on Phase 11's reliable delivery (AckTracker, MessageQueue, dedup) and adds two new modules: `shared/exec-protocol.js` (command allowlist, tier classification, sanitized env) and `james/exec-handler.js` (execution engine with approval gate). Bono gets a symmetric handler.

The security model is strict by design: commands are identified by enum name (not shell strings), executed via `execFile` array-args form (never shell), with a sanitized environment containing only PATH/SYSTEMROOT/TEMP. Three approval tiers gate execution: auto-execute for read-only, notify-and-execute for moderate, require-approval for dangerous. Unapproved commands default-deny after 10 minutes. The approval notification goes to Uday via WhatsApp (Evolution API, already wired in alert-manager.js). Approval/rejection comes back through HTTP relay endpoints on James's side.

**Primary recommendation:** Define the command allowlist as a static enum object mapping command names to `{ binary, args, tier, timeoutMs, description }` entries. The remote side sends only the command name and parameters -- never a shell string. This makes injection impossible by construction, not by sanitization.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| EXEC-01 | Either AI can send exec_request specifying command from allowlist | exec-protocol.js COMMAND_REGISTRY enum + exec_request/exec_result message types in protocol.js |
| EXEC-02 | Commands use array-args child_process form only (never shell string) | execFile() with explicit args array, shell:true prohibited, validated by Pitfall #18 research |
| EXEC-03 | Auto-approve tier: read-only commands execute immediately | COMMAND_REGISTRY tier: 'auto' for status/health/version commands |
| EXEC-04 | Notify-and-execute tier: moderate commands execute + notify Uday | COMMAND_REGISTRY tier: 'notify' + sendEvolutionText (already wired in bono/alert-manager.js) |
| EXEC-05 | Require-approval tier: dangerous commands pause and wait for human approval | COMMAND_REGISTRY tier: 'approve' + pendingApprovals Map + HTTP relay endpoints |
| EXEC-06 | Unapproved commands default-deny after timeout (10 min) | setTimeout on pending approval entries, configurable via EXEC_APPROVAL_TIMEOUT_MS env var |
| EXEC-07 | Results (stdout, stderr, exit code) returned as exec_result | Structured exec_result message with execId, exitCode, stdout, stderr, durationMs |
| EXEC-08 | Sanitized environment: only PATH/SYSTEMROOT/TEMP | Explicit env option on execFile() call, SAFE_ENV constant |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:child_process (execFile) | Node 22.14.0 built-in | Execute commands without shell | Array-args form prevents injection by construction; already used in system-metrics.js and watchdog.js |
| node:events (EventEmitter) | Node 22.14.0 built-in | ExecHandler events (pending_approval, exec_started, exec_completed) | Project convention from v1.0 |
| ws@^8.19.0 | 8.19.0 | WebSocket transport (unchanged) | Already in use |
| shared/ack-tracker.js | Phase 9 | Reliable delivery of exec_request/exec_result | Phase 11 complete; exec messages use sendTracked() |
| shared/protocol.js | Phase 9 | Message envelope with exec_request/exec_result types | Extend existing MessageType enum |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| bono/alert-manager.js (sendEvolutionText) | v1.0 | WhatsApp notification for approval requests | Tier 2 (notify) and Tier 3 (approve) commands |
| node:timers (setTimeout) | Built-in | Approval timeout (10 min default-deny) | Every Tier 3 pending approval |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Enum allowlist | Regex pattern matching | Regex is error-prone (Pitfall #18); enum is closed-set, impossible to accidentally match dangerous commands |
| execFile array-args | spawn with shell:false | Equivalent security, but execFile buffers stdout/stderr which is simpler for returning results |
| WhatsApp approval callback | HTTP webhook from WhatsApp | Evolution API is send-only (no incoming webhooks); approval comes via HTTP relay instead |

**No new npm dependencies required.** Everything uses Node.js stdlib + existing project modules.

## Architecture Patterns

### Recommended Project Structure

```
shared/
  exec-protocol.js          # NEW: Command registry, tier enum, env sanitization, validation
  protocol.js               # MODIFIED: Add exec_request, exec_result, exec_approval types
  ack-tracker.js             # UNCHANGED: Used by exec for reliable delivery

james/
  exec-handler.js            # NEW: Receive exec_request, check tier, execute, return result
  index.js                   # MODIFIED: Wire exec handler + HTTP relay routes

bono/
  index.js                   # MODIFIED: Wire exec handler (symmetric) + exec request sending
```

### Pattern 1: Enum-Based Command Registry (CRITICAL)

**What:** Commands are defined as a frozen object mapping command names to execution specs. The remote side sends a command NAME (string key), never a shell command string. The handler looks up the name in the registry and executes the predefined binary + args.

**When to use:** Always. This is the only acceptable pattern for remote command execution.

**Example:**

```javascript
// shared/exec-protocol.js
export const ApprovalTier = Object.freeze({
  AUTO: 'auto',           // Read-only, execute immediately
  NOTIFY: 'notify',       // Execute immediately, notify Uday
  APPROVE: 'approve',     // Pause, notify Uday, wait for approval
});

export const COMMAND_REGISTRY = Object.freeze({
  // --- Auto-approve: read-only status commands ---
  git_status: {
    binary: 'git',
    args: ['status'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 10000,
    description: 'Git working tree status',
  },
  git_log: {
    binary: 'git',
    args: ['log', '--oneline', '-20'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 10000,
    description: 'Recent git commits',
  },
  node_version: {
    binary: 'node',
    args: ['--version'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 5000,
    description: 'Node.js version',
  },
  health_check: {
    binary: 'curl',
    args: ['-s', '-o', '/dev/null', '-w', '%{http_code}', 'http://127.0.0.1:8766/relay/health'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 5000,
    description: 'Comms-link health check',
  },
  disk_usage: {
    binary: 'wmic',
    args: ['logicaldisk', 'get', 'size,freespace,caption'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 10000,
    description: 'Disk space usage',
  },
  tasklist_node: {
    binary: 'tasklist',
    args: ['/FI', 'IMAGENAME eq node.exe', '/FO', 'CSV'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 5000,
    description: 'List running Node.js processes',
  },
  rc_agent_status: {
    binary: 'tasklist',
    args: ['/FI', 'IMAGENAME eq rc-agent.exe', '/FO', 'CSV'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 5000,
    description: 'Check if rc-agent is running',
  },
  uptime: {
    binary: 'net',
    args: ['statistics', 'workstation'],
    tier: ApprovalTier.AUTO,
    timeoutMs: 5000,
    description: 'System uptime statistics',
  },

  // --- Notify-and-execute: moderate impact ---
  npm_install: {
    binary: 'npm',
    args: ['install'],
    tier: ApprovalTier.NOTIFY,
    timeoutMs: 120000,
    cwd: 'C:/Users/bono/racingpoint/comms-link',
    description: 'Install npm dependencies',
  },
  git_pull: {
    binary: 'git',
    args: ['pull'],
    tier: ApprovalTier.NOTIFY,
    timeoutMs: 30000,
    description: 'Pull latest from remote',
  },

  // --- Require-approval: dangerous operations ---
  restart_daemon: {
    binary: 'taskkill',
    args: ['/F', '/IM', 'node.exe', '/T'],
    tier: ApprovalTier.APPROVE,
    timeoutMs: 15000,
    description: 'Kill all Node.js processes (daemon will be respawned by supervisor)',
  },
  reboot_machine: {
    binary: 'shutdown',
    args: ['/r', '/t', '60', '/c', 'Remote reboot requested via comms-link'],
    tier: ApprovalTier.APPROVE,
    timeoutMs: 5000,
    description: 'Reboot machine in 60 seconds',
  },
  deploy_pull: {
    binary: 'git',
    args: ['pull', 'origin', 'main'],
    tier: ApprovalTier.APPROVE,
    timeoutMs: 60000,
    cwd: 'C:/Users/bono/racingpoint/comms-link',
    description: 'Pull latest code from origin/main',
  },
});
```

### Pattern 2: Sanitized Environment Constant

**What:** A frozen object containing only safe environment variables. Passed as the `env` option to every `execFile` call.

**Example:**

```javascript
// shared/exec-protocol.js
export function buildSafeEnv() {
  return Object.freeze({
    PATH: process.env.PATH,
    SYSTEMROOT: process.env.SYSTEMROOT || 'C:\\Windows',
    TEMP: process.env.TEMP || 'C:\\Users\\bono\\AppData\\Local\\Temp',
    TMP: process.env.TMP || process.env.TEMP || 'C:\\Users\\bono\\AppData\\Local\\Temp',
    // On Linux (Bono's VPS):
    HOME: process.env.HOME || '',
  });
}
```

### Pattern 3: ExecHandler with DI (Project Convention)

**What:** ExecHandler class accepts injectable functions for all external dependencies.

**Example:**

```javascript
// james/exec-handler.js
export class ExecHandler extends EventEmitter {
  constructor({
    execFileFn,           // Injectable: (binary, args, opts, cb) => ChildProcess
    sendResultFn,         // Injectable: (execId, result) => void
    notifyFn,             // Injectable: (text) => Promise -- WhatsApp notification
    nowFn = Date.now,     // Injectable clock
    approvalTimeoutMs = 600000,  // 10 minutes default
    commandRegistry = COMMAND_REGISTRY,
    safeEnv,              // Injectable sanitized env
  })
}
```

### Pattern 4: HTTP Relay Routes for Approval

**What:** Claude Code (or Uday via web terminal) approves/rejects pending commands through the existing HTTP relay.

**New routes on james/index.js:**

```
GET  /relay/exec/pending          -> List pending approval requests
POST /relay/exec/approve/:execId  -> Approve a pending command
POST /relay/exec/reject/:execId   -> Reject a pending command
GET  /relay/exec/history?limit=20 -> Recent execution results
```

### Anti-Patterns to Avoid

- **Shell string execution:** NEVER use shell-based process execution or set `shell: true` on execFile. This is the #1 security risk (Pitfall #18).
- **Passing remote-supplied arguments directly:** The remote side sends a command NAME, not arguments. Arguments come from the registry. If parameterization is needed, validate against strict regex.
- **Inheriting full process environment:** NEVER omit the `env` option on execFile. The default inherits COMMS_PSK, API keys, auth tokens (Pitfall #22).
- **Allowlisting `env`, `set`, `printenv`, or arbitrary file readers:** These dump secrets even through the sanitized execution path.
- **Blocking the event loop with synchronous execution:** Use async execFile wrapped in callback/Promise. The daemon's WS message handling must not freeze during command execution.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Shell injection prevention | Input sanitization regex | execFile array-args (built-in) | Sanitization is always incomplete; array-args form makes injection impossible by construction |
| Message delivery guarantee | Custom retry logic | AckTracker (Phase 9, already built) | sendTracked() handles retry, timeout, reconnect replay |
| WhatsApp notification | New notification system | sendEvolutionText (already in alert-manager.js) | Battle-tested in v1.0 alerting |
| Duplicate execution prevention | Custom locking | DeduplicatorCache (Phase 9, already built) | exec_request deduplication uses existing UUID-based cache |
| Approval timeout | Manual timer management | setTimeout with Map cleanup | Simple, testable, no library needed |

**Key insight:** The entire exec infrastructure is built from existing project primitives (AckTracker, DeduplicatorCache, EventEmitter, DI pattern, HTTP relay, sendEvolutionText). The only genuinely new code is the command registry and the execution engine.

## Common Pitfalls

### Pitfall 1: Shell Injection (Pitfall #18 -- CRITICAL)

**What goes wrong:** Command string passed to shell-based execution allows metacharacter injection (`&`, `|`, `;`, backticks).
**Why it happens:** Using shell-based APIs or setting `shell: true` instead of array-args `execFile()`.
**How to avoid:** Command registry maps names to `{ binary, args }` tuples. Remote side sends NAME only. execFile with array-args is the only execution path. `shell: true` is never set.
**Warning signs:** Any code path where a string from a WebSocket message ends up as an argument to child process execution without going through the registry lookup.

### Pitfall 2: Environment Leakage (Pitfall #22 -- CRITICAL)

**What goes wrong:** Child process inherits COMMS_PSK, API keys, auth tokens. Command output (stdout/stderr) leaks them back over WebSocket.
**Why it happens:** execFile inherits process.env by default when no `env` option is specified.
**How to avoid:** Always pass `{ env: SAFE_ENV }` containing only PATH/SYSTEMROOT/TEMP/TMP. Never allowlist commands that dump environment variables.
**Warning signs:** Any execFile call without an explicit `env` option.

### Pitfall 3: Duplicate Execution on Reconnect Replay

**What goes wrong:** exec_request is replayed after reconnect, causing the command to run twice.
**Why it happens:** AckTracker replays unACKed messages on reconnect. If the original exec_request was received and executed but the exec_result ACK was lost, the request replays.
**How to avoid:** DeduplicatorCache on the receiver side. Check `isDuplicate(msg.id)` before executing. Already wired in james/index.js (line 144). For exec specifically: also track execId in a completed-executions set.
**Warning signs:** Commands that are not idempotent (e.g., `npm install` with version changes) executing twice.

### Pitfall 4: Approval Timeout Race with Daemon Restart

**What goes wrong:** A command is pending approval. Daemon restarts. Pending approvals are lost (in-memory Map).
**Why it happens:** pendingApprovals is an in-memory Map, not persisted.
**How to avoid:** Persist pending approvals to the WAL (MessageQueue). On restart, reload and resume timeout timers. Alternatively, accept the simpler design: on restart, pending approvals are lost and the requester's AckTracker will timeout -- acceptable for a 2-node system.
**Warning signs:** Uday approves after daemon restart but the approval has no matching pending entry.

### Pitfall 5: Stdout/Stderr Buffer Overflow

**What goes wrong:** A command produces megabytes of output (e.g., `npm install` verbose). execFile's default maxBuffer (1MB in Node 22) is exceeded, killing the child process with ENOBUFS.
**Why it happens:** execFile buffers all output in memory before calling the callback.
**How to avoid:** Set `maxBuffer` explicitly (e.g., 5MB). Truncate output before sending over WebSocket (e.g., first 50KB of stdout, first 10KB of stderr). Document the truncation in the exec_result payload.
**Warning signs:** Commands that succeed locally but fail via remote execution with "maxBuffer length exceeded" errors.

## Code Examples

### exec_request Message Format

```javascript
// New message types for protocol.js
export const MessageType = Object.freeze({
  // ... existing types ...
  exec_request: 'exec_request',
  exec_result: 'exec_result',
  exec_approval: 'exec_approval',
});

// exec_request payload
{
  execId: 'ex_abc123',        // Unique execution ID
  command: 'git_status',       // Key from COMMAND_REGISTRY (not a shell string)
  params: {},                  // Optional parameters (validated against PARAMETERIZABLE)
  requestedBy: 'bono',        // Who requested
  reason: 'Checking deploy status', // Human-readable reason
}

// exec_result payload
{
  execId: 'ex_abc123',        // Correlates to request
  command: 'git_status',
  exitCode: 0,
  stdout: 'On branch main\nnothing to commit, working tree clean\n',
  stderr: '',
  durationMs: 145,
  truncated: false,            // True if output was truncated
  tier: 'auto',               // Which tier was applied
}

// exec_approval payload (for HTTP relay -> WS bridge)
{
  execId: 'ex_def456',
  approved: true,              // or false for rejection
  approvedBy: 'uday',         // Who approved
}
```

### ExecHandler Core Logic

```javascript
// james/exec-handler.js (simplified)
import { execFile } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { COMMAND_REGISTRY, ApprovalTier, buildSafeEnv } from '../shared/exec-protocol.js';

export class ExecHandler extends EventEmitter {
  #pendingApprovals = new Map();
  #completedExecs = new Set(); // Dedup for idempotency
  #execFileFn;
  #sendResultFn;
  #notifyFn;
  #nowFn;
  #approvalTimeoutMs;
  #registry;
  #safeEnv;

  constructor({ execFileFn, sendResultFn, notifyFn, nowFn = Date.now,
                approvalTimeoutMs = 600000, commandRegistry = COMMAND_REGISTRY,
                safeEnv = buildSafeEnv() }) {
    super();
    this.#execFileFn = execFileFn || execFile;
    this.#sendResultFn = sendResultFn;
    this.#notifyFn = notifyFn;
    this.#nowFn = nowFn;
    this.#approvalTimeoutMs = approvalTimeoutMs;
    this.#registry = commandRegistry;
    this.#safeEnv = safeEnv;
  }

  handleExecRequest(msg) {
    const { execId, command, params } = msg.payload;

    // Dedup: skip if already executed
    if (this.#completedExecs.has(execId)) return;

    // Lookup in registry -- reject unknown commands
    const spec = this.#registry[command];
    if (!spec) {
      this.#sendResultFn(execId, {
        execId, command, exitCode: -1,
        stdout: '', stderr: `Unknown command: ${command}`,
        durationMs: 0, tier: 'rejected',
      });
      return;
    }

    if (spec.tier === ApprovalTier.AUTO) {
      this.#execute(execId, command, spec);
    } else if (spec.tier === ApprovalTier.NOTIFY) {
      this.#execute(execId, command, spec);
      this.#notifyFn?.(`[EXEC] ${command} executed on ${process.env.COMPUTERNAME || 'unknown'}`);
    } else if (spec.tier === ApprovalTier.APPROVE) {
      this.#queueForApproval(execId, command, spec, msg.from);
    }
  }

  #execute(execId, command, spec) {
    const start = this.#nowFn();
    this.emit('exec_started', { execId, command });

    this.#execFileFn(spec.binary, spec.args, {
      env: this.#safeEnv,
      timeout: spec.timeoutMs,
      maxBuffer: 5 * 1024 * 1024,
      cwd: spec.cwd || undefined,
      shell: false, // Explicit: never use shell
    }, (err, stdout, stderr) => {
      const duration = this.#nowFn() - start;
      const result = {
        execId,
        command,
        exitCode: err ? (err.code ?? -1) : 0,
        stdout: (stdout || '').slice(0, 50000),
        stderr: (stderr || (err?.message) || '').slice(0, 10000),
        durationMs: duration,
        truncated: (stdout || '').length > 50000 || (stderr || '').length > 10000,
        tier: spec.tier,
      };

      this.#completedExecs.add(execId);
      this.#sendResultFn(execId, result);
      this.emit('exec_completed', { execId, result });
    });
  }

  #queueForApproval(execId, command, spec, requestedBy) {
    const timer = setTimeout(() => {
      this.#pendingApprovals.delete(execId);
      this.#sendResultFn(execId, {
        execId, command, exitCode: -1,
        stdout: '', stderr: 'Approval timed out (default-deny)',
        durationMs: 0, tier: 'timed_out',
      });
      this.emit('approval_timeout', { execId, command });
    }, this.#approvalTimeoutMs);

    this.#pendingApprovals.set(execId, { command, spec, requestedBy, timer });
    this.emit('pending_approval', { execId, command, requestedBy });
    this.#notifyFn?.(
      `[APPROVAL NEEDED] ${requestedBy} wants to run "${command}". ` +
      `Approve at http://192.168.31.27:8766/relay/exec/approve/${execId} or denied in 10 min.`
    );
  }

  approveCommand(execId) {
    const pending = this.#pendingApprovals.get(execId);
    if (!pending) return false;
    clearTimeout(pending.timer);
    this.#pendingApprovals.delete(execId);
    this.#execute(execId, pending.command, pending.spec);
    return true;
  }

  rejectCommand(execId, reason = 'Rejected by operator') {
    const pending = this.#pendingApprovals.get(execId);
    if (!pending) return false;
    clearTimeout(pending.timer);
    this.#pendingApprovals.delete(execId);
    this.#sendResultFn(execId, {
      execId, command: pending.command, exitCode: -1,
      stdout: '', stderr: reason,
      durationMs: 0, tier: 'rejected',
    });
    return true;
  }

  get pendingApprovals() {
    return Array.from(this.#pendingApprovals.entries()).map(([id, entry]) => ({
      execId: id, command: entry.command, requestedBy: entry.requestedBy,
    }));
  }

  shutdown() {
    for (const [, entry] of this.#pendingApprovals) {
      clearTimeout(entry.timer);
    }
    this.#pendingApprovals.clear();
  }
}
```

### Wiring into james/index.js

```javascript
// In james/index.js -- add after existing imports
import { ExecHandler } from './exec-handler.js';
import { COMMAND_REGISTRY, buildSafeEnv } from '../shared/exec-protocol.js';

// After AckTracker setup
const execHandler = new ExecHandler({
  sendResultFn: (execId, result) => {
    sendTracked('exec_result', result);
  },
  notifyFn: async (text) => {
    // Send WhatsApp via Bono's Evolution API (relay through WS)
    client.send('message', { text, channel: 'whatsapp_notify' });
  },
});

// In message handler, add:
if (msg.type === 'exec_request') {
  execHandler.handleExecRequest(msg);
  return;
}

// In HTTP relay, add:
if (req.method === 'GET' && req.url === '/relay/exec/pending') {
  jsonResponse(res, 200, { pending: execHandler.pendingApprovals });
  return;
}
if (req.method === 'POST' && req.url?.startsWith('/relay/exec/approve/')) {
  const execId = req.url.split('/').pop();
  const ok = execHandler.approveCommand(execId);
  jsonResponse(res, ok ? 200 : 404, { ok });
  return;
}
if (req.method === 'POST' && req.url?.startsWith('/relay/exec/reject/')) {
  const execId = req.url.split('/').pop();
  const ok = execHandler.rejectCommand(execId);
  jsonResponse(res, ok ? 200 : 404, { ok });
  return;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Shell string execution | Array-args execFile() | Always best practice, reinforced by CVE-2025-68613 | Prevents injection by construction |
| Inherit process.env | Explicit sanitized env | Node.js security best practices | Prevents secret leakage |
| wmic for process detection | tasklist /FO CSV | Windows 11 deprecated wmic | Already done in comms-link v1.0 |
| Regex-based command filtering | Enum allowlist lookup | Industry shift to positive security model | Closed-set: only defined commands run |

**Deprecated/outdated:**
- `wmic`: Deprecated on Windows 11. Use `tasklist` instead (already used in system-metrics.js).
- Shell-based child process APIs: Never appropriate for remote-supplied commands. Always use `execFile()`.

## Open Questions

1. **Bono-side command registry**
   - What we know: The architecture is symmetric -- both sides can send and receive exec_request. James needs commands like git_status, tasklist. Bono likely needs fewer commands (health check, pm2 status).
   - What's unclear: The exact set of commands Bono should expose for James to call.
   - Recommendation: Start with James-side only (Bono sends requests, James executes). Add Bono-side execution in a follow-up if needed. The exec-protocol module is shared and works for both.

2. **WhatsApp approval response mechanism**
   - What we know: Evolution API can send messages. Approval notifications go to Uday via WhatsApp.
   - What's unclear: How Uday responds to approve. WhatsApp incoming webhooks are not wired up. The web terminal at :9999 could be used, or the HTTP relay at :8766.
   - Recommendation: Approval via HTTP relay endpoints (`POST /relay/exec/approve/:id`). Uday uses the web terminal or direct curl. The WhatsApp message includes the approval URL. No incoming webhook needed.

3. **Parameterizable commands**
   - What we know: Some commands need parameters (e.g., pod number for pod-specific status).
   - What's unclear: Which commands need parameters and what validation rules apply.
   - Recommendation: Start with zero parameterizable commands. All commands are fully static in the registry. Add parameterization only when a concrete need arises, with strict validation per parameter.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node 22.14.0) |
| Config file | none -- tests run via `node --test test/*.test.js` |
| Quick run command | `node --test test/exec-*.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXEC-01 | Send exec_request, receive exec_result | unit + integration | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-02 | Array-args only, shell:false enforced | unit | `node --test test/exec-protocol.test.js` | Wave 0 |
| EXEC-03 | Auto-approve tier executes immediately | unit | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-04 | Notify tier executes + sends notification | unit | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-05 | Approve tier pauses, waits for approval | unit | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-06 | Default-deny after timeout | unit | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-07 | exec_result includes stdout/stderr/exitCode | unit | `node --test test/exec-handler.test.js` | Wave 0 |
| EXEC-08 | Sanitized env (PATH/SYSTEMROOT/TEMP only) | unit | `node --test test/exec-protocol.test.js` | Wave 0 |

### Sampling Rate

- **Per task commit:** `node --test test/exec-handler.test.js test/exec-protocol.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `test/exec-protocol.test.js` -- covers EXEC-02, EXEC-08 (command registry validation, sanitized env, tier classification)
- [ ] `test/exec-handler.test.js` -- covers EXEC-01, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07 (execution lifecycle, approval flow, timeout, result format)
- [ ] `test/exec-wiring.test.js` -- covers EXEC-01 end-to-end (WS message routing for exec_request/exec_result through james/index.js and bono/index.js)

## Sources

### Primary (HIGH confidence)

- Direct codebase analysis -- james/index.js, bono/index.js, shared/protocol.js, shared/ack-tracker.js, james/system-metrics.js (existing execFile usage patterns)
- [Node.js child_process documentation](https://nodejs.org/api/child_process.html) -- execFile array-args security model, env option, maxBuffer, shell option
- Project research: PITFALLS.md Pitfall #18 (shell injection) and Pitfall #22 (environment leakage) -- HIGH confidence, real-world CVE evidence
- Project research: ARCHITECTURE.md exec-protocol.js and exec-handler.js API shapes
- Project research: FEATURES.md 3-tier approval model

### Secondary (MEDIUM confidence)

- [Prevent Command Injection Node.js Child_Process](https://securecodingpractices.com/prevent-command-injection-node-js-child-process/) -- execFile vs exec security comparison
- [Preventing Command Injection Attacks in Node.js Apps (Auth0)](https://auth0.com/blog/preventing-command-injection-attacks-in-node-js-apps/) -- input validation, environment sanitization
- [Node.js Best Practices - Child Processes Security](https://github.com/goldbergyoni/nodebestpractices/blob/master/sections/security/childprocesses.md) -- community consensus on execFile over exec
- [Node.js env sanitization commit](https://github.com/nodejs/node/commit/abd8cdfc4e) -- Windows env variable case-insensitivity handling

### Tertiary (LOW confidence)

- n8n CVE-2025-68613 (CVSS 9.4) -- referenced in PITFALLS.md as evidence for shell injection risk; not independently verified but cited in multiple security sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all Node.js stdlib, zero new dependencies, existing patterns
- Architecture: HIGH -- builds directly on Phase 9/11 primitives (AckTracker, DeduplicatorCache, protocol.js), follows established DI + EventEmitter conventions
- Pitfalls: HIGH -- shell injection and env leakage are textbook security issues with real CVE evidence; other pitfalls (dedup, buffer overflow) are standard distributed systems concerns
- Command registry: MEDIUM -- the specific command list is a product decision; the enum pattern is HIGH confidence but the exact entries need validation with Uday/Bono

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain -- Node.js child_process API is mature)
