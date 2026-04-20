#!/usr/bin/env bash
# Linux counterpart to autostart-surfaces.ps1 (Windows).
# Enumerates autostart surfaces on a Linux host + on-disk scan for browser/kiosk refs.
# Output: JSON to stdout OR to $1 if provided.
#
# Surfaces covered:
#   - systemd system units (/etc/systemd/system, enabled state)
#   - systemd user units (~/.config/systemd/user, enabled state)
#   - /etc/cron.d/*
#   - /etc/crontab
#   - root crontab (crontab -l)
#   - user crontabs (/var/spool/cron/crontabs/*)
#   - /etc/rc.local
#   - /etc/profile.d/*.sh
#   - ~/.bashrc, ~/.profile, ~/.bash_profile (for root)
#   - XDG autostart (~/.config/autostart, /etc/xdg/autostart) - rarely used on
#     headless servers but included for parity
#   - pm2 dump (pm2 is the primary long-running process manager for racecontrol
#     on Bono VPS - covers kiosk/web/admin/racecontrol processes)
#   - On-disk scan of /root/ and /opt/racingpoint/ for .sh/.js/.ts/.json files
#     referencing chromium|firefox|msedge|chrome|--kiosk|--app|puppeteer|playwright
#   - Running processes matching browser/kiosk patterns
#
# Usage:
#   ./autostart-surfaces.sh                -> JSON to stdout
#   ./autostart-surfaces.sh /tmp/out.json  -> JSON to file

set -u

OUT="${1:-}"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT

# Helper: emit JSON array from command producing lines. Each line becomes a string.
jarr_from_lines() {
    local first=1
    printf '['
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        if [ $first -eq 1 ]; then first=0; else printf ','; fi
        # JSON-escape: backslash, quotes, control chars
        printf '%s' "$line" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read().rstrip("\n")), end="")'
    done
    printf ']'
}

# Helper: read file contents as a single JSON string (if file exists) or null
jfile() {
    if [ -f "$1" ] && [ -r "$1" ]; then
        python3 -c 'import sys,json; print(json.dumps(open(sys.argv[1],"r",errors="replace").read()), end="")' "$1"
    else
        printf 'null'
    fi
}

# Helper: JSON array of objects {name, enabled, state, unit_file} for systemd units
systemd_units_json() {
    local scope="$1"  # "system" or "--user"
    local args=""
    [ "$scope" = "user" ] && args="--user"
    if ! command -v systemctl >/dev/null 2>&1; then
        printf '[]'
        return
    fi
    systemctl $args list-unit-files --type=service --no-legend --no-pager 2>/dev/null |
        awk '{printf "{\"unit\":\"%s\",\"state\":\"%s\",\"preset\":\"%s\"}\n", $1, $2, ($3?$3:"-")}' |
        python3 -c '
import sys, json
objs = []
for line in sys.stdin:
    line=line.strip()
    if not line: continue
    try: objs.append(json.loads(line))
    except: pass
print(json.dumps(objs))
'
}

# Build JSON
{
    printf '{'
    printf '"host":"%s",' "$(hostname 2>/dev/null || echo unknown)"
    printf '"user":"%s",' "${USER:-$(whoami 2>/dev/null || echo unknown)}"
    printf '"timestamp_utc":"%s",' "$(date -u +%Y-%m-%dT%H:%M:%S.000Z)"
    # last boot (best effort)
    BOOT=$(uptime -s 2>/dev/null || echo unknown)
    printf '"last_boot_local":"%s",' "$BOOT"
    printf '"os":"%s",' "$(uname -srv | tr -d '"' | head -c 200)"
    printf '"surfaces":{'

    # --- systemd units ---
    printf '"systemd_system_units":'
    systemd_units_json system
    printf ','

    printf '"systemd_user_units":'
    systemd_units_json user
    printf ','

    # --- cron ---
    printf '"etc_cron_d":'
    if [ -d /etc/cron.d ]; then
        find /etc/cron.d -maxdepth 1 -type f 2>/dev/null | jarr_from_lines
    else
        printf '[]'
    fi
    printf ','

    printf '"etc_crontab":'
    jfile /etc/crontab
    printf ','

    printf '"user_crontabs":'
    if [ -d /var/spool/cron/crontabs ]; then
        sudo ls /var/spool/cron/crontabs 2>/dev/null | jarr_from_lines
    elif [ -d /var/spool/cron ]; then
        ls /var/spool/cron 2>/dev/null | jarr_from_lines
    else
        printf '[]'
    fi
    printf ','

    printf '"root_crontab":'
    jfile <(crontab -l 2>/dev/null)
    printf ','

    # --- rc.local / profile ---
    printf '"etc_rc_local":'
    jfile /etc/rc.local
    printf ','

    printf '"etc_profile_d":'
    if [ -d /etc/profile.d ]; then
        find /etc/profile.d -maxdepth 1 -type f 2>/dev/null | jarr_from_lines
    else
        printf '[]'
    fi
    printf ','

    printf '"user_shell_init":{'
    printf '"bashrc":'; jfile ~/.bashrc; printf ','
    printf '"profile":'; jfile ~/.profile; printf ','
    printf '"bash_profile":'; jfile ~/.bash_profile
    printf '},'

    # --- XDG autostart (rare on servers) ---
    printf '"xdg_autostart":'
    {
        [ -d ~/.config/autostart ] && find ~/.config/autostart -maxdepth 1 -type f 2>/dev/null
        [ -d /etc/xdg/autostart ] && find /etc/xdg/autostart -maxdepth 1 -type f 2>/dev/null
    } | jarr_from_lines
    printf ','

    # --- pm2 process list (primary long-running manager on Bono VPS) ---
    printf '"pm2_list":'
    if command -v pm2 >/dev/null 2>&1; then
        pm2 jlist 2>/dev/null | python3 -c '
import sys, json
try:
    data = json.load(sys.stdin)
    out = []
    for p in data:
        pm2_env = p.get("pm2_env", {})
        out.append({
            "name": p.get("name"),
            "pid": p.get("pid"),
            "status": pm2_env.get("status"),
            "exec": pm2_env.get("pm_exec_path"),
            "cwd": pm2_env.get("pm_cwd"),
            "args": pm2_env.get("args"),
            "autorestart": pm2_env.get("autorestart"),
            "uptime_ms": int(pm2_env.get("pm_uptime") or 0),
        })
    print(json.dumps(out))
except Exception as e:
    print("[]")
' || echo '[]'
    else
        printf '[]'
    fi
    printf ','

    # --- On-disk scan for browser/kiosk refs ---
    printf '"on_disk_browser_refs":'
    {
        for root in /root /opt/racingpoint /srv/racingpoint; do
            [ -d "$root" ] || continue
            # Grep for browser/kiosk patterns, .sh/.js/.ts/.json/.toml files, max depth 4
            find "$root" -maxdepth 4 -type f \( \
                -name '*.sh' -o -name '*.js' -o -name '*.ts' -o -name '*.json' -o -name '*.toml' -o -name '*.service' \
                \) 2>/dev/null | while read -r f; do
                # skip large files (>1MB) and node_modules
                case "$f" in
                    *node_modules*|*.git/*|*\.next/*) continue ;;
                esac
                sz=$(stat -c %s "$f" 2>/dev/null || echo 999999999)
                [ "$sz" -gt 1048576 ] && continue
                if grep -lEi 'chromium|msedge|--kiosk|--app=http|puppeteer\.launch|playwright\.chromium|browser\s*=\s*["'"'"']chromium' "$f" >/dev/null 2>&1; then
                    matches=$(grep -oEi 'chromium|msedge|--kiosk|--app=http|puppeteer|playwright' "$f" 2>/dev/null | sort -u | tr '\n' ',' | sed 's/,$//')
                    mtime=$(stat -c %Y "$f" 2>/dev/null || echo 0)
                    printf '{"path":"%s","size":%s,"mtime_epoch":%s,"matches":"%s"}\n' \
                        "$f" "$sz" "$mtime" "$matches"
                fi
            done
        done
    } | python3 -c '
import sys, json
objs=[]
for line in sys.stdin:
    line=line.strip()
    if not line: continue
    try: objs.append(json.loads(line))
    except: pass
print(json.dumps(objs))
'
    printf ','

    # --- Running browser/kiosk/pm2-managed processes ---
    printf '"running_processes":'
    ps -eo pid,ppid,etimes,comm,args --no-headers 2>/dev/null |
        grep -Ei 'chromium|msedge|chrome|--kiosk|pm2|node.*racecontrol|node.*kiosk|node.*admin' |
        grep -v grep |
        python3 -c '
import sys, json
out=[]
for line in sys.stdin:
    parts=line.split(None, 4)
    if len(parts) < 5: continue
    pid, ppid, etime, comm, args = parts
    out.append({
        "pid": int(pid),
        "ppid": int(ppid),
        "uptime_secs": int(etime) if etime.isdigit() else 0,
        "comm": comm,
        "cmd": args.rstrip("\n")[:400],
    })
print(json.dumps(out))
' || echo '[]'

    printf '}'  # surfaces
    printf '}'  # root
} > "$TMP"

# Validate JSON
if ! python3 -m json.tool "$TMP" >/dev/null 2>&1; then
    echo "ERROR: produced invalid JSON" >&2
    cat "$TMP" >&2
    exit 1
fi

# Pretty-print
if [ -n "$OUT" ]; then
    python3 -m json.tool "$TMP" > "$OUT"
    echo "Wrote $OUT"
else
    python3 -m json.tool "$TMP"
fi
