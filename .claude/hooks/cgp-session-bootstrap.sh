#!/usr/bin/env bash
# CGP Session Bootstrap Hook — Cognitive Gate Protocol v3.0 Enforcer
#
# PURPOSE: Inject CGP gate awareness at the START of every session.
# Fixes: W-01 (voluntary read), W-03 (no hook), W-04 (continued sessions),
#        W-12 (no session-start gate)
#
# This hook runs via Claude Code's SessionStart event. Its output becomes
# part of the conversation context, ensuring the AI has CGP gate triggers
# and proof requirements loaded before any work begins.
#
# IMPORTANT: This hook outputs to STDOUT. Claude Code treats hook output
# as additional context (like a system-reminder). Keep output concise
# but complete — it must survive context compaction.

set -euo pipefail

# Determine repo root (hook runs from project working directory)
REPO_ROOT="$(pwd)"

# Check which repo we're in to find CGP source
CGP_FILE=""
if [ -f "$REPO_ROOT/COGNITIVE-GATE-PROTOCOL.md" ]; then
  CGP_FILE="$REPO_ROOT/COGNITIVE-GATE-PROTOCOL.md"
elif [ -f "$REPO_ROOT/../racecontrol/COGNITIVE-GATE-PROTOCOL.md" ]; then
  CGP_FILE="$REPO_ROOT/../racecontrol/COGNITIVE-GATE-PROTOCOL.md"
elif [ -f "/root/racecontrol/COGNITIVE-GATE-PROTOCOL.md" ]; then
  CGP_FILE="/root/racecontrol/COGNITIVE-GATE-PROTOCOL.md"
fi

# Extract CGP version from file if found
CGP_VERSION="v3.0"
if [ -n "$CGP_FILE" ]; then
  VER=$(grep -oP 'Cognitive Gate Protocol v\K[0-9.]+' "$CGP_FILE" 2>/dev/null | head -1)
  [ -n "$VER" ] && CGP_VERSION="v$VER"
fi

# Emit structured CGP bootstrap block
# This format is designed to be:
# 1. Scannable by the AI (structured, not prose)
# 2. Resistant to context compaction (key info in compact format)
# 3. Self-verifying (includes a checklist the AI must acknowledge)
cat << 'CGPEOF'
[CGP-BOOT] Cognitive Gate Protocol v3.0 — Session Bootstrap
============================================================
STATUS: ACTIVE | ENFORCEMENT: MANDATORY | BYPASS: EMERGENCY ONLY
MERGED: CGP v2.1 + Unified Operations Protocol v3.0 (2026-04-01)
FULL DOC: COGNITIVE-GATE-PROTOCOL.md

LIFECYCLE: Phase 0 (Start) → 1 (Plan) → 2 (Create) → 3 (Verify) → 4 (Deploy) → 5 (Ship)
           Phase D (Debug) | Phase E (Emergency) | Phase B (Break-Glass) | Phase I (Island)

GATE TRIGGERS & REQUIRED PROOFS (embedded in lifecycle phases):
  G0  New task      [Ph 0,1]  → PROBLEM: + SYMPTOMS: + PLAN: block
  G1  Before "done" [Ph 3,5]  → Behavior tested + method + raw evidence (not proxies)
  G2  After any fix [Ph 4,5]  → Per-target fleet scope table with evidence
  G3  User info     [Ph D]    → Show APPLICATION (command+output), not summary
  G4  Success claim [Ph 3,5]  → Tested / Not Tested (risk) / Follow-up Plan
  G5  Anomaly       [Ph D,3]  → 2+ hypotheses with falsification tests
  G6  Topic change  [Any]     → PAUSED: + STATUS: + NEXT: + RESUME BY: block
  G7  Tool pick     [Ph 2,4]  → Requirement + tool + compatibility check
  G8  Shared change [Ph 2,4]  → Changed + downstream consumers + verification
  G9  Resolution    [Ph D]    → Root cause + prevention + similar past

ENFORCEMENT:
  - Every completion claim MUST end with: GATES TRIGGERED: [...] | PROOFS: [Y/N] | SKIPPED: [reason]
  - Emergency bypass (Phase E): G0/5/6/7/8/9 deferred. G1/2/4 ALWAYS apply.
  - "Obvious enough to skip" IS the bias — write it out anyway
============================================================
CGPEOF
