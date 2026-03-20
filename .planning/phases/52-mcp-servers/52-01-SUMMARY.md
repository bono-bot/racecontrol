---
phase: 52-mcp-servers
plan: 01
subsystem: infra
tags: [mcp, google-workspace, oauth, google-sheets, google-calendar, nodejs, esm, mcp-sdk]

# Dependency graph
requires:
  - phase: racingpoint-google
    provides: "getAuthClient, sheets service, calendar service — CJS modules"
provides:
  - "racingpoint-mcp-sheets: read_sheet and write_sheet MCP tools for Claude Code"
  - "racingpoint-mcp-calendar: list_events, create_event, delete_event MCP tools for Claude Code"
  - "refresh-token.js: utility to validate and rotate Google OAuth refresh tokens"
  - "settings.json updated with racingpoint-sheets and racingpoint-calendar MCP entries"
affects:
  - "52-02 if it exists (further MCP work)"
  - "Claude Code sessions (MCP tools available after OAuth re-auth)"

# Tech tracking
tech-stack:
  added:
    - "@modelcontextprotocol/sdk ^1.27.1 (sheets and calendar MCP servers)"
    - "zod ^3.24.0 (tool parameter validation)"
    - "node:module createRequire (ESM-to-CJS bridge)"
  patterns:
    - "ESM server.js with createRequire bridge to CJS @racingpoint/google modules"
    - "Auth per-call via getAuthClient() — never at module scope"
    - "StdioServerTransport for Claude Code MCP integration"

key-files:
  created:
    - "C:/Users/bono/racingpoint/racingpoint-mcp-sheets/package.json"
    - "C:/Users/bono/racingpoint/racingpoint-mcp-sheets/server.js"
    - "C:/Users/bono/racingpoint/racingpoint-mcp-calendar/package.json"
    - "C:/Users/bono/racingpoint/racingpoint-mcp-calendar/server.js"
    - "C:/Users/bono/racingpoint/racingpoint-google/refresh-token.js"
  modified:
    - "C:/Users/bono/.claude/settings.json"

key-decisions:
  - "createRequire CJS bridge pattern copied exactly from racingpoint-mcp-gmail — proven approach for ESM servers using CJS google libs"
  - "GOOGLE_REFRESH_TOKEN set to PLACEHOLDER_REAUTH_NEEDED in settings.json — will be updated after OAuth re-auth in Task 3"
  - "Same CLIENT_ID and CLIENT_SECRET as Gmail/Drive entries — all 4 servers share one OAuth app"
  - "New MCP server dirs initialized as separate git repos (no parent repo)"

patterns-established:
  - "MCP server pattern: ESM + createRequire + per-call auth + StdioServerTransport"
  - "settings.json update: add new entries without touching existing ones"

requirements-completed:
  - MCP-01
  - MCP-02
  - MCP-03

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 52 Plan 01: MCP Servers Summary

**Sheets and Calendar MCP servers created with ESM + createRequire pattern; settings.json updated; OAuth re-auth checkpoint reached**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T06:37:49Z
- **Completed:** 2026-03-20T06:45:49Z
- **Tasks:** 2/3 complete (Task 3 is checkpoint:human-action — OAuth re-auth required)
- **Files modified:** 6

## Accomplishments

- Created racingpoint-mcp-sheets with read_sheet and write_sheet tools following exact Gmail MCP pattern
- Created racingpoint-mcp-calendar with list_events, create_event, delete_event tools
- Created refresh-token.js utility in racingpoint-google for OAuth token validation
- Updated settings.json with racingpoint-sheets and racingpoint-calendar MCP entries (REFRESH_TOKEN placeholder until re-auth)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Sheets and Calendar MCP servers + refresh-token utility**
   - `e90532b` in racingpoint-google — refresh-token.js
   - `7f0ce1e` in racingpoint-mcp-sheets — package.json + server.js
   - `7e2c8ce` in racingpoint-mcp-calendar — package.json + server.js
2. **Task 2: Update settings.json with Sheets and Calendar MCP entries** — settings.json edit (not committed to a git repo — file lives outside all repos)
3. **Task 3: OAuth re-authorization** — AWAITING HUMAN ACTION (checkpoint)

## Files Created/Modified

- `C:/Users/bono/racingpoint/racingpoint-mcp-sheets/package.json` - ESM package with MCP SDK, zod, @racingpoint/google
- `C:/Users/bono/racingpoint/racingpoint-mcp-sheets/server.js` - Sheets MCP server (read_sheet, write_sheet)
- `C:/Users/bono/racingpoint/racingpoint-mcp-calendar/package.json` - ESM package with same deps
- `C:/Users/bono/racingpoint/racingpoint-mcp-calendar/server.js` - Calendar MCP server (list_events, create_event, delete_event)
- `C:/Users/bono/racingpoint/racingpoint-google/refresh-token.js` - OAuth token refresh utility (CJS)
- `C:/Users/bono/.claude/settings.json` - Added racingpoint-sheets and racingpoint-calendar MCP entries

## Decisions Made

- createRequire CJS bridge pattern copied exactly from racingpoint-mcp-gmail — proven approach
- GOOGLE_REFRESH_TOKEN set to PLACEHOLDER_REAUTH_NEEDED — will be updated after OAuth re-auth
- Same CLIENT_ID and CLIENT_SECRET as existing Gmail/Drive entries (shared OAuth app)
- New MCP server directories initialized as standalone git repos

## Deviations from Plan

None — plan executed exactly as written for Tasks 1 and 2.

## User Setup Required

**Task 3 requires manual OAuth re-authorization.** The checkpoint details:

1. Go to https://developers.google.com/oauthplayground
2. Click gear icon → check "Use your own OAuth credentials"
3. Enter Client ID: read from `racingpoint-gmail` entry in `~/.claude/settings.json`
4. Enter Client Secret: read from `racingpoint-gmail` entry in `~/.claude/settings.json`
5. Select ALL 4 scopes:
   - https://mail.google.com/
   - https://www.googleapis.com/auth/spreadsheets
   - https://www.googleapis.com/auth/calendar
   - https://www.googleapis.com/auth/drive
6. Authorize as james@racingpoint.in and exchange code for tokens
7. Copy the refresh_token value
8. Replace ALL 4 GOOGLE_REFRESH_TOKEN values in `C:\Users\bono\.claude\settings.json` (racingpoint-gmail, racingpoint-drive, racingpoint-sheets, racingpoint-calendar)
9. Go to Google Cloud Console → APIs & Services → OAuth consent screen → "Publish App"
10. Restart Claude Code

## Next Phase Readiness

- MCP server code is complete and ready to use
- settings.json has correct structure — only REFRESH_TOKEN needs updating
- After OAuth re-auth: all 3 Google Workspace MCP tools (Gmail, Sheets, Calendar) will be operational
- Phase 52 Plan 02 (rc-ops-mcp) can proceed independently while waiting for OAuth

---
*Phase: 52-mcp-servers*
*Completed: 2026-03-20*

## Self-Check: PASSED
- `C:/Users/bono/racingpoint/racingpoint-mcp-sheets/server.js` — FOUND
- `C:/Users/bono/racingpoint/racingpoint-mcp-calendar/server.js` — FOUND
- `C:/Users/bono/racingpoint/racingpoint-google/refresh-token.js` — FOUND
- `e90532b` in racingpoint-google — FOUND
- `7f0ce1e` in racingpoint-mcp-sheets — FOUND
- `7e2c8ce` in racingpoint-mcp-calendar — FOUND
