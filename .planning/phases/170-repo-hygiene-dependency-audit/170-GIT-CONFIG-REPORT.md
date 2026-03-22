# Git Config Normalization Report — Phase 170

**Date:** 2026-03-23 IST
**Plan:** 170-02 — Git Config & .gitignore Hygiene

---

## Git Config: Before / After

| Repo | Previous user.name | Previous user.email | Status |
|------|--------------------|---------------------|--------|
| comms-link | James Vowles | james@racingpoint.in | Already correct |
| pod-agent | James Vowles | james@racingpoint.in | Already correct |
| racecontrol | James Vowles | james@racingpoint.in | Already correct |
| racingpoint-admin | James Vowles | james@racingpoint.in | Already correct |
| racingpoint-api-gateway | (not set) | (not set) | Fixed |
| racingpoint-discord-bot | (not set) | (not set) | Fixed |
| racingpoint-google | James Vowles | james@racingpoint.in | Already correct |
| racingpoint-mcp-calendar | James Vowles | james@racingpoint.in | Already correct |
| racingpoint-mcp-drive | (not set) | (not set) | Fixed |
| racingpoint-mcp-gmail | (not set) | (not set) | Fixed |
| racingpoint-mcp-sheets | James Vowles | james@racingpoint.in | Already correct |
| racingpoint-whatsapp-bot | James Vowles | james@racingpoint.in | Already correct |
| rc-ops-mcp | James Vowles | james@racingpoint.in | Already correct |
| whatsapp-bot | James Vowles | james@racingpoint.in | Already correct |
| deploy-staging | James Vowles | james@racingpoint.in | Already correct |
| people-tracker | James Vowles | james@racingpoint.in | Already correct |

**Summary:** 12 repos already correct. 4 repos fixed (racingpoint-api-gateway, racingpoint-discord-bot, racingpoint-mcp-drive, racingpoint-mcp-gmail).

---

## Final State (All 16 repos)

All repos:
- `user.name` = `James Vowles`
- `user.email` = `james@racingpoint.in`

This is a local `.git/config` change only — no commits needed.

---

## .gitignore Status

See Task 2 results — .gitignore normalized across all active repos.
