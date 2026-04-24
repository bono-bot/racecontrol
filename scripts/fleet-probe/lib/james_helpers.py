"""james_helpers.py -- Phase 448 Plan 06
Helper script for probe-james.sh to avoid heredoc/stdin issues on Windows.
All subcommands read from file paths (sys.argv), never from stdin.

Usage:
  python james_helpers.py parse_tasklist <csv_file> <out_json_file>
  python james_helpers.py parse_schtasks <list_file> <out_json_file>
  python james_helpers.py parse_autostart <out_json_file>
  python james_helpers.py make_config_hash <hash_value> <out_json_file>
  python james_helpers.py append_error <errors_json> <sub_probe> <msg>
  python james_helpers.py elapsed_ms
"""
import csv
import hashlib
import json
import os
import subprocess
import sys
import time


def parse_tasklist(args):
    """Parse tasklist /V /FO CSV output into JSON array. Args: [csv_file, out_file]"""
    csv_file, out_file = args[0], args[1]
    rows = []
    if os.path.exists(csv_file):
        with open(csv_file, encoding="utf-8-sig", errors="replace") as f:
            reader = csv.reader(f)
            for i, row in enumerate(reader):
                if i == 0:
                    continue  # skip header
                if len(row) < 2:
                    continue
                name = row[0]
                try:
                    pid = int(row[1])
                except Exception:
                    continue
                cmdline = " ".join(row)
                h = hashlib.sha256(cmdline.encode("utf-8", "replace")).hexdigest()
                rows.append({"name": name, "pid": pid, "cmdline_hash": h})
    with open(out_file, "w") as f:
        json.dump(rows, f)


def parse_schtasks(args):
    """Parse schtasks /Query /V /FO LIST output into JSON array. Args: [list_file, out_file]"""
    list_file, out_file = args[0], args[1]
    entries = []
    if os.path.exists(list_file):
        cur = {}
        with open(list_file, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.rstrip("\r\n")
                if not line.strip():
                    if cur.get("name") and cur.get("state"):
                        entries.append({"name": cur["name"], "state": cur["state"]})
                    cur = {}
                    continue
                if ":" in line:
                    k, _, v = line.partition(":")
                    k = k.strip()
                    v = v.strip()
                    if k == "TaskName":
                        cur["name"] = v.lstrip("\\")
                    elif k == "Status":
                        cur["state"] = v
        if cur.get("name") and cur.get("state"):
            entries.append({"name": cur["name"], "state": cur["state"]})
        entries = entries[:100]
    with open(out_file, "w") as f:
        json.dump(entries, f)


def parse_autostart(args):
    """Collect HKLM/HKCU Run registry entries + startup folder into JSON. Args: [out_file]"""
    out_file = args[0]
    entries = []
    for hive, source in (("HKLM", "HKLM_Run"), ("HKCU", "HKCU_Run")):
        try:
            out = subprocess.check_output(
                ["reg", "query", hive + r"\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"],
                text=True,
                stderr=subprocess.DEVNULL,
            )
            for line in out.splitlines():
                line = line.strip()
                if not line or line.startswith("HKEY_"):
                    continue
                parts = line.split(None, 2)
                if len(parts) == 3:
                    entries.append({"source": source, "key": parts[0], "value": parts[2]})
        except Exception:
            pass
    startup = os.path.expandvars(r"%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup")
    if os.path.isdir(startup):
        for fname in os.listdir(startup):
            entries.append(
                {"source": "startup_folder", "key": fname, "value": os.path.join(startup, fname)}
            )
    with open(out_file, "w") as f:
        json.dump(entries, f)


def make_config_hash(args):
    """Build config_hash JSON from a single hash value. Args: [hash_value, out_file]"""
    hash_value, out_file = args[0], args[1]
    with open(out_file, "w") as f:
        json.dump({"comms-link/config.toml": hash_value}, f)


def append_error(args):
    """Append a probe error entry to a JSON array. Prints updated JSON to stdout.
    Args: [errors_json_str, sub_probe, message]"""
    errors_json, sub_probe, msg = args[0], args[1], args[2]
    a = json.loads(errors_json)
    a.append({"sub_probe": sub_probe, "error": msg})
    print(json.dumps(a))


def elapsed_ms(args):
    """Print current epoch milliseconds. Args: []"""
    print(int(time.time() * 1000))


COMMANDS = {
    "parse_tasklist": parse_tasklist,
    "parse_schtasks": parse_schtasks,
    "parse_autostart": parse_autostart,
    "make_config_hash": make_config_hash,
    "append_error": append_error,
    "elapsed_ms": elapsed_ms,
}

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        print(f"Usage: {sys.argv[0]} <{' | '.join(COMMANDS)}> [args...]", file=sys.stderr)
        sys.exit(1)
    COMMANDS[sys.argv[1]](sys.argv[2:])
