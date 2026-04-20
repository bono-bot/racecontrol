#!/usr/bin/env python3
"""
Deploy-staging parity checker — workflow-plan board-state surface.

Detects the `deploy-server.sh` install-drift class: `scripts/deploy-server-ssh.sh`
in git has a committed fix, but `racingpoint/deploy-staging/deploy-server.sh`
(the gitignored path that production deploys actually run from) has not been
re-synced, so the fix never reaches the deploy path.

Concrete history: commit 7e170dd4 (2026-04-20) fixed 3 recurring bugs in the
tracked `scripts/deploy-server-ssh.sh` (smoke-test 401 false-rollback,
cosmetic rollback, broken SWAPLOG append). The staging copy at
`../deploy-staging/deploy-server.sh` was only hand-synced at 18:23 IST, ~1h
after the problematic 17:27 IST deploy. Without an automated check, the next
"fix tracked source, forget to copy to staging" cycle will reintroduce all 3
bugs silently.

Detection strategy:
  1. Compute md5 of the tracked source `scripts/deploy-server-ssh.sh`.
  2. Try to compute md5 of the staging target
     `<repo_parent>/deploy-staging/deploy-server.sh`.
  3. If staging path does not exist → SKIPPED (CI / cloud / fresh clone have
     no deploy-staging dir — that's by design).
  4. If md5 differs → red (gaps list includes both paths + a diff hint).
  5. If md5 matches → green.

Exit codes:
  0 — green (md5 match) OR staging path missing (skipped).
  1 — red (md5 mismatch between tracked source and staging install).
  2 — tracked source missing (script itself is misconfigured — should not
      happen unless `scripts/deploy-server-ssh.sh` was removed).

Designed for:
  - run-all-checkers.sh orchestrator registration
  - Pre-commit cascade hook (same surface as schema-dpdp + fleet-swaplog-parity)
  - Bono pm2 workflow-monitor (though note: Bono VPS has no deploy-staging dir
    so it will always report SKIPPED there — that is correct).
"""
import hashlib
import sys
from pathlib import Path


def md5_of(path: Path) -> str:
    h = hashlib.md5()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    # Resolve paths relative to this script's location so the checker is
    # cwd-independent. script is at racecontrol/scripts/audit/*.py ;
    # repo root is 2 levels up.
    script_dir = Path(__file__).resolve().parent
    repo_root = script_dir.parent.parent
    tracked = repo_root / "scripts" / "deploy-server-ssh.sh"
    # deploy-staging lives OUTSIDE the repo, at racingpoint/deploy-staging/.
    staging = repo_root.parent / "deploy-staging" / "deploy-server.sh"

    if not tracked.exists():
        print(f"ERROR: tracked source missing: {tracked}")
        return 2

    tracked_md5 = md5_of(tracked)

    if not staging.exists():
        # CI / cloud / fresh clone — deploy-staging is a James-local dir.
        print(
            f"deploy-staging-parity: SKIPPED (staging path not present at {staging}) — "
            f"tracked source md5={tracked_md5[:12]}"
        )
        return 0

    staging_md5 = md5_of(staging)

    if tracked_md5 == staging_md5:
        print(
            f"deploy-staging-parity: green "
            f"(tracked={tracked_md5[:12]} matches staging={staging_md5[:12]})"
        )
        return 0

    # Red path — drift detected.
    print("deploy-staging-parity: DRIFT DETECTED")
    print(f"GAPS:")
    print(f"  - tracked: {tracked}")
    print(f"      md5={tracked_md5}")
    print(f"  - staging: {staging}")
    print(f"      md5={staging_md5}")
    print(
        f"FIX: cp '{tracked}' '{staging}'  "
        f"(if staging has the newer changes, cp the OTHER way + commit to the tracked path)"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
