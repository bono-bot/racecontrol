# Runtime Inventory — Bono VPS (control node, 72.60.101.58)

**Captured:** 2026-06-07 (IST) · **Mode:** READ-ONLY observation. No services started/stopped/restarted/rebuilt; no nginx/pm2 bounced; no source/manifest/config changed.
**"This repo"** = the `racecontrol` git repo. This file lives in the worktree `/root/rp-legacy-removal` (branch `chore/remove-v1-admin`), which is a worktree of `/root/racecontrol`. "Repo-relative" paths below are relative to that repo root.

**Method (ground truth, not memory/path/commit-date):** for each listening socket (`ss -ltnp`) the bound process pid is resolved to its actual working directory via `readlink /proc/<pid>/cwd` + `/proc/<pid>/cmdline`. pm2 metadata (`pm2 jlist` → `pm_cwd`/`pm_exec_path`) is recorded *alongside* `/proc` and **discrepancies are flagged** — `/proc` wins. Hostnames are from the live loaded nginx config (`nginx -T`), not repo copies under `cloud/`.

---

## A. Listening services → exact source tree

### A.1 Application / web services (source-tree-backed)

| Port | Bind | pid | process | pm2 name | `/proc/<pid>/cwd` (GROUND TRUTH source tree) | In `racecontrol` repo? | Tree last commit |
|------|------|-----|---------|----------|----------------------------------------------|------------------------|------------------|
| **8080** | 0.0.0.0 | 3432787 | `racecontrol` (Rust bin) | `racecontrol` | `/root/racecontrol` (binary `target/release/racecontrol`) | **INSIDE** — repo root | repo HEAD `6e70a116` |
| **8090** | 0.0.0.0 | 3432787 | `racecontrol` (Rust bin) | `racecontrol` | same process as :8080 | **INSIDE** | — |
| **3500** | 0.0.0.0 | 352999 | next-server | `racingpoint-web-v2` | `/root/racecontrol/web-v2/.next/standalone` | **INSIDE** — `web-v2/` | `0056627b` 2026-05-20 |
| **3501** | 0.0.0.0 | 353050 | next-server | `racecontrol-pwa` | `/root/racecontrol/pwa/.next/standalone` | **INSIDE** — `pwa/` | `5804c535` 2026-05-17 |
| **3201** | 0.0.0.0 | 352965 | next-server | `racingpoint-admin` | `/root/racingpoint-admin/.next/standalone` | **OUTSIDE** — separate git repo `/root/racingpoint-admin` | `4dccbd5` 2026-05-17 |
| **3220** | * | 4133356 | next-server | `racecontrol-console` | `/root/rp-v2-apps/apps/racecontrol-console` | **OUTSIDE** — rp-v2-apps | (rp-v2-apps repo) |
| **3221** | * | 3117908 | next-server | `captain-console` | `/root/rp-v2-apps-wt-v31gapfill/apps/captain-console` | **OUTSIDE** — rp-v2-apps worktree ⚠️ concurrently edited | (rp-v2-apps) |
| **3211** | * | 1526981 | node (tsx, `src/server.ts`) | `rp2-admin-proxy-bono` | `/srv/rp-v2-apps/apps/admin-proxy-bono` | **OUTSIDE** — rp-v2-apps @ `/srv` | (rp-v2-apps) |
| **3213** | * | 2990678 | node (`src/server.ts`) | (docker `rp2-proxy`, node:22) | `/app/apps/admin-proxy-bono` (container fs) | **OUTSIDE** — container `rp2-proxy` (host-network) | (container build) |
| **3150** | * | 353160 | node (tsx) | `whatsapp-bot` | `/root/racingpoint/whatsapp-bot` | **OUTSIDE** | — |
| **3100** | * | 352860 | PM2 cluster master | `racingpoint-api-gateway` | `/root/racingpoint-api-gateway` (cluster) | **OUTSIDE** | — |
| **3050** | * | 352893 | node | `racingpoint-hiring` | `/root/racingpoint-hiring-bot` | **OUTSIDE** | — |
| **3000** | * | 353038 | node | `racingpoint-bot` | `/root/racingpoint-whatsapp-bot` | **OUTSIDE** | — |
| **8765** | * | 352901 | node | `comms-link` | `/root/comms-link` | **OUTSIDE** | — |
| **3395** | 127.0.0.1 | 2303246 | next-server | (not pm2-managed) | `/tmp/kvh/apps/kiosk` | **OUTSIDE** — `/tmp` staging build | — |

> Note: `racecontrol` pm2 entry's `pm_exec_path` is a wrapper (`scripts/exit-trace-lite.sh`); the actual listening pid 3432787 is the Rust binary `target/release/racecontrol` with cwd `/root/racecontrol`.

### A.2 Infrastructure / third-party (not application source trees)

| Port | Process | Notes |
|------|---------|-------|
| 80 / 443 / 8443 | nginx (master 3413603 + workers) | reverse proxy — see §B |
| 8091 | wstunnel (2959315) | `ws://127.0.0.1:8091 --restrict-to 127.0.0.1:51820` (WireGuard-over-wstunnel) |
| 8799 | python3 (3362227) | `python3 -m http.server 8799 --directory /root/racecontrol` (deploy-staging file server) |
| 5432 | postgres (353941) | localhost |
| 6379 | redis-server (353834) | localhost |
| 22 | sshd · 631 cupsd · 53 systemd-resolve · tailscaled | host services |
| 7474 / 7687 | docker → `rp-neo4j` | graphify |
| 5678 | docker → `n8n` |  |
| 3001 | docker → `uptime-kuma` |  |
| 3302 | docker → `rp2-pwa` (node:22) | V2 PWA container (see §B app/apex) |
| 53622 | docker → `evolution-api-...` | WhatsApp (Evolution) |
| 58290 | docker → `paymenter-...` |  |
| 3100 (internal) | docker → `cloud-pwa-1` | container-internal only; host :3100 is the pm2 api-gateway above |

### A.3 Localhost-only, non-public (no public route)

| Port | pid | Identity |
|------|-----|----------|
| 40623 / 37835 / 42541 | 361004 / 232981 / 232925 | Cursor IDE remote server (`.cursor-server/...node`) |
| 3395 | 2303246 | kiosk next-server from `/tmp/kvh/apps/kiosk` (see A.1) |

---

## B. Public hostname → location → port → backing tree (live `nginx -T`)

Only **active** (non-`#DECOMM#`, non-commented) blocks are listed. Source files: `/etc/nginx/sites-enabled/{racingpoint.cloud, racingpoint.cloud-apex, console.racecontrol.in, bono.racingpoint.cloud, default}`.

| Hostname | location | → port | Backing service / source tree |
|----------|----------|--------|-------------------------------|
| `racingpoint.cloud`, `www.racingpoint.cloud` | `/`, `/_next/static/` | 3302 | `rp2-pwa` container (V2 PWA) |
| `racingpoint.cloud`, `www.racingpoint.cloud` | `/api/` | 3213 | `admin-proxy-bono` (container `rp2-proxy`) |
| `app.racingpoint.cloud` | `/`, `/_next/static/` | 3302 | `rp2-pwa` container (V2 PWA) |
| `app.racingpoint.cloud` | `/api/` | 3213 | `admin-proxy-bono` (container `rp2-proxy`) |
| `admin.racingpoint.cloud` | `/` | 3221 | `captain-console` (`/root/rp-v2-apps-wt-v31gapfill/apps/captain-console`) |
| `admin.racingpoint.cloud` | `= /admin`, `~ ^/admin/(.+)$` | 3500 | **`racecontrol/web-v2`** (`/root/racecontrol/web-v2`) |
| `admin.racingpoint.cloud` | `^~ /api/v2/` | 3211 | `admin-proxy-bono` (bare-metal `/srv/rp-v2-apps`) |
| `admin.racingpoint.cloud` (`:8443` ssl) | `/` | 53622 | Evolution API (docker) |
| `rc.racingpoint.cloud` | `/`, `/ws/` | 8080 | `racecontrol` (Rust) |
| `api.racingpoint.cloud` | `/` | 8080 | `racecontrol` (Rust) |
| `api.racingpoint.cloud` | `/webhook/kapso` | 3000/webhook | `racingpoint-bot` |
| `console.racecontrol.in` | `/`, `/_next/static/` | 3220 | `racecontrol-console` (`/root/rp-v2-apps/apps/racecontrol-console`) |
| `bono.racingpoint.cloud` | `/` | 8091 | wstunnel (WG transport) |

**Decommissioned in live config** (`#DECOMM-20260606T153844Z#`, present but inactive): `dashboard.racingpoint.cloud`→:3400, `james.racingpoint.cloud`→:8080/:3500, `cloud.racingpoint.cloud`→:3700, `staff.racingpoint.cloud`→:3300.

---

## C. Same-name directory collisions

For each shared basename, **exactly one** copy is the live source per §A `/proc` evidence; all others are not.

### C.1 `racingpoint-admin`
**LIVE source (serves :3201):** `/root/racingpoint-admin` — its own git repo (`toplevel=/root/racingpoint-admin`), has `package.json` + `.next/standalone`, last commit `4dccbd5` 2026-05-17. **OUTSIDE the racecontrol repo.**
- Also reachable via symlink **`/root/racingpoint/racingpoint-admin → /root/racingpoint-admin`** (this is why pm2 `pm_cwd=/root/racingpoint/racingpoint-admin` and `/proc cwd=/root/racingpoint-admin` differ — *same physical tree*, not a conflict).

**NOT live — in the `racecontrol` repo (orphan: no `package.json`, no `.next/standalone`, last commit `9b6e94f3` 2026-04-11):**
- `/root/racecontrol/racingpoint-admin` (main checkout)
- `/root/rp-legacy-removal/racingpoint-admin` (this worktree)
- `/root/racecontrol-deploy-clean/racingpoint-admin`, `/root/racecontrol-wt-lb/...`, `/root/racecontrol-wt-main/...`, `/root/racecontrol-wt-reaper/...`, `/root/racecontrol-wt-s0/racingpoint-admin`
- `/root/racecontrol/.claude/worktrees/agent-a202b14468c246969/racingpoint-admin`

**NOT live — other:** `/root/vaults/racingpoint-admin` (backup/vault copy).

### C.2 `web-v2`
**LIVE source (serves :3500):** `/root/racecontrol/web-v2` — `racecontrol` repo (main checkout), last commit `0056627b` 2026-05-20. **INSIDE this repo** (`web-v2/`).

**NOT live (same repo, other checkouts):** `/root/rp-legacy-removal/web-v2` (this worktree), `/root/racecontrol-deploy-clean/web-v2`, `/root/racecontrol-wt-{lb,main,reaper,s0}/web-v2`, `/root/racecontrol/.claude/worktrees/agent-a202b14468c246969/web-v2`.

### C.3 `web`
**LIVE source:** **NONE.** No process listens on :3200 (or the decommissioned :3400); `web` is dark fleet-wide on this host.

**On disk (all `racecontrol` repo checkouts, not running; main-checkout last commit `56f67a8f` 2026-05-14):** `/root/racecontrol/web`, `/root/rp-legacy-removal/web`, `/root/racecontrol-deploy-clean/web`, `/root/racecontrol-wt-{lb,main,reaper,s0}/web`, `/root/racecontrol/.claude/worktrees/agent-.../web`.
**Not the app (test artifacts):** `…/tests/visual-regression/__screenshots__/web` in each checkout.

---

## D. Special focus — :3201 and :3220 (+ port-claim flags)

### :3201
- **Process:** pid 352965, `next-server (v16.1.6)`, pm2 name `racingpoint-admin`.
- **Source tree (`/proc`):** `/root/racingpoint-admin/.next/standalone` → repo `/root/racingpoint-admin` — **OUTSIDE** the racecontrol repo (separate git repo). This is the legacy V1 admin.
- **Routing:** **NOT routed by live nginx.** `nginx -T` mentions `3201` only in two *comments*, incl. `# V3 cutover 2026-06-06: captain-console (:3221) replaces V1 admin (:3201).` → the live admin surface is `captain-console` (:3221) + `web-v2 /admin` (:3500). So :3201 is **running but orphaned** (no inbound hostname).
- **Single claimant?** Yes — only pid 352965 listens on :3201. The pm2/`/proc` path difference is the `/root/racingpoint/racingpoint-admin` **symlink** (→ `/root/racingpoint-admin`), not a second tree.

### :3220
- **Process:** pid 4133356, `next-server (v16.1.6)`, pm2 name `racecontrol-console` (pnpm parent pid 4133344).
- **Source tree (`/proc`):** `/root/rp-v2-apps/apps/racecontrol-console` — **OUTSIDE** the racecontrol repo (rp-v2-apps).
- **Routing:** `console.racecontrol.in` → `/` and `/_next/static/` → :3220. Routed and live.
- **Single claimant?** Yes — only pid 4133356 listens on :3220.

### Port-claim / dual-instance flags
- **No listening port is claimed by 2+ processes** — every TCP listener in `ss -ltnp` maps to exactly one pid.
- **`admin-proxy-bono` runs as TWO instances** (same codebase, two deploys, two ports): **:3211** bare-metal (`/srv/rp-v2-apps/apps/admin-proxy-bono`, pm2 `rp2-admin-proxy-bono`) and **:3213** containerized (`rp2-proxy`, node:22, `/app/apps/admin-proxy-bono`). nginx routes `admin.racingpoint.cloud /api/v2/`→:3211 and `app|apex /api/`→:3213.
- **One hostname, three backing trees:** `admin.racingpoint.cloud` is served by `captain-console` (:3221, `/`), `racecontrol/web-v2` (:3500, `/admin*`), and `admin-proxy-bono` (:3211, `/api/v2/`).
- **:3201 pm2 vs /proc cwd mismatch** is explained by the symlink above (same tree) — flagged so it isn't mistaken for a second admin copy.

---

## E. Build-context / deploy-script references (not live deploy mechanism)

The live cloud serve is **nginx + pm2 + the `rp2-proxy`/`rp2-pwa` docker containers** (no Caddy container is running). The repo still contains older references:
- `cloud/compose.yml` (last touched 2026-03-22; **no caddy container live**): `dashboard` build context `./racecontrol/web`; `admin` → `./racingpoint-admin` (the **sibling**, not the in-repo copy); `pwa` → `./racecontrol/pwa`.
- `scripts/deploy/deploy-nextjs.sh:69` → `SRC="C:/Users/bono/racingpoint/racecontrol/web"` (Windows/venue path).
- Knowledge-graph tooling (`graphify-meta/*.mjs`) lists `subdir: 'racecontrol/web'` (read-only graph generation; non-deploy).

---

## F. Summary — what is live, and from where

| Tree (basename) | Live? | Port | Routed hostname | Source of truth (per `/proc`) | In racecontrol repo? |
|-----------------|-------|------|-----------------|-------------------------------|----------------------|
| `web-v2` | **LIVE** | 3500 | `admin.racingpoint.cloud /admin*` | `/root/racecontrol/web-v2` | **INSIDE** (`web-v2/`) |
| `pwa` | LIVE | 3501 | (no active hostname found; running) | `/root/racecontrol/pwa` | INSIDE (`pwa/`) |
| `racecontrol` (Rust) | LIVE | 8080/8090 | `rc.` + `api.racingpoint.cloud` | `/root/racecontrol` | INSIDE (binary) |
| `racingpoint-admin` | running, **UNROUTED** | 3201 | none (superseded by :3221) | `/root/racingpoint-admin` | **OUTSIDE** |
| `racecontrol-console` | LIVE | 3220 | `console.racecontrol.in` | `/root/rp-v2-apps/apps/racecontrol-console` | OUTSIDE |
| `captain-console` | LIVE | 3221 | `admin.racingpoint.cloud /` | `/root/rp-v2-apps-wt-v31gapfill/apps/captain-console` | OUTSIDE (⚠️ concurrently edited) |
| `admin-proxy-bono` | LIVE ×2 | 3211 + 3213 | `admin /api/v2/` + `app|apex /api/` | `/srv/rp-v2-apps/...` + container `/app/...` | OUTSIDE |
| `web` | **DARK** | — (3200/3400 not listening) | none | (no process) | n/a |
| in-repo `racecontrol/racingpoint-admin` | **DARK** (orphan, no build) | — | none | (no process) | INSIDE |

**Caveats:** snapshot at one instant (2026-06-07 IST); process state is temporal. `pwa` (:3501) is running but no active nginx hostname proxies to :3501 in the current config (the `app`/apex PWA route goes to the `:3302` `rp2-pwa` container, not :3501). Docker-internal ports (e.g. `cloud-pwa-1` :3100) are container-local and not the host listeners of the same number.
