---
phase: 52-mcp-servers
plan: 02
subsystem: infra
tags: [mcp, node, rest-api, racecontrol, fleet-management]

# Dependency graph
requires:
  - phase: 52-01
    provides: MCP pattern established (ESM, McpServer, StdioServerTransport, zod) and settings.json mcpServers structure

provides:
  - rc-ops-mcp Node.js MCP server with 10 tools wrapping racecontrol REST API
  - Claude Code direct access to fleet health, sessions, billing, laps, drivers, pod exec, and self-test
  - settings.json entry for rc-ops-mcp pointing to server at http://192.168.31.23:8080/api/v1

affects: [53-deployment-automation, 54-structured-logging, 55-netdata-fleet]

# Tech tracking
tech-stack:
  added:
    - "@modelcontextprotocol/sdk 1.27.1 (rc-ops-mcp)"
    - "zod ^3.24.0 (rc-ops-mcp)"
    - "Node 22 native fetch (no axios/node-fetch)"
  patterns:
    - "MCP tool wrapping REST API with try/catch + isError: true on failure"
    - "RACECONTROL_BASE_URL env var with sensible LAN default"
    - "Per-pod exec via /pods/{id}/exec (NOT /fleet/exec which does not exist)"

key-files:
  created:
    - "C:/Users/bono/racingpoint/rc-ops-mcp/server.js"
    - "C:/Users/bono/racingpoint/rc-ops-mcp/package.json"
  modified:
    - "C:/Users/bono/.claude/settings.json"

key-decisions:
  - "rc-ops-mcp runs on James's machine (.27) not on server — avoids exposing server REST API externally"
  - "Native fetch() only — no axios or node-fetch dependency (Node 22 has built-in fetch)"
  - "10 tools cover all priority racecontrol endpoints: fleet health, sessions, billing (active/stop/history/daily), laps, drivers, pod exec, self-test"
  - "RACECONTROL_BASE_URL defaults to http://192.168.31.23:8080/api/v1 — override for dev/staging"
  - "Error handling: each tool wrapped in try/catch, returns isError:true on failure (never throws)"

patterns-established:
  - "MCP REST proxy pattern: apiCall() helper with BASE_URL + native fetch, 10 tools all using same helper"
  - "settings.json mcpServers entry format: command=node.exe, args=[server.js], env={API vars only}"

requirements-completed: [MCP-04]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 52 Plan 02: rc-ops-mcp MCP Server Summary

**Node.js MCP server with 10 tools wrapping racecontrol REST API — Claude Code can now query fleet health, billing, sessions, and exec commands on pods directly from natural language**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T06:45:42Z
- **Completed:** 2026-03-20T06:48:42Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created rc-ops-mcp MCP server (server.js, package.json) with exactly 10 tools covering all priority racecontrol REST endpoints
- Used native Node 22 fetch — no extra HTTP dependencies (no axios, no node-fetch)
- Added rc-ops-mcp to Claude Code settings.json with RACECONTROL_BASE_URL pointing to server .23:8080

## Task Commits

Each task was committed atomically:

1. **Task 1: Create rc-ops-mcp server with 10 racecontrol API tools** - `40214df` (feat) — committed in rc-ops-mcp repo
2. **Task 2: Add rc-ops-mcp to Claude Code settings.json** - settings.json is outside any git repo; change documented here

**Plan metadata:** (see final commit below)

## Files Created/Modified
- `C:/Users/bono/racingpoint/rc-ops-mcp/server.js` - 10 MCP tools wrapping racecontrol REST API via native fetch
- `C:/Users/bono/racingpoint/rc-ops-mcp/package.json` - ESM module, MCP SDK + zod only
- `C:/Users/bono/.claude/settings.json` - Added rc-ops-mcp entry with RACECONTROL_BASE_URL env var

## Decisions Made
- Native fetch() only (no axios/node-fetch) — Node 22 has built-in fetch, no extra dependencies needed
- Per-pod exec via `/pods/{id}/exec` — the plan explicitly notes `/fleet/exec` does not exist
- No startup env var validation — BASE_URL has a sensible LAN default, no Google OAuth needed
- rc-ops-mcp directory initialized as its own git repo (separate from racecontrol monorepo)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - rc-ops-mcp uses the racecontrol REST API directly with no external OAuth or credentials.

Note: Claude Code will load rc-ops-mcp on next session restart. The server will be able to call `get_fleet_health`, `exec_on_pod`, `stop_billing`, and other tools against http://192.168.31.23:8080/api/v1 without any additional setup.

## Next Phase Readiness
- rc-ops-mcp is ready for Claude Code to use on next session restart
- All 10 tools verified against racecontrol endpoint definitions from routes.rs
- Phase 52 MCP-04 requirement complete
- Next: Phase 53 Deployment Automation (James workstation only, no pod access required)

---
*Phase: 52-mcp-servers*
*Completed: 2026-03-20*
