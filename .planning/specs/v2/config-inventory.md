# V2-P1 Static Config-Read Inventory

**Produced:** 2026-04-21 | **Method:** Static grep (read-only) across active code paths | **Scope:** racecontrol/crates, whatsapp-bot/src, racecontrol/{admin,kiosk,pwa,web}/src, comms-link/src | **Excluded:** `.planning/` docs, `rc-agent-1136fd1a/` historical snapshot, `deploy-staging/`, `graphify-out/`, node_modules, `.next/`

---

## Classification legend

| Class | Meaning | V2-P1 in-scope? |
|---|---|---|
| **S** | Secret / credential material (JWT keys, API keys, HMAC keys) | YES — consolidate into env-only + secret vault |
| **F** | Feature/behavior flag (boolean or enum gate) | YES — migrate onto Phase 177 `feature_flags` |
| **C** | Runtime config value (URL, path, numeric threshold) | YES — migrate onto Phase 177 `config_push_queue` |
| **B** | Bootstrap config (needed to reach Phase 177 service) | NO — stays in racecontrol.toml / env (chicken-and-egg) |
| **O** | OS identity (COMPUTERNAME, APPDATA, LOCALAPPDATA, USERNAME, HOSTNAME) | NO — OS is authoritative |
| **G** | Game/runtime-file config (race.ini, video.ini, python.ini, gui.ini, controls.ini) | NO — belongs to V2-P6 (one kiosk runtime / drop python.ini AC bridge) |
| **A** | Autostart / OS policy registry (HKLM Run, HKCU Policies) | NO — belongs to V2-P5 (one supervisor layer) |
| **T** | Test-only (Playwright spec env vars, build.rs GIT_HASH) | NO — out of scope |

---

## Phase 177 precedent (do not reinvent)

Phase 177 shipped 2026-03-24 (verified 13/13). Already built:

- `feature_flags` SQLite table + REST CRUD (`GET/POST /api/v1/flags`, `PUT /api/v1/flags/:name`)
- `config_push_queue` (per-pod queued typed config with seq_num + schema validation)
- `config_audit_log` (append-only, `pushed_by` from JWT)
- Per-pod override resolution at WS broadcast (`1fc92867`)
- `CoreToAgentMessage::FlagSync` + `::ConfigPush` + `::ConfigAck` over existing WS
- `packages/shared-types/src/config.ts` TS interfaces
- `docs/openapi.yaml` 6 endpoints

**V2-P1 is NOT "build a config service." V2-P1 is "migrate remaining callers onto Phase 177's service."**

What Phase 177 does NOT cover and P1 must add:
- rc-agent as a consumer (today rc-agent has its own `cloud_sync_pull` path reading `kiosk_settings` — needs reconciliation with Phase 177)
- Secret-class env vars (not safe in WS payloads — separate track)
- whatsapp-bot as a consumer (entire bot reads its own `process.env`)
- Frontend runtime config (NEXT_PUBLIC_* is build-time-baked today; needs `/api/v1/client-config` endpoint)

---

## Class S — Secrets (env-only today; P1 hardens to vault)

| Key | File:line | Notes |
|---|---|---|
| `RC_JWT_SECRET` | [crates/racecontrol/src/config/mod.rs:165](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L165) | **Canonical (Phase 0.5a, PACT-20260503-017 Q2 AGREE-B).** Signs customer + staff JWTs. Matches racingpoint-admin TS `RC_JWT_SECRET`. |
| `RACECONTROL_JWT_SECRET` | (same call site) | **Legacy fallback** — deprecated; will be removed in V2.1. Emits deprecation warn if used. Migrate to `RC_JWT_SECRET`. |
| `RACECONTROL_ADMIN_PIN_HASH` | [config/mod.rs:396](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L396) | Admin login hash |
| `RACECONTROL_TERMINAL_SECRET` | [config/mod.rs:402](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L402) | Terminal/kiosk auth |
| `RACECONTROL_RELAY_SECRET` | [config/mod.rs:407](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L407) | Comms-link relay auth |
| `RACECONTROL_EVOLUTION_API_KEY` | [config/mod.rs:412](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L412) | WhatsApp Evolution API |
| `RACECONTROL_GMAIL_CLIENT_SECRET` | [config/mod.rs:417](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L417) | Gmail OAuth |
| `RACECONTROL_GMAIL_REFRESH_TOKEN` | [config/mod.rs:422](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L422) | Gmail OAuth |
| `RACECONTROL_SYNC_HMAC_KEY` | [config/mod.rs:427](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L427) | Cloud↔venue sync integrity |
| `RACECONTROL_ENCRYPTION_KEY` | [crypto/encryption.rs:88](racingpoint/racecontrol/crates/racecontrol/src/crypto/encryption.rs#L88) | At-rest encryption |
| `RACECONTROL_HMAC_KEY` | [crypto/encryption.rs:91](racingpoint/racecontrol/crates/racecontrol/src/crypto/encryption.rs#L91) | Data HMAC |
| `OPENROUTER_KEY` | multiple: [ai/providers.rs:108](racingpoint/racecontrol/crates/racecontrol/src/ai/providers.rs#L108), [rc-agent/openrouter.rs:279](racingpoint/racecontrol/crates/rc-agent/src/openrouter.rs#L279), [rc-sentry/mma_engine.rs:22](racingpoint/racecontrol/crates/rc-sentry/src/mma_engine.rs#L22), [ai_behavior_batch_mma.rs:31](racingpoint/racecontrol/crates/racecontrol/src/ai_behavior_batch_mma.rs#L31), [server_diagnostics_infra.rs:233](racingpoint/racecontrol/crates/racecontrol/src/server_diagnostics_infra.rs#L233) | **DUPLICATED 5× across crates** — candidate for `rc-common` helper |
| `OPENROUTER_API_KEY` | [rc-agent/ai_debugger.rs:601](racingpoint/racecontrol/crates/rc-agent/src/ai_debugger.rs#L601), [rc-watchdog/mma_diagnosis.rs:261](racingpoint/racecontrol/crates/rc-watchdog/src/mma_diagnosis.rs#L261), whatsapp-bot | ⚠️ **Two names in use** (`OPENROUTER_KEY` vs `OPENROUTER_API_KEY`) — this is the drift Phase 177 exists to fix |
| `OPENROUTER_MGMT_KEY` | [rc-agent/openrouter.rs:319](racingpoint/racecontrol/crates/rc-agent/src/openrouter.rs#L319) | OpenRouter child-key provisioning |
| `ANTHROPIC_API_KEY` | [config/mod.rs:387](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L387) | Claude API |
| `RCAGENT_SERVICE_KEY` | [rc-agent/remote_ops.rs:220](racingpoint/racecontrol/crates/rc-agent/src/remote_ops.rs#L220), [rc-agent/mesh_key_cache.rs:137](racingpoint/racecontrol/crates/rc-agent/src/mesh_key_cache.rs#L137) | rc-agent exec auth. ⚠️ **Gap 4 history** — HKLM readers migrated to HTTP fetch `d06769b` 2026-04-18 |
| `RCSENTRY_SERVICE_KEY` | [rc-sentry/main.rs:65](racingpoint/racecontrol/crates/rc-sentry/src/main.rs#L65) | rc-sentry exec auth |
| `GUARDIAN_COMMS_KEY` | [rc-guardian/comms.rs:65](racingpoint/racecontrol/crates/rc-guardian/src/comms.rs#L65) | Guardian-to-comms-link auth |
| `GUARDIAN_EVOLUTION_KEY` | [rc-guardian/config.rs:181](racingpoint/racecontrol/crates/rc-guardian/src/config.rs#L181) | Guardian-direct WhatsApp |
| `NVR_USERNAME` + `NVR_PASSWORD` | [rc-sentry-ai/config.rs:360,369](racingpoint/racecontrol/crates/rc-sentry-ai/src/config.rs#L360) | Dahua NVR RTSP |
| `COMMS_PSK` | [rc-watchdog/bono_alert.rs:19](racingpoint/racecontrol/crates/rc-watchdog/src/bono_alert.rs#L19) | Comms-link PSK |
| `RC_TERMINAL_SECRET` | whatsapp-bot (4 files) | Duplicates server `RACECONTROL_TERMINAL_SECRET` with different name — drift |
| `RC_SERVICE_PIN` | [whatsapp-bot/src/config.ts:8](racingpoint/whatsapp-bot/src/config.ts#L8) | |
| `EVOLUTION_API_KEY` | [whatsapp-bot/src/config.ts:4](racingpoint/whatsapp-bot/src/config.ts#L4) | ⚠️ **Hardcoded fallback** `'zNAKEHsXudyqL3dFngyBJAZWw9W4hWN0'` |
| `EVOLUTION_WEBHOOK_SECRET` + `EVOLUTION_WEBHOOK_TOKEN` | whatsapp-bot/routes/webhook | Inbound WhatsApp auth |
| `INTERNAL_API_SECRET` | [web/src/app/api/cafe/generate-graphic/route.tsx:102](racingpoint/racecontrol/web/src/app/api/cafe/generate-graphic/route.tsx#L102) | Server-internal RPC |
| `NEXT_PUBLIC_WS_TOKEN` | web + kiosk multiple | ⚠️ **NEXT_PUBLIC_ prefix means this ships to the browser** — not a secret in the crypto sense, but named like one |

**P1 secrets action:** Consolidate into one `secrets.rs` loader with canonical names (pick ONE of `OPENROUTER_KEY` vs `OPENROUTER_API_KEY`, delete the other fleet-wide). No Phase 177 migration — secrets stay env-only. Remove hardcoded fallback in `whatsapp-bot/config.ts:4`.

---

## Class F — Feature/behavior flags (migrate to Phase 177 `feature_flags`)

| Key | File:line | Current source | Target |
|---|---|---|---|
| `RC_IS_CLOUD` | [config/mod.rs:131](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L131) | env | Flag — cloud vs venue runtime role. Set at pm2 start; not hot-swappable. **Borderline B** |
| `RC_ALLOW_VENUE_STAFF_WRITE` | [config/mod.rs:150](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L150) | env | Flag — authorization escape hatch |
| `RC_ALLOW_CLOUD_VENUE_WRITE` | [config/mod.rs:156](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L156) | env | Flag — authorization escape hatch |
| `RC_ALLOW_SESSION0` | [rc-agent/main.rs:635](racingpoint/racecontrol/crates/rc-agent/src/main.rs#L635) | env | Flag — bypass Session 1 enforcement (test only). Banned in prod |
| `TEST_MODE` | [api/customer_auth.rs:148](racingpoint/racecontrol/crates/racecontrol/src/api/customer_auth.rs#L148), [api/routes.rs:133](racingpoint/racecontrol/crates/racecontrol/src/api/routes.rs#L133) | env | Flag — enables `/customer/test-mint-jwt`. Set at boot in branch `fix/pos-kiosk-disable-20260421` |
| `RCSENTRY_PERMISSIVE_MODE` | [rc-sentry/main.rs:76](racingpoint/racecontrol/crates/rc-sentry/src/main.rs#L76) | env | Flag — rc-sentry relaxed auth |
| `MAINTENANCE_MODE` | (sentinel file on disk, not env) | file existence | Flag — blocks all restarts once set. Not reachable by Phase 177 today (pod-local). Keep as sentinel for now |
| `kiosk_lockdown_enabled` | `kiosk_settings` DB table | DB | ⚠️ **Already in Phase 177 territory** but via `cloud_sync_pull`, NOT `config_push_queue`. Reconcile |
| `screen_blanking_pods` | `kiosk_settings` DB table | DB | Same as above |
| `pos_lockdown` | `kiosk_settings` DB table | DB | Same as above |
| `NEXT_PUBLIC_IS_CLOUD` | [pwa/src/app/book/page.tsx:19](racingpoint/racecontrol/pwa/src/app/book/page.tsx#L19) | build-time env | Flag — cloud vs venue PWA behavior. Today baked at build time → mirror of backend `RC_IS_CLOUD` |

**P1 flags action:** Add a "Phase 177 first" lint — new env flags must register in `feature_flags` table. Existing `kiosk_settings` reads should converge with Phase 177 (today they're parallel paths, which is the drift).

---

## Class C — Runtime config values (migrate to Phase 177 `config_push_queue`)

| Key | File:line | Current source | Target |
|---|---|---|---|
| `RACECONTROL_SERVER_IP` | [diagnostic_engine.rs:589,644](racingpoint/racecontrol/crates/rc-agent/src/diagnostic_engine.rs#L589), [tier_engine.rs:445,2539](racingpoint/racecontrol/crates/rc-agent/src/tier_engine.rs#L445) | env with hardcoded fallback `192.168.31.23` | Bootstrap-ish. If env unset, falls back to literal IP. Candidate C |
| `OLLAMA_URL`, `OLLAMA_MODEL` | [config/mod.rs:379,383](racingpoint/racecontrol/crates/racecontrol/src/config/mod.rs#L379) | env | Config push candidate |
| `RACECONTROL_DB` | [weekly-report/main.rs:14](racingpoint/racecontrol/crates/weekly-report/src/main.rs#L14) | env | DB path — bootstrap |
| `EMAIL_SCRIPT`, `EMAIL_RECIPIENT` | [weekly-report/main.rs:16,18](racingpoint/racecontrol/crates/weekly-report/src/main.rs#L16) | env | Config push candidate |
| `ESCALATION_WHATSAPP_NUMBER` | [whatsapp_escalation.rs:48](racingpoint/racecontrol/crates/racecontrol/src/whatsapp_escalation.rs#L48) | env | Config push candidate (business data) |
| `ADMIN_PHONE`, `RAMU_PHONE` | whatsapp-bot | env | Config push candidate |
| `GUARDIAN_*` (10+ vars) | [rc-guardian/config.rs:152-184](racingpoint/racecontrol/crates/rc-guardian/src/config.rs#L152) | env | Config push candidate |
| `RACECONTROL_URL` | whatsapp-bot (5+ files) | env with hardcoded fallback `http://localhost:8080` | Config push candidate |
| `POS_URL`, `DASHBOARD_URL` | [whatsapp-bot/src/staff/dashboard.ts:125,165,187](racingpoint/whatsapp-bot/src/staff/dashboard.ts#L125) | env with hardcoded fallbacks | Config push candidate |
| `SQLITE_PATH`, `RACECONTROL_DB_PATH` | [whatsapp-bot/src/config.ts:10,11](racingpoint/whatsapp-bot/src/config.ts#L10) | env with hardcoded fallback | Bootstrap-ish |
| `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_WS_URL`, `NEXT_PUBLIC_SENTRY_URL`, `NEXT_PUBLIC_SENTRY_WS`, `NEXT_PUBLIC_GATEWAY_URL` | web + kiosk + pwa (20+ sites) | **build-time env** | ⚠️ **Architecturally different** — compile-time bake means a config change requires rebuild + redeploy. Needs `/api/v1/client-config` runtime endpoint for true migration |
| `AI_MODEL` | [whatsapp-bot/claudeService.js:45](racingpoint/whatsapp-bot/src/services/claudeService.js#L45) | env | Config push candidate |
| `RACECONTROL_INTERNAL_URL` | [web/api/cafe/generate-graphic/route.tsx:103](racingpoint/racecontrol/web/src/app/api/cafe/generate-graphic/route.tsx#L103) | env | Config push candidate |
| `GUARDIAN_POLL_INTERVAL`, `GUARDIAN_DEAD_MAN_THRESHOLD` | [rc-guardian/config.rs:167,171](racingpoint/racecontrol/crates/rc-guardian/src/config.rs#L167) | env numeric | Config push candidate |
| `LOG_LEVEL` | whatsapp-bot/utils/logger.js | env | Config push candidate |

---

## Class B — Bootstrap (excluded from P1 — chicken-and-egg)

These must resolve BEFORE the config service can be reached:

- `racecontrol.toml` path + contents (server)
- `rc-agent.toml` path + contents (pods)
- Initial DB connection string
- Initial server URL for agents to fetch config from
- `RC_IS_CLOUD` (determines which code paths run, must be known at boot)
- `COMPUTERNAME` / `HOSTNAME` / pod identity (class O — OS-authoritative)

**P1 rule:** bootstrap config stays in files + env. Document the minimal bootstrap surface; everything else migrates.

---

## Class O — OS identity (excluded — OS is authoritative)

| Key | File:line |
|---|---|
| `COMPUTERNAME` | [rc-watchdog/service.rs:190](racingpoint/racecontrol/crates/rc-watchdog/src/service.rs#L190), [rc-agent/main.rs:651](racingpoint/racecontrol/crates/rc-agent/src/main.rs#L651), [mesh_gossip.rs:233](racingpoint/racecontrol/crates/rc-agent/src/mesh_gossip.rs#L233), [self_heal.rs:216](racingpoint/racecontrol/crates/rc-agent/src/self_heal.rs#L216), [tier_engine.rs:588,2197](racingpoint/racecontrol/crates/rc-agent/src/tier_engine.rs#L588), [mdns.rs:33](racingpoint/racecontrol/crates/racecontrol/src/mdns.rs#L33) |
| `HOSTNAME` | [mdns.rs:34](racingpoint/racecontrol/crates/racecontrol/src/mdns.rs#L34) (fallback for COMPUTERNAME) |
| `USERNAME` | [tier_engine.rs:2333](racingpoint/racecontrol/crates/rc-agent/src/tier_engine.rs#L2333) |
| `APPDATA` | [process_guard.rs:628](racingpoint/racecontrol/crates/rc-agent/src/process_guard.rs#L628), [rc-process-guard/main.rs:446](racingpoint/racecontrol/crates/rc-process-guard/src/main.rs#L446) |
| `LOCALAPPDATA` | [startup_cleanup.rs:435,525](racingpoint/racecontrol/crates/rc-agent/src/startup_cleanup.rs#L435) |

Don't touch.

---

## Class G — Game config files (excluded — V2-P6 scope)

INI files rc-agent writes as part of game-launch control (Assetto Corsa Content Manager / CSP mode):

- `gui.ini` — CSP FORCE_START (kiosk mode entry)
- `video.ini` — 7680x1440@179Hz triple-screen enforcement
- `controls.ini` — Conspit wheelbase FFB template
- `race.ini` — session config (car, track, AI level, AI cars)
- `assists.ini` — difficulty preset
- `python.ini` — `[RACECONTROL]` plugin enablement (Zero Laps root cause)
- `models.ini` (car content validation)

All writes at [crates/rc-agent/src/ac_launcher.rs](racingpoint/racecontrol/crates/rc-agent/src/ac_launcher.rs). **V2-P6 goal:** drop python.ini AC bridge entirely (AC SDK or UDP-only telemetry); keep the other four as runtime-written artifacts rc-agent owns.

---

## Class A — Autostart / OS policy registry (excluded — V2-P5 scope)

Registry reads/writes that implement supervision or lockdown:

- `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` — RCAgent / RCSentry / BillingDashboard / server watchdog boot
- `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` — bloatware cleanup targets
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings` — lock-screen notification suppression
- `HKCU\Software\Policies\Microsoft\Windows\Explorer` — kiosk lockdown (NoWinKeys)
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System` — DisableTaskMgr
- `HKLM\SOFTWARE\Policies\Microsoft\Edge` — Edge enterprise policy
- `HKLM\SOFTWARE\OpenSSH` — OpenSSH service removal

Found in: [rc-installer/main.rs](racingpoint/racecontrol/crates/rc-installer/src/main.rs), [rc-process-guard/main.rs](racingpoint/racecontrol/crates/rc-process-guard/src/main.rs), [rc-agent/self_heal.rs](racingpoint/racecontrol/crates/rc-agent/src/self_heal.rs), [rc-agent/startup_cleanup.rs](racingpoint/racecontrol/crates/rc-agent/src/startup_cleanup.rs), [rc-agent/kiosk.rs](racingpoint/racecontrol/crates/rc-agent/src/kiosk.rs), [rc-agent/lock_screen.rs](racingpoint/racecontrol/crates/rc-agent/src/lock_screen.rs), [rc-agent/process_guard.rs](racingpoint/racecontrol/crates/rc-agent/src/process_guard.rs).

**V2-P5 goal:** replace HKLM Run + schtasks + pm2 + RCWatchdog tangle with one declarative supervisor. Registry policy lockdowns (kiosk) stay Windows-native — not "config" in the v2 sense.

---

## Class T — Test-only (excluded)

- `STAFF_PIN`, `POD_NAME`, `GAME`, `DRIVER_NAME`, `TIER`, `RC_API_URL` — Playwright spec env vars
- `GIT_HASH_FORCE` — build.rs override
- `RACECONTROL_BUILD_ID` — runtime build ID

---

## Drift & duplication findings (call out separately)

1. **`OPENROUTER_KEY` vs `OPENROUTER_API_KEY`** — two names for the same thing across 7+ files. Canonicalize.
2. **`RC_TERMINAL_SECRET` (whatsapp-bot) vs `RACECONTROL_TERMINAL_SECRET` (server)** — same secret, two names. Canonicalize.
3. **Hardcoded `EVOLUTION_API_KEY` fallback** at [whatsapp-bot/src/config.ts:4](racingpoint/whatsapp-bot/src/config.ts#L4) — literal API key in source. Remove.
4. **Hardcoded `RC_TERMINAL_SECRET` fallback** `'rp-terminal-2026'` at [notificationRouter.js:14](racingpoint/whatsapp-bot/src/services/notificationRouter.js#L14), [staffAlertService.js:6](racingpoint/whatsapp-bot/src/services/staffAlertService.js#L6). Remove — secrets MUST NOT have code fallbacks.
5. **Hardcoded `192.168.31.23` fallback** for `RACECONTROL_SERVER_IP` in 4 call sites — reasonable for venue-default, but env-override should be `config_push_queue` post-P1.
6. **`kiosk_settings` table reads vs Phase 177 `config_push_queue`** — parallel paths for the same purpose. Reconcile: one owns, the other deprecates.
7. **NEXT_PUBLIC_* baked at build time** — a config change needs rebuild + redeploy. 20+ sites. Architectural debt; needs `/api/v1/client-config` endpoint.

---

## P1 scope (locked by this inventory)

**In:**
- Reconcile `kiosk_settings` DB reads with Phase 177 `config_push_queue` — pick one
- Migrate rc-agent's "read server endpoint + parse WS push" path onto Phase 177 consumer API (exists but not widely used)
- Consolidate all secret-class env reads into one `rc-common::secrets` loader
- Canonicalize duplicate-named env vars fleet-wide (OPENROUTER_KEY, RC_TERMINAL_SECRET)
- Remove hardcoded secret fallbacks in whatsapp-bot
- Add `/api/v1/client-config` runtime endpoint + migrate NEXT_PUBLIC_* sites off build-time
- CI drift-detector: block new `std::env::var` / `process.env` outside `rc-common::secrets` or the client-config endpoint

**Out (documented for later phases):**
- AC INI files (V2-P6)
- HKLM autostart + OS policy (V2-P5)
- OS identity env vars
- Bootstrap config files

---

## Next step

Ship this inventory + the pivoted `V2-P1-CONFIG-SERVICE.md` to Uday for review. Questions to lock before P1 kickoff:

1. `OPENROUTER_KEY` vs `OPENROUTER_API_KEY` — which name wins?
2. `RC_TERMINAL_SECRET` vs `RACECONTROL_TERMINAL_SECRET` — merge to which?
3. `/api/v1/client-config` design — Uday's preference on freshness vs bundle size?
4. `kiosk_settings` table — deprecate or keep as Phase 177's read-model? (Today code reads both.)
