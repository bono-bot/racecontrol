# OTP Evolution→MSG91 — V1 Rust-layer retire & convergence plan (cutover-gated)

**Status:** PLAN (cutover-gated). Companion to the ratified §S-146 RCA
`.planning/specs/v2/RCA/otp-msg91-s146-20260602.md` (MMA §A7 consensus) and its
Mechanism-Trust-Check `.planning/specs/v2/MECHANISM-TRUST/msg91-otp-20260602.json`.
**Author:** bono (sole pilot §S-448), 2026-06-02. **Lane:** racecontrol bono-sole.

## Why this document exists

RCA finding **C1** ("A2.6 convergence is incomplete unless V1 is *deleted*, not
bypassed") and **C2** ("guardian path + billing-start gate blocks first-INR") are
both **cutover-gated** — they cannot be executed pre-cutover without breaking the
*live* Evolution OTP path. §S-146 RCA §5 requires that any temporary V1 retention
carry "an explicit follow-up trigger condition that retires the V1 path." **This
file is that trigger condition**, with the boundary map and sequence verified
against the live tree this session (not memory-projected).

What landed *now* (this PR), because it is cutover-independent:
- **A2.7** — constant-time daily-debug-PIN compare (`auth::ct_pin_eq`) across all
  **3** call sites (`token_consume.rs::validate_pin`, `auth::validate_employee_pin`,
  `auth::validate_employee_pin_kiosk`) + unit tests. RCA only named 1 site; the
  other 2 were found by enumeration (H4) and fixed under the Cascade-Updates rule.

What this file plans (executed AT cutover, under its own per-PR Captain auth):
- **C1** — hard-delete the V1 Rust OTP verifier + columns + Evolution sender.
- **C2** — guardian-OTP migration + the (verified) no-op on the billing-start gate.

---

## Boundary map — verified against the tree 2026-06-02 (re-verify line nums at cutover)

| Surface | Path | Role | Shared? |
|---|---|---|---|
| Customer OTP send | `auth/otp.rs::send_otp` (~:52), `resend_otp` (~:178), `generate_and_store_otp` (~:220) | writes `drivers.otp_code` (argon2 **hash**, not plaintext) + `otp_expires_at`; 30s-reuse window | customer-only |
| Customer OTP verify | `auth/otp.rs::verify_otp` (~:239) | reads `drivers.otp_code`, argon2 constant-time verify, issues JWT + `customer_sessions` row | customer-only |
| OTP hasher | `auth/otp.rs::hash_otp` (~:17), `verify_otp_hash` (~:33) | argon2id | **SHARED** customer + guardian |
| WhatsApp sender | `auth/otp.rs::send_otp_whatsapp` (~:126) | Evolution `POST /message/sendText/{instance}`; reads `config.auth.evolution_url/_api_key/_instance` | **SHARED** customer + guardian |
| Guardian OTP send | `auth/otp.rs::send_guardian_otp` (~:311) | writes `drivers.guardian_otp_code` + `guardian_otp_expires_at`; resets `guardian_otp_verified=0`; calls the SHARED `send_otp_whatsapp` | guardian-only |
| Guardian OTP verify | `auth/otp.rs::verify_guardian_otp` (~:366) | argon2 verify → sets `drivers.guardian_otp_verified=1` + `guardian_otp_verified_at` | guardian-only |
| Billing-start minor gate | `api/billing_start_validate.rs::lookup_and_validate_driver` (~:216,:266) | reads `guardian_otp_verified` (**provider-agnostic boolean**); rejects only `if is_minor` | reader |
| Guardian send caller | `api/pricing_routes.rs` (`send_guardian_otp` / `verify_guardian_otp` route handlers) | HTTP surface | — |
| `drivers.otp_code` consumers | `cloud_sync_upsert.rs`, `api/routes_handlers.rs`, `auth/otp.rs`, `db/migrate_core.rs` | column read/write/sync/schema | cascade |
| Downstream (UNCHANGED) | otp-issued HMAC bridge → Server `internal_auth.rs` → Server `otp_state` → `createHousehold` | transport-agnostic | not modified |

**Critical shared-dependency fact:** `send_otp_whatsapp` (the Evolution egress) and
`hash_otp`/`verify_otp_hash` (argon2) are **shared by BOTH the customer and guardian
OTP paths**. Deleting them while Evolution is the live provider breaks guardian OTP
(minor consent) as well as customer OTP. This is the hard sequencing constraint.

---

## C2 — guardian path & billing-start gate

### Correction to the RCA's C2 framing (verified this session)
The RCA/MMA stated the billing-start gate "reads `drivers.otp_code`". It does **not**.
`lookup_and_validate_driver` reads `drivers.guardian_otp_verified` — a
**provider-agnostic boolean** — and the minor branch fires **only `if is_minor`**
(`billing_start_validate.rs:265`). Consequences:
1. **The billing-start check itself needs NO change** to support MSG91 — it already
   keys off a provider-neutral flag. There is nothing to "repoint" on the read side.
2. **The literal first adult INR is unaffected** — for `is_minor == false` the
   guardian gate is bypassed entirely. C2 blocks *minor-customer* billing, which is
   gated behind (and therefore not a blocker for) the first adult INR.

### What actually has to migrate (cutover-coupled, ENGINEERING-IN-FLIGHT)
The thing that *sets* `guardian_otp_verified=1` today is the Evolution-backed Rust
`send_guardian_otp` → `verify_guardian_otp`. Under MSG91 that flow must be replaced
so the boolean still gets set. Two candidate designs (decide at cutover via MMA PLAN):
- **(a) Mode-B server-side SendOTP** (RCA A5 Mode B): bono initiates guardian OTP via
  MSG91 `MSG91_AUTHKEY` server-side, verifies, then sets `drivers.guardian_otp_verified`
  in the racecontrol DB via a guardian-verified signal over the existing bridge.
- **(b) Widget-relay**: guardian verifies via the MSG91 widget in the registration UI;
  the verified-guardian fact propagates to racecontrol `drivers` the same way customer
  identity does.
**Substrate gap:** there is currently **no cross-layer write path** that lets a
bono-side (MSG91) guardian verification set `drivers.guardian_otp_verified` in the
racecontrol heart DB. Designing that path is the real C2 engineering and is cutover-
coupled. → classify **ENGINEERING-IN-FLIGHT (substrate-missing)** until cutover.

---

## C1 — delete-not-bypass (V1 hard-removal)

**Why not now:** C1 targets a *"present-but-unused"* verifier. The Rust OTP verifier
is **not unused** while `RP_OTP_PROVIDER` defaults to `evolution` and MSG91 is
unprovisioned. Deleting now violates RCA **C7** (drain Evolution ≥ max-TTL before the
flip) and breaks the live customer + guardian OTP paths via the shared sender/hasher.

**Delete set (at cutover), in dependency order:**
1. **Confirm the Rust customer-OTP routes are dead under V2** — enumerate every caller
   of `send_otp`/`verify_otp`/`resend_otp` (kiosk, PWA, POS, cloud). V2 customer OTP
   runs through the bono admin-proxy MSG91 widget; the Rust customer routes should have
   zero live callers post-cutover. **Do not delete without this enumeration** (Rule 0).
2. **Migrate guardian OTP off Evolution** (C2 above) so the shared `send_otp_whatsapp`
   has no remaining caller.
3. **Drop columns** `drivers.otp_code`, `otp_expires_at` and `drivers.guardian_otp_code`,
   `guardian_otp_expires_at` — coordinated **venue + cloud** SQLite migration (DEPLOY
   PARITY). Update the `drivers.otp_code` consumers: `cloud_sync_upsert.rs` and
   `api/routes_handlers.rs` in the SAME migration (Cascade-Updates rule).
4. **Remove** `send_otp_whatsapp` + the `config.auth.evolution_url/_api_key/_instance`
   keys (closes inherited-issue A2.4 server-direct egress + A2.3 committed-creds class).
5. **Remove** `hash_otp`/`verify_otp_hash` only after confirming no other consumer.
6. Keep `guardian_otp_verified` + `guardian_otp_verified_at` (the provider-agnostic
   flag the billing gate reads) — these survive; only the Evolution *producer* changes.

**Class:** schema + protocol change, multi-consumer, cross-environment → full §S-146
(not the §S-186 small-fix fast-lane) + MMA PLAN (Step 2) at cutover + per-PR Captain
named-surface merge auth + DEPLOY PARITY (venue .23 + Bono VPS cloud).

---

## Retire-trigger (the §S-146 §5 condition) & cutover sequence

Execute C1+C2 when **ALL** hold (composes with RCA C7 epoch-cutover):
1. MSG91 provisioned (creds + India DLT live) — **Captain/operator item #5**.
2. `verifyAccessToken` endpoint/shape captured from the MSG91 dashboard (MTC Q3) and
   the customer-OTP MSG91 path verified on a real pod (the Steps 3–4 bono-admin-proxy
   wiring exercised end-to-end).
3. Guardian-OTP cross-layer set-path (C2 design a or b) built + tested.
4. `RP_OTP_PROVIDER=msg91` cutover performed by session generation-mode/epoch (C7),
   Evolution drained ≥ max-TTL, rollback path staged (purge MSG91 `otp_state` by ts).
5. Customer-OTP Rust-route caller enumeration returns zero live callers.

Until all 5 hold, the V1 Rust OTP path is **retained intentionally** as the live
interim provider (kaizen-correct V1 retention per RCA §5), and A2.7 is the only
hardening applied to it.

## Out of scope here
- **A2.7** — done in this PR (constant-time PIN compare).
- **Secret rotation (item #2)** — Captain/operator; closes the A2.7 *forgeability*
  half (leaked `jwt_secret` → forgeable daily PIN) that a constant-time compare does
  not address.
- **MSG91 provisioning (item #5)** — Captain/operator.
- **Mode-B server SendOTP + OTP step-up** on the privileged-mutation set — RCA A6
  fast-follow, post-first-INR.

## Related finding (surfaced, not fixed here)
The employee-debug-PIN path in `validate_pin` runs **before** the customer-lockout
gate and has **no lockout threshold** on the staff path — so beyond the timing leak
(closed by A2.7), the 4-digit daily PIN is brute-forceable unthrottled (~10⁴ guesses).
A throttle on the staff-PIN path is a separate small hardening; logged here so it
isn't lost. Not bundled into A2.7 to keep that change minimal/reversible.
