#!/usr/bin/env bash
# fix_log.sh — Cause Elimination Process (GATE-05)
# Prompts for 5 structured debugging fields and appends to LOGBOOK.md.
# Standing rule: any bug taking >30 min to isolate MUST use this process.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOGBOOK="$REPO_ROOT/LOGBOOK.md"

if [ ! -f "$LOGBOOK" ]; then
  echo "ERROR: LOGBOOK.md not found at $LOGBOOK"
  exit 1
fi

# Generate IST timestamp
TIMESTAMP=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date -u -d '+5 hours 30 minutes' '+%Y-%m-%d %H:%M IST')

# --- Helper: read multiline input until blank line or EOF ---
read_multiline() {
  local result=""
  local line
  while IFS= read -r line; do
    [ -z "$line" ] && break
    if [ -z "$result" ]; then
      result="$line"
    else
      result="$result"$'\n'"$line"
    fi
  done
  echo "$result"
}

# --- 1/5 SYMPTOM ---
while true; do
  echo ""
  echo "1/5 — SYMPTOM: What exactly happened? (error message, behavior observed, screenshot path)"
  echo "(Enter text, then press Enter on a blank line to finish)"
  SYMPTOM=$(read_multiline)
  if [ -z "$(echo "$SYMPTOM" | tr -d '[:space:]')" ]; then
    echo "ERROR: Symptom is required. Cannot skip."
  else
    break
  fi
done

# --- 2/5 HYPOTHESES ---
while true; do
  echo ""
  echo "2/5 — HYPOTHESES: List ALL possible causes (one per line, include software/hardware/config/network/user error):"
  echo "(Enter text, then press Enter on a blank line to finish)"
  HYPOTHESES=$(read_multiline)
  if [ -z "$(echo "$HYPOTHESES" | tr -d '[:space:]')" ]; then
    echo "ERROR: Hypotheses is required. Cannot skip."
    continue
  fi
  # Count non-empty lines
  HYPO_COUNT=$(echo "$HYPOTHESES" | grep -c '.' || true)
  if [ "$HYPO_COUNT" -lt 2 ]; then
    echo "ERROR: List at least 2 hypotheses. Single-hypothesis debugging skips the elimination step."
  else
    break
  fi
done

# --- 3/5 ELIMINATION ---
while true; do
  echo ""
  echo "3/5 — ELIMINATION: For each hypothesis, what test was run and what was the result? (one per line, format: 'H1: tested X — result Y — ELIMINATED/CONFIRMED'):"
  echo "(Enter text, then press Enter on a blank line to finish)"
  ELIMINATION=$(read_multiline)
  if [ -z "$(echo "$ELIMINATION" | tr -d '[:space:]')" ]; then
    echo "ERROR: Elimination is required. Cannot skip."
  else
    break
  fi
done

# --- 4/5 CONFIRMED CAUSE ---
while true; do
  echo ""
  echo "4/5 — CONFIRMED CAUSE: Which hypothesis survived elimination?"
  read -r CONFIRMED_CAUSE
  if [ -z "$(echo "$CONFIRMED_CAUSE" | tr -d '[:space:]')" ]; then
    echo "ERROR: Confirmed cause is required. Cannot skip."
  else
    break
  fi
done

# --- 5/5 VERIFICATION ---
while true; do
  echo ""
  echo "5/5 — VERIFICATION: How was the fix confirmed to work? (command run, output observed, screenshot path):"
  echo "(Enter text, then press Enter on a blank line to finish)"
  VERIFICATION=$(read_multiline)
  if [ -z "$(echo "$VERIFICATION" | tr -d '[:space:]')" ]; then
    echo "ERROR: Verification is required. Cannot skip."
  else
    break
  fi
done

# --- Format hypotheses and elimination with "- " prefix ---
FORMATTED_HYPOTHESES=$(echo "$HYPOTHESES" | sed 's/^/- /')
FORMATTED_ELIMINATION=$(echo "$ELIMINATION" | sed 's/^/- /')

# --- Append to LOGBOOK.md ---
{
  echo ""
  echo "### Cause Elimination — $TIMESTAMP"
  echo ""
  echo "**Symptom:** $SYMPTOM"
  echo ""
  echo "**Hypotheses:**"
  echo "$FORMATTED_HYPOTHESES"
  echo ""
  echo "**Elimination:**"
  echo "$FORMATTED_ELIMINATION"
  echo ""
  echo "**Confirmed cause:** $CONFIRMED_CAUSE"
  echo ""
  echo "**Verification:** $VERIFICATION"
  echo ""
  echo "---"
} >> "$LOGBOOK"

echo ""
echo "Entry appended to LOGBOOK.md at $TIMESTAMP"
echo "File: $LOGBOOK"
