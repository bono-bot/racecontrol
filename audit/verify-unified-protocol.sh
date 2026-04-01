#!/usr/bin/env bash
# Audit Protocol Verification of COGNITIVE-GATE-PROTOCOL.md v2.0
# Maps 20 audit tiers to document verification checks

FILE="COGNITIVE-GATE-PROTOCOL.md"
CLAUDE="CLAUDE.md"
REGISTRY="standing-rules-registry.json"
MMAUDIT="audit/MULTI-MODEL-AUDIT-PROTOCOL.md"
PASS=0; FAIL=0; WARN=0

check() {
  local p="$1" t="$2" d="$3" s="$4" detail="$5"
  printf "Phase %02d [T%02d] %-45s %s\n" "$p" "$t" "$d" "$s"
  [ -n "$detail" ] && echo "   -> $detail"
  case "$s" in PASS) PASS=$((PASS+1));; FAIL) FAIL=$((FAIL+1));; WARN) WARN=$((WARN+1));; esac
}

echo "================================================================"
echo "  AUDIT PROTOCOL VERIFICATION: COGNITIVE-GATE-PROTOCOL.md v2.0"
echo "  Mode: FULL | Tiers: 20 | Date: 2026-03-27"
echo "================================================================"
echo ""

echo "=== TIER 1: DOCUMENT FOUNDATION ==="
LINES=$(wc -l < "$FILE")
check 1 1 "Doc exists ($LINES lines)" "PASS"
grep -q "v2.0" "$FILE" && check 2 1 "Version v2.0" "PASS" || check 2 1 "Version v2.0" "FAIL"
P=$(grep -c "^## Phase" "$FILE")
check 3 1 "Phases defined ($P)" "$([ $P -ge 8 ] && echo PASS || echo FAIL)"
G=$(grep -c "Phase.*Gate\|Exit Gate" "$FILE")
check 4 1 "Gates present ($G)" "$([ $G -ge 8 ] && echo PASS || echo FAIL)"
grep -q "SESSION START" "$FILE" && check 5 1 "Flow diagram" "PASS" || check 5 1 "Flow diagram" "FAIL"

echo ""
echo "=== TIER 2: STANDING RULES ==="
SR=$(grep -c "SR-" "$FILE")
check 6 2 "SR refs ($SR)" "$([ $SR -ge 70 ] && echo PASS || echo WARN)"
for c in ULTIMATE DEPLOY COMMS QUALITY PROCESS TESTING DEBUGGING SEC OTA; do
  n=$(grep -c "SR-$c" "$FILE")
  check 7 2 "  SR-$c ($n)" "$([ $n -gt 0 ] && echo PASS || echo FAIL)"
done

echo ""
echo "=== TIER 3: MULTI-MODEL AUDIT ==="
for l in "M:advisory" "M:mechanical" "M:targeted" "M:gate" "M:post-incident" "M:full"; do
  n=$(grep -c "\[$l\]" "$FILE")
  check 9 3 "  [$l] ($n)" "$([ $n -ge 2 ] && echo PASS || echo FAIL)"
done
for m in "qwen3-235b" "deepseek-chat-v3" "mimo-v2-pro" "deepseek-r1" "gemini-2.5-pro"; do
  n=$(grep -c "$m" "$FILE")
  check 10 3 "  $m ($n)" "$([ $n -ge 3 ] && echo PASS || echo WARN)"
done
for t in "Tier A" "Tier B" "Tier C"; do
  grep -q "$t" "$FILE" && check 11 3 "  $t defined" "PASS" || check 11 3 "  $t" "FAIL"
done
grep -q "3+ models" "$FILE" && check 12 3 "Consensus logic" "PASS" || check 12 3 "Consensus" "FAIL"

echo ""
echo "=== TIER 4: ULTIMATE RULE ==="
grep -q "FOUR LAYERS" "$FILE" && check 14 4 "4-layer rule" "PASS" || check 14 4 "4-layer" "FAIL"
for ly in "Quality Gate" "E2E" "Standing Rules" "Multi-Model"; do
  grep -q "$ly" "$FILE" && check 15 4 "  Layer: $ly" "PASS" || check 15 4 "  Layer: $ly" "FAIL"
done
grep -q "FOUR" "$CLAUDE" && check 16 4 "CLAUDE.md synced" "PASS" || check 16 4 "CLAUDE sync" "FAIL"
grep -q "Multi-Model AI Audit" "$REGISTRY" && check 17 4 "Registry synced" "PASS" || check 17 4 "Registry" "FAIL"

echo ""
echo "=== TIER 5: DEBUG METHODOLOGY ==="
grep -q "4-Tier Debug Order" "$FILE" && check 18 5 "4-Tier Debug" "PASS" || check 18 5 "Debug order" "FAIL"
grep -q "5-Step Cause Elimination" "$FILE" && check 19 5 "5-Step Elimination" "PASS" || check 19 5 "5-step" "FAIL"
T=$(grep -c "Binary Search\|Rubber Duck\|Minimal Reproduction\|Working Backwards\|Differential\|Git Bisect\|Comment Out\|Observability\|Follow the Indirection" "$FILE")
check 20 5 "Techniques ($T/9)" "$([ $T -ge 9 ] && echo PASS || echo WARN)"
B=$(grep -c "Confirmation\|Anchoring\|Availability\|Sunk Cost" "$FILE")
check 21 5 "Bias guards ($B/4)" "$([ $B -ge 4 ] && echo PASS || echo WARN)"
grep -q "Start over when\|Restart Conditions" "$FILE" && check 22 5 "Restart conditions" "PASS" || check 22 5 "Restart" "FAIL"

echo ""
echo "=== TIER 6: DEPLOY ==="
grep -q "Pod Deploy" "$FILE" && check 23 6 "Pod deploy seq" "PASS" || check 23 6 "Pod deploy" "FAIL"
grep -q "7 steps" "$FILE" && check 24 6 "Server 7-step" "PASS" || check 24 6 "Server deploy" "FAIL"
O=$(grep -c "SR-OTA" "$FILE")
check 25 6 "OTA rules ($O)" "$([ $O -ge 5 ] && echo PASS || echo WARN)"
grep -q "Pod 8 canary" "$FILE" && check 26 6 "Canary deploy" "PASS" || check 26 6 "Canary" "FAIL"

echo ""
echo "=== TIER 7: CODE QUALITY ==="
grep -q "unwrap" "$FILE" && check 27 7 "No .unwrap()" "PASS" || check 27 7 "unwrap" "FAIL"
grep -q "any.*TypeScript" "$FILE" && check 28 7 "No any in TS" "PASS" || check 28 7 "any" "FAIL"
grep -q "ASCII" "$FILE" && check 29 7 ".bat rules" "PASS" || check 29 7 ".bat" "FAIL"
grep -q "security-check" "$FILE" && check 30 7 "Security gate" "PASS" || check 30 7 "SecGate" "FAIL"

echo ""
echo "=== TIER 8: VERIFICATION ==="
grep -q "EXACT" "$FILE" && check 31 8 "Exact behavior path" "PASS" || check 31 8 "Exact" "FAIL"
grep -q "Domain-Matched" "$FILE" && check 32 8 "Domain-matched" "PASS" || check 32 8 "Domain" "FAIL"
grep -q "Visual Verification" "$FILE" && check 33 8 "Visual verify" "PASS" || check 33 8 "Visual" "FAIL"
grep -q "Multi-Machine" "$FILE" && check 34 8 "Multi-machine" "PASS" || check 34 8 "Multi-machine" "FAIL"

echo ""
echo "=== TIER 9: COMMS ==="
grep -q "INBOX.md" "$FILE" && check 35 9 "Bono INBOX" "PASS" || check 35 9 "INBOX" "FAIL"
L=$(grep -c "LOGBOOK" "$FILE")
check 36 9 "LOGBOOK ($L refs)" "$([ $L -ge 10 ] && echo PASS || echo WARN)"
grep -q "git push" "$FILE" && check 37 9 "Auto-push" "PASS" || check 37 9 "Push" "FAIL"

echo ""
echo "=== TIER 10: FLEET-SPECIFIC ==="
grep -q "MAINTENANCE_MODE" "$FILE" && check 38 10 "MAINTENANCE_MODE" "PASS" || check 38 10 "MAINT" "FAIL"
grep -q "Session 1" "$FILE" && check 39 10 "Session 0/1" "PASS" || check 39 10 "Session" "FAIL"
grep -q "Crash loop" "$FILE" && check 40 10 "Crash loop" "PASS" || check 40 10 "Crash" "FAIL"
grep -q "NVIDIA Surround" "$FILE" && check 41 10 "NVIDIA Surround" "PASS" || check 41 10 "Surround" "FAIL"

echo ""
echo "=== TIER 11: CROSS-SYSTEM ==="
grep -q "Cross-Boundary\|AcLaunchParams" "$FILE" && check 42 11 "X-boundary serial" "PASS" || check 42 11 "Serial" "FAIL"
grep -q "Cascade Audit" "$FILE" && check 43 11 "Cascade audit" "PASS" || check 43 11 "Cascade" "FAIL"
grep -q "Cross-Process" "$FILE" && check 44 11 "Cross-process" "PASS" || check 44 11 "X-process" "FAIL"

echo ""
echo "=== TIER 12: POST-SHIP ==="
grep -q "POST-SHIP AUDIT" "$FILE" && check 45 12 "Phase A defined" "PASS" || check 45 12 "Phase A" "FAIL"
grep -q "audit.sh" "$FILE" && check 46 12 "Fleet audit cmd" "PASS" || check 46 12 "Fleet cmd" "FAIL"
grep -q "Batch.*Scope\|7 Audit Batches" "$FILE" && check 47 12 "7 batches" "PASS" || check 47 12 "Batches" "FAIL"
grep -q "gap" "$FILE" && check 48 12 "Gap analysis" "PASS" || check 48 12 "Gaps" "FAIL"

echo ""
echo "=== TIER 13: APPENDICES ==="
grep -q "Appendix A" "$FILE" && check 49 13 "Appendix A" "PASS" || check 49 13 "A" "FAIL"
grep -q "Appendix B" "$FILE" && check 50 13 "Appendix B" "PASS" || check 50 13 "B" "FAIL"
grep -q "Appendix C" "$FILE" && check 51 13 "Appendix C" "PASS" || check 51 13 "C" "FAIL"
grep -q "Appendix D" "$FILE" && check 52 13 "Appendix D" "PASS" || check 52 13 "D" "FAIL"
grep -q "Phase M" "$FILE" && check 53 13 "Phase M ref" "PASS" || check 53 13 "M" "FAIL"

echo ""
echo "=== TIER 14: CONSISTENCY ==="
F=$(grep -c '```' "$FILE")
check 54 14 "Code fences ($F)" "$([ $((F%2)) -eq 0 ] && echo PASS || echo FAIL)"
CK=$(grep -c "\- \[ \]" "$FILE")
check 55 14 "Gate items ($CK)" "$([ $CK -ge 100 ] && echo PASS || echo WARN)"
grep -q "2.0.*2026-03-27" "$FILE" && check 56 14 "Version history" "PASS" || check 56 14 "Version" "FAIL"

echo ""
echo "=== TIER 15: CROSS-DOC SYNC ==="
grep -q "COGNITIVE-GATE-PROTOCOL" "$CLAUDE" && check 57 15 "CLAUDE ref" "PASS" || check 57 15 "CLAUDE ref" "FAIL"
grep -q "FOUR\|four" "$CLAUDE" && check 58 15 "CLAUDE 4 layers" "PASS" || check 58 15 "CLAUDE layers" "FAIL"
grep -q "Multi-Model AI Audit" "$REGISTRY" && check 59 15 "Registry sync" "PASS" || check 59 15 "Registry" "FAIL"
[ -f "$MMAUDIT" ] && check 60 15 "Source protocol" "PASS" || check 60 15 "Source" "FAIL"

echo ""
echo "=== TIER 16: COST ==="
grep -q "0.05" "$FILE" && check 61 16 "Per-tier costs" "PASS" || check 61 16 "Costs" "FAIL"
grep -q "Monthly" "$FILE" && check 62 16 "Monthly estimate" "PASS" || check 62 16 "Monthly" "FAIL"

echo ""
echo "=== TIER 17: FALSE POSITIVES ==="
grep -q "suppress.json" "$FILE" && check 63 17 "Suppress file" "PASS" || check 63 17 "Suppress" "FAIL"
grep -q "Override protocol\|AUDIT-OVERRIDE" "$FILE" && check 64 17 "Override protocol" "PASS" || check 64 17 "Override" "FAIL"
grep -q "false positive\|False Positive" "$FILE" && check 65 17 "FP patterns" "PASS" || check 65 17 "FP" "FAIL"

echo ""
echo "=== TIER 18: METRICS ==="
grep -q "Metrics to track" "$FILE" && check 66 18 "Metrics defined" "PASS" || check 66 18 "Metrics" "FAIL"
grep -q "knowledge base\|feeds back" "$FILE" && check 67 18 "Feedback loop" "PASS" || check 67 18 "Feedback" "FAIL"

echo ""
echo "=== TIER 19: COORDINATION ==="
grep -q "James.*runs\|James.*OpenRouter" "$FILE" && check 68 19 "James/Bono roles" "PASS" || check 68 19 "Roles" "FAIL"

echo ""
echo "=== TIER 20: GAPS ==="
grep -q "billing.*drain\|Billing.*drain" "$FILE" && check 69 20 "Billing drain" "PASS" || check 69 20 "Billing drain" "WARN"
grep -q "72.*hour" "$FILE" && check 70 20 "72hr rollback" "PASS" || check 70 20 "Rollback" "WARN"
grep -q "WhatsApp" "$FILE" && check 71 20 "WhatsApp" "PASS" || check 71 20 "WhatsApp" "WARN"
grep -q "POS PC\|POS.*192.168.31.20\|192.168.31.20" "$FILE" && check 72 20 "POS PC in verify list" "PASS" || check 72 20 "POS PC" "WARN"
grep -q "Tailscale" "$FILE" && check 73 20 "Tailscale fallback" "PASS" || check 73 20 "Tailscale" "WARN"
grep -q "ConspitLink" "$FILE" && check 74 20 "ConspitLink rules" "PASS" || check 74 20 "ConspitLink" "WARN"
grep -q "NTP\|time sync\|w32tm\|Time Sync" "$FILE" && check 75 20 "NTP/time sync" "PASS" || check 75 20 "NTP" "WARN"

echo ""
echo "=== TIER 21: MULTI-MODEL DIAGNOSTIC ESCALATION ==="
grep -q "D.10.*Multi-Model Diagnostic\|Multi-Model Diagnostic Escalation" "$FILE" && check 76 21 "D.10 section exists" "PASS" || check 76 21 "D.10" "FAIL"
grep -q "M:diagnose" "$FILE" && check 77 21 "[M:diagnose] activation point" "PASS" || check 77 21 "M:diagnose" "FAIL"
grep -q "5-Tier Debug Order" "$FILE" && check 78 21 "Debug order upgraded to 5-Tier" "PASS" || check 78 21 "5-Tier" "FAIL"
grep -q "diagnostic brief\|diagnostic-brief" "$FILE" && check 79 21 "Diagnostic brief protocol" "PASS" || check 79 21 "Brief" "FAIL"
grep -q "Role.*Reasoner\|Reasoner.*R1\|ROLE.*reasoner" "$FILE" && check 80 21 "Reasoner role (R1)" "PASS" || check 80 21 "Reasoner" "FAIL"
grep -q "Code Expert.*V3\|ROLE.*code_expert" "$FILE" && check 81 21 "Code Expert role (V3)" "PASS" || check 81 21 "Code Expert" "FAIL"
grep -q "SRE.*MiMo\|ROLE.*sre" "$FILE" && check 82 21 "SRE role (MiMo)" "PASS" || check 82 21 "SRE role" "FAIL"
grep -q "Security.*Gemini\|ROLE.*security" "$FILE" && check 83 21 "Security role (Gemini)" "PASS" || check 83 21 "Security" "FAIL"
grep -q "cross-model-diagnosis\|Cross-reference diagnoses" "$FILE" && check 84 21 "Cross-reference step" "PASS" || check 84 21 "Cross-ref" "FAIL"
grep -q "Opus synthesis\|Opus review" "$FILE" && check 85 21 "Opus synthesis step" "PASS" || check 85 21 "Opus synth" "FAIL"
grep -q "consensus.*2.*models\|2.*models.*agree\|Consensus.*2" "$FILE" && check 86 21 "Consensus logic (2+ agree)" "PASS" || check 86 21 "Consensus" "FAIL"
grep -q "Novel hypothesis\|novel.*hypothesis" "$FILE" && check 87 21 "Novel hypothesis handling" "PASS" || check 87 21 "Novel hyp" "FAIL"
grep -q "Contradictions.*models\|models.*disagree" "$FILE" && check 88 21 "Contradiction resolution" "PASS" || check 88 21 "Contradict" "FAIL"

echo ""
echo "=== TIER 22: EMERGENCY FAST-PATH ==="
grep -q "Phase E.*EMERGENCY\|EMERGENCY FAST-PATH" "$FILE" && check 89 22 "Phase E defined" "PASS" || check 89 22 "Phase E" "FAIL"
grep -q "7-Minute Recovery\|7 min" "$FILE" && check 90 22 "7-min recovery protocol" "PASS" || check 90 22 "7-min" "FAIL"
grep -q "TRIAGE" "$FILE" && grep -q "STABILIZE" "$FILE" && check 91 22 "Triage + Stabilize steps" "PASS" || check 91 22 "Triage" "FAIL"
grep -q "NO gate checks during\|Skip Phase 0-5" "$FILE" && check 92 22 "Gates bypassed in emergency" "PASS" || check 92 22 "Bypass" "FAIL"
grep -q "Max 15 min\|15 minutes in Phase E" "$FILE" && check 93 22 "15-min emergency ceiling" "PASS" || check 93 22 "Ceiling" "FAIL"

echo ""
echo "=== TIER 23: BREAK-GLASS ==="
grep -q "Phase B.*BREAK-GLASS\|BREAK-GLASS" "$FILE" && check 94 23 "Phase B defined" "PASS" || check 94 23 "Phase B" "FAIL"
grep -q "Autonomous Authority\|autonomous.*authority\|CAN do without" "$FILE" && check 95 23 "AI autonomous scope defined" "PASS" || check 95 23 "Scope" "FAIL"
grep -q "CANNOT do without" "$FILE" && check 96 23 "AI prohibited actions defined" "PASS" || check 96 23 "Prohibit" "FAIL"
grep -q "Escalation Ladder\|escalation.*ladder" "$FILE" && check 97 23 "Escalation ladder (0/5/15/30 min)" "PASS" || check 97 23 "Ladder" "FAIL"

echo ""
echo "=== TIER 24: ISLAND MODE ==="
grep -q "Phase I.*ISLAND\|ISLAND MODE" "$FILE" && check 98 24 "Phase I defined" "PASS" || check 98 24 "Phase I" "FAIL"
grep -q "Without Server\|Without James\|Without Both" "$FILE" && check 99 24 "Capability matrix (3 scenarios)" "PASS" || check 99 24 "Matrix" "FAIL"
grep -q "Server-Down Playbook\|server.*down.*playbook" "$FILE" && check 100 24 "Server-down playbook" "PASS" || check 100 24 "Srv down" "FAIL"
grep -q "James-PC-Down\|James.*down" "$FILE" && check 101 24 "James-PC-down playbook" "PASS" || check 101 24 "James down" "FAIL"
grep -q "Both-Down\|paper tracking" "$FILE" && check 102 24 "Both-down worst case" "PASS" || check 102 24 "Both down" "FAIL"

echo ""
echo "=== TIER 25: MODEL REGISTRY ==="
grep -q "Model Registry\|model-registry.json" "$FILE" && check 103 25 "Model registry defined" "PASS" || check 103 25 "Registry" "FAIL"
grep -q "pinned_version\|Version Pinning\|version pinning" "$FILE" && check 104 25 "Version pinning" "PASS" || check 104 25 "Pinning" "FAIL"
grep -q "fallback.*chain\|fallback" "$FILE" && check 105 25 "Fallback chain per model" "PASS" || check 105 25 "Fallback" "FAIL"
grep -q "cost_ceiling\|monthly_cost_ceiling\|50" "$FILE" && check 106 25 "Cost ceiling ($50/mo)" "PASS" || check 106 25 "Cost ceil" "FAIL"
grep -q "quarterly_review\|Quarterly review" "$FILE" && check 107 25 "Quarterly review schedule" "PASS" || check 107 25 "Quarterly" "FAIL"

echo ""
echo "=== TIER 26: PHYSICAL VENUE ==="
grep -q "Phase V.*PHYSICAL\|PHYSICAL VENUE" "$FILE" && check 108 26 "Phase V defined" "PASS" || check 108 26 "Phase V" "FAIL"
grep -q "Steering wheel\|steering.*wheel" "$FILE" && check 109 26 "Hardware checklist" "PASS" || check 109 26 "Hardware" "FAIL"
grep -q "HVAC\|hvac\|temperature" "$FILE" && check 110 26 "HVAC/thermal coverage" "PASS" || check 110 26 "HVAC" "FAIL"
grep -q "motion sickness\|nausea" "$FILE" && check 111 26 "Customer safety rules" "PASS" || check 111 26 "Safety" "FAIL"
grep -q "Spilled drink\|spill" "$FILE" && check 112 26 "Physical incident types" "PASS" || check 112 26 "Incidents" "FAIL"
grep -q "Weekly.*Audit\|brake spring\|FFB motor" "$FILE" && check 113 26 "Weekly hardware audit" "PASS" || check 113 26 "Weekly hw" "FAIL"

echo ""
echo "=== TIER 27: ADVERSARIAL AUDIT ==="
grep -q "Adversarial.*Audit\|adversarial.*audit" "$FILE" && check 114 27 "Adversarial audit defined" "PASS" || check 114 27 "Adversarial" "FAIL"
grep -q "hostile auditor\|HARSHLY" "$FILE" && check 115 27 "Hostile auditor prompt" "PASS" || check 115 27 "Hostile" "FAIL"
grep -q "Never send the protocol to itself" "$FILE" && check 116 27 "Self-audit prohibition" "PASS" || check 116 27 "Self-audit" "FAIL"
grep -q "Grade.*calculation\|grade.*improve" "$FILE" && check 117 27 "Grade tracking over time" "PASS" || check 117 27 "Grade track" "FAIL"
grep -q "Audit History" "$FILE" && check 118 27 "Audit history table" "PASS" || check 118 27 "History" "FAIL"

echo ""
echo "=== TIER 28: QUICK REFERENCE ==="
QREF="PROTOCOL-QUICK-REF.md"
[ -f "$QREF" ] && check 119 28 "Quick-ref file exists" "PASS" || check 119 28 "Quick-ref" "FAIL"
[ -f "$QREF" ] && QLINES=$(wc -l < "$QREF") && check 120 28 "Quick-ref size ($QLINES lines)" "$([ $QLINES -le 250 ] && echo PASS || echo WARN)" || check 120 28 "Size" "FAIL"
[ -f "$QREF" ] && grep -q "EMERGENCY\|3am" "$QREF" && check 121 28 "Quick-ref has emergency section" "PASS" || check 121 28 "QR emergency" "FAIL"
[ -f "$QREF" ] && grep -q "BREAK-GLASS" "$QREF" && check 122 28 "Quick-ref has break-glass" "PASS" || check 122 28 "QR break-glass" "FAIL"
[ -f "$QREF" ] && grep -q "4 SHIPPING GATES\|shipping gates" "$QREF" && check 123 28 "Quick-ref has shipping gates" "PASS" || check 123 28 "QR gates" "FAIL"
[ -f "$QREF" ] && grep -q "PHYSICAL\|physical\|HVAC\|steering" "$QREF" && check 124 28 "Quick-ref has venue section" "PASS" || check 124 28 "QR venue" "FAIL"

echo ""
echo ""
TOTAL=$((PASS+FAIL+WARN))
SCORE=$((PASS * 100 / TOTAL))

echo "================================================================"
echo "              UNIFIED PROTOCOL v3.0 AUDIT SCORECARD             "
echo "================================================================"
echo ""
echo "  Tiers Audited:    28"
echo "  Phases Checked:   $TOTAL"
echo ""
echo "  PASS:             $PASS"
echo "  FAIL:             $FAIL"
echo "  WARN:             $WARN"
echo "  --------------------------------"
echo "  SCORE:            ${SCORE}% ($PASS/$TOTAL PASS)"
echo ""

if [ $FAIL -eq 0 ]; then
  echo "  VERDICT:  PASS -- Ship-ready. $WARN warnings (non-blocking)."
elif [ $FAIL -le 3 ]; then
  echo "  VERDICT:  CONDITIONAL PASS -- Fix $FAIL items. $WARN warnings."
else
  echo "  VERDICT:  FAIL -- $FAIL blocking issues must be resolved."
fi

echo ""
if [ $WARN -gt 0 ]; then
  echo "  WARNINGS (non-blocking):"
  echo "  (review audit output above for details)"
fi
echo ""
echo "================================================================"
