#!/usr/bin/env bash
# fix-scope.sh — Blast radius analysis before applying a fix
# Usage: bash scripts/fix-scope.sh <file> [function_name]
# Pure bash + grep. Works on Linux and Git Bash (Windows).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
FILE="${1:-}"; FUNC="${2:-}"
[ -z "$FILE" ] && { echo "Usage: bash scripts/fix-scope.sh <file> [function_name]"; exit 1; }
[[ "$FILE" != /* ]] && FILE="$REPO/$FILE"
[ ! -f "$FILE" ] && { echo "ERROR: File not found: $FILE"; exit 1; }

REL="${FILE#$REPO/}"; MODULE="$(basename "$FILE" .rs)"
CRATE=""
case "$REL" in
  crates/racecontrol/*) CRATE="crates/racecontrol" ;; crates/rc-agent/*) CRATE="crates/rc-agent" ;;
  crates/rc-common/*) CRATE="crates/rc-common" ;; crates/rc-guardian/*) CRATE="crates/rc-guardian" ;;
  crates/rc-watchdog/*) CRATE="crates/rc-watchdog" ;; crates/rc-sentry/*) CRATE="crates/rc-sentry" ;;
  pwa/*) CRATE="pwa" ;; kiosk/*) CRATE="kiosk" ;; web/*) CRATE="web" ;;
esac

# Extract function body (rough) or whole file
if [ -n "$FUNC" ]; then
  BODY=$(sed -n "/fn ${FUNC}/,/^pub \(async \)\{0,1\}fn /p" "$FILE" 2>/dev/null | head -300)
  [ -z "$BODY" ] && BODY=$(sed -n "/fn ${FUNC}/,\$p" "$FILE" 2>/dev/null | head -300)
else
  BODY=$(cat "$FILE")
fi

echo "========================================================"; C=0; X=0; S=0; D=0; F=0; TH=0; TM=0
echo "BLAST RADIUS ANALYSIS: ${FUNC:-$MODULE} in $REL"
echo "========================================================"

# ── DIRECT CALLERS ──
echo ""; echo "DIRECT CALLERS:"
if [ -n "$FUNC" ]; then
  while IFS= read -r l; do
    case "$l" in *"fn $FUNC"*) continue ;; esac
    C=$((C+1)); echo "  $l"
  done < <(grep -rn --include="*.rs" "$FUNC" "$REPO/crates/" 2>/dev/null | grep -v "$REL" | head -25)
  [ "$C" -eq 0 ] && echo "  (none found)"
else echo "  (specify function name)"; fi

# ── CROSS-CRATE CONSUMERS ──
echo ""; echo "CROSS-CRATE CONSUMERS:"
if [ -n "$CRATE" ] && [ -n "$FUNC" ]; then
  while IFS= read -r l; do
    case "$l" in *"$CRATE/"*) continue ;; esac; X=$((X+1)); echo "  $l"
  done < <(grep -rn --include="*.rs" "$FUNC" "$REPO/crates/" 2>/dev/null | grep -v "$CRATE/" | head -15)
fi
[ "$X" -eq 0 ] && echo "  (none found)"

# ── SHARED STATE ──
echo ""; echo "SHARED STATE:"
for FLD in db pods billing game_launcher agent_senders feature_flags terminal_sessions \
  dashboard_tx bono_event_tx config http_client camera ac_server pod_watchdog_states \
  pod_deploy_states pending_deploys fleet_deploy_session assist_cache pending_ws_execs \
  pending_command_acks otp_rate_limits customer_pin_failures staff_pin_failures; do
  if echo "$BODY" | grep -q "state\.$FLD" 2>/dev/null; then
    ACC="read"
    echo "$BODY" | grep -q "state\.${FLD}\.write\|state\.${FLD}\.lock" 2>/dev/null && ACC="read/write"
    S=$((S+1)); echo "  AppState.$FLD — $ACC"
  fi
done
[ "$S" -eq 0 ] && echo "  (no AppState access detected)"

# ── DB TABLES ──
echo ""; echo "DB TABLES TOUCHED:"
for OP in "INSERT INTO" "UPDATE" "DELETE FROM"; do
  while IFS= read -r m; do
    [ -z "$m" ] && continue; D=$((D+1))
    TBL=$(echo "$m" | sed "s/.*${OP}\s*//I" | sed 's/[" (].*//' | tr -d '\\')
    [ -n "$TBL" ] && echo "  $TBL — ${OP%% *}"
  done < <(echo "$BODY" | grep -oiE "${OP}\s+[a-z_]+" 2>/dev/null | sort -u)
done
while IFS= read -r m; do
  [ -z "$m" ] && continue; D=$((D+1))
  TBL=$(echo "$m" | sed 's/.*FROM\s*//I' | sed 's/[" (].*//' | tr -d '\\')
  [ -n "$TBL" ] && echo "  $TBL — SELECT"
done < <(echo "$BODY" | grep -oiE "SELECT\s+.*\s+FROM\s+[a-z_]+" 2>/dev/null | sort -u)
[ "$D" -eq 0 ] && echo "  (no SQL operations detected)"

# ── API ROUTES ──
echo ""; echo "API ROUTES USING THIS:"
R=0
if [ -n "$FUNC" ] && [ -d "$REPO/crates/racecontrol/src/api" ]; then
  while IFS= read -r l; do R=$((R+1)); echo "  $l"
  done < <(grep -rn --include="*.rs" "$FUNC\|$MODULE" "$REPO/crates/racecontrol/src/api/" 2>/dev/null | head -10)
fi
[ "$R" -eq 0 ] && echo "  (none found)"

# ── FRONTEND CONSUMERS ──
echo ""; echo "FRONTEND CONSUMERS:"
if [ -n "$FUNC" ]; then
  SEG=$(echo "$FUNC" | sed 's/_/-/g')
  for DIR in pwa kiosk web; do
    [ ! -d "$REPO/$DIR/src" ] && continue
    while IFS= read -r l; do F=$((F+1)); echo "  $l"
    done < <(grep -rn --include="*.ts" --include="*.tsx" "$SEG\|$FUNC" "$REPO/$DIR/src/" 2>/dev/null | \
             grep -v node_modules | head -10)
  done
fi
[ "$F" -eq 0 ] && echo "  (no frontend references found)"

# ── TEST COVERAGE ──
echo ""; echo "TEST COVERAGE:"
if [ -n "$FUNC" ]; then
  while IFS= read -r l; do TH=$((TH+1)); echo "  + $l"
  done < <(grep -rn --include="*.rs" "$FUNC" "$REPO/crates/" "$REPO/tests/" 2>/dev/null | \
           grep -E "test_|mod tests|#\[test|#\[tokio::test" | head -10)
  # Check untested callers
  while IFS= read -r cf; do
    [ -z "$cf" ] && continue
    CFN=$(echo "$cf" | grep -oE 'fn [a-z_]+' | head -1 | sed 's/fn //')
    [ -z "$CFN" ] && continue
    grep -rq --include="*.rs" "test.*$CFN\|$CFN.*test" "$REPO/crates/" 2>/dev/null || {
      TM=$((TM+1)); echo "  - $CFN — NO test coverage"; }
  done < <(grep -rn --include="*.rs" "$FUNC" "$REPO/crates/" 2>/dev/null | grep -v "$REL" | grep -v test | grep "fn " | head -8)
  [ "$TH" -eq 0 ] && [ "$TM" -eq 0 ] && echo "  - NO test coverage found for $FUNC"
else echo "  (specify function name)"; fi

# ── RISK ──
echo ""; echo "──────────────────────────────────────────────────────────"
RISK="LOW"; RR=""
echo "$REL" | grep -qiE "billing|wallet|accounting|finance" && { RISK="HIGH"; RR="${RR}  - Financial code path\n"; }
echo "$BODY" | grep -qiE "INSERT INTO|UPDATE|DELETE FROM" 2>/dev/null && { RISK="HIGH"; RR="${RR}  - DB write operations\n"; }
[ "$C" -gt 5 ] && { RISK="HIGH"; RR="${RR}  - $C direct callers (high fan-in)\n"; }
echo "$BODY" | grep -q "\.write()\|\.lock()" 2>/dev/null && { [ "$RISK" = "LOW" ] && RISK="MEDIUM"; RR="${RR}  - Shared state writes\n"; }
[ "$X" -gt 0 ] && { [ "$RISK" = "LOW" ] && RISK="MEDIUM"; RR="${RR}  - $X cross-crate consumers\n"; }
[ "$TH" -eq 0 ] && [ -n "$FUNC" ] && { [ "$RISK" = "LOW" ] && RISK="MEDIUM"; RR="${RR}  - Zero test coverage\n"; }
[ "$F" -gt 0 ] && { [ "$RISK" = "LOW" ] && RISK="MEDIUM"; RR="${RR}  - $F frontend consumers\n"; }
echo "RISK: $RISK"
[ -n "$RR" ] && { echo "Reasons:"; echo -e "$RR"; }
echo "SUMMARY: ${C} callers | ${X} cross-crate | ${S} state | ${D} DB | ${F} frontend | ${TH} tested / ${TM} untested"
