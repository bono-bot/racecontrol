# Admin Panel as Source of Truth — Milestone Spec

**Created:** 2026-04-22 04:52 IST by James
**Status:** SPEC ONLY — no code changes committed against these items yet
**Driver:** Uday requested "admin is the truth on which Billing and .23 Kiosk operate"

## Goal

Every operator-configurable knob that Billing and the .23 BillingDashboard kiosk consume must be edited in admin and propagated without a code deploy. Today, critical rates and timers are hardcoded in Rust source, and the .23 kiosk has no config-push channel.

## Current state (verified 2026-04-22)

Closed in same session:
- `/memberships` + `/wallet-transactions` un-hidden in sidebar (admin commit `3ec0529`).
- Coupon create form now has client-side validation (code shape, value ranges, future valid_until).

Build: `npm run build` exit 0 against admin HEAD `3ec0529` with unhide wired into sidebar — routes `/memberships` and `/wallet-transactions` both appear in the Next.js route table.

Open items captured below as P1-P5. Each requires backend Rust work + MMA audit per `Subagent Gates` and `MMA audit MANDATORY before deploying new cross-system bridges` rules in racecontrol/CLAUDE.md.

---

## P1 — Centralise billing-rate constants

**Problem.** The per-minute rate `2500` paise, snap boundary `70000` paise @ 30 min, and snap boundary `90000` paise @ 60 min are hardcoded in five places:
- `crates/racecontrol/src/billing.rs:168,239`
- `crates/racecontrol/src/billing_orphan.rs:92`
- `crates/racecontrol/src/billing_pricing.rs:128,179-181,242,258`

Changing a rate currently requires a global find-replace + server binary redeploy + deploy parity to cloud. Admin `/billing/rates` page writes to the `billing_rates` DB table, but the snap pricing path reads constants from code, not DB.

**Fix.** Introduce `BillingConstants` struct loaded from `billing_rates` DB (or a new `billing_constants` KV table) at start_billing transaction time. Replace all hardcoded call sites with `BillingConstants::load(&tx).await`. Admin `/billing/rates` already writes the DB rows — the struct just reads them.

**Risk.** Touches the billing path. Blast radius = server .23 + 8 pods + cloud racecontrol. Changing a constant mid-session is the primary failure mode (session already priced at old rate, new rate appears on next tick).

**Gates.** MMA mandatory (cross-system bridge). Billing session drain per `Billing sessions must drain before binary swap`. Deploy parity. FATM unit tests must stay green.

**Effort.** 1 day code + 1 day MMA + 1 day deploy = 3 days.

---

## P2 — Idle threshold as fleet config

**Problem.** `rc-agent/src/driving_detector.rs:28` hardcodes `idle_threshold: Duration::from_secs(10)`. Changing requires rc-agent rebuild + fleet redeploy. `billing_tests.rs:1426` references a separate `idle_threshold_secs = 300` (5-min drift threshold); the two are unrelated but easy to confuse.

**Fix.** Move idle threshold to `AgentConfig` (already delivered to pods via `config_push_full`). Admin gains a knob on `/fleet/config` or a new `/settings/billing` page. Default 10s, range 5-60s.

**Risk.** Pod-side config; affects billing timer behaviour. A too-low value double-charges bathroom breaks; too-high lets customers idle on paid time.

**Gates.** MMA mandatory. First-run check after enabling new value on Pod 8 canary. Fleet-wide rollout per standing rule.

**Effort.** 0.5 day code + 1 day MMA + 0.5 day fleet deploy = 2 days.

---

## P3 — Referrals admin page

**Problem.** `crates/racecontrol/src/api/customer_referral.rs` ships three endpoints (`customer_referral_code`, `customer_generate_referral_code`, `customer_redeem_referral`), but no admin route exposes referrals for listing, tuning reward rules, or auditing commission. Reward crediting logic runs silently in session-end hooks.

**Fix.** Two parts:
1. **Backend (Rust)** — add admin routes under `admin_marketing.rs` (new file): `GET /admin/referrals` (list with pagination + filters), `GET /admin/referrals/stats` (per-referrer aggregates), `POST /admin/referral-rules` (create/edit commission rules), `PATCH /admin/referrals/:id` (mark credited/revoke). New table `referral_rules(id, tier, reward_paise, min_referrer_sessions, active, created_at)`.
2. **Admin UI** — new page `/referrals` with tabs: Recent, Stats, Rules. List component mirrors `/coupons` pattern. Rules tab uses the same form-validator pattern just added to `/coupons`.

**Risk.** Admin-only backend endpoints; does not touch billing hot path. Reward crediting logic in `billing_hooks.rs` must read rules from DB instead of any hardcoded value — verify before/after behaviour.

**Gates.** MMA recommended (first-admin-route in `customer_marketing` module). Deploy parity (cloud admin + cloud racecontrol).

**Effort.** 2 days backend + 1.5 days admin + 1 day MMA + 0.5 day deploy = 5 days.

---

## P4 — Kiosk config-push channel

**Problem.** `config_push.rs:1-9` routes `/api/v1/config/push` and `/api/v1/config/pod/{pod_id}` only. There is no endpoint the .23 BillingDashboard kiosk can fetch or subscribe to. Kiosk polling interval, dashboard URL, and refresh rate are all hardcoded in the Next.js kiosk build. Admin-edited config cannot reach the kiosk without a rebuild + SCP.

**Fix.**
1. Backend: `POST /api/v1/config/kiosk` (staff JWT) + `GET /api/v1/config/kiosk` (kiosk service key). New table `kiosk_config(key, value, updated_at, updated_by)`. WS channel `KioskConfigPush` broadcast on change (same pattern as `ConfigPushSent`).
2. Kiosk (.23): Next.js app gains a `useKioskConfig()` hook that opens WS to racecontrol + falls back to polling `/api/v1/config/kiosk` every 30s. Current hardcoded intervals (`fetch('/fleet/health')` cadence, etc.) read from the hook.
3. Admin: new page `/settings/kiosk` with fields for refresh_ms, dashboard_url, theme, alert_volume, and a Preview button that pushes config and reads it back.

**Risk.** The .23 kiosk is customer-facing. A bad config push could blank or freeze it mid-race. Need a safe-mode fallback (if config load fails, use last-good cached values).

**Gates.** MMA MANDATORY (cross-system bridge: admin → racecontrol → .23 kiosk). Pre-flight smoke test before accepting any config. UI-SPEC.md + UI-REVIEW.md per `Subagent Gates`.

**Effort.** 2 days backend + 2 days kiosk + 2 days admin + 1 day MMA + 1 day deploy = 8 days.

---

## P5 — Hook toggles and templates

**Problem.** `billing_hooks.rs` fires session-end side-effects (WhatsApp receipt, PDF generation, referral credit, review nudge) in fire-and-forget mode with hardcoded message templates. Operators cannot disable a hook per season/region or edit message text without a code deploy. Failures are silent — no retry, no admin alert.

**Fix.** New table `hooks_config(name, enabled, template_text, retry_count, fallback_channel)`. Admin `/settings/hooks` page: toggle list + template editor + fallback selector + preview. `billing_hooks.rs` reads the table at each fire, logs failures to a new `hook_failures` table surfaced at `/settings/hooks#failures`.

**Risk.** Receipt WhatsApp and referral credit are customer-facing. Template changes that break rendering cause receipt delivery failure; new retry logic must not DDoS Evolution API.

**Gates.** MMA recommended (new cross-system bridge for retry/fallback). UI-SPEC.md for the admin settings page.

**Effort.** 2 days backend + 1.5 days admin + 1 day MMA + 0.5 day deploy = 5 days.

---

## Order of execution (recommended)

1. P5 first — lowest billing-path risk, highest customer trust gain (receipts stop silently failing).
2. P3 — admin-only routes, no billing impact, unblocks marketing team.
3. P2 — small surface, good MMA practice, single rc-agent deploy cycle.
4. P1 — central billing change, needs careful MMA + session drain.
5. P4 — largest cross-system change, schedule after P1 lessons.

## Not in scope

- Calendar CRUD (touches Gateway + Google Calendar API — separate phase).
- Packages CRUD (needs customer-side `packages` table admin routes — separate phase).
- Per-minute pricing tier overrides via admin (tier edit UI exists at `/pricing/tiers`; verify it writes and takes effect before assuming gap).

## Deploy parity note

All P-items must follow the DEPLOY PARITY rule in racecontrol/CLAUDE.md: any local (server .23) change requires cloud (Bono VPS) parity deploy in the same session. This spec assumes standard deploy sequence, not ad-hoc SCP.
