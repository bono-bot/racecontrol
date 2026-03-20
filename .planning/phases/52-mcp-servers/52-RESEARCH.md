# Phase 52: MCP Servers — Research

**Researched:** 2026-03-20 IST
**Domain:** MCP SDK (Node.js), Google Workspace APIs, racecontrol REST API wrapping
**Confidence:** HIGH — primary source is the existing working codebase (racingpoint-mcp-gmail, racingpoint-google)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Gmail MCP:** Fix existing racingpoint-mcp-gmail — zero code changes to server.js. Only re-authorize OAuth and update settings.json REFRESH_TOKEN.
- **Sheets/Calendar services:** Already exist in racingpoint-google (`services/sheets.js` and `services/calendar.js`). Not net-new — index.js already exports them.
- **Sheets/Calendar MCP servers:** Create new `racingpoint-mcp-sheets` and `racingpoint-mcp-calendar` — follow exact Gmail MCP pattern.
- **Same OAuth credentials:** CLIENT_ID + CLIENT_SECRET + REFRESH_TOKEN shared across Gmail, Drive, Sheets, Calendar — may need additional scopes added to the consent.
- **rc-ops-mcp:** Node.js MCP server on James's machine (.27), talks to `http://192.168.31.23:8080/api/v1/`. Full read+write API surface (~10 tools). No WebSocket subscription (deferred).
- **OAuth auto-refresh script:** Add to racingpoint-google — proactively refreshes tokens before expiry, logs refresh events.

### Claude's Discretion
- MCP tool naming conventions (e.g. `list_inbox` vs `gmail_list_inbox`)
- Sheets API: which methods to expose (read range, write range, list sheets)
- Calendar API: which methods to expose (list events, get event, create event)
- rc-ops-mcp: exact tool names and parameter shapes
- Auto-refresh script scheduling (cron vs startup check)

### Deferred Ideas (OUT OF SCOPE)
- Custom rc-ops-mcp with WebSocket subscription for real-time pod events
- Google Tasks MCP integration
- MCP for Dahua security cameras
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MCP-01 | Claude Code can read Gmail messages via Google Workspace MCP using existing racingpoint-google OAuth | Gmail MCP fix is re-auth only — server.js is complete and working. OAuth flow documented below. |
| MCP-02 | Claude Code can read and write Google Sheets via the same MCP server | sheets.js already exists with readRange + writeRange. New racingpoint-mcp-sheets server needed. |
| MCP-03 | Claude Code can read Google Calendar events via the same MCP server | calendar.js already exists with listEvents + createEvent. New racingpoint-mcp-calendar server needed. |
| MCP-04 | Claude Code can query racecontrol REST API (/fleet/health, /sessions, /billing, /laps) via a custom rc-ops-mcp | routes.rs has full API surface. ~10 priority endpoints identified. node-fetch + MCP SDK pattern. |
</phase_requirements>

---

## Summary

The codebase is more advanced than the CONTEXT.md implies. The `racingpoint-google` package already has `services/sheets.js` and `services/calendar.js` fully implemented — these are NOT net-new writes. The `index.js` already exports them. This means Sheets and Calendar MCP servers are purely scaffolding work (copy Gmail/Drive pattern, import the existing service, register tools).

The Gmail MCP fix is a single credential update — re-run OAuth consent, paste the new REFRESH_TOKEN into settings.json. The server.js code is complete and correct.

The `rc-ops-mcp` is the only genuine build task. It is a pure Node.js MCP server using `node-fetch` (or `axios`) to proxy calls to `http://192.168.31.23:8080/api/v1/`. No auth needed for the racecontrol API (LAN-only, no JWT required on local requests based on routes.rs — `fleet/health` is listed as "public — no auth required").

The OAuth auto-refresh script is a background Node.js process that calls `auth.getAuthClient()`, triggers a token refresh, and persists the new REFRESH_TOKEN to `~/.claude/settings.json`.

**Primary recommendation:** Treat Sheets and Calendar as scaffolding tasks (30 min each). The real work is rc-ops-mcp tool design and the OAuth re-auth procedure.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@modelcontextprotocol/sdk` | `1.27.1` | MCP server framework — `McpServer`, `StdioServerTransport` | Installed and working in racingpoint-mcp-gmail. Exact version confirmed from node_modules. |
| `zod` | `^3.24.0` | Parameter schema validation for MCP tools | Already used in Gmail and Drive MCP servers. Required by MCP SDK's `server.tool()` signature. |
| `@racingpoint/google` | `file:../racingpoint-google` (local) | Shared OAuth helper + service methods | Already used by Gmail and Drive MCP servers. Contains sheets.js and calendar.js. |
| `googleapis` | `^144.0.0` | Google API client — underlying library in racingpoint-google | Used by all services (Gmail, Sheets, Calendar, Drive). Already in racingpoint-google/package.json. |

### Supporting (rc-ops-mcp only)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `node-fetch` | `^3.x` (ESM) | HTTP client for racecontrol REST API calls | rc-ops-mcp only — makes requests to http://192.168.31.23:8080/api/v1/. Node 22 has native fetch; use native `fetch` (no install needed). |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Native `fetch` (Node 22) | `axios` or `node-fetch` | Native fetch is available in Node 22 (confirmed: `node -v` = v22.14.0). No extra dependency for rc-ops-mcp HTTP calls. |
| StdioServerTransport | HTTP transport | stdio is the standard for Claude Code MCP servers — all existing servers use it. HTTP transport is for web-based MCP. |

**Installation (new servers):**
```bash
# For racingpoint-mcp-sheets and racingpoint-mcp-calendar:
cd /c/Users/bono/racingpoint
mkdir racingpoint-mcp-sheets && cd racingpoint-mcp-sheets
npm init -y
npm install @modelcontextprotocol/sdk@1.27.1 zod @racingpoint/google@file:../racingpoint-google

cd /c/Users/bono/racingpoint
mkdir racingpoint-mcp-calendar && cd racingpoint-mcp-calendar
npm init -y
npm install @modelcontextprotocol/sdk@1.27.1 zod @racingpoint/google@file:../racingpoint-google

# For rc-ops-mcp:
cd /c/Users/bono/racingpoint
mkdir rc-ops-mcp && cd rc-ops-mcp
npm init -y
npm install @modelcontextprotocol/sdk@1.27.1 zod
```

---

## Architecture Patterns

### Established MCP Server Pattern (from racingpoint-mcp-gmail/server.js)

Every new MCP server follows this exact pattern — no deviation:

```javascript
#!/usr/bin/env node
// Source: C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { getAuthClient } = require('@racingpoint/google');
const { methodA, methodB } = require('@racingpoint/google/services/TARGET');

const CLIENT_ID = process.env.GOOGLE_CLIENT_ID;
const CLIENT_SECRET = process.env.GOOGLE_CLIENT_SECRET;
const REFRESH_TOKEN = process.env.GOOGLE_REFRESH_TOKEN;

if (!CLIENT_ID || !CLIENT_SECRET || !REFRESH_TOKEN) {
  console.error('Required environment variables: GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN');
  process.exit(1);
}

function getAuth() {
  return getAuthClient({ clientId: CLIENT_ID, clientSecret: CLIENT_SECRET, refreshToken: REFRESH_TOKEN });
}

const server = new McpServer({ name: 'racingpoint-TARGET', version: '1.0.0' });

server.tool('tool_name', 'description', { param: z.string().describe('...') }, async ({ param }) => {
  const auth = await getAuth();
  const result = await methodA({ auth, param });
  return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
});

const transport = new StdioServerTransport();
await server.connect(transport);
```

**Key observations from the pattern:**
- `type: "module"` in package.json (ES module)
- `createRequire(import.meta.url)` bridges ESM server.js → CJS racingpoint-google package
- `getAuthClient()` is called inside each tool handler (not at startup) — this is correct, it memoizes internally
- Tool returns `{ content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] }`
- All three env vars must be present or process exits

### settings.json MCP Entry Pattern (from ~/.claude/settings.json)

```json
"racingpoint-TARGET": {
  "command": "C:\\Program Files\\nodejs\\node.exe",
  "args": ["C:\\Users\\bono\\racingpoint\\racingpoint-mcp-TARGET\\server.js"],
  "env": {
    "GOOGLE_CLIENT_ID": "<see ~/.claude/settings.json>",
    "GOOGLE_CLIENT_SECRET": "<see ~/.claude/settings.json>",
    "GOOGLE_REFRESH_TOKEN": "<NEW_REFRESH_TOKEN_AFTER_REAUTH>"
  }
}
```

**For rc-ops-mcp** (no Google OAuth needed):
```json
"rc-ops-mcp": {
  "command": "C:\\Program Files\\nodejs\\node.exe",
  "args": ["C:\\Users\\bono\\racingpoint\\rc-ops-mcp\\server.js"],
  "env": {
    "RACECONTROL_BASE_URL": "http://192.168.31.23:8080/api/v1"
  }
}
```

### Existing Services — Already Implemented

**sheets.js** (`C:\Users\bono\racingpoint\racingpoint-google\services\sheets.js`):
- `readRange({ auth, spreadsheetId, range })` — reads cell values, returns 2D array
- `writeRange({ auth, spreadsheetId, range, values, append })` — writes or appends rows
- `extractSpreadsheetId(input)` — accepts full URL or raw sheet ID
- API scope needed: `https://www.googleapis.com/auth/spreadsheets`

**calendar.js** (`C:\Users\bono\racingpoint\racingpoint-google\services\calendar.js`):
- `listEvents({ auth, calendarId, maxResults })` — lists future events from `timeMin: new Date()`, IST timezone
- `createEvent({ auth, calendarId, summary, start, end, description, location, attendees })` — creates event with IST timezone
- `deleteEvent({ auth, calendarId, eventId })` — deletes by event ID
- API scope needed: `https://www.googleapis.com/auth/calendar`

### racecontrol API — Full Route Map (from routes.rs)

Base URL: `http://192.168.31.23:8080/api/v1`

**Priority endpoints for rc-ops-mcp tools:**

| Tool Name | Endpoint | Method | Notes |
|-----------|----------|--------|-------|
| `get_fleet_health` | `/fleet/health` | GET | Returns array of PodFleetStatus — ws_connected, http_reachable, version, build_id, uptime_secs, last_seen, ip_address |
| `list_sessions` | `/sessions` | GET | Active and recent sessions |
| `get_active_billing` | `/billing/active` | GET | Currently billing sessions |
| `list_laps` | `/laps` | GET | Recent laps (accepts query params) |
| `list_drivers` | `/drivers` | GET | All registered drivers |
| `stop_billing` | `/billing/{id}/stop` | POST | End a billing session |
| `exec_on_pod` | `/pods/{id}/exec` | POST | Body: `{ "cmd": "...", "timeout_ms": 30000 }`. Proxies to rc-agent via WS. |
| `pod_self_test` | `/pods/{id}/self-test` | GET | Triggers 30s self-test — returns probe results + LLM verdict |
| `get_daily_report` | `/billing/report/daily` | GET | Daily billing summary |
| `list_billing_sessions` | `/billing/sessions` | GET | Historical billing sessions |

**Note on `/fleet/exec`:** CONTEXT.md mentions `/fleet/exec` but routes.rs only has `/pods/{id}/exec`. There is no bulk fleet exec route. The per-pod exec is the correct endpoint. Use `exec_on_pod` per-pod in rc-ops-mcp.

**PodFleetStatus fields** (from fleet_health.rs):
```
pod_number, pod_id, ws_connected, http_reachable, version, build_id,
uptime_secs, crash_recovery, ip_address, last_seen, last_http_check
```

### OAuth Re-Authorization Procedure

The Gmail MCP is broken because REFRESH_TOKEN expired. The fix:

1. Run the OAuth consent flow in racingpoint-google to get a new refresh token
2. Check if racingpoint-google has a re-auth script:

```bash
ls /c/Users/bono/racingpoint/racingpoint-google/
```

3. If no re-auth script, use the Google OAuth 2.0 Playground at https://developers.google.com/oauthplayground with the existing CLIENT_ID and CLIENT_SECRET, authorize with scopes:
   - `https://mail.google.com/`
   - `https://www.googleapis.com/auth/spreadsheets`
   - `https://www.googleapis.com/auth/calendar`
   - `https://www.googleapis.com/auth/drive`
4. Exchange auth code for tokens → copy refresh_token
5. Update settings.json GOOGLE_REFRESH_TOKEN in all 3 MCP entries (gmail, drive, sheets, calendar)

**Critical:** The googleapis Node.js client (`auth.js`) uses `google.auth.OAuth2` with `setCredentials({ refresh_token })`. The googleapis library auto-refreshes access tokens when they expire using the refresh token. The ONLY thing that breaks is when the refresh token itself expires (happens when OAuth consent app is in "Testing" mode — tokens expire after 7 days unless app is published to production).

### Auto-Refresh Script Pattern

The auto-refresh script should call `authClient.refreshAccessToken()` proactively and persist the result. However, the googleapis OAuth2 client handles access token refresh automatically — the real fix is ensuring the app is set to "In production" status in Google Cloud Console so refresh tokens do not expire.

```javascript
// racingpoint-google/refresh-token.js
// Standalone script — run manually or on schedule
const { getAuthClient } = require('./auth');

async function refreshAndReport({ clientId, clientSecret, refreshToken }) {
  const auth = getAuthClient({ clientId, clientSecret, refreshToken });
  const { credentials } = await auth.refreshAccessToken();
  const expiry = new Date(credentials.expiry_date).toISOString();
  console.log(`[${new Date().toISOString()}] Token refreshed. Expires: ${expiry}`);
  // If a new refresh token is returned (rare), log it for manual settings.json update
  if (credentials.refresh_token && credentials.refresh_token !== refreshToken) {
    console.log(`NEW REFRESH TOKEN: ${credentials.refresh_token}`);
  }
}
```

**Important:** Google only issues a new refresh token during the initial consent flow. `refreshAccessToken()` refreshes the ACCESS token (short-lived, 1h), not the refresh token itself. The auto-refresh script's value is (1) confirming the refresh token still works on schedule, and (2) logging expiry warnings. The real prevention is setting the app to production mode.

### Anti-Patterns to Avoid

- **Calling `getAuthClient()` at module load time:** The Gmail/Drive servers call it inside each tool handler. This is correct — the memoized singleton works fine. Do not move it to module scope (would fail if env vars are missing at startup).
- **Creating a separate racingpoint-google `services/sheets.js`:** Already exists. Do NOT create it again.
- **Using `axios` for rc-ops-mcp:** Node 22 has native `fetch`. No extra dependency needed.
- **Putting RACECONTROL_BASE_URL hardcoded in rc-ops-mcp:** Use env var `RACECONTROL_BASE_URL` (default `http://192.168.31.23:8080/api/v1`) so the base can be overridden without code change.
- **Using `"type": "commonjs"` in new MCP servers:** All existing servers use `"type": "module"` (ESM) in package.json. Stay consistent.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OAuth access token refresh | Custom HTTP call to Google token endpoint | `googleapis` OAuth2 client auto-refresh | googleapis calls `refreshAccessToken()` transparently when access token expires. No manual token management needed. |
| MCP tool schema validation | Custom JSON validation | `zod` schemas in `server.tool()` | MCP SDK calls zod `.parse()` before invoking handler — type errors returned as MCP errors automatically. |
| Google Sheets URL parsing | Custom regex | `extractSpreadsheetId()` in sheets.js | Already written. Accepts both full URL and raw ID. |
| racecontrol response parsing | Custom response shapes | Pass JSON directly to MCP text content | Claude interprets JSON natively. Don't transform — return `JSON.stringify(result, null, 2)` directly. |

---

## Common Pitfalls

### Pitfall 1: OAuth App in "Testing" Mode — 7-Day Refresh Token Expiry
**What goes wrong:** Google OAuth apps in "Testing" mode expire refresh tokens after 7 days. This is why the Gmail MCP broke.
**Why it happens:** Google Cloud Console OAuth consent screen has two states: Testing (limited users, tokens expire in 7 days) and In production (tokens valid indefinitely until revoked).
**How to avoid:** After re-authorizing, set the OAuth consent screen to "In production" in Google Cloud Console. This does not require app review for personal/workspace apps used by a single user.
**Warning signs:** Gmail MCP worked for 7 days after setup, then broke. Pattern repeats.

### Pitfall 2: Missing Scopes After Re-Auth
**What goes wrong:** Re-authorizing for Gmail with `https://mail.google.com/` only. Then Sheets/Calendar MCP fails because spreadsheets + calendar scopes were not included.
**Why it happens:** OAuth consent authorizes a specific set of scopes. Adding new scopes requires a new consent flow.
**How to avoid:** Authorize ALL scopes in one consent flow: mail.google.com + spreadsheets + calendar + drive. Single refresh token covers all four MCP servers.
**Warning signs:** Gmail tools work but Sheets/Calendar tools return 403.

### Pitfall 3: `createRequire` is Required for CJS Interop
**What goes wrong:** `import { getAuthClient } from '@racingpoint/google'` fails with "ERR_REQUIRE_ESM" or similar.
**Why it happens:** `racingpoint-google` is `"type": "commonjs"` (no type field = CJS default). MCP servers are `"type": "module"` (ESM). Direct import fails.
**How to avoid:** Use the established pattern: `const require = createRequire(import.meta.url); const { getAuthClient } = require('@racingpoint/google');`
**Warning signs:** Server crashes at startup with module resolution error.

### Pitfall 4: racecontrol API Authentication
**What goes wrong:** rc-ops-mcp returns 401 Unauthorized on all requests.
**Why it happens:** Some racecontrol endpoints require authentication (JWT tokens from `/customer/login` or terminal PIN auth).
**How to avoid:** `/fleet/health` is confirmed public (routes.rs comment: "public — no auth required"). Check other endpoints. For the ~10 priority tools, most read-only fleet/billing endpoints are either public or only require LAN access (no auth on LAN from observed patterns). If auth is needed, add a `RACECONTROL_API_TOKEN` env var to rc-ops-mcp settings.json.
**Warning signs:** 401 response from specific endpoints despite correct base URL.

### Pitfall 5: `fleet/exec` Does Not Exist
**What goes wrong:** CONTEXT.md mentions `/fleet/exec` as an endpoint to expose. This route does NOT exist in routes.rs.
**Why it happens:** CONTEXT.md was written from memory/description, not from reading routes.rs directly.
**How to avoid:** Use `/pods/{id}/exec` (POST) instead. This is the correct per-pod exec endpoint. To exec on multiple pods, call it multiple times.
**Warning signs:** 404 on POST /fleet/exec.

---

## Code Examples

### Sheets MCP server.js skeleton
```javascript
// Source: pattern from C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js
#!/usr/bin/env node
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { getAuthClient } = require('@racingpoint/google');
const { readRange, writeRange } = require('@racingpoint/google/services/sheets');

// ... env var checks + getAuth() ...

server.tool('read_sheet', 'Read a range from a Google Sheet.', {
  spreadsheetId: z.string().describe('Sheet ID or full URL'),
  range: z.string().describe('A1 notation range, e.g. "Sheet1!A1:D10"'),
}, async ({ spreadsheetId, range }) => {
  const auth = await getAuth();
  const values = await readRange({ auth, spreadsheetId, range });
  return { content: [{ type: 'text', text: JSON.stringify(values, null, 2) }] };
});
```

### rc-ops-mcp fleet health tool
```javascript
// Source: routes.rs GET /fleet/health
server.tool('get_fleet_health', 'Get real-time status of all 8 pods. Returns ws_connected, http_reachable, version, uptime_secs, ip_address per pod.', {}, async () => {
  const res = await fetch(`${process.env.RACECONTROL_BASE_URL}/fleet/health`);
  const data = await res.json();
  return { content: [{ type: 'text', text: JSON.stringify(data, null, 2) }] };
});
```

### rc-ops-mcp pod exec tool
```javascript
// Source: routes.rs POST /pods/{id}/exec
server.tool('exec_on_pod', 'Execute a command on a specific pod via rc-agent. Pod ID format: "pod-1" through "pod-8".', {
  podId: z.string().describe('Pod ID, e.g. "pod-1"'),
  cmd: z.string().describe('Command to execute on the pod'),
  timeoutMs: z.number().default(30000).describe('Timeout in milliseconds'),
}, async ({ podId, cmd, timeoutMs }) => {
  const res = await fetch(`${process.env.RACECONTROL_BASE_URL}/pods/${podId}/exec`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ cmd, timeout_ms: timeoutMs }),
  });
  const data = await res.json();
  return { content: [{ type: 'text', text: JSON.stringify(data, null, 2) }] };
});
```

---

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| Manual curl to racecontrol API | rc-ops-mcp — Claude calls tools directly | Phase 52 goal |
| Google OAuth playground one-time token | googleapis auto-refresh | Already implemented in auth.js |
| Fixed OAuth scope per app | All scopes in single refresh token | One re-auth covers all 4 MCP servers |

**What already exists (confirmed from codebase):**
- `racingpoint-mcp-gmail/server.js` — complete, working, 5 tools
- `racingpoint-mcp-drive/server.js` — complete, working, 3 tools
- `racingpoint-google/services/sheets.js` — complete, 2 methods
- `racingpoint-google/services/calendar.js` — complete, 3 methods
- `racingpoint-google/index.js` — already exports sheets and calendar
- `~/.claude/settings.json` — already has `racingpoint-gmail` and `racingpoint-drive` entries with all three env vars

**What needs to be built:**
- `racingpoint-mcp-sheets/` — new dir + package.json + server.js (~60 lines)
- `racingpoint-mcp-calendar/` — new dir + package.json + server.js (~60 lines)
- `rc-ops-mcp/` — new dir + package.json + server.js (~120 lines, ~10 tools)
- `racingpoint-google/refresh-token.js` — auto-refresh utility script
- `~/.claude/settings.json` — add 3 new mcpServers entries
- Re-authorize OAuth and update REFRESH_TOKEN in settings.json

---

## Open Questions

1. **racecontrol endpoint auth requirements**
   - What we know: `/fleet/health` is explicitly marked public in routes.rs
   - What's unclear: Do `/sessions`, `/billing/active`, `/laps`, `/drivers` require auth? The routes.rs doesn't show auth middleware clearly from the grep output.
   - Recommendation: Test with `curl http://192.168.31.23:8080/api/v1/sessions` from James's machine. If 401, check routes.rs for auth middleware layer. For initial build, assume public (LAN-only).

2. **OAuth scope coverage**
   - What we know: Current REFRESH_TOKEN was authorized for Gmail + Drive only (original setup)
   - What's unclear: Whether it covers spreadsheets + calendar scopes
   - Recommendation: Re-authorize with all 4 scopes in one flow regardless. New REFRESH_TOKEN replaces old across all entries.

3. **racingpoint-google re-auth script existence**
   - What we know: index.js exports getAuthClient, no re-auth script found in directory listing
   - What's unclear: Whether a setup script exists elsewhere
   - Recommendation: Check `ls /c/Users/bono/racingpoint/racingpoint-google/` in plan execution. If no re-auth script, use OAuth playground or write a simple `get-token.js` script.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual smoke test (no automated test framework for MCP servers) |
| Config file | none |
| Quick run command | Ask Claude: "list my latest emails" + "read sheet [ID]!A1:B5" |
| Full suite command | Test each MCP tool once manually after settings.json update |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MCP-01 | `list_emails` returns inbox | manual-only | `echo '{"tool":"list_emails","args":{"maxResults":3}}' \| node server.js` | N/A |
| MCP-02 | `read_sheet` returns cell values | manual-only | Ask Claude to read a known sheet range | N/A |
| MCP-03 | `list_events` returns calendar events | manual-only | Ask Claude to list upcoming events | N/A |
| MCP-04 | `get_fleet_health` returns pod status array | smoke | `curl http://192.168.31.23:8080/api/v1/fleet/health` (verifies underlying API) | N/A — Wave 0 |

**Justification for manual-only:** MCP servers use stdio transport — there is no HTTP interface to test programmatically without a full MCP client. The authoritative test is Claude successfully calling each tool in a real session.

### Sampling Rate
- **Per task commit:** Verify the specific MCP server starts without errors: `node server.js` (should hang waiting for stdio, Ctrl+C after confirming no crash)
- **Per wave merge:** Ask Claude to invoke one tool per MCP server
- **Phase gate:** All 4 requirement tools working in a live Claude Code session before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] No automated test infrastructure for MCP servers — manual verification is the standard
- [ ] `curl http://192.168.31.23:8080/api/v1/sessions` — verify auth requirements before rc-ops-mcp build

---

## Sources

### Primary (HIGH confidence)
- `C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js` — definitive MCP pattern (McpServer + StdioServerTransport + createRequire CJS bridge)
- `C:\Users\bono\racingpoint\racingpoint-mcp-drive\server.js` — second MCP example confirming pattern
- `C:\Users\bono\racingpoint\racingpoint-google\services\sheets.js` — confirmed complete implementation
- `C:\Users\bono\racingpoint\racingpoint-google\services\calendar.js` — confirmed complete implementation
- `C:\Users\bono\racingpoint\racingpoint-google\index.js` — confirmed exports sheets + calendar already
- `C:\Users\bono\racingpoint\racecontrol\crates\racecontrol\src\api\routes.rs` — full route table
- `C:\Users\bono\racingpoint\racecontrol\crates\racecontrol\src\fleet_health.rs` — PodFleetStatus shape
- `C:\Users\bono\.claude\settings.json` — current MCP entries, env var pattern, OAuth credentials

### Secondary (MEDIUM confidence)
- `@modelcontextprotocol/sdk@1.27.1` installed version — confirmed from node_modules
- `googleapis@^144.0.0` — confirmed from racingpoint-google/package.json

### Tertiary (LOW confidence)
- Google OAuth "Testing" mode 7-day expiry — based on training knowledge; confirmed by pattern (MCP worked initially, broke after ~7 days per STATE.md "Gmail OAuth tokens expired")

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries confirmed from installed node_modules and working servers
- Architecture patterns: HIGH — extracted directly from working server.js files
- racecontrol API surface: HIGH — read directly from routes.rs
- Service method signatures: HIGH — read directly from sheets.js and calendar.js
- OAuth pitfalls: MEDIUM-HIGH — pattern evidence from broken state + googleapis docs behavior

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (MCP SDK stable, googleapis stable, racecontrol routes stable)
