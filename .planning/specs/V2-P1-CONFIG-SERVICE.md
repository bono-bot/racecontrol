# V2 Phase 1 — Unified Config/State Service (pivoted after Phase 177 discovery)

**Parent:** `V2-FOUNDATION-MILESTONE.md`
**Status:** DRAFT — awaiting sign-off
**Created:** 2026-04-21
**Pivoted:** 2026-04-21 — after discovering Phase 177 (`server-side-registry-config-foundation`, shipped 2026-03-24) already built the service. V2-P1 is now **caller migration**, not new service build.
**Inventory:** `v2/config-inventory.md` (190 config-read sites enumerated; scope classes S/F/C/B/O/G/A/T)
**Duration estimate:** 2 wk dev + 2 wk staged rollout (down from 3+2 pre-pivot)
**Kills:** state-drift class (Gap 4, DB-2, Zero Laps python.ini, HKLM/ini/DB triple-tracking)

---

## Problem

Today a single config value can live in up to 5 places:
- Windows Registry (`HKLM\SOFTWARE\...` and `HKCU\...`)
- `%APPDATA%\*.ini` or install-dir `*.ini` (python.ini Zero Laps precedent)
- SQLite tables (`kiosk_settings`, `pricing_rules`, `pods`)
- rc-agent in-memory cache (fetched at boot, refreshed periodically)
- WebSocket payload (server-pushed overrides)

No single path is authoritative. Fixes land in one location and drift from the others. Examples from memory:
- **Gap 4** — rc-agent read `mesh_service_key` from HKLM; server owned truth; fetch-at-boot path added 2026-04-18 but HKLM readers survive.
- **Zero Laps** — `python.ini` in `Documents\Assetto Corsa\` overrode install-dir ini; 7/8 pods missing `[RACECONTROL]` for months; 0 laps ever recorded.
- **DB-2** — SQL schema fix deployed; server DB at `data/racecontrol.db` vs expected location — config drift in the deploy itself.

---

## Goal (revised)

Migrate remaining config callers onto Phase 177's shipped service. Where Phase 177 doesn't cover (secrets, client-side runtime config, INI files), add the missing piece with the same isolation contract.

**Phase 177 already provides:**
- `feature_flags` table + REST + WS FlagSync + per-pod overrides (`1fc92867`)
- `config_push_queue` per-pod typed config with seq_num + schema validation
- `config_audit_log` append-only with `pushed_by` from JWT
- `packages/shared-types/src/config.ts` TypeScript interfaces
- `docs/openapi.yaml` 6 endpoints

**What V2-P1 adds:**
- Reconcile `kiosk_settings` DB reads with Phase 177 `config_push_queue` (today they are parallel paths — the drift)
- `rc-common::secrets` loader: single-owner for all S-class env reads (JWT secrets, API keys, HMAC keys)
- `/api/v1/client-config` runtime endpoint + migrate 20+ `NEXT_PUBLIC_*` build-time sites onto it
- CI drift-detector: block new `std::env::var` / `process.env` outside the two allowed paths
- Canonicalize drift (`OPENROUTER_KEY` vs `OPENROUTER_API_KEY`, `RC_TERMINAL_SECRET` vs `RACECONTROL_TERMINAL_SECRET`)
- Remove hardcoded secret fallbacks in `whatsapp-bot/src/config.ts:4` and `notificationRouter.js:14`

---

## Non-goals

- NOT replacing SQLite as operational data store. Billing sessions, laps, events stay where they are.
- NOT touching runtime secrets storage (Windows Credential Manager, env vars for API keys). Out of scope.
- NOT a distributed config system (no etcd, no Consul). Single-node Rust crate + shim for JS consumers.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│  rc-config (Rust crate)                         │
│  - ConfigStore::get(key) -> TypedValue          │
│  - ConfigStore::set(key, value, caller)         │
│  - ConfigStore::subscribe(key) -> Stream        │
│  - Persistence: SQLite table `config_v2`        │
│  - Write-ahead audit log (who/when/old/new)     │
└───────────────┬─────────────────────────────────┘
                │
     ┌──────────┼──────────────┬─────────────┐
     │          │              │             │
┌────▼───┐  ┌───▼────┐   ┌─────▼─────┐  ┌───▼────┐
│rc-agent│  │server  │   │admin (JS) │  │kiosk   │
│(native)│  │(native)│   │via HTTP   │  │via HTTP│
└────────┘  └────────┘   └───────────┘  └────────┘
```

- **Native consumers** (rc-agent, server) link `rc-config` as a crate.
- **JS consumers** (admin, pwa, kiosk) read via a new HTTP endpoint `/api/v1/config/:key` with signed tokens.
- Writes are centralized — only admin + staff endpoints can mutate; migration shims write on behalf of legacy code paths.

---

## Isolation contract (per milestone rules)

1. **Additive deploy.** `rc-config` crate ships with the next server/rc-agent binary, but callers are gated behind a per-key feature flag.
2. **Single-flag activation per key.** Env var `RC_CONFIG_V2_KEYS=mesh_service_key,pod_id` — only listed keys route through the service. Others use legacy path.
3. **Per-target rollout.** Enable one key on Bono VPS first (no customer traffic), then Pod 4 (historically most stable), then widen.
4. **Kill-switch.** Remove key from `RC_CONFIG_V2_KEYS`; legacy path resumes on next binary restart (<60 s). No redeploy.
5. **Observability before traffic.** Prometheus metrics `rc_config_get_total{key,source=v2|legacy}` + log line on every v2 read/write, scraped and dashboarded before first flip.

---

## Rollout order (revised around Phase 177)

Ordered by dependency and blast radius:

| # | Work | Duration | Isolation | Why this order |
|---|---|---|---|---|
| 1 | **Canonicalize drift pairs** (`OPENROUTER_KEY` ← `OPENROUTER_API_KEY`, `RACECONTROL_TERMINAL_SECRET` ← `RC_TERMINAL_SECRET`). Commit reads both, writes one; flip when all readers updated | 2 days | Dual-read shim | Zero behavior change; removes follow-on ambiguity |
| 2 | **Remove hardcoded secret fallbacks** in whatsapp-bot (`config.ts:4`, `notificationRouter.js:14`, `staffAlertService.js:6`). Fail loudly on missing env | 1 day | Whatsapp-bot only — no server touch | Security win; isolated to one process |
| 3 | **`rc-common::secrets` loader** — central S-class reader. All crates import the loader; no new `std::env::var` for secrets | 3 days | Additive — old call sites untouched until migrated | Blueprint for CI drift-detector |
| 4 | **CI drift-detector** — lint blocks new `std::env::var` / `process.env` outside allowlisted paths | 2 days | CI only, zero runtime impact | Freezes the surface before widening |
| 5 | **Reconcile `kiosk_settings` ↔ Phase 177** — pick one to be authoritative, make the other a read-through shim, update all consumers | 4 days | Read-through shim; old readers keep working | Kills the specific drift class the 2026-04-18 lockdown incident exposed |
| 6 | **`/api/v1/client-config` runtime endpoint** — runtime-served replacement for `NEXT_PUBLIC_*` sites. Kiosk + PWA + admin web fetch at boot + poll | 5 days | New endpoint; old build-time bake keeps working until each site migrates | Architectural change; must be gradual |
| 7 | **Migrate 20+ `NEXT_PUBLIC_*` call sites** onto `/api/v1/client-config` — one file at a time, behind a flag in Phase 177 `feature_flags` | 4 days | Per-file, behind flag | Each file migration is independently revertable |

**Total:** ~3 wk dev (down from original 3+2 estimate). Rollout is additive throughout — no fleet cutover needed for items 1-4.

**Out of P1 scope (documented in inventory for later phases):**
- AC INI files (race.ini, video.ini, python.ini, etc.) — V2-P6
- HKLM Run / autostart / OS policy — V2-P5
- OS identity env vars (COMPUTERNAME, APPDATA, USERNAME) — never migrate
- Bootstrap config (racecontrol.toml, rc-agent.toml, initial DB path) — never migrate (chicken-and-egg)

---

## Build sequence

### Week 1 — Foundations (no fleet touch)
- [ ] Scaffold `rc-config` crate in racecontrol workspace
- [ ] Define `ConfigKey` enum, `TypedValue` union, `ConfigStore` trait
- [ ] SQLite schema `config_v2` + audit-log `config_v2_audit`
- [ ] Unit tests for get/set/subscribe with in-memory backend
- [ ] **Static audit** (pre-work): grep every repo for `HKLM`, `read_to_string.*\.ini`, `std::env::var`, `ConfigStore`-like patterns → produce `config-inventory.md`

### Week 2 — Key 1 (mesh_service_key) end-to-end
- [ ] Migration shim: `rc-config` reads existing server DB → writes into `config_v2` on first run
- [ ] rc-agent: new call site behind `RC_CONFIG_V2_KEYS` check
- [ ] Server: `/api/v1/config/mesh_service_key` endpoint + token gating
- [ ] Metrics + logs wired
- [ ] Dashboards published

### Week 3 — Key 1 rollout + Keys 2-3
- [ ] Enable on Bono VPS (soak 48h)
- [ ] Enable on Pod 4 (soak 24h)
- [ ] Widen to Pods 1-3, 5-8, POS (one per day minimum)
- [ ] Begin Key 2 (`pod_id`) and Key 3 (`MAINTENANCE_MODE`)

### Weeks 4-5 — Keys 4-6
- [ ] `kiosk_lockdown_enabled` — careful, per `2026-04-18 lockdown` incident
- [ ] `pricing_rules_snapshot` — audit log finally exists
- [ ] `python_ini_contents` — ends Zero Laps class permanently

---

## What starts THIS session (zero runtime impact)

1. This doc (written) + V2-FOUNDATION-MILESTONE.md (written)
2. **NEXT**: Static audit grep pass over all repos → `config-inventory.md` draft (read-only, local)

That's the immediate-effect, isolated work. Nothing else this session unless you approve more.

---

## Evidence-before-claims checklist (per CGP H3)

For every key migration, the "complete" claim requires:

- **BEHAVIOR:** "Pod N rc-agent now fetches `mesh_service_key` from `/api/v1/config/mesh_service_key` and the legacy HKLM read path is not exercised on boot."
- **RAW OUTPUT:** tail of rc-agent log showing `rc_config_get{key=mesh_service_key,source=v2}` counter increment and absence of HKLM read log line.
- **WHERE:** "Pod 4 from James via relay exec; Bono VPS via local rc-agent log."
- **NOT TESTED:** explicit list — e.g., "rotation while pod offline not tested this run."

No "config migrated" claims without this block per target per key.

---

## Rollback runbook (per key)

1. `ssh` to affected target
2. Edit service env file: remove key from `RC_CONFIG_V2_KEYS=...`
3. `systemctl restart rc-agent` (or equivalent supervisor)
4. Confirm legacy read in logs: `rc_config_get{source=legacy,key=...}` increment
5. Post in comms-link INBOX

Expected revert time: **under 60 seconds** per target.

---

## Open questions for Uday (revised after Phase 177 pivot)

1. **`OPENROUTER_KEY` vs `OPENROUTER_API_KEY` — which name wins?** Need to pick one fleet-wide. Same for `RC_TERMINAL_SECRET` vs `RACECONTROL_TERMINAL_SECRET`.
2. **`kiosk_settings` table authority** — deprecate and fold into Phase 177 `config_push_queue`, OR keep `kiosk_settings` as the read model and have Phase 177 write through to it? Today both exist and drift.
3. **`/api/v1/client-config` design** — single endpoint returning all client-visible config, or per-feature endpoints? Fresh fetch on every page, or cached with SSE invalidation?
4. **Secrets storage long-term** — env vars forever, or move to Windows Credential Manager / HashiCorp Vault / AWS SSM at some point? (P1 just consolidates the reader; vault migration is a separate future phase.)
5. **`RC_IS_CLOUD` handling** — it's currently env-set at pm2 start and determines which auth middleware runs. Is it safe to make this hot-swappable (Phase 177 flag), or must it stay boot-only?
