# Venue Infrastructure — Canonical Doctrine (RacingPoint / RaceControl)

> **Status:** RATIFIED in chat (Captain, 2026-07-03/04). Records the venue-as-a-product architecture for **own + sold** venues so it is not lost. Companion to `venue-registry.json` (data) + `VENUE-NODE-ROLE-TAXONOMY.md` (node roles). Execution plan: bono `/root/.claude/plans/silly-snuggling-meadow.md`.
> **Grounding note:** claims here are code-cited where they assert current behavior; "to build" items are explicitly marked. Verify against code before asserting as fact (per Rule 0 / capability-claim discipline).

## 0. First principles (Captain-locked)
1. **V3 is the SOLE go-forward stack for EVERY venue** (own + sold, present + future). **V2 exists ONLY at `rp-vlm` and is being retired there** (the W5 uninstall). No other venue — present or future — runs V2; all greenfield are V3-only. The transient V3-alongside-legacy-V2 co-install at rp-vlm is a **one-time cutover mechanism, NOT a venue-infrastructure property.**
2. **Cloud is out of the live money path.** The public cloud PWA is a **read-only** reflection of the operator-scoped reconcile replica; **live top-up + sessions happen in-venue on the LAN** (the edge is the money authority). Aligns with V3-as-built: `rc-edge` dials cloud one-way `POST /flush`; `rc-cloud` holds only an operator-scoped, RLS-isolated replica.
3. **Own venues are permanently entitled.** `venue_type == "own"` (the `rp-` prefix) ⇒ **never license-suspended or cut off**. The entitlement/kill-switch applies to **sold** operators only. Invariant — no entitlement/suspension path may gate an `rp-*` venue. *(Currently NOT enforced in code — a build item.)*
4. **Sold to other venues is a SUBSCRIPTION** (monthly or annual). Maps onto the existing `SubscriptionSet { rate, cycle }` / `BillingCycle {Monthly, Annual}` (`rc-contract/src/distribution.rs:97`).
5. **Installation must be as easy/turnkey as possible** — HQ-driven, minimal operator effort; KPI = time-to-first-INR at a new venue.

## 1. Domains (two-domain doctrine)
Ratified `comms-link/CAPTAIN-RATIFY-RACECONTROL-STRUCTURE-V1-2026-06-01.md:30`.
- `console.racecontrol.in` — HQ / platform-owner control panel (LIVE).
- `app.racingpoint.cloud` — **own** operator (RacingPoint); all `rp-*` venues.
- `<operator>.racecontrol.in` — **sold** operators (e.g. `apex.racecontrol.in`), **one subdomain per OPERATOR** (not per venue), covering all that operator's venues.

## 2. Tenancy
- `operator_id` = the isolation boundary (RLS in `rc-cloud`; `rc-cloud/src/lib.rs:5-13`).
- `venue_id` (e.g. `rp-vlm`, `apex-chennai`) + `branch_id` = sub-keys within an operator.
- `operator_slug` (e.g. `apex`) = the public subdomain label. Regex `^[a-z][a-z0-9-]{1,30}$` (DNS-safe). Venue `apex-chennai` → operator `apex` → `apex.racecontrol.in`.

## 3. Two serving planes (never conflate)
- **In-venue (LAN, live money):** `rc-edge` (authority, loopback `127.0.0.1:8431`) + `rc-gateway` (`:8432` loopback / `0.0.0.0:443` TLS — sole HMAC signer + reverse proxy + cockpit static). Serves POS/launcher/pod-display **and** the live customer PWA-BFF (`rc-edge/src/pwa.rs`) to in-hall devices. Built (PRs #88/#89/#90). TLS = per-venue internal CA (`scripts/venue/gen-venue-tls.sh`).
- **Cloud (public, read-only):** the per-operator subdomain serves a PWA that reads balance/history from that operator's reconcile replica; a clear "top-up / play in venue" affordance covers live actions. Cloud never reaches into a venue.

### 3.1 Venue surfaces (in-hall) — the full serving layer, all gateway-served on the LAN
Every in-hall UI is served by **rc-gateway** under `/app/<surface>` on the venue LAN (TLS `:443` / plain `:8432`), same-origin with `/gw/*` + the proxied bus/REST. The customer PWA is **one** of these surfaces, not the whole story.

| Venue surface | Persona | Served at (in-hall) | Backend | Money-path? |
|---|---|---|---|---|
| **POS / Reception** | Staff | gateway `/app/pos` | `rc-edge/src/pos.rs` (`/pos/*`) | **Live** (top-up, sign-in) |
| **Game Launcher** | Staff | gateway `/app/launcher` | `rc-edge/src/launcher.rs` + `/heart/*` | **Live** (green-light = the only billing trigger) |
| **Pod Display (glass)** | Customer (per pod) | gateway `/app/pod?pod_id=` | `rc-edge/src/pod.rs` `/pod/:seat/glass` + `session` bus | Read-only (time/spend; anti-cheat) |
| **Chef / Kitchen Display** | Staff | gateway `/app/chef` | `KitchenTicket` bus `/bus/kitchen/*` | No (money never rides this channel) |
| **Admin Panel** | Venue-Operator | gateway `/app/admin` | **sole writer of `VenueConfig`** `/bus/config` | No (config; money `bounds` are Captain-PIN) |
| **Leaderboard / Spectator board** | Customer / spectator | gateway `/app/leaderboard` (or board display) | derived-on-read from `laps_event` | Read-only |
| **Kiosk** | Customer | gateway `/app/kiosk` | register / wallet-view | Read + staff-mediated |
| **Venue Portal** | Operator / staff (front door) | heart-served hub (`/portal`) | links to the surfaces above | No |
| **Customer PWA (in-venue)** | Customer | gateway-served; live wallet/session | `rc-edge/src/pwa.rs` (edge PWA-BFF) | **Live** in-venue (cloud copy is read-only) |

Live-money surfaces (POS, Launcher, in-venue PWA) hit the **edge authority**; read-only/notification surfaces (Pod-Display, Leaderboard, Chef) ride the **bus** (`/bus/events` SSE). The exact bundle layout for `/app/<surface>` (3 SPAs vs 1, base path `/app/<surface>/`) is the pending **W1(b)** Design bundle-layout answer.

## 4. Per-venue node inventory (V3 target)
Roles are canonical per `VENUE-NODE-ROLE-TAXONOMY.md`. rp-vlm = instance #1.

| Node role | Count/venue | Runs (V3) | Ports |
|---|---|---|---|
| **venue_heart** | 1 | rc-edge (money authority) + rc-gateway (signer/proxy/cockpit static) + Postgres `rc-pg16` (append-only durable store) + `RCV3Watchdog` (2-min respawn; 60-min `MAINTENANCE_MODE` TTL) | rc-edge `127.0.0.1:8431` · gateway `:8432` loopback / `:443` TLS · PG `127.0.0.1:5433` — **only the gateway LAN port is exposed** |
| **pod** | N (8 at rp-vlm) | pod agent → edge `/agent/*` (WS transport over the HTTP message model) | `:8090` agent · `:8091` rc-sentry |
| **pos_terminal** | 1+ | browser → gateway `/app/pos` | client |
| **control_node** | 1 (off-venue) | runs `install-venue.sh`/`deploy-venue.sh`/`pod-mgmt`; reaches pods per `pod_transport` | SSH/relay client |

- **Network layers:** venue LAN `192.168.31.0/24` · Tailscale `100.x` (**own only**) · WAN (heart dials out — V3 edge→cloud flush) · cloud (VPS).
- **Own vs sold delta:** own = Tailscale + `tailscale-ssh` pod transport + installer **provisions** OpenSSH; sold = **no Tailscale** + `heart-exec` (control_node → heart `pod_exec` → rc-sentry → pod, command-filtered) + installer **removes** OpenSSH. Default `VenueType = Sold`.
- **Pod access (LOCKED):** sold pods = **heart-exec, NO direct pod SSH**; the heart is the LAN jump-host to pods, so HQ only ever needs a path to the *heart*.

## 5. Capacity / storage (measured, per venue)
Anchor = rp-vlm's real 3-month DB: money+customer data = **~0.5 MB / 3 months** (wallets 144, txns 1,853, drivers 251). **Raw telemetry is NOT persisted** (adapter emits one ~0.4 KB lap-record/lap only). A V3 venue's data store is **low single-digit GB/year** — a 128 GB+ heart SSD is ~100× over-provisioned for data. The real disk consumer is **game content on the pods** (tens–hundreds of GB/pod), a pod line-item separate from the heart. **The money ledger is the crown jewel — back it up** (backup/DR is a current gap).

## 6. Services by persona
- **Customer:** PWA (register/OTP · wallet · top-up · café order · history · claim walk-in · ≤4 shared drivers) · Kiosk · Pod-Display glass (read-only, anti-cheat) · Leaderboard (read-only) · Café order.
- **Staff:** POS/Reception (sign-in · top-up · bounded refund · café · reconcile · walk-in) · Game Launcher (**green-light = the only billing trigger**) · Chef/Kitchen display (money never rides this channel) · Staff console (create-staff, PIN) · session lifecycle (crash/resume are billing-free) · refund/adjust (Captain-PIN over `bounds`).
- **Venue-Operator:** **Admin Panel** = sole writer of `VenueConfig` (rate · GST · café · branding · PWA modules · fleet · money `bounds`) · Venue Portal · **PlatformGrants reflection** (read-only; can toggle a licensed feature, never self-grant) · UpstreamRequest · sync status.
- **HQ / Platform-Owner (RaceControl SaaS):** `rc-cloud/src/bff.rs` `/control/*` — register operators, provision venues, grant entitlement, MI-spend + fleet-health per operator. HQ-key-gated, fail-closed. Where entitlement/suspension for sold operators + the `rp-*` exemption live.

## 7. Connection topology (V3 target)
```
in-venue browser → rc-gateway (:443 TLS / :8432; sole LAN-facing node, sole HMAC signer)
   ├ /app/*  → cockpit static (no auth)
   ├ /gw/login,/gw/select-customer → verify PIN at edge → mint TTL Bearer (Model A)
   └ fallback proxy → authenticate → sign HMAC + inject identity → rc-edge (127.0.0.1:8431)
        rc-edge (venue money/session/identity authority) → postgres rc_app@127.0.0.1:5433 (append-only)
        /bus/events SSE (config·platform·kitchen·session), streamed, bounded by token exp
   pod agent → rc-edge /agent/{confirm,crash,last-signal,link}
venue→cloud: rc-edge flush → POST /flush (HMAC, ~30s, opt-in) → rc-cloud  [one-way UP; already-applied money]
cloud:       rc-cloud = operator-scoped RLS reconcile replica + HQ /control/* ; console.racecontrol.in ; per-operator read-only PWA domain
```
**Venue-wins on reconnect** (append-only ledgers + monotonic venue seq + F5 audit; merge by txn-id — `SYNC-CONTRACT.md §5-6`).

## 8. Commercial + legal
- **Sold billing = subscription (monthly/annual)** — `SubscriptionSet`/`BillingCycle`. To build: set-rate route + renewal event (sets `paid_through`) + PSP collection + invoicing/dunning. **Open:** rate/tiers + which PSP collects.
- **Customer top-up PSP + merchant-account ownership — UNDEFINED** (real ₹ flows out-of-band today). Open: for a sold venue, operator's vs RacingPoint's merchant account.
- **Legal:** GST (SAC 9996 gaming @18%, SAC 9963 café @5% — mostly built) + **DPDP Act 2023 / Rules 2025 — Data Fiduciary** (documented in `docs/LEGAL-TAX-OPERATIONS-GUIDE.md`; machinery — consent, data-subject rights incl. erasure, retention/purge, grievance officer, breach notification, children's consent — to build). **Open:** per-operator Data-Fiduciary allocation for sold venues (ties to merchant-account).

## 9. Known gaps (elimination pass, 2026-07-03; evidence in the plan file)
Must-build infra: **money-ledger backup/DR (CRITICAL)** · NTP/time-sync · outbound V3 alerting · per-venue OTP provisioning · OTA/update channel · game-content sync + sim-license inventory · hardware BOM. Captain-decisions: top-up PSP + merchant-ownership · subscription rate/tiers + collecting PSP · non-payment degraded mode · MI-spend enforcement · Data-Fiduciary allocation · support/SLA/offboarding.

## 10. Install-ease (turnkey goal)
HQ-driven over the reverse tunnel; one operator intake form → derived config; auto-preflight; idempotent+resumable provisioning w/ auto-rollback (`install-venue.sh`/`deploy-venue.sh`); automated pod enrollment; the P1–P10 acceptance suite as the single green=done gate. KPI = time-to-first-INR at a new venue. Operator-facing wizard → Claude Code Design (bono authors the brief).
