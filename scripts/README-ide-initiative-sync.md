# IDE → Console initiative sync

Makes IDE work **auto-appear as tracked evidence** on the Captain's Console
`/initiatives` board — operationalizing *"all IDE work is RaceControl Captain's
Console work"* (Captain, 2026-06-06). Doctrine: [`../.planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md`](../.planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md) §5.

## The contract — tag your commits

Every commit that advances an initiative carries a trailer in the commit body:

```
<your normal commit message>

Development: <initiative-id>
```

`<initiative-id>` must be an `id` in `.planning/specs/dev-platform/developments.yaml`
(e.g. `dev-ide-operating-model`, `dev-console-v2`, `initiative-money-path-reliability`).
A trailer whose id is **not** in the registry is reported as an **orphan** — never
auto-created (curate a real initiative or fix the id).

## Cadence (Captain-chosen): auto-harvest on commit · manual deploy

| Step | What | When |
|---|---|---|
| **Harvest** | `sync-ide-initiatives.py` scans all 3 repos for `Development:` trailers → writes `developments.auto-evidence.json` + `.ide-sync-cursor.json` (curated `developments.yaml` untouched) | **automatic** on every commit (git `post-commit` hook, backgrounded, fail-open) |
| **Deploy** | `deploy-console-registry.sh` = harvest(+PR) → `gen-dev-registry.py` → `inject-auto-evidence.py` → `pm2 restart racecontrol-console` → health | **manual**, run on a sensible cadence (end of session) |

## Files

| File | Role | Lane |
|---|---|---|
| `sync-ide-initiatives.py` | harvester (trailers → auto-evidence.json; `--dry-run`, `--with-pr`) | racecontrol (bono-sole) |
| `inject-auto-evidence.py` | merge auto-evidence into the **generated** `dev-registry.json` (never the YAML, never Console source) | racecontrol |
| `deploy-console-registry.sh` | the sanctioned manual deploy (only path that keeps IDE evidence — bare generator drops it) | racecontrol |
| `git-hooks/post-commit-ide-sync.sh` | the appendable hook block (source of truth) | racecontrol |
| `install-ide-sync-hooks.sh` | idempotently appends the block to `.git/hooks/post-commit` (preserves graphify) | racecontrol |

## Invariants

- **Curated YAML stays pristine** — a PyYAML round-trip would strip comments + re-trigger the `health: on` bool trap, so harvested data lives in a *separate* JSON injected into the generated artifact.
- **No new initiatives auto-created** — registry owns "which initiatives exist"; orphans are reported, not minted.
- **Curation boundary** — auto-sync only writes *evidence/activity*. Gate-phase transitions, CTQ, health, freeze stay deliberate Console sign-offs.
- **Idempotent** — harvest dedupes by `(dev_id, sha)`; inject strips prior `[IDE ...]` anchors and re-adds. Re-running converges.
- **Fail-open** — the commit hook backgrounds the harvest and never blocks/slows a commit.

## Install / verify

```bash
bash scripts/install-ide-sync-hooks.sh                 # one-time (idempotent)
python3 scripts/sync-ide-initiatives.py --dry-run      # see what it would harvest
bash scripts/deploy-console-registry.sh                # refresh the live board
```

## Revert

Remove the scripts + the `# ide-sync-start..end` block from `.git/hooks/post-commit`,
and regenerate `dev-registry.json` from the YAML (bare `gen-dev-registry.py`) — back
to pre-sync behaviour. The doctrine is a doc; delete to revert.
