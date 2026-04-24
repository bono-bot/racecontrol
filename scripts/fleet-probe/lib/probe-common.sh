#!/bin/bash
# scripts/fleet-probe/lib/probe-common.sh -- Phase 448 Plan 01
# Shared helper library for all fleet probe scripts.
# SOURCE-ONLY -- do NOT run directly. No `exit` calls; use `return 1` for errors.
# Usage: source "$(dirname "$0")/lib/probe-common.sh"
#
# Contract (10 required functions):
#   json_escape, write_manifest, sha256_of, sha256_of_stdin, sha256_of_remote_file,
#   iso_ist_now, probe_status_from_errors, env_names_hash, env_names_hash_remote, cmdline_hash
set -eo pipefail

# json_escape STRING
# Escapes backslash, double-quote, and control characters.
# stdout: JSON-escaped string (no surrounding quotes).
# Uses python3 for correctness across all edge cases.
json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.argv[1])[1:-1])' "$1"
}

# iso_ist_now
# Returns ISO-8601 timestamp with +05:30 suffix.
# MUST use UTC_EPOCH + 19800 pattern -- TZ env override silently fails on Git Bash (returns UTC).
# stdout: "2026-04-24T17:30:00+05:30"
iso_ist_now() {
  local utc_epoch ist_epoch
  utc_epoch=$(date -u +%s)
  ist_epoch=$((utc_epoch + 19800))
  date -u -d "@$ist_epoch" '+%Y-%m-%dT%H:%M:%S+05:30' 2>/dev/null || \
    python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($ist_epoch).strftime('%Y-%m-%dT%H:%M:%S+05:30'))"
}

# sha256_of FILEPATH
# stdout: 64-char lowercase hex SHA256 of the given file.
sha256_of() {
  if [ -f "$1" ]; then
    sha256sum "$1" | awk '{print $1}'
  else
    echo "sha256_of: file not found: $1" >&2
    return 1
  fi
}

# sha256_of_stdin
# stdin: bytes -> stdout: 64-char lowercase hex SHA256.
sha256_of_stdin() {
  sha256sum | awk '{print $1}'
}

# sha256_of_remote_file SSH_TARGET REMOTE_PATH [OS=windows|linux]
# Returns 64-char lowercase hex; uses certutil on Windows, sha256sum on Linux.
sha256_of_remote_file() {
  local ssh_target="$1" remote_path="$2" os="${3:-windows}"
  if [ "$os" = "windows" ]; then
    ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "certutil -hashfile \"$remote_path\" SHA256 2>nul" 2>/dev/null \
      | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0); exit}'
  else
    ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "sha256sum \"$remote_path\"" 2>/dev/null | awk '{print $1}'
  fi
}

# probe_status_from_errors CONNECT_ERR_COUNT SUBPROBE_ERR_COUNT
# connect-stage errors dominate: if any connect error => probe_failed; else if any subprobe error => partial; else ok.
# stdout: "ok" | "probe_failed" | "partial"
probe_status_from_errors() {
  local connect_err="${1:-0}" subprobe_err="${2:-0}"
  if [ "$connect_err" -gt 0 ]; then
    echo "probe_failed"
  elif [ "$subprobe_err" -gt 0 ]; then
    echo "partial"
  else
    echo "ok"
  fi
}

# env_names_hash
# stdout: sha256 of sorted local env NAMES (SECURITY BOUNDARY: names only, never values).
env_names_hash() {
  env | awk -F= '{print $1}' | sort | sha256sum | awk '{print $1}'
}

# env_names_hash_remote SSH_TARGET
# stdout: sha256 of sorted remote env NAMES via SSH.
# For Windows over SSH: `set` returns NAME=VALUE lines; awk strips values.
env_names_hash_remote() {
  local ssh_target="$1"
  ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "set" 2>/dev/null \
    | tr -d '\r' | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}'
}

# cmdline_hash CMDLINE_STRING
# stdout: sha256 of the full cmdline string (SECURITY: hash only, never store raw).
cmdline_hash() {
  printf '%s' "$1" | sha256sum | awk '{print $1}'
}

# write_manifest TARGET_ID MANIFEST_JSON
# Requires: MANIFEST_TS env var must be set.
# Writes state/fleet-manifest/$MANIFEST_TS/<target_id>.json (pretty-printed via python3 json.tool).
# If FLEET_PROBE_VALIDATE=1, validates via validate-manifest-file.mjs before mv; returns 1 on failure.
write_manifest() {
  local target_id="$1" manifest_json="$2"
  if [ -z "${MANIFEST_TS:-}" ]; then
    echo "write_manifest: MANIFEST_TS env var not set" >&2
    return 2
  fi
  local out_dir="state/fleet-manifest/$MANIFEST_TS"
  mkdir -p "$out_dir"
  local out_file="$out_dir/$target_id.json"
  local tmp_file="$out_file.tmp"

  if ! echo "$manifest_json" | python3 -m json.tool > "$tmp_file" 2>/dev/null; then
    echo "write_manifest: $target_id manifest is not valid JSON" >&2
    rm -f "$tmp_file"
    return 1
  fi

  if [ "${FLEET_PROBE_VALIDATE:-0}" = "1" ]; then
    if ! node scripts/fleet-probe/validate-manifest-file.mjs "$tmp_file" >&2; then
      echo "write_manifest: $target_id manifest failed schema validation" >&2
      rm -f "$tmp_file"
      return 1
    fi
  fi

  mv "$tmp_file" "$out_file"
  return 0
}
