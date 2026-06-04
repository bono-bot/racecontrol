#!/usr/bin/env python3
"""
P1 readout generator for the dev-platform registry (DEV-PLATFORM-DESIGN.md §8, P1).

Reads the P0 hand-maintained registry (apps.yaml + developments.yaml), runs the
HOST-AVAILABLE probes live (gh CI status, gh open-PR count, git last-commit), and
renders a readout (REGISTRY.md + registry-live.json).

Honesty rules (design §5 + capability-claim discipline):
  - Probe values are LIVE (run now), never projected. A failed probe records
    "unavailable: <reason>", never a guessed value.
  - Venue/auth/CI-secret-gated probes (/api/v1/fleet/health, /fleet/intelligence,
    check-parity, billing SQL) are NOT reachable from this host -> marked DEFERRED.

Regenerate:  python3 scripts/dev-platform/build_registry.py
"""
import yaml, json, subprocess, os, sys, socket
from datetime import datetime, timezone, timedelta

HERE = os.path.dirname(os.path.abspath(__file__))
SPEC_DIR = os.path.abspath(os.path.join(HERE, "..", "..", ".planning", "specs", "dev-platform"))

REPO_ROOTS = {
    "racecontrol": "/root/racecontrol",
    "rp-v2-apps": "/root/rp-v2-apps",
    "racingpoint-cloud-dashboard": "/root/racingpoint-cloud-dashboard",
    "racingpoint-api-gateway": "/root/racingpoint-api-gateway",
}

# Venue/auth-gated probes we deliberately defer (cannot run honestly from this host)
DEFERRED_PROBES = [
    ("build_id / fleet state", "GET http://192.168.31.23:8080/api/v1/fleet/health", "venue .23 + pods (0/8 off)"),
    ("pod health score", "GET /api/v1/fleet/intelligence", "venue .23 + staff JWT"),
    ("contract parity", "pnpm run check-parity", "needs rp-v2-apps install / CI runner"),
    ("revenue / session success", "billing_sessions SQL", "venue/cloud DB + auth"),
    ("code coverage %", "(not instrumented)", "no nyc/tarpaulin in CI yet"),
]

SYM = {"done": "✅", "in_phase": "🟡", "not_started": "🔴", "gated": "⛔", "frozen": "❄️"}


def run(cmd, cwd=None, timeout=40):
    try:
        r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        if r.returncode != 0:
            return None, (r.stderr.strip().splitlines()[-1] if r.stderr.strip() else f"exit {r.returncode}")
        return r.stdout.strip(), None
    except Exception as e:
        return None, str(e)


def repo_dir(repo):
    d = REPO_ROOTS.get(repo)
    return d if d and os.path.isdir(d) else None


# --- repo-level probes (cached) ---
_ci_cache, _pr_cache = {}, {}


def repo_ci(repo):
    if repo in _ci_cache:
        return _ci_cache[repo]
    d = repo_dir(repo)
    if not d:
        res = {"status": "unavailable", "reason": "repo dir not found"}
    else:
        out, err = run(["gh", "run", "list", "--limit", "1", "--json",
                        "status,conclusion,workflowName,headBranch"], cwd=d)
        if err:
            res = {"status": "unavailable", "reason": err[:70]}
        else:
            try:
                arr = json.loads(out)
                res = {"status": "no-runs"} if not arr else {
                    "status": arr[0].get("status"),
                    "conclusion": arr[0].get("conclusion"),
                    "workflow": arr[0].get("workflowName"),
                    "branch": arr[0].get("headBranch"),
                }
            except Exception:
                res = {"status": "parse-error"}
    _ci_cache[repo] = res
    return res


def repo_open_prs(repo):
    if repo in _pr_cache:
        return _pr_cache[repo]
    d = repo_dir(repo)
    val = None
    if d:
        out, err = run(["gh", "pr", "list", "--state", "open", "--limit", "200", "--json", "number"], cwd=d)
        if not err:
            try:
                val = len(json.loads(out))
            except Exception:
                val = None
    _pr_cache[repo] = val
    return val


def app_last_commit(repo, path):
    d = repo_dir(repo)
    if not d:
        return None
    out, _ = run(["git", "-C", d, "log", "-1", "--format=%cI", "--", path])
    return (out or "")[:10] or None  # YYYY-MM-DD


def load(name):
    with open(os.path.join(SPEC_DIR, name)) as f:
        return yaml.safe_load(f)


def main():
    apps_doc = load("apps.yaml")
    devs_doc = load("developments.yaml")
    apps = apps_doc["apps"]
    devs = devs_doc["developments"]
    dev_by_id = {d["id"]: d for d in devs}

    now = datetime.now(timezone.utc)
    ist = now + timedelta(hours=5, minutes=30)
    gen = {"generated_at_utc": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
           "generated_at_ist": ist.strftime("%Y-%m-%d %H:%M IST"),
           "host": socket.gethostname()}

    # enrich apps with live probes
    live_apps = []
    for a in apps:
        ci = repo_ci(a["repo"])
        rec = {
            "id": a["id"], "name": a["name"], "product_line": a["product_line"],
            "repo": a["repo"], "path": a.get("path"),
            "last_commit": app_last_commit(a["repo"], a.get("path", ".")),
            "repo_ci": ci, "repo_open_prs": repo_open_prs(a["repo"]),
            "active_developments": a.get("active_developments", []),
            "candidate": a.get("candidate", False),
        }
        live_apps.append(rec)

    out = {"meta": gen, "apps": live_apps, "developments": devs, "deferred_probes": DEFERRED_PROBES}
    with open(os.path.join(SPEC_DIR, "registry-live.json"), "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)

    # render REGISTRY.md
    L = []
    L.append("# REGISTRY — Dev-Platform live readout (P1, GENERATED)")
    L.append("")
    L.append(f"> **GENERATED FILE — do not hand-edit.** Regenerate: `python3 scripts/dev-platform/build_registry.py`")
    L.append(f"> **Generated:** {gen['generated_at_ist']} (UTC {gen['generated_at_utc']}) on `{gen['host']}` · "
             f"P1 of [`DEV-PLATFORM-DESIGN.md`](./DEV-PLATFORM-DESIGN.md) §8.")
    L.append(f"> **Source registries (hand-maintained):** [`apps.yaml`](./apps.yaml) · [`developments.yaml`](./developments.yaml). "
             f"Probe values below are LIVE; failures show `unavailable`; venue/auth probes are DEFERRED (see end).")
    L.append("")
    L.append(f"**Portfolio:** {len(apps)} product apps · {len(devs)} DMADV developments "
             f"({sum(1 for d in devs if d['freeze_status']=='frozen')} frozen).")
    L.append("")
    # Apps table
    L.append("## Applications (live probes)")
    L.append("")
    L.append("| App | Line | Last commit | CI (latest) | Repo open PRs | Active devs |")
    L.append("|---|---|---|---|---|---|")
    for r in live_apps:
        ci = r["repo_ci"]
        ci_txt = ci.get("conclusion") or ci.get("status") or "—"
        if ci.get("reason"):
            ci_txt = f"{ci_txt} ({ci['reason']})"
        prs = "—" if r["repo_open_prs"] is None else str(r["repo_open_prs"])
        lc = r["last_commit"] or "—"
        cand = " *(candidate)*" if r["candidate"] else ""
        L.append(f"| `{r['id']}`{cand} | {r['product_line']} | {lc} | {ci_txt} | {prs} | {len(r['active_developments'])} |")
    L.append("")
    L.append("*CI/open-PRs are repo-level (monorepo single pipeline) attributed to each app in that repo.*")
    L.append("")
    # Developments DMADV board
    L.append("## Developments — DMADV board")
    L.append("")
    L.append("| Development | D | M | A | Des | V | Current phase | Freeze |")
    L.append("|---|:-:|:-:|:-:|:-:|:-:|---|---|")
    for d in devs:
        m = d["dmadv"]
        cells = " | ".join(SYM.get(m[k], "?") for k in ("D", "M", "A", "Design", "Verify"))
        fz = {"unfrozen": "live", "frozen": "❄️ frozen", "in_flight": "in-flight"}.get(d["freeze_status"], d["freeze_status"])
        L.append(f"| {d['title']} | {cells} | {d['current_phase']} | {fz} |")
    L.append("")
    L.append("Legend: ✅ done · 🟡 in-phase · 🔴 not-started · ⛔ gated · ❄️ frozen.")
    L.append("")
    # Deferred probes
    L.append("## Deferred probes (not runnable from this host — design §5 🟠/🔴)")
    L.append("")
    L.append("| Metric | Source | Why deferred |")
    L.append("|---|---|---|")
    for metric, src, why in DEFERRED_PROBES:
        L.append(f"| {metric} | `{src}` | {why} |")
    L.append("")
    L.append("These land when P1 runs from a venue-reachable/authed context (or P2 automation wires CI secrets + a venue probe relay).")
    L.append("")

    with open(os.path.join(SPEC_DIR, "REGISTRY.md"), "w") as f:
        f.write("\n".join(L) + "\n")

    print(f"OK: wrote REGISTRY.md + registry-live.json to {SPEC_DIR}")
    print(f"apps={len(apps)} developments={len(devs)} generated={gen['generated_at_ist']}")
    # surface probe availability summary
    for repo in sorted({a['repo'] for a in apps}):
        ci = repo_ci(repo); prs = repo_open_prs(repo)
        print(f"  repo {repo}: ci={ci.get('conclusion') or ci.get('status')} open_prs={prs}")


if __name__ == "__main__":
    main()
