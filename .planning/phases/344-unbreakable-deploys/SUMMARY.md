# Phase 344: Unbreakable Deploys — SUMMARY

**Status:** CODE-COMPLETE (not live-deployed)
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Completed:** 2026-04-09

## What Was Built

### 344-01 + 344-02: Deploy Scripts + Node Pin (`racingpoint-admin@b10b487`)

All delivered in a single commit in the `racingpoint-admin` repo:

1. **`scripts/admin-deploy.sh`** — Unified bash deploy for venue Windows + cloud Linux
   - Node 22 hard check (fails loudly on wrong version)
   - Archive prev build to `.next/prev-standalone` for 72h+ rollback
   - `npm ci` + `next build` + copy static assets into standalone tree
   - `npm rebuild better-sqlite3` against runtime Node (ABI fix)
   - Copy `server-bootstrap.js` + `.env.production.local` into standalone
   - Pre-start verify (static checks) + restart via `$ADMIN_RESTART_CMD` + live verify
   - `--rollback` flag reverts to prev build in <60s
   - `--dry-run` builds without restart

2. **`scripts/verify-deploy.js`** — 5-check post-deploy gate (exits 1 on ANY failure)
   - `.next/standalone/.next/static/` exists + has files
   - `better-sqlite3` loads (ABI check)
   - Required env vars present (`RC_URL`, `RC_JWT_SECRET`, `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_GATEWAY_URL`)
   - HTTP `/api/health` returns 200 (live only)
   - `POST /api/auth/login` round-trip with cookie (live only, if `ADMIN_TEST_PIN` set)

3. **`scripts/server-bootstrap.js`** — Env loader wrapper for standalone `server.js`
   - Loads `.env.production.local` into `process.env` (runner wins)
   - Asserts required env vars, exits 1 with clear error
   - Sets `PORT=3201`, `HOSTNAME=0.0.0.0` defaults

4. **`.nvmrc`** — Pins Node 22 for nvm users
5. **`package.json`** — `engines: ">=22.0.0 <23.0.0"` + deploy/verify/rollback npm scripts
6. **`next.config.ts`** — `outputFileTracingRoot: path.join(__dirname)` (prevents build-machine-path bug)

### 344-03: Archive Stale PIN Scripts

8 scripts moved from `deploy-staging/` to `deploy-staging/archived/stale-pin-scripts-v47/`:
- `set-pin.js`, `set-pin-node.js`, `set-pin.ps1`, `set-pin.py`, `set-pin.json`
- `set-staff-pin.json`, `update-pin.js`, `set-vishal-pin.js`

All hardcoded stale admin PIN `130424`. README.md explains why archived and canonical replacement (admin `/admin/staff` page from Phase 347).

## Success Criteria Verification

| SC | Criteria | Status | Evidence |
|----|----------|--------|----------|
| SC-1 | Fresh deploy <3 minutes from clone to running | CODE-COMPLETE | 9-step pipeline, not live-tested |
| SC-2 | Verify gate catches all 4 P0 failure modes | CODE-COMPLETE | 5 checks in verify-deploy.js |
| SC-3 | Rollback within 60 seconds | CODE-COMPLETE | `--rollback` flag + prev-standalone |
| SC-4 | Deploy script fails loudly on any step error | CODE-COMPLETE | `set -euo pipefail` + verify gate |
| SC-5 | Six stale PIN scripts deleted from deploy-staging root | DONE | 8 scripts archived (found 8, not 6) |

## Not Deployed / Not Runtime-Tested

- Running `admin-deploy.sh` against live venue (.23) or cloud (Bono VPS)
- Venue `start-admin.bat` update to use `server-bootstrap.js`
- Cloud pm2 `ecosystem.config.js` env update
- Venue Node 24 → 22 downgrade (tracked as separate pre-work)
- Login round-trip verification (needs `ADMIN_TEST_PIN` env + live server)

## Tests

- `node -c scripts/verify-deploy.js` — valid JS syntax
- `node -c scripts/server-bootstrap.js` — valid JS syntax
- `bash -n scripts/admin-deploy.sh` — valid bash syntax

## Deploy Requirements

- **Venue (.23):** Update `start-admin.bat` to call `node server-bootstrap.js` instead of `node server.js`. Downgrade Node to 22 LTS.
- **Cloud (Bono VPS):** Update pm2 ecosystem.config.js to use `server-bootstrap.js` as script entry. Ensure Node 22.
- **No DB changes, no Rust changes.**
