---
phase: 414-continuous-billing-session
plan: 06
status: PARTIAL — Task 1 done; Tasks 2-5 deferred to Uday manual execution
date: 2026-04-18
---

# Phase 414 Plan 06 — PARTIAL Summary (autonomous portion done)

## Tasks status

| Task | Description | Status | Artifact |
|------|-------------|--------|----------|
| 1 | MMA audit | DONE (substituted) | [414-MMA.md](414-MMA.md) (commit `19824faf`) |
| 2 | Venue financial E2E | DEFERRED to Uday | Awaits venue presence + real wallet |
| 3 | Server .23 racecontrol deploy | DEFERRED to Uday (Option α) | Binary staged at `~/racingpoint/deploy-staging/racecontrol-19824faf.exe` |
| 4a | Bono VPS racecontrol deploy | DEFERRED until venue green | — |
| 4b | Server .23 kiosk frontend | DEFERRED to Uday (Option α) | Tarball staged at `/tmp/kiosk-414.tar.gz` |
| 4c | Bono VPS kiosk + 3 cloud frontends | DEFERRED until venue green | — |
| 4d | First-run venue verify | DEFERRED to Uday | Awaits live session post-deploy |
| 5 | Final ship-gate sign-off | DEFERRED until 1-4 complete | — |

## Task 1 (MMA audit) — Done

**Deviation:** Single-model focused review (Sonnet via feature-dev:code-reviewer) substituted for the planned 5-model consensus. `scripts/multi-model-audit.js` auto-loaded full codebase (727k tokens), exceeded all 5 model context windows (163k–262k limits). Two retries per model exhausted — no model returned a result. Tracked: deferred 5-model run as Phase 415 backlog item (script needs `--input` flag fix).

**Findings:** 0 P0 / 1 P1 NEW (resolved `8a52cc36`) / 1 P1 PRE-EXISTING (accepted, tracked) / 2 P2 (deferred Phase 415) / 2 NIT (intentional behavior).

**Resolution of P1-A (idle_auto_end_queued one-shot guard):**
- `crates/racecontrol/src/billing.rs` — added `idle_auto_end_queued: bool` field
- `crates/racecontrol/src/billing_timer.rs:176` — guarded push with `&& !timer.idle_auto_end_queued`; sets flag inside lock
- `crates/racecontrol/src/billing_session_start.rs:339`, `billing_session_lifecycle.rs:356`, `billing_orphan.rs:109` — initialized `false` in 3 explicit struct literals
- `crates/racecontrol/src/billing_game_status.rs:255, 341` — reset to `false` alongside `idle_warning_sent` on resume + game-stop
- 187 billing tests pass post-fix

**Ship recommendation from MMA reviewer:** SHIP (audit-dimensions all pass).

## Tasks 3 + 4b (server .23 deploy) — Artifacts ready, awaits Uday

**Why deferred:** `scripts/deploy-server.sh` requires `SENTRY_KEY` env var (server's rc-sentry service key). Sandbox correctly denied SSH credential discovery on production. Uday holds the credential.

**Pre-staged artifacts:**

| Artifact | Path | Size | SHA256 |
|----------|------|------|--------|
| racecontrol binary | `~/racingpoint/deploy-staging/racecontrol-19824faf.exe` | 60,486,656 B | `cb7a316616d7f33796376f767cce4672a4851190711676a86f8113c0168af9f4` |
| racecontrol binary (alias) | `~/racingpoint/deploy-staging/racecontrol.exe` | same | same |
| kiosk frontend bundle | `/tmp/kiosk-414.tar.gz` | 158,098,582 B | (re-tar with hash if you need verification) |

**Deploy commands for Uday (Option α):**

```bash
cd C:/Users/bono/racingpoint/racecontrol
export SENTRY_KEY="<from C:\\RacingPoint\\racecontrol.toml on .23>"
bash scripts/deploy-server.sh

# After server health 200 + build_id == 19824faf:
scp /tmp/kiosk-414.tar.gz ADMIN@100.125.108.37:C:/RacingPoint/kiosk-414.tar.gz
ssh ADMIN@100.125.108.37 "cd C:/RacingPoint && tar xzf kiosk-414.tar.gz -C kiosk-new --strip-components=0 && schtasks /End /TN StartKiosk && schtasks /Run /TN StartKiosk"
# (adjust kiosk extract path if different — kiosk app lives at C:\RacingPoint\kiosk per existing deploys)
```

**Post-deploy verification (Uday runs from any LAN machine):**

```bash
# Backend build_id
curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id
# Expected: "19824faf"

# Fleet pods still connected
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '[.[] | select(.ws_connected == true)] | length'
# Expected: 8

# Kiosk API proxy (CRITICAL per CLAUDE.md — must return JSON, NOT HTML)
curl -s http://192.168.31.23:3300/api/v1/health
# Expected: JSON with build_id

# Kiosk deep health
curl -s http://192.168.31.23:3300/kiosk/api/health/deep | jq .healthy
# Expected: true
```

## Tasks 2 + 4d (venue verification) — Awaits Uday at venue

Per `414-06-PLAN.md` Task 2 `<how_to_verify_human>` section, run 4 financial E2E tests on Pod 8 (canary):

1. **TEST 1 — Cumulative snap single-game swap:** start session → AC 10min → exit AC → wait 2min → F1 25 10min → end session → wallet delta = ₹500 (NOT ₹250+₹250 separately)
2. **TEST 2 — Cumulative snap CROSSES 30-min threshold:** AC 25min → F1 25 5min → end → wallet delta = ₹700 (snap, NOT ₹750)
3. **TEST 3 — IdleWarning + 15-min auto-end:** AC 5min → exit → wait 10min → IdleWarningDialog appears with countdown 5:00 → wait 5 more min → auto-end as Completed → charged for 5min only (₹125)
4. **TEST 4 — Out-of-credits Branch B:** balance ~₹30 driver → AC 1min → exit → wait 10min → IdleWarningDialog "Out of credits" + sole "End session" CTA

Document in `414-FINANCIAL-E2E.md` per the plan's table format.

## Tasks 4a + 4c (cloud parity to Bono VPS) — Held until venue green

After Uday confirms venue tests pass, James resumes:
- `git push` (already done; just verify Bono can pull)
- comms-link relay `git_pull` on Bono VPS
- `cargo build --release` on Bono VPS
- `pm2 restart racecontrol` on Bono VPS
- Verify `https://racecontrol.cloud/api/v1/health | jq .build_id` == `19824faf`
- Cloud kiosk rebuild via comms-link
- Verify `https://kiosk.racingpoint.cloud/api/v1/health` returns JSON
- Update LOGBOOK + STATE + ROADMAP marking Phase 414 SHIPPED

## Task 5 (final ship-gate) — After all above

11-item checklist per Plan 06 Task 5. James drives once cloud parity confirmed.

## Outstanding work tracked

- Plan 415 backlog (per MMA report): 5-model audit re-run with `--input` flag fix; pre-existing dual-lock pattern cleanup; FSM `PausedManual + EndEarly` gap; `seconds_remaining` dynamic computation
- Wave 6 Tasks 2, 3, 4, 5 status reset to "open — awaits Uday/venue"

## Total Phase 414 metrics so far

- 7 plans across 7 waves
- 22 commits between `94d91d19` (pre-Phase-414 base) and `19824faf` (HEAD)
- 21 files modified across `crates/`, `kiosk/`, `web/`, `packages/`
- 19 REQ-IDs from VALIDATION.md mapped to plans
- 18 UI-SPEC AC items code-verified (venue verification deferred)
- ~3500 LOC added/modified
- 187 billing tests + 254 rc-common tests + 54 TS contract tests all GREEN
- 0 production deploys (all backend code already on .23 via parallel session's deploy of `68f4d61e` ancestor; only the MMA P1-A fix delta + docs remain)
