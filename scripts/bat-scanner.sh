#!/usr/bin/env bash
# scripts/bat-scanner.sh -- Bat file drift detection + syntax validation for all 8 pods
# chmod +x scripts/bat-scanner.sh
#
# Usage:
#   ./bat-scanner.sh              # scan all 8 pods
#   ./bat-scanner.sh --all        # scan all 8 pods
#   ./bat-scanner.sh --pod 3      # scan single pod
#   ./bat-scanner.sh --validate FILE  # syntax validation only on a local file
#   ./bat-scanner.sh --json       # output results as JSON
#   ./bat-scanner.sh --help       # usage
#
# Audit integration: source this file and call bat_scan_pod, bat_scan_all, bat_validate_syntax directly.

set -u
set -o pipefail
# NO set -e -- errors reported via output, not exit codes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Pod IP map (same as deploy-pod.sh) ---
pod_ip() {
  case "$1" in
    1) echo "192.168.31.89" ;; 2) echo "192.168.31.33" ;;
    3) echo "192.168.31.28" ;; 4) echo "192.168.31.88" ;;
    5) echo "192.168.31.86" ;; 6) echo "192.168.31.87" ;;
    7) echo "192.168.31.38" ;; 8) echo "192.168.31.91" ;;
    *) echo "" ;;
  esac
}

SENTRY_PORT=8091

# --- Canonical bat file paths ---
CANONICAL_RCAGENT="$REPO_ROOT/scripts/deploy/start-rcagent.bat"
CANONICAL_RCSENTRY="$REPO_ROOT/scripts/deploy/start-rcsentry.bat"

# --- Color codes ---
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# --- Temp directory ---
BAT_SCAN_TMPDIR="${TMPDIR:-/tmp}/bat-scanner-$$"
mkdir -p "$BAT_SCAN_TMPDIR"
_bat_scanner_cleanup() { rm -rf "$BAT_SCAN_TMPDIR" 2>/dev/null; }
# Only set trap if running standalone (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  trap _bat_scanner_cleanup EXIT
fi

# =============================================================================
# FUNCTION: bat_validate_syntax(file_path, label)
#
# Validates a bat file for 5 known anti-patterns that have caused production failures.
# Returns: number of violations found. Prints each violation to stdout.
# =============================================================================
bat_validate_syntax() {
  local file_path="$1"
  local label="${2:-$(basename "$file_path")}"
  local violations=0

  if [[ ! -f "$file_path" ]]; then
    echo "ERROR: File not found: $file_path"
    return 1
  fi

  # Check 1: UTF-8 BOM
  # Standing rule: .bat files must be clean ASCII + CRLF. BOM breaks cmd.exe.
  if command -v xxd >/dev/null 2>&1; then
    if xxd -l 3 "$file_path" 2>/dev/null | grep -qi 'efbb bf'; then
      echo "  [$label] Line 1: UTF-8 BOM detected -- remove first 3 bytes (sed '1s/^\xef\xbb\xbf//' or re-create without BOM)"
      violations=$((violations + 1))
    fi
  else
    # Fallback: check first 3 bytes with od
    local first3
    first3=$(od -A n -t x1 -N 3 "$file_path" 2>/dev/null | tr -d ' ')
    if [[ "$first3" == "efbbbf" ]]; then
      echo "  [$label] Line 1: UTF-8 BOM detected -- remove first 3 bytes"
      violations=$((violations + 1))
    fi
  fi

  # Check 2: Parentheses in if/else blocks
  # Standing rule: .bat files NEVER use parentheses in if/else -- use goto labels.
  local paren_lines
  paren_lines=$(grep -nP '^\s*if\s.*\(' "$file_path" 2>/dev/null || true)
  if [[ -n "$paren_lines" ]]; then
    while IFS= read -r line; do
      local linenum="${line%%:*}"
      echo "  [$label] Line $linenum: parentheses in if/else block -- use goto labels instead (standing rule)"
      violations=$((violations + 1))
    done <<< "$paren_lines"
  fi
  local paren_else
  paren_else=$(grep -nP '^\s*\)\s*else\s*\(' "$file_path" 2>/dev/null || true)
  if [[ -n "$paren_else" ]]; then
    while IFS= read -r line; do
      local linenum="${line%%:*}"
      echo "  [$label] Line $linenum: parentheses in else block -- use goto labels instead (standing rule)"
      violations=$((violations + 1))
    done <<< "$paren_else"
  fi

  # Check 3: /dev/null redirection (Unix-ism in bat file)
  local devnull_lines
  devnull_lines=$(grep -n '/dev/null' "$file_path" 2>/dev/null || true)
  if [[ -n "$devnull_lines" ]]; then
    while IFS= read -r line; do
      local linenum="${line%%:*}"
      echo "  [$label] Line $linenum: /dev/null is Unix -- use NUL or >nul on Windows"
      violations=$((violations + 1))
    done <<< "$devnull_lines"
  fi

  # Check 4: timeout command (fails in non-interactive context)
  # Note: 'timeout /t N /nobreak' works in HKLM Run context (startup bat files).
  # Only flag bare 'timeout' without /nobreak, which fails in SSH/exec contexts.
  local timeout_lines
  timeout_lines=$(grep -nP '^\s*timeout\s' "$file_path" 2>/dev/null || true)
  if [[ -n "$timeout_lines" ]]; then
    while IFS= read -r line; do
      # Skip if /nobreak is present (works in most non-interactive contexts)
      if echo "$line" | grep -qi '/nobreak'; then
        continue
      fi
      local linenum="${line%%:*}"
      echo "  [$label] Line $linenum: timeout command fails in non-interactive context -- use 'ping -n N 127.0.0.1 >nul'"
      violations=$((violations + 1))
    done <<< "$timeout_lines"
  fi

  # Check 5: taskkill without restart
  # For each taskkill line, check if there is a corresponding start or schtasks
  # within 20 lines after for the same process.
  local total_lines
  total_lines=$(wc -l < "$file_path")
  local tk_lines
  tk_lines=$(grep -nP 'taskkill\s.*/IM\s+' "$file_path" 2>/dev/null || true)
  if [[ -n "$tk_lines" ]]; then
    while IFS= read -r tk_line; do
      local linenum="${tk_line%%:*}"
      # Extract process name from /IM <name>
      local proc_name
      proc_name=$(echo "$tk_line" | grep -oP '/IM\s+\K\S+' | tr -d '"' | head -1)
      if [[ -z "$proc_name" ]]; then
        continue
      fi
      # Skip bloatware/cleanup kills -- these are intentional kills without restart
      # (Variable_dump, Creative Cloud, Copilot, ollama, Clockify, OneDrive, powershell, ConspitLink)
      case "$proc_name" in
        Variable_dump.exe|"Creative Cloud UI Helper.exe"|M365Copilot.exe|Copilot.exe|ollama.exe|ClockifyWindows.exe|OneDrive.exe|powershell.exe|ConspitLink2.0.exe)
          continue
          ;;
      esac
      # Check next 20 lines for a matching start or schtasks
      local search_end=$((linenum + 20))
      if [[ $search_end -gt $total_lines ]]; then
        search_end=$total_lines
      fi
      local proc_base="${proc_name%.exe}"
      local found_restart=false
      if sed -n "$((linenum+1)),${search_end}p" "$file_path" 2>/dev/null | grep -qi "start.*${proc_base}\|schtasks.*${proc_base}"; then
        found_restart=true
      fi
      if [[ "$found_restart" == "false" ]]; then
        echo "  [$label] Line $linenum: taskkill /IM $proc_name without matching restart -- process will stay dead"
        violations=$((violations + 1))
      fi
    done <<< "$tk_lines"
  fi

  return $violations
}

# =============================================================================
# FUNCTION: bat_scan_pod(pod_num, bat_name, canonical_path)
#
# Scans a single pod for a specific bat file via rc-sentry /files endpoint.
# Returns: 0 for match, 1 for drift or unreachable.
# =============================================================================
bat_scan_pod() {
  local pod_num="$1"
  local bat_name="$2"
  local canonical_path="$3"
  local ip
  ip=$(pod_ip "$pod_num")

  if [[ -z "$ip" ]]; then
    echo -e "${RED}POD $pod_num: INVALID POD NUMBER${NC}"
    return 1
  fi

  if [[ ! -f "$canonical_path" ]]; then
    echo -e "${YELLOW}POD $pod_num: SKIP ($bat_name) -- canonical file not found${NC}"
    return 1
  fi

  # Build JSON payload -- standing rule: write JSON to file, never inline
  local payload_file="$BAT_SCAN_TMPDIR/payload_pod${pod_num}_${bat_name}.json"
  printf '{"path":"C:\\\\RacingPoint\\\\%s"}' "$bat_name" > "$payload_file"

  # Fetch via rc-sentry /files endpoint
  local response_file="$BAT_SCAN_TMPDIR/pod${pod_num}_${bat_name}"
  local http_code
  http_code=$(curl -s --max-time 10 -w '%{http_code}' -o "$response_file" \
    -X POST "http://${ip}:${SENTRY_PORT}/files" \
    -H "Content-Type: application/json" \
    -d @"$payload_file" 2>/dev/null) || true

  if [[ "$http_code" != "200" ]] || [[ ! -s "$response_file" ]]; then
    echo -e "${RED}POD $pod_num: UNREACHABLE ($bat_name) -- HTTP $http_code${NC}"
    return 1
  fi

  # Compute SHA256 hashes (strip \r for cross-platform comparison)
  local canonical_hash fetched_hash
  canonical_hash=$(tr -d '\r' < "$canonical_path" | sha256sum | awk '{print $1}')
  fetched_hash=$(tr -d '\r' < "$response_file" | sha256sum | awk '{print $1}')

  if [[ "$canonical_hash" == "$fetched_hash" ]]; then
    echo -e "${GREEN}POD $pod_num: MATCH ($bat_name)${NC}"
    # Run syntax validation even on matching files
    local syntax_output
    syntax_output=$(bat_validate_syntax "$response_file" "pod-$pod_num/$bat_name")
    local syntax_violations=$?
    if [[ $syntax_violations -gt 0 ]]; then
      echo "$syntax_output"
      echo -e "  ${YELLOW}$syntax_violations syntax violation(s) found${NC}"
    fi
    return 0
  else
    echo -e "${RED}POD $pod_num: DRIFT ($bat_name)${NC}"
    # Show diff
    diff --color=auto \
      --label "canonical" \
      --label "pod-$pod_num" \
      <(tr -d '\r' < "$canonical_path") \
      <(tr -d '\r' < "$response_file") || true
    echo ""
    # Run syntax validation on the drifted file
    local syntax_output
    syntax_output=$(bat_validate_syntax "$response_file" "pod-$pod_num/$bat_name")
    local syntax_violations=$?
    if [[ $syntax_violations -gt 0 ]]; then
      echo "$syntax_output"
      echo -e "  ${YELLOW}$syntax_violations syntax violation(s) found${NC}"
    fi
    return 1
  fi
}

# =============================================================================
# FUNCTION: bat_scan_all()
#
# Loop over pods 1-8, scan both start-rcagent.bat and start-rcsentry.bat.
# Prints summary at end.
# =============================================================================
bat_scan_all() {
  local matches=0 drifts=0 unreachable=0
  local bat_files=("start-rcagent.bat")

  # Add rcsentry if canonical exists
  if [[ -f "$CANONICAL_RCSENTRY" ]]; then
    bat_files+=("start-rcsentry.bat")
  else
    echo -e "${YELLOW}WARNING: Canonical start-rcsentry.bat not found at $CANONICAL_RCSENTRY -- skipping rcsentry scan${NC}"
  fi

  echo -e "${CYAN}=== Bat File Fleet Scan ===${NC}"
  echo ""

  for pod_num in 1 2 3 4 5 6 7 8; do
    echo -e "${CYAN}--- Pod $pod_num ($(pod_ip "$pod_num")) ---${NC}"
    for bat_name in "${bat_files[@]}"; do
      local canonical_path
      if [[ "$bat_name" == "start-rcagent.bat" ]]; then
        canonical_path="$CANONICAL_RCAGENT"
      else
        canonical_path="$CANONICAL_RCSENTRY"
      fi

      if bat_scan_pod "$pod_num" "$bat_name" "$canonical_path"; then
        matches=$((matches + 1))
      else
        # Distinguish drift from unreachable by checking output
        if [[ -s "$BAT_SCAN_TMPDIR/pod${pod_num}_${bat_name}" ]]; then
          drifts=$((drifts + 1))
        else
          unreachable=$((unreachable + 1))
        fi
      fi
    done
    echo ""
  done

  local total=$((matches + drifts + unreachable))
  echo -e "${CYAN}=== Summary ===${NC}"
  echo -e "  Total checks: $total"
  echo -e "  ${GREEN}Match:${NC}       $matches"
  echo -e "  ${RED}Drift:${NC}       $drifts"
  echo -e "  ${YELLOW}Unreachable:${NC} $unreachable"

  if [[ $drifts -gt 0 ]] || [[ $unreachable -gt 0 ]]; then
    return 1
  fi
  return 0
}

# =============================================================================
# FUNCTION: bat_scan_pod_json(pod_num, bat_name, canonical_path)
#
# Same as bat_scan_pod but returns JSON object for --json mode.
# =============================================================================
bat_scan_pod_json() {
  local pod_num="$1"
  local bat_name="$2"
  local canonical_path="$3"
  local ip
  ip=$(pod_ip "$pod_num")

  if [[ -z "$ip" ]] || [[ ! -f "$canonical_path" ]]; then
    printf '{"pod":%d,"bat":"%s","status":"SKIP","violations":[],"diff":""}' "$pod_num" "$bat_name"
    return
  fi

  # Build JSON payload
  local payload_file="$BAT_SCAN_TMPDIR/jpayload_pod${pod_num}_${bat_name}.json"
  printf '{"path":"C:\\\\RacingPoint\\\\%s"}' "$bat_name" > "$payload_file"

  local response_file="$BAT_SCAN_TMPDIR/jpod${pod_num}_${bat_name}"
  local http_code
  http_code=$(curl -s --max-time 10 -w '%{http_code}' -o "$response_file" \
    -X POST "http://${ip}:${SENTRY_PORT}/files" \
    -H "Content-Type: application/json" \
    -d @"$payload_file" 2>/dev/null) || true

  if [[ "$http_code" != "200" ]] || [[ ! -s "$response_file" ]]; then
    printf '{"pod":%d,"bat":"%s","status":"UNREACHABLE","violations":[],"diff":""}' "$pod_num" "$bat_name"
    return
  fi

  local canonical_hash fetched_hash
  canonical_hash=$(tr -d '\r' < "$canonical_path" | sha256sum | awk '{print $1}')
  fetched_hash=$(tr -d '\r' < "$response_file" | sha256sum | awk '{print $1}')

  local status="MATCH"
  local diff_output=""
  if [[ "$canonical_hash" != "$fetched_hash" ]]; then
    status="DRIFT"
    diff_output=$(diff \
      --label "canonical" \
      --label "pod-$pod_num" \
      <(tr -d '\r' < "$canonical_path") \
      <(tr -d '\r' < "$response_file") 2>/dev/null || true)
  fi

  # Collect syntax violations
  local violations_json="[]"
  local syntax_output
  syntax_output=$(bat_validate_syntax "$response_file" "pod-$pod_num/$bat_name" 2>/dev/null)
  local syntax_violations=$?
  if [[ $syntax_violations -gt 0 ]] && [[ -n "$syntax_output" ]]; then
    # Convert violations to JSON array
    violations_json="["
    local first=true
    while IFS= read -r vline; do
      if [[ -n "$vline" ]]; then
        # Escape for JSON
        local escaped
        escaped=$(printf '%s' "$vline" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/\\t/g')
        if [[ "$first" == "true" ]]; then
          first=false
        else
          violations_json+=","
        fi
        violations_json+="\"$escaped\""
      fi
    done <<< "$syntax_output"
    violations_json+="]"
  fi

  # Escape diff for JSON
  local diff_escaped=""
  if [[ -n "$diff_output" ]]; then
    diff_escaped=$(printf '%s' "$diff_output" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/\\t/g' | tr '\n' '\\' | sed 's/\\/\\n/g')
  fi

  printf '{"pod":%d,"bat":"%s","status":"%s","violations":%s,"diff":"%s"}' \
    "$pod_num" "$bat_name" "$status" "$violations_json" "$diff_escaped"
}

# =============================================================================
# FUNCTION: bat_scan_all_json()
#
# JSON output mode for audit integration.
# =============================================================================
bat_scan_all_json() {
  local bat_files=("start-rcagent.bat")
  if [[ -f "$CANONICAL_RCSENTRY" ]]; then
    bat_files+=("start-rcsentry.bat")
  fi

  echo "["
  local first=true
  for pod_num in 1 2 3 4 5 6 7 8; do
    for bat_name in "${bat_files[@]}"; do
      local canonical_path
      if [[ "$bat_name" == "start-rcagent.bat" ]]; then
        canonical_path="$CANONICAL_RCAGENT"
      else
        canonical_path="$CANONICAL_RCSENTRY"
      fi

      if [[ "$first" == "true" ]]; then
        first=false
      else
        echo ","
      fi
      bat_scan_pod_json "$pod_num" "$bat_name" "$canonical_path"
    done
  done
  echo ""
  echo "]"
}

# =============================================================================
# STANDALONE MODE
# =============================================================================
bat_scanner_usage() {
  cat <<'USAGE'
bat-scanner.sh -- Bat file drift detection + syntax validation

Usage:
  bat-scanner.sh [OPTIONS]

Options:
  --all             Scan all 8 pods (default)
  --pod N           Scan single pod (1-8)
  --validate FILE   Run syntax validation only on a local file
  --json            Output results as JSON (for audit integration)
  --help            Show this help

Examples:
  bat-scanner.sh                          # scan all pods
  bat-scanner.sh --pod 3                  # scan pod 3 only
  bat-scanner.sh --validate my-file.bat   # validate syntax only
  bat-scanner.sh --json                   # JSON output for all pods
  bat-scanner.sh --json --pod 5           # JSON output for pod 5

Syntax checks (5 known anti-patterns):
  1. UTF-8 BOM detection
  2. Parentheses in if/else blocks (use goto labels)
  3. /dev/null redirection (use NUL on Windows)
  4. timeout command (fails non-interactive; use ping -n)
  5. taskkill without matching restart

Exit code: 0 if all checks pass, 1 if any drift/unreachable/violation.

Audit integration: source this file and call functions directly:
  source scripts/bat-scanner.sh
  bat_scan_pod 3 "start-rcagent.bat" "$CANONICAL_RCAGENT"
  bat_validate_syntax /path/to/file.bat "label"
USAGE
}

# Only run main logic if executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  # Parse arguments
  MODE="all"
  POD_NUM=""
  VALIDATE_FILE=""
  JSON_OUTPUT=false

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --all)
        MODE="all"
        shift
        ;;
      --pod)
        MODE="pod"
        POD_NUM="${2:-}"
        if [[ -z "$POD_NUM" ]]; then
          echo "ERROR: --pod requires a pod number (1-8)"
          exit 1
        fi
        shift 2
        ;;
      --validate)
        MODE="validate"
        VALIDATE_FILE="${2:-}"
        if [[ -z "$VALIDATE_FILE" ]]; then
          echo "ERROR: --validate requires a file path"
          exit 1
        fi
        shift 2
        ;;
      --json)
        JSON_OUTPUT=true
        shift
        ;;
      --help|-h)
        bat_scanner_usage
        exit 0
        ;;
      *)
        echo "ERROR: Unknown option: $1"
        bat_scanner_usage
        exit 1
        ;;
    esac
  done

  case "$MODE" in
    validate)
      echo -e "${CYAN}=== Syntax Validation: $VALIDATE_FILE ===${NC}"
      output=$(bat_validate_syntax "$VALIDATE_FILE" "$(basename "$VALIDATE_FILE")")
      result=$?
      if [[ $result -eq 0 ]]; then
        echo -e "${GREEN}No syntax violations found.${NC}"
        exit 0
      else
        echo "$output"
        echo -e "${RED}$result syntax violation(s) found.${NC}"
        exit 1
      fi
      ;;
    pod)
      if [[ "$JSON_OUTPUT" == "true" ]]; then
        echo "["
        local first=true
        for bat_name in "start-rcagent.bat" "start-rcsentry.bat"; do
          local canonical_path
          if [[ "$bat_name" == "start-rcagent.bat" ]]; then
            canonical_path="$CANONICAL_RCAGENT"
          else
            canonical_path="$CANONICAL_RCSENTRY"
            if [[ ! -f "$canonical_path" ]]; then
              continue
            fi
          fi
          if [[ "$first" == "true" ]]; then
            first=false
          else
            echo ","
          fi
          bat_scan_pod_json "$POD_NUM" "$bat_name" "$canonical_path"
        done
        echo ""
        echo "]"
      else
        exit_code=0
        echo -e "${CYAN}--- Pod $POD_NUM ($(pod_ip "$POD_NUM")) ---${NC}"
        bat_scan_pod "$POD_NUM" "start-rcagent.bat" "$CANONICAL_RCAGENT" || exit_code=1
        if [[ -f "$CANONICAL_RCSENTRY" ]]; then
          bat_scan_pod "$POD_NUM" "start-rcsentry.bat" "$CANONICAL_RCSENTRY" || exit_code=1
        fi
        exit $exit_code
      fi
      ;;
    all)
      if [[ "$JSON_OUTPUT" == "true" ]]; then
        bat_scan_all_json
      else
        bat_scan_all
      fi
      exit $?
      ;;
  esac
fi
