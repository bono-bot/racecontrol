#!/usr/bin/env python3
"""Harvest `Development: <id>` git trailers into the Console initiative registry.

The IDE→initiative link is the locked `Development: <id>` commit trailer
(Captain 2026-06-06). This script reads those trailers across the bono-owned
repos and records, per initiative, the commits that advanced it — as ADVISORY
evidence + latest-activity. It NEVER touches the hand-curated developments.yaml
(a YAML round-trip would strip comments + re-trigger the `health: on` bool trap)
and it NEVER mints new initiatives (a trailer whose id is not in the registry is
reported as an orphan for a human to curate). The merge into the served board is
done by inject-auto-evidence.py at deploy time.

Idempotency guarantee = dedupe by (dev_id, full sha). Re-running is safe and
loses nothing; the cursor file is an observability record, not the safety net.

Cadence (Captain 2026-06-06): auto-harvest on commit (git post-commit hook,
fail-open, backgrounded) · manual deploy. This script is the harvest half.

Lane: racecontrol/** (bono-sole). Run:  python3 scripts/sync-ide-initiatives.py [--dry-run] [--with-pr]
Doctrine: .planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md §5
"""
import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

DEV_PLATFORM = "/root/racecontrol/.planning/specs/dev-platform"
DEVELOPMENTS_YAML = os.path.join(DEV_PLATFORM, "developments.yaml")
AUTO_EVIDENCE = os.path.join(DEV_PLATFORM, "developments.auto-evidence.json")
CURSOR = os.path.join(DEV_PLATFORM, ".ide-sync-cursor.json")

# Repos scanned for trailers. The post-commit hook fires in racecontrol; the
# harvest scans all three so a commit in any of them is eventually caught.
REPOS = {
    "racecontrol": "/root/racecontrol",
    "comms-link": "/root/comms-link",
    "rp-v2-apps": "/root/rp-v2-apps",
}

# git record/field separators (avoid collision with message text)
RS, FS = "\x1e", "\x1f"


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_registry_ids():
    """Read-only: the set of initiative ids that legitimately exist."""
    try:
        import yaml
    except ImportError:
        print("ERROR: pyyaml not available; cannot validate ids.", file=sys.stderr)
        sys.exit(2)
    with open(DEVELOPMENTS_YAML) as f:
        doc = yaml.safe_load(f)
    return {d["id"] for d in doc.get("developments", []) if isinstance(d, dict) and d.get("id")}


def harvest_repo(repo_name, repo_path):
    """Return list of {dev_id, sha, repo, subject, ts} for trailer-tagged commits."""
    if not os.path.isdir(os.path.join(repo_path, ".git")) and not os.path.exists(os.path.join(repo_path, ".git")):
        return []
    fmt = FS.join(["%H", "%cI", "%s", "%b"]) + RS
    try:
        out = subprocess.run(
            ["git", "-C", repo_path, "log", "--all", "--grep=^Development:", "--format=" + fmt],
            capture_output=True, text=True, timeout=60,
        ).stdout
    except Exception as exc:  # noqa: BLE001 — never fatal per repo
        print(f"WARN: git log failed for {repo_name}: {exc}", file=sys.stderr)
        return []

    rows = []
    for rec in out.split(RS):
        rec = rec.strip("\n")
        if not rec.strip():
            continue
        parts = rec.split(FS)
        if len(parts) < 4:
            continue
        sha, ts, subject, body = parts[0], parts[1], parts[2], parts[3]
        for line in body.splitlines():
            line = line.strip()
            if line.lower().startswith("development:"):
                dev_id = line.split(":", 1)[1].strip().strip(",").split()[0] if ":" in line else ""
                if dev_id:
                    rows.append({"dev_id": dev_id, "sha": sha, "repo": repo_name,
                                 "subject": subject.strip(), "ts": ts})
    return rows


def pr_number(repo_path, sha):
    """Best-effort PR lookup via gh; never fatal, never slow the hook (opt-in)."""
    try:
        r = subprocess.run(
            ["gh", "pr", "list", "--search", sha, "--state", "all", "--json", "number", "-q", ".[0].number"],
            cwd=repo_path, capture_output=True, text=True, timeout=10,
        )
        n = (r.stdout or "").strip()
        return f"#{n}" if n and n.isdigit() else None
    except Exception:  # noqa: BLE001
        return None


def main():
    ap = argparse.ArgumentParser(description="Harvest Development: trailers → Console initiative evidence")
    ap.add_argument("--dry-run", action="store_true", help="compute + print, write nothing")
    ap.add_argument("--with-pr", action="store_true", help="enrich anchors with PR numbers via gh (slower; for manual deploy)")
    args = ap.parse_args()

    registry_ids = load_registry_ids()

    # existing evidence (preserve + dedupe by full sha within each dev_id)
    existing = {"evidence": {}, "activity": {}, "orphans": []}
    if os.path.isfile(AUTO_EVIDENCE):
        try:
            with open(AUTO_EVIDENCE) as f:
                existing = json.load(f)
        except Exception as exc:  # noqa: BLE001
            print(f"WARN: could not read existing {AUTO_EVIDENCE}: {exc}", file=sys.stderr)
    evidence = existing.get("evidence", {})
    seen = {dev: {e["sha"] for e in lst} for dev, lst in evidence.items()}

    all_rows = []
    for name, path in REPOS.items():
        all_rows.extend(harvest_repo(name, path))

    added, orphan_rows = 0, []
    for row in all_rows:
        dev_id = row["dev_id"]
        if dev_id not in registry_ids:
            orphan_rows.append(row)
            continue
        if row["sha"] in seen.get(dev_id, set()):
            continue  # idempotent: already recorded
        if args.with_pr:
            row["pr"] = pr_number(REPOS[row["repo"]], row["sha"])
        else:
            row.setdefault("pr", None)
        evidence.setdefault(dev_id, []).append(row)
        seen.setdefault(dev_id, set()).add(row["sha"])
        added += 1

    # sort newest-first; derive activity
    activity = {}
    for dev_id, lst in evidence.items():
        lst.sort(key=lambda e: e.get("ts", ""), reverse=True)
        top = lst[0]
        activity[dev_id] = {"last_sha": top["sha"][:8], "last_ts": top["ts"],
                            "last_subject": top["subject"], "count": len(lst)}

    # orphans: dedupe the freshly-found ones by (dev_id, sha)
    orphans, oseen = [], set()
    for r in orphan_rows:
        k = (r["dev_id"], r["sha"])
        if k not in oseen:
            oseen.add(k)
            orphans.append({"dev_id": r["dev_id"], "sha": r["sha"][:8],
                            "repo": r["repo"], "subject": r["subject"], "ts": r["ts"]})

    out = {"generated_at": now_iso(), "evidence": evidence, "activity": activity, "orphans": orphans}

    # summary (always printed — this is the harvest's observable behaviour)
    print(f"[ide-sync] scanned {len(all_rows)} trailer-commit(s) across {len(REPOS)} repo(s)")
    print(f"[ide-sync] +{added} new anchor(s); {len(evidence)} initiative(s) with evidence; {len(orphans)} orphan trailer(s)")
    for dev_id in sorted(activity):
        a = activity[dev_id]
        print(f"  - {dev_id}: {a['count']} anchor(s), last {a['last_sha']} ({a['last_ts']})")
    if orphans:
        print("  ORPHAN trailers (id not in registry — curate an initiative or fix the trailer):")
        for o in orphans:
            print(f"    ! {o['dev_id']} <- {o['repo']} {o['sha']} {o['subject']}")

    if args.dry_run:
        print("[ide-sync] --dry-run: no files written")
        return

    os.makedirs(DEV_PLATFORM, exist_ok=True)
    with open(AUTO_EVIDENCE, "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
    with open(CURSOR, "w") as f:
        json.dump({"last_run": now_iso(),
                   "repos": {n: _head(p) for n, p in REPOS.items()},
                   "anchors_total": sum(len(v) for v in evidence.values()),
                   "orphans": len(orphans)}, f, indent=2)
    print(f"[ide-sync] wrote {AUTO_EVIDENCE} + {os.path.basename(CURSOR)}")


def _head(repo_path):
    try:
        return subprocess.run(["git", "-C", repo_path, "rev-parse", "HEAD"],
                              capture_output=True, text=True, timeout=10).stdout.strip()[:12]
    except Exception:  # noqa: BLE001
        return None


if __name__ == "__main__":
    main()
