#!/usr/bin/env bash
# CGP Compliance Checker — Machine-readable gate proof detection
#
# Scans a text response (stdin or file) for Cognitive Gate Protocol v3.0
# proof artifacts. Returns a structured report of which gates were triggered
# and whether proofs are present.
#
# Usage:
#   echo "response text" | bash scripts/cgp-compliance-check.sh
#   bash scripts/cgp-compliance-check.sh < response.txt
#   bash scripts/cgp-compliance-check.sh --file response.txt
#
# Exit codes:
#   0 — All triggered gates have proofs
#   1 — Missing proofs for triggered gates
#   2 — No gates detected (may be fine for simple responses)
#
# Fixes: W-06 (machine-readable), W-10 (metrics), W-11 (external validation)

set -euo pipefail

# Read input
INPUT=""
if [[ "${1:-}" == "--file" ]] && [[ -n "${2:-}" ]]; then
  INPUT="$(cat "$2")"
elif [[ ! -t 0 ]]; then
  INPUT="$(cat)"
else
  echo "Usage: echo 'response' | $0"
  echo "       $0 --file response.txt"
  exit 2
fi

# Gate detection patterns (regex for proof artifacts)
declare -A GATE_TRIGGERS
declare -A GATE_PROOFS
declare -A GATE_FOUND

# G0: Problem Definition
GATE_TRIGGERS[G0]="PROBLEM:|SYMPTOMS:|PLAN:"
GATE_PROOFS[G0]="PROBLEM:.*SYMPTOMS:.*PLAN:"

# G1: Outcome Verification (completion claims trigger this)
GATE_TRIGGERS[G1]="fixed|done|complete|deployed|verified|confirmed|working|PASS"
GATE_PROOFS[G1]="Behavior tested|Method of observation|Raw evidence|Tested:|Evidence:"

# G2: Fleet Scope
GATE_TRIGGERS[G2]="Target.*Applies.*Applied.*Evidence|Fleet Scope|fleet scope"
GATE_PROOFS[G2]="Target.*Applies.*Applied.*Evidence|\| .* \| Y"

# G3: Apply Now (user shares info → application shown)
# This is context-dependent, hard to detect mechanically
GATE_TRIGGERS[G3]=""
GATE_PROOFS[G3]=""

# G4: Confidence Calibration
GATE_TRIGGERS[G4]="Tested:|Not Tested:|Follow-up Plan:|Confidence"
GATE_PROOFS[G4]="Tested:.*Not Tested:|Not Tested.*Follow-up"

# G5: Competing Hypotheses
GATE_TRIGGERS[G5]="Hypothesis A|Hypothesis B|hypothesis|hypotheses"
GATE_PROOFS[G5]="Hypothesis [AB].*Test:|hypothes.*falsif"

# G6: Context Parking
GATE_TRIGGERS[G6]="PAUSED:|STATUS:|NEXT:|RESUME BY:"
GATE_PROOFS[G6]="PAUSED:.*STATUS:.*NEXT:"

# G7: Tool Verification
GATE_TRIGGERS[G7]="Requirement:.*Tool:.*Compatibility|Tool selected|Compatibility check"
GATE_PROOFS[G7]="Requirement:.*Tool:|Tool selected:.*Compatibility"

# G8: Dependency Cascade
GATE_TRIGGERS[G8]="Changed:.*Downstream|downstream consumers|Dependency Cascade"
GATE_PROOFS[G8]="Changed:.*Downstream consumers:|downstream consumers:.*Verification"

# G9: Retrospective
GATE_TRIGGERS[G9]="ROOT CAUSE:|PREVENTION:|SIMILAR PAST:|Retrospective"
GATE_PROOFS[G9]="ROOT CAUSE:.*PREVENTION:"

# Check for gate summary block
HAS_SUMMARY=false
if echo "$INPUT" | grep -qiP "GATES TRIGGERED:.*PROOFS:"; then
  HAS_SUMMARY=true
fi

# Detect which gates appear to be triggered and whether proofs exist
TRIGGERED=0
PROVEN=0
MISSING=0
REPORT=""

for gate in G0 G1 G2 G4 G5 G6 G7 G8 G9; do
  trigger="${GATE_TRIGGERS[$gate]}"
  proof="${GATE_PROOFS[$gate]}"

  if [[ -z "$trigger" ]]; then
    continue
  fi

  # Check if gate trigger patterns appear in text
  if echo "$INPUT" | grep -qiP "$trigger" 2>/dev/null; then
    TRIGGERED=$((TRIGGERED + 1))

    # Check if proof patterns appear
    # Use multiline grep for proofs that span lines
    FULL_INPUT=$(echo "$INPUT" | tr '\n' ' ')
    if echo "$FULL_INPUT" | grep -qiP "$proof" 2>/dev/null; then
      PROVEN=$((PROVEN + 1))
      GATE_FOUND[$gate]="PROOF"
      REPORT="${REPORT}  ${gate}: TRIGGERED + PROOF FOUND\n"
    else
      MISSING=$((MISSING + 1))
      GATE_FOUND[$gate]="MISSING"
      REPORT="${REPORT}  ${gate}: TRIGGERED — PROOF MISSING\n"
    fi
  fi
done

# Output report
echo "=== CGP Compliance Report ==="
echo "Gates triggered: $TRIGGERED"
echo "Proofs found:    $PROVEN"
echo "Proofs missing:  $MISSING"
echo "Summary block:   $HAS_SUMMARY"
echo ""
if [[ -n "$REPORT" ]]; then
  echo -e "Gate Details:"
  echo -e "$REPORT"
fi

# Determine exit code
if [[ $TRIGGERED -eq 0 ]]; then
  echo "STATUS: NO_GATES — No gate triggers detected (OK for simple responses)"
  exit 2
elif [[ $MISSING -gt 0 ]]; then
  echo "STATUS: INCOMPLETE — $MISSING gate(s) triggered without proof"
  exit 1
else
  echo "STATUS: COMPLIANT — All triggered gates have proofs"
  exit 0
fi
