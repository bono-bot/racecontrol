# Phase 6 Backfill — Mechanism-Trust Audit Summary

**Date**: 2026-05-10 IST · **Auditor**: bono · **Phase**: 6 of §S-174 4-phase enforcement plan

## Overview

9 V2-foundational-boundary delivery surfaces audited under the §S-174 mechanism-trust-check upstream rule. Results inform what work needs to land next on the V2 reliability layer and act as the seed cache for the Phase 1 hook (`pre-v2-edit-rca-check.js`).

## Results

| Surface | Class | Verdict | Failing questions | Strongest evidence |
|---|---|---|---|---|
| **deploy-pod** | delivery-mechanism | **FAIL** | All 5 NO | §S-174 empirical anchor; MMA-DEPLOY-RCA-DIAGNOSE 9 CONSENSUS findings |
| **rc-agent** | pod-state-channel | **FAIL** | atomic / TTL / dry-run / contracts (4 NO) | PR #66 closed silent-loop-death class; remaining V1-shape lives in delivery |
| **rc-watchdog** | pod-state-channel | **FAIL** | All 5 NO | CF-9 NOVEL P0/P1 finding (deploy-aware health checks missing) |
| **rc-sentry** | pod-state-channel | **FAIL** | atomic / behavioral / dry-run / contracts (4 NO + 1 N/A) | CF-4 + CF-5 + CF-6 cluster |
| **fleet-health-api** | pod-state-channel | **FAIL** | TTL / behavioral / contracts (3 NO) | §S-150 anchor: silent_reconnect_suspected flag never auto-clears |
| **wallet** | wallet | **FAIL** | atomic / behavioral / contracts (3 NO) | W3 RCA `78f82654`; F-05 PATCHED-ONLY anti-pattern |
| **auth** | auth | **PARTIAL** | guard_contracts (1 NO + 4 YES) | W1-S5 + W1-S6 RCAs in flight; cross-coupling Q-CROSS-1 BLOCKING |
| **billing** | billing | **PASS** | 5 YES (or 4 YES + 1 N/A) | F25a substrate at `9f1c0a37` is the V2-aligned exemplar |
| **comms-link-exec-protocol** | transport | **PASS** | 5 YES (or 4 YES + 1 N/A) | PACT-021 substrate-immutability + PACT-022 layer-distinction |

**Tally**: 6 FAIL · 1 PARTIAL · 2 PASS.

## Headline findings

### F1 — Pod-state-channel layer is uniformly V1-shaped (4/4 FAIL)

`rc-agent` (excluding the PR #66 closure of silent-loop-death) + `rc-watchdog` + `rc-sentry` + `fleet-health-api` all fail the audit. The class of V1 anti-patterns (manual ops bypassing ratified flows · audit-blind proxy checking · organ silos without skeleton · recovery cascades) lives concentrated in this layer. This matches the empirical anchor: PR #66 was a clean V2 fix that broke on V1-shaped delivery exactly because the supporting infrastructure was V1-shape.

### F2 — Delivery-mechanism is the immediate remediation target

`deploy-pod` failed all 5. CF-1+CF-2 bundle PR is the V2-aligned remediation path (already at MMA Step 2 PLAN; HALTED at Step 4 VERIFY pending Captain G33 v6/v7 disposition per §S-167 BLOCK at 2.12/5). Closing this surface unblocks Pod 1-7 fleet rollout of PR #66 binary AND removes the empirical anchor for §S-174 doctrine.

### F3 — Wallet is FAIL but actively being remediated

W3 RCA `78f82654` + PACT-024 §A wallet-client.js wrapper + Wave 1 PR-D constitute the in-flight V2-alignment work. F-05 anti-pattern lint (sub-PACT codification candidate) closes the audit-blind proxy class for this surface.

### F4 — Auth is PARTIAL with one BLOCKING cross-coupling

Sliding-window auth (W1-S5 RCA `15490644`) + PIN-LOCKOUT (W1-S6 RCA `ff26b502`) are the in-flight V2-alignment work. NEW-Q-CROSS-1 lockout-check-on-refresh BLOCKING in MMA Step 4 VERIFY → Wave A.2.1 surgical amendment in flight. Closing Q-CROSS-1 lifts auth from PARTIAL → PASS.

### F5 — Billing + comms-link-exec-protocol are V2-aligned exemplars

`billing` (F25a substrate `9f1c0a37` with HISTORICAL block + behavior parity tests) and `comms-link-exec-protocol` (PACT-021 substrate-immutability + ApprovalTier + ALLOWED_BINARIES allowlist) are the patterns to apply elsewhere. Notably:
- **billing** demonstrates Strategy pattern with V1-era preserved as known-strategy (not accidental fall-through) — applicable to wallet (V1 driver_id-scoped vs V2 customer_id-scoped) and other V1↔V2 boundaries.
- **comms-link-exec-protocol** demonstrates frozen-registry + tier-based-auth + allowlist-not-deny-first — applicable to rc-sentry BLOCKED_PATTERNS (CF-4 fix shape).

## Per-surface remediation pointers

| Surface | Remediation in flight | Gating event |
|---|---|---|
| deploy-pod | CF-1+CF-2 bundle PR (MMA Step 2 PLAN authored) | Captain G33 v6/v7 disposition on Step 4 VERIFY 3.50/5 BLOCK |
| rc-agent | (none specific; closes when delivery + watchdog land) | follows-from rc-watchdog + deploy-pod |
| rc-watchdog | CF-9 watchdog deploy-aware health checks (MMA novel finding) | Captain Q-MMA-DEPLOY-2 disposition on novel-finding accept/defer |
| rc-sentry | CF-4 BLOCKED_PATTERNS refactor (parser-not-regex + allowlist) | Captain disposition on CF-4 PR sequencing |
| fleet-health-api | §S-153 server-side heartbeat-mtime reader (deferred per §S-146) | Pod 1-7 PR #66 deploy complete + own §S-146 5-section RCA |
| wallet | Wave 1 PR-D wallet HOLD-RELEASE-CAPTURE; PACT-024 §A wrapper | Captain Q-W3-RECONCILE-1..2 + bono-LEAD wrapper authoring |
| auth | Wave A.2.1 amendment (W1-S5 + W1-S6 cross-coupling Q-CROSS-1) | Captain G33 v7 #1 ratification (already received per §S-169) |
| billing | F25b Way A flip (B-axis sub-step) | Captain F25b-Q-RECONCILE-1..6 disposition |
| comms-link-exec-protocol | (none — V2-aligned baseline) | n/a |

## Hook impact (Phase 1 cache seed)

These 9 JSON files now sit at `racecontrol/.planning/specs/v2/MECHANISM-TRUST/<surface>-2026-05-10.json` with mtime fresh today. The Phase 1 hook (`pre-v2-edit-rca-check.js`) treats any cache file within 30 days as PASS condition (b) — interpretation: caller has acknowledged the V1-shape by running this audit.

This means an Edit/Write to these 9 surfaces in the next 30 days will **PASS** the Phase 1 gate (cache exists). After 2026-06-09, re-audit needed or a §S-146 5-section RCA artifact must exist at `.planning/specs/v2/RCA/<surface>/<hash>.md`.

**Caveat**: PASSing the cache check doesn't mean the surface is V2-aligned. It means the audit was run and the V1-shape is acknowledged. FAIL surfaces still produce a cache file (with overall: FAIL); the hook treats both PASS and FAIL caches as condition (b) satisfied. This is **intentional** — the alternative (BLOCK on FAIL cache) would prevent any Edit to FAIL surfaces, including the remediation work itself. The audit cache is an attestation that the auditor has thought about the V1-shape, not a verdict that the surface is safe.

## NOT TESTED at this audit

- **Date convention**: filenames use IST date `2026-05-10`; `mechanism-trust-check.sh` uses `date -u` (UTC). Resolution: post-rename to IST. Script fix (accept --date or default to IST) deferred to next maintenance.
- **5-question taxonomy stability**: 5 questions chosen at §S-174 authoring; could need refinement after empirical use across more surfaces.
- **PARTIAL verdict semantics**: 4 YES + 1 NO produces PARTIAL; not yet decided whether PARTIAL counts as "audit complete" for the Phase 1 hook (currently: yes).
- **N/A vs YES/NO weighting**: N/A doesn't count toward verdict; could mask gaps. Re-evaluate after first wave of usage.
- **Re-audit cadence enforcement**: 30-day TTL is hardcoded; no auto-reminder when cache stales. Could pair with Phase 4 bilateral hook parity check at SessionStart.
- **james-side bilateral parity**: this audit is bono-only; james hasn't run his own. Some surfaces (rc-agent, rc-watchdog, rc-sentry) james has more recent code intuition on; james-side audit could amend.
- **Cache invalidation on commit**: a commit that materially changes a surface should invalidate that surface's cache; not yet enforced via hook.

## Composes-with

- §S-174 V2-MASTER-STATE entry — Phase 0 ratification + 4-phase plan
- §S-179 V2-MASTER-STATE entry (forthcoming) — Phase 2 + 6 close anchor
- `racecontrol/.planning/hooks-bilateral/pre-v2-edit-rca-check.spec.md` — Phase 1 hook (consumer of this cache)
- `racecontrol/.planning/hooks-bilateral/v2-foundational-surfaces.json` — surface-name source-of-truth
- `~/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` — canonical doctrine
- MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md — empirical evidence used in deploy-pod / rc-watchdog / rc-sentry audits
- §S-150 silent-loop-death E2E pipeline — empirical evidence used in rc-agent / fleet-health-api audits
- W1-S5 RCA `15490644` + W1-S6 RCA `ff26b502` — empirical evidence used in auth audit
- W3 RCA `78f82654` — empirical evidence used in wallet audit
- F25a substrate `9f1c0a37` — empirical evidence used in billing audit (PASS exemplar)
- PACT-021 substrate-immutability + PACT-022 layer-distinction — used in comms-link-exec-protocol audit (PASS exemplar)
