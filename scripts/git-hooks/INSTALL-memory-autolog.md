# RISK-2 Memory Hygiene Auto-Append Hook — Install Guide (v2)

**Status:** authored 2026-05-05 IST; **NOT YET INSTALLED** — opt-in only.
**Source:** MMA Iter4 5/5 consensus (residual-risks audit `.tmp/mma-playbook-upgrade-20260505/`).
**Version:** v2 (JSONL-only) — supersedes v1 (committed `0492b602`) which wrote to MEMORY.md and V2-MASTER-STATE directly.

## Why v2 (what changed from v1)

v1 wrote to `comms-link/MEMORY.md` and `V2-MASTER-STATE.md` directly. That left comms-link working-tree dirty after every PACT merge, colliding with the auto-flush guard on `inbox-append.js` (PACT-026 atomic-publish).

v2 writes ONLY to `comms-link/data/memory-autolog.jsonl` as an append-only audit ledger. No MEMORY.md or V2-MASTER-STATE.md mutation. Promotion from JSONL → MEMORY.md is a separate human-gated step done during periodic Q5 split review. This eliminates the working-tree-dirt class entirely.

## What it does

A `post-merge` git hook that auto-appends one JSONL row to `comms-link/data/memory-autolog.jsonl` each time a PACT-related branch is merged. A row looks like:

```json
{"ts":"2026-05-05T11:47:02.000Z","pact_id":"PACT-20260505-004","source_branch":"pact-20260505-004-state-divergence-pre-commit-hook","merge_hash":"a1ac70a","origin_repo":"racecontrol","summary":"Merge PACT-20260505-004 into main"}
```

Detection rules:
- Fires if commit message contains `PACT-YYYYMMDD-NNN` OR `PACT-NNN`
- Fires if merged source branch matches `pact-*`
- Silent no-op if neither matches

## Smoke test passed (2026-05-05, james workstation .27)

```
$ bash scripts/git-hooks/post-merge-memory-autolog.test.sh
Total: 12 | PASS: 12 | FAIL: 0
```

6 cases covered: PACT-NNN message / idempotency / pact-* branch detection / non-PACT no-op / BYPASS env var / missing comms-link sibling. All PASS.

## Install (1 command)

```bash
# from racecontrol repo root:
git config --local core.hooksPath scripts/git-hooks
chmod +x scripts/git-hooks/post-merge-memory-autolog.sh
ln -sf post-merge-memory-autolog.sh scripts/git-hooks/post-merge
```

The third line is needed because git invokes hooks by their canonical name (`post-merge`), not the descriptive name. The symlink lets the executable have a meaningful filename in repo while still being picked up by git.

## Rollback (1 command, fully reversible)

```bash
git config --local --unset core.hooksPath
# or (if you want to delete entirely)
rm -f scripts/git-hooks/post-merge scripts/git-hooks/post-merge-memory-autolog.sh
```

## BYPASS for individual operations

If you need to suppress the hook for a single merge without uninstalling:

```bash
BYPASS_MEMORY_AUTOLOG=1 git merge <branch>
```

## Acceptance criteria (verified by smoke test)

- [x] Ledger entry appended on `pact-*` branch merge
- [x] Ledger entry appended on commit-message `PACT-NNN` reference
- [x] Idempotent: re-running hook on same merge does NOT duplicate
- [x] Non-PACT merges produce ZERO ledger entries
- [x] BYPASS env var fully suppresses hook
- [x] Missing comms-link sibling: silent exit 0 (no error, no entry)
- [x] All entries are valid single-line JSON

## Failure modes (from hook design)

- **comms-link not found**: hook silently exits 0 → no log entry, but no crash. Acceptable degradation. Verified in test Case 6.
- **Ledger write race**: append via `>>` is single-syscall on POSIX/Windows for lines under PIPE_BUF (4KB); each row is well under that. No read-modify-write race possible.
- **Idempotency**: hook checks last 100 lines for `{pact_id, merge_hash}` pair before appending. Verified in test Case 2.
- **Hook errors during git op**: hook is `set -euo pipefail` and exits 0 on most paths; if it does crash, git treats post-merge hook failures as non-fatal (the merge has already committed by the time post-merge fires).

## What this does NOT do (intentional v2 simplifications)

- **Does NOT mutate `comms-link/MEMORY.md`** — eliminates working-tree-dirt class
- **Does NOT mutate `V2-MASTER-STATE.md`** — same reason
- **Does NOT commit or push anything** — ledger is committed manually as part of normal comms-link workflow
- **Does NOT install itself fleet-wide** — per-clone opt-in via `git config core.hooksPath`
- **Does NOT promote ledger entries to MEMORY.md** — that's a separate human-gated curation step (matches the original RISK-2 spirit of "force the curator to think about what's worth keeping")

## Promotion workflow (manual, separate from hook)

Periodically (during Q5 split review or on-demand):
1. Read `comms-link/data/memory-autolog.jsonl`
2. For each entry not yet promoted, decide:
   - Worth a one-line MEMORY.md index entry? → manually add under appropriate category
   - Worth a §S-N V2-MASTER-STATE entry? → manually append per existing §S-N conventions
   - Routine PACT, no promotion needed? → leave in ledger only
3. Commit ledger + any MEMORY.md/V2-MASTER-STATE updates together

## When to activate

Suggested cadence:
1. **Phase 1 (now-eligible after smoke test green):** activate on bono VPS clone first
2. **Phase 2 (after 7 days clean):** activate on james venue clone
3. **Phase 3 (after 30 days):** review ledger growth rate — if >5 entries/day average, consider tightening the PACT detection rules

JSONL-only design means asymmetric activation is safe — bono and james each maintain their own view of the ledger; reconciliation happens via normal git pull/merge of the ledger file.

## Cross-reference

- `.tmp/mma-playbook-upgrade-20260505/SYNTHESIS-RESIDUAL.md` — Iter 4 panel rationale for RISK-2
- `0492b602` — v1 commit (now superseded; v1 hook should NOT be activated)
- PACT-026 atomic-publish-guard (the guard that v1 collided with — v2 avoids by design)
- PACT-20260505-004 (P6 State-Divergence Pre-Commit Hook) — sibling substrate-class artifact in the same playbook upgrade family
