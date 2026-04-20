#!/usr/bin/env bash
# kiosk-swap-verify.sh - Linux counterpart to kiosk-swap-verify.ps1
#
# Post-swap invariant check for Linux hosts (Bono VPS, future Linux kiosks).
# Run within 5 min of any kiosk/browser swap.
#
# Fails (exit 1) if:
#   - any autostart surface still references $OLD_BINARY
#   - running process count of $OLD_BINARY > $MAX_OLD_PROCS
#   - no autostart surface references $NEW_BINARY (sanity check swap happened)
#
# Surfaces checked (from autostart-surfaces.sh JSON):
#   - systemd system + user units (unit file contents via systemctl cat)
#   - /etc/cron.d/*, /etc/crontab, user crontabs, root crontab
#   - /etc/rc.local, /etc/profile.d/*, ~/.bashrc / ~/.profile / ~/.bash_profile
#   - XDG autostart
#   - pm2 jlist (exec path + args)
#   - on-disk browser-ref scan (matches field from enumerator)
#   - running processes (cmd field)
#
# Usage:
#   ./kiosk-swap-verify.sh --old chromium --new firefox [--max-old 0] [--ignore-regex '-backup-'] [--baseline /tmp/out.json]
#
# The --ignore-regex filter matches the Windows .ps1 -IgnorePathsRegex default
# (-backup-) so 72h rollback backup files aren't counted as drift.

set -u

OLD_BINARY=""
NEW_BINARY=""
MAX_OLD_PROCS=0
IGNORE_REGEX='-backup-'
BASELINE_OUT=""

while [ $# -gt 0 ]; do
    case "$1" in
        --old)          OLD_BINARY="$2"; shift 2 ;;
        --new)          NEW_BINARY="$2"; shift 2 ;;
        --max-old)      MAX_OLD_PROCS="$2"; shift 2 ;;
        --ignore-regex) IGNORE_REGEX="$2"; shift 2 ;;
        --baseline)     BASELINE_OUT="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,/^set -u/p' "$0" | sed 's/^#//'
            exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$OLD_BINARY" ] || [ -z "$NEW_BINARY" ]; then
    echo "ERROR: --old and --new are required" >&2
    exit 2
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
ENUMERATOR="$HERE/autostart-surfaces.sh"
if [ ! -x "$ENUMERATOR" ] && [ ! -f "$ENUMERATOR" ]; then
    echo "ERROR: missing $ENUMERATOR" >&2
    exit 2
fi

TMP_JSON=$(mktemp)
trap 'rm -f "$TMP_JSON"' EXIT

# Run the enumerator. It accepts an output path as $1.
bash "$ENUMERATOR" "$TMP_JSON" >/dev/null || {
    echo "ERROR: autostart-surfaces.sh failed" >&2
    exit 2
}

if [ -n "$BASELINE_OUT" ]; then
    cp -f "$TMP_JSON" "$BASELINE_OUT"
fi

# Walk the JSON with python3 and classify matches into old_refs / new_refs /
# ignored_refs. Matching is substring-based (same as .ps1 -like) to be forgiving
# about binary path vs basename.
ANALYSIS=$(mktemp)
trap 'rm -f "$TMP_JSON" "$ANALYSIS"' EXIT

python3 - "$TMP_JSON" "$OLD_BINARY" "$NEW_BINARY" "$IGNORE_REGEX" > "$ANALYSIS" <<'PYEOF'
import json, re, sys

path, old_bin, new_bin, ignore_regex = sys.argv[1:5]
state = json.load(open(path))
surfaces = state.get("surfaces", {})
ignore_re = re.compile(ignore_regex) if ignore_regex else None

old_refs = []
new_refs = []
ignored_refs = []

def classify(surface_name, ref_str):
    if not ref_str:
        return
    s = str(ref_str)
    ignored = bool(ignore_re and ignore_re.search(s))
    if old_bin in s and not ignored:
        old_refs.append({"surface": surface_name, "ref": s[:300]})
    elif old_bin in s and ignored:
        ignored_refs.append({"surface": surface_name, "ref": s[:300]})
    if new_bin in s:
        new_refs.append({"surface": surface_name, "ref": s[:300]})

for scope_key in ("systemd_system_units", "systemd_user_units"):
    for u in surfaces.get(scope_key, []) or []:
        classify(scope_key, f"{u.get('unit','')} state={u.get('state','')}")

for f in surfaces.get("etc_cron_d", []) or []:
    classify("etc_cron_d", f)
classify("etc_crontab", surfaces.get("etc_crontab"))
classify("root_crontab", surfaces.get("root_crontab"))
for name in surfaces.get("user_crontabs", []) or []:
    classify("user_crontabs", name)

classify("etc_rc_local", surfaces.get("etc_rc_local"))
for f in surfaces.get("etc_profile_d", []) or []:
    classify("etc_profile_d", f)

shell_init = surfaces.get("user_shell_init", {}) or {}
for k in ("bashrc", "profile", "bash_profile"):
    classify(f"user_shell_init.{k}", shell_init.get(k))

for f in surfaces.get("xdg_autostart", []) or []:
    classify("xdg_autostart", f)

for p in surfaces.get("pm2_list", []) or []:
    classify("pm2", f"{p.get('name','')}: {p.get('exec','')} {p.get('args','')}")

for r in surfaces.get("on_disk_browser_refs", []) or []:
    classify("on_disk", f"{r.get('path','')}: {r.get('matches','')}")

running_old = 0
running_new = 0
for p in surfaces.get("running_processes", []) or []:
    cmd = p.get("cmd", "") or ""
    comm = p.get("comm", "") or ""
    classify("running", f"{comm}: {cmd}")
    if old_bin == comm or old_bin in cmd:
        running_old += 1
    if new_bin == comm or new_bin in cmd:
        running_new += 1

print(json.dumps({
    "old_refs": old_refs,
    "new_refs": new_refs,
    "ignored_refs": ignored_refs,
    "running_old": running_old,
    "running_new": running_new,
}))
PYEOF

# Parse the analysis JSON into counts + refs
OLD_COUNT=$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['old_refs']))" "$ANALYSIS")
NEW_COUNT=$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['new_refs']))" "$ANALYSIS")
IGNORED_COUNT=$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))['ignored_refs']))" "$ANALYSIS")
RUNNING_OLD=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['running_old'])" "$ANALYSIS")
RUNNING_NEW=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['running_new'])" "$ANALYSIS")

FAIL=0

if [ "$OLD_COUNT" -gt 0 ]; then
    FAIL=1
    echo "FAIL: $OLD_BINARY still referenced in $OLD_COUNT autostart surface(s):"
    python3 -c "
import json,sys
for r in json.load(open(sys.argv[1]))['old_refs']:
    print(f\"  - [{r['surface']}] {r['ref']}\")
" "$ANALYSIS"
else
    echo "OK:   No autostart surface references $OLD_BINARY"
fi

if [ "$IGNORED_COUNT" -gt 0 ]; then
    echo "INFO: $IGNORED_COUNT $OLD_BINARY ref(s) in --ignore-regex '$IGNORE_REGEX' (not counted as drift):"
    python3 -c "
import json,sys
for r in json.load(open(sys.argv[1]))['ignored_refs']:
    print(f\"  - [{r['surface']}] {r['ref']}\")
" "$ANALYSIS"
fi

if [ "$RUNNING_OLD" -gt "$MAX_OLD_PROCS" ]; then
    FAIL=1
    echo "FAIL: $OLD_BINARY process count = $RUNNING_OLD (max allowed = $MAX_OLD_PROCS)"
else
    echo "OK:   $OLD_BINARY process count = $RUNNING_OLD (<= $MAX_OLD_PROCS)"
fi

if [ "$NEW_COUNT" -eq 0 ]; then
    FAIL=1
    echo "FAIL: $NEW_BINARY is NOT referenced by any autostart surface - swap did not take effect"
else
    echo "OK:   $NEW_BINARY referenced by $NEW_COUNT autostart surface(s)"
fi

if [ "$RUNNING_NEW" -eq 0 ]; then
    echo "WARN: $NEW_BINARY not currently running (may not have launched yet - wait and re-check)"
else
    echo "OK:   $NEW_BINARY process count = $RUNNING_NEW"
fi

exit $FAIL
