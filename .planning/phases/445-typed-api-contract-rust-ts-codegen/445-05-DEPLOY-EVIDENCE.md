---
phase: 445-typed-api-contract-rust-ts-codegen
plan: 05
artifact: deploy-evidence
---

# Phase 445 Plan 05 — Cloud Deploy Evidence

**Timestamp:** 2026-04-21 22:07 IST (Tuesday)
**Triggered by:** James (claude-code on James .27 + SSH to Bono root@100.70.177.44)
**Cloud host:** Bono VPS (srv1422716.hstgr.cloud, Tailscale 100.70.177.44)
**Public host:** admin.racingpoint.cloud
**Merge commit (racecontrol main):** `d52a8a72` — Merge pull request #9 from bono-bot/feat/phase-445-typed-api-contract
**Admin git_commit:** `dfaabe6` (racingpoint-admin main, in sync with origin)
**Admin Next.js build_id:** `cDyHRUgWTiqZTchmlEPgz`

## Step A — racecontrol HEAD on Bono (post-merge)

```
$ ssh root@100.70.177.44 "cd /root/racingpoint/racecontrol && git branch --show-current && git log -1 --oneline && git fetch origin main && git log origin/main -1 --oneline"

main
d52a8a72 Merge pull request #9 from bono-bot/feat/phase-445-typed-api-contract
From github.com:bono-bot/racecontrol
 * branch              main       -> FETCH_HEAD
d52a8a72 Merge pull request #9 from bono-bot/feat/phase-445-typed-api-contract
```

Bono's racecontrol checkout is on main, HEAD matches the PR #9 merge commit produced 16:28:09Z.

## Step B — racingpoint-admin HEAD on Bono

```
$ ssh root@100.70.177.44 "cd /root/racingpoint/racingpoint-admin && git branch --show-current && git log -1 --oneline && git fetch origin main && git log origin/main -1 --oneline"

main
dfaabe6 Merge remote-tracking branch 'origin/main'
From github.com:bono-bot/racingpoint-admin
 * branch            main       -> FETCH_HEAD
dfaabe6 Merge remote-tracking branch 'origin/main'
```

racingpoint-admin already at latest main (no pull needed). Path alias `@racingpoint/types` → `../racecontrol/packages/shared-types/src/index.ts` resolves into Phase 445's new generated/ re-exports automatically when admin rebuilds.

## Step C — Bono admin tsc + npm run build

```
$ ssh root@100.70.177.44 "cd /root/racingpoint/racingpoint-admin && npx tsc --noEmit && npm run build"

(tsc: no output, exit 0)
... [next.js build output, all 32 routes prerendered + 25 dynamic]
postbuild: copied .next/static -> standalone
postbuild: copied public/ -> standalone
postbuild: wrote git-commit.txt (dfaabe6)
postbuild: verified 72 JS chunks in standalone
```

tsc clean (no errors → 445 type re-exports resolve correctly through path alias). Build complete with 72 JS chunks in standalone bundle. git-commit.txt stamped with `dfaabe6`.

## Step D — pm2 restart racingpoint-admin

```
$ ssh root@100.70.177.44 "pm2 restart racingpoint-admin --update-env"

[PM2] Applying action restartProcessId on app [racingpoint-admin](ids: [ 20 ])
[PM2] [racingpoint-admin](20) ✓
... [pm2 list output, racingpoint-admin id=20 status=online, restart counter incremented to 8]
```

## Step E — Cloud admin health probe (from James .27)

```
$ curl -sI --max-time 10 https://admin.racingpoint.cloud/

HTTP/1.1 307 Temporary Redirect
Server: nginx/1.24.0 (Ubuntu)
Date: Tue, 21 Apr 2026 16:36:49 GMT
Connection: keep-alive
location: /login
```

Root path serves nginx → Next.js → middleware redirect to /login (expected — admin behind staff JWT auth).

```
$ curl -s --max-time 10 https://admin.racingpoint.cloud/api/health

{"status":"ok","service":"racingpoint-admin","version":"0.1.0","build_id":"cDyHRUgWTiqZTchmlEPgz","git_commit":"dfaabe6","deploy":{"pages_expected":32,"pages_available":57,"pages_missing":[],"pages_extra":["/billing/analytics","/billing/live","/cafe/promos","/config","/customers/[id]","/fleet/content-drift","/fleet/verify","/games","/hr/recognition","/mesh-intelligence","/metrics","/presets","/pricing/tiers","/sessions/export","/sessions/suspect","/settings/business-rules","/settings/health","/settings/pipeline","/staff","/staff/manage","/wallet/bonus-tiers","/wallet/topup-presets"],"static_assets":true,"healthy":true}}
```

Cloud admin responding through full stack: nginx → Next.js handler → /api/health route returns JSON with `healthy: true`, `pages_missing: []`, `static_assets: true`. Build_id `cDyHRUgWTiqZTchmlEPgz` matches the Bono-local build. git_commit `dfaabe6` matches both Bono local and racingpoint-admin origin/main.

## Step F — racecontrol cloud health (sanity)

```
$ curl -s http://localhost:8766/relay/exec/run -d '{"command":"racecontrol_health"}'
{"execId":"...","exitCode":0,"stdout":"200","durationMs":12}
```

Cloud racecontrol (port 8080) returning 200 on /api/v1/health. Phase 445's new `gen-types` binary is part of the racecontrol crate but only invoked via `cargo run --bin gen-types --features gen-types` for codegen — it does not affect runtime serving. Cloud racecontrol unchanged in behavior.

## Verdict

| Step | Test | Result |
|---|---|---|
| A | Bono racecontrol HEAD = `d52a8a72` (PR #9 merge commit) | Match |
| B | Bono racingpoint-admin HEAD = `dfaabe6` (main tip) | Match |
| C | Bono `npx tsc --noEmit` + `npm run build` exit codes | Both 0 |
| D | `pm2 restart racingpoint-admin --update-env` | Process restarted, status online |
| E | `curl https://admin.racingpoint.cloud/` HEAD | 307 → /login (expected) |
| E | `curl https://admin.racingpoint.cloud/api/health` build_id | `cDyHRUgWTiqZTchmlEPgz` (matches Bono build) |
| E | git_commit echoed in /api/health | `dfaabe6` (matches Bono local + remote main) |
| E | `pages_missing` count | `[]` (zero) |
| E | `static_assets` | `true` |
| E | `healthy` | `true` |
| F | Cloud racecontrol /api/v1/health | 200 |

## NOT TESTED

- **Actual rendered UI of a Phase-445-migrated admin page** (e.g., /fleet-health which consumes `FleetHealthResponse` from generated/) — would require staff JWT + browser navigation, deferred (admin live but not user-tested through UI).
- **Schema-equivalence of admin's runtime API responses against the new generated TS types** — type-correctness only proven at compile time (tsc green), runtime API contract conformance not asserted.
- **Workspace cargo build green on cloud** — Bono runs racecontrol pre-built; rebuild not attempted because Phase 445 added no Rust code path that rc-agent/racecontrol runtime executes (gen-types is a build-only binary).
- **Drift gate full chain on cloud** — `bash scripts/check-generated-types-drift.sh` not invoked on Bono (it's a pre-commit/CI gate, not a runtime check).
- **vitest regression-drift fixture on cloud** — not invoked on Bono.
- **Venue (.23 server)** — Phase 445 deploy_targets=[james, cloud], does not include venue server. Venue is unaffected by 445 (no Rust binary changes invoked at runtime; admin is cloud-only deploy target).
- **POS (.130) and pods 1-8** — Phase 445 does not touch pod or POS code paths.
- **Comms-link** — unrelated to 445; comms-link.git_pull verified separately as up to date.
- **Cargo build of `gen-types` binary on cloud** — only built locally on James for the Phase 445 dev workflow; never deployed because it's a developer-time codegen tool.

## Notes

- Skipped `scripts/admin-deploy.sh` in favor of direct `pm2 restart` because the build was already complete locally on Bono via `npm run build` — admin-deploy.sh would have re-run npm ci + rebuild (~60s extra latency) for no functional benefit.
- Admin pm2 process `racingpoint-admin` (id=20) restart counter went 7→8, consistent with one clean restart cycle.
- racingpoint-admin Bono node version: v22.22.0 (matches admin-deploy.sh's required v22.x check).
- DEPLOY PARITY (UNIVERSAL): Phase 445 only required cloud admin rebuild (per Plan 05 deploy block: `cloud_parity: [frontend], targets: [james, cloud]`). Server .23 racecontrol binary unchanged; no venue deploy required.
