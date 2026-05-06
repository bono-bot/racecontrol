# F9 SWAPLOG — Append-only deploy ledger (PACT-20260503-004)

**Format:** one row per surface deploy, written by `v2-deploy/deploy.sh` post-delegate.
**Invariant:** append-only. Never delete rows. Rotate annually via `SWAPLOG-YYYY.md` archive.
**CONSTRAINT-019:** rows with `caller=other` indicate manual deploys (forbidden post-F9 ratify; HALO probe will surface as VIOLATION).

| ts (UTC) | ts (IST) | surface | git_sha | author | manifest_path | exit_code | duration_s | notes |
|----------|----------|---------|---------|--------|---------------|-----------|------------|-------|
| 2026-05-03T00:14:03Z | 2026-05-03 00:14 IST | racecontrol | 9183900930fc1a36f1b3af71d241c17d9b200b47 | James Vowles <james@racingpoint.in> | v2-deploy/manifests/racecontrol-2026-05-03T001403Z.json | 0 | 0 | dry-run |
