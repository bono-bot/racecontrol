# ARCHITECTURE TRUTH — RacingPoint V2 Admin & Sync Topology

> **Status:** canonical runtime-truth reference. **Read this first** in any session that touches the cloud
> admin layer, the venue heart, or the sync boundary.
>
> **Why this doc exists.** The repo's machine-readable maps (`DEPENDENCIES.json`, `ECOSYSTEM-MANIFEST.json`,
> the pm2 ecosystem configs, `cloud/compose.yml`, nginx vhosts) drifted out of sync with the running system
> after the V2 cloud rollout. This doc records the **territory** (what is actually running) next to the
> **map** (what the configs claim), so a later **sync contract** and **V1 unwiring** are mechanical, not
> dangerous. It is descriptive only — it changes no behavior and edits no runtime-consumed config.
>
> **Provenance.** Runtime facts are from read-only probes on **2026-06-07 ~12:50–12:55Z**:
> cloud = Bono VPS `72.60.101.58` via localhost `ss -ltnp` / `pm2 jlist` / `/proc/<pid>/{cmdline,cwd}` /
> `curl` / `docker ps` / `nginx -T`; venue `.23` (`192.168.31.23`) via Tailscale `100.125.108.37`.
> Re-probe before relying on any single row — runtime is a moving target.
>
> **2026-06-09 expansion.** Added **§9** (component-identity registry), **§10** (inter-app connection
> topology, incl. a **MISROUTED** cloud→V1-admin money-edge finding), **§11** (residuals), **§12**
> (control/config plane, incl. a **DISJOINT flag plane** + a **split, unbridged config hub** finding);
> and corrected **§1 / §2.2 / §7** from read-only probes on **2026-06-08→09** (cloud localhost + venue `.23` read-shell +
> Tailscale + on-`.23` PostgreSQL `SELECT count(*)`). Naming for the kiosk / launch-portal / launch-core
> collision (§9.1) is **OPEN — Captain-owned** (a parallel session owns that decision); rows are labelled by
> code-identity, not by any invented canonical name.
>
> **2026-06-23 expansion (connection-topology delta).** Re-probed and updated **§10.1** (the MISROUTED cloud
> `:3211` admin edge — re-probed read-only 2026-06-23, **STILL TRUE**); upgraded **§10.2** venue-surface edges
> from UNCONFIRMED → **VERIFIED** (launcher/pos/pos130/captain-admin reach james via their own BFF over
> **loopback `127.0.0.1:3211`**); and added **§10.7** — the **edge-tier topology** (common-BFF-vs-patchwork
> frame), the **PWA public→venue bridge** (live reverse-proxy over a wstunnel-wrapped WireGuard reverse tunnel;
> the per-venue multi-tenant gap), and **what a common BFF would require** (consolidation vs net-new) with the
> live-proxy-vs-sync decision (recorded **✅ DECIDED 2026-06-23** in §10.7). Source: bono memory
> `reference_backend_connection_topology_common_bff_grounding_20260623` (7-agent read-only grounding 2026-06-23)
> + the newer per-surface deployed-truth docs
> `comms-link/surfaces/{launch-portal,pos,pos130,captain-admin,customer-pwa,admin-proxy-james,_MULTI-TENANT}.md`
> (2026-06-22→23). Descriptive only — changes no runtime config (STOP-list untouched).

---

## 1. The Direction (North Star)

A venue is a sim-racing center, and **latency is a first-class design constraint**. Admin is not one speed —
it is tiered, and each tier has a different home dictated by how much latency and how much offline-survival it
can tolerate.

### Tier 0 — real-time, hardware/money, must survive an internet outage
Launch/kill a session, pod control & restart, wallet debit at the point of play, the meter. These **never**
take a cloud round-trip. **Home: the venue heart (`:8080`) + kiosk + launch-portal + POS.** Already local;
stays local.

> **⚠️ §S-469 AMENDMENT (Captain-ratified 2026-06-08) — the V2 money ENGINE is `admin-proxy-james`, not the
> Rust heart.** Runtime-confirmed (probe 2026-06-08): the live V2 billing path is the TS **`admin-proxy-james`
> BillingEngine running AT the venue** (`.23:3211`) — it ticks, debits the **credits** wallet, fires the 402
> gate, and writes the F5 audit. The Rust heart (`:8080`) serves **pod-state only** for V2 (`heart_v2` creates
> no billing row and debits no wallet). So Tier-0 "money at the venue heart" is read precisely as: **the venue
> *runs* the money engine, and that engine is the TS proxy** — co-located at the venue, so the latency /
> offline-survival rationale still holds at the machine level. The Rust §S-466 meter is **RETIRED from the V2
> path** (it retains V1 billing + pod-state); its logic is being **PORTED onto the TS meter**
> ([`docs/S466-TS-PORT-PLAN.md`](./S466-TS-PORT-PLAN.md)) with C1–C5 as the verified spec/test-oracle.
> *Residue:* probed with pods off (no in-flight session). **Durability — CORRECTED 2026-06-09:** the earlier
> "no venue Postgres / in-memory wallet" reading is **FALSE**. PostgreSQL is **live on `.23:5432`**
> (`racingpoint_v2`, listener confirmed) and the deployed james `.env` sets **`WALLET_STORE=pg`** (+
> `DATABASE_URL=postgresql://rp:***@127.0.0.1:5432/racingpoint_v2`); the durable companion stores
> (audit / idempotency / session-pricing / flag) are pg-backed too. Evidence: a read-only `SELECT count(*)`
> on `wallet_ledger` this session returned **0** — table present, schema live, no writes yet (pre-first-rupee).
> The boot path is fail-LOUD on a missing pg module with **no silent memory fallback**
> (`wallet-durable-boot.ts`) → the configured `WALLET_STORE=pg` is operative unless the process loudly
> crashed (it is up, serving 200). *Not separately probed (no endpoint exposed):* the running process's live
> store-kind via a debug route — but in-memory fallback is structurally impossible here. The standing
> "in-memory wallet" caveat is **retired**. Canonical: §S-469 ·
> [`docs/FINDINGS-C6-C9-DISCOVERY-20260607.md`](./FINDINGS-C6-C9-DISCOVERY-20260607.md).

### Tier 1 — fast floor money decisions (especially on a pod fault)
Bill-from-start, write-off, apology credit. Time-critical and must stay reachable on the venue LAN even when
cloud is down. **DECIDED (owner, 2026-06-07): the Tier-1 surface is a POS function wired direct to the local
venue authority** — *not* a captain-console offline mode (rejected: puts the riskiest money writes on the
least-tested path), and *not* a separate heart panel. The POS is already local, already on the venue LAN, and
already a money surface, so it is **always local** — there is no "fallback mode" and no offline-detection logic
to get wrong.

The three Tier-1 actions, each with its risk-class control:

| Action | Who may authorize | Control rationale |
|---|---|---|
| **bill-from-start** | floor staff | recovers revenue; abuse self-surfaces as an over-charge / complaint |
| **apology credit** | floor staff up to **one booked session** (rate-card-derived: ₹700/30min · ₹900/60min, §S-456); above → manager PIN | a *gift*, not a reconciliation — capped at the lost session + audited. **Two instruments** (track credit vs café money-back refund) — see SYNC-CONTRACT §5B / §S-457 |
| **write-off** | **manager PIN required every time**, even offline | erases revenue silently; highest abuse risk; gate hardest |

**Invariant (non-negotiable):** the POS fast-path **talks to** the local venue authority; it never self-approves
on a cached token and never fakes the authority. The venue authority validates, commits to the venue DB, and
writes the **local F5 audit row**. On reconnect, that row syncs up and the **venue wins** on venue-state.
(Worked through end-to-end in [`docs/SYNC-CONTRACT.md`](./SYNC-CONTRACT.md) §6.)

### Tier 2 — deliberate management, latency-tolerant
Rate card / pricing, staff & PINs, feature flags, reports, daily close, class promotion, cafe menu.
**Home: the CLOUD admin panel = `captain-console`**, multi-tenant and per-venue-scoped (a venue sees only its
own data; isolation is by JWT/tenant scope, served by ONE cloud app — deploy-once, no per-venue frontends).

### Invariants (never violated)

1. **The venue is the sole authority** over its own money / sessions / pods, and the **sole writer of the F5
   audit**. The cloud admin **issues** requests; the venue **authorizes and audits** them (the existing
   bono → james mutation-authority gate). Cloud never holds canonical venue money-state. *(§S-469: "the venue"
   = the venue **machine** `.23`; the V2 money **engine** that authorizes + audits is the TS `admin-proxy-james`
   proxy co-located there, **not** the Rust heart — see the Tier-0 §S-469 amendment above.)*
2. **On reconnect after an outage, the venue wins** on venue-state. The cloud's stale view must never
   overwrite venue transactions that accrued offline.
3. **Three sync flows, each with exactly one direction of truth** — never conflate them (see §5).

Operational and customer-money surfaces stay LOCAL by latency (they drive hardware, take money, and must
survive outages). Management and customer surfaces (`captain-console`, the customer PWA) are CLOUD
(deploy-once across venues; per-venue-frontend drift is the V1 disease being eliminated).

---

## 2. Runtime Truth (what is actually serving)

### 2.1 Cloud — Bono VPS `72.60.101.58` (probed localhost 2026-06-07 ~12:50Z)

| Port | Process | cwd / image | Health (local) | nginx public route | Status |
|---|---|---|---|---|---|
| 8080 | `racecontrol` (pm2, `RC_IS_CLOUD=1`) | `/root/racecontrol` | heart | `rc.racingpoint.cloud /`, `api.racingpoint.cloud /` | LIVE — cloud heart |
| 3211 | admin-proxy-bono (pm2 `rp2-admin-proxy-bono`, node TS) | **`/srv/rp-v2-apps`**`/apps/admin-proxy-bono` | `/api/v2/healthz` → 200 | `admin.racingpoint.cloud ^~ /api/v2/` | LIVE — **V2 admin API edge** |
| 3213 | admin-proxy-bono (docker `rp2-proxy`, **host-net**) | `/app/apps/admin-proxy-bono` (← `rp-v2-apps/apps/admin-proxy-bono`) | `/healthz` → 200 (`/api/v2/healthz` → 400) | `app.racingpoint.cloud /api/`, `racingpoint.cloud /api/` | LIVE — **PWA API edge** |
| 3302 | `rp2-pwa` (docker `node:22`, bridge) | container `/app/apps/pwa` | `/` → 200, **`/api/v2/healthz` → 500** | `app.racingpoint.cloud /`, apex `/` | LIVE — V2 customer PWA (healthz anomaly, see §7) |
| 3221 | `captain-console` (pm2, next) | **`/root/rp-v2-apps-wt-v31gapfill`**`/apps/captain-console` | `/api/health` → 200 (`ui:v3`) | `admin.racingpoint.cloud /` | LIVE — **V2 admin UI root** (runs from a **git worktree**, see §7) |
| 3220 | `racecontrol-console` (pm2, next) | `/root/rp-v2-apps/apps/racecontrol-console` | `/` → 307 | `console.racecontrol.in /` | LIVE — HQ console |
| 3500 | `racingpoint-web-v2` (pm2, next) | `/root/racecontrol/web-v2` | `/` → 200 (basePath `/v2`) | `admin.racingpoint.cloud /admin` + `/admin/*` | LIVE — older admin fragment, **demoted to `/admin`** |
| 3501 | `racecontrol-pwa` (pm2, next) | `/root/racecontrol/pwa` | — | **none** | LIVE but **UNROUTED** |
| 3201 | `racingpoint-admin` (pm2, next standalone) | `/root/racingpoint-admin/.next/standalone` | `/api/health` → 200 (git `4dccbd5`, 57 pages) | **none** | LIVE but **UNROUTED** — V1 admin (see §3) |
| 3100 | `cloud-pwa-1` (docker image `cloud-pwa`) | container | internal only | **none** | **ZOMBIE** — orphan from the dead `cloud/compose.yml`, unrouted |
| 53622 | `evolution-api` (docker) | container | — | `admin.racingpoint.cloud:8443 /` | LIVE — WhatsApp Evolution (unrelated to admin) |

Stopped pm2 apps present but not running: `racingpoint-kiosk`, `racingpoint-dashboard` (`:3400`),
`cloud-dashboard` (`:3700`).

External liveness confirmed (HTTPS GET, read-only): `console.racecontrol.in/` → 307 ·
`admin.racingpoint.cloud/` → 200 · `admin.racingpoint.cloud/api/v2/healthz` → 200 ·
`admin.racingpoint.cloud/admin` → 200 · `app.racingpoint.cloud/` → 200.

**`admin.racingpoint.cloud` is a three-upstream split** (exact nginx location order):

| Location | Upstream | Serves |
|---|---|---|
| `= /.well-known/jwks.json` | alias `/etc/racingpoint/f6-public/jwks-bono.json` | F6 bono JWKS |
| `^~ /api/v2/` | `:3211` (admin-proxy-bono, host) | V2 admin API |
| `= /admin` and `~ ^/admin/(.+)$` | `:3500` (web-v2) | older admin UI fragment |
| `/` | `:3221` (captain-console) | V2 admin UI root |

### 2.2 Venue — `.23` (`192.168.31.23`, Tailscale `100.125.108.37`)

| Port | Service | build | Status |
|---|---|---|---|
| 8080 | racecontrol heart | `213712a9` (5 ahead of `origin/main` — PR #130 unmerged; 3 behind, none billing-relevant) | **degraded, fleet 2/8 pods** |
| 8090 | racecontrol server-ops | — | 200 |
| **3211** | **`admin-proxy-james`** (TS, `node --env-file=.env --import tsx src/server.ts`) | tarball `28ef314a` base **+ in-place edits, no `.git`** → not commit-pinnable | **LIVE — the V2 money ENGINE** (§S-469): BillingEngine + credits `wallet_ledger` + 402 gate + F5 audit; healthz `pc:"james"`, `heart:8080`. *Was OMITTED from this table before 2026-06-09 — the single most important V2 money surface; now added.* |
| **5432** | **PostgreSQL** (`racingpoint_v2`) | — | **LISTENING** — durable wallet / audit / idempotency / session-pricing / flag stores; james `.env` `WALLET_STORE=pg`; `wallet_ledger`=0 (pre-first-rupee). **Corrects §1's prior "no venue Postgres".** |
| 3200 | `web-dashboard` (`racecontrol/web`) | git `7d638430`, 48 pages | **LIVE** — venue staff/ops dashboard |
| 3300 | kiosk *(code-identity OPEN — see §9.1)* | — | LIVE |
| 3360 | launch-portal *(see §9.1)* | — | LIVE |
| 3201 | (`racingpoint-admin`) | — | **DOWN** (000) |

---

## 3. Name disambiguation — "racingpoint-admin" is three distinct things

The single name `racingpoint-admin` is overloaded across three filesystem locations with different runtime
roles. Treating them as one is the root of the confusion this doc resolves. Use the proposed canonical names.

| Canonical name | Path | Host / runtime | Role | Referenced by |
|---|---|---|---|---|
| **`rp-admin-live`** (V1 admin app) | `/root/racingpoint-admin` (canonical) | cloud, pm2 `racingpoint-admin`, `:3201`, Next standalone, git `4dccbd5` | The **actual running V1 admin**; owns `data/admin.db` | `DEPENDENCIES.json` (admin.db owner + racecontrol.db reader, `repo_path /root/racingpoint-admin` ✓); `ECOSYSTEM-MANIFEST.json` `systems.racingpoint-admin` (`path ../racingpoint-admin` ✓); heart `crates/racecontrol/src/subsystem_health_probes.rs:492` probes `../racingpoint-admin/admin.db` (= this sibling ✓). **Unrouted** by cloud nginx. |
| **`rp-admin-symlink`** (alias) | `/root/racingpoint/racingpoint-admin` → symlink to `rp-admin-live` | cloud (pm2 `cwd`) | Pure alias — the pm2 working-dir path. Not a separate thing. | pm2 `racingpoint-admin.cwd` |
| **`rp-admin-scaffold`** (in-repo orphan) | `/root/racecontrol/racingpoint-admin` | runs **nowhere** | In-repo, `src/` only (4 `.tsx`), **no `package.json` / no build** | `cloud/compose.yml:55` build context `./racingpoint-admin` (resolves to `/root/racecontrol/cloud/racingpoint-admin`, which is **MISSING** — broken regardless); `crates/racecontrol/src/app_health_restart.rs:26` name-map `"cloud-admin" → "racingpoint-admin"` |

**Resolution.** `DEPENDENCIES.json` and the heart's `admin.db` probe **correctly target the sibling**
(`rp-admin-live`). The in-repo scaffold (`rp-admin-scaffold`) is the orphan, and the only reference to it (the
compose build context) is itself broken. **No live consumer points at the in-repo scaffold.** This naming must
be settled before any deletion so the unrouted-but-live `:3201` app is never confused with the dead scaffold.

---

## 4. Config reconciliation — map vs territory

These corrections are **proposed, not applied.** Each target is a runtime-consumed file, so it requires owner
review before edit (see §6 STOP list). "Consumed by" = what reads the file.

### 4.1 `DEPENDENCIES.json` *(consumed by: CGP/MMA/ad-hoc audits that iterate it; possibly restart tooling)*

| Entry | Claims | Runtime shows | Proposed |
|---|---|---|---|
| `pm2_services.racingpoint-admin.port` | 3200 | **3201** | 3201 |
| `pm2_services.racingpoint-admin.health` | `null` | `/api/health` returns ok | `"/api/health"` |
| `pm2_services.racecontrol-pwa.port` | 3500 | **3501** (`:3500` = web-v2) | 3501; add `racingpoint-web-v2: 3500` |
| V2 layer | absent | admin-proxy-bono `:3211`+`:3213`, captain-console `:3221`, racecontrol-console `:3220`, web-v2 `:3500`, rp2-pwa `:3302` | add these services |
| `last_updated` | 2026-03-12 | — | bump |

### 4.2 `ECOSYSTEM-MANIFEST.json` *(consumed by: "ANY audit MUST iterate this list")*

| Entry | Claims | Runtime shows | Proposed |
|---|---|---|---|
| `web-dashboard.runs_on` | `[server_23, bono_vps]` | cloud `:3200` not running; venue `:3200` LIVE | `[server_23]` |
| `racingpoint-admin.runs_on` | `[server_23, bono_vps]` | cloud `:3201` LIVE; venue `:3201` DOWN | note cloud-only currently |
| `pwa.port` | 3500 | `:3500` = web-v2, pwa = `:3501`, rp2-pwa = `:3302` | split: pwa → 3501; add web-v2 → 3500; add rp2-pwa → 3302 |
| `kiosk.runs_on` | `[server_23, bono_vps]` | cloud stopped; venue LIVE | `[server_23]` |
| V2 layer (rp-v2-apps) | absent | LIVE (see 4.1) | add admin-proxy-bono / captain-console / racecontrol-console / web-v2 / rp2-pwa |
| `_updated` | 2026-04-04 | — | bump |

### 4.3 pm2 ecosystem configs

| File | Finding | Proposed |
|---|---|---|
| `ecosystem.config.cjs` | defines **only** `racecontrol`; the cloud V1 web/admin/pwa + all V2 apps are started ad-hoc, in **no committed ecosystem file** → no single source of truth | (later) consolidate; flag now |
| `web-v2/ecosystem.config.cjs` | port 3500 ✓ matches runtime; **comment** references `v2.racingpoint.cloud → :3500`, but that vhost is **disabled** (route is now `admin.racingpoint.cloud/admin`) | stale comment only |
| `pwa/ecosystem.config.cjs` | `racecontrol-pwa` `:3501`, unrouted | flag |

### 4.4 `cloud/compose.yml` *(consumed by: `docker compose` — currently NOT running)*

- **Not running:** no `caddy` process; host nginx owns 80/443 (caddy maps 80/443 → conflict).
- **All three build contexts MISSING** relative to `cloud/`: `./racecontrol/pwa`, `./racingpoint-admin`,
  `./racecontrol/web` — every `build:` is dead, not just line 55.
- **Zombie remnant:** `cloud-pwa-1` (`:3100`, image `cloud-pwa`) is still up from this stack, unrouted.

### 4.5 nginx *(consumed by: nginx; live request routing)*

- `v2.racingpoint.cloud` vhost exists in `sites-available` but is **not symlinked** in `sites-enabled` → not served.
- `:3201` (`racingpoint-admin`) and `:3501` (`racecontrol-pwa`) are **LIVE but have no nginx route** (publicly
  unreachable on cloud) — prime unwiring candidates.
- 11 `.bak/.pre-*` `racingpoint.cloud*` variants clutter `sites-available` (none enabled).

---

## 5. The three sync flows (one direction of truth each)

| Flow | Direction of truth | Consistency | Conflict rule |
|---|---|---|---|
| **State / telemetry** | venue → cloud | eventual | read-only projection for the admin UI; staleness is **shown, not harmful** |
| **Commands / admin writes** | cloud → venue | strong (per request) | not "done" until the venue commits + audits; venue may reject |
| **Identity / config** (tiers, staff, rate card, flags) | cloud → venue | cloud authoritative, venue caches | venue keeps enforcing its **cached** copy when cloud is down |

Anchor points in code today: cloud→venue pull set = `crates/racecontrol/src/cloud_sync.rs` `SYNC_TABLES`
(`drivers`, `wallets`, `pricing_tiers`, `pricing_rules`, `kiosk_experiences`, `kiosk_settings`); venue→cloud
push set = `DEPENDENCIES.json.sync_boundary.venue_to_cloud` (`laps`, `track_records`, `personal_bests`,
`billing_sessions`, `pods`, `drivers`, `wallets`, `wallet_transactions`). **Invariant 2 (venue-wins-on-reconnect)
must hold for every venue→cloud table** — formalizing this is the next step (§8).

---

## 6. STOP list — runtime-consumed, needs owner approval before edit

1. `DEPENDENCIES.json` (§4.1 corrections) — present diff, approve before edit.
2. `ECOSYSTEM-MANIFEST.json` (§4.2 corrections) — present diff, approve before edit.
3. `cloud/compose.yml` — recommend **leave** (dead infra; handle during unwiring).
4. pm2 ecosystem consolidation (§4.3) — recommend **leave** for now.
5. nginx (§4.5) — recommend **leave**; unrouted `:3201` / `:3501` noted for the unwiring scope.

This doc (`docs/ARCHITECTURE-TRUTH.md`) is the only file written for this task; nothing in the STOP list was edited.

---

## 7. Flags (surfaced, not fixed)

- **`cloud/compose.yml` is fully dead:** all 3 build contexts missing + caddy/nginx 80/443 conflict +
  `cloud-pwa-1` zombie container. → **Mark for REMOVAL in the future unwiring pass** (not reconciliation; see §7.1 REMOVE-COMPOSE).
- **`app.racingpoint.cloud` `:3302` `/api/v2/healthz` → 500** (PWA healthz broken; `/` is 200).
- **`captain-console` (`:3221`) runs from a git worktree** `/root/rp-v2-apps-wt-v31gapfill`, not the main
  checkout — deployment-provenance drift risk. → backlog item **STABILIZE-CC** (§7.1).
- **Two admin-proxy-bono instances from *different trees* (sharpened 2026-06-09):** `:3211` host runs
  **`/srv/rp-v2-apps`** (non-git tarball) — this is the **MISROUTED admin edge** (§10.1, forwards to the cloud
  V1 admin, not the venue james); `:3213` docker `rp2-proxy` (host-net) bind-mounts **`/tmp/rp-v2-pwa-build`**
  (a git worktree, detached `3d0a4d9`), **not** `/srv` — this is the **correct customer/PWA edge**. They are
  **not the same checkout.** See §10.1 for the routing consequence + the latent `:3201`-vs-`:3211` port trap.
- **`/srv/msg91-test.html`** is live at `app.racingpoint.cloud/msg91-test.html` (200, nginx `= /msg91-test.html`)
  but is **in no git repo** — a standalone static page, **not wired into the registration/OTP flow** (the
  canonical OTP verify is server-side on admin-proxy-bono; see §11).
- **`:3201` (V1 admin) and `:3501` (racecontrol-pwa) are live but unrouted** — strong unwiring candidates.

### 7.1 Flagged backlog (do NOT act now)

- **STABILIZE-CC — stabilize `captain-console` deployment (worktree → clean build).** It serves the live V2
  admin UI from `/root/rp-v2-apps-wt-v31gapfill`; move it to a clean checkout + build before it drifts.
- **REMOVE-COMPOSE — remove the dead `cloud/compose.yml` stack** (+ the `cloud-pwa-1` zombie) during the
  unwiring pass; not part of reconciliation.

---

## 8. Next step — the sync contract

Formalize §5 into a written contract: per data-domain, the direction of truth, the transport, and the
conflict-resolution rule (venue-wins-on-reconnect for venue-state), mapped onto `cloud_sync.rs` `SYNC_TABLES`
and the `DEPENDENCIES.json.sync_boundary` push set. → **Authored (design only):
[`docs/SYNC-CONTRACT.md`](./SYNC-CONTRACT.md).**

---

## 9. Per-component identity registry (design-name / repo-code / deployed)

> §2.1/§2.2 are PORT/PROCESS tables. This section adds the **COMPONENT** layer: one canonical responsibility
> per app, derived from its routes/code (**not** its name). **Naming for the kiosk / launch-portal /
> launch-core collision (§9.1) is OPEN — the Captain assigns the canonical name-strings, and a parallel
> session owns that decision. Rows are labelled by code-identity, never by an invented canonical name.**
> Provenance: repo reads + cloud localhost + venue `.23` Tailscale (already-200) 2026-06-08→09.

### 9.1 The kiosk / launch-portal / launch-core collision — names OPEN (Captain)

| Code identity (label, not a canonical name) | Path | Top-line responsibility (from code) | Canonical name |
|---|---|---|---|
| `@rp/kiosk` | `rp-v2-apps/apps/kiosk` | **Gaming-hall wall display** — full-screen 8-pod grid + runout alarm + 2-min grace countdown + maintenance fallback (CR-4); SSE pod-state (`app/page.tsx:15` "single full-screen 8-pod grid") | ‹OPEN — Captain› |
| `racecontrol-kiosk` | `racecontrol/kiosk` | **V1-era on-pod kiosk** (Rust-repo Next app, `package.json name=racecontrol-kiosk`) | ‹OPEN — Captain› |
| `@rp/launch-portal` | `rp-v2-apps/apps/launch-portal` | **Staff pod-launch console** — `boot`/`chef`/`launch`/`lobbies`/`launch/[pod]` routes (staff picks pod → launches game); venue `:3360` | ‹OPEN — Captain› |
| Rust launch-core | `racecontrol/crates/racecontrol` (`ac_launcher.rs`, `heart_v2` launch) | **Backend acs.exe spawn / launch contract** — NOT a frontend | ‹OPEN — Captain› |

**Same name → different responsibility (surfaced, not resolved):**
- **"kiosk"** has THREE referents: `@rp/kiosk` (V2 hall-grid display) · `racecontrol-kiosk` (V1 on-pod) · the deployed venue `:3300` (codebase unconfirmed ungated — historically `racecontrol-kiosk` per CLAUDE.md Server Services).
- **"launch(er)"** has TWO: `@rp/launch-portal` (staff frontend, `:3360`) vs the Rust launch-core (acs.exe spawn). Design phase 196 `game-launcher-structural-rework` targets the **Rust core**, not the TS launch-portal.

→ **Captain assigns canonical names.** This registry only disambiguates by responsibility + code-identity.

### 9.2 Thin display components (absent from §2.1 / §2.2)

| Component | Path | Consumes (feed) | Canonical responsibility |
|---|---|---|---|
| `@rp/chef-display` | `apps/chef-display` | `GET /cafe/orders/kitchen` → `prep_state: CafePrepState` (cafe.yaml v0.1.3-draft; `lib/api.ts:52,69,81`) | **Kitchen prep-queue display** — café orders by prep-state. NOT a SessionState consumer. |
| `@rp/pod-display` | `apps/pod-display` | `GET /pods/{podId}/state` via SSE/EventSource (`lib/api.ts:107,190`), payload `state: SessionState` | **Per-pod customer-facing screen** — live session state + runout alarm. SessionState sibling. |

Both are ABSENT from §2.1/§2.2 (not deployed on cloud; venue deployment unconfirmed ungated).

### 9.3 SessionState producer / canonical / consumer reconciliation

- **PRODUCER (Rust heart):** `crates/racecontrol/src/api/heart_v2.rs:61` `pub enum SessionState` = {Preflight, Loading, Ready, Running, Paused, Ending, Ended, AutoBilled} (8, snake_case on wire).
- **CANONICAL (TS contracts):** `packages/contracts/src/session.ts:31` `z.enum([preflight, loading, ready, running, paused, ending, ended, auto_billed])` (8 — EXACT match to producer).
- **CONSUMERS — all re-declare a LOCAL union; none import the canonical enum:**

| Consumer | union site | variant set vs canonical | status |
|---|---|---|---|
| `@rp/kiosk` | `lib/api.ts:55` | **+2 phantom: `active`, `switching`** (10 vs 8) | **DRIFTED (value)** — fix = unmerged PR #45 `feat/kiosk-sessionstate-prune` |
| `@rp/pod-display` | `lib/api.ts:62` | exact 8 | values clean; structural duplicate (not imported) |
| `@rp/launch-portal` | `lib/api.ts:57` | exact 8 | values clean; structural duplicate |
| `@rp/staff-tablet` | `lib/api.ts:181` | exact 8 | values clean; structural duplicate |

The cast-and-local-union PATTERN is in all four; only kiosk has value drift (phantom `active`/`switching` the heart never emits). Structural fix (import the contracts enum) is NOT applied here.

### 9.4 Drift corrections — APPLIED to §1 / §2.2 / §7 in this revision (changelog)

1. **§2.2 now carries the `admin-proxy-james` `:3211` row** — previously OMITTED, yet §1's §S-469 amendment names it as the V2 money engine. *Applied above.*
2. **§7 sharpened: the two admin-proxy-bono instances run *different trees*** — `:3211`=`/srv/rp-v2-apps` (non-git), `:3213`=`/tmp/rp-v2-pwa-build` (worktree, detached `3d0a4d9`). *Applied in §7 + §10.1.*
3. **§2.2 venue heart `213712a9` noted 5-ahead of `origin/main`** (PR #130 unmerged; deployed ≠ main). *Applied above.* Cloud heart `:8080` is `build_id=21531f31-dirty` (built from a dirty worktree; not cleanly commit-pinnable).
4. **§1 durability claim corrected** — "no venue Postgres / in-memory wallet" is FALSE (pg live, `WALLET_STORE=pg`). *Applied in §1.*
5. **§7 adds `/srv/msg91-test.html`** — live at `app.racingpoint.cloud/msg91-test.html`, in no git repo, not wired into registration. *Applied in §7.*

### 9.5 Components missing from the runtime tables (registry gap)

Not in §2.1/§2.2 at all: `chef-display`, `pod-display`, `staff-tablet`, `pos`, `pos130`, **`admin-proxy-james` (venue — now added to §2.2)**, plus dev/infra apps `mock-heart`, `bono-internal-relay`, `deploy-agent-james`, `deploy-controller-bono`. The venue POS surfaces (`pos` login/walk-ins; `pos130` household-lookup/cafe-link/wallet/ps5-bill/close-of-shift) are named in §1 Tier-1 prose but have no runtime row (venue `.130`, DARK — see §10.4).

---

## 10. Inter-app connection topology (both ends verified per edge)

> Method: BOTH ends per edge — the caller's **actual** target (env/config value it dials) AND **what answers**
> there. Observations, not verdicts. Classification {VERIFIED | MISROUTED | UNCONFIRMED | DARK}. Probes:
> cloud = Bono VPS localhost (`ss`/`pm2`/`docker`/`/proc`/`curl`/`nginx -T`/`wg`) 2026-06-09; venue = `.23`
> read-shell + Tailscale; `.130` probed from `.23`.

### 10.1 ⚠️ Money-path routing — one MISROUTED admin edge; the customer path is CORRECT

**`resolveJamesUrl()`** (`admin-proxy-bono/src/james-url.ts`): explicit `JAMES_ADMIN_PROXY_URL` wins → else
`NODE_ENV=production` ⇒ `http://10.42.0.2:3201` → else dev default `http://127.0.0.1:3201`. **`10.42.0.2` = the
venue `.23`** over a James-initiated WireGuard reverse-tunnel (`wg0`: peer `10.42.0.2/32`, "Server (Hyderabad
.23) … R-204 §S-407"; handshake live; `ip route get 10.42.0.2 → wg0`).

| Cloud bono instance | JAMES target (deployed) | Answers at target | Verdict |
|---|---|---|---|
| **`:3213`** docker `rp2-proxy` (mount `/tmp/rp-v2-pwa-build`, `NODE_ENV=development`) — **PWA edge** | **explicit `JAMES_ADMIN_PROXY_URL=http://10.42.0.2:3211`** | `10.42.0.2:3211` healthz `pc:james, heart:8080` — byte-identical to direct `.23:3211` (Tailscale) | **VERIFIED → venue james** |
| **`:3211`** bare-metal pm2 (`/srv/rp-v2-apps`) — **admin-API edge** | dev-loopback default `http://127.0.0.1:3201` (no override; `NODE_ENV`≠production) | `127.0.0.1:3201` `/api/health` = **`racingpoint-admin 0.1.0 git 4dccbd5`** (cloud V1 admin) | **🔴 MISROUTED → cloud V1 admin** |

- **Customer first-rupee path is CORRECT:** PWA bakes `https://app.racingpoint.cloud` → nginx `location /api/ → :3213` → `JAMES_ADMIN_PROXY_URL=10.42.0.2:3211` → WG → venue james → heart.
- **Admin edge is MISROUTED:** captain-console bakes `https://admin.racingpoint.cloud/api/v2` → nginx `^~ /api/v2/ → :3211` → `127.0.0.1:3201` (cloud V1 admin, NOT venue james). captain-console wallet **topups** (`mutationAuthority:"james"`) therefore forward to the V1 admin, which has no `/api/v2/*` route (404 — fail-not-corrupt). **Do not conflate with the customer path.**
- **🪤 Latent trap:** the code's *production* default + the runbook both name `10.42.0.2:`**`3201`**, but venue james is `:`**`3211`** and `.23:3201` is DOWN. So flipping `:3211` to `NODE_ENV=production` would **still misroute** (to `.23:3201`). The **only** correct remediation is the explicit override `JAMES_ADMIN_PROXY_URL=http://10.42.0.2:3211` on the `:3211` instance. **Remediation = PENDING-CAPTAIN-GO (separate turn; no env change made here).**
- **✅ Re-probed 2026-06-23 (read-only, Bono VPS localhost) — STILL TRUE, unchanged since 2026-06-09.** `:3211` listener = `node /srv/rp-v2-apps/apps/admin-proxy-bono` (pid 3522709); its `/proc/<pid>/environ` carries **`BONO_ADMIN_PROXY_PORT=3211` only — `JAMES_ADMIN_PROXY_URL` UNSET and `NODE_ENV` UNSET** (≠production) ⇒ `resolveJamesUrl()` falls to the dev default `http://127.0.0.1:3201`, which still answers as the **cloud V1 admin** (`{service:"racingpoint-admin", git_commit:"4dccbd5", build_id:"KzAfwMAskQAMcu6DVhHLF"}`). nginx still routes `admin.racingpoint.cloud ^~ /api/v2/ → :3211`. The customer PWA edge `:3213` (pid 2990678) **does** carry `JAMES_ADMIN_PROXY_URL=http://10.42.0.2:3211` (+`NODE_ENV=development`) → correctly reaches venue james. **Split persists: PWA edge correct, admin edge misrouted.**

### 10.2 Request chains (caller API-base → proxy → james → heart → agent)

| Surface | API-base env (repo) | Deployed value | Hits | Onward |
|---|---|---|---|---|
| PWA | `NEXT_PUBLIC_BONO_ADMIN_BASE` | `https://app.racingpoint.cloud` | `:3213` (PWA edge) | → WG → **venue james** ✓ |
| captain-console | `NEXT_PUBLIC_BONO_ADMIN_BASE` | `https://admin.racingpoint.cloud/api/v2` | `:3211` (admin edge) | → **cloud V1 admin** ✗ (§10.1) |
| launch-portal `:3360` | own BFF `lib/james-upstream.ts`; client base unset → same-origin | **VERIFIED 06-23** (BFF in `main`/wt-formb; live two-hop probed) | own BFF → **loopback `127.0.0.1:3211`** (`james-upstream.ts:22`) | → james → heart launch (§10.7) |
| pos `:3120` (real till) | own BFF `apps/pos/lib/james-upstream.ts` (`feat/pos-bff-till-step1@6a94a7c`) | **VERIFIED 06-23** (BFF source + loopback target) | own BFF → **loopback `:3211`** | → james → heart |
| pos130 `:3130` | — (MOCK) | **VERIFIED 06-23 MOCK** | in-process `lib/mock-store.ts` — **reaches nothing** | (no james, no heart) |
| captain-admin `:3370` | own BFF `apps/captain-admin/lib/admin-upstream.ts` (`feat/captain-admin-slice@e1e6fd7`) | **VERIFIED 06-23** (BFF source + loopback target) | own BFF → **loopback `:3211`** (bypasses admin-proxy-bono) | → james |
| staff-tablet | `NEXT_PUBLIC_JAMES_ADMIN_BASE` | UNCONFIRMED (not re-probed 06-23) | venue james `:3211` | → heart |
| pod-display | `NEXT_PUBLIC_JAMES_ADMIN_BASE` | UNCONFIRMED (not re-probed 06-23; delivery-lane §S-543) | venue james `:3211` (SSE) | ← heart pod-state |

james → heart: `RACECONTROL_HEART_URL=http://127.0.0.1:8080` (deployed `.23` james `.env`) → venue heart. heart → rc-agent (pod `:8090`) via WS (known; not re-probed this turn → UNCONFIRMED-live).

### 10.3 SSE / data feeds (producer ↔ consumer, both ends)

- **pod-state:** PRODUCER heart `:8080` `/heart/pods/state/stream` → james `sse-bridge.ts:220` fetches it (`accept: text/event-stream`) → re-emits `/api/v2/pods/state/stream` (`server.ts:2065`, `routes/pod-state-sse.ts`). CONSUMERS pod-display/kiosk/launch-portal/staff-tablet `EventSource(JAMES_ADMIN_BASE + /pods/state/stream)`. (Live stream bytes not opened — would hang; code-verified both ends.)
- **kitchen:** chef-display `EventSource(JAMES_ADMIN_BASE + /cafe/orders/kitchen)` ← james cafe feed (snapshot `GET /cafe/orders/kitchen`, `prep_state`).
- **household lookup:** staff-tablet → james `/registration/households/{id}…` (UNCONFIRMED live).

### 10.4 The `.130` POS box

`.130` is **unreachable from `.23`** ("Destination host unreachable", `ping` from venue 2026-06-09) → **DARK**
(point-in-time; box off / off-LAN). By config pos/pos130 reach venue james via `NEXT_PUBLIC_JAMES_ADMIN_BASE`
(LAN), but the live edge cannot be verified while the box is down. **Not inferred.**

### 10.5 Store ownership (reader / writer, both ends)

| Store | Writer | Reader | Verified |
|---|---|---|---|
| **`wallet_ledger`** (pg `racingpoint_v2`) | **admin-proxy-james ONLY** (`durable-wallet-store.ts:375 INSERT INTO wallet_ledger` — the sole INSERT; only admin-proxy-james imports the store) | james (balance/history) | **VERIFIED sole-writer** |
| heart `racecontrol.db` (`wallet_transactions`, driver_id, V1) | heart binary (`billing_orphan.rs`, `billing_hooks.rs`) | heart + cloud_sync push | VERIFIED (code) |
| heart `racecontrol-v2.db` (`heart_v2_sessions`, `wallets`, `sessions`, `wallet_topups`) | heart binary | heart | VERIFIED (code) |
| `f2_flag_config` (pg) | james flag-admin (`FLAG_ADMIN_STORE=pg`) | james | VERIFIED (empty, 0 rows) |
| `admin.db` | racingpoint-admin V1 (`src/lib/db.ts`) | V1 admin + heart health-probe | VERIFIED (code) |
| `subscription.yaml` | authored (manual) | racecontrol-console (`lib/subscriptions.ts`, `read-model.ts`) | VERIFIED (reader) |
| `venue-registry.json` / fleet-state | heart (Rust) | heart + cloud_sync | UNCONFIRMED (not re-probed) |

### 10.6 Console projections

- **racecontrol-console (`:3220`, HQ Tier-1):** server-side reads — `subscription.yaml` + its own read-model DB (`lib/db.ts`, `read-model.ts`); **no client API base** (backend-rendered). Reads only.
- **captain-console (`:3221`, cloud Tier-2 admin):** reads/writes via `BONO_ADMIN_BASE` = `admin.racingpoint.cloud/api/v2` → `:3211` → **cloud V1 admin** (§10.1 misroute). Wallet **writes** (`posCashTopup`/`posDigitalTopup`/… `mutationAuthority:"james"`) and **reads** (`getWalletBalance/History` `mutationAuthority:"read"`) both traverse the misrouted edge. Also reads `app.racingpoint.cloud/r/` (deep-links) + `signing.racingpoint.cloud` (cert/CRL).

**Edge counts (this map):** VERIFIED ≈13 · **MISROUTED 2** (the `:3211` admin edge + captain-console inheriting it) · UNCONFIRMED 5 · DARK 1. See §11. *(2026-06-23: 4 venue edges upgraded UNCONFIRMED→VERIFIED — see §10.2 / §10.7.)*

### 10.7 Edge-tier topology — common BFF vs patchwork, the PWA bridge, and what consolidation would need (2026-06-23)

> Added 2026-06-23 from a 7-agent read-only grounding (bono memory
> `reference_backend_connection_topology_common_bff_grounding_20260623`), cross-checked against the newer
> per-surface deployed-truth docs `comms-link/surfaces/{launch-portal,pos,pos130,captain-admin,customer-pwa,
> admin-proxy-james,_MULTI-TENANT}.md` (2026-06-22→23). It frames §10's edges around one question: **is there a
> common BFF, or patchwork?** Descriptive only.

**The two-tier shape.**
- **Authority tier = ALREADY common.** `admin-proxy-james` (`:3211`) is the single shared backend-of-record
  every surface funnels into (sole authorization authority §S-469/§S-397; owns pg `wallet_ledger` in-process;
  in-process BillingEngine; 402 gate; F5 audit; only downstream = Rust heart `:8080`, no money). It is **not** a
  proxy to the older cloud express `racingpoint-api-gateway` (`:3100`, razorpay/bookings, zero wallet/pg —
  james never calls it). Engine-of-record = TS james.
- **Edge tier = PATCHWORK.** `@rp/proxy-core` is a **LIBRARY, not a service** — its own package.json:
  *"the framework; admin-proxy-bono and admin-proxy-james consume it"* (`packages/proxy-core/package.json:2-5`).
  Only the **two admin-proxies** (`admin-proxy-bono` cloud edge + `admin-proxy-james` venue authority) consume
  it. Every **deployed standalone surface BFF rolls its OWN `james-upstream` forwarder**, copy-pasted — the pos
  BFF header literally reads *"Lifted from the launch-portal BFF"*. The copies carry **drifting header policy**:
  pos hardcodes `x-rp-surface-origin: pos130`, the launcher stamps **no** venue id (relies on james enforcing
  venue from the F6 JWT), captain-admin **keeps** the `Authorization: Bearer` that the launcher deliberately
  drops. So "one common BFF" is true at the **authority** layer (`:3211`) and **false** at the **edge** layer.

**Per-surface edge (VERIFIED 2026-06-23 — source, plus live where noted).**

| Surface | Edge | Target | proxy-core? |
|---|---|---|---|
| launch-portal `:3360` | own Next catch-all BFF `lib/james-upstream.ts` | **loopback `127.0.0.1:3211`** (`james-upstream.ts:22`); live two-hop probed (`/api/v2/foo`→404 BFF-allowlist, `/sessions/launch`→401 james) | ❌ rolls own |
| pos `:3120` (real till) | own catch-all (`apps/pos`, `feat/pos-bff-till-step1@6a94a7c`) "lifted from launcher" | loopback `:3211`; Basic→`x-rp-staff-pin-auth` on login path only; stamps `surface-origin: pos130` | ❌ rolls own (copy) |
| pos130 `:3130` | route handlers but **MOCK** | in-process `lib/mock-store.ts` — **reaches nothing** | ❌ |
| captain-admin `:3370` | own `lib/admin-upstream.ts` (`feat/captain-admin-slice@e1e6fd7`) | loopback `:3211` directly (**bypasses** admin-proxy-bono) | ❌ rolls own |
| PWA (cloud) | `admin-proxy-bono` (the one real remote edge) | WG → venue james (see bridge below) | ✅ proxy-core |
| captain-console (prod) | `admin-proxy-bono` `:3211` | → V1 admin (§10.1 MISROUTE, re-probed 06-23 still true) | ✅ proxy-core |

> Pattern: **venue-resident surfaces loopback to james `:3211`**; **remote surfaces (PWA, prod captain-console)
> go through `admin-proxy-bono`** over the tunnel. (The `.130` POS box live-reach remains §10.4 DARK — the
> rows above are source + loopback-target verified, not live-box-from-`.23`.)

**Launcher launch correction (vs an earlier reading).** The launch-portal BFF does a **single** forward of
`POST /api/v2/sessions/launch` to james. The dual-effect — 402 pre-launch gate → forward to heart
`/heart/sessions/launch` → on heart-OK `billing.onSessionLaunched` — lives **inside james**
(`admin-proxy-james/src/m5-handlers.ts:129/168/206`), **not** in the launcher BFF.

**The PWA public→venue bridge (live chain, probed on the Bono VPS).**

```
browser → app.racingpoint.cloud (nginx, TLS)
  ├─ / & /_next/static → 127.0.0.1:3302  (rp2-pwa docker — pure Next SPA, no BFF)
  └─ /api/  (nginx strips /api/v2 prefix) → 127.0.0.1:3213  (rp2-proxy docker = admin-proxy-bono)
       → JAMES_ADMIN_PROXY_URL=http://10.42.0.2:3211
       → WireGuard wg0 (10.42.0.1 → peer 10.42.0.2/32)
           …carried INSIDE an outer wstunnel REVERSE tunnel (venue dials OUT; ws://127.0.0.1:8091)
       → venue admin-proxy-james :3211
```

- **LIVE reverse-proxy, per-request — NOT a sync, NOT a cache.** The cloud holds **no** venue state of its own.
- **HAND-BUILT per-venue, NOT templated** (confirms `comms-link/surfaces/_MULTI-TENANT.md`): the WG tunnel +
  nginx vhost are bespoke RP scaffolding with no generator; the single-tenant artifact is the
  capability-manifest store (`get(_tenant_id)` ignores its arg, returns `rp-hyd`). The james target itself is
  **env-overridable** (`JAMES_ADMIN_PROXY_URL`), **not** hard-coded.
- **VPS reaches the venue ONLY via WG peer `10.42.0.2`, never `192.168.31.23` directly** (`ip route`: `.23` →
  public eth0 gw; `10.42.0.2` → `wg0`).
- **Reliability gap:** at probe time the WG forward to james **timed out** (`10.42.0.2:3211` → HTTP 000, N=2;
  proxy up, WG handshake fresh). A live-proxy-only design = PWA venue data **dead whenever the tunnel flaps**.

**What a common BFF would require (design-decision input — descriptive, not a decision).**
- **Already exists (no build):** the shared backend (`admin-proxy-james :3211`), a remote edge the PWA already
  uses (`admin-proxy-bono`), and the shared library (`@rp/proxy-core` + `@rp/contracts` + `@rp/wallet-client`).
- **Consolidation, not net-new (venue surfaces):** collapse the 3–4 hand-rolled forwarders onto `proxy-core`.
  The win is **policy-in-one-place** (Basic→staff-pin translation, venue stamp, surface-origin, route allowlists
  — duplicated and drifting today), **not** throughput. Venue-local surfaces loopback to james directly today;
  forcing them through an edge tier adds a hop for marginal trust (james re-validates every request regardless).
- **Net-new (the PWA remote boundary only):** **(a)** multi-tenant templating — resolve venue from
  request/host instead of the hard-coded `rp-hyd` manifest, and generate the WG tunnel + nginx vhost per venue
  (closes the `_MULTI-TENANT.md` leak); **(b)** the **OPEN DECISION** below.

> **✅ DECIDED 2026-06-23 (Captain) — live-proxy-made-RELIABLE now; venue→cloud sync is the post-venue-live
> upgrade (deferred, NOT cancelled).** Near-term (to venue-live): **keep the live per-request proxy and make it
> reliable** — a venue→cloud replication/read-model is net-new scope **off the proven critical path** and is
> **not** built before venue-live. Long-term: a customer-facing PWA should **not** have venue-data availability
> hostage to the venue tunnel being up at that instant, so **venue→cloud sync/read-model is the right eventual
> architecture** — sequenced **after** venue-live as a resilience/latency upgrade (deferred, **not cancelled**).
> **Near-term implication (flagged follow-up, not part of this doc edit):** the current live-proxy path needs
> **reliability work** — the WG-inside-wstunnel tunnel was timing out (HTTP 000 at probe, §10.7 bridge); the
> near-term need is **tunnel reliability / monitoring / retry**, **NOT** a new data layer. Descriptive only —
> records the decision; changes no runtime config.

> **⚠️ SAFETY (redeploy-from-main).** The live `admin-proxy-james` (`:3211`) carries **+78 lines of
> uncommitted A3 staff-PIN login in ZERO git refs** (`staff-pin-auth-verifier.ts` / `staff-login-handler.ts`;
> money files byte-identical to committed `main`). Any future edge consolidation that triggers a
> redeploy-from-main **must preserve that staff-login path first** (TEST01/CAP0001 + captain-admin `:3370`
> depend on it). Canonical: `comms-link/surfaces/admin-proxy-james.md` governance flag.

---

## 11. Registry residuals & open items (carried, not smoothed)

- **UNCONFIRMED edges:** (1) heart `:8080` → rc-agent pod `:8090` live WS (code-known, not re-probed); (2) ~~venue-surface deployed `NEXT_PUBLIC_JAMES_ADMIN_BASE` value~~ **RESOLVED 2026-06-23 for launcher/pos/pos130/captain-admin** — these have their OWN BFF forwarding over **loopback `127.0.0.1:3211`** (not a browser-direct base); confirmed source + target in §10.7. **staff-tablet/pod-display still UNCONFIRMED** (not re-probed 06-23); (3) staff-tablet → james household-lookup live; (4) SSE consumer base *value* as deployed; (5) `venue-registry.json` / fleet-state writer (not re-probed).
- **DARK edge (1):** `.130` POS box — unreachable from `.23` at probe (§10.4); pos/pos130 live edge unverifiable while down.
- **Stale james pin:** venue `admin-proxy-james` (`.23:3211`) runs a **`28ef314a` tarball base + in-place edits, no `.git`** — the V2 money engine is **NOT commit-pinnable**; "deployed == HEAD" is unverifiable for it (and for all cloud TS surfaces — no `/version`, PWA buildId is random). The heart binary IS pinned (`213712a9`).
- **OTP / registration block:** canonical OTP verify is **server-side on admin-proxy-bono** (`registration-forwarders.ts:258` compares client `code` vs server-stored `row.code`, issues an opaque token) — there is **no MSG91 in the deployed bono src**; `/srv/msg91-test.html` is a separate unwired static page (§7). The **sandbox variant** returns `household: null` and, with **`OTP_BRIDGE_ENABLED` (RP_OTP_BRIDGE_ENABLED) default OFF**, `createHousehold` → James **401** → end-to-end registration is **blocked** until an operator flips the bridge ON. (Auth-boundary decision = Captain.)
- **Durability residual — CLOSED this session:** `WALLET_STORE=pg` confirmed in the deployed `.env` + pg live + `wallet_ledger` queryable (=0) (§1, §2.2). Remaining sliver: the running process's live store-kind via a debug endpoint (none exposed) — but in-memory fallback is structurally impossible (fail-loud boot).
- **Naming OPEN:** the kiosk / launch-portal / launch-core canonical name-strings (§9.1) are **Captain-owned** — not assigned here.
- **MISROUTED remediation:** §10.1 — `JAMES_ADMIN_PROXY_URL=http://10.42.0.2:3211` on the `:3211` instance — is **PENDING-CAPTAIN-GO** (no env/code change made in this doc revision).

---

## 12. Control / config plane (settings-propagation graph — both ends per edge)

> §10 mapped the **money/request** edges; this maps the **CONFIG** plane: how a settings change in the admin
> panel reaches (or fails to reach) each consuming app. Both ends per edge (write-site + read/receive-site).
> Classification {VERIFIED | UI-ONLY | UNCONFIRMED | DISJOINT-PLANE}. Provenance: three read-only Explore
> sweeps over the **git checkouts** (`/root/rp-v2-apps`, `/root/racecontrol`) + live `.23` stores (pg via
> on-host psql; heart sqlite from a 02:33 IST snapshot) 2026-06-09. **⚠️ The deployed james tarball
> (`28ef314a`+in-place edits) DIVERGES from the checkout** — `session_pricing`/`pricing_snapshots` exist on
> the live pg (migration 0034, §S-474) but are absent from the checkout; code citations are repo-truth, live
> probes are deployed-truth, divergences flagged.

### 12.1 🔴 DISJOINT FLAG PLANE — two flag systems, and the admin panel writes only one

There is **no single flag plane.** Two independent flag stores exist on two engines, two write surfaces, two
propagation mechanisms, **no bridge between them:**

| Flag plane | Store | Write surface | Propagation to consumers | Live rows (probe) |
|---|---|---|---|---|
| **HEART (V1/Rust)** | `feature_flags` (sqlite `racecontrol.db`) | heart `POST/PUT /api/v1/flags` (`flags.rs:98-181 / 184-295`) + `config_audit_log` | **WS `FlagSync` push** to rc-agents + agent **300s HTTP pull** (`rc-agent/feature_flags.rs:117-223`); heart reads `heart_v2.rs:738-742` | **6 rows** incl. `heart_v2_real_launch=1` (v5), `kiosk_launch_cards_enabled=1`, `phase363/4/5_*` |
| **JAMES (V2/TS)** | `f2_flag_config` (pg `racingpoint_v2`) | captain-console `/flags` → `PUT /admin/flags/{name}` → james `flag-admin-handlers.ts:294` → `flag-admin-store.ts:165 INSERT…ON CONFLICT` | local 30s TTL + `invalidate()` + bono HTTP fan-out + Redis pub/sub on write | **0 rows** (empty) |

- The captain-console flag UI writes **ONLY the james pg plane** (`f2_flag_config`, currently empty). It **cannot** write `heart_v2_real_launch` or any heart `feature_flags` row — those are set by the heart's own `/api/v1/flags` on a different engine the panel never touches.
- **Consequence:** the flag that gates the heart's real launch/money dispatch — `heart_v2_real_launch` (`heart_v2.rs:738`) — is **invisible to and unwritable from the admin panel.** A "two understandings" hazard: the captain-console flags page is not the flags that gate the launch path.
- `S466_CURVE_ENABLED` is in **neither** plane (`f2_flag_config`=0 rows) → gated env-side / in-code, **outside both admin flag planes** (exact read-site UNCONFIRMED this turn).

### 12.2 🔴 SPLIT CONFIG HUB, UNBRIDGED — james writes, heart distributes, no edge between

There is no single config-distribution hub. The plane splits by config-type, and the two halves do not talk:

- **JAMES = the V2 admin-config WRITE hub.** Everything the captain-console changes lands in james stores via `captain-console → admin-proxy-bono → admin-proxy-james` with `mutationAuthority:"james"`: flags (`f2_flag_config`), rate card (`pricing_store`/`session_pricing`), staff & PINs (`staff_users` pg), café menu, pod out-of-service (capability manifest).
- **HEART = the pod-facing config DISTRIBUTION hub.** The heart is the **only** service that pushes config down to the rc-agents (pods): WS `FlagSync` (`flags.rs:178 broadcast_flag_sync`) + `ConfigPush` for kiosk settings (`cloud_sync_pull.rs`). Its own config (`feature_flags`, `kiosk_settings`, `pricing_tiers`) is fed **cloud→venue by `cloud_sync.rs`** (`SYNC_TABLES`, ~30s relay / configurable HTTP fallback, `cloud_sync.rs:42,246-377`), authored cloud-side (V1 admin), **not** by the captain-console.
- **THE GAP:** there is **no james→heart config write edge.** The pod-OOS handler explicitly makes **no** heart call (`capability-manifest-handlers.ts:22-25` docstring + `:214/260` mutate the james-local store only). So a captain-console config change lands in a james store and **never reaches the heart or the pods**; the config that does reach the pods arrives via `cloud_sync` from the cloud, on a pipe the panel does not drive.

**Plain answer — heart or james as the config hub:** **SPLIT.** James is the admin/write hub for the V2 plane; the heart is the distribution hub to the pods; they are **disconnected** for config. "The admin panel controls all apps" is therefore **not yet true** for the heart/pod plane (gaps §12.5).

> **Control-plane FIX (james→heart config bridge · flag-plane unification · pod-OOS→heart wiring) = PENDING-CAPTAIN + HELD-ON-PARALLEL-LANES.** It must **not** begin while session 1bee95ad's §S-466 curve arc is live in the same contested repos (comms-link / rp-v2-apps). No code/env change made in this doc revision — this section is descriptive only.

### 12.3 Config edge-list (setting → write-surface → store → consumer → mechanism → both-ends? → class)

| # | Setting | Write surface (file:line) | Store | Consumer(s) | Propagation | Both ends? | Class |
|---|---|---|---|---|---|---|---|
| 1 | V2 feature flag | captain-console `/flags` → `PUT /admin/flags/{name}` → james `flag-admin-handlers.ts:294` | `f2_flag_config` (pg) | james billing pipeline | write+invalidate (30s TTL · Redis · bono fan-out) | code ✓ · live 0 rows | **VERIFIED** |
| 2 | Heart/V1 flag (`heart_v2_real_launch`) | heart `POST/PUT /api/v1/flags` (`flags.rs:98/184`) | `feature_flags` (sqlite) | heart + rc-agents | WS `FlagSync` push + 300s pull | code ✓ · live 6 rows | **VERIFIED → DISJOINT** (not admin-panel-reachable) |
| 3 | Rate card (V2) | `putRateCard` mount (`pricing-mounts.ts:73`); **captain-console UI not located** | in-mem `pricing_store` (repo) / `session_pricing`+`pricing_snapshots` (pg live, empty) | BillingEngine | resolved ONCE at `green_light_at`, **immutable per session** (`session-billing.ts:647` → `billing-engine/index.ts:113`) | write-mount ✓ · UI ✗ · prod source UNCONFIRMED | **UI-ONLY / UNCONFIRMED** |
| 4 | Rate card (V1) | cloud → `cloud_sync.rs` pull | `pricing_tiers`/`pricing_rules` (sqlite) | V1 meter `billing_session_start.rs:58` | 30s cloud_sync poll | code ✓ · live ₹700/₹900 tiers | **VERIFIED** (V1-only; V2 meter does NOT read it) |
| 5 | Pod out-of-service (V2) | captain-console `/capability/pods/{id}/out-of-service` → `POST/DELETE` → james `capability-manifest-handlers.ts:214/260` | capability manifest (in-mem james) | **nobody downstream — no heart call** | version bump only; does NOT reach heart/pods | write ✓ · **consumer wire MISSING** | **UI-ONLY (does not propagate)** |
| 6 | Pod maintenance (heart) | **no admin API** — heart self-sets on `PreFlightFailed` (`PodLifecycle::Maintenance`, `heart_v2.rs:53`) | `pods` (heart mem) | rc-agent via PodState SSE | SSE push | code ✓ | **VERIFIED** (but NOT admin-triggerable) |
| 7 | Staff users / PINs | captain-console `/staff` → `POST /auth/staff/users/…` → james `staff-admin-handlers.ts:89` | `staff_users` (pg) + F5 audit | james auth | immediate; F5 audit row | code ✓ | **VERIFIED** |
| 8 | Café menu | captain-console `/cafe-menu` → `POST/PATCH /cafe/menu` → james `cafe-mounts.ts` | cafe menu store | pos / chef-display | not re-probed | write ✓ · consumer UNCONFIRMED | **UNCONFIRMED (consumer)** |
| 9 | Kiosk settings | cloud → `cloud_sync_pull.rs` (**NOT the panel**) | `kiosk_settings` (sqlite) | heart + rc-agents (`ConfigPush`) | 30s poll + `ConfigPush` WS | code ✓ · live 11 keys | **VERIFIED** (not panel-controlled) |
| 10 | Reason-class threshold | captain-console `/reason-class-monitor` → `PUT /pricing/reason-class-monitor/config` | james | james pricing monitor | not re-probed | UI ✓ · backend mount not located | **UNCONFIRMED (backend)** |
| 11 | Leaderboard (V1) | heart `lap_tracker.rs:68` (sync at session end) | `laps`/`track_records`/`personal_bests` (sqlite) | **V1 kiosk/spectator** (`kiosk_settings.spectator_show_leaderboard`) via `leaderboard_public.rs` | synchronous write at session end | code ✓ · live 0 rows | **VERIFIED** (V1 essence-piece) |
| 12 | Leaderboard (V2 read) | — (read only) | cross-venue source | captain-console `/leaderboard` `GET /ac/leaderboard/cross-venue` | on-demand fetch (PII-strict zod) | read ✓ · **no kiosk/pod-display V2 consumer** | **VERIFIED (read-only)** |

### 12.4 Rate-card propagation (owner-edits-card → meter-uses-card), traced

- **Write:** `putRateCard` (`pricing-handlers.ts:146` → `:189`) — in the git checkout this is an **in-memory** `pricingStore` (sandbox), with **no captain-console UI located**. On the **live `.23`** the `session_pricing` + `pricing_snapshots` pg tables exist (0034) but are **empty**.
- **Read at tick:** the meter does **NOT** read a store per tick. The rate is **resolved ONCE at `green_light_at`** (`session-billing.ts:647 resolvePricing` → snapshot cached → `engine.start({effective_rate_per_min_paise})`), and every tick reads the **immutable** snapshot (`billing-engine/index.ts:113-115 creditsPerTick = ceil(rate_per_min_paise/100)`). A rate change therefore affects only **new** sessions; in-flight sessions keep their snapshot. No restart required; no per-tick store read.
- **Open (UNCONFIRMED):** in production, whether `resolvePricing` sources the rate from the james pricing store, the heart pricing crate, or a request param is **not closed** — the live tarball diverges from the checkout. **The rate path must be re-read against the RUNNING bytes before any action.**

### 12.5 Control-coverage gaps (settings the panel shows/owns but that do NOT reach the consuming app)

1. **Heart flags are unreachable from the panel (DISJOINT, §12.1).** `heart_v2_real_launch`, `kiosk_launch_cards_enabled`, the `phase36x` flags — all in heart `feature_flags`, none writable from captain-console `/flags` (which writes the empty `f2_flag_config`).
2. **Pod out-of-service does not actually disable the pod (§12.3 #5).** The captain-console toggle writes the james-local capability manifest + bumps a version, but makes **no heart call** — the heart and the rc-agent never learn the pod is out-of-service. UI + store work; the wire to the consuming app is missing (stub).
3. **Rate card has no located admin UI (§12.4)** + the production rate source is UNCONFIRMED (deployed tarball diverges).
4. **Kiosk settings + V1 pricing are cloud-sync-owned, not panel-owned (§12.3 #9, #4).** `kiosk_settings`, `pricing_tiers`/`pricing_rules` change only via `cloud_sync` from the cloud (V1 admin), not the captain-console.
5. **Leaderboard is split + V2-admin-read-only (§12.3 #11, #12).** Producing/consuming pair is V1/Rust (heart writes laps → V1 kiosk/spectator displays via `spectator_show_leaderboard`); the V2 captain-console has only a separate cross-venue **read**. No V2 `@rp/kiosk`/`pod-display` consumer; currently 0 lap rows.

### 12.6 Provenance caveats (carried)

- Code both-ends are from the **git checkouts**; the **deployed james tarball diverges** (`session_pricing`/`pricing_snapshots`/§S-466 curve present on live, absent in checkout) — re-read the deployed tarball before acting on the rate path.
- Heart sqlite rows are a **02:33 IST snapshot copy** (config tables change rarely; not the live handle).
- Edges #8 (café consumer), #10 (reason-class backend), and the production rate source (#3 / §12.4) are **UNCONFIRMED** — not closed both-ends this turn. Leaderboard tables are empty (0 rows) at probe.
