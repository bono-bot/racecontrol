# RISK-2 Memory Hygiene Auto-Append Hook — Install Guide

**Status:** authored 2026-05-05 IST; **NOT YET INSTALLED** — opt-in only.
**Source:** MMA Iter4 5/5 consensus (residual-risks audit `.tmp/mma-playbook-upgrade-20260505/`).

## What it does

A `post-merge` git hook that auto-appends a one-line entry to `comms-link/MEMORY.md` and increments `comms-link/V2-MASTER-STATE.md` §S-N each time a `pact-*` branch (or any commit referencing `PACT-NNN`) is merged.

## Why ship as opt-in (not default)

Iter4 panel verdict: blast_radius=1-2, captain_consent_required=NO. But to keep parity with CGP H1 / Permanence Gate / Universal Sync, install is gated behind an explicit `git config` call — no surprise activation on existing clones.

## Install (1 command)

```bash
# from racecontrol repo root:
git config --local core.hooksPath scripts/git-hooks
chmod +x scripts/git-hooks/post-merge-memory-autolog.sh
ln -sf post-merge-memory-autolog.sh scripts/git-hooks/post-merge
```

## Rollback (1 command, fully reversible)

```bash
git config --local --unset core.hooksPath
# or (if you want to delete entirely)
rm -f scripts/git-hooks/post-merge scripts/git-hooks/post-merge-memory-autolog.sh
```

## Smoke test (before activating)

```bash
# Dry-run on the HEAD commit:
bash scripts/git-hooks/post-merge-memory-autolog.sh
tail -3 ../comms-link/MEMORY.md  # should show new auto-line if HEAD msg has PACT-NNN
```

If MEMORY.md tail does NOT show a new line, either:
- HEAD commit doesn't reference a PACT (expected no-op, exit 0)
- comms-link directory not found (script silently exits — check sibling layout)

## Acceptance criteria (matches Iter4 h3_evidence_after_change)

After installing + merging a `pact-*` branch:
- `tail -1 comms-link/MEMORY.md` matches `^- \[auto\] \d{4}-\d{2}-\d{2}.*PACT-`
- `wc -l comms-link/MEMORY.md` increased by exactly 1
- `comms-link/V2-MASTER-STATE.md` has new `## §S-N+1` block
- No corruption of either file (run `head -1 comms-link/V2-MASTER-STATE.md` post-hook)

## Failure modes (anti-theater)

- **comms-link not found**: hook silently exits 0 → no log entry, but no crash. Acceptable degradation.
- **MEMORY.md write race**: atomic temp file + `mv` prevents partial writes; idempotency check prevents duplicates if hook fires twice.
- **V2-MASTER-STATE update fails**: append wrapped in `|| true` — primary log entry succeeds even if §S-N increment fails. Captain can manually reconcile.
- **PACT-ID parse fails**: hook only fires when `PACT-NNN` regex OR `pact-*` branch detected; non-PACT merges no-op.

## What this does NOT do

- Does NOT push to any remote (purely local file append).
- Does NOT run claude-cli or generate AI summaries (the simplest reversible change uses the merge-commit's first line directly).
- Does NOT enforce 200-char per entry (manual hygiene during Q5 split still required for MEMORY.md index, only the auto-line is bounded).
- Does NOT install itself fleet-wide (per-clone opt-in via `git config`).

## When to activate

Suggested: after Captain ratifies the SYNTHESIS-RESIDUAL.md memo and confirms install path is acceptable. Activate first on bono VPS clone for 7 days; if no anomalies, activate on james venue clone.
