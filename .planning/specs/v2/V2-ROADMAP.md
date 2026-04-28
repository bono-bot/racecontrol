# V2 Foundation — Phase Roadmap

**Parent:** `V2-FOUNDATION-MILESTONE.md`
**Inventory:** `config-inventory.md`
**P1 Detail:** `../V2-P1-CONFIG-SERVICE.md`
**Created:** 2026-04-21
**Status:** DRAFT — awaiting Uday kickoff approval

---

## How to read this doc

Each phase is a `/gsd:plan-phase NNN` target. Entry format:

- **Readiness:** IMMEDIATE (can kick off today, zero fleet risk) / GATED (blocked by prior phase or in-flight work)
- **Entry:** what must be true before `/gsd:plan-phase NNN`
- **Exit:** what `/gsd:verify-work` must see for the phase to ship
- **Isolation:** the one-line contract — additive deploy, single-flag activation, per-target rollout, kill-switch
- **Rollback:** exact one-line revert

Phase numbers start at 446 (Phase 445 is the in-flight typed API contract; v50.0 reserved 429-444).

---

## P1 — Config migration (phases 446-452)

### Phase 446 — Canonicalize `OPENROUTER_KEY`

**Readiness:** **IMMEDIATE** (4 of 4 gates pass — pure source change, no fleet deploy needed for isolation)
**Dev estimate:** 1-2 days
**Kills:** dual-name drift G9 class (7+ files reference 2 names for same secret)

**Scope:**
- Canonical name: `OPENROUTER_KEY`
- Add `rc-common::secrets::openrouter_key()` helper that reads `OPENROUTER_KEY`, falls back to `OPENROUTER_API_KEY` with `tracing::warn!` deprecation once per process
- Migrate the 7 direct `std::env::var` call sites to the helper (ai/providers.rs, openrouter.rs, mma_diagnosis.rs, mma_engine.rs, ai_behavior_batch_mma.rs, server_diagnostics_infra.rs, ai_debugger.rs)
- Migrate whatsapp-bot `process.env.OPENROUTER_API_KEY` → `OPENROUTER_KEY` (dual-read shim)
- Update `.env.production.local` + deploy docs: `OPENROUTER_KEY` is canonical

**Entry:**
- Sign-off that `OPENROUTER_KEY` is the canonical name
- No in-flight MMA audit running (those scripts read the env var directly)

**Exit:**
- Grep count of `std::env::var("OPENROUTER_API_KEY")` outside the helper = 0
- Grep count of `process.env.OPENROUTER_API_KEY` outside dual-read = 0
- Existing MMA script run with only `OPENROUTER_KEY` set succeeds (evidence = non-zero-tokens API response)
- With only `OPENROUTER_API_KEY` set, deprecation warn fires once + script still succeeds

**Isolation:**
- Additive: new helper added, old call sites still work during migration
- Single-flag: none needed — behavior unchanged
- Per-target: commit-by-commit migration; each commit compiles + tests
- Kill-switch: revert commit

**Rollback:** `git revert <phase-head>`

**NOT covered (explicit):**
- `OPENROUTER_MGMT_KEY` — separate env var for child-key provisioning, keeps its name
- Any other drift pair (that's Phase 447)

---

### Phase 447 — Canonicalize `RACECONTROL_TERMINAL_SECRET` + remove hardcoded secret fallbacks

**Readiness:** **IMMEDIATE**
**Dev estimate:** 1 day
**Kills:** hardcoded-secret-in-source class + dual-name drift (RC_TERMINAL_SECRET vs RACECONTROL_TERMINAL_SECRET)

**Scope:**
- Canonical name: `RACECONTROL_TERMINAL_SECRET`
- Same dual-read pattern as Phase 446 (accept old name, warn, migrate)
- Remove hardcoded fallback `'rp-terminal-2026'` in [whatsapp-bot/src/services/notificationRouter.js:14](racingpoint/whatsapp-bot/src/services/notificationRouter.js#L14) and [staffAlertService.js:6](racingpoint/whatsapp-bot/src/services/staffAlertService.js#L6) — fail loudly on missing env
- Remove hardcoded `EVOLUTION_API_KEY` literal at [whatsapp-bot/src/config.ts:4](racingpoint/whatsapp-bot/src/config.ts#L4)
- Remove hardcoded `OPENROUTER_KEY` in CLAUDE.md standing rules (historical doc — replace with note "see dashboard")

**Entry:**
- Phase 446 merged (establishes dual-read pattern)
- Confirmed: all production pm2/service configs actually set `RC_TERMINAL_SECRET` today (grep VPS pm2 config)

**Exit:**
- Grep for literal `'rp-terminal-2026'` = 0 hits in source
- Grep for `'zNAKEHsXudyqL3dFngyBJAZWw9W4hWN0'` = 0 hits in source
- whatsapp-bot boot with no env set logs `FATAL missing RACECONTROL_TERMINAL_SECRET` and exits non-zero (verified from Bono VPS pm2 logs)

**Isolation:**
- Additive for the dual-read shim
- The hardcoded-removal is a breaking change IF env is unset — mitigated by grep-verifying pm2 configs already set it

**Rollback:** `git revert <phase-head>` — hardcoded fallback returns if env actually missing

**NOT covered:**
- Moving secrets to Windows Credential Manager or Vault (post-v2)

---

### Phase 448 — `rc-common::secrets` central loader

**Readiness:** **IMMEDIATE** (depends on 446 pattern, but can proceed in parallel)
**Dev estimate:** 2-3 days
**Kills:** scattered-env-read class; establishes single-owner for all S-class secrets

**Scope:**
- New module `crates/rc-common/src/secrets.rs`
- Typed getters for every S-class entry in `config-inventory.md`:
  - `jwt_secret()`, `admin_pin_hash()`, `terminal_secret()`, `relay_secret()`, `evolution_api_key()`, `gmail_client_secret()`, `gmail_refresh_token()`, `sync_hmac_key()`, `encryption_key()`, `hmac_key()`, `openrouter_key()`, `openrouter_mgmt_key()`, `anthropic_api_key()`, `rcagent_service_key()`, `rcsentry_service_key()`, `guardian_comms_key()`, `comms_psk()`, `nvr_credentials()`
- Each getter: reads env, memoizes, returns `Result<SecretString>` (uses `secrecy::SecretString` to prevent accidental log leakage)
- Migrate existing scattered call sites (post-446/447) — each crate's `config/mod.rs` delegates to `rc_common::secrets`
- Unit tests: missing env → error; present env → `SecretString` without leaking into Debug

**Entry:**
- Phase 446 merged (OPENROUTER_KEY single name)
- Phase 447 merged (RACECONTROL_TERMINAL_SECRET single name, no hardcoded fallbacks)

**Exit:**
- Grep for `std::env::var("RACECONTROL_` outside `rc_common::secrets` = 0
- Grep for `std::env::var("OPENROUTER` outside `rc_common::secrets` = 0
- `cargo test -p rc-common` passes

**Isolation:**
- Additive — old call sites keep working during migration
- Per-crate migration (one crate PR at a time)

**Rollback:** per-commit revert (each crate migration is independent)

**NOT covered:**
- OS-identity env (COMPUTERNAME, APPDATA, etc.) — out of scope per inventory
- `RC_IS_CLOUD` and boot-flag env vars — keeps reading `std::env::var` directly (documented exception)

---

### Phase 449 — CI drift-detector for config reads

**Readiness:** **IMMEDIATE** (no runtime impact by definition)
**Dev estimate:** 1-2 days
**Kills:** regression risk — new code re-introducing the drift the prior 3 phases cleaned up

**Scope:**
- New GitHub Actions workflow job `config-drift-lint`
- Rust side: grep diff for new `std::env::var(` outside `rc_common::secrets::*` or boot-flag allowlist → fail CI
- TS side: grep diff for new `process.env.` outside `whatsapp-bot/src/config.ts` (the canonical whatsapp config module) or `.next/` build output → fail CI
- Allowlist file `.github/config-drift-allowlist.txt` lists known legitimate exceptions (boot flags, OS identity, test-only)
- Pre-commit hook copy: same check locally before push

**Entry:**
- Phase 448 merged (drift-detector only makes sense after loader exists)

**Exit:**
- Opening a PR that adds `std::env::var("SOMETHING_NEW")` outside allowlist → CI red
- Opening a PR that adds a proper `rc_common::secrets::something_new()` call → CI green
- 31-item security-check.js security gate still green (Phase 449 doesn't break existing gates)

**Isolation:**
- CI-only, zero runtime impact

**Rollback:** remove workflow YAML

**NOT covered:**
- Runtime drift-detection (separate phase if needed post-P1)

---

### Phase 450 — Reconcile `kiosk_settings` with Phase 177

**Readiness:** **GATED** on 448/449
**Dev estimate:** 3-4 days
**Kills:** parallel-write-path class — `kiosk_settings` DB is written by 4+ code paths that bypass Phase 177's `config_push_queue`; this was the root cause surface for the 2026-04-18 lockdown incident

**Scope:**
- Decision (per recommendation): `kiosk_settings` stays as the read model; Phase 177 `config_push_queue` writes through to `kiosk_settings`
- Identify every `INSERT/UPDATE kiosk_settings` site outside Phase 177 (grep)
- Route each through Phase 177's `push_config` endpoint
- Optional: DB trigger rejecting direct UPDATEs on `kiosk_settings` from non-Phase-177 session
- Integration test: WS-push a `kiosk_lockdown_enabled=false` via Phase 177 → verify `kiosk_settings` row updated + `config_audit_log` entry created + FlagSync received by connected test pod

**Entry:**
- Phase 449 CI drift-detector live (so any backslide is blocked)
- Fresh grep of every `kiosk_settings` write site — exhaustive list in phase CONTEXT.md

**Exit:**
- Grep of `UPDATE kiosk_settings` and `INSERT INTO kiosk_settings` outside `api/config_push.rs` + migration files = 0
- Integration test green on James local (verified behavior — not health endpoint)
- 2026-04-18 lockdown-replay test: staff-triggered lockdown via Phase 177 correctly flips fleet kiosk state, audit-logged, rollback tested

**Isolation:**
- Dual-write phase FIRST — old UPDATEs also write to `config_audit_log` for 1 week
- Then read-only `kiosk_settings` enforcement
- Kill-switch: re-enable direct UPDATEs (feature flag in Phase 177)

**Rollback:** flip Phase 177 feature flag `kiosk_settings_direct_write_allowed=true`

---

### Phase 451 — `/api/v1/client-config` runtime endpoint

**Readiness:** **GATED** on 448
**Dev estimate:** 4-5 days
**Kills:** build-time-baked class — 20+ `NEXT_PUBLIC_*` sites need rebuild+redeploy for a config change

**Scope:**
- New endpoint `/api/v1/client-config` (staff/customer JWT or kiosk service-key gated per field)
- Returns typed JSON: `{ api_url, ws_url, sentry_url, sentry_ws, gateway_url, is_cloud, ws_token }`
- SSE event `config_updated` on Phase 177 flag or config push affecting client-visible fields
- Client-side TS helper `useClientConfig()` — fetches at boot, subscribes to SSE, falls back to 60s poll if SSE drops
- Generated types via ts-rs (Phase 445 pattern) so admin/kiosk/pwa share the shape

**Entry:**
- Phase 449 CI drift-detector live
- ts-rs wired in racecontrol (Phase 445 Wave 1+ already provides this)

**Exit:**
- `curl http://host:8080/api/v1/client-config` returns valid JSON matching TS type (verified from James local + Bono VPS)
- SSE test: Phase 177 flag change triggers `config_updated` event within 2s
- Admin page using `useClientConfig()` reloads API URL without page refresh (verified visually — not bundle check)

**Isolation:**
- Additive — endpoint is new; no existing `NEXT_PUBLIC_*` site changes yet
- Frontend sites opt in one at a time (Phase 452)

**Rollback:** remove endpoint route registration

---

### Phase 452 — Migrate `NEXT_PUBLIC_*` call sites onto `/api/v1/client-config`

**Readiness:** **GATED** on 451
**Dev estimate:** 3-4 days
**Kills:** build-time drift fleet-wide — currently a URL change means rebuild + redeploy every frontend

**Scope:**
- Enumerate 20+ sites from `config-inventory.md` (kiosk + pwa + web)
- Migrate in this order (safest first):
  1. Non-critical read-only sites (spectator, leaderboard-display, login page health-check)
  2. Kiosk data-fetch sites (api.ts)
  3. WS sites (useKioskSocket, useWebSocket) — **risky** because WS URL change = reconnect storm
  4. PWA booking flow
  5. Admin page
- Per-site: Feature-flag gate (`client_config_endpoint_enabled`) lets each site flip individually
- Keep `NEXT_PUBLIC_*` env vars as fallback during transition — remove only after all sites migrated + 7-day soak

**Entry:**
- Phase 451 merged, endpoint live on both venue + cloud
- 24h soak on endpoint at Bono VPS with zero 5xx

**Exit:**
- Grep of `process.env.NEXT_PUBLIC_API_URL` in `web/src`, `kiosk/src`, `pwa/src` = 0 (outside legacy fallback block)
- Full staff PIN login flow works on POS browser + James local browser + Pod 4 kiosk (evidence = actual login success screenshots)
- `.env.production.local` reduced to bootstrap + build-metadata vars only

**Isolation:**
- Per-file flag-gated migration
- Per-environment rollout (James local first → Bono VPS → venue kiosk fleet)
- Kill-switch per site: flip flag, site uses old NEXT_PUBLIC read

**Rollback:** per-site flag flip

---

## P2-P7 — placeholder phase numbers (details when kicked off)

Not yet phase-numbered because entry criteria depend on P1 outcomes. Will be assigned numbers starting 460+ when P1 verification closes.

| Phase | Entry criteria | Intended kickoff |
|---|---|---|
| **P2 Event ledger** | P1 green; F1 pre-work (design doc exists) | ~6 weeks from now |
| **P3 Finish Phase 445** | Phase 445 Wave 5 branch rebased on clean base (split from unverified POS `ff12f161`) | Now, if branch surgery done |
| **P4 Monorepo** | P3 complete; ts-rs extended to PWA + whatsapp-bot | ~8 weeks from now |
| **P5 One supervisor** | P1 green; Pattern I Part 4 MMA Steps 1+2 complete | ~10 weeks from now — THE hardest phase |
| **P6 One kiosk runtime** | P2 complete (AC telemetry writes to ledger); kiosk-swap-verify.ps1 soak green on 8 pods | ~16 weeks from now |
| **P7 Real CI/CD** | P5 complete (unit files exist as deploy target); ghcr.io repo + cosign keys provisioned | ~20 weeks from now |

---

## Immediate-start summary

**4 phases can kick off right now with zero fleet risk:**

- **Phase 446** Canonicalize OPENROUTER_KEY (1-2 days)
- **Phase 447** Canonicalize RACECONTROL_TERMINAL_SECRET + remove hardcoded secrets (1 day)
- **Phase 448** rc-common::secrets central loader (2-3 days)
- **Phase 449** CI drift-detector (1-2 days)

**Total P1 immediate-start budget: ~1 week of dev work.** All commits are source-only, CI-only, or whatsapp-bot-only. Zero pod deploys required. Zero server binary swaps. Fully revertable per-commit.

**3 phases GATED after the first 4:**

- **Phase 450** Reconcile kiosk_settings (3-4 days) — needs 448/449 pattern first
- **Phase 451** `/api/v1/client-config` endpoint (4-5 days) — needs 448 + Phase 445 Wave 1 ts-rs
- **Phase 452** Migrate NEXT_PUBLIC_* (3-4 days) — needs 451

**Total P1 full budget: ~3 weeks dev + 1 week rollout soak.**

---

## Kickoff command sequence (when Uday approves)

```bash
# Sequential — each waits for prior to verify green
/gsd:plan-phase 446
/gsd:execute-phase 446
/gsd:verify-work 446

/gsd:plan-phase 447
/gsd:execute-phase 447
/gsd:verify-work 447

/gsd:plan-phase 448
/gsd:execute-phase 448
/gsd:verify-work 448

/gsd:plan-phase 449
/gsd:execute-phase 449
/gsd:verify-work 449

# After 449 green, GATED phases unblock:
/gsd:plan-phase 450  # only after fresh kiosk_settings grep inventory
/gsd:plan-phase 451  # only after Phase 445 ts-rs path confirmed extensible
/gsd:plan-phase 452  # only after 451 SSE soak
```

Or run autonomously via `/gsd:autonomous` once Uday green-lights.

---

## What's still blocking

- Sign-off on the 10 locked recommendations (user said "on your recommendations" — treating as approved)
- Sequencing gates from V2-FOUNDATION-MILESTONE.md: Phase 445 Wave 5 split, Pattern I Parts 4/5, Phase 414 deploy, F4 PR — **none of these block Phase 446-449** (those are S-class secret refactors on orthogonal code paths). They DO block Phase 450+ which touches kiosk_settings / typed contracts / supervisor.
- Venue connectivity restored (POS + Pod 4 audit resume) — NOT a blocker for 446-449 which ship via source commits only.

**Recommended kickoff: Phase 446 can start as soon as Uday says go.**
