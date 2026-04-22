#!/usr/bin/env bash
# common.sh — shared helpers for behavioral E2E tests.
#
# Sourced by every test. Provides:
#   - IST timestamp formatting (repo is venue-timezone)
#   - JSONL log emit
#   - JWT acquisition + caching
#   - pod_exec wrapper with admin JWT
#   - assertion helpers (assert_*)
#
# All functions return 0 on success, non-zero on failure.
# Failure does NOT exit the shell — caller decides continuation policy.

set -uo pipefail

SERVER_URL="${SERVER_URL:-http://192.168.31.23:8080}"
TARGET_POD="${TARGET_POD:-pod_8}"
ADMIN_PIN="${ADMIN_PIN:-261121}"
LOG_DIR="${LOG_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/logs}"
LOG_FILE="${LOG_FILE:-${LOG_DIR}/run-$(date -u +%Y%m%dT%H%M%S).jsonl}"

mkdir -p "$LOG_DIR"

# IST timestamp = UTC + 5:30, computed manually per CLAUDE.md warning
# (Git Bash `TZ=Asia/Kolkata` silently fails on Windows).
ist_now() {
    python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%dT%H:%M:%S+05:30'))" 2>/dev/null || \
        date -u -Iseconds
}

# Emit a JSONL log line.
# Args: $1=test_id  $2=phase  $3=result (pass|fail|skip|observe)  $4=message  $5=(opt)evidence
log_emit() {
    local test_id="$1" phase="$2" result="$3" msg="$4" evidence="${5:-}"
    local ts; ts=$(ist_now)
    python3 -c "
import json, sys
d = {
    'ts': '$ts',
    'test_id': '$test_id',
    'phase': '$phase',
    'result': '$result',
    'msg': '''$msg''',
}
ev = '''$evidence'''
if ev: d['evidence'] = ev[:2000]
print(json.dumps(d))
" >> "$LOG_FILE"
}

# Print stdout + log.
report() {
    local test_id="$1" phase="$2" result="$3" msg="$4" evidence="${5:-}"
    echo "[${phase}] ${result}: ${msg}"
    log_emit "$test_id" "$phase" "$result" "$msg" "$evidence"
}

# ─── JWT acquisition (cached for 10 min to avoid lockout noise) ────────────

JWT_CACHE="${LOG_DIR}/.jwt.cache"
JWT_CACHE_TTL=600

get_jwt() {
    local now; now=$(date +%s)
    if [ -f "$JWT_CACHE" ]; then
        local mtime; mtime=$(stat -c %Y "$JWT_CACHE" 2>/dev/null || echo 0)
        if [ $((now - mtime)) -lt $JWT_CACHE_TTL ]; then
            cat "$JWT_CACHE"
            return 0
        fi
    fi
    local token
    token=$(curl -s --max-time 5 -X POST \
        -H "Content-Type: application/json" \
        -d "{\"pin\":\"${ADMIN_PIN}\"}" \
        "${SERVER_URL}/api/v1/auth/admin-login" 2>/dev/null \
        | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('token',''))" 2>/dev/null)
    if [ -z "$token" ]; then
        return 1
    fi
    echo "$token" > "$JWT_CACHE"
    echo "$token"
}

# ─── Pod command execution (server-mediated) ───────────────────────────────

# pod_exec <pod_id> <cmd> — returns JSON {stdout, stderr, success} from server.
# Writes JSON to a temp file to avoid shell-quoting hell.
pod_exec() {
    local pod_id="$1" cmd="$2"
    local token; token=$(get_jwt)
    if [ -z "$token" ]; then
        echo '{"stdout":"","stderr":"JWT-ACQUISITION-FAILED","success":false}'
        return 1
    fi
    local payload; payload=$(mktemp)
    python3 -c "import json; print(json.dumps({'cmd': '''$cmd'''}))" > "$payload"
    curl -s --max-time 15 -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${token}" \
        --data @"$payload" \
        "${SERVER_URL}/api/v1/pods/${pod_id}/exec" 2>/dev/null
    rm -f "$payload"
}

# ─── Process inspection on a pod ───────────────────────────────────────────

# pod_pid_of <pod_id> <process.exe> — echo first PID found or empty
pod_pid_of() {
    local pod_id="$1" proc="$2"
    local out; out=$(pod_exec "$pod_id" "tasklist /FI \\\"IMAGENAME eq ${proc}\\\" /FO CSV /NH")
    echo "$out" | python3 -c "
import sys, json, re
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    # CSV rows: \"name\",\"pid\",...
    m = re.search(r'^\"[^\"]+\",\"(\d+)\"', stdout, re.M)
    if m: print(m.group(1))
except Exception:
    pass
" 2>/dev/null
}

# pod_proc_count <pod_id> <process.exe> — echo integer count
pod_proc_count() {
    local pod_id="$1" proc="$2"
    local out; out=$(pod_exec "$pod_id" "tasklist /FI \\\"IMAGENAME eq ${proc}\\\" /FO CSV /NH")
    echo "$out" | python3 -c "
import sys, json, re
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    count = len(re.findall(r'^\"', stdout, re.M))
    print(count)
except Exception:
    print(0)
" 2>/dev/null || echo 0
}

# pod_window_titles <pod_id> — echo all visible-window titles (one per line)
pod_window_titles() {
    local pod_id="$1"
    local out; out=$(pod_exec "$pod_id" 'tasklist /V /FO CSV /NH')
    echo "$out" | python3 -c "
import sys, json, csv, io
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    reader = csv.reader(io.StringIO(stdout))
    for row in reader:
        if len(row) >= 9 and row[8] and row[8] != 'N/A':
            print(row[8])
except Exception:
    pass
" 2>/dev/null
}

# ─── Assertions ────────────────────────────────────────────────────────────
# All assertions log evidence + return 0/1. Caller aggregates into PASS/FAIL.

# assert_single_pid <pod_id> <process.exe> <test_id> <phase>
assert_single_pid() {
    local pod_id="$1" proc="$2" test_id="$3" phase="$4"
    local count; count=$(pod_proc_count "$pod_id" "$proc")
    if [ "$count" -eq 1 ]; then
        report "$test_id" "$phase" "pass" "expected 1 ${proc}, saw 1"
        return 0
    fi
    report "$test_id" "$phase" "fail" "expected 1 ${proc}, saw ${count}" "tasklist count=${count}"
    return 1
}

# assert_pid_stable <pod_id> <process.exe> <before_pid> <test_id> <phase>
assert_pid_stable() {
    local pod_id="$1" proc="$2" before="$3" test_id="$4" phase="$5"
    local after; after=$(pod_pid_of "$pod_id" "$proc")
    if [ "$before" = "$after" ] && [ -n "$after" ]; then
        report "$test_id" "$phase" "pass" "${proc} PID stable at ${after}"
        return 0
    fi
    report "$test_id" "$phase" "fail" "${proc} PID changed: ${before} -> ${after}" "before=${before} after=${after}"
    return 1
}

# assert_http_status <url> <expected_code> <test_id> <phase>
assert_http_status() {
    local url="$1" expected="$2" test_id="$3" phase="$4"
    local got; got=$(curl -s --max-time 5 -o /dev/null -w "%{http_code}" "$url")
    if [ "$got" = "$expected" ]; then
        report "$test_id" "$phase" "pass" "${url} returned ${got}"
        return 0
    fi
    report "$test_id" "$phase" "fail" "${url} expected ${expected}, got ${got}" "curl: ${url} -> ${got}"
    return 1
}

# ─── Test runner plumbing ──────────────────────────────────────────────────

# test_start <test_id> <description>
test_start() {
    local test_id="$1" desc="$2"
    echo ""
    echo "══════ ${test_id}: ${desc} ══════"
    report "$test_id" "start" "observe" "$desc"
}

# test_end <test_id> <final_result: pass|fail|skip>
test_end() {
    local test_id="$1" final="$2"
    report "$test_id" "end" "$final" "test complete"
}

# pod_preflight — verify target pod is reachable before running any test
pod_preflight() {
    local pod_id="$1"
    local health
    health=$(curl -s --max-time 3 "${SERVER_URL}/api/v1/fleet/health" 2>/dev/null \
        | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    for p in d.get('pods', []):
        if p.get('pod_id') == '$pod_id':
            print('reachable' if p.get('http_reachable') else 'unreachable')
            sys.exit(0)
    print('not_found')
except Exception:
    print('error')
" 2>/dev/null)
    [ "$health" = "reachable" ]
}
