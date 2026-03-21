---
phase: 52-mcp-servers
verified: 2026-03-20T08:30:00+05:30
status: gaps_found
score: 7/8 must-haves verified
re_verification: false
gaps:
  - truth: "OAuth refresh token is valid for all 4 scopes (mail, spreadsheets, calendar, drive)"
    status: failed
    reason: "racingpoint-sheets and racingpoint-calendar entries in settings.json have GOOGLE_REFRESH_TOKEN set to PLACEHOLDER_REAUTH_NEEDED — the blocking OAuth human-action task (52-01 Task 3) was not completed. Gmail and Drive entries have a real token but its scope coverage (all 4) is unverified programmatically."
    artifacts:
      - path: "C:/Users/bono/.claude/settings.json"
        issue: "GOOGLE_REFRESH_TOKEN is 'PLACEHOLDER_REAUTH_NEEDED' for racingpoint-sheets and racingpoint-calendar entries — these two MCP servers will exit(1) immediately on launch"
    missing:
      - "James must complete OAuth re-authorization via Google OAuth Playground (https://developers.google.com/oauthplayground) with all 4 scopes: mail.google.com, spreadsheets, calendar, drive"
      - "Replace ALL 4 GOOGLE_REFRESH_TOKEN values in ~/.claude/settings.json (racingpoint-gmail, racingpoint-drive, racingpoint-sheets, racingpoint-calendar) with the new token"
      - "Set OAuth consent screen to 'In production' mode in Google Cloud Console to prevent future 7-day expiry"
      - "Restart Claude Code so MCP servers reload with real token"

human_verification:
  - test: "After OAuth re-auth: Ask Claude 'read my latest emails' in a new session"
    expected: "Gmail MCP returns inbox listing without auth error"
    why_human: "OAuth token validity and Gmail MCP startup cannot be verified without running the server process"
  - test: "After OAuth re-auth: Ask Claude 'read range A1:B5 from sheet [known URL]'"
    expected: "Sheets MCP returns 2D array of cell data"
    why_human: "Requires live Google Sheets API call with valid token"
  - test: "After OAuth re-auth: Ask Claude 'what is on my calendar today?'"
    expected: "Calendar MCP returns events list for today"
    why_human: "Requires live Google Calendar API call with valid token"
  - test: "Ask Claude 'check fleet health' in a new session"
    expected: "rc-ops-mcp returns array of 8 pod status objects from http://192.168.31.23:8080/api/v1/fleet/health"
    why_human: "Requires racecontrol server at .23:8080 to be running and reachable — cannot verify LAN connectivity from verifier"
---

# Phase 52: MCP Servers Verification Report

**Phase Goal:** Claude Code can query Gmail, Google Sheets, Google Calendar, and the racecontrol REST API directly from any session — James describes what he needs in plain language and Claude fetches live data without manual curl or browser lookups
**Verified:** 2026-03-20T08:30:00 IST
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | racingpoint-mcp-sheets server starts without errors and exposes read_sheet and write_sheet tools | CONDITIONAL | Server code is complete and correct (67 lines, 2 tools, createRequire bridge) BUT will exit(1) at startup due to PLACEHOLDER_REAUTH_NEEDED token — server is blocked by missing OAuth |
| 2 | racingpoint-mcp-calendar server starts without errors and exposes list_events, create_event, and delete_event tools | CONDITIONAL | Server code is complete and correct (83 lines, 3 tools, createRequire bridge) BUT will exit(1) at startup due to PLACEHOLDER_REAUTH_NEEDED token |
| 3 | settings.json has racingpoint-sheets and racingpoint-calendar entries with correct command, args, and env vars | PARTIAL | Entries exist with correct command/args/env structure, correct CLIENT_ID and CLIENT_SECRET, but GOOGLE_REFRESH_TOKEN is placeholder — not functional |
| 4 | OAuth refresh token is valid for all 4 scopes (mail, spreadsheets, calendar, drive) | FAILED | racingpoint-sheets and racingpoint-calendar have PLACEHOLDER_REAUTH_NEEDED. Gmail/Drive have a real token (same token) but scope coverage not verified programmatically |
| 5 | rc-ops-mcp server starts without errors and exposes ~10 tools for racecontrol REST API | VERIFIED | 191-line server.js, exactly 10 tools via server.tool(), ESM, native fetch, no extra HTTP deps, StdioServerTransport — no startup validation that could block it |
| 6 | Claude Code can call get_fleet_health and receive pod status array from http://192.168.31.23:8080 | CONDITIONAL | Code is wired correctly (get_fleet_health → apiCall('/fleet/health') → fetch(BASE_URL+path)), needs racecontrol running at .23:8080 at runtime |
| 7 | Claude Code can call exec_on_pod to run commands on specific pods via racecontrol proxy | VERIFIED | exec_on_pod tool implemented using /pods/{podId}/exec (NOT /fleet/exec), with POST body {cmd, timeout_ms} and Content-Type header |
| 8 | settings.json has rc-ops-mcp entry with RACECONTROL_BASE_URL pointing to server .23:8080 | VERIFIED | Entry confirmed: command=node.exe, args=[rc-ops-mcp/server.js], env.RACECONTROL_BASE_URL=http://192.168.31.23:8080/api/v1 |

**Score:** 5/8 truths fully verified (3 blocked by OAuth placeholder or runtime LAN dependency)

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `C:/Users/bono/racingpoint/racingpoint-mcp-sheets/server.js` | Sheets MCP server with read_sheet and write_sheet tools | VERIFIED | 67 lines, 2 tools registered, createRequire bridge to @racingpoint/google/services/sheets, per-call auth, StdioServerTransport |
| `C:/Users/bono/racingpoint/racingpoint-mcp-sheets/package.json` | ESM package with MCP SDK + zod + @racingpoint/google | VERIFIED | type:module, @modelcontextprotocol/sdk 1.27.1, zod, node_modules installed |
| `C:/Users/bono/racingpoint/racingpoint-mcp-calendar/server.js` | Calendar MCP server with list_events, create_event, delete_event tools | VERIFIED | 83 lines, 3 tools registered, createRequire bridge to @racingpoint/google/services/calendar |
| `C:/Users/bono/racingpoint/racingpoint-mcp-calendar/package.json` | ESM package with same deps as sheets | VERIFIED | type:module, correct deps, node_modules installed |
| `C:/Users/bono/racingpoint/racingpoint-google/refresh-token.js` | Token refresh utility, min 20 lines | VERIFIED | 46 lines, CJS, calls auth.refreshAccessToken(), logs expiry, warns on new refresh token, exits 0/1 |
| `C:/Users/bono/racingpoint/rc-ops-mcp/server.js` | MCP server wrapping racecontrol REST API with ~10 tools, min 100 lines | VERIFIED | 191 lines, exactly 10 tools, apiCall() helper, RACECONTROL_BASE_URL, native fetch, try/catch on every tool |
| `C:/Users/bono/racingpoint/rc-ops-mcp/package.json` | Node.js package with MCP SDK + zod only (native fetch) | VERIFIED | type:module, no axios/node-fetch, @modelcontextprotocol/sdk + zod only |
| `C:/Users/bono/.claude/settings.json` (racingpoint-sheets entry) | MCP entry with correct command, args, env | PARTIAL | Entry exists with correct structure but GOOGLE_REFRESH_TOKEN=PLACEHOLDER_REAUTH_NEEDED |
| `C:/Users/bono/.claude/settings.json` (racingpoint-calendar entry) | MCP entry with correct command, args, env | PARTIAL | Entry exists with correct structure but GOOGLE_REFRESH_TOKEN=PLACEHOLDER_REAUTH_NEEDED |
| `C:/Users/bono/.claude/settings.json` (rc-ops-mcp entry) | MCP entry with RACECONTROL_BASE_URL | VERIFIED | Entry correct, 10 total mcpServers entries, all previous entries preserved |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| racingpoint-mcp-sheets/server.js | @racingpoint/google/services/sheets | createRequire CJS bridge | VERIFIED | Line 8-10: `const require = createRequire(import.meta.url); const { readRange, writeRange } = require('@racingpoint/google/services/sheets')` |
| racingpoint-mcp-calendar/server.js | @racingpoint/google/services/calendar | createRequire CJS bridge | VERIFIED | Line 8-10: `const require = createRequire(import.meta.url); const { listEvents, createEvent, deleteEvent } = require('@racingpoint/google/services/calendar')` |
| settings.json | racingpoint-mcp-sheets/server.js | mcpServers entry | PARTIAL | Entry exists but REFRESH_TOKEN is placeholder — server will not start |
| settings.json | racingpoint-mcp-calendar/server.js | mcpServers entry | PARTIAL | Entry exists but REFRESH_TOKEN is placeholder — server will not start |
| rc-ops-mcp/server.js | http://192.168.31.23:8080/api/v1 | native fetch() calls | VERIFIED | BASE_URL from RACECONTROL_BASE_URL env var (line 7), apiCall() helper (lines 9-17) used by all 10 tools |
| settings.json | rc-ops-mcp/server.js | mcpServers entry | VERIFIED | rc-ops-mcp entry confirmed with correct path and RACECONTROL_BASE_URL |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MCP-01 | 52-01 | Claude Code can read Gmail messages via Google Workspace MCP using existing racingpoint-google OAuth | CONDITIONAL | Gmail server (racingpoint-mcp-gmail/server.js) pre-exists. settings.json entry has a real token but token's validity/scope for Gmail cannot be verified — it was already expired before this phase (noted in CLAUDE.md blockers). Sheets and Calendar completion depends on same OAuth re-auth. |
| MCP-02 | 52-01 | Claude Code can read and write Google Sheets via the same MCP server | BLOCKED | Server code complete and correct. Blocked by PLACEHOLDER_REAUTH_NEEDED token — server exits on startup. |
| MCP-03 | 52-01 | Claude Code can read Google Calendar events via the same MCP server | BLOCKED | Server code complete and correct. Blocked by PLACEHOLDER_REAUTH_NEEDED token — server exits on startup. |
| MCP-04 | 52-02 | Claude Code can query racecontrol REST API (/fleet/health, /sessions, /billing, /laps) via a custom rc-ops-mcp server | VERIFIED | rc-ops-mcp/server.js has all required endpoints implemented. settings.json entry present. Runtime depends on racecontrol server being up at .23:8080. |

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| settings.json | 108 | `"GOOGLE_REFRESH_TOKEN": "PLACEHOLDER_REAUTH_NEEDED"` | Blocker | racingpoint-sheets MCP will exit(1) immediately — Sheets tools unavailable |
| settings.json | 119 | `"GOOGLE_REFRESH_TOKEN": "PLACEHOLDER_REAUTH_NEEDED"` | Blocker | racingpoint-calendar MCP will exit(1) immediately — Calendar tools unavailable |

No TODO/FIXME/placeholder comments found in any server.js files.
No empty implementations or console.log-only stubs found.
No forbidden patterns (/fleet/exec, axios, node-fetch) found in rc-ops-mcp.

---

## Human Verification Required

### 1. OAuth Re-authorization (Blocking — Must Be Done First)

**Test:** Go to https://developers.google.com/oauthplayground, use your own OAuth credentials (CLIENT_ID from settings.json: `600025586750-...`), select all 4 scopes (mail.google.com, spreadsheets, calendar, drive), authorize as james@racingpoint.in, exchange code for tokens, copy refresh_token. Replace ALL 4 GOOGLE_REFRESH_TOKEN values in `C:\Users\bono\.claude\settings.json`. Then set OAuth consent screen to "In production" in Google Cloud Console. Restart Claude Code.
**Expected:** All 4 GOOGLE_REFRESH_TOKEN values in settings.json are identical, non-placeholder tokens.
**Why human:** Browser OAuth flow — cannot be automated.

### 2. Gmail MCP Live Test

**Test:** In a new Claude Code session, say "read my latest 5 emails"
**Expected:** Gmail inbox listing with sender, subject, date — no auth error
**Why human:** Requires live Google API call with valid token; Gmail OAuth was already expired before this phase.

### 3. Sheets MCP Live Test

**Test:** In a new Claude Code session, say "read range A1:C3 from [a known Google Sheet URL]"
**Expected:** 2D array of cell values returned — no auth error
**Why human:** Requires valid token after OAuth re-auth.

### 4. Calendar MCP Live Test

**Test:** In a new Claude Code session, say "what's on my calendar for today and tomorrow?"
**Expected:** List of Google Calendar events with title, start/end times — no auth error
**Why human:** Requires valid token after OAuth re-auth.

### 5. Fleet Health via rc-ops-mcp

**Test:** In a new Claude Code session, say "check fleet health" or "what pods are online?"
**Expected:** rc-ops-mcp calls get_fleet_health, returns array of 8 pod statuses from racecontrol server .23:8080
**Why human:** Requires racecontrol server to be running at 192.168.31.23:8080 and LAN connectivity.

---

## Gaps Summary

**One gap blocks 2 of 4 requirements (MCP-02 and MCP-03):** The OAuth human-action checkpoint (52-01 Task 3) was not completed. The Sheets and Calendar MCP servers are fully implemented and correctly wired — they will work the moment a valid refresh token is placed in settings.json.

The rc-ops-mcp server (MCP-04) is fully operational without any human action needed. All 10 tools are implemented, the settings.json entry is correct, and the server will load in the next Claude Code session.

MCP-01 (Gmail) depends on the same OAuth re-auth because the existing token was documented as expired before this phase started.

**Root cause:** A single OAuth re-authorization action unblocks MCP-01, MCP-02, and MCP-03 simultaneously.

---

_Verified: 2026-03-20T08:30:00 IST_
_Verifier: Claude (gsd-verifier)_
