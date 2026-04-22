<!--
  Plan 2 Wave 3 (plan_deploy_poe_gates_20260423.md) — surfaces the Gate 1
  disruption-hypothesis enumeration in the PR body itself. Grace period:
  2 weeks warning-only; then PR-check action enforces. Gate 1 script
  (scripts/deploy-poe.sh) NOT YET SHIPPED — sections below are the
  human-readable enumeration that script will later automate.
-->

## Summary

<!-- 1-3 bullets: what changes, why, and which plan/phase it belongs to. -->
- 
- 
- 

## Test plan

<!-- Bulleted checklist of what you tested + what you did not. -->
- [ ] 
- [ ] 

## Disruption hypotheses (Gate 1)

> Check each before requesting review. `scripts/deploy-poe.sh` will automate
> this once shipped; until then, the author ticks manually.

- [ ] **Function-layer** — Does any caller of a changed function break? _(Evidence: `scripts/graphify-helpers/deploy-probe.sh <branch>` or manual caller-grep.)_
- [ ] **Data-layer** — Does data shape change? Migration / backward-compat required? _(Evidence: grep JSON / struct usage; check `customer_data_delete()` for new FK rows; check `packages/shared-types/generated/` drift.)_
- [ ] **Env-layer** — Does change alter process start/stop/env-vars? _(Evidence: `scripts/audit/process-ownership.sh` — the PR #19 static audit.)_
- [ ] **External-layer** — Does change call an external service with a new pattern? New HTTP verb, new endpoint, new auth header? _(Evidence: list new calls + verify service accepts them.)_
- [ ] **State-layer** — Does runtime state invariant change? _(Evidence: PR #20 E2E suite if state-carrying paths touched.)_
- [ ] **Multi-owner** — Does any process now have >1 spawn site? _(Evidence: `scripts/audit/process-registry-check.sh` once S8 `rc-process-manager` lands; manual grep for `Command::new` / `spawn_safe` until then.)_
- [ ] **Permanence** — Does the change survive redeploy? Or is there a manual fix elsewhere that will regress? _(Evidence: CGP Permanence Gate — is the fix in git vs server-side?)_
- [ ] **Fleet-parity** — Does the change need to land on all target hosts? Server .23 | Pods 1-8 | POS .130 | James .27 | Bono VPS | Cloud apps | Comms-link. _(Evidence: explicit per-target list, not "all 8 pods" hand-wave.)_

<!-- Mark hypotheses as N/A with a one-line justification if genuinely inapplicable. "Documentation-only", "comment typo", "version bump" are legitimate N/A reasons — "I don't know" is not. -->

## NOT tested

<!-- H3 anti-theater rule: empty list = lie, always something untested. -->
- 

## Rollback plan

<!-- How to undo if this breaks production. "revert the PR" is only acceptable if the change is pure-source; infra / migration changes need explicit steps. -->
- 

## Paired plans / related PRs

<!-- If this is a wave of a multi-PR plan, link the plan file + sibling PRs. -->
- 

🤖 Generated with [Claude Code](https://claude.com/claude-code)
