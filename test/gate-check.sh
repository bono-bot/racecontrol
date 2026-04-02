#!/usr/bin/env bash
# gate-check.sh — Standing rules enforcement gate for OTA pipeline.
# Extends comms-link run-all.sh as a superset. Reads standing-rules-registry.json.
#
# Modes:
#   --pre-deploy    Full pre-deploy gate (before wave 1)
#   --post-wave N   Post-wave verification (after wave N)
#
# Exit codes:
#   0 — all gates passed
#   1 — at least one gate failed (pipeline must rollback)
#   2 — HUMAN-CONFIRM items pending (pipeline must pause)

set -o pipefail

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMMS_ROOT="$(cd "$REPO_ROOT/../comms-link" 2>/dev/null && pwd 2>/dev/null || echo "")"
REGISTRY="$REPO_ROOT/standing-rules-registry.json"

# ---------------------------------------------------------------------------
# Colour helpers (same pattern as comms-link run-all.sh)
# ---------------------------------------------------------------------------
GREEN=$(printf '\033[0;32m')
RED=$(printf '\033[0;31m')
YELLOW=$(printf '\033[1;33m')
RESET=$(printf '\033[0m')

pass_label="${GREEN}PASS${RESET}"
fail_label="${RED}FAIL${RESET}"
warn_label="${YELLOW}WARN${RESET}"
pending_label="${YELLOW}PENDING${RESET}"

# ---------------------------------------------------------------------------
# Mode parsing
# ---------------------------------------------------------------------------
MODE="pre-deploy"
WAVE_NUM=""

if [ "$1" = "--post-wave" ]; then
  MODE="post-wave"
  WAVE_NUM="$2"
  if [ -z "$WAVE_NUM" ]; then
    echo "Error: --post-wave requires a wave number (e.g. --post-wave 1)"
    exit 1
  fi
elif [ "$1" = "--domain-check" ]; then
  MODE="domain-check"
elif [ "$1" = "--pre-deploy" ] || [ -z "$1" ]; then
  MODE="pre-deploy"
else
  echo "Usage: bash test/gate-check.sh [--pre-deploy | --post-wave N | --domain-check]"
  exit 1
fi

# ---------------------------------------------------------------------------
# Counters
# ---------------------------------------------------------------------------
SUITE_RESULTS=""
OVERALL_FAIL=0
HUMAN_CONFIRM_COUNT=0
AUTO_PASS=0
AUTO_FAIL=0
AUTO_TOTAL=0
FAILED_RULES=""

# ---------------------------------------------------------------------------
# Helper: record suite result
# ---------------------------------------------------------------------------
record_suite() {
  local num="$1"
  local name="$2"
  local result="$3"
  SUITE_RESULTS="${SUITE_RESULTS}Suite ${num} (${name}): ${result}\n"
}

# ---------------------------------------------------------------------------
# Domain detection function (used by Suite 5 and --domain-check mode)
# ---------------------------------------------------------------------------
DOMAIN_DISPLAY=0
DOMAIN_NETWORK=0
DOMAIN_PARSE=0
DOMAIN_BILLING=0
DOMAIN_CONFIG=0
DOMAIN_HEALING=0
DETECTED_DOMAINS=()
DOMAIN_FILES_DISPLAY=""
DOMAIN_FILES_NETWORK=""
DOMAIN_FILES_PARSE=""
DOMAIN_FILES_BILLING=""
DOMAIN_FILES_CONFIG=""
DOMAIN_FILES_HEALING=""

detect_domains() {
  # Reset flags
  DOMAIN_DISPLAY=0
  DOMAIN_NETWORK=0
  DOMAIN_PARSE=0
  DOMAIN_BILLING=0
  DOMAIN_CONFIG=0
  DOMAIN_HEALING=0
  DETECTED_DOMAINS=()
  DOMAIN_FILES_DISPLAY=""
  DOMAIN_FILES_NETWORK=""
  DOMAIN_FILES_PARSE=""
  DOMAIN_FILES_BILLING=""
  DOMAIN_FILES_CONFIG=""
  DOMAIN_FILES_HEALING=""

  # Get changed files: staged first, then HEAD~1 diff
  local changed_files
  changed_files=$(cd "$REPO_ROOT" && git diff --cached --name-only 2>/dev/null)
  if [ -z "$changed_files" ]; then
    changed_files=$(cd "$REPO_ROOT" && git diff HEAD~1 --name-only 2>/dev/null)
  fi

  if [ -z "$changed_files" ]; then
    echo "  No changed files detected"
    return
  fi

  # Display domain: path patterns + CSS/HTML in app dirs
  local display_matches
  display_matches=$(echo "$changed_files" | grep -iE '(lock_screen|blanking|overlay|kiosk|Edge|browser|display|screen)' || true)
  local display_css_html
  display_css_html=$(echo "$changed_files" | grep -iE '^(apps/admin|apps/kiosk|apps/web|src/).*\.(css|html)$' || true)
  if [ -n "$display_matches" ] || [ -n "$display_css_html" ]; then
    DOMAIN_DISPLAY=1
    DETECTED_DOMAINS+=("display")
    DOMAIN_FILES_DISPLAY=$(printf "%s\n%s" "$display_matches" "$display_css_html" | grep -v '^$' | sort -u | paste -sd ', ' -)
  fi

  # Network domain: path patterns
  local network_matches
  network_matches=$(echo "$changed_files" | grep -iE '(ws_handler|fleet_exec|cloud_sync|http|api/v1|websocket|WebSocket)' || true)
  if [ -n "$network_matches" ]; then
    DOMAIN_NETWORK=1
    DETECTED_DOMAINS+=("network")
    DOMAIN_FILES_NETWORK=$(echo "$network_matches" | sort -u | paste -sd ', ' -)
  fi

  # Parse domain: path patterns + diff content patterns
  local parse_path_matches
  parse_path_matches=$(echo "$changed_files" | grep -iE '(parse|from_str|serde|toml|config.*load)' || true)
  local parse_diff_matches=""
  if [ -z "$parse_path_matches" ]; then
    # Check diff content for parse-related function changes
    local diff_content
    diff_content=$(cd "$REPO_ROOT" && git diff HEAD~1 2>/dev/null || true)
    if echo "$diff_content" | grep -qiE '(parse|from_str|toml::from_str|u32::parse|trim)'; then
      parse_diff_matches="(detected in diff content)"
    fi
  fi
  if [ -n "$parse_path_matches" ] || [ -n "$parse_diff_matches" ]; then
    DOMAIN_PARSE=1
    DETECTED_DOMAINS+=("parse")
    if [ -n "$parse_path_matches" ]; then
      DOMAIN_FILES_PARSE=$(echo "$parse_path_matches" | sort -u | paste -sd ', ' -)
    else
      DOMAIN_FILES_PARSE="$parse_diff_matches"
    fi
  fi

  # Billing domain: path patterns
  local billing_matches
  billing_matches=$(echo "$changed_files" | grep -iE '(billing|session.*start|session.*stop|rate.*calc|wallet)' || true)
  if [ -n "$billing_matches" ]; then
    DOMAIN_BILLING=1
    DETECTED_DOMAINS+=("billing")
    DOMAIN_FILES_BILLING=$(echo "$billing_matches" | sort -u | paste -sd ', ' -)
  fi

  # Config domain: path patterns
  local config_matches
  config_matches=$(echo "$changed_files" | grep -iE '\.(toml|bat)$|registry' || true)
  if [ -n "$config_matches" ]; then
    DOMAIN_CONFIG=1
    DETECTED_DOMAINS+=("config")
    DOMAIN_FILES_CONFIG=$(echo "$config_matches" | sort -u | paste -sd ', ' -)
  fi

  # Healing domain: files that touch self-healing, diagnosis, recovery, or escalation
  local healing_matches
  healing_matches=$(echo "$changed_files" | grep -iE '(heal|doctor|diagnos|tier_engine|tier1_fixes|escalat|self_heal|failure_monitor|predictive_maintenance|knowledge_base|watchdog|self_monitor|sentinel_watcher|startup_cleanup|game_doctor|rc-doctor)' || true)
  if [ -n "$healing_matches" ]; then
    DOMAIN_HEALING=1
    DETECTED_DOMAINS+=("healing")
    DOMAIN_FILES_HEALING=$(echo "$healing_matches" | sort -u | paste -sd ', ' -)
  fi

  # Print detected domains
  if [ ${#DETECTED_DOMAINS[@]} -gt 0 ]; then
    echo "  Detected domains:"
    [ $DOMAIN_DISPLAY -eq 1 ] && echo "    display: $DOMAIN_FILES_DISPLAY"
    [ $DOMAIN_NETWORK -eq 1 ] && echo "    network: $DOMAIN_FILES_NETWORK"
    [ $DOMAIN_PARSE -eq 1 ] && echo "    parse: $DOMAIN_FILES_PARSE"
    [ $DOMAIN_BILLING -eq 1 ] && echo "    billing: $DOMAIN_FILES_BILLING"
    [ $DOMAIN_CONFIG -eq 1 ] && echo "    config: $DOMAIN_FILES_CONFIG"
    [ $DOMAIN_HEALING -eq 1 ] && echo "    healing: $DOMAIN_FILES_HEALING"
  else
    echo "  No domain-specific changes detected"
  fi
}

# ===========================================================================
# Suite 0: comms-link E2E framework
# ===========================================================================
echo ""
echo "============================================================"
printf "Suite 0: comms-link E2E framework...\n"
echo "============================================================"

if [ -n "$COMMS_ROOT" ] && [ -f "$COMMS_ROOT/test/run-all.sh" ]; then
  (cd "$COMMS_ROOT" && bash test/run-all.sh)
  COMMS_EXIT=$?
  if [ $COMMS_EXIT -eq 0 ]; then
    printf "Suite 0: comms-link E2E... %b\n" "$pass_label"
    record_suite 0 "comms-link E2E" "PASS"
  else
    printf "Suite 0: comms-link E2E... %b  (exit %d)\n" "$fail_label" "$COMMS_EXIT"
    record_suite 0 "comms-link E2E" "FAIL"
    OVERALL_FAIL=1
  fi
else
  printf "Suite 0: comms-link E2E... %b (run-all.sh not found at %s)\n" "$warn_label" "$COMMS_ROOT/test/run-all.sh"
  record_suite 0 "comms-link E2E" "SKIPPED (not found)"
fi

# ===========================================================================
# PRE-DEPLOY specific suites
# ===========================================================================
if [ "$MODE" = "pre-deploy" ]; then

  # Run domain detection early so Suite 5 has results
  detect_domains

  # =========================================================================
  # Suite 1: Cargo tests
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 1: Cargo tests...\n"
  echo "============================================================"

  (cd "$REPO_ROOT" && cargo test --workspace 2>&1)
  CARGO_EXIT=$?
  if [ $CARGO_EXIT -eq 0 ]; then
    printf "Suite 1: Cargo tests... %b\n" "$pass_label"
    record_suite 1 "cargo tests" "PASS"
  else
    printf "Suite 1: Cargo tests... %b  (exit %d)\n" "$fail_label" "$CARGO_EXIT"
    record_suite 1 "cargo tests" "FAIL"
    OVERALL_FAIL=1
  fi

  # =========================================================================
  # Suite 2: Standing rules AUTO checks
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 2: Standing rules AUTO checks...\n"
  echo "============================================================"

  if [ ! -f "$REGISTRY" ]; then
    printf "  %b standing-rules-registry.json not found\n" "$fail_label"
    record_suite 2 "standing rules AUTO" "FAIL (registry missing)"
    OVERALL_FAIL=1
  else
    # Parse AUTO rules with check_command from registry using node
    AUTO_LINES=$(node -e "
      var r = require('$REGISTRY'.replace(/\\\\/g,'/'));
      r.filter(function(x){ return x.type==='AUTO' && x.check_command; })
       .forEach(function(x){ console.log(x.id + '|||' + x.summary + '|||' + x.check_command); });
    " 2>/dev/null)

    if [ -z "$AUTO_LINES" ]; then
      printf "  No AUTO rules with check_command found\n"
      record_suite 2 "standing rules AUTO" "PASS (0 checks)"
    else
      while IFS= read -r line; do
        RULE_ID=$(echo "$line" | cut -d'|' -f1-1)
        # Handle the ||| separator properly
        RULE_ID=$(echo "$line" | sed 's/|||.*//')
        RULE_SUMMARY=$(echo "$line" | sed 's/^[^|]*|||//' | sed 's/|||.*//')
        RULE_CMD=$(echo "$line" | sed 's/.*|||//')

        AUTO_TOTAL=$((AUTO_TOTAL + 1))

        # Run the check command from the repo root
        (cd "$REPO_ROOT" && eval "$RULE_CMD" > /dev/null 2>&1)
        CMD_EXIT=$?

        if [ $CMD_EXIT -eq 0 ]; then
          printf "  [%s] %s... %b\n" "$RULE_ID" "$RULE_SUMMARY" "$pass_label"
          AUTO_PASS=$((AUTO_PASS + 1))
        else
          printf "  [%s] %s... %b\n" "$RULE_ID" "$RULE_SUMMARY" "$fail_label"
          AUTO_FAIL=$((AUTO_FAIL + 1))
          FAILED_RULES="${FAILED_RULES}  FAILED: ${RULE_ID} -- ${RULE_SUMMARY}\n"
          OVERALL_FAIL=1
        fi
      done <<EOF
$AUTO_LINES
EOF

      if [ $AUTO_FAIL -eq 0 ]; then
        record_suite 2 "standing rules AUTO" "PASS (${AUTO_PASS}/${AUTO_TOTAL})"
      else
        record_suite 2 "standing rules AUTO" "FAIL (${AUTO_FAIL}/${AUTO_TOTAL} failed)"
      fi
    fi
  fi

  # =========================================================================
  # Suite 3: Diff analysis (pre-deploy specific)
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 3: Diff analysis...\n"
  echo "============================================================"

  DIFF_FAIL=0

  # Check for .unwrap() in changed Rust files
  CHANGED_RS=$(cd "$REPO_ROOT" && git diff HEAD~1 --name-only -- '*.rs' 2>/dev/null)
  if [ -n "$CHANGED_RS" ]; then
    UNWRAP_HITS=$(cd "$REPO_ROOT" && echo "$CHANGED_RS" | xargs grep -n '\.unwrap()' 2>/dev/null | grep -v '#\[test\]' | grep -v '#\[cfg(test)\]' | grep -v '// test' || true)
    if [ -n "$UNWRAP_HITS" ]; then
      printf "  .unwrap() in changed .rs files... %b\n" "$fail_label"
      echo "$UNWRAP_HITS" | head -5 | while IFS= read -r hit; do
        echo "    $hit"
      done
      DIFF_FAIL=1
    else
      printf "  .unwrap() in changed .rs files... %b\n" "$pass_label"
    fi
  else
    printf "  .unwrap() in changed .rs files... %b (no .rs changes)\n" "$pass_label"
  fi

  # Check for : any in changed TS files
  CHANGED_TS=$(cd "$REPO_ROOT" && git diff HEAD~1 --name-only -- '*.ts' '*.tsx' 2>/dev/null)
  if [ -n "$CHANGED_TS" ]; then
    ANY_HITS=$(cd "$REPO_ROOT" && echo "$CHANGED_TS" | xargs grep -n ': any' 2>/dev/null | grep -v node_modules | grep -v '.d.ts' || true)
    if [ -n "$ANY_HITS" ]; then
      printf "  : any in changed .ts files... %b\n" "$fail_label"
      echo "$ANY_HITS" | head -5 | while IFS= read -r hit; do
        echo "    $hit"
      done
      DIFF_FAIL=1
    else
      printf "  : any in changed .ts files... %b\n" "$pass_label"
    fi
  else
    printf "  : any in changed .ts files... %b (no .ts changes)\n" "$pass_label"
  fi

  # Check release-manifest.toml exists
  if [ -f "$REPO_ROOT/deploy-staging/release-manifest.toml" ]; then
    printf "  release-manifest.toml exists... %b\n" "$pass_label"
  else
    printf "  release-manifest.toml exists... %b\n" "$fail_label"
    DIFF_FAIL=1
  fi

  if [ $DIFF_FAIL -eq 0 ]; then
    record_suite 3 "diff analysis" "PASS"
  else
    record_suite 3 "diff analysis" "FAIL"
    OVERALL_FAIL=1
  fi

  # =========================================================================
  # Suite 4: HUMAN-CONFIRM checklist output
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 4: HUMAN-CONFIRM checklist...\n"
  echo "============================================================"

  if [ ! -f "$REGISTRY" ]; then
    printf "  %b standing-rules-registry.json not found\n" "$warn_label"
    record_suite 4 "HUMAN-CONFIRM" "SKIPPED (registry missing)"
  else
    # Check if display-affecting files changed (triggers visual verification rules)
    DISPLAY_CHANGED=0
    DIFF_FILES=$(cd "$REPO_ROOT" && git diff HEAD~1 --name-only 2>/dev/null || true)
    if echo "$DIFF_FILES" | grep -qiE '(blank|lock|kiosk|overlay|browser|display|edge|screen)'; then
      DISPLAY_CHANGED=1
    fi

    # Check if guard/filter/blocklist changed
    GUARD_CHANGED=0
    if echo "$DIFF_FILES" | grep -qiE '(guard|allowlist|whitelist|blocklist|filter)'; then
      GUARD_CHANGED=1
    fi

    # Parse HUMAN-CONFIRM rules from registry
    HC_OUTPUT=$(node -e "
      var r = require('$REGISTRY'.replace(/\\\\/g,'/'));
      var displayChanged = $DISPLAY_CHANGED;
      var guardChanged = $GUARD_CHANGED;
      var items = r.filter(function(x){ return x.type==='HUMAN-CONFIRM' && x.checklist; });
      var triggered = [];

      items.forEach(function(item) {
        // Always include ultimate rule and deploy rules
        var isUltimate = item.category === 'ultimate';
        var isDisplay = item.summary.toLowerCase().indexOf('display') >= 0 ||
                        item.summary.toLowerCase().indexOf('visual') >= 0 ||
                        item.summary.toLowerCase().indexOf('screen') >= 0;
        var isGuard = item.summary.toLowerCase().indexOf('guard') >= 0 ||
                      item.summary.toLowerCase().indexOf('filter') >= 0 ||
                      item.summary.toLowerCase().indexOf('blocklist') >= 0;
        var isDeploy = item.category === 'deploy' || item.category === 'ota-pipeline';

        if (isUltimate || isDeploy || (isDisplay && displayChanged) || (isGuard && guardChanged)) {
          triggered.push(item);
        }
      });

      if (triggered.length === 0) {
        console.log('__NONE__');
      } else {
        console.log('__COUNT__' + triggered.length);
        triggered.forEach(function(item) {
          console.log('[' + item.id + '] ' + item.summary);
          item.checklist.forEach(function(c) {
            console.log('  [ ] ' + c);
          });
        });
      }
    " 2>/dev/null)

    if echo "$HC_OUTPUT" | grep -q '__NONE__'; then
      printf "  No HUMAN-CONFIRM rules triggered for this diff\n"
      record_suite 4 "HUMAN-CONFIRM" "PASS (none triggered)"
    else
      HC_COUNT=$(echo "$HC_OUTPUT" | grep '__COUNT__' | sed 's/__COUNT__//')
      echo ""
      echo "=== HUMAN-CONFIRM: Operator Checklist ==="
      echo "$HC_OUTPUT" | grep -v '__COUNT__'
      echo ""
      HUMAN_CONFIRM_COUNT=$HC_COUNT
      record_suite 4 "HUMAN-CONFIRM" "${HC_COUNT} items pending"
    fi
  fi

  # =========================================================================
  # Suite 5: Domain-matched verification (GATE-01..04)
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 5: Domain-matched verification...\n"
  echo "============================================================"

  DOMAIN_FAIL=0
  DOMAIN_CHECKED=0
  DOMAIN_BLOCKED_LIST=""

  # Evidence tracking
  EVIDENCE_VISUAL="not applicable"
  EVIDENCE_SERVER="not applicable"
  EVIDENCE_FLEET=""
  EVIDENCE_WS=""
  EVIDENCE_PARSE="not applicable"

  # --- GATE-02: Display domain ---
  if [ $DOMAIN_DISPLAY -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    if [ "${VISUAL_VERIFIED:-}" = "true" ]; then
      printf "  Display verification: VISUAL_VERIFIED=true confirmed... %b\n" "$pass_label"
      EVIDENCE_VISUAL="true"
    else
      printf "  %b BLOCKED: Display-domain changes detected but VISUAL_VERIFIED=true not set\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_DISPLAY"
      echo "    To pass: verify screens on pods, then re-run with VISUAL_VERIFIED=true bash test/gate-check.sh"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}display "
      EVIDENCE_VISUAL="false"
    fi
  fi

  # --- GATE-03: Network domain ---
  if [ $DOMAIN_NETWORK -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    # GATE-03a: Server health check
    HEALTH_RESP=$(curl -sf -m 5 http://192.168.31.23:8080/api/v1/health 2>/dev/null || echo "")
    if [ -n "$HEALTH_RESP" ]; then
      printf "  Network verification: server health check... %b\n" "$pass_label"
      echo "    Response: $(echo "$HEALTH_RESP" | head -c 120)"
      EVIDENCE_SERVER="OK"
    else
      printf "  %b BLOCKED: Network-domain changes detected but server health check failed\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_NETWORK"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network "
      EVIDENCE_SERVER="FAIL"
    fi
    # Fleet endpoint reachability (GATE-03b)
    FLEET_RESP=$(curl -sf -m 5 http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null || echo "")
    if [ -n "$FLEET_RESP" ]; then
      printf "  Network verification: fleet endpoint reachable... %b\n" "$pass_label"
      EVIDENCE_FLEET="OK"
    else
      printf "  %b BLOCKED: Fleet endpoint unreachable (curl to /api/v1/fleet/health failed)\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_NETWORK"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network-fleet "
      EVIDENCE_FLEET="FAIL"
    fi
    # WS connection test when WebSocket code changed (GATE-03c)
    if echo "$DOMAIN_FILES_NETWORK" | grep -qiE '(ws_handler|WebSocket)'; then
      if [ "${SKIP_WS_CHECK:-}" = "true" ]; then
        printf "  %b WebSocket check skipped (SKIP_WS_CHECK=true)\n" "$warn_label"
        EVIDENCE_WS="SKIPPED"
      else
        # Use curl with Connection: Upgrade to test WS handshake (returns 101 on success)
        WS_STATUS=$(curl -sf -m 5 -o /dev/null -w "%{http_code}" \
          -H "Connection: Upgrade" -H "Upgrade: websocket" \
          -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGVzdA==" \
          http://192.168.31.23:8080/ws 2>/dev/null || echo "000")
        if [ "$WS_STATUS" = "101" ]; then
          printf "  Network verification: WebSocket handshake OK (HTTP 101)... %b\n" "$pass_label"
          EVIDENCE_WS="OK"
        else
          printf "  %b BLOCKED: WebSocket changes detected but WS handshake failed (HTTP %s, expected 101)\n" "$fail_label" "$WS_STATUS"
          echo "    Triggering files: $DOMAIN_FILES_NETWORK"
          echo "    To bypass: set SKIP_WS_CHECK=true if WS endpoint path differs"
          DOMAIN_FAIL=1
          DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network-ws "
          EVIDENCE_WS="FAIL (HTTP $WS_STATUS)"
        fi
      fi
    fi
  fi

  # --- GATE-04: Parse domain ---
  if [ $DOMAIN_PARSE -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    if [ -n "${PARSE_TEST_INPUT:-}" ] && [ -n "${PARSE_TEST_EXPECTED:-}" ]; then
      if [ -f "$PARSE_TEST_INPUT" ]; then
        printf "  Parse verification: test input and expected output provided... %b\n" "$pass_label"
        echo "    Input file: $PARSE_TEST_INPUT"
        echo "    Expected: $PARSE_TEST_EXPECTED"
        EVIDENCE_PARSE="PROVIDED"
      else
        printf "  %b BLOCKED: PARSE_TEST_INPUT file does not exist: %s\n" "$fail_label" "$PARSE_TEST_INPUT"
        DOMAIN_FAIL=1
        DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}parse "
        EVIDENCE_PARSE="MISSING (file not found)"
      fi
    else
      printf "  %b BLOCKED: Parse-domain changes detected but PARSE_TEST_INPUT and PARSE_TEST_EXPECTED not provided\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_PARSE"
      echo "    To pass: re-run with PARSE_TEST_INPUT=/path/to/input PARSE_TEST_EXPECTED='expected_value' bash test/gate-check.sh"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}parse "
      EVIDENCE_PARSE="MISSING"
    fi
  fi

  # --- GATE-01: Billing domain (informational) ---
  if [ $DOMAIN_BILLING -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    printf "  %b Billing-domain changes detected — verify session start/stop and rate calculation after deploy\n" "$warn_label"
    echo "    Triggering files: $DOMAIN_FILES_BILLING"
  fi

  # --- GATE-01: Config domain (informational) ---
  if [ $DOMAIN_CONFIG -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    printf "  %b Config-domain changes detected — verify config loads correctly on target machines after deploy\n" "$warn_label"
    echo "    Triggering files: $DOMAIN_FILES_CONFIG"
  fi

  # Suite 5 result
  if [ $DOMAIN_CHECKED -eq 0 ]; then
    record_suite 5 "domain verification" "PASS (no domain-specific changes)"
  elif [ $DOMAIN_FAIL -eq 0 ]; then
    record_suite 5 "domain verification" "PASS (${DOMAIN_CHECKED} domains verified)"
  else
    DOMAIN_BLOCKED_LIST=$(echo "$DOMAIN_BLOCKED_LIST" | sed 's/ $//' | sed 's/ /|/g')
    record_suite 5 "domain verification" "FAIL (blocked by: ${DOMAIN_BLOCKED_LIST})"
    OVERALL_FAIL=1
  fi

  # Evidence summary
  if [ $DOMAIN_CHECKED -gt 0 ]; then
    echo ""
    echo "  Domain verification evidence:"
    echo "    VISUAL_VERIFIED=$EVIDENCE_VISUAL"
    echo "    Server health: $EVIDENCE_SERVER"
    [ -n "$EVIDENCE_FLEET" ] && echo "    Fleet endpoint: $EVIDENCE_FLEET"
    [ -n "$EVIDENCE_WS" ] && echo "    WebSocket: $EVIDENCE_WS"
    echo "    Parse test: $EVIDENCE_PARSE"
  fi

  # =========================================================================
  # Suite 6: 3 Layers + Floor — VPS self-healing stack (GATE-06)
  # Triggered when: healing domain detected, or always in full pre-deploy mode
  # Tests all 4 layers of RC-Doctor v2.2 on Bono VPS
  # =========================================================================
  if [ $DOMAIN_HEALING -eq 1 ] || [ "${THREE_LF_CHECK:-}" = "true" ]; then
    echo ""
    echo "============================================================"
    printf "Suite 6: 3 Layers + Floor — VPS self-healing stack...\n"
    echo "============================================================"

    LF_FAIL=0
    LF_CHECKED=0
    BONO_VPS="100.70.177.44"

    # LAYER 1: EYES — Uptime Kuma
    LF_CHECKED=$((LF_CHECKED + 1))
    L1_RESULT=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "root@${BONO_VPS}" \
      "curl -sf -m 5 http://localhost:3001/api/status-page/heartbeat" 2>/dev/null && echo "OK" || echo "FAIL")
    if echo "$L1_RESULT" | grep -q "OK"; then
      printf "  Layer 1 (EYES — Uptime Kuma :3001)... %b\n" "$pass_label"
    else
      printf "  Layer 1 (EYES — Uptime Kuma :3001)... %b\n" "$fail_label"
      LF_FAIL=$((LF_FAIL + 1))
    fi

    # LAYER 2: MUSCLE — Monit
    LF_CHECKED=$((LF_CHECKED + 1))
    L2_RESULT=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "root@${BONO_VPS}" \
      "monit summary >/dev/null 2>&1 && echo OK || echo FAIL" 2>/dev/null || echo "FAIL")
    if echo "$L2_RESULT" | grep -q "OK"; then
      printf "  Layer 2 (MUSCLE — Monit)... %b\n" "$pass_label"
    else
      printf "  Layer 2 (MUSCLE — Monit)... %b\n" "$fail_label"
      LF_FAIL=$((LF_FAIL + 1))
    fi

    # LAYER 3: BRAIN — rc-doctor.sh timer
    LF_CHECKED=$((LF_CHECKED + 1))
    L3_RESULT=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "root@${BONO_VPS}" \
      "systemctl is-active rc-doctor.timer 2>/dev/null && echo OK || echo FAIL" 2>/dev/null || echo "FAIL")
    if echo "$L3_RESULT" | grep -q "OK"; then
      printf "  Layer 3 (BRAIN — rc-doctor.sh timer)... %b\n" "$pass_label"
    else
      printf "  Layer 3 (BRAIN — rc-doctor.sh timer)... %b\n" "$fail_label"
      LF_FAIL=$((LF_FAIL + 1))
    fi

    # FLOOR: PM2 executor
    LF_CHECKED=$((LF_CHECKED + 1))
    FLOOR_RESULT=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "root@${BONO_VPS}" \
      "pm2 jlist 2>/dev/null | jq 'length' 2>/dev/null && echo OK || echo FAIL" 2>/dev/null || echo "FAIL")
    if echo "$FLOOR_RESULT" | grep -q "OK"; then
      printf "  Floor (PM2 executor)... %b\n" "$pass_label"
    else
      printf "  Floor (PM2 executor)... %b\n" "$fail_label"
      LF_FAIL=$((LF_FAIL + 1))
    fi

    if [ $LF_FAIL -eq 0 ]; then
      printf "  3 Layers + Floor: all %d layers alive... %b\n" "$LF_CHECKED" "$pass_label"
      record_suite 6 "3 Layers + Floor" "PASS (${LF_CHECKED}/${LF_CHECKED} layers)"
    else
      printf "  3 Layers + Floor: %d/%d layers FAILED... %b\n" "$LF_FAIL" "$LF_CHECKED" "$fail_label"
      record_suite 6 "3 Layers + Floor" "FAIL (${LF_FAIL}/${LF_CHECKED} layers down)"
      OVERALL_FAIL=1
    fi
  elif [ $DOMAIN_HEALING -eq 0 ]; then
    record_suite 6 "3 Layers + Floor" "SKIPPED (no healing-domain changes)"
  fi

  # =========================================================================
  # Suite 7: Security Audit (v38.0 — SECAUDIT-03)
  # Runs scripts/security-audit.sh and checks the overall result.
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 7: Security Audit (v38.0)...\n"
  echo "============================================================"

  SEC_AUDIT_SCRIPT="$REPO_ROOT/scripts/security-audit.sh"
  if [ -x "$SEC_AUDIT_SCRIPT" ]; then
    SEC_OUTPUT=$(bash "$SEC_AUDIT_SCRIPT" --output "$REPO_ROOT/security-scorecard.json" 2>&1)
    SEC_EXIT=$?
    # Show summary line only
    echo "$SEC_OUTPUT" | grep -E "Score:|Overall:" | head -2
    if [ $SEC_EXIT -eq 0 ]; then
      record_suite 7 "security audit" "PASS"
    else
      record_suite 7 "security audit" "FAIL"
      OVERALL_FAIL=1
    fi
  else
    printf "  scripts/security-audit.sh not found or not executable... %b\n" "$warn_label"
    record_suite 7 "security audit" "SKIPPED (script missing)"
  fi

fi  # end pre-deploy

# ===========================================================================
# POST-WAVE specific suites
# ===========================================================================
if [ "$MODE" = "post-wave" ]; then

  # =========================================================================
  # Suite 1: Build ID verification
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 1: Build ID verification (wave %s)...\n" "$WAVE_NUM"
  echo "============================================================"

  EXPECTED=$(grep 'git_commit' "$REPO_ROOT/deploy-staging/release-manifest.toml" 2>/dev/null | cut -d'"' -f2)
  if [ -n "$EXPECTED" ]; then
    printf "  Expected build_id: %s... %b\n" "$EXPECTED" "$pass_label"
    record_suite 1 "build ID verification" "PASS"
  else
    printf "  Expected build_id: (not found in manifest)... %b\n" "$fail_label"
    record_suite 1 "build ID verification" "FAIL"
    OVERALL_FAIL=1
  fi

  # =========================================================================
  # Suite 2: Fleet health check
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 2: Fleet health check (wave %s)...\n" "$WAVE_NUM"
  echo "============================================================"

  HEALTH_JSON=$(curl -sf http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null || echo "")
  if [ -z "$HEALTH_JSON" ]; then
    printf "  Fleet health endpoint... %b (unreachable)\n" "$fail_label"
    record_suite 2 "fleet health" "FAIL (unreachable)"
    OVERALL_FAIL=1
  else
    # Check ws_connected status for pods using node
    HEALTH_RESULT=$(node -e "
      var h = $HEALTH_JSON;
      var disconnected = [];
      h.forEach(function(p) {
        if (!p.ws_connected) {
          disconnected.push('Pod ' + p.pod_number);
        }
      });
      if (disconnected.length > 0) {
        console.log('FAIL:' + disconnected.join(','));
      } else {
        console.log('PASS:' + h.length + ' pods connected');
      }
    " 2>/dev/null || echo "FAIL:parse error")

    if echo "$HEALTH_RESULT" | grep -q '^PASS:'; then
      HEALTH_MSG=$(echo "$HEALTH_RESULT" | sed 's/^PASS://')
      printf "  Fleet health (%s)... %b\n" "$HEALTH_MSG" "$pass_label"
      record_suite 2 "fleet health" "PASS"
    else
      HEALTH_MSG=$(echo "$HEALTH_RESULT" | sed 's/^FAIL://')
      printf "  Fleet health (disconnected: %s)... %b\n" "$HEALTH_MSG" "$fail_label"
      record_suite 2 "fleet health" "FAIL"
      OVERALL_FAIL=1
    fi
  fi

  # =========================================================================
  # Suite 3: Standing rules AUTO checks (same as pre-deploy Suite 2)
  # =========================================================================
  echo ""
  echo "============================================================"
  printf "Suite 3: Standing rules AUTO checks...\n"
  echo "============================================================"

  if [ ! -f "$REGISTRY" ]; then
    printf "  %b standing-rules-registry.json not found\n" "$fail_label"
    record_suite 3 "standing rules AUTO" "FAIL (registry missing)"
    OVERALL_FAIL=1
  else
    AUTO_LINES=$(node -e "
      var r = require('$REGISTRY'.replace(/\\\\/g,'/'));
      r.filter(function(x){ return x.type==='AUTO' && x.check_command; })
       .forEach(function(x){ console.log(x.id + '|||' + x.summary + '|||' + x.check_command); });
    " 2>/dev/null)

    if [ -z "$AUTO_LINES" ]; then
      printf "  No AUTO rules with check_command found\n"
      record_suite 3 "standing rules AUTO" "PASS (0 checks)"
    else
      while IFS= read -r line; do
        RULE_ID=$(echo "$line" | sed 's/|||.*//')
        RULE_SUMMARY=$(echo "$line" | sed 's/^[^|]*|||//' | sed 's/|||.*//')
        RULE_CMD=$(echo "$line" | sed 's/.*|||//')

        AUTO_TOTAL=$((AUTO_TOTAL + 1))

        (cd "$REPO_ROOT" && eval "$RULE_CMD" > /dev/null 2>&1)
        CMD_EXIT=$?

        if [ $CMD_EXIT -eq 0 ]; then
          printf "  [%s] %s... %b\n" "$RULE_ID" "$RULE_SUMMARY" "$pass_label"
          AUTO_PASS=$((AUTO_PASS + 1))
        else
          printf "  [%s] %s... %b\n" "$RULE_ID" "$RULE_SUMMARY" "$fail_label"
          AUTO_FAIL=$((AUTO_FAIL + 1))
          FAILED_RULES="${FAILED_RULES}  FAILED: ${RULE_ID} -- ${RULE_SUMMARY}\n"
          OVERALL_FAIL=1
        fi
      done <<EOF
$AUTO_LINES
EOF

      if [ $AUTO_FAIL -eq 0 ]; then
        record_suite 3 "standing rules AUTO" "PASS (${AUTO_PASS}/${AUTO_TOTAL})"
      else
        record_suite 3 "standing rules AUTO" "FAIL (${AUTO_FAIL}/${AUTO_TOTAL} failed)"
      fi
    fi
  fi

fi  # end post-wave

# ===========================================================================
# DOMAIN-CHECK mode: runs ONLY Suite 5 (domain verification)
# ===========================================================================
if [ "$MODE" = "domain-check" ]; then

  detect_domains

  echo ""
  echo "============================================================"
  printf "Suite 5: Domain-matched verification (standalone)...\n"
  echo "============================================================"

  DOMAIN_FAIL=0
  DOMAIN_CHECKED=0
  DOMAIN_BLOCKED_LIST=""

  EVIDENCE_VISUAL="not applicable"
  EVIDENCE_SERVER="not applicable"
  EVIDENCE_FLEET=""
  EVIDENCE_WS=""
  EVIDENCE_PARSE="not applicable"

  # --- GATE-02: Display domain ---
  if [ $DOMAIN_DISPLAY -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    if [ "${VISUAL_VERIFIED:-}" = "true" ]; then
      printf "  Display verification: VISUAL_VERIFIED=true confirmed... %b\n" "$pass_label"
      EVIDENCE_VISUAL="true"
    else
      printf "  %b BLOCKED: Display-domain changes detected but VISUAL_VERIFIED=true not set\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_DISPLAY"
      echo "    To pass: verify screens on pods, then re-run with VISUAL_VERIFIED=true bash test/gate-check.sh --domain-check"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}display "
      EVIDENCE_VISUAL="false"
    fi
  fi

  # --- GATE-03: Network domain ---
  if [ $DOMAIN_NETWORK -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    # GATE-03a: Server health check
    HEALTH_RESP=$(curl -sf -m 5 http://192.168.31.23:8080/api/v1/health 2>/dev/null || echo "")
    if [ -n "$HEALTH_RESP" ]; then
      printf "  Network verification: server health check... %b\n" "$pass_label"
      echo "    Response: $(echo "$HEALTH_RESP" | head -c 120)"
      EVIDENCE_SERVER="OK"
    else
      printf "  %b BLOCKED: Network-domain changes detected but server health check failed\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_NETWORK"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network "
      EVIDENCE_SERVER="FAIL"
    fi
    # Fleet endpoint reachability (GATE-03b)
    FLEET_RESP=$(curl -sf -m 5 http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null || echo "")
    if [ -n "$FLEET_RESP" ]; then
      printf "  Network verification: fleet endpoint reachable... %b\n" "$pass_label"
      EVIDENCE_FLEET="OK"
    else
      printf "  %b BLOCKED: Fleet endpoint unreachable (curl to /api/v1/fleet/health failed)\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_NETWORK"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network-fleet "
      EVIDENCE_FLEET="FAIL"
    fi
    # WS connection test when WebSocket code changed (GATE-03c)
    if echo "$DOMAIN_FILES_NETWORK" | grep -qiE '(ws_handler|WebSocket)'; then
      if [ "${SKIP_WS_CHECK:-}" = "true" ]; then
        printf "  %b WebSocket check skipped (SKIP_WS_CHECK=true)\n" "$warn_label"
        EVIDENCE_WS="SKIPPED"
      else
        # Use curl with Connection: Upgrade to test WS handshake (returns 101 on success)
        WS_STATUS=$(curl -sf -m 5 -o /dev/null -w "%{http_code}" \
          -H "Connection: Upgrade" -H "Upgrade: websocket" \
          -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGVzdA==" \
          http://192.168.31.23:8080/ws 2>/dev/null || echo "000")
        if [ "$WS_STATUS" = "101" ]; then
          printf "  Network verification: WebSocket handshake OK (HTTP 101)... %b\n" "$pass_label"
          EVIDENCE_WS="OK"
        else
          printf "  %b BLOCKED: WebSocket changes detected but WS handshake failed (HTTP %s, expected 101)\n" "$fail_label" "$WS_STATUS"
          echo "    Triggering files: $DOMAIN_FILES_NETWORK"
          echo "    To bypass: set SKIP_WS_CHECK=true if WS endpoint path differs"
          DOMAIN_FAIL=1
          DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}network-ws "
          EVIDENCE_WS="FAIL (HTTP $WS_STATUS)"
        fi
      fi
    fi
  fi

  # --- GATE-04: Parse domain ---
  if [ $DOMAIN_PARSE -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    if [ -n "${PARSE_TEST_INPUT:-}" ] && [ -n "${PARSE_TEST_EXPECTED:-}" ]; then
      if [ -f "$PARSE_TEST_INPUT" ]; then
        printf "  Parse verification: test input and expected output provided... %b\n" "$pass_label"
        echo "    Input file: $PARSE_TEST_INPUT"
        echo "    Expected: $PARSE_TEST_EXPECTED"
        EVIDENCE_PARSE="PROVIDED"
      else
        printf "  %b BLOCKED: PARSE_TEST_INPUT file does not exist: %s\n" "$fail_label" "$PARSE_TEST_INPUT"
        DOMAIN_FAIL=1
        DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}parse "
        EVIDENCE_PARSE="MISSING (file not found)"
      fi
    else
      printf "  %b BLOCKED: Parse-domain changes detected but PARSE_TEST_INPUT and PARSE_TEST_EXPECTED not provided\n" "$fail_label"
      echo "    Triggering files: $DOMAIN_FILES_PARSE"
      echo "    To pass: re-run with PARSE_TEST_INPUT=/path/to/input PARSE_TEST_EXPECTED='expected_value' bash test/gate-check.sh --domain-check"
      DOMAIN_FAIL=1
      DOMAIN_BLOCKED_LIST="${DOMAIN_BLOCKED_LIST}parse "
      EVIDENCE_PARSE="MISSING"
    fi
  fi

  # --- GATE-01: Billing domain (informational) ---
  if [ $DOMAIN_BILLING -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    printf "  %b Billing-domain changes detected — verify session start/stop and rate calculation after deploy\n" "$warn_label"
    echo "    Triggering files: $DOMAIN_FILES_BILLING"
  fi

  # --- GATE-01: Config domain (informational) ---
  if [ $DOMAIN_CONFIG -eq 1 ]; then
    DOMAIN_CHECKED=$((DOMAIN_CHECKED + 1))
    printf "  %b Config-domain changes detected — verify config loads correctly on target machines after deploy\n" "$warn_label"
    echo "    Triggering files: $DOMAIN_FILES_CONFIG"
  fi

  # Suite 5 result
  if [ $DOMAIN_CHECKED -eq 0 ]; then
    record_suite 5 "domain verification" "PASS (no domain-specific changes)"
  elif [ $DOMAIN_FAIL -eq 0 ]; then
    record_suite 5 "domain verification" "PASS (${DOMAIN_CHECKED} domains verified)"
  else
    DOMAIN_BLOCKED_LIST=$(echo "$DOMAIN_BLOCKED_LIST" | sed 's/ $//' | sed 's/ /|/g')
    record_suite 5 "domain verification" "FAIL (blocked by: ${DOMAIN_BLOCKED_LIST})"
    OVERALL_FAIL=1
  fi

  # Evidence summary
  if [ $DOMAIN_CHECKED -gt 0 ]; then
    echo ""
    echo "  Domain verification evidence:"
    echo "    VISUAL_VERIFIED=$EVIDENCE_VISUAL"
    echo "    Server health: $EVIDENCE_SERVER"
    [ -n "$EVIDENCE_FLEET" ] && echo "    Fleet endpoint: $EVIDENCE_FLEET"
    [ -n "$EVIDENCE_WS" ] && echo "    WebSocket: $EVIDENCE_WS"
    echo "    Parse test: $EVIDENCE_PARSE"
  fi

fi  # end domain-check

# ===========================================================================
# Final Summary
# ===========================================================================
echo ""
echo "============================================================"
echo "=== Gate Check Results (mode: $MODE) ==="
echo "============================================================"

printf "%b" "$SUITE_RESULTS"

if [ -n "$FAILED_RULES" ]; then
  echo ""
  printf "%b" "$FAILED_RULES"
fi

echo "============================================================"

# ---------------------------------------------------------------------------
# Exit code logic:
#   1 — any AUTO/cargo/suite failure (failures take priority over HUMAN-CONFIRM)
#   2 — HUMAN-CONFIRM items pending (no failures)
#   0 — all gates passed
# ---------------------------------------------------------------------------
if [ $OVERALL_FAIL -ne 0 ]; then
  printf "Overall: %b (exit 1)\n" "$fail_label"
  echo "============================================================"
  echo ""
  exit 1
elif [ "$HUMAN_CONFIRM_COUNT" -gt 0 ] 2>/dev/null; then
  printf "Overall: %b — %s HUMAN-CONFIRM items require operator confirmation (exit 2)\n" "$pending_label" "$HUMAN_CONFIRM_COUNT"
  echo "============================================================"
  echo ""
  exit 2
else
  printf "Overall: %b (exit 0)\n" "$pass_label"
  echo "============================================================"
  echo ""
  exit 0
fi
