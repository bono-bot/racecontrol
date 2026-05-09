# MMA Step 4 VERIFY — adversarial review of CF-1+CF-2 bundle PLAN

You are an adversarial reviewer. Your job is to find what's WRONG with the proposed PLAN — not validate it. The PLAN was synthesized from 5-model consensus in MMA Step 2. We need 3 fresh vendor-disjoint models to challenge it before any code is written.

## Pipeline context

- **Step 1 DIAGNOSE** (5 models, 2026-05-09 ~14:08 UTC, $0.0409): identified 9 CONSENSUS findings CF-1..CF-9 + 4 NOVEL re: Windows-pod fleet deploy mechanism failures during PR #66 rollout (Pod 8 SHIPPED, Pods 1-7 NOT-DEPLOYED).
- **Step 2 PLAN** (5 different models, 2026-05-09 ~14:18 UTC, $0.0474): selected CF-1 (atomic kill+swap race vs RCWatchdog) + CF-2 (OTA_DEPLOYING sentinel discipline gap) for first PR; produced 7-action consensus PLAN.
- **Step 4 VERIFY** (you, 3 fresh-vendor models): adversarial gate before Step 3 EXECUTE. Score ≥4.0 = PASS, <4.0 = block PR authoring until amended.

## Background — what failed today

8 sim-racing pods on Windows. Each runs `rc-agent.exe` (port 8090, main agent), `rc-sentry.exe` (port 8091, /exec endpoint), `rc-watchdog.exe` (Windows service, polls rc-agent every 5-10s, calls `perform_rollback()` if rc-agent fails health check 2+ times after restart).

**CF-1 (root cause for 7 pods)**: `scripts/deploy-pod.sh` uses 3 separate /exec HTTP roundtrips for kill / copy / move. RCWatchdog wins the race; old binary respawns from `rc-agent-prev.exe`.

**CF-2 (root cause for Pod 1 manual recovery)**: `OTA_DEPLOYING` sentinel file suppresses rollback IF present. Discipline gap: no TTL → can leak forever; managed external to swap → easily omitted; `perform_rollback()` only checks `is_file()` so if the deploy script forgets to set it, watchdog rolls back the new binary 5-10s after deploy.

## CONSENSUS-PLAN.md (the PLAN you are reviewing)

### Selected approach
- **CF-1**: `single_exec_chain` via Windows CMD `&` operator (one HTTP call to rc-sentry /exec, single chain). 4/5 consensus; gemini dissented for `new_atomic_endpoint` (~200 LOC, deferred for CF-9 bundle).
- **CF-2**: `ttl_json_body` mirroring `auto_clear_maintenance_mode_json` proven pattern (5/5 unanimous). Sentinel becomes `{"timestamp_epoch":N}` JSON, watchdog auto-clears after TTL=300s, mtime fallback for legacy bare-file sentinels (Pod 8 backward compat).

### Files touched
- `crates/rc-watchdog/src/rollback_manager.rs` (5/5) — new `auto_clear_ota_deploying_json()` helper + replace bare `is_file()` check with TTL-aware
- `scripts/deploy-pod.sh` (5/5) — replace 3 separate /exec calls with single atomic chain; replace bare-file sentinel write with JSON
- `crates/rc-watchdog/src/service.rs` (3/5) — call auto-clear in main poll loop

### 7 actions (file + LOC + risk)
| ID | File | Summary | LOC | Risk |
|---|---|---|---|---|
| A1 | rollback_manager.rs | Add `auto_clear_ota_deploying_json(max_age_secs: u64)` mirroring `auto_clear_maintenance_mode_json` | ~30 | low |
| A2 | rollback_manager.rs | Replace bare `ota_deploying.is_file()` (line 121) with TTL-aware JSON check; preserve mtime fallback | ~15 | medium |
| A3 | service.rs | Call `auto_clear_ota_deploying_json(OTA_TTL_SECS)` in main poll loop | ~5 | low |
| A4 | rollback_manager.rs | Define `OTA_TTL_SECS = 300` constant | ~3 | low |
| A5 | scripts/deploy-pod.sh | Replace 3 /exec calls with single atomic chain: `taskkill /F /IM rc-agent.exe & del /Q rc-agent-prev.exe & ren rc-agent.exe rc-agent-prev.exe & ren rc-agent-new.exe rc-agent.exe & echo SWAPPED` (single `&` not `&&` — BLOCKED_PATTERNS-safe) | ~25 | medium |
| A6 | scripts/deploy-pod.sh | Replace bare-file sentinel with JSON body: `echo {"timestamp_epoch":$(date +%s)} > C:\RacingPoint\OTA_DEPLOYING` | ~5 | low |
| A7 | rollback_manager.rs | Unit tests: `test_auto_clear_ota_json_fresh_preserved`, `..._expired_removed`, `..._legacy_bare_mtime_fallback` | ~50 | low |

Total: ~133 LOC code + ~50 LOC tests = ~183 LOC. Single PR shape.

### Test plan
- T1-T3: unit tests on auto_clear (fresh / expired / legacy bare-file mtime fallback)
- T4: integration test — mock rc-sentry, run deploy-pod.sh, assert exactly ONE POST containing the chain
- T5: live Pod 1 deploy with new script + new rc-watchdog; observe `OTA_DEPLOYING sentinel present — suppressing rollback` log + zero rollback events for 5min after deploy

### Rollback
1. `git revert <merge>`, rebuild rc-watchdog, redeploy via `sc stop RCWatchdog & copy /Y rc-watchdog-prev.exe rc-watchdog.exe & sc start RCWatchdog`
2. Backward-compat: new code reads JSON OR mtime; old code reads bare file → both directions safe

### Open Q-DECISIONs (Captain defaults proposed)
- CF12-Q1: TTL=300s (4/5 default, deepseek dissent 600s)
- CF12-Q2: fixed constant (no env var; mirrors MAINTENANCE_MODE)
- CF12-Q3: read-bare-file mtime fallback YES (zero-migration)
- CF12-Q4: rc-watchdog deploy mechanism — `scripts/deploy-watchdog.sh` does NOT EXIST today; will be added (mirror `deploy-sentry.sh`)
- CF12-Q5: Step 4 VERIFY (this run!) recommended before Step 3 EXECUTE

### Minority dissent
- gemini Step 2: `new_atomic_endpoint` (~200 LOC `/exec_atomic_deploy`) — deferred for CF-9 bundle PR
- deepseek Step 2: 600s TTL — rejected for 300s default
- mimo Step 2: blast_radius `single_pod` — synthesis chose `fleet_pods` because Pod 1 canary unblocks 2-7

## Your adversarial task

For each scoring dimension, return a numeric score 0-5 with rationale. Then list specific FLAWS or risks the consensus may have missed.

### Scoring dimensions
1. **Correctness**: does single `&` chain actually win the race against rc-watchdog poll cycle (5-10s)? Does the JSON sentinel TTL semantically prevent the watchdog rollback during deploy?
2. **Risk coverage**: does the rollback plan cover failure modes (clock skew, malformed JSON, partial swap, watchdog respawn during chain)?
3. **Backward compatibility**: does Pod 8 canary on OLD rc-watchdog binary safely read the new JSON-format sentinel via bare `is_file()`? Will A2's `mtime fallback` actually fire when a JSON body sentinel exists but is older than TTL?
4. **Test plan adequacy**: do T1-T5 actually exercise the race? T5 is happy-path Pod 1 deploy — does it test the SCENARIO where watchdog wakes during the chain WITHOUT OTA_DEPLOYING set (the original failure)?
5. **Concreteness**: are the 7 actions actually implementable from the descriptions? Any missing files, missing pre-conditions, missing config?
6. **Independence from prompt-anchoring bias**: would these recommendations hold if the CLAUDE.md "Remote deploy sequence" canonical pattern was stripped from the Step 1+2 prompts? (Step 1+2 prompts included the canonical deploy sequence which biased models toward `single_exec_chain` over alternatives.)

### Specific challenge questions

- **Race window math**: Windows CMD chain with `taskkill ; del ; ren ; ren` over a single HTTP roundtrip — how long does this take? rc-watchdog polls every 5-10s. Is there ANY window where watchdog could wake mid-chain and observe rc-agent dead before the final `ren` completes? Does timeout_ms in the /exec curl payload need a specific minimum?
- **Sentinel race**: A6 writes JSON sentinel BEFORE A5 atomic chain. What if the deploy script crashes between A6 and A5? Sentinel exists but no swap → rc-agent down + rollback suppressed → silent fleet death until TTL expires (300s).
- **CF12-Q4 deploy-watchdog gap**: PLAN assumes new rc-watchdog must reach the pods. But if `scripts/deploy-watchdog.sh` doesn't exist yet AND Pod 8 stays on OLD rc-watchdog, then ONLY pods that get the new rc-watchdog get the TTL behavior. Pods on OLD rc-watchdog will rollback any new rc-agent unless OTA_DEPLOYING sentinel persists. This is a coupling the PLAN should expose.
- **Watchdog deploy bootstrapping**: `sc stop RCWatchdog & copy /Y rc-watchdog-prev.exe rc-watchdog.exe & sc start RCWatchdog` — what monitors rc-watchdog itself? What if `sc stop` succeeds but `sc start` fails? Pod becomes unmanaged. Is there a watchdog-of-the-watchdog?
- **A2 logic**: when JSON parse fails (corrupted file), does the new code default to "treat as suppression active" (safe — preserves rollback suppression) or "treat as no sentinel" (unsafe — allows rollback)? Either choice has trade-offs; the PLAN doesn't specify.
- **Pod 1 first-canary gap**: T5 says deploy to Pod 1 for live test. Pod 1 was last-session degraded (binary swap by manual atomic chain failed because of CF-2 omission, watchdog rolled back). Pod 1 may now be on OLD `c5f94e31-dirty` rc-agent + OLD rc-watchdog. Verifying TTL behavior on Pod 1 ALSO requires deploying new rc-watchdog to Pod 1 first — chicken-and-egg.

## Output format (JSON only)

```json
{
  "scores": {
    "correctness": 0.0,
    "risk_coverage": 0.0,
    "backward_compatibility": 0.0,
    "test_plan_adequacy": 0.0,
    "concreteness": 0.0,
    "independence_from_anchoring": 0.0,
    "overall": 0.0
  },
  "rationale_per_dimension": {
    "correctness": "...",
    "risk_coverage": "...",
    "backward_compatibility": "...",
    "test_plan_adequacy": "...",
    "concreteness": "...",
    "independence_from_anchoring": "..."
  },
  "flaws_identified": [
    {"id": "FL-1", "severity": "P0|P1|P2", "title": "...", "description": "...", "fix_recommendation": "..."}
  ],
  "missing_from_plan": ["..."],
  "verdict": "PASS|FLAG|BLOCK",
  "verdict_rationale": "<= 200 chars",
  "would_recommend_amendments": ["..."]
}
```

**Scoring scale**: 0 = catastrophically wrong / 1 = major flaws / 2 = significant gaps / 3 = workable with caveats / 4 = solid, ship-ready / 5 = excellent. Overall = arithmetic mean. Verdict gate: overall ≥4.0 = PASS, 3.0-3.99 = FLAG (proceed with amendments), <3.0 = BLOCK.

Be adversarial. Default to skepticism. If the PLAN is good, the score reflects it; if it has gaps, surface them.
