---
phase: 52
slug: mcp-servers
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 52 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification (MCP servers, OAuth, API queries) |
| **Config file** | `~/.claude/settings.json` (MCP config) |
| **Quick run command** | `node racingpoint-mcp-gmail/server.js --test 2>&1 \| head -1` |
| **Full suite command** | Verify all 4 MCP entries in settings.json + OAuth token validity |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Verify file exists + settings.json entry present
- **After every plan wave:** Test MCP server starts without error
- **Before `/gsd:verify-work`:** Full MCP tool invocation test in Claude Code session
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 52-01-01 | 01 | 1 | MCP-01 | config+api | `grep "racingpoint-gmail" ~/.claude/settings.json` | ✅ | ⬜ pending |
| 52-01-02 | 01 | 1 | MCP-02 | file+config | `test -f racingpoint-mcp-sheets/server.js && grep "racingpoint-sheets" ~/.claude/settings.json` | ❌ W0 | ⬜ pending |
| 52-01-03 | 01 | 1 | MCP-03 | file+config | `test -f racingpoint-mcp-calendar/server.js && grep "racingpoint-calendar" ~/.claude/settings.json` | ❌ W0 | ⬜ pending |
| 52-02-01 | 02 | 1 | MCP-04 | file+config | `test -f racingpoint-rc-ops-mcp/server.js && grep "rc-ops" ~/.claude/settings.json` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*No test framework install needed. Verification is file existence + settings.json grep + MCP server startup test.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Gmail read via Claude | MCP-01 | Requires live Claude Code session with MCP | Ask Claude "read my latest emails" — expect inbox listing |
| Sheets read/write via Claude | MCP-02 | Requires live Claude Code session with Sheets MCP | Ask Claude to read a cell range from a known sheet |
| Calendar read via Claude | MCP-03 | Requires live Claude Code session with Calendar MCP | Ask Claude "what's on my calendar today?" |
| Fleet health via Claude | MCP-04 | Requires live racecontrol server | Ask Claude "check fleet health" — expect pod statuses |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
