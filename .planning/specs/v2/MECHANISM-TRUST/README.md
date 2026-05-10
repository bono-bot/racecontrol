# MECHANISM-TRUST/ — 5-question audit cache for V2-foundational-boundary delivery surfaces

**Phase 2 of §S-174 4-phase enforcement plan** · bono-LEAD · authored 2026-05-10

## Purpose

This directory holds JSON cache files produced by `racecontrol/scripts/mechanism-trust-check.sh`. Each file records a 5-question audit on whether a V2-foundational-boundary delivery surface (rc-agent / rc-watchdog / rc-sentry / deploy-pod.sh / billing/ / wallet/ / auth/ / fleet_health_api.rs / comms-link relay+exec / migrations/ / etc.) is V2-aligned or V1-shaped.

The Phase 1 hook (`~/.claude/hooks/pre-v2-edit-rca-check.js`) reads this directory as PASS condition (b): if a cache file matching `<surface>-<date>.json` exists with mtime within `V2_RCA_TRUST_TTL_DAYS` (default 30), the hook permits an Edit/Write to that surface without requiring a full §S-146 5-section RCA artifact. Interpretation: the caller has acknowledged the V1-shape by running this audit.

## File-naming convention

```
<surface-name>-<YYYY-MM-DD>.json
```

Where `<surface-name>` exactly matches the `name` field in `racecontrol/.planning/hooks-bilateral/v2-foundational-surfaces.json`. Examples:

```
rc-agent-2026-05-10.json
rc-watchdog-2026-05-10.json
deploy-pod-2026-05-10.json
billing-2026-05-10.json
```

Re-audit by overwriting (same date) or appending a new file (newer date). The Phase 1 hook reads the freshest matching file.

## JSON schema

```json
{
  "version": "0.1.0-bono",
  "surface": "rc-agent",
  "date": "2026-05-10",
  "audited_at": "2026-05-10T01:55:00.000Z",
  "auditor": "bono",
  "questions": {
    "atomic_primitives": {
      "answer": "YES|NO|N/A",
      "evidence": "<path:line / commit / one-liner>"
    },
    "ttl_sentinels": { ... },
    "behavioral_verify": { ... },
    "single_target_dry_run": { ... },
    "guard_contracts": { ... }
  },
  "overall": "PASS|FAIL|PARTIAL",
  "notes": "<free-text or empty>"
}
```

### The 5 questions

1. **atomic_primitives**: Does the delivery chain use a single /exec atomic sequence (canonical CLAUDE.md pattern), not multi-step pipelined operations that race against the watchdog?
2. **ttl_sentinels**: Are sentinels (e.g. `OTA_DEPLOYING`) TTL-bounded, integrated with the atomic primitive (cannot be omitted), bilateral-mutex with the watchdog?
3. **behavioral_verify**: Is success measured by behavior (binary hash post-swap / mtime advance / ws_uptime>0) rather than echo-string ("SWAPPED echoed") or exit code?
4. **single_target_dry_run**: Is there a way to test the chain on one target before fleet rollout? `--canary` flag / staging endpoint / behavioral test harness?
5. **guard_contracts**: Do guards (rc-sentry `BLOCKED_PATTERNS`, rc-watchdog `rollback_manager`, comms-link relay PSK auth) have written contracts with the delivery script? Are blocked patterns parser-not-regex with explicit allowlist?

### Verdict computation

| Yes count | No count | Verdict |
|---|---|---|
| 4 or 5 | 0 | PASS |
| any | 3 or more | FAIL |
| any | other | PARTIAL |

`N/A` answers don't count as Yes or No.

## Usage

### Interactive mode (default)

```bash
racecontrol/scripts/mechanism-trust-check.sh --surface rc-agent
```

Prompts for each question + evidence. Writes JSON to default path.

### Non-interactive mode (for batch / CI)

```bash
racecontrol/scripts/mechanism-trust-check.sh \
  --surface rc-agent \
  --mode non-interactive \
  --atomic-primitives NO \
  --atomic-primitives-evidence "rc-agent main loop atomic at the panic-hook level (PR #66 d6c623d7) but deploy-side delivery is non-atomic (CF-1)" \
  --ttl-sentinels NO \
  --ttl-sentinels-evidence "OTA_DEPLOYING sentinel external to swap chain (§S-176 deploy MMA CF-2)" \
  --behavioral-verify NO \
  --behavioral-verify-evidence "deploy-pod.sh checks SWAPPED echo not binary hash (G9-class #4)" \
  --single-target-dry-run NO \
  --single-target-dry-run-evidence "no --canary flag; burned 7 pods cycling same SHA failure (G9-class #1)" \
  --guard-contracts NO \
  --guard-contracts-evidence "rc-sentry BLOCKED_PATTERNS deny-first regex no allowlist (CF-4)" \
  --notes "Deploy mechanism is the §S-174 empirical anchor for V1-shaped delivery on V2-clean fix"
```

### Template mode (placeholder JSON for hand-editing)

```bash
racecontrol/scripts/mechanism-trust-check.sh --surface rc-agent --mode template --out /tmp/template.json
```

## Composes-with

- `racecontrol/.planning/hooks-bilateral/pre-v2-edit-rca-check.spec.md` — Phase 1 hook spec (consumes this cache)
- `racecontrol/.planning/hooks-bilateral/v2-foundational-surfaces.json` — surface-name source-of-truth
- `~/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` — canonical doctrine
- §S-174 V2-MASTER-STATE entry — Phase 0 ratification
- §S-179 V2-MASTER-STATE entry (forthcoming) — Phase 2 + 6 ship close anchor

## Bilateral notes

- Bono-LEAD authored 2026-05-10.
- James-side install path: git_pull racecontrol → `chmod +x racecontrol/scripts/mechanism-trust-check.sh` → tested via `--mode template` round-trip → ready for use.
- Re-audit cadence: re-run on any surface that materially changes (V2-pattern adoption / new V1-shape introduced).
- Phase 6 backfill (this session): 9 surfaces covered with `--mode non-interactive` using empirical evidence from §S-174 + §S-176 + MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md.
