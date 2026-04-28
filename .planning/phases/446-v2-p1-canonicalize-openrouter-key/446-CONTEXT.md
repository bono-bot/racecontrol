# Phase 446: Canonicalize `OPENROUTER_KEY` — Context

**Gathered:** 2026-04-21
**Status:** Ready for planning
**Source:** `/gsd:plan-phase 446` Option-2 bypass — derived from authoritative kickoff doc [.planning/specs/v2/PHASE-446-KICKOFF.md](../../specs/v2/PHASE-446-KICKOFF.md)
**Parent milestone:** v2.0 Foundation — Drift-Class Elimination (P1 Config Migration)

<domain>
## Phase Boundary

**In scope:** Migrate 3 remaining `OPENROUTER_API_KEY` env-var read sites to canonical `OPENROUTER_KEY` using inline dual-read + deprecation warn. Deploy the 3 affected binaries (rc-agent, rc-watchdog, whatsapp-bot) + audit deploy-script env writers.

**Specifically:**
- 3 Rust/JS source files with `std::env::var` / `process.env` reads of `OPENROUTER_API_KEY`
- Deploy scripts that set the env var (bat files + Bono VPS pm2)
- Per-target behavior verification on Pod 4 + Bono VPS

**Out of scope (explicit — each would over-scope this phase):**
- `rc-common::secrets` central loader — **Phase 448**
- TOML field `openrouter_api_key` in rc-agent.toml — separate migration, not the env-var path
- `OPENROUTER_MGMT_KEY` env var — different secret, keeps its name
- rc-sentry + racecontrol server binaries — already canonical, zero diff
- POS kiosk — does not run rc-agent, does not exercise OpenRouter path
- Comment/log-message text mentioning `OPENROUTER_API_KEY` — update only if it breaks log-search queries

**Class:** S-class drift-canonicalize refactor (source-only; additive dual-read; per-commit `git revert` rollback).

**Total diff:** ~20 LOC across 3 files.

</domain>

<decisions>
## Implementation Decisions

### Canonical name
- **Locked:** `OPENROUTER_KEY` wins (5/8 sites already use it; matches `OPENROUTER_MGMT_KEY` convention; shorter log lines).

### Migration pattern — Rust
- **Locked:** Inline dual-read at each of the 3 Rust sites. Do NOT abstract into a helper — Phase 448 owns that.
- Canonical name checked FIRST; old name is fallback only.
- Deprecation warn fires exactly once per process on fallback hit, via `tracing::warn!`. No warn on canonical read (no log spam when env is correct).
- Preserve existing "not set" log messages verbatim so log-search queries still match.

### Migration pattern — JS (whatsapp-bot)
- **Locked:** Same pattern as Rust. IIFE that checks `process.env.OPENROUTER_KEY` first, falls back to `OPENROUTER_API_KEY` with one-shot `console.warn`.
- **Recommended (not locked):** Rename local const `OPENROUTER_API_KEY` to `OPENROUTER_KEY` and update [claudeService.js line 35](../../whatsapp-bot/src/services/claudeService.js#L35) redaction regex + line 69 `Authorization` usage. Plan should decide: rename vs keep.

### Sites to modify
1. [crates/rc-watchdog/src/mma_diagnosis.rs:261](../../crates/rc-watchdog/src/mma_diagnosis.rs#L261) — rc-watchdog MMA diagnosis (runs on every pod)
2. [crates/rc-agent/src/ai_debugger.rs:601](../../crates/rc-agent/src/ai_debugger.rs#L601) — rc-agent AI debugger path (has existing TOML-config fallback at line 607; do NOT disturb that path)
3. [whatsapp-bot/src/services/claudeService.js:3](../../../whatsapp-bot/src/services/claudeService.js#L3) — whatsapp-bot Claude-via-OpenRouter calls

### Sites NOT to touch (already canonical)
- [crates/rc-sentry/src/mma_engine.rs:22](../../crates/rc-sentry/src/mma_engine.rs#L22)
- [crates/racecontrol/src/ai_behavior_batch_mma.rs:31](../../crates/racecontrol/src/ai_behavior_batch_mma.rs#L31)
- [crates/racecontrol/src/ai/providers.rs:108](../../crates/racecontrol/src/ai/providers.rs#L108)
- [crates/rc-agent/src/openrouter.rs:279](../../crates/rc-agent/src/openrouter.rs#L279)
- [crates/racecontrol/src/server_diagnostics_infra.rs:233](../../crates/racecontrol/src/server_diagnostics_infra.rs#L233)

### Deploy-script env-writer audit (part of this phase)
The migration is only complete when production env sets `OPENROUTER_KEY`. Writers to inspect + update:
- `scripts/deploy/start-rcagent.bat` — grep for `OPENROUTER_API_KEY`
- `scripts/deploy/start-rcsentry.bat` — same
- Bono VPS pm2 ecosystem file for whatsapp-bot (`pm2 env <id>`)
- `.env.production.local` on Bono VPS
- Keep existing `data/openrouter-mma-key.txt` fallback file unchanged (not an env var)
- Do NOT rename TOML field `openrouter_api_key` in `racecontrol.toml` / `rc-agent.toml` — different migration class.

### Deploy surface
| Binary | Targets | Mechanism |
|---|---|---|
| `rc-agent.exe` | Pods 1-8 | Staged deploy via `/exec` + atomic rename-swap per CLAUDE.md |
| `rc-watchdog.exe` | Pods 1-8 | Same as rc-agent (runs alongside) |
| whatsapp-bot Node | Bono VPS | `git_pull` + `pm2 restart racingpoint-bot` |

**NOT rebuilt this phase:** `racecontrol.exe` (server + VPS), `rc-sentry.exe`, any Next.js frontend.

### Rollback plan
- Pre-rollout: `git revert <phase-head-commit>` → compiles → ship. Zero behavior impact.
- Post partial rollout: flip `start-rcagent.bat` / `start-rcsentry.bat` env back to old name (fallback still works), OR swap to `rc-agent-prev.exe`.
- If deprecation warn becomes log noise: downgrade to `tracing::info!` or add `OnceLock<()>` guard.

### MMA audit
- **Not required.** S-class drift refactor, no cross-system bridge, no new crate. Risk is covered by `cargo build` + `cargo test` + per-target behavior check + existing security gate. If planner flags hidden complexity, re-evaluate.

### Claude's Discretion
- Plan decomposition (single plan vs split Rust/JS/deploy-audit) — planner decides based on task granularity.
- Commit count (single atomic commit vs per-site + per-bat commit) — planner decides; kickoff does not lock this.
- Which test harness to verify the deprecation path (curl to a Pod 4 rc-agent endpoint vs a one-shot Rust integration test) — planner decides.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (planner + executor) MUST read these before planning or implementing.**

### Phase specs
- [.planning/specs/v2/PHASE-446-KICKOFF.md](../../specs/v2/PHASE-446-KICKOFF.md) — Authoritative phase handoff (entry state with file:line evidence, migration pattern code samples, exit criteria, deploy surface, rollback runbook, MMA reasoning)
- [.planning/specs/v2/V2-ROADMAP.md](../../specs/v2/V2-ROADMAP.md) — Parent P1 roadmap (phases 446–452)
- [.planning/specs/v2/config-inventory.md](../../specs/v2/config-inventory.md) — Full S-class secret inventory (for Phase 448 context; 446 touches only 1 row)

### Codebase refs (source of truth for current state)
- [crates/rc-watchdog/src/mma_diagnosis.rs](../../crates/rc-watchdog/src/mma_diagnosis.rs) — old-name read site #1
- [crates/rc-agent/src/ai_debugger.rs](../../crates/rc-agent/src/ai_debugger.rs) — old-name read site #2 (plus TOML fallback at :607 that must stay intact)
- [whatsapp-bot/src/services/claudeService.js](../../../whatsapp-bot/src/services/claudeService.js) — old-name read site #3 + redaction regex at :35 + Authorization usage at :69
- [crates/rc-sentry/src/mma_engine.rs](../../crates/rc-sentry/src/mma_engine.rs) — canonical reference implementation (copy the pattern)

### Project rules
- [CLAUDE.md — Deploy section](../../../CLAUDE.md) — rc-agent + rc-watchdog atomic-swap via `/exec` + bat-file sync
- [CLAUDE.md — Security section](../../../CLAUDE.md) — `node comms-link/test/security-check.js` must stay green post-change
- [CLAUDE.md — Regression Prevention](../../../CLAUDE.md) — any manual env fix must be encoded in `start-*.bat` so it survives reboot
- [CLAUDE.md — Cross-Process Updates](../../../CLAUDE.md) — cascade-update all environments (venue + cloud + James .27)

### MMA / security audit plumbing that READS `OPENROUTER_KEY`
- `scripts/multi-model-audit.js` — already reads canonical; verifies-after by green run
- `scripts/lib/openrouter-key-recovery.js` — shared 401-auto-recovery module; already canonical

</canonical_refs>

<specifics>
## Specific Ideas

### Rust dual-read snippet (copy-paste target — kickoff doc lines 84–108)
```rust
// Replace
let api_key = match std::env::var("OPENROUTER_API_KEY") {
    Ok(k) if !k.is_empty() => k,
    _ => { /* fallback */ }
};

// With
let api_key = match std::env::var("OPENROUTER_KEY") {
    Ok(k) if !k.is_empty() => k,
    _ => match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.is_empty() => {
            tracing::warn!("OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY (read once, will not repeat)");
            k
        }
        _ => { /* existing fallback */ }
    }
};
```

### JS dual-read snippet (copy-paste target — kickoff doc lines 121–136)
```js
const OPENROUTER_KEY = (() => {
  if (process.env.OPENROUTER_KEY && process.env.OPENROUTER_KEY.length > 0) {
    return process.env.OPENROUTER_KEY;
  }
  if (process.env.OPENROUTER_API_KEY && process.env.OPENROUTER_API_KEY.length > 0) {
    console.warn('[whatsapp-bot] OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY in pm2 env');
    return process.env.OPENROUTER_API_KEY;
  }
  return undefined;
})();
```

### Exit-criteria behaviors to test (CGP H3 pre-verified, from kickoff)
1. **rc-agent Pod 4:** AI debugger path succeeds with `OPENROUTER_KEY` set; log shows canonical read, no warn. Command: trigger AI debug call via Pod 4 endpoint.
2. **rc-watchdog Pod 4:** MMA diagnosis completes with canonical name; no warn.
3. **whatsapp-bot Bono VPS:** Claude-via-OpenRouter test message receives LLM response; `pm2 logs racingpoint-bot` shows no warn.
4. **Deprecation path:** On a test pod, unset `OPENROUTER_KEY` + set `OPENROUTER_API_KEY`; warn fires once per process on fallback read; MMA still succeeds.

### Static checks (automated — plan should include exact commands as task acceptance criteria)
- `grep -rn 'std::env::var("OPENROUTER_API_KEY")' crates/` → 0 hits outside dual-read blocks
- `grep -rn 'process.env.OPENROUTER_API_KEY' whatsapp-bot/src/ --include=*.js --include=*.ts` → 0 hits outside dual-read
- `cargo build --release --bin rc-agent` + `--bin rc-watchdog` + `--bin racecontrol` all green
- `cd whatsapp-bot && npm run lint` passes
- `cargo test -p rc-agent-crate` passes
- `node comms-link/test/security-check.js` passes (31 assertions green)

### Plan numbering suggestion (planner may deviate)
- **446-01-PLAN.md** — Rust dual-read at rc-watchdog + rc-agent (2 sites, 1 commit or split). Tasks: read, edit, `cargo build`, `cargo test`.
- **446-02-PLAN.md** — JS dual-read at whatsapp-bot. Tasks: read, edit, `npm run lint`, local smoke test.
- **446-03-PLAN.md** — Deploy-script env-writer audit + update (bat files + VPS pm2 + .env.production.local). Tasks: grep writers, update canonical name, document which targets need staff follow-up for pm2 reload.
- **446-04-PLAN.md** — Per-target behavior verification (Pod 4 rc-agent + rc-watchdog + Bono VPS whatsapp-bot + deprecation-path test on isolated pod). Tasks: curl/pm2 behavior tests with exact evidence shape.

### H3/H4 compliance baked into plans
- Every verification task must name the EXACT behavior tested + include the RAW OUTPUT capture command + WHERE run + NOT-TESTED list.
- "All pods updated" claims must grep + enumerate Pods 1-8 individually.
- Per PERMANENCE GATE: every production env edit must be reflected in git-permanent `start-*.bat` before claiming fixed.

</specifics>

<deferred>
## Deferred Ideas

- **`rc-common::secrets` helper module** — Phase 448 consolidates the 3 inline dual-reads into one typed getter. Out of scope here to keep each phase independently revertable.
- **TOML field rename `openrouter_api_key` → `openrouter_key`** — separate migration class (config file, not env var). Not numbered yet.
- **Removing old name entirely (drop the fallback read)** — after Phase 449 CI drift-detector catches regressions, a later phase can remove fallback branches. Not this phase.
- **Load test under high concurrent MMA calls** — not a behavior change; skip.
- **BOTH vars set to DIFFERENT values edge case** — canonical should win per pattern, but not explicitly asserted by plan.
- **Canonical set to empty string edge case** — existing `is_empty()` filter handles, not asserted in new path.
- **Re-verifying rc-sentry + racecontrol server** — no code change, no re-test.

</deferred>

---

*Phase: 446-v2-p1-canonicalize-openrouter-key*
*Context derived: 2026-04-21 via Option-2 bypass of `/gsd:plan-phase` (STATE.md pinned at v40.0; Phase 446 in new v2.0 Foundation milestone).*
*Authoritative handoff: `.planning/specs/v2/PHASE-446-KICKOFF.md`.*
