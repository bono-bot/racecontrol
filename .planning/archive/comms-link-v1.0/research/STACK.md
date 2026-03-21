# Technology Stack

**Project:** James-Bono Comms Link
**Researched:** 2026-03-12

## Recommended Stack

### Runtime & Language

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Node.js | 22.x LTS | Runtime for both sides | Already installed on James (v22.14.0), available on Bono's VPS. LTS = stable for production. Both sides speak the same language. | HIGH |
| ESM modules | `"type": "module"` | Module system | Matches existing Racing Point repos (racingpoint-mcp-gmail uses ESM). Modern, clean `import/export`. No reason to use CommonJS in 2026. | HIGH |
| No TypeScript | N/A | Skip TS compilation | This is infrastructure glue, not a large app. ~500 lines total across all files. TS adds build step complexity for zero benefit at this scale. Plain JS with JSDoc comments where needed. | HIGH |

### WebSocket (Core Communication)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `ws` | ^8.19.0 | WebSocket client (James) + server (Bono) | The standard. 22.7k GitHub stars, actively maintained (releases in 2025-2026), RFC 6455 compliant, minimal overhead. Used by Socket.IO internally. No abstractions we don't need. | HIGH |

**Why NOT Socket.IO:** Socket.IO adds ~100KB of overhead, rooms/namespaces/fallback-to-polling we don't need. This is a 1:1 connection between two known endpoints. `ws` gives us exactly what we need: raw WebSocket with ping/pong support built into the protocol. Socket.IO would be overengineering.

**Why NOT reconnecting-websocket:** Last published 6 years ago (v4.4.0, 2020). Unmaintained. The reconnection logic is ~30 lines of code with exponential backoff -- trivial to write ourselves on top of `ws`, and we control the behavior exactly.

**Why NOT uWebSockets.js:** Performance overkill for a single connection. Adds C++ compilation complexity. Not worth it when `ws` handles our load (one message every few seconds) without breaking a sweat.

### Process Management (Watchdog)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Node.js `child_process` | built-in | Spawn/monitor Claude Code | Built into Node.js. `spawn()` gives us PID tracking, exit event handling, stdin/stdout pipes. No external dependency needed. | HIGH |
| `tasklist` / `taskkill` | Windows built-in | Process detection & cleanup | Shell out via `execFileSync('tasklist', [...])` to detect running Claude processes. `execFileSync('taskkill', ['/F', '/T', '/PID', pid])` for cleanup. Already proven in rc-agent deployments. | HIGH |
| NSSM | 2.24 | Run watchdog as Windows service | Registers the watchdog Node.js script as a Windows service so it starts on boot and survives user logoff. Battle-tested (15+ years), single .exe, no dependencies. Download from nssm.cc. | MEDIUM |

**Security note:** Always use `execFile` / `execFileSync` (from `node:child_process`) instead of `exec` / `execSync`. `execFile` bypasses the shell, preventing command injection. This is especially important since process names could theoretically be tainted.

**Why NSSM over node-windows:** `node-windows` is stuck at 1.0.0-beta.8, last published ~3 years ago. Still beta after years. NSSM is proven, works with any executable, and James already uses similar patterns (HKLM Run key for rc-agent). NSSM is more reliable for a production watchdog.

**Why NSSM over WinSW:** WinSW requires .NET Framework or shipping a .NET 7 native binary. NSSM is a single 300KB .exe with zero dependencies. Simpler.

**Why NSSM over Servy:** Servy is newer (2025) and feature-rich, but unproven for this use case. NSSM's simplicity is a feature -- we don't need a desktop monitoring UI for a headless watchdog.

**Why NOT PM2 on James:** PM2 has spotty Windows support. The Windows file watcher doesn't work correctly, and service installation requires hacks. PM2 is great on Linux (Bono already uses it for WhatsApp bot) but not reliable on Windows.

### HTTP Client (Evolution API + Gmail Fallback)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `fetch` (global) | built-in | HTTP requests to Evolution API, Gmail MCP | Node.js 22 has stable global `fetch` (based on undici). Zero dependencies. Perfect for REST API calls to Evolution API and Gmail. | HIGH |

**Why NOT axios:** Global `fetch` is built into Node.js 18+. Adding axios for a handful of HTTP calls is unnecessary dependency weight. `fetch` API is standardized and sufficient.

### File Operations (LOGBOOK.md Sync)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `node:fs/promises` | built-in | Read/write LOGBOOK.md | Built-in async file I/O. `readFile()` and `writeFile()` cover our needs. | HIGH |
| `node:crypto` | built-in | Content hashing for sync | SHA-256 hash of file content to detect changes without sending full file every heartbeat. Only sync when hash differs. | HIGH |

### WhatsApp Alerts (Evolution API)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Evolution API v2 | REST API | Send WhatsApp alerts to Uday | Already running on Bono's VPS. REST endpoint: `POST /message/sendText/{instance}`. Send with `fetch()`. No npm package needed -- it's a simple HTTP call. | HIGH |

**Evolution API endpoint:**
```
POST https://{server-url}/message/sendText/{instance}
Headers: Content-Type: application/json, apikey: {key}
Body: { "number": "91XXXXXXXXXX", "text": "James is down" }
```

### Email Fallback (Gmail API)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `@racingpoint/google` | local | Gmail API via shared OAuth | Already exists at `../racingpoint-google`. Proven, handles token refresh. Import and use `sendEmail()`. | HIGH |
| MCP Gmail server | running | Alternative: call MCP server | The racingpoint-mcp-gmail server is already running. Could call it via its HTTP transport if simpler than importing the Google package directly. | MEDIUM |

### Configuration

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| JSON config file | N/A | `comms-link.json` | Simple, no parser dependency. Node.js has `JSON.parse()` built in. Store server URL, heartbeat interval, Claude Code command, logbook path, API keys. | HIGH |
| `dotenv` | N/A | NOT using | API keys go in the JSON config file (not committed). `.env` adds a dependency for no benefit when we have one config file. | HIGH |

## Complete Dependency List

### James Side (WebSocket Client + Watchdog)

```json
{
  "name": "@racingpoint/comms-link",
  "version": "1.0.0",
  "type": "module",
  "dependencies": {
    "ws": "^8.19.0"
  }
}
```

That's it. **One dependency.** Everything else is Node.js built-ins:
- `node:child_process` -- process spawning and monitoring (use `execFile`, never `exec`)
- `node:fs/promises` -- file I/O for LOGBOOK.md
- `node:crypto` -- content hashing
- `fetch` (global) -- HTTP calls to Evolution API
- `node:timers` -- setInterval for heartbeat
- `node:os` -- hostname/platform info in heartbeat

### Bono Side (WebSocket Server)

```json
{
  "name": "@racingpoint/comms-link-server",
  "version": "1.0.0",
  "type": "module",
  "dependencies": {
    "ws": "^8.19.0"
  }
}
```

Same single dependency. The server creates a `WebSocketServer` on a port, accepts James's connection, handles heartbeat, and dispatches messages.

### System-Level (Not npm)

| Tool | Where | Purpose | Install |
|------|-------|---------|---------|
| NSSM 2.24 | James only | Register watchdog as Windows service | Download from nssm.cc, place in PATH |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| WebSocket | `ws` | Socket.IO | Overhead, rooms/namespaces unused, 1:1 connection doesn't need it |
| WebSocket | `ws` | `reconnecting-websocket` | Abandoned (6 years), trivial to implement ourselves |
| WebSocket | `ws` | `uWebSockets.js` | C++ compilation, overkill for single connection |
| Service mgr | NSSM | `node-windows` | Stuck in beta (1.0.0-beta.8), unmaintained 3 years |
| Service mgr | NSSM | WinSW | Requires .NET runtime, more complex |
| Service mgr | NSSM | PM2 | Poor Windows support, file watcher issues |
| Service mgr | NSSM | Servy | Too new, unproven, feature bloat for this use case |
| HTTP client | global `fetch` | axios | Unnecessary dependency, `fetch` is built-in on Node 22 |
| Config | JSON file | dotenv / .env | One config file doesn't need a parser library |
| Language | JavaScript (ESM) | TypeScript | Build step overhead for ~500 lines of infrastructure glue |
| Module system | ESM | CommonJS | Matches existing repos, modern standard |

## Architecture Decision: Why Minimal Dependencies

This project has exactly **one npm dependency** (`ws`) per side. This is intentional:

1. **Reliability** -- Fewer dependencies = fewer things to break. A watchdog must be rock-solid.
2. **Security** -- Smaller supply chain attack surface. Critical infrastructure should minimize third-party code.
3. **Maintainability** -- No dependency updates to chase. `ws` is stable and mature.
4. **Startup speed** -- Watchdog must start fast on boot. Fewer modules = faster module resolution.
5. **Debuggability** -- When something breaks at 2 AM, you want to read your own code, not dig through `node_modules`.

Node.js 22 has everything we need built in: `fetch`, `crypto`, `fs/promises`, `child_process`, `timers`, `events`, `os`.

## Key Patterns to Implement (Not Libraries)

These are patterns we write ourselves, not libraries we install:

### 1. Reconnecting WebSocket Client (~30 lines)
```javascript
// Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 30s
function connect() {
  const ws = new WebSocket(url);
  ws.on('open', () => { backoff = 1000; });
  ws.on('close', () => {
    setTimeout(connect, backoff);
    backoff = Math.min(backoff * 2, 30000);
  });
  ws.on('error', () => {}); // close event always follows error
}
```

### 2. Heartbeat with Ping/Pong (~20 lines)
```javascript
// Server pings, client pongs (automatic per RFC 6455)
// Server detects dead client if no pong within interval
const HEARTBEAT_INTERVAL = 15000; // 15 seconds
setInterval(() => {
  if (!ws.isAlive) return ws.terminate();
  ws.isAlive = false;
  ws.ping();
}, HEARTBEAT_INTERVAL);
ws.on('pong', () => { ws.isAlive = true; });
```

### 3. Process Monitor (~40 lines)
```javascript
// Check if claude process is running, restart if not
import { execFileSync, spawn } from 'node:child_process';

function isRunning(processName) {
  try {
    const out = execFileSync('tasklist', [
      '/FI', `IMAGENAME eq ${processName}`,
      '/NH'
    ], { encoding: 'utf8' });
    return out.includes(processName);
  } catch {
    return false;
  }
}
```

### 4. File Sync via Hash (~25 lines)
```javascript
// Only send file when content changes
import { createHash } from 'node:crypto';

function fileHash(content) {
  return createHash('sha256').update(content).digest('hex');
}

const hash = fileHash(content);
if (hash !== lastHash) {
  ws.send(JSON.stringify({ type: 'logbook-sync', content, hash }));
  lastHash = hash;
}
```

## NSSM Installation (One-Time Setup)

```bash
# Download NSSM from nssm.cc
# Place nssm.exe in C:\RacingPoint\tools\

# Install watchdog as service
nssm install CommsLinkWatchdog "C:\Program Files\nodejs\node.exe" "C:\Users\bono\racingpoint\comms-link\watchdog.js"
nssm set CommsLinkWatchdog AppDirectory "C:\Users\bono\racingpoint\comms-link"
nssm set CommsLinkWatchdog Start SERVICE_AUTO_START
nssm set CommsLinkWatchdog AppStdout "C:\Users\bono\racingpoint\comms-link\logs\watchdog.log"
nssm set CommsLinkWatchdog AppStderr "C:\Users\bono\racingpoint\comms-link\logs\watchdog-error.log"
nssm set CommsLinkWatchdog AppRotateFiles 1
nssm set CommsLinkWatchdog AppRotateBytes 1048576

# Start the service
nssm start CommsLinkWatchdog
```

## Sources

- [ws npm package](https://www.npmjs.com/package/ws) -- v8.19.0, 22.7k GitHub stars, actively maintained
- [ws GitHub repository](https://github.com/websockets/ws) -- Heartbeat/ping-pong examples in README
- [node-windows npm](https://www.npmjs.com/package/node-windows) -- v1.0.0-beta.8, last published ~3 years ago (stale)
- [NSSM](https://nssm.cc/) -- Non-Sucking Service Manager, battle-tested Windows service wrapper
- [Servy vs NSSM vs WinSW comparison](https://dev.to/aelassas/servy-vs-nssm-vs-winsw-2k46) -- 2025 comparison article
- [Evolution API docs](https://doc.evolution-api.com/v2/api-reference/message-controller/send-text) -- v2 send text endpoint
- [Evolution API GitHub](https://github.com/EvolutionAPI/evolution-api) -- Open-source WhatsApp integration
- [Node.js child_process docs](https://nodejs.org/api/child_process.html) -- spawn, execFile, execFileSync
- [WebSocket heartbeat patterns](https://oneuptime.com/blog/post/2026-01-24-websocket-heartbeat-ping-pong/view) -- Jan 2026 guide
- [reconnecting-websocket npm](https://www.npmjs.com/package/reconnecting-websocket) -- v4.4.0, last published ~6 years ago (abandoned)
- [Socket.IO vs WebSocket](https://velt.dev/blog/socketio-vs-websocket-guide-developers) -- Sept 2025 comparison
- [WebSocket complete guide 2026](https://devtoolbox.dedyn.io/blog/websocket-complete-guide) -- Current best practices
