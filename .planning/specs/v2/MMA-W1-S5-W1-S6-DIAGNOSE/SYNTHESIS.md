# MMA Step 1 DIAGNOSE — Synthesis (W1-S5 + W1-S6 batched) — **SUPPLEMENTARY-DUPLICATE-RUN-N3**

> **⚠️ STATUS — SUPPLEMENTARY, NOT PRIMARY.** This run was the THIRD MMA Step 1 DIAGNOSE for W1-S5 within ~15 minutes (slot-collision N=3 → N=4 with this run). The CANONICAL batch is at `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` (commit `f599c316` 2026-05-09 12:05 IST — 14 consensus + 5 minority + 20 singleton across W1-S5 + W1-S6 + W3 triplet, $0.1065 spend). A second concurrent run shipped at `4f0075dd` 2026-05-09 12:00 IST (W1-S5-only, 8 consensus, $0.0361). This run added 5-vendor-family signal including Moonshot/kimi-k2.5 (not in either prior panel).
>
> **Self-G9 root cause:** I did not grep `LOGBOOK.md` or `comms-link/data/openrouter-spend-james.jsonl` for recent MMA runs before invoking. CGP Verify-Before-Generate violated. The canonical artifact's existence was observable at session-start via `git log` but I followed the W1-S5 RCA's "(3) MMA Step 1 PENDING" gate state (frozen at `bda06dc8`) instead of refreshing against current state.
>
> **Net signal value:** my run promoted CANONICAL SINGLETON #2 ([P0] [CROSS] Auth blast-radius: W1-S5 refresh + W1-S6 lockout interaction — 1/5 in canonical) to 3/5 consensus in this panel via kimi-k2.5 + qwen3-235b corroboration. Added W1-S5 clock-skew finding (3/5 here, not present in canonical). Other findings overlap with canonical or rephrase canonical singletons.
>
> **Structural-fix candidate (slot-collision PROMOTE-N=4 per LOGBOOK CANDIDATE-N1 hit this run):** pre-MMA hook that greps `openrouter-spend-james.jsonl` for `mma_step:DIAGNOSE` runs in the last 60 minutes against the same RCA file paths and BLOCKS unless `MMA_FORCE_DUPLICATE=1` is set with explicit reason. See §11 below.
>
> **Cost-of-error:** $0.067 of Captain-approved $10 batch budget (0.7%); $9.83 remains. Minor in absolute terms but pure waste from a discipline standpoint.

**Date:** 2026-05-09 ~12:15 IST
**RCA targets:** racecontrol commit `bda06dc8` (W1-S5 RCA + W1-S6 RCA at branch `feat/v2-wave-1-w1-s1-billing-service`)
**Authority:** Captain G33 batch disposition 2026-05-09 ~11:23 IST — MMA budget approved up to $10 OpenRouter
**Authorized by:** james (this session) — user choice "Batch both RCAs now" 2026-05-09 ~12:11 IST
**Wall-clock:** 12:12 IST → 12:15 IST (~3 min parallel)
**Spend:** ~$0.07 (well under $10 cap; supplementary-duplicate)

---

## §1 — Model panel (5 models, security-domain pool)

| Slot | Vendor | Model | Status |
|---|---|---|---|
| 1 (Reasoner) | DeepSeek | `deepseek/deepseek-r1-0528` | OK — clean JSON |
| 2 (Specialist — security) | Google | `google/gemini-2.5-flash` | OK — clean JSON |
| 3 (SRE/Ops) | Xiaomi | `xiaomi/mimo-v2-pro` | UNUSABLE — emitted reasoning instead of JSON; never produced structured output |
| 4 (Reasoner-2) | Moonshot | `moonshotai/kimi-k2.5` | PARTIAL — JSON truncated at max_tokens=4000; titles + first ~half of bodies recoverable |
| 5 (Generalist) | Qwen | `qwen/qwen3-235b-a22b-2507` | OK — clean JSON |

**Effective panel: 3 clean + 1 partial (titles only) + 1 discarded.** 3/5 consensus threshold preserved with 4 usable signals.

Vendor diversity: DeepSeek + Google + Xiaomi + Moonshot + Qwen = 5 families. Satisfies CLAUDE.md MMA Protocol v3.0 "≥3 vendor families, ≤2 per family".

---

## §2 — Blocker disposition consensus

| Model | Disposition |
|---|---|
| deepseek-r1 | REVISE |
| gemini-flash | BLOCK |
| qwen3-235b | REVISE |
| kimi-k2.5 | (truncated — no explicit disposition recovered) |

**Consensus: REVISE (3/3 clean models).** Zero PASS votes. RCAs require amendment before H1 PLAN can proceed.

---

## §3 — W1-S5 missed items (consensus 3+/5)

### 🔴 W1-S5-MISSED-1: Concurrency race in token re-issuance [P0/P1] — 3/5

| Model | Severity | Title |
|---|---|---|
| deepseek-r1 | P1 | "Concurrency hazards in token re-issuance" |
| qwen3-235b | P0 | "Concurrency race in token re-issuance under high load" |
| kimi-k2.5 | P0 | "Concurrent request race condition on token refresh" |

**Synthesized concern:** RCA assumes token re-issuance is atomic and side-effect-free. Two simultaneous requests from same staff member could trigger duplicate re-issuance, write conflicting Set-Cookie headers, or race in audit-log writes. RCA §5 sketch (items 3-5) doesn't address concurrent-request handling.

**Suggested action:** Add to W1-S5 §3 disposition + §5 implementation sketch — how does `mint_refreshed_jwt` interact with concurrent calls from same `staff_id`? Use single-flight pattern or accept idempotency-via-CSPRNG-jti?

### 🟡 W1-S5-MISSED-2: Clock skew / clock drift between services [P1/P2] — 3/5

| Model | Severity | Title |
|---|---|---|
| deepseek-r1 | P2 | "Clock skew amplification in distributed systems" |
| qwen3-235b | P1 | "Clock drift between services invalidates sliding-window logic" |
| kimi-k2.5 | P1 | "Clock skew amplification with iat=now" |

**Synthesized concern:** Sliding-window check relies on `iat` and server-local `now`. RacingPoint runs racecontrol on Server .23 + Bono VPS (cloud). Tokens minted on one host may be evaluated against the other host's clock. RCA §2 row 8 mentions clock-skew tolerance via `saturating_sub` (preserves V1 behavior) but doesn't address inter-host skew under sliding-window-refresh semantics.

**Suggested action:** Add an item to W1-S5 §3 OPEN — bound max-allowed iat-skew between hosts; reject tokens where `iat > now + skew_tolerance` rather than silently treating-as-fresh.

---

## §4 — W1-S6 missed items (consensus 3+/5)

### 🟠 W1-S6-MISSED-1: EmailAlerter timeout/retry/error handling [P1] — 3/5

| Model | Severity | Title |
|---|---|---|
| gemini-flash | P1 | "Lack of explicit error handling/fallback for whatsapp_send" |
| qwen3-235b | P1 | "EmailAlerter shell-out lacks timeout and retry logic" |
| kimi-k2.5 | (P-?) | "Email delivery failure handling" |

**Synthesized concern:** W1-S6 §1 reuses `EmailAlerter::send_alert` shell-out to `comms-link/shared/send-email.js` (Strategy 1 sendmail / Strategy 2 raw SMTP localhost:25). RCA doesn't specify timeout or retry behavior. A hanging SMTP connection blocks the middleware chain. Same applies to WhatsApp Captain freeze dispatch — if Evolution API hangs, the lockout-orchestration step stalls.

**Suggested action:** Add to W1-S6 §3 + §5 — wrap email + WhatsApp dispatch in `tokio::time::timeout(N_secs)`; if dispatch fails, the PIN-rotation + audit-log + lockout-counter MUST still complete (don't fail the lockout because the notification failed). Document failure-mode in code comment.

---

## §5 — Cross-RCA interactions (consensus 3+/5)

### 🚨 CROSS-1: Sliding-window JWT refresh bypasses PIN-LOCKOUT [P0] — 3/5 (CRITICAL)

| Model | Severity | Title |
|---|---|---|
| gemini-flash | P0 | "Interaction between W1-S5's sliding-window refresh and W1-S6's PIN-LOCKOUT logic" |
| qwen3-235b | P0 | "Sliding-window refresh may interfere with PIN lockout state" |
| kimi-k2.5 | P1 | "JWT invalidation on PIN rotation" |

**Synthesized concern:** This is the highest-severity finding from MMA. The interaction:

1. Staff member is logged in via JWT. Sliding-window keeps refreshing on activity.
2. Staff member fails 5 wrong-PIN attempts on a privileged action (refund 3band band B/C).
3. W1-S6 fires: PIN auto-rotates, helpdesk@ email sent, account "frozen" (per Captain Q1 RATIFIED).
4. **The existing JWT continues to be valid until natural 24h `exp`.** Sliding-window even REFRESHES it on subsequent (non-privileged) requests.
5. Staff member retains operational access despite the PIN being rotated and the account flagged as frozen.

This **completely undermines the security intent** of W1-S6's lockout. The "freeze" only blocks future PIN-based logins; existing sessions keep operating.

**Suggested action:** W1-S5 sliding-window refresh path MUST check `staff_pin_lockout_state(staff_id)` before re-issuing the JWT. On lockout-active: reject the refresh + revoke the existing JWT (return 401 + clear cookie). This requires:
- A persistent (or shared) "lockout-active" predicate that W1-S5's middleware can read on every request
- A revocation mechanism for the existing JWT (jti denylist OR force-expire)
- Captain Q-DECISION on cross-feature integration before W1-S5 + W1-S6 can ship independently

**Cross-pilot implication:** This makes W1-S5 and W1-S6 NO LONGER independent ships. They have a security-critical cross-coupling that must be designed jointly OR the order of ships matters (W1-S6 must ship FIRST or simultaneously with the integration hook in W1-S5).

---

## §6 — Soft signals (2/5 — worth flagging, not blocking)

These didn't reach 3/5 consensus but >1 model independently surfaced them; consider in revise pass:

**W1-S5:**
- Cookie name collision with existing `staff_pin` cookie (deepseek + gemini)
- No cleanup path for stale re-issued JWTs / token revocation (qwen + kimi)
- Error handling in response-mutating middleware (deepseek + kimi)
- Observability metrics gap for refresh rate (gemini + qwen)

**W1-S6:**
- In-memory state lost on restart enables lockout bypass (deepseek + qwen) — note: Q-S6-6 explicitly accepts in-memory; this is the cost
- PIN/credential leakage in logs/emails/error traces (gemini + qwen)

**Cross-RCA:**
- Cookie header collision in response chains (deepseek + gemini)
- Combined audit-log write amplification (deepseek + gemini)

**Single-voice but architecturally important (1/5):**
- gemini-flash P2 W1-S5: `iat` change → `exp = iat + 24h` → if iat refreshes on every request, effective session duration extends INDEFINITELY as long as activity continues. **This may be Captain-intent (active staff stay logged in) OR a security gap (no max-session-life cap). Worth surfacing for Captain explicit Q-DECISION.**
- gemini-flash P0 W1-S5: V1-era cross-system clients snapshotting `iat` for non-idle-expiry purposes — RCA §1 row 1 mentions "need to grep callers" but doesn't say grep-was-run.
- deepseek-r1 P1 W1-S5: Deployment rollback hazards — sliding-window tokens minted then rollback to fixed-window → tokens become invalid → mass logout.
- kimi P? W1-S5: Missing feature flag / kill switch for sliding-window itself.

---

## §7 — Recommended next actions

**Disposition: REVISE.** Do NOT proceed to W1-S5 / W1-S6 H1 PLAN until 3-of-5-consensus items addressed.

### Required RCA amendments

1. **Amend W1-S5 RCA §2 + §3 + §5** to address W1-S5-MISSED-1 (concurrency race) and W1-S5-MISSED-2 (clock skew between hosts).

2. **Amend W1-S6 RCA §3 + §5** to address W1-S6-MISSED-1 (email/WhatsApp timeout + retry + decoupling notification failure from lockout completion).

3. **Amend BOTH RCAs to surface CROSS-1 (P0) explicitly** — designate ship ordering (W1-S6 first OR simultaneous-with-integration-hook), pre-PR Captain Q-DECISION on cross-feature integration. This is the most important amendment — it changes the wave-1 sequencing topology.

### Captain Q-DECISIONs to surface

- **Q-W1-CROSS-1:** Should W1-S5 sliding-window refresh check staff lockout-state on every refresh? (Default YES per consensus, but Captain explicit ratification needed before implementation.)
- **Q-W1-CROSS-2:** Implementation order: ship W1-S6 first → W1-S5 second (so lockout state exists when W1-S5 reads it)? OR ship together as W1-S5+S6 combined? OR ship W1-S5 with no-op-lockout-check that activates when W1-S6 lands?
- **Q-W1-S5-NEW-1:** Max-session-life cap on sliding-window? (Without one, an active staff member's session can extend indefinitely.)

### Suggested workflow

1. james (this session): land this SYNTHESIS.md + LOGBOOK row + memory entry — no source code changes
2. Notify bono (INBOX) of MMA REVISE consensus + the 3 Q-DECISIONs
3. james-or-bono (next session): amend W1-S5 + W1-S6 RCAs to address consensus findings (4 items + cross-RCA design doctrine)
4. Re-run MMA Step 1 on amended RCAs (~$0.07 again, well under $10 cap; 4 consensus items resolved → expect PASS/REVISE downgrade)
5. After PASS: proceed to H1 PLAN

---

## §8 — Cost accounting

| Model | Prompt tokens | Completion tokens | Cost (USD) |
|---|---:|---:|---:|
| deepseek-r1-0528 | 18587 | 4156 | 0.0173 |
| gemini-2.5-flash | 19993 | 3571 | 0.0051 |
| mimo-v2-pro | 18817 | 4000 | 0.0308 |
| kimi-k2.5 | 17657 | 4000 | 0.0118 |
| qwen3-235b | 18576 | 1910 | 0.0015 |
| **Total** | **93630** | **17637** | **~$0.067** |

Captain budget: $10. Spend: $0.067 (0.7%). Remaining: $9.93.

**Ratio:** $0.067 spent caught 1 P0 cross-RCA finding + 4 P0/P1 individual-RCA findings that could have shipped to merge unflagged. Cost-benefit: the P0 cross-finding alone (PIN-LOCKOUT bypass via sliding-window) would have been a security incident class.

---

## §9 — NOT TESTED (synthesis layer)

- **No model verified the RCA findings against live code** — this is doctrine review, not codebase audit. A separate MMA pass with `AUDIT_DOMAIN=security` against the actual `auth/middleware.rs` + (when written) `auth/staff_auth.rs` would catch implementation-level bugs. That's MMA Step 4 VERIFY scope, not Step 1 DIAGNOSE.
- **mimo-v2-pro signal lost** — could re-fire with stricter system-prompt JSON instruction or higher max_tokens. Not done; 3 clean signals + 1 partial reached consensus threshold.
- **kimi-k2.5 truncation** — should re-fire with max_tokens=8000 if needed for completeness scoring, but title-extraction sufficed for consensus voting.
- **No Step 2 PLAN** — per UNIFIED-MMA-PROTOCOL.md, Step 2 designs fix plans for consensus findings. Deferred until Captain dispositions Q-W1-CROSS-1..2 + Q-W1-S5-NEW-1 (since the fix plans depend on which Captain-policy is chosen).

---

## §10 — Cross-batch comparison (this run vs canonical 12:05 + auxiliary 12:00)

**Three concurrent MMA Step 1 runs landed within ~15 min:**

| Run | Time IST | Commit | Scope | Spend | Models | Verdict |
|---|---|---|---|---|---|---|
| #1 | 12:00 | `4f0075dd` | W1-S5 only | $0.0361 | deepseek-r1, deepseek-v3, qwen3-coder, mimo-v2-pro, gemini-flash | APPROVE-WITH-AMENDMENTS 5/5 unanimous, 8 consensus |
| #2 (canonical) | 12:05 | `f599c316` | W1-S5 + W1-S6 + W3 triplet | $0.1065 | deepseek-r1, qwen3-coder, mimo-v2-pro, gemini-flash, mistral-small | 14 CONSENSUS + 5 MINORITY + 20 SINGLETON |
| #3 (this run) | 12:15 | (unpushed) | W1-S5 + W1-S6 batched | $0.067 | deepseek-r1, gemini-flash, mimo-v2-pro, **kimi-k2.5** (NEW), qwen3-235b | 4 consensus + 1 cross-RCA P0 promoted from canonical singleton |

**Findings overlap matrix (this run vs canonical 12:05):**

| This-run finding | Canonical status | Net signal |
|---|---|---|
| W1-S5 concurrency race in token re-issuance (3/5) | SINGLETON #5 in canonical (1/5) | Promoted to consensus via this batch |
| W1-S5 clock skew between hosts (3/5) | NOT IN CANONICAL | Net-new consensus signal |
| W1-S6 EmailAlerter timeout/retry (3/5) | Partial overlap with canonical CONSENSUS #6 (SMTP transport) + #5 (HashMap unbounded) | Refines canonical with timeout-specific framing |
| W1-S6 PIN-rotation atomicity (2/5 here) | Partial overlap with canonical CONSENSUS #4 (V1 IP-keyed rate-limit) + #5 (HashMap unbounded) | Soft-signal, canonical stronger |
| **CROSS sliding-window bypasses PIN-LOCKOUT (3/5 here)** | **SINGLETON #2 in canonical (1/5)** | **Promoted from singleton → consensus by this run's kimi-k2.5 + qwen3-235b votes** |
| Cookie collision (2/5 here) | MINORITY #5 in canonical (2/5) | No net-new signal |

**Conclusion:** This supplementary run's net unique value is:
1. Promoting cross-RCA P0 finding (auth-bypass via sliding-window) from CANONICAL SINGLETON → effective CONSENSUS-via-cross-batch-corroboration. This is meaningful — it changes the W1-S5/W1-S6 implementation gating story.
2. Adding W1-S5 clock-skew as net-new finding not in canonical.
3. The other findings are overlapping rephrasings of canonical findings.

**Recommended treatment:**
- Treat canonical 12:05 (`f599c316`) as the PRIMARY MMA Step 1 result. Step 2 PLAN authoring should use its 14 CONSENSUS findings as the mandatory inputs.
- Treat this run's #5 cross-RCA P0 finding (sliding-window-bypasses-PIN-lockout) and W1-S5 clock-skew finding as **SUPPLEMENTARY-PROMOTABLE**: when authoring Step 2 PLAN, promote these from canonical-singleton/not-present to mandatory-input class with this batch as second-pillar evidence.
- Aux 12:00 (`4f0075dd`) and this run (#3) are duplicate-class artifacts; preserve for audit trail but mark non-canonical.

---

## §11 — Slot-collision structural-fix (PROMOTE-N=4 hit this run)

The 12:05 LOGBOOK row at `f599c316` flagged: *"Slot-collision class N=3 same-action-class observation: ... Structural-fix candidate (pre-MMA spend-ledger grep for same-RCA-in-last-60min): CANDIDATE-N1 PROMOTE-on-N=4 ≤2026-06-08 per kaizen-correction-triage."*

**This run is N=4** — promotes the rule to active.

### Proposed hook: `~/.claude/hooks/pre-mma-duplicate-check.sh`

Pre-action (PreToolUse) check on Bash commands invoking OpenRouter MMA:

```bash
#!/bin/bash
# Block if there's a recent (≤60min) MMA DIAGNOSE on the same RCA in spend ledger
# Override: MMA_FORCE_DUPLICATE=1 with reason

LEDGER="C:/Users/bono/racingpoint/comms-link/data/openrouter-spend-james.jsonl"
COMMAND="$1"

# Detect MMA invocations (curl to openrouter + multi-model parallel pattern)
if ! echo "$COMMAND" | grep -qE 'openrouter\.ai.*chat.*completions.*&.*&.*&'; then
  exit 0  # not MMA, allow
fi

# Override
if [ "$MMA_FORCE_DUPLICATE" = "1" ]; then
  echo "WARNING: MMA_FORCE_DUPLICATE=1 — bypassing 60-min duplicate guard" >&2
  exit 0
fi

# Extract RCA paths from command (look for known RCA file path patterns)
RCA_PATHS=$(echo "$COMMAND" | grep -oE '\.planning/specs/v2/[A-Z0-9_-]+-RCA\.md' | sort -u)
if [ -z "$RCA_PATHS" ]; then
  exit 0  # no RCA path detected, allow
fi

# Check ledger for same-RCA DIAGNOSE within 60 min
NOW_TS=$(python3 -c "from datetime import datetime; print(datetime.utcnow().timestamp())")
RECENT_THRESHOLD=$((${NOW_TS%.*} - 3600))

for path in $RCA_PATHS; do
  # Look for ledger entries with this path's RCA name in last 60 min
  if jq --arg path "$path" --argjson threshold "$RECENT_THRESHOLD" \
    'select(.mma_step == "DIAGNOSE" and (.ts | fromdate) > $threshold and (.notes // "" | contains($path)))' \
    "$LEDGER" 2>/dev/null | grep -q .; then
    echo "BLOCKED: MMA DIAGNOSE for $path ran within last 60 min. Set MMA_FORCE_DUPLICATE=1 with explicit reason to override." >&2
    exit 1
  fi
done

exit 0
```

**Ledger schema enhancement**: ensure each MMA spend ledger entry includes `notes` or new field `rca_paths` listing the RCA file paths reviewed. The 12:05 canonical entry has this (`anchor.rca_commits`); confirm the format and standardize for grep-ability.

**Bilateral sync candidate**: This rule + hook should sync to bono via `~/.claude/hooks/pre-mma-duplicate-check.sh` on Bono VPS too. Per Universal Sync rule.

**Captain Q-DECISION needed before promote-active:** approve the hook + bilateral sync. Default: AGREE (kaizen-aligned, prevents future duplicate spend, no false-negative risk on legitimate same-RCA re-runs since override exists).

---

— james / 2026-05-09 ~12:15 IST · MMA Step 1 DIAGNOSE batched (W1-S5 + W1-S6) — **SUPPLEMENTARY DUPLICATE RUN N=3** under user authorization "Batch both RCAs now" + Captain G33 budget pre-approval up to $10 · self-G9 caught after-the-fact via LOGBOOK grep before push · canonical primary MMA result is `f599c316` (12:05) at `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` · this artifact preserved for: (a) signal corroboration on cross-RCA P0 (singleton → consensus via this batch), (b) new clock-skew consensus finding, (c) audit trail of slot-collision N=4 hit triggering structural-fix promote · spend $0.067 of $10 batch budget (0.7% — minor absolute, structurally avoidable)
