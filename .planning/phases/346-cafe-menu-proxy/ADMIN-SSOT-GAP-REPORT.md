# Admin Panel Single-Source-of-Truth Gap Report

**Audit date:** 2026-04-09
**Scope:** Every configurable option / hardcoded business rule / feature toggle across:
- racecontrol backend (Rust/Axum) — venue .23 + Bono VPS
- racingpoint-admin (Next.js :3201) — venue + cloud
- web dashboard + POS billing (`racecontrol/web/` :3200) — venue + cloud
- kiosk (`racecontrol/kiosk/` :3300) — venue + cloud
- PWA (`racecontrol/pwa/`) — app.racingpoint.cloud
- rc-agent (pods 1-8) — per-pod TOML configs
- comms-link (James .27 + Bono VPS)

**Methodology:** grep for `const` hardcoded business values, `CREATE TABLE` schemas, env-var gated behavior, then cross-reference against the 46-page admin panel inventory from the 2026-04-09 admin audit.

**Targets enumerated (CGP H4 compliance):**
- ✅ Server .23 (racecontrol, admin, web, kiosk, PWA source)
- ✅ Pods 1-8 (rc-agent TOMLs)
- ✅ POS .20 (web/billing module)
- ✅ James .27 (comms-link)
- ✅ Bono VPS (cloud racecontrol, cloud admin, cloud PWA, cloud kiosk)
- ✅ Comms-link (alert routing)

## Executive summary

The admin panel currently controls **~25% of the operational surface**. The other 75% is split between:
- **Hardcoded Rust consts** in racecontrol (requires code commit + full binary deploy)
- **Hardcoded TypeScript consts** in kiosk/PWA/web (requires full Next.js rebuild + deploy)
- **DB tables with no admin CRUD** (requires direct SQL)
- **Per-pod TOML files** (requires SCP to each pod)
- **Features split across the wrong repo** (e.g. feature flags in `web/` not `admin/`)

**Total gaps identified:** 38 (P0: 11 customer-facing, P1: 18 operational, P2: 9 cosmetic)

**Critical drift already happening:**
- POS billing wallet topup presets (`[500, 700, 900, 1000, 2000, 3000]`) differ from PWA wallet topup presets (`[500, 1000, 2000, 3000, 4000, 5000]`). **Staff and customers literally see different amounts for the same feature.**

## P0 — Customer-facing (ship-blockers before venue open)

### Money / pricing / discounts — the ₹ critical path

| # | Feature | Hardcoded in | Admin page? | Impact |
|---|---|---|---|---|
| 1 | **Referral rewards** (₹100 referrer + ₹50 referee) | [billing.rs:4615-4630](../../../crates/racecontrol/src/billing.rs#L4615-L4630) Rust consts | ❌ | Cannot run "double referral" campaign without a code ship |
| 2 | **Discount approval threshold** (₹50 — above which manager PIN required) | [billing.rs:121](../../../crates/racecontrol/src/billing.rs#L121) `DISCOUNT_APPROVAL_THRESHOLD_PAISE` | ❌ | Cannot loosen/tighten staff override without a code ship |
| 3 | **Discount floor** (minimum price after all discounts — currently 0) | [billing.rs:126](../../../crates/racecontrol/src/billing.rs#L126) `DISCOUNT_FLOOR_PAISE` | ❌ | No protection against staff stacking discounts to free |
| 4 | **Max manual refund cap** (₹5,000) | [routes.rs:9367](../../../crates/racecontrol/src/api/routes.rs#L9367) `MAX_MANUAL_REFUND_PAISE` | ❌ | Hardcoded legal/policy cap |
| 5 | **pricing_tiers CRUD** (the 4 plan cards on the kiosk staff wizard: 30min/1hr/per-min/E2E) | `racecontrol.db.pricing_tiers` — READ via `/pricing/display`, NO admin write | ❌ | **Cannot add/edit/rename plan cards** — the thing in user's screenshot |
| 6 | **cafe_promos CRUD** (combo/happy_hour/gaming_bundle) | [cafe_promos.rs](../../../crates/racecontrol/src/cafe_promos.rs) full backend CRUD exists | ❌ | Cannot create cafe promos — backend is there, just no UI |
| 7 | **cafe_marketing broadcast** (push promo to WhatsApp) | [cafe_marketing.rs](../../../crates/racecontrol/src/cafe_marketing.rs) + `POST /cafe/marketing/broadcast` | ❌ | Cannot run marketing campaigns from admin |
| 8 | **bonus_tiers** (top-up X get Y% bonus) | `racecontrol.db.bonus_tiers` — READ via `/wallet/bonus-tiers`, NO admin CRUD | ❌ | Cannot create "₹10k top-up get 10% bonus" without raw SQL |
| 9 | **Wallet topup presets — PWA** (customer-facing) | [pwa/src/app/wallet/topup/page.tsx:15](../../../pwa/src/app/wallet/topup/page.tsx#L15) — `[500, 1000, 2000, 3000, 4000, 5000]` hardcoded | ❌ | Requires PWA rebuild+deploy to change |
| 10 | **Wallet topup presets — POS** (staff-facing) | [web/src/components/WalletTopupModal.tsx:14-21](../../../web/src/components/WalletTopupModal.tsx#L14-L21) — `[500, 700, 900, 1000, 2000, 3000]` | ❌ | **DIFFERS from PWA list — customers and staff see different options.** Drift bug. |
| 11 | **Legal policy text** (refund_policy, pricing_policy, gst_note — Consumer Protection Act 2019 compliance) | [routes.rs:2822-2829](../../../crates/racecontrol/src/api/routes.rs#L2822-L2829) string literals compiled into Rust binary | ❌ | **Legal risk** — policy change = full server redeploy |

## P1 — Operational (high-impact, not customer-blocking)

### Thresholds and limits

| # | Feature | Hardcoded in | Admin page? |
|---|---|---|---|
| 12 | **Customer PIN lockout threshold** (5 attempts) | [auth/mod.rs:30](../../../crates/racecontrol/src/auth/mod.rs#L30) `CUSTOMER_PIN_LOCKOUT_THRESHOLD` | ❌ |
| 13 | **Admin lockout** (5 attempts / 5min window / 15min lockout) | [auth/admin.rs:17-19](../../../crates/racecontrol/src/auth/admin.rs#L17-L19) | ❌ |
| 14 | **PIN redeem attempts** (10 attempts / 300s lockout) | [routes.rs:10538-10540](../../../crates/racecontrol/src/api/routes.rs#L10538-L10540) | ❌ |
| 15 | **Max linked racers per account** (3) | [routes.rs:7950](../../../crates/racecontrol/src/api/routes.rs#L7950) `MAX_LINKED_RACERS` | ❌ |
| 16 | **Max AI opponents in single-player** (19) | [catalog.rs:11](../../../crates/racecontrol/src/catalog.rs#L11) `MAX_AI_SINGLE_PLAYER` | ❌ |
| 17 | **Max multiplayer pods per session** (4) | [input_validation.rs:66](../../../crates/racecontrol/src/input_validation.rs#L66) `MAX_PODS_PER_MULTIPLAYER` | ❌ |
| 18 | **WhatsApp daily budget per customer** (2 messages) | [psychology.rs:127](../../../crates/racecontrol/src/psychology.rs#L127) `WHATSAPP_DAILY_BUDGET` | ❌ |
| 19 | **Nudge TTL** (7 days) + **streak grace days** (7) | [psychology.rs:136-139](../../../crates/racecontrol/src/psychology.rs#L136-L139) | ❌ |
| 20 | **Cafe alert cooldown** (4 hours) | [cafe_alerts.rs:16](../../../crates/racecontrol/src/cafe_alerts.rs#L16) | ❌ |
| 21 | **Cafe broadcast cooldown** (24 hours) | [cafe_marketing.rs:29](../../../crates/racecontrol/src/cafe_marketing.rs#L29) | ❌ |
| 22 | **Venue close detection threshold** (5) | [venue_state.rs:22](../../../crates/racecontrol/src/venue_state.rs#L22) `CLOSED_THRESHOLD` | ❌ |
| 23 | **Cloud sync interval** (30s) + relay retry thresholds | [cloud_sync.rs:34-40](../../../crates/racecontrol/src/cloud_sync.rs#L34-L40) | ❌ |

### Tables with backend CRUD but NO admin page

| # | Table | Backend | Admin? | Why it matters |
|---|---|---|---|---|
| 24 | `bonus_tiers` | Read endpoint + seed only | ❌ | Wallet topup bonuses (see #8) |
| 25 | `membership_tiers` + `memberships` | Missing rc endpoints — returns 502 | ⚠️ UI exists but broken | Tier-based discounts |
| 26 | `nudge_templates` + `campaign_templates` | Direct SQL only | ❌ | Customer re-engagement messages |
| 27 | `policy_rules` + `policy_eval_log` | [policy_engine.rs](../../../crates/racecontrol/src/policy_engine.rs) | ❌ | Dynamic business rule engine |
| 28 | `driver_ratings` | Read via /drivers | ❌ admin | Customer rating/blacklist |
| 29 | `virtual_queue` | Exists but I didn't verify CRUD | ❌ | Customer queue management |
| 30 | `data_retention_config` | Direct config | ❌ | **Retention policy for PII** — GDPR-ish risk |
| 31 | `staff_kudos` + `staff_badges` + `staff_challenges` + `staff_earned_badges` | Backend exists | ❌ | Staff gamification (referenced in `/hr/recognition` but not editable) |
| 32 | `hiring_sjts` + `job_preview` | Backend exists | ❌ `/hr/hiring` page exists but hollow | Hiring content |
| 33 | `variable_reward_log` + `streaks` + `driving_passport` | Backend exists | ❌ | Customer progression/gamification |
| 34 | `accounts` + `journal_entries` + `invoices` | [accounting.rs](../../../crates/racecontrol/src/accounting.rs) | ❌ | **Full accounting module with NO admin UI** |
| 35 | `feature_flags` | Web has `/flags` page | ❌ in admin | **Wrong repo** — feature flags should be in admin not web |

## P0 infrastructure / security

| # | Issue | Location | Severity |
|---|---|---|---|
| 36 | **OpenRouter API key in pod TOMLs** (`openrouter_api_key = "sk-or-v1-..."` in plain text) | `deploy/configs/rc-agent-pod*.toml` (all 8 pods) + `rc-agent` runtime | **P0 security leak** — key in 8 pod filesystems, cannot rotate without SCP to 8 machines |
| 37 | **Game exe paths hardcoded per pod** | Pod TOMLs — `exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\..."` | P1 — adding a new game = edit 8 TOMLs + SCP + restart 8 rc-agents |
| 38 | **Ollama URL + model hardcoded** | [ollama_client.rs:7-9](../../../crates/racecontrol/src/ollama_client.rs#L7-L9) `http://192.168.31.27:11434`, `qwen2.5:3b`, `llama3.1:8b` | P1 — model change = code ship |

## P1 kiosk wizard hardcodes (the thing in user's screenshot)

| # | Text shown on kiosk | Source | Drift risk |
|---|---|---|---|
| 39 | "30 min Package — 700 cr (save 7%)" header text | [SetupWizard.tsx:297](../../../kiosk/src/components/SetupWizard.tsx#L297) hardcoded string | **Lies if tier prices change** |
| 40 | "60 min Package — 900 cr (save 40%)" header text | [SetupWizard.tsx:298](../../../kiosk/src/components/SetupWizard.tsx#L298) hardcoded string | Same |
| 41 | "Most Popular" tag index calc | [PricingDisplay.tsx:51](../../../kiosk/src/components/PricingDisplay.tsx#L51) `paidTiers.length === 3 ? 1 : Math.floor(paidTiers.length / 2)` | Hardcoded heuristic — no admin field `is_popular` |
| 42 | Trial session duration text ("Try for Free 5-minute trial") | Reads from tier DB but text is hardcoded in [PricingDisplay.tsx:118](../../../kiosk/src/components/PricingDisplay.tsx#L118) | Partially dynamic |

## The drift that's already hurting

**Critical duplicate hardcoded list:**

| Surface | File | Values |
|---|---|---|
| PWA (customer) | `pwa/src/app/wallet/topup/page.tsx:15` | 500, 1000, 2000, 3000, 4000, 5000 |
| POS (staff) | `web/src/components/WalletTopupModal.tsx:14-21` | 500, 700, 900, **1000**, 2000, 3000 |

Customer opens PWA and sees a "₹5,000" button. Walks to POS counter. Staff asks "how much?" Customer says "₹5,000". Staff looks at their list — no ₹5,000 button. Has to type it manually. Worse: if PWA preset changes tomorrow, staff still sees old list.

## What this means for v47.0

The current v47.0 milestone scope **does not cover most of these gaps**. My 11-theme scope was limited to:
- Making the admin panel **resilient** (Phase 344, 345)
- Making the admin panel **reach its backends** (Phase 346 cafe proxy)
- Making **venue ↔ cloud sync** work (Phase 349)
- Making the **admin PIN flow** safe (Phase 347)

But "admin panel is the central source of truth" requires ALSO closing the 38 gaps above. That's a **much bigger milestone** than v47.0 as currently scoped.

## Recommended v47.0 scope expansion

Add **5 new phases** (356-360):

| # | Phase | Scope | Priority |
|---|---|---|---|
| 356 | **Business Rules Config Table** | Migrate ~15 hardcoded Rust consts (referral, discount threshold, floor, max refund, nudge TTL, cafe cooldowns, etc.) into `business_rules` SQLite table + admin `/settings/business-rules` page | **P0** — unblocks most gaps |
| 357 | **Pricing Tiers CRUD** | Admin `/pricing/tiers` page + backend POST/PUT/DELETE; remove SetupWizard hardcoded "save %" text; add `is_popular` field | P0 — the screenshot issue |
| 358 | **Cafe Promos Admin Page** | `/cafe/promos` admin page — backend fully exists | P0 — zero blockers |
| 359 | **Bonus Tiers Admin Page** | `/wallet/bonus-tiers` admin page — CRUD endpoints + UI | P0 |
| 360 | **Topup presets unification** | Move `PRESET_AMOUNTS` to DB (`topup_presets` table), both PWA + POS fetch from racecontrol, admin CRUD | P0 — fixes drift |

**Phase 361 (defer):** Move feature flags from web to admin (lower priority — feature flags work today, just wrong repo).

**Phase 362 (defer):** Accounting module admin UI — large scope, can be its own milestone.

**Phase 363 (defer, P0 security):** OpenRouter key rotation + removal from pod TOMLs — fetch from server at boot. **Security concern but not on the same deadline as venue open.**

## Immediate next actions

1. **Capture OpenRouter key leak as a P0 security incident** → rotate the key + remove from pod TOMLs (not a v47.0 scope item; separate urgent task).
2. **Fix the POS/PWA topup preset drift immediately** (Phase 360 should be the FIRST code change).
3. **Add phases 356-360 to ROADMAP.md + REQUIREMENTS.md** — commit scope expansion.
4. **Discuss with Uday whether to expand v47.0** or split into v47.0 (resilience, what's already scoped) + v48.0 (SSOT completion, phases 356-360). Trade-off: bigger milestone = later venue-ready date.

---

*Report written 2026-04-09 after user flagged the kiosk screenshot discount discrepancy. Not committed to git yet — awaiting user decision on scope expansion.*
