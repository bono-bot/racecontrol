# Phase 351: Data Durability - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 351-data-durability
**Areas discussed:** Gap analysis (auto mode)
**Mode:** --auto (all decisions auto-selected based on codebase evidence)

---

## Gap Analysis (auto-selected)

| Finding | Evidence | Decision |
|---------|----------|----------|
| Venue racecontrol.db backup failing | Today's backup dir has cloud DBs but no venue DB | P0 — diagnose and fix SSH/sqlite3 failure |
| No schtask registered | `schtasks.exe /Query` returns empty | Verify actual trigger mechanism, register if missing |
| No 12-month first-of-month retention | `backup-databases.sh` only prunes to 30 days | Add exemption in prune loop |
| admin.db not backed up | Not in script's backup list | Add to backup-databases.sh |
| No post-backup validation/alert | Script exits 0 even on failures | Add validation + comms-link alert |

**Auto decision:** [auto] All gaps selected for resolution. Phase is a gap-close on existing Phase 300 infrastructure, not a new build.

---

## Claude's Discretion

- Alert message format for post-backup validation
- Whether admin.db goes in Rust pipeline or shell script
- SSH failure diagnostic approach

## Deferred Ideas

None
