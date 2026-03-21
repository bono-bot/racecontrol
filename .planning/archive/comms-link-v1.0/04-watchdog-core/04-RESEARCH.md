# Phase 4: Watchdog Core - Research

**Researched:** 2026-03-12
**Domain:** Windows process monitoring, Task Scheduler, Node.js child_process management
**Confidence:** HIGH

## Summary

Phase 4 implements a watchdog that monitors the Claude Code process (`claude.exe`) and automatically restarts it when it crashes or exits. The watchdog must detect crashes within 5 seconds, kill all zombie/orphan processes before restarting, and survive reboots via Windows Task Scheduler running in user Session 1 (not Session 0).

An existing PowerShell watchdog script (`C:\Users\bono\.claude\claude_watchdog.ps1`) already provides a working reference implementation. Its log reveals real-world scenarios: zombie cleanup (8 instances at once), successful restarts, and a 5-minute cooldown. This phase replaces that script with a Node.js implementation integrated into the comms-link project.

The Claude Code executable path changes with each version update. The current path is `C:\Users\bono\AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\claude-code\2.1.72\claude.exe`. The existing PS1 script handles this by scanning the `claude-code` directory, sorting by version, and selecting the latest -- this pattern must be replicated.

**Primary recommendation:** Build a `ClaudeWatchdog` class with a polling loop (3-second interval using `setInterval`), `tasklist` for detection, `taskkill /F /T` for zombie cleanup, and `spawn({ detached: true, stdio: 'ignore' })` for restarting Claude. Register via `schtasks /create /sc onlogon /it` to ensure Session 1 execution.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WD-01 | Monitor Claude Code process and detect crash/exit within 5 seconds | Polling `tasklist` every 3 seconds ensures detection within 3-6 seconds. The existing `detectClaude()` function in `system-metrics.js` already does exactly this check and can be reused or adapted. |
| WD-02 | Auto-restart Claude Code after crash with zombie cleanup (taskkill /F /T tree kill) | Use `execFile('taskkill', ['/F', '/T', '/IM', 'claude.exe'])` for tree kill of all instances, then `spawn()` with `{ detached: true, stdio: 'ignore' }` + `unref()` to launch new process. Must resolve latest version directory dynamically. |
| WD-03 | Run watchdog in user session via Task Scheduler (NOT as Windows service) | `schtasks /create /sc onlogon /tn "CommsLink-Watchdog" /tr "node james/watchdog.js" /it /rl highest /f` creates a logon-triggered task that runs interactively in Session 1 (not Session 0). The `/it` flag is the critical piece. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:child_process | Node 22.14.0 built-in | Process detection (tasklist), killing (taskkill), launching (spawn) | Zero dependencies, native Windows process management |
| node:fs | Node 22.14.0 built-in | Read version directories to find latest claude.exe | Zero dependencies |
| node:path | Node 22.14.0 built-in | Path construction for Windows paths | Zero dependencies |
| node:test | Node 22.14.0 built-in | Test runner (project standard) | Already used by all 57 existing tests |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| node:timers | Node 22.14.0 built-in | setInterval for polling loop | Core polling mechanism |
| node:events | Node 22.14.0 built-in | EventEmitter for watchdog events | Same pattern as CommsClient, HeartbeatMonitor |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw tasklist polling | npm `tasklist` package | Extra dependency for zero benefit -- our check is 3 lines of execFile |
| Raw taskkill | npm `taskkill` package | Extra dependency, and we need exactly `taskkill /F /T /IM claude.exe` which is trivial |
| setInterval polling | fs.watch on process | Unreliable -- no file to watch, process monitoring requires active polling |
| Node.js watchdog | Keep PowerShell script | Doesn't integrate with comms-link, can't emit events for heartbeat/alerting |

**Installation:**
```bash
# No new dependencies needed -- everything is Node.js built-in
```

## Architecture Patterns

### Recommended Project Structure
```
james/
  watchdog.js          # ClaudeWatchdog class
  watchdog-runner.js   # Entry point for Task Scheduler (starts watchdog standalone)
  index.js             # Main entry (will integrate watchdog in Phase 5)
  comms-client.js      # Existing
  heartbeat-sender.js  # Existing
  system-metrics.js    # Existing (has detectClaude() to reuse)
test/
  watchdog.test.js     # Unit tests for ClaudeWatchdog
```

### Pattern 1: EventEmitter Watchdog Class
**What:** ClaudeWatchdog extends EventEmitter, emits 'crash_detected', 'zombies_killed', 'restart_success', 'restart_failed'
**When to use:** Always -- this is the core pattern used by CommsClient, HeartbeatMonitor, and HeartbeatSender in this project
**Example:**
```javascript
// Source: Project convention from comms-client.js, heartbeat-monitor.js
import { EventEmitter } from 'node:events';

export class ClaudeWatchdog extends EventEmitter {
  #interval = null;
  #pollMs;

  constructor({ pollMs = 3000 } = {}) {
    super();
    this.#pollMs = pollMs;
  }

  start() {
    this.stop();
    this.#poll();  // Check immediately
    this.#interval = setInterval(() => this.#poll(), this.#pollMs);
  }

  stop() {
    if (this.#interval !== null) {
      clearInterval(this.#interval);
      this.#interval = null;
    }
  }

  async #poll() {
    const running = await this.#isClaudeRunning();
    if (!running) {
      this.emit('crash_detected', { timestamp: Date.now() });
      await this.#killZombies();
      await this.#restart();
    }
  }
}
```

### Pattern 2: Dependency Injection for Testability
**What:** Inject process management functions (detect, kill, spawn) to enable mocking in tests
**When to use:** All process-interaction methods -- same pattern as HeartbeatSender's `collectFn` DI
**Example:**
```javascript
// Source: Project convention from heartbeat-sender.js
constructor({
  pollMs = 3000,
  detectFn = defaultDetect,
  killFn = defaultKill,
  spawnFn = defaultSpawn,
  findExeFn = defaultFindExe,
} = {}) {
  super();
  this.#detectFn = detectFn;
  // ... etc
}
```

### Pattern 3: Latest Version Discovery
**What:** Scan `claude-code` directory for version subdirectories, sort semver descending, return latest `claude.exe` path
**When to use:** Every restart -- Claude Code auto-updates, so the version directory may change between restarts
**Example:**
```javascript
// Source: Existing claude_watchdog.ps1 Find-ClaudeExe function
import { readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const CLAUDE_CODE_BASE = join(
  process.env.LOCALAPPDATA,
  'Packages', 'Claude_pzs8sxrjxfjjc',
  'LocalCache', 'Roaming', 'Claude', 'claude-code'
);

function findClaudeExe() {
  const dirs = readdirSync(CLAUDE_CODE_BASE, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name)
    .sort((a, b) => {
      // Compare as semver segments
      const pa = a.split('.').map(Number);
      const pb = b.split('.').map(Number);
      for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
        if ((pb[i] || 0) !== (pa[i] || 0)) return (pb[i] || 0) - (pa[i] || 0);
      }
      return 0;
    });

  if (dirs.length === 0) return null;

  const exe = join(CLAUDE_CODE_BASE, dirs[0], 'claude.exe');
  return existsSync(exe) ? exe : null;
}
```

### Pattern 4: Task Scheduler Registration Script
**What:** A one-time setup script (or npm script) that registers the watchdog with Task Scheduler
**When to use:** Run once during setup, or re-run after code changes
**Example:**
```bash
# Create Task Scheduler entry for watchdog
# /sc onlogon  -- triggers on user login
# /it          -- interactive only (Session 1, not Session 0)
# /rl highest  -- run with elevated privileges
# /f           -- force overwrite if exists
schtasks /create /tn "CommsLink-Watchdog" /tr "node C:\Users\bono\racingpoint\comms-link\james\watchdog-runner.js" /sc onlogon /it /rl highest /f
```

### Anti-Patterns to Avoid
- **Running as Windows Service (NSSM):** Causes Session 0 isolation -- Claude Code needs user session to display terminal UI. Explicitly out of scope per REQUIREMENTS.md.
- **Using process.kill() on Windows:** Node.js `process.kill()` and `ChildProcess.kill()` do NOT support tree kill on Windows. Only `taskkill /F /T` provides reliable tree kill.
- **Hardcoding claude.exe path:** The version directory changes with every Claude Code update. Always discover dynamically.
- **Polling too frequently:** Polling every 100ms wastes CPU. Polling every 3 seconds is the sweet spot -- detection within 3-6s satisfies the 5s target on average, and the 1-second overshoot on worst case is acceptable.
- **Spawning claude.exe without detached/unref:** If the watchdog spawns Claude without `detached: true` and `child.unref()`, killing the watchdog will also kill Claude.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process detection | Custom WMI queries | `execFile('tasklist', ...)` | Existing `detectClaude()` in system-metrics.js already works; tasklist is simpler than WMI |
| Process tree kill | Manual PID traversal | `execFile('taskkill', ['/F', '/T', '/IM', 'claude.exe'])` | `/T` flag handles tree kill; `/IM` by image name catches all instances |
| Semver comparison | Custom version parser | Simple numeric sort on split('.') segments | Two version dirs (2.1.51, 2.1.72) -- no need for a semver library |
| Task scheduling | COM API calls to Task Scheduler | `schtasks` CLI tool | schtasks is the standard way; COM API is complex and requires native bindings |

**Key insight:** The entire watchdog is composed of 4 shell commands: `tasklist` (detect), `taskkill` (kill), `spawn` (restart), and `schtasks` (register). No libraries needed.

## Common Pitfalls

### Pitfall 1: Session 0 vs Session 1
**What goes wrong:** Watchdog starts Claude Code in Session 0 (system session) where there's no desktop -- Claude appears to start but user can't see or interact with it.
**Why it happens:** Using `schtasks` with `/ru SYSTEM` or without `/it` flag causes the task to run in Session 0.
**How to avoid:** Always use `/it` (interactive) flag and run under the user account (not SYSTEM). The task must be configured as "Run only when user is logged on."
**Warning signs:** Claude process appears in Task Manager but no terminal window is visible.

### Pitfall 2: Zombie Process Accumulation
**What goes wrong:** Multiple Claude instances accumulate if the watchdog restarts without killing first. The existing PS1 log shows 7-8 zombie instances.
**Why it happens:** Claude spawns child processes (conhost.exe, bash.exe). A simple `process.kill()` doesn't kill the tree. Previous claude.exe instances may still be shutting down when the new one starts.
**How to avoid:** ALWAYS run `taskkill /F /T /IM claude.exe` before spawning a new instance, even if detection says "not running." Wait briefly (1-2 seconds) after kill to let OS clean up handles.
**Warning signs:** Multiple claude.exe entries in tasklist, increasing memory usage.

### Pitfall 3: Version Directory Changes
**What goes wrong:** Watchdog tries to launch an old version's claude.exe after an auto-update, or the directory is deleted.
**Why it happens:** Claude Code auto-updates create new version directories (observed: 2.1.51 and 2.1.72 on this machine). Old versions may be cleaned up.
**How to avoid:** Always resolve the latest version directory at restart time (not at watchdog startup). Use `readdirSync` + sort on each restart.
**Warning signs:** "claude.exe not found" errors in logs.

### Pitfall 4: Spawn Without Detach
**What goes wrong:** Claude Code becomes a child of the watchdog process. If the watchdog exits or restarts, Claude is killed too.
**Why it happens:** Default `spawn()` behavior keeps parent-child relationship.
**How to avoid:** Use `spawn(exe, [], { detached: true, stdio: 'ignore' })` followed by `child.unref()`. This makes Claude independent of the watchdog process.
**Warning signs:** Claude dies whenever the watchdog restarts.

### Pitfall 5: Watchdog Restart Race
**What goes wrong:** Watchdog detects "Claude not running," starts restart, but during the restart delay another poll fires and starts a second restart.
**Why it happens:** The polling interval fires while a restart is still in progress.
**How to avoid:** Set a `#restarting` flag before starting the restart sequence. Skip poll cycles while the flag is set. Clear the flag after restart verification.
**Warning signs:** Multiple Claude instances spawned simultaneously.

### Pitfall 6: Task Scheduler Command Length
**What goes wrong:** Long `/tr` command strings in `schtasks` get truncated or fail.
**Why it happens:** The `/tr` parameter has a 262-character limit.
**How to avoid:** Use a short batch file or wrapper script as the `/tr` target instead of a long `node path/to/file.js` command. Or keep paths short.
**Warning signs:** Scheduled task fails silently on system boot.

## Code Examples

Verified patterns from official sources and project conventions:

### Process Detection (reuse existing pattern)
```javascript
// Source: james/system-metrics.js (existing, proven in production)
import { execFile } from 'node:child_process';

function isClaudeRunning() {
  return new Promise((resolve) => {
    execFile(
      'tasklist',
      ['/NH', '/FI', 'IMAGENAME eq claude.exe'],
      { timeout: 5000 },
      (err, stdout) => {
        if (err) { resolve(false); return; }
        resolve(stdout.toLowerCase().includes('claude.exe'));
      },
    );
  });
}
```

### Zombie Kill (taskkill /F /T)
```javascript
// Source: Microsoft docs for taskkill + Node.js child_process docs
import { execFile } from 'node:child_process';

function killAllClaude() {
  return new Promise((resolve) => {
    execFile(
      'taskkill',
      ['/F', '/T', '/IM', 'claude.exe'],
      { timeout: 10000 },
      (err) => {
        // Error is expected if no processes exist -- that's fine
        resolve();
      },
    );
  });
}
```

### Spawn Detached Process
```javascript
// Source: Node.js v22 child_process docs (detached mode on Windows)
import { spawn } from 'node:child_process';

function launchClaude(exePath) {
  const child = spawn(exePath, [], {
    detached: true,
    stdio: 'ignore',
    cwd: 'C:\\Users\\bono',
  });
  child.unref();
  return child.pid;
}
```

### Task Scheduler Registration
```bash
# Source: Microsoft schtasks create documentation
# /sc onlogon  = trigger on user login
# /it          = interactive only (Session 1)
# /rl highest  = elevated privileges
# /f           = force overwrite existing
schtasks /create ^
  /tn "CommsLink-Watchdog" ^
  /tr "\"C:\Program Files\nodejs\node.exe\" \"C:\Users\bono\racingpoint\comms-link\james\watchdog-runner.js\"" ^
  /sc onlogon ^
  /it ^
  /rl highest ^
  /f
```

### Test Pattern (DI mocking)
```javascript
// Source: Project convention from heartbeat.test.js
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

describe('ClaudeWatchdog', () => {
  it('detects crash and triggers restart', async (t) => {
    t.mock.timers.enable({ apis: ['setInterval', 'setTimeout'] });

    const events = [];
    const watchdog = new ClaudeWatchdog({
      pollMs: 3000,
      detectFn: async () => false,  // Simulate: Claude is not running
      killFn: async () => { events.push('kill'); },
      spawnFn: async () => { events.push('spawn'); return 12345; },
      findExeFn: () => '/fake/claude.exe',
    });

    watchdog.on('crash_detected', () => events.push('crash'));
    watchdog.on('restart_success', () => events.push('restarted'));

    watchdog.start();
    // First poll is immediate
    await Promise.resolve();

    assert.deepEqual(events, ['crash', 'kill', 'spawn', 'restarted']);
    watchdog.stop();
  });
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| PowerShell watchdog via schtasks /sc minute | Node.js integrated watchdog via schtasks /sc onlogon | This phase | Continuous monitoring (3s poll) vs. 1-minute checks; event integration with comms-link |
| Kill zombies by sorting StartTime, keeping newest | Kill ALL instances, then spawn fresh | This phase | Simpler, more reliable -- no race conditions from keeping one alive |
| Cooldown via file timestamp | In-memory flag (Phase 4 core); escalating cooldown in Phase 5 | Phase 4/5 | Phase 4 keeps it simple -- just a `#restarting` flag to prevent double-restart |

**Deprecated/outdated:**
- `claude_watchdog.ps1`: Will be replaced by this phase. Keep it in place until Phase 4 is verified working, then delete.
- NSSM service approach: Explicitly rejected in REQUIREMENTS.md due to Session 0 isolation.

## Open Questions

1. **Post-restart verification method**
   - What we know: Phase 5 (WD-05) requires a self-test after restart. Phase 4 only needs to verify the process is alive (PID exists).
   - What's unclear: Whether checking PID existence is sufficient for Phase 4, or if we should also verify the process stays alive for a few seconds.
   - Recommendation: Phase 4 checks PID existence after a 3-5 second delay. Phase 5 adds the full self-test.

2. **Watchdog-runner vs. integrated into index.js**
   - What we know: Phase 5 (WD-06) will connect watchdog events to WebSocket reconnection. Phase 4 is standalone.
   - What's unclear: Whether the watchdog should run as a separate process from the comms-link, or be integrated.
   - Recommendation: Create `watchdog-runner.js` as the Task Scheduler entry point (standalone). Phase 5 will integrate into `index.js`. This keeps Phase 4 independently testable and deployable.

3. **Old watchdog removal timing**
   - What we know: The existing PS1 watchdog may be registered in Task Scheduler (though current check shows no Claude scheduled tasks).
   - What's unclear: Whether it's running via a different mechanism.
   - Recommendation: Do not delete the PS1 script in Phase 4. Add a note in Phase 5 to retire it after the new watchdog is proven stable.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node.js 22.14.0) |
| Config file | None -- configured via package.json `"test": "node --test test/*.test.js"` |
| Quick run command | `node --test test/watchdog.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WD-01 | Detect Claude crash/exit within 5s (3s poll) | unit | `node --test test/watchdog.test.js` | No -- Wave 0 |
| WD-02 | Kill zombies + restart Claude | unit | `node --test test/watchdog.test.js` | No -- Wave 0 |
| WD-03 | Task Scheduler registration (onlogon, /it, Session 1) | manual | `schtasks /query /tn "CommsLink-Watchdog" /v` | N/A (manual verification) |

### Sampling Rate
- **Per task commit:** `node --test test/watchdog.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/watchdog.test.js` -- covers WD-01 (detection) and WD-02 (kill + restart) via DI mocks
- [ ] No framework install needed -- node:test already in use
- [ ] No shared fixtures needed -- each test creates its own mock functions

## Sources

### Primary (HIGH confidence)
- [Node.js v22 child_process documentation](https://nodejs.org/api/child_process.html) -- spawn detached mode, execFile, Windows behavior
- [Microsoft schtasks create documentation](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-create) -- /sc onlogon, /it, /rl parameters
- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference) -- CLI flags, resume session, startup options
- Existing `claude_watchdog.ps1` at `C:\Users\bono\.claude\claude_watchdog.ps1` -- proven patterns for find exe, kill zombies, restart
- Existing `claude_watchdog.log` at `C:\Users\bono\.claude\claude_watchdog.log` -- real-world zombie accumulation data (7-8 instances)
- Live process inspection -- confirmed Claude exe path: `C:\Users\bono\AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\claude-code\2.1.72\claude.exe`
- Live process tree inspection -- confirmed Claude spawns conhost.exe and bash.exe child processes
- Existing codebase (`system-metrics.js`, `heartbeat-sender.js`) -- project conventions for DI, EventEmitter, testing patterns

### Secondary (MEDIUM confidence)
- [SS64 schtasks reference](https://ss64.com/nt/schtasks.html) -- additional schtasks examples
- [Node.js GitHub issues](https://github.com/nodejs/node/issues/7281) -- Windows process.kill() limitations confirmed

### Tertiary (LOW confidence)
- None -- all findings verified with primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all Node.js built-ins, zero external dependencies, patterns verified in existing codebase
- Architecture: HIGH -- follows exact same EventEmitter + DI + private fields pattern used by CommsClient, HeartbeatSender, HeartbeatMonitor
- Pitfalls: HIGH -- Session 0/1 verified with Microsoft docs, zombie accumulation confirmed by real log data, spawn detach verified in Node.js docs
- Process management: HIGH -- tasklist/taskkill commands verified on this machine, Claude exe path confirmed via live process inspection

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- Windows Task Scheduler and Node.js child_process APIs are mature)
