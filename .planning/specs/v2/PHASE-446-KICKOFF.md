# Phase 446 — Canonicalize `OPENROUTER_KEY` — Kickoff Handoff

**Parent milestone:** v2 Foundation (`V2-FOUNDATION-MILESTONE.md`)
**Parent phase group:** P1 Config Migration (`V2-P1-CONFIG-SERVICE.md` + `V2-ROADMAP.md`)
**Phase number:** 446
**Phase type:** Source-code refactor (S-class drift canonicalize)
**Readiness:** IMMEDIATE — all entry gates pass
**Created:** 2026-04-21
**Prepared by:** James (for next-session execute or `/gsd:plan-phase 446` subagent)

---

## TL;DR for the executor

Two env var names (`OPENROUTER_KEY`, `OPENROUTER_API_KEY`) refer to the same secret. 5 sites already use the canonical `OPENROUTER_KEY`; 3 sites still read the old `OPENROUTER_API_KEY`. Migrate the 3 stragglers to canonical with deprecation warn. Ship via per-commit git push; deploy the 3 affected binaries (rc-agent, rc-watchdog, whatsapp-bot) on the next normal deploy window.

**Total diff: ~20 LOC across 3 files. Zero behavior change if env is correctly set. Full revert = `git revert`.**

---

## Why this phase

1. Drift pair — same secret, two names — creates confusion every time a new call site is added
2. The wrong name in one pod's env gets silently swallowed by `.unwrap_or_default()` → calls fail at auth check downstream → MMA diagnostics silently disabled on that pod
3. Locks the pattern all subsequent P1 drift-pair migrations reuse (Phase 447 does the same for `RC_TERMINAL_SECRET`)

---

## Entry state — grep evidence 2026-04-21

### Old name `OPENROUTER_API_KEY` — 3 active-code sites to migrate

| File:line | Context | Consumer |
|---|---|---|
| [crates/rc-watchdog/src/mma_diagnosis.rs:261](racingpoint/racecontrol/crates/rc-watchdog/src/mma_diagnosis.rs#L261) | `std::env::var("OPENROUTER_API_KEY")` | rc-watchdog MMA diagnosis (runs on every pod) |
| [crates/rc-agent/src/ai_debugger.rs:601](racingpoint/racecontrol/crates/rc-agent/src/ai_debugger.rs#L601) | `std::env::var("OPENROUTER_API_KEY").ok().filter(...)` | rc-agent AI debugger path (with TOML-config fallback at line 607) |
| [whatsapp-bot/src/services/claudeService.js:3](racingpoint/whatsapp-bot/src/services/claudeService.js#L3) | `process.env.OPENROUTER_API_KEY` | whatsapp-bot Claude-via-OpenRouter calls |

### Canonical `OPENROUTER_KEY` — 5 sites already correct (do not touch)

| File:line | Consumer |
|---|---|
| [crates/rc-sentry/src/mma_engine.rs:22](racingpoint/racecontrol/crates/rc-sentry/src/mma_engine.rs#L22) | rc-sentry MMA engine |
| [crates/racecontrol/src/ai_behavior_batch_mma.rs:31](racingpoint/racecontrol/crates/racecontrol/src/ai_behavior_batch_mma.rs#L31) | Server AI behavior batch audit |
| [crates/racecontrol/src/ai/providers.rs:108](racingpoint/racecontrol/crates/racecontrol/src/ai/providers.rs#L108) | Server AI provider selection |
| [crates/rc-agent/src/openrouter.rs:279](racingpoint/racecontrol/crates/rc-agent/src/openrouter.rs#L279) | rc-agent OpenRouter HTTP client |
| [crates/racecontrol/src/server_diagnostics_infra.rs:233](racingpoint/racecontrol/crates/racecontrol/src/server_diagnostics_infra.rs#L233) | Server diagnostics infra |

### Related but out of scope (do not touch this phase)

- `OPENROUTER_MGMT_KEY` ([rc-agent/openrouter.rs:319](racingpoint/racecontrol/crates/rc-agent/src/openrouter.rs#L319)) — separate env var for child-key provisioning; keeps its name
- TOML-config fallback at [ai_debugger.rs:607](racingpoint/racecontrol/crates/rc-agent/src/ai_debugger.rs#L607) (`openrouter_api_key` in rc-agent.toml) — Phase 363 work; Phase 446 just normalizes the **env-var** read path, not the TOML field name
- Comment references and log messages mentioning `OPENROUTER_API_KEY` (e.g., claudeService.js line 35 redaction regex) — update only if it would confuse log reading
- `rc-common::secrets` loader itself — Phase 448 scope, NOT this phase

### Deploy scripts + env writers (need audit during execution)

The migration is only complete when production env files set `OPENROUTER_KEY` (not `OPENROUTER_API_KEY`). Writers to audit during Phase 446 execution:

- `scripts/deploy/start-rcagent.bat` — grep for `OPENROUTER_API_KEY`
- `scripts/deploy/start-rcsentry.bat` — same
- Bono VPS pm2 ecosystem file (whatsapp-bot) — check via `pm2 env <id>`
- `racecontrol.toml` on server — contains `openrouter_api_key` per ai_debugger.rs:607 — **leave TOML field name alone; different migration**
- `.env.production.local` on Bono VPS — check + update if present
- `data/openrouter-mma-key.txt` file fallback — unchanged (not an env var)

---

## Canonical name decision

**`OPENROUTER_KEY` wins.** Rationale:
- Already used in 5/8 active sites (dominant)
- Matches `OPENROUTER_MGMT_KEY` convention (also uses short form)
- Shorter; reads naturally in logs

---

## Migration pattern (per site)

### Rust sites — dual-read with deprecation warn

Replace direct `std::env::var("OPENROUTER_API_KEY")` with:

```rust
// Old (at rc-watchdog/src/mma_diagnosis.rs:261)
let api_key = match std::env::var("OPENROUTER_API_KEY") {
    Ok(k) if !k.is_empty() => k,
    _ => {
        tracing::info!("OPENROUTER_API_KEY not set — using deterministic fallback");
        return deterministic_fallback();
    }
};

// New
let api_key = match std::env::var("OPENROUTER_KEY") {
    Ok(k) if !k.is_empty() => k,
    _ => match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.is_empty() => {
            tracing::warn!("OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY (read once, will not repeat)");
            k
        }
        _ => {
            tracing::info!("OPENROUTER_KEY not set — using deterministic fallback");
            return deterministic_fallback();
        }
    }
};
```

Rules:
- **Canonical read first.** `OPENROUTER_KEY` is checked before `OPENROUTER_API_KEY`.
- **Deprecation warn only on fallback hit.** If canonical works, no warn (no log spam when env is correct).
- **Warn level = `tracing::warn!`** — shows up in server logs + pod logs + jsonl.
- **Keep existing log strings intact** where possible so log-search queries still match.
- **Don't abstract.** Inline the pattern in all 3 Rust sites. Phase 448 will dedupe into `rc-common::secrets`.

### JS site — same pattern

At [whatsapp-bot/src/services/claudeService.js:3](racingpoint/whatsapp-bot/src/services/claudeService.js#L3):

```js
// Old
const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY;

// New
const OPENROUTER_API_KEY = (() => {
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

Keep the const name `OPENROUTER_API_KEY` in the JS file (it's a local const, not the env var) OR rename to `OPENROUTER_KEY` for consistency. Recommended: rename, update line 35 redaction regex to `/OPENROUTER_KEY/g` and line 69 `Authorization` usage.

---

## Exit criteria — what "done" looks like

**Behavior that must be tested (CGP H3 pre-verified):**

1. **BEHAVIOR:** rc-agent on Pod 4 uses OpenRouter AI debugger path successfully with `OPENROUTER_KEY` set (not `OPENROUTER_API_KEY`)
   - Command: trigger an AI debug call via `curl http://192.168.31.88:18889/debug/ai-request -d '{"symptom":"test"}'` (or whichever endpoint invokes ai_debugger.rs)
   - Expected: HTTP 200 + non-empty response; pod log shows `rc_config_get{source=env,key=OPENROUTER_KEY}` (or equivalent) without deprecation warn
   - NOT acceptable: health endpoint OK alone (that's build_id theater)

2. **BEHAVIOR:** rc-watchdog on Pod 4 runs MMA diagnosis successfully with canonical name
   - Command: trigger rc-watchdog MMA via `/rc-watchdog/trigger-diagnosis` endpoint (or equivalent)
   - Expected: MMA call completes; log shows canonical-name read, no deprecation warn

3. **BEHAVIOR:** whatsapp-bot on Bono VPS sends a Claude-via-OpenRouter message using canonical name
   - Command: send a test message to staff WhatsApp instance that routes to `claudeService.js`
   - Expected: message receives LLM response; `pm2 logs racingpoint-bot` shows no deprecation warn

4. **BEHAVIOR (deprecation path):** Unset `OPENROUTER_KEY`, set only `OPENROUTER_API_KEY` on one test pod (not production)
   - Expected: `tracing::warn!` fires once per process on fallback read; MMA calls still succeed

**Static checks (automated, run as part of `/gsd:verify-work 446`):**

- `grep -rn 'std::env::var("OPENROUTER_API_KEY")' crates/` → returns zero hits outside the dual-read blocks (grep should match only the 3 migrated lines, each adjacent to a canonical read)
- `grep -rn 'process.env.OPENROUTER_API_KEY' whatsapp-bot/src/ --include=*.js --include=*.ts` → returns zero hits outside dual-read
- `cargo build --release --bin rc-agent` → compiles
- `cargo build --release --bin rc-watchdog` → compiles
- `cargo build --release --bin racecontrol` → compiles (should be unchanged)
- `cd whatsapp-bot && npm run lint` → passes
- `cargo test -p rc-agent-crate` → passes (no new tests needed; existing ones cover the code paths)

**NOT tested this phase (explicit list per CGP H3):**
- Behavior when BOTH env vars are set to DIFFERENT values — canonical should win, but not asserted
- Behavior when canonical is set to empty string — existing `.filter(!is_empty)` should handle, but not asserted in new code path
- rc-sentry / racecontrol server — unchanged this phase, so not re-tested
- POS kiosk — does not run rc-agent (runs billing UI only), OpenRouter path not exercised
- Cloud racecontrol on Bono VPS — same code as server, unchanged this phase
- Load test under high concurrent MMA calls — not a behavior change, skip

---

## Deploy surface

**Binaries that must rebuild + ship as part of Phase 446 verification:**

| Binary | Targets | Deploy mechanism |
|---|---|---|
| `rc-agent.exe` | Pods 1-8 | Existing staged deploy via `/exec` + rename-atomic swap (per CLAUDE.md Deploy rule) |
| `rc-watchdog.exe` | Pods 1-8 | Same as rc-agent — runs alongside |
| whatsapp-bot Node.js | Bono VPS | `git_pull` + `pm2 restart racingpoint-bot` |

**NOT rebuilt / re-deployed this phase:**
- `racecontrol.exe` (server + VPS) — no source change
- `rc-sentry.exe` — no source change
- Kiosk Next.js apps — no source change

**Deploy gate:** Before first pod deploy, confirm `OPENROUTER_KEY` env var is set in `start-rcagent.bat` + `start-rcsentry.bat`. If production env still says `OPENROUTER_API_KEY`, pods after rebuild will hit the fallback warn path on every MMA call. Not broken, but spam. Fix env first or in same PR.

---

## Rollback runbook

### If a bug surfaces BEFORE fleet rollout
- `git revert <phase-head-commit>` → compiles → ship
- Zero behavior impact since original code is unchanged from what shipped pre-phase

### If a bug surfaces AFTER some pods have new binary
- Revert env var: flip `start-rcagent.bat` + `start-rcsentry.bat` back to `OPENROUTER_API_KEY=...` on affected pods (old name still works via fallback)
- OR: swap to previous `rc-agent-prev.exe` via SSH per Deploy rule
- OR: `git revert` + redeploy — same 3-binary rebuild

### If the deprecation warn becomes log noise
- Option 1: change `tracing::warn!` to `tracing::info!` — still records the issue without escalating
- Option 2: downgrade to `debug!` + add a once-per-process guard (`std::sync::OnceLock<()>`)

Expected revert wall-clock: under 10 minutes per binary.

---

## Dependencies

**Upstream (none — this phase is IMMEDIATE):**
- No other v2 phase needs to land first
- No in-flight work blocks this (Phase 445 Wave 5, Pattern I 4/5, Phase 414, F4 PR all touch orthogonal code)

**Downstream (Phase 446 unlocks):**
- Phase 447 (RACECONTROL_TERMINAL_SECRET canonicalize) reuses this exact dual-read pattern
- Phase 448 (rc-common::secrets loader) subsumes the 3 inline dual-reads into one central helper
- Phase 449 (CI drift-detector) relies on 448 being the only allowed `std::env::var` path

---

## MMA audit — NOT required for this phase

Reasoning: Phase 446 is a S-class drift-canonicalize refactor. No new feature, no cross-system bridge, no new crate. The risk surface is covered by:
- `cargo build` + `cargo test` (structural)
- Per-target behavior verification on Pod 4 + Bono VPS (functional)
- Existing security gate `node comms-link/test/security-check.js` (regression protection)

Per CLAUDE.md Standing Rules, MMA is mandatory for **cross-system bridges** — this isn't one. Skipping MMA saves ~$2-5 and a day.

If the `/gsd:plan-phase 446` subagent flags complexity we missed, re-evaluate.

---

## Execution sequence (recommended)

1. `/gsd:plan-phase 446` — creates `.planning/phases/446-v2-p1-canonicalize-openrouter-key/` with CONTEXT.md + PLAN.md
2. Review PLAN.md for deviation from this handoff — push back on subagent if it tries to over-engineer (e.g., building `rc-common::secrets` module here instead of inlining)
3. `/gsd:execute-phase 446` — runs the atomic-commit executor on the 3-file diff + env audit
4. `/gsd:verify-work 446` — runs exit-criteria checks
5. Before first deploy: audit env writers (bat files + VPS pm2) per Deploy Surface section above
6. Deploy Pod 4 canary → 24h soak → remaining pods
7. Deploy Bono VPS whatsapp-bot (low-risk — no canary needed; single target)
8. Mark phase complete via `/gsd:complete-phase 446` or equivalent

---

## Five things the next session / agent might get wrong

1. **Building `rc-common::secrets` here.** NO — that's Phase 448. Phase 446 uses inline dual-read at each of 3 sites. The YAGNI protects each phase's independence.
2. **Renaming the TOML field `openrouter_api_key` in rc-agent.toml.** NO — that's separate from env var migration. ai_debugger.rs:607 TOML read stays as-is. A later phase (post-v2-P1) can normalize that too if we want.
3. **Touching rc-sentry or server binaries "while we're at it."** NO — those 5 sites already use canonical `OPENROUTER_KEY`. Leave alone. Zero diff, zero rebuild.
4. **Treating "source commit" as "deployed."** NO — CGP H3 + PERMANENCE GATE. Commit = permanent. Deploy + verify per target = complete. The two-phase completion rule applies.
5. **Claiming fleet-wide without per-target evidence.** NO — enumerate per CGP H4. Pods 1-8 checked individually; log evidence per pod; POS irrelevant (doesn't run rc-agent); Bono VPS checked separately for whatsapp-bot.

---

## Session metrics expectation

If Phase 446 is executed end-to-end in one session (plan + execute + verify + 1-pod deploy):
- Claims: ~3-5 (commit, build, deploy, verify)
- Corrections: target 0 — scope is tight enough to avoid G9s
- FCR: target 0%
- G9s: target 0

Full fleet rollout (Pods 1-8 + POS + Bono VPS) likely spans 2-3 sessions with 24h+ soak gaps. That's the staged-rollout contract, not a failure.

---

## Handoff verification (for next session / agent)

Before executing, confirm:
- [ ] Handoff read in full
- [ ] Fresh `git status` shows clean working tree on branch (no stray Phase 445 Wave 5 changes)
- [ ] `git log --oneline -5` confirms HEAD matches a green-verified state
- [ ] OpenRouter service is operational (quick curl to api key endpoint)
- [ ] No concurrent MMA audit running (scripts read env directly; don't race)
- [ ] Venue Tailscale connectivity verified OR this phase is deploy-deferred until venue returns

If any checkbox fails, STOP and surface to Uday before proceeding.
