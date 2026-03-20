# Phase 52: MCP Servers - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Claude Code can query Gmail, Google Sheets, Google Calendar, and the racecontrol REST API directly from any session — James describes what he needs in plain language and Claude fetches live data without manual curl or browser lookups.

Deliverables: Fix existing Gmail MCP (re-auth OAuth), add Sheets + Calendar services to racingpoint-google, create racingpoint-mcp-sheets and racingpoint-mcp-calendar servers, build rc-ops-mcp wrapping racecontrol full API, add auto-refresh script for OAuth tokens.

</domain>

<decisions>
## Implementation Decisions

### Gmail MCP Approach
- **Fix existing** racingpoint-mcp-gmail — do NOT replace with google_workspace_mcp
- The server.js already works (sendEmail, listInbox, readEmail, markAsRead, replyEmail)
- Only the OAuth refresh token is expired — re-authorize and update settings.json env vars
- Zero code changes to racingpoint-mcp-gmail itself

### Sheets/Calendar Access
- **Extend racingpoint-google** package with new services: `services/sheets.js` and `services/calendar.js`
- Create separate MCP servers: `racingpoint-mcp-sheets` and `racingpoint-mcp-calendar`
- Follow the exact same pattern as racingpoint-mcp-gmail (MCP SDK + StdioServerTransport + racingpoint-google OAuth)
- Same OAuth credentials (CLIENT_ID, CLIENT_SECRET, REFRESH_TOKEN) — may need additional scopes (spreadsheets, calendar.readonly)

### rc-ops-mcp Scope
- **Full API surface** — expose all racecontrol REST endpoints as MCP tools (~10 tools)
- **Full read/write** — includes write endpoints (start/stop billing, exec commands, pod restart)
- Endpoints to expose: /fleet/health, /sessions, /billing, /laps, /drivers, /config, /fleet/exec, /pods/{id}/self-test
- Node.js MCP server running on James's machine (.27), talks to server :8080
- This is model-invocable — Claude can query and act on racecontrol state directly

### OAuth Re-authorization
- Re-run OAuth consent flow manually to get new refresh token
- Update settings.json env vars with new REFRESH_TOKEN
- **Add auto-refresh script** to racingpoint-google that refreshes tokens before expiry — prevents future breakage
- Script checks token expiry, refreshes proactively, updates stored token

### Claude's Discretion
- MCP tool naming conventions (list_inbox vs gmail_list_inbox)
- Sheets API: which methods to expose (read range, write range, list sheets, etc.)
- Calendar API: which methods to expose (list events, get event, etc.)
- rc-ops-mcp: exact tool names and parameter shapes
- Auto-refresh script scheduling (cron vs startup check)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing MCP Infrastructure
- `C:\Users\bono\.claude\settings.json` — Current MCP server configs, OAuth credentials, env vars
- `C:\Users\bono\racingpoint\racingpoint-mcp-gmail\server.js` — Working Gmail MCP pattern (McpServer + StdioServerTransport + zod)
- `C:\Users\bono\racingpoint\racingpoint-google\` — Shared OAuth package (auth.js, services/gmail.js)

### Research
- `.planning/research/FEATURES.md` — MCP feature landscape, table stakes vs differentiators
- `.planning/research/ARCHITECTURE.md` — Where MCP servers fit in system architecture
- `.planning/research/PITFALLS.md` — OAuth token expiry pitfall (#2)

### Racecontrol API
- `crates/racecontrol/src/api/` — API route definitions (routes.rs)
- `CLAUDE.md` §Fleet Endpoints — endpoint paths and PodFleetStatus fields

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `racingpoint-mcp-gmail/server.js` — complete MCP server template using @modelcontextprotocol/sdk
- `racingpoint-google/auth.js` — OAuth getAuthClient() with CLIENT_ID/SECRET/REFRESH_TOKEN
- `racingpoint-google/services/gmail.js` — Gmail service methods (sendEmail, listInbox, etc.)
- `racingpoint-mcp-drive/server.js` — Drive MCP server (same pattern)

### Established Patterns
- MCP servers use StdioServerTransport (not HTTP) — stdio is the standard for Claude Code
- OAuth credentials passed via env vars in settings.json mcpServers block
- Each MCP server is a separate Node.js process with its own entry in settings.json
- Tools defined with zod schemas for parameter validation

### Integration Points
- `~/.claude/settings.json` mcpServers section — add new entries for sheets, calendar, rc-ops
- `racingpoint-google/` package — add services/sheets.js and services/calendar.js
- `http://192.168.31.23:8080/api/v1/` — racecontrol REST API base URL for rc-ops-mcp
- Google OAuth scopes — may need to add `spreadsheets` and `calendar.readonly` to consent

</code_context>

<specifics>
## Specific Ideas

- Gmail MCP fix should be tested by asking Claude to "read my latest emails" after re-auth
- rc-ops-mcp should expose tools Claude can chain: "check fleet health, then end stuck billing on Pod 3"
- Auto-refresh script should log when it refreshes tokens so James knows the auth is alive

</specifics>

<deferred>
## Deferred Ideas

- Custom rc-ops-mcp with WebSocket subscription for real-time pod events — too complex for v9.0
- Google Tasks MCP integration — lower priority than Gmail/Sheets/Calendar
- MCP for Dahua security cameras — no vision pipeline (explicitly out of scope in REQUIREMENTS.md)

</deferred>

---

*Phase: 52-mcp-servers*
*Context gathered: 2026-03-20*
