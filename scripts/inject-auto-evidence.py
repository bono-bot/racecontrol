#!/usr/bin/env python3
"""Merge harvested IDE evidence into the generated dev-registry.json (deploy step).

The generator (rp-v2-apps/apps/racecontrol-console/scripts/gen-dev-registry.py)
emits dev-registry.json straight from the hand-curated developments.yaml. This
step runs AFTER it and folds the auto-harvested evidence (from
sync-ide-initiatives.py) into each development's `evidence_anchors` — the field
the Console /initiatives tracker renders. It touches ONLY the generated artifact,
never the curated YAML and never the Console source (no PR needed; same write-
class the generator already performs).

Idempotent: it strips any prior auto anchors (marked `[IDE ...]`) and re-adds the
current set, so re-running converges. Curated anchors (no `[IDE` prefix) are
preserved and stay first.

Run order (see deploy-console-registry.sh):
  sync-ide-initiatives.py  →  gen-dev-registry.py  →  THIS  →  pm2 restart
Doctrine: .planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md §5
"""
import json
import os
import sys

DEV_PLATFORM = "/root/racecontrol/.planning/specs/dev-platform"
AUTO_EVIDENCE = os.path.join(DEV_PLATFORM, "developments.auto-evidence.json")
DEV_REGISTRY = "/root/rp-v2-apps/apps/racecontrol-console/data/dev-registry.json"

AUTO_PREFIX = "[IDE"          # marker that distinguishes auto anchors from curated ones
MAX_PER_DEV = 12             # cap individual commit anchors to keep the list readable


def _anchor(rec):
    sha = rec.get("sha", "")[:8]
    repo = rec.get("repo", "?")
    subj = (rec.get("subject") or "").strip()
    pr = rec.get("pr")
    tail = f" ({pr})" if pr else ""
    return f"[IDE {sha}] {repo}: {subj}{tail}"


def main():
    if not os.path.isfile(AUTO_EVIDENCE):
        print(f"[inject] no auto-evidence at {AUTO_EVIDENCE} — nothing to inject (run sync-ide-initiatives.py first)")
        return
    if not os.path.isfile(DEV_REGISTRY):
        print(f"ERROR: {DEV_REGISTRY} not found — run gen-dev-registry.py first", file=sys.stderr)
        sys.exit(1)

    with open(AUTO_EVIDENCE) as f:
        auto = json.load(f)
    with open(DEV_REGISTRY) as f:
        reg = json.load(f)

    evidence = auto.get("evidence", {})
    activity = auto.get("activity", {})
    by_id = {d.get("id"): d for d in reg.get("developments", []) if isinstance(d, dict)}

    injected, skipped = 0, []
    for dev_id, recs in evidence.items():
        dev = by_id.get(dev_id)
        if not dev:
            skipped.append(dev_id)        # in auto-evidence but not in registry (shouldn't happen — harvester filters)
            continue
        anchors = dev.get("evidence_anchors", []) or []
        curated = [a for a in anchors if not (isinstance(a, str) and a.startswith(AUTO_PREFIX))]

        recs_sorted = sorted(recs, key=lambda r: r.get("ts", ""), reverse=True)
        act = activity.get(dev_id, {})
        summary = f"[IDE activity] {act.get('count', len(recs_sorted))} commit(s); last {act.get('last_sha','?')} ({(act.get('last_ts','') or '')[:10]})"
        auto_anchors = [summary] + [_anchor(r) for r in recs_sorted[:MAX_PER_DEV]]

        dev["evidence_anchors"] = curated + auto_anchors
        injected += 1

    with open(DEV_REGISTRY, "w") as f:
        json.dump(reg, f, indent=2, ensure_ascii=False)

    print(f"[inject] merged IDE evidence into {injected} initiative(s) in {DEV_REGISTRY}")
    if skipped:
        print(f"[inject] WARN: {len(skipped)} auto-evidence id(s) not in registry (stale?): {', '.join(skipped)}", file=sys.stderr)


if __name__ == "__main__":
    main()
