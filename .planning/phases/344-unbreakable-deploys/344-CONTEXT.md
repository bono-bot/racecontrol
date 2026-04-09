# Phase 344: Unbreakable Deploys — Context

**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Wave:** 1 (no Phase 343 dependency)
**Status:** Ready to execute
**Depends on:** Nothing (first phase of v47.0)
**Blocks:** Phase 345, 346, 347 (all need working deploys)

## Why this phase exists

The 2026-04-09 Admin Dashboard audit found 4 P0 deploy-related failures:
1. **Cloud admin login returns 500** — `Error: RC_URL environment variable is required` at module load. PM2 starts `.next/standalone/server.js` which does NOT auto-load `.env.local`.
2. **Cloud admin `.next/standalone/.next/static/` missing** — static assets 404 because deploy never copied them into the standalone tree.
3. **Local admin `better-sqlite3` ABI mismatch** — venue runs Node 24, binding was built for Node 22 (NODE_MODULE_VERSION 127 vs 137). Every admin.db API throws at first `getDb()` call.
4. **Six stale `deploy-staging/set-*pin*.js/ps1/py` scripts** hardcode `130424` but real admin PIN is `8141` — they silently 401 if anyone still uses them (Phase 343 finding C3).

All four are missing deploy steps. The root cause is: "deploy is a prose procedure in memory, not executable code." Fix: ONE script with a verify gate that fails loudly.

## Scope

| In scope | Out of scope |
|---|---|
| `racingpoint-admin/scripts/admin-deploy.sh` | Actual live venue/cloud redeploys (trigger in execute phase) |
| `racingpoint-admin/scripts/verify-deploy.js` | CI integration (future phase) |
| `racingpoint-admin/scripts/server-bootstrap.js` | Full rollback strategy (basic rollback only) |
| `.nvmrc` + `package.json` engines pin | Venue Node 24→22 downgrade (pre-work, tracked separately) |
| `start-admin.bat` env exports (venue) | Litestream install (Phase 349) |
| PM2 `ecosystem.config.js` env vars (cloud) | WhatsApp alert wiring (Phase 352) |
| Archive/delete 6 stale PIN scripts | Deploy-script integration with GitHub Actions |

## Requirements covered

- ADMIN-01: Single admin-deploy.sh works on venue + cloud
- ADMIN-02: Post-deploy verify gate
- ADMIN-03: Rollback within 60s
- ADMIN-04: Node version pinned
- ADMIN-05: Previous build preserved 72h
- ADMIN-06: Six stale scripts archived
- ADMIN-07: Env var validation before Node start

## Key decisions

- **D-01:** `admin-deploy.sh` is bash (works on Git Bash on Windows, native bash on Linux). No PowerShell script — one source of truth.
- **D-02:** `server-bootstrap.js` is a thin wrapper that sources env from `.env.production.local` then requires `./server.js`. Platform-agnostic.
- **D-03:** `verify-deploy.js` runs as a post-step in `admin-deploy.sh` and exits non-zero on any failure. Fails the deploy.
- **D-04:** Rollback uses a `prev/` symlink-or-copy strategy. On each deploy: current → prev; new → current. Rollback: current → trash; prev → current; restart.
- **D-05:** Node version pinned via 3 layers: `.nvmrc` (devs), `package.json#engines` (npm warnings), deploy-script `node -v` check (hard fail).
- **D-06:** Stale `set-*pin*` scripts are MOVED to `deploy-staging/archived/` with a README explaining why. Git history preserves them.
- **D-07:** The verify gate checks EXACTLY these 5 things:
  1. `.next/standalone/.next/static/` exists and has files
  2. `better-sqlite3` loads without ABI error
  3. Required env vars present (`RC_URL`, `RC_JWT_SECRET`, `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_GATEWAY_URL`)
  4. HTTP listener responds on `:3201`
  5. `POST /api/auth/login` with real PIN returns 200 with cookie

## References

- Audit finding trail: `.planning/phases/343-staff-pin-hardening/343-COORDINATION.md` → "Audit findings feeding v47.0 Feature 1"
- PITFALLS: `.planning/research/PITFALLS-v47.md` P0 pitfalls #2, #3, #4
- CLAUDE.md deploy lessons (especially `feedback_admin_deploy_path.md`)
- Memory file: `~/.claude/projects/C--Users-bono/memory/feedback_admin_deploy_path.md`

## Canonical file paths

- Source repo: `C:/Users/bono/racingpoint/racingpoint-admin/`
- Venue deploy target: `C:\RacingPoint\admin\`
- Venue runner: `C:\RacingPoint\start-admin.bat` → Scheduled Task `StartAdminSvc`
- Cloud deploy target: `/root/racingpoint/racingpoint-admin/`
- Cloud runner: PM2 process `racingpoint-admin` (id 22)
- Cloud nginx: `/etc/nginx/sites-enabled/racingpoint.cloud` → `admin.racingpoint.cloud` → `127.0.0.1:3201`

## Success criteria

1. Fresh VM deploy completes in <3 minutes from clone to running admin
2. Post-deploy verify gate catches all 4 known P0 failure modes
3. Rollback command reverts to previous build within 60 seconds
4. Deploy script fails loudly on any step error
5. Six stale `deploy-staging/set-*pin*` scripts deleted from git

---

*Phase 344 — scaffolded 2026-04-09 for milestone v47.0.*
