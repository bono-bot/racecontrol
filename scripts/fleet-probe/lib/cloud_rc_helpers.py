"""
scripts/fleet-probe/lib/cloud_rc_helpers.py -- Phase 448 Plan 06
Python helpers for probe-cloud-rc.sh.
Each subcommand is selected via sys.argv[1]:
  validate_json  <file>       -- exit 0 if valid JSON, 1 otherwise
  extract_bid    <file>       -- print value of build_id field (or empty)
  build_manifest <target_id> <host> <ip> <probed_at> <probe_status>
                 <build_id_json> <env_hash> <probe_errors_json> <out_file>

Called as: python3 <this_file> <subcommand> [args...]
"""
import json, sys

def validate_json(args):
    try:
        with open(args[0]) as f:
            json.load(f)
        sys.exit(0)
    except Exception:
        sys.exit(1)

def extract_bid(args):
    try:
        with open(args[0]) as f:
            d = json.load(f)
        v = d.get("build_id", "")
        if v:
            print(v)
    except Exception:
        pass

def build_manifest(args):
    (target_id, host, ip,
     probed_at, probe_status,
     build_id_json, env_hash,
     probe_errors_json,
     out_file) = args[:9]

    m = {
        "schema_version":    "1.0",
        "target_id":         target_id,
        "host":              host,
        "ip":                ip,
        "role":              "cloud_racecontrol",
        "probed_at_ist":     probed_at,
        "probe_status":      probe_status,
        "binary_sha256":     {},
        "build_id":          json.loads(build_id_json),
        "config_hash":       {},
        "running_procs":     [],
        "scheduled_tasks":   [],
        "autostart_entries": [],
        "env_vars_hash":     env_hash,
        "last_deploy_ts":    None,
    }
    errors = json.loads(probe_errors_json)
    if errors:
        m["probe_errors"] = errors

    with open(out_file, "w") as f:
        json.dump(m, f)

COMMANDS = {
    "validate_json": validate_json,
    "extract_bid": extract_bid,
    "build_manifest": build_manifest,
}

if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd not in COMMANDS:
        sys.stderr.write(f"Unknown command: {cmd}\n")
        sys.exit(1)
    COMMANDS[cmd](sys.argv[2:])
