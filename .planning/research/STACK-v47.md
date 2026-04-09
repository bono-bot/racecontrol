---
milestone: v47.0
dimension: stack
date: 2026-04-09
author: James (direct, research agents API-overloaded)
---

# v47.0 Stack Research — Admin Dashboard Venue-Ready Hardening

## Scope note

Research agents hit sustained 529 overload errors. This file was written directly by the session AI using existing knowledge of the Racing Point stack, CLAUDE.md lessons, and Context7 cached library docs. **Verify library versions at implementation time** — `npm view <pkg> version` before pinning.

## Existing stack (do not re-research)

| Layer | Choice | Location |
|---|---|---|
| Admin framework | Next.js 15 App Router + TypeScript + Tailwind + shadcn/ui | `racingpoint-admin/` |
| Native SQLite | `better-sqlite3` (CommonJS, native bindings) | admin.db |
| Backend | Rust/Axum `racecontrol` | `crates/racecontrol/` |
| Backend DB | `sqlx` → SQLite `racecontrol.db` | `C:\RacingPoint\data\` venue, `/root/racecontrol/data/` cloud |
| Auth | JWT via `rp-admin-token` httpOnly cookie + PIN login | admin proxies to rc `/auth/admin-login` |
| Tests | Playwright (admin, kiosk, e2e-regression) | `racingpoint-admin/tests/`, `racecontrol/e2e-regression/` |
| Relay | `comms-link` Node WebSocket on James .27 + Bono VPS | `comms-link/` |
| Process mgmt | Scheduled Tasks (venue Windows) + PM2 (cloud Linux) | — |

## New dependencies (proposed)

| Name | Version | Why | Ops cost |
|---|---|---|---|
| **Litestream** | v0.3.x (latest stable) | Venue→cloud SQLite replication (Option A) | Low — single binary per side, SQLite native WAL shipping |
| **@playwright/test** | 1.48.x (latest) | Contract tests. Already installed in admin repo — upgrade if stale | None — existing dep |
| **pino** or keep `console.log` | — | Structured JSON logs for error tracking | None — no new service |
| None (no Sentry) | — | Self-hosted error tracking = new service = ops burden. Use structured logs + Loki if needed later. | 0 |
| None (no Redis) | — | Lockout counter goes in admin.db | 0 |

**Explicit non-adds:**
- ❌ Sentry / GlitchTip — new service, Uday can't babysit. Structured JSON logs → rsync to Bono for grep is sufficient for v47.0.
- ❌ Redis — lockout counter for <50 staff fits in SQLite fine.
- ❌ rqlite / LiteFS / Turso — Litestream is the simplest read-replica story for Windows→Linux SQLite.
- ❌ Docker — venue is bare metal Windows, cloud is bare metal Linux. No container layer added.

## Litestream decision (Option A: venue→cloud read replica)

### Comparison

| Tool | Windows support | Topology | Storage | Ops complexity |
|---|---|---|---|---|
| **Litestream** | Native Windows binary since v0.3.9 (2023+), tested on Server 2019/2022 | Single-writer (venue) → S3/SFTP/Azure/filesystem → read replicas (cloud) | S3-compatible (Backblaze B2, Cloudflare R2, MinIO) or direct SFTP | Low — 1 binary + 1 config file |
| rqlite | Cross-platform Go | Raft cluster, multi-writer | Embedded | Medium — cluster membership, leader election |
| LiteFS | Linux-only (FUSE) | Primary/replica with fly.io integration | Lease system | High — requires FUSE, no Windows |
| Turso | Edge SQLite fork | Cloud-first, embedded replicas | libSQL protocol | Medium — cloud dep |

**Decision: Litestream.** Native Windows binary, single-config, battle-tested for SQLite. Topology matches our needs exactly: venue is the writer, cloud is the read replica.

### Recommended topology

```
┌──────────────────────────┐         ┌──────────────────────────┐
│  VENUE (.23 Windows)     │         │  BONO VPS (Linux)        │
│  racecontrol.db (WRITER) │         │  racecontrol.db (REPLICA)│
│        │                 │         │        ▲                 │
│        ▼                 │         │        │                 │
│  litestream replicate ───┼────►────┼── litestream restore     │
│        │                 │         │    (continuous)          │
│        ▼                 │         │        │                 │
│   Backblaze B2 bucket    │◄────────┤ Pull WAL segments        │
│   (cheap, $0.005/GB)     │         │                          │
└──────────────────────────┘         └──────────────────────────┘
```

### Storage choice

- **Backblaze B2** ($0.005/GB-month + egress-free via Cloudflare Bandwidth Alliance) — our racecontrol.db is ~120MB, cost = $0.0006/month. Effectively free.
- Alternative: SFTP to Bono VPS directly (skip B2). Simpler but no DR off-site backup. Option A wants venue as source of truth, so SFTP-direct is defensible.
- **Recommendation:** B2 for DR + Bono as restore target. Two-bird-one-stone: replication + off-site backup.

### Config snippet (venue side)

```yaml
# C:\RacingPoint\litestream.yml
dbs:
  - path: C:\RacingPoint\data\racecontrol.db
    replicas:
      - type: s3
        bucket: racingpoint-replica
        path: racecontrol
        endpoint: s3.us-west-002.backblazeb2.com
        region: us-west-002
        access-key-id: ${B2_KEY_ID}
        secret-access-key: ${B2_APP_KEY}
        retention: 720h    # 30 days WAL history
        sync-interval: 1s
```

### Config snippet (Bono VPS side)

```bash
# /etc/systemd/system/litestream.service
# Continuous restore daemon
ExecStart=/usr/bin/litestream restore \
  -o /root/racecontrol/data/racecontrol.db \
  -if-db-not-exists \
  s3://racingpoint-replica/racecontrol
```

### Lag detection

Litestream has `litestream replicas -o json` for age of last sync. Add a per-subsystem probe in `/api/health`:

```ts
// admin-next: GET /api/health
litestream_lag_seconds: await checkReplicaLag()  // shell out once per probe
```

Threshold: `>300s` = WARN, `>900s` = CRITICAL, triggers WhatsApp alert.

### Restore drill

```bash
# Monthly drill on a scratch path
litestream restore -o /tmp/rc-restore.db s3://racingpoint-replica/racecontrol
sqlite3 /tmp/rc-restore.db "SELECT COUNT(*) FROM drivers;"
# Compare to live: should be within 1s of venue count
```

### Schema migrations with Litestream

Litestream is schema-agnostic (ships WAL segments, not logical rows). Schema migrations on venue propagate automatically. **Gotcha:** if cloud racecontrol runs a migration independently, it will diverge. **Policy:** on Litestream-mode, cloud is READ-ONLY for replicated tables. Cloud racecontrol must not run migrations on these tables. Document in ARCHITECTURE.md.

## Next.js 15 standalone canonical deploy

The exact bug we hit on cloud: `.next/standalone/server.js` does NOT auto-load `.env.local`. Env vars must come from process environment.

### Canonical deploy steps (admin-deploy.sh)

```bash
#!/usr/bin/env bash
set -euo pipefail

# 1. Clean + install (reproducible)
npm ci

# 2. Build
npx next build

# 3. Copy static assets into standalone tree (Next's known gap)
cp -r .next/static .next/standalone/.next/static
cp -r public .next/standalone/public

# 4. Rebuild native bindings against the runtime Node
cd .next/standalone && npm rebuild better-sqlite3 && cd -

# 5. Copy env file to standalone root (Next will NOT read .env.local automatically,
#    but exporting these vars in the runner will work)
cp .env.production.local .next/standalone/.env.production || true

# 6. Post-deploy verify gate — fails the deploy if any check fails
node scripts/verify-deploy.js
```

### Env loading at runtime (the fix)

Two options, pick ONE:

**Option A — Export in the runner (preferred):**
- Venue: `start-admin.bat` explicitly `set RC_URL=...` before `start node .next/standalone/server.js`
- Cloud: PM2 ecosystem.config.js with `env: { RC_URL: "...", RC_JWT_SECRET: "...", ... }` and `pm2 restart 22 --update-env`

**Option B — dotenv bootstrap:**
Create a 5-line `server-bootstrap.js` that `require('dotenv').config({path:'.env.production'})` then `require('./server.js')`. Launch that instead. Survives both platforms identically.

**Recommendation: Option B.** Shared bootstrap means ONE source of truth for env loading, works identically on Windows bat + Linux pm2, no platform divergence.

### `next.config.ts` requirements

```ts
import path from "path";

export default {
  output: "standalone",
  outputFileTracingRoot: path.join(__dirname),  // critical — prevents baked-in build-machine paths
  experimental: {
    outputFileTracingExcludes: {
      "*": ["**/*.md", "**/*.map", "**/test/**"]
    }
  }
}
```

**Known trap:** without `outputFileTracingRoot`, `required-server-files.json` bakes in `appDir: "C:\\Users\\bono\\racingpoint\\racingpoint-admin"` from the build machine. On deploy to `C:\RacingPoint\admin\`, Next can't find tracked files → 500s. Already documented in CLAUDE.md from 2026-03-25 kiosk incident.

## better-sqlite3 ABI stability

### Root cause of the v47 audit finding

Venue runs Node 24 (NODE_MODULE_VERSION 137). `better-sqlite3` was built on Node 22 (127). Native binding refuses to load → every admin.db API throws at first `getDb()` call.

### Fix strategy (layered)

1. **Pin Node version** in both environments. Add to `package.json`:
   ```json
   "engines": { "node": "22.x" }
   ```
   Add `.nvmrc` with `22.11.0`. Venue `start-admin.bat` explicitly uses `C:\Program Files\nodejs22\node.exe`.

2. **`npm rebuild better-sqlite3` in deploy script** — runs against the runtime Node, rebuilds the native binding fresh. Already in the admin-deploy.sh above (step 4).

3. **Startup self-check** — add a boot probe: `require('better-sqlite3')(':memory:').pragma('journal_mode=WAL')`. If it throws, log fatal + exit(1). This surfaces the bug at deploy time, not at first-user-click time.

4. **CI guard** — add a GitHub Actions job that builds on both Node 22 and Node 24 matrices. Catches ABI drift before it ships.

**Do not** switch to `node-sqlite3` (slower + more deps). better-sqlite3 is the right choice; we just need the rebuild hook.

## Error tracking choice

### Comparison

| Option | Setup cost | Ongoing cost | Ops burden | Fits Racing Point? |
|---|---|---|---|---|
| **Structured JSON logs to file + rsync to Bono** | 0 (pino or native `console.log(JSON.stringify(...))`) | 0 | Low — grep on Bono VPS | ✅ |
| Sentry SaaS | 0 | $26/mo entry | Low | Over-engineered for 1-venue |
| GlitchTip self-hosted | 2h (Docker on Bono) | Maintenance | Medium | Adds moving part |
| Loki + Promtail | 4h (Bono VPS) | Maintenance | Medium-high | Overkill |

**Recommendation: structured JSON logs to rotating file, synced daily to Bono via the existing backup rsync.** Zero new services. Grep-able. Bono can already read `/root/backups/venue-logs/` with zero auth changes.

Log format:
```json
{"ts":"2026-04-09T18:00:00Z","level":"error","route":"/api/cafe/menu","status":500,"user_id":"admin","err":"ABI_MISMATCH","corr_id":"..."}
```

## Lockout counter storage

Options for per-staff-id + per-IP lockout with ~50 staff scale:

| Store | Writes/sec | Survives restart | New dep |
|---|---|---|---|
| **admin.db SQLite table** | 100+ (WAL mode) | ✅ | ❌ (better-sqlite3 already) |
| In-memory LazyLock (current) | 1M+ | ❌ | ❌ |
| Redis | 50K+ | ✅ (RDB/AOF) | ✅ new service |

**Recommendation: new `lockout_counters` table in admin.db (venue) + `racecontrol.db` (cloud admin routes).** Survives restart, zero new services, trivial schema:

```sql
CREATE TABLE lockout_counters (
  key_type TEXT NOT NULL,        -- 'staff_id' or 'ip'
  key_value TEXT NOT NULL,
  failure_count INTEGER NOT NULL DEFAULT 0,
  first_failure_at INTEGER NOT NULL,   -- unix seconds
  last_failure_at INTEGER NOT NULL,
  locked_until INTEGER,                -- NULL if not locked
  PRIMARY KEY (key_type, key_value)
);
CREATE INDEX idx_lockout_locked_until ON lockout_counters(locked_until) WHERE locked_until IS NOT NULL;
```

Backend module: extend existing `crates/racecontrol/src/auth/admin.rs` (Phase 343 finding C7 already called this out — has a `persist_lockout_to_db()` stub).

## Playwright contract test pattern

### Template: admin→POS cafe menu propagation

```ts
// racingpoint-admin/tests/contracts/cafe-menu-propagation.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Cafe menu admin→POS contract', () => {
  test.slow(); // 70s budget

  test('admin menu edit reflects on POS billing within 10s', async ({ browser }) => {
    // 1. Open admin + login
    const adminCtx = await browser.newContext();
    const admin = await adminCtx.newPage();
    await admin.goto(process.env.ADMIN_URL!);
    await admin.fill('[name="pin"]', process.env.ADMIN_PIN!);
    await admin.click('button[type="submit"]');

    // 2. Add a test menu item (UUID name to avoid collisions)
    const testItem = `TEST_${Date.now()}_ESPRESSO`;
    await admin.goto(`${process.env.ADMIN_URL}/cafe`);
    await admin.click('text=Add Item');
    await admin.fill('[name="name"]', testItem);
    await admin.fill('[name="price"]', '75');
    await admin.click('text=Save');
    await expect(admin.locator(`text=${testItem}`)).toBeVisible();

    // 3. Open POS billing in a separate context
    const posCtx = await browser.newContext();
    const pos = await posCtx.newPage();
    await pos.goto(`${process.env.POS_URL}/billing`);

    // 4. Wait for item to appear (with retry — within 10s SLA)
    await expect(pos.locator(`text=${testItem}`)).toBeVisible({ timeout: 10_000 });

    // 5. Cleanup — delete test item
    await admin.goto(`${process.env.ADMIN_URL}/cafe`);
    await admin.click(`[data-item="${testItem}"] >> button:text("Delete")`);
  });
});
```

### Phase 343-04 sync-wait pattern

```ts
// staff-pin-lifecycle uses this pattern — reuse for any cloud-authority test
test.slow();  // 3x default timeout
await page.waitForTimeout(70_000);  // 2x sync_interval + 10s margin
```

**Key insight from Phase 343:** any test crossing venue↔cloud sync boundary MUST wait longer than `sync_interval_secs`. Unit tests miss silent-revert bugs because they don't wait for the second sync tick.

## Daily SQLite backup pattern

### Venue (Windows) — daily Scheduled Task

```powershell
# C:\RacingPoint\scripts\backup-admin-db.ps1
$date = Get-Date -Format "yyyy-MM-dd"
$backupDir = "C:\RacingPoint\backups"
New-Item -ItemType Directory -Force $backupDir | Out-Null

# Use sqlite3 .backup (handles WAL correctly, unlike file copy)
& "C:\RacingPoint\tools\sqlite3.exe" "C:\RacingPoint\admin\data\admin.db" ".backup '$backupDir\admin-$date.db'"
& "C:\RacingPoint\tools\sqlite3.exe" "C:\RacingPoint\data\racecontrol.db" ".backup '$backupDir\racecontrol-$date.db'"

# Compress
Compress-Archive "$backupDir\admin-$date.db","$backupDir\racecontrol-$date.db" -DestinationPath "$backupDir\backup-$date.zip" -Force
Remove-Item "$backupDir\admin-$date.db","$backupDir\racecontrol-$date.db"

# Retention: keep 30 daily + 12 monthly
Get-ChildItem $backupDir -Filter "backup-*.zip" | Where-Object {$_.LastWriteTime -lt (Get-Date).AddDays(-30)} | Remove-Item

# Rsync to Bono VPS (via scp since we're on Windows)
scp "$backupDir\backup-$date.zip" bono-vps:/root/backups/venue/
```

Scheduled Task: daily at 03:00 IST (post-close, pre-open).

### Cloud — daily cron

```bash
# /etc/cron.d/backup-admin-db
0 3 * * * root sqlite3 /root/racingpoint/racingpoint-admin/data/admin.db ".backup /root/backups/cloud/admin-$(date +%F).db" && gzip /root/backups/cloud/admin-$(date +%F).db
```

Note: racecontrol.db on cloud is the Litestream replica — no separate backup needed (Litestream history IS the backup with 30d retention).

### Restore drill (quarterly)

```bash
# On a scratch machine
unzip backup-2026-04-08.zip
sqlite3 admin-2026-04-08.db "SELECT COUNT(*) FROM menu_items;"
# Must match production count ± recent writes
```

## Integration points with existing stack

| New component | Connects to | How |
|---|---|---|
| Litestream replicate | venue racecontrol.db | reads WAL file; no code change in racecontrol |
| Litestream restore | cloud racecontrol.db | writes SQLite file before cloud racecontrol starts |
| `admin-deploy.sh` | existing `start-admin.bat` / pm2 | called manually by James/Bono at deploy time |
| `verify-deploy.js` | existing `/api/health` | curls 5 subsystem probes after deploy |
| lockout_counters table | existing `auth/admin.rs` | replaces in-memory LazyLock |
| cafe proxy rewrite | existing `crates/racecontrol/src/cafe.rs` | admin rcFetch → rc cafe/items CRUD |
| WhatsApp alerter | existing comms-link relay | admin `/api/health` POST to James :8766 `/relay/alert` |
| backup scripts | existing Scheduled Tasks + cron | new scripts, existing execution infrastructure |

## What NOT to add (explicit exclusions)

- ❌ **Sentry / error tracking SaaS** — see error tracking section
- ❌ **Redis** — lockout counter fits in SQLite
- ❌ **Docker / containers** — bare metal works, adds nothing
- ❌ **Kubernetes / Nomad / orchestrators** — single-venue
- ❌ **React Query / SWR** — existing admin uses plain fetch + useEffect, no need to introduce a fetching library for this milestone
- ❌ **tRPC / GraphQL** — rcFetch proxy is fine
- ❌ **Passkeys / WebAuthn / SSO** — future milestone
- ❌ **Mobile app wrapper** — responsive web is enough for Uday
- ❌ **Multi-tenancy** — one venue
- ❌ **Sentry replay / session recording** — privacy + cost
- ❌ **Prometheus / Grafana metrics UI** — v34 already shipped /metrics page; don't duplicate
- ❌ **Keycloak / auth0 / external auth** — existing JWT+PIN is fine for <50 staff
- ❌ **Swagger UI / API explorer** — existing OpenAPI spec is enough

## Open questions (escalate to plan-phase)

1. **Litestream on Windows** — does the `v0.3.x` Windows binary work with our SQLite version? **Test in Phase 344 pre-flight.**
2. **Cloud racecontrol schema writes** — when cloud becomes a read replica, can cloud racecontrol still accept writes to non-replicated tables (wallets/billing)? Need to identify which tables are replicated vs cloud-authoritative. See Phase 343 D-01 for the `authoritative_tables` list.
3. **Bootstrap.js env loader** — do we introduce it as a shared pattern, or platform-specific env config (bat file + pm2 ecosystem)? Plan-phase decision.
4. **Lockout table location** — venue admin.db (admin-scoped) or racecontrol.db (backend-scoped)? Backend makes more sense because `crates/racecontrol/src/auth/admin.rs` owns the lockout logic.

---

*Written directly due to agent API overload. Quality is lower than a full 4-agent research pass — plan-phase should re-run key questions if needed.*
