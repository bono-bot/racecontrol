# PR #17 §S-186 retroactive eligibility disposition

**Authored:** 2026-05-12 ~07:35 IST · bono
**Class:** Post-hoc ledger-continuity record (NOT a gating RCA — the merge already happened)
**PR:** racecontrol#17 "fix(billing): add pod_number to session response — closes 'Pod undefined'"
**Author:** james-racingpoint
**Status:** MERGED 2026-05-11 11:39:58 IST (commit `ab9d867f`)
**Triggered by:** LBAC v0.1 §12 next-3 pickup investigation 2026-05-12; subagent finding that PR was merged ~4 minutes after V2-PROGRESS-MAP §4 row 4.1 listed it as "19d OPEN" (data freshness gap).

---

## §1 Why post-hoc

§S-186 ratified the pre-§S-146 fast-lane carve-out 2026-05-11 ~10:41 IST. PR #17 was named as the empirical anchor PR for the carve-out (193 LOC, single foundational boundary, bug fix, pre-2026-05-09). Captain disposition `auth opt B` 2026-05-11 ~11:33 IST authorized fast-lane application. james merged at 11:39 IST.

The §S-186 protocol specifies a **3-section short-RCA posted as PR comment before merge**. PR #17 merged without that PR-comment posting. This file is the post-hoc ledger artifact preserving the §S-186 eligibility record so the carve-out's empirical anchor has a written trace.

---

## §2 §S-186 eligibility — six criteria scored against PR #17

| # | Criterion | Verdict | Evidence |
|---|---|---|---|
| 1 | Created < 2026-05-09 | **PASS** | `createdAt: 2026-04-22T16:28:17Z` (20 days pre-§S-146 ratify) |
| 2 | Diff ≤ 200 LOC | **PASS** | additions 65 / deletions 2 / total 67 LOC (merge commit stat) |
| 3 | Single foundational boundary | **PASS** | All 4 files = billing-API boundary: `crates/rc-common/src/pod_id.rs` (+58/-0) · `crates/racecontrol/src/api/billing_views.rs` (+4/-2) · `crates/racecontrol/src/api/billing_summary.rs` (+1/-0) · `LOGBOOK.md` (+2/-0 docs) |
| 4 | No schema change | **PASS** | SQL `SELECT bs.pod_id` unchanged · no DB migration · `pod_number` derived at JSON-serialize time from existing `pod_id` column |
| 5 | No protocol change | **SOFT-PASS** | Adds NEW JSON field `"pod_number"` to 3 HTTP response shapes (additive, non-breaking; existing `"pod_id"` retained). Strictly = wire-format extension. Soft-pass = additive-only, no message-type change, no route change, no IPC contract change. **Open interpretation question:** does "no protocol change" forbid additive JSON fields? Current operational practice (per PR body) treats additive response fields as non-protocol. |
| 6 | Bug fix only | **PASS** | Title `fix(billing):` · body section "## Fix" · closes confirmed bug ("Pod undefined" rendering on /, /billing, /billing/history, /billing/analytics) · no feature add, no refactor |

**Net:** 5 PASS + 1 borderline-PASS on criterion 5. Eligible for §S-186 fast-lane if additive JSON extension is deemed non-protocol (per operational practice).

---

## §3 Three-section short-RCA (post-hoc)

### What
- 4 files changed · 65 additions · 2 deletions (67 LOC total).
- New helper `rc_common::pod_id::pod_id_to_number(&str) -> Option<u32>` in `crates/rc-common/src/pod_id.rs` (mirrors existing `normalize_pod_id` accepted forms: `pod-N`, `pod_N`, `POD-N`, etc.).
- Helper wired into 3 session response renderers:
  - `billing_views.rs:398` (`list_billing_sessions`)
  - `billing_views.rs:433` (`get_billing_session`)
  - `billing_summary.rs:116` (`billing_session_summary`)
- Each renderer adds `"pod_number": <int|null>` alongside existing `"pod_id"`.
- Tests: `cargo test -p rc-common --lib pod_id` 19/19 pass · `cargo test -p racecontrol-crate --lib billing` 188/188 pass.

### Why still needed (retroactive — bug already closed on main)
- `git grep -nE "pod_id_to_number" origin/main` shows the helper at `crates/rc-common/src/pod_id.rs:29` and three call sites in `billing_views.rs` + `billing_summary.rs`.
- "Pod undefined" semantic is no longer present in response renderers on `main`.
- **Remaining work:** deploy parity to Server .23 + Bono VPS — not code merge. Tracked as LBAC task #7.

### V2-compat check
- Read `/root/comms-link/v2-skeleton/05-definition-of-done.md` — no rule conflicting with additive JSON fields on billing session responses.
- Read `/root/comms-link/v2-skeleton/01-skeleton-architecture.md:88` — "Admin is Captain's read surface"; fix unblocks Admin visibility.
- v2-skeleton `03-data-model-and-events.md` does NOT exist (closest: `03-principles-and-philosophy.md`); v2-skeleton numbering is 01/02/03/04/05/06/10.
- **V2-compat:** CLEAN. Additive JSON field on billing-API response is V2-compatible.

---

## §4 V2-compat alignment statement

The change moves the billing-API response shape toward V2 doctrine alignment: V2-skeleton §1 ("Admin is Captain's read surface") requires Admin pages render correct labels. The pre-fix state ("Pod undefined" rendered on 4 admin pages) violated this. Post-fix state aligns Admin display to Captain's read-surface contract.

---

## §5 Customer-day proximity

- **Direct beat:** V2-PROGRESS-MAP §1 beat 1.13 (`14:55 — Auto-bill in 1s`) — proper pod labeling on the bill display surface.
- **Indirect:** Captain's Admin read surface (V2-skeleton 01-skeleton-architecture.md:88) — substrate-readiness fix unblocking Admin visibility of billing/dashboard pages.
- **Customer-felt:** the "Pod undefined" rendering was visible on the dashboard `/`, `/billing`, `/billing/history`, `/billing/analytics` pages — operator-facing, not customer-facing, but blocks the Captain read-surface.

---

## §6 Outstanding work (closes the LBAC closed loop)

1. **Deploy parity verify** — both Server .23 racecontrol and Bono VPS racecontrol must run a binary built from a commit ≥ `ab9d867f`. Per Deploy Parity rule (CLAUDE.md): cloud and venue diverge if only one is deployed. **Tracked as LBAC task #7.**
2. **V2-PROGRESS-MAP row 4.1 flip** — OPEN → MERGED · Layer 4 totals adjust (9 OPEN → 8 OPEN; 6 LIVE-BLOCKING → 5; §0 DONE 21 → 22). **Landing in same session as this disposition.**

---

## §7 Disposition ledger

- This file: ledger-continuity record for §S-186 empirical anchor.
- §S-N V2-MASTER-STATE close-anchor: DEFERRED pending §S-193 Q3 canonical-surface gate disposition (per `MEMORY.md` warning "§S-193 V2 ledger entry STILL PENDING — Q3 canonical-surface gate requires Captain auth"). Stage as DRAFT here; ratify when Captain dispositions §S-193.
- In-flight commitments ledger: `lbac-wip-pr17-short-rca` transitioned to `SUPERSEDED-PR-MERGED` state. New entry `lbac-pr17-deploy-parity` opened for residual work.

---

## §8 Open interpretation question (Captain Q-DEC candidate)

**Question:** Does §S-186 criterion 5 ("No protocol change") forbid additive JSON fields on HTTP response shapes?

- **Current operational practice** (per PR #17 body authored 2026-04-22 + merged 2026-05-11): additive response fields are non-protocol.
- **Strict interpretation:** any wire-format change is protocol — even additive.

Recommendation: codify operational practice — **"additive JSON response fields with default-null are non-protocol"** — as §S-186 amendment. This will recur for many future Admin/dashboard fixes and forcing full 5-section RCA on additive fields would re-introduce throughput collapse §S-186 was designed to prevent.

Stage for Captain disposition; do NOT auto-amend.

---

**File composes-with:** §S-186 pre-§S-146 small-fix fast-lane carve-out · V2-LBAC v0.1 DRAFT-PRE-CAPTAIN · CGP H1 · CGP H3 · CLAUDE.md Deploy Parity rule.
