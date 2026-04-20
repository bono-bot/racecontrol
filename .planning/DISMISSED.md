# DISMISSED — Work Items Consolidated Out

Ledger of work items **actively being rediscovered or re-done** that are in
fact already handled on `HEAD`, already landed in a workflow check, or whose
premise has been invalidated.

**Purpose:** stop spending cycles re-walking gaps that the Day 1/Day 2
board-state workflow (or prior commits) already cover. Each row records
enough evidence that a future session can verify without re-auditing.

**Appendment rules:**
- One row per dismissal, newest at the top.
- Every row cites a commit hash (or explicit "no code change, premise only").
- `replaced-by` points to the automated check or the superseding work that
  makes re-doing the item wasteful.
- Never dismiss something just because it's annoying; dismissal = "the
  problem was real, and has already been solved; further manual tracking
  is the drift."

---

## Ledger

| # | Dismissed on | Item | Memory/Handoff ref | HEAD evidence | Replaced by | Action taken |
|---|--------------|------|---------------------|---------------|-------------|---------------|
| 1 | 2026-04-20 17:00 IST | DPDP coverage-matrix hand-audit (session re-discovery of 9 FK gaps) | This session's "step (1) now + (2) as the fix" | `1257a1b5 fix(legal): DPDP erase — close 9 FK coverage gaps surfaced by Day 1 checker` + `e83fe292 fix(legal): visits nullable migration` + `4ab2e835 fix(legal): DPDP right-of-erasure no longer silent-fails or under-covers` | `scripts/audit/dpdp-coverage-check.py` + `.planning/board-state/status.json` `schema-dpdp: green` | No code change — session edits were duplicates of HEAD (`git diff HEAD` empty). Triage enumerated in [MEMORY.md Active Work] but not actionable. |
| 2 | 2026-04-20 17:00 IST | MI Gap 4 (pod `RCAGENT_SERVICE_KEY` ≠ server `pods.sentry_service_key` → rc-agent Tier 0 dead fleet-wide) | [session_handoff_20260418_mi_seed_verification_and_gap4.md](../../../.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_verification_and_gap4.md) | `9fb4a1a2` Phase 413.1 Plan 06 — Option Z fetch-at-boot + 300s periodic refetch. Pod 3 canary: "Mesh key cache initial fetch ok". | Code change shipped to fleet in 413.1-06; handoff frontmatter already reflects RESOLVED | MEMORY.md index line updated to reflect resolution; handoff file unchanged. |
| 3 | 2026-04-20 17:00 IST | Phase 373 AC Multiplayer Canary preflight + canary bundle (Pods 7+8) | [session_handoff_20260419_ac_mp_canary_plan.md](../../../.claude/projects/C--Users-bono/memory/session_handoff_20260419_ac_mp_canary_plan.md) | `519f0aa5 fix(kiosk): remove dead multiplayer scaffolding — mechanic does not exist in kiosk` (2026-04-20 15:29 IST) | Option B chosen by user = delete kiosk MP scaffolding. Canary premise (kiosk-triggered MP join from wizard → Pods 7+8) is invalidated because the mechanic never existed in kiosk to begin with. | Handoff banner added; MEMORY.md "Active Work — AC Multiplayer Canary" line rewritten to "DISMISSED — premise invalidated". |

---

## Items considered and NOT dismissed

These looked superficially similar but survive because they represent real
pending work, not drift:

- **AI Debugger GAP-2/8/9/10/11** — code fixes exist on HEAD (`de9b4108`, `45f31355`, `e51a9328`) but have **not been deployed** to the fleet yet. The handoff file is accurate. Keep tracking until the fleet-deploy row lands.
- **GAP-12 (pattern key coarseness)** — intentionally deferred per design note Option 12γ; existing safety rails self-limit. No dismissal, explicit deferral on file.
- **deploy-server.sh SWAPLOG append bug** — real infrastructure bug, not drift. 10 consecutive manual appends. **Promoted** to a Day 3 board-state check (`fleet-swaplog-parity` — see `scripts/audit/fleet-swaplog-parity-check.py`).
- **deploy-pod.sh SHA256 JSON-parse bug** — real infrastructure bug, rc-agent deploys continue to bypass it manually. Survives; candidate for future Day N check.
- **rc-sentry BLOCKED_PATTERNS `&&` silent-fail** — real infrastructure bug, caused 5-pod 3-min outage 2026-04-19. Survives.
- **Pattern A Pod 4 F1 25** and **Pattern E Pod 6 AC SHM** — blocked investigations on OPEN-PATTERNS.md. Pod 6 was swapped to `a13942f2` at 2026-04-20 04:06 IST, which may have lifted the Pattern E hold — needs re-verify, not dismissal.

---

## Protocol for future sessions

When you catch yourself about to audit a surface that smells like DPDP
(a matrix of FK edges, a coverage list, a "which tables does X touch"):

1. Run `bash scripts/audit/run-all-checkers.sh` first. If the relevant check
   is already green, the matrix already exists and further hand-audit is drift.
2. If a matrix is needed but no check exists, **write the checker**, not the
   matrix. The matrix rots the moment a migration lands; the checker does not.
3. If a premise-dependent plan is in memory and the premise is questioned,
   verify the premise against HEAD before executing the plan. If the premise
   fails, append a banner to the handoff and dismiss here.
