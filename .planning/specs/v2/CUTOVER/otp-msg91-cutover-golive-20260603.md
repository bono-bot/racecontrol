# RUNBOOK — OTP Evolution→MSG91 GO-LIVE cutover (venue)

- **Author:** bono · **Date:** 2026-06-03 IST · **Status:** DRAFT for Captain/operator review.
- **Scope:** turn ON MSG91 SMS OTP for the **customer** login/registration money-path at the venue (first-INR `register(OTP)→createHousehold`). 
- **NOT in scope:** the V1 Rust-layer *retire* (delete Evolution verifier/columns) — that's the companion `racecontrol/.planning/specs/v2/CUTOVER/otp-msg91-v1-retire-20260602.md`, run *after* go-live is stable (drain Evolution first). Guardian/minor-OTP is **post-first-adult-INR** (own RCA + DLT).
- **Reversibility:** every step is flag-driven. Rollback = flip flags back; the Evolution path is byte-identical and untouched. **No code rollback needed.**
- **Grounding:** flags/behaviours below verified against the live tree this session (paths cited). Items marked **[CONFIRM]** are operator/panel facts I could not verify from code — confirm before relying.

---

## 0. Preconditions (must ALL be true before any flag flips)

| # | Precondition | Owner | Verify |
|---|---|---|---|
| P1 | **PR #27** merged (customer-OTP contract F1/F2/F3) | Captain | `gh pr view 27` → MERGED |
| P2 | **PR #28** merged (verify atomic single-use G-CUT-2) | Captain | `gh pr view 28` → MERGED |
| P3 | Bridge **Rule-#5 / CD-20 sync-ratify** done (AUTH-MEMO-1) | Captain | the net-new `/internal/auth/otp-issued` identity surface is ratified |
| P4 | PWA adopts canonical `VerifyOtpWidgetRequest` (cleanup; non-blocking — widget path already works via the interim local schema) | James | `apps/pwa/lib/api.ts` imports the contract variant |
| P5 | Contracts + admin-proxy-bono + admin-proxy-james rebuilt/redeployed from the merged code to **both** Server .23 and Bono VPS | operator | build_id parity on both |

---

## 1. Provisioning checklist (operator — the REAL venue gate; do FIRST, has lead time)

Set as **env-managed secrets** (never hardcoded; never logged — the MSG91 client refuses to log authkey/token/URL/phone, msg91-api-client.ts:24).

| # | Item | Env var | Where | Verify | Status |
|---|---|---|---|---|---|
| V1 | MSG91 server auth key | `MSG91_AUTHKEY` | admin-proxy-bono env (Server .23 + Bono VPS) | a test `verifyAccessToken` returns a phone, not a vendor error | **[CONFIRM]** |
| V2 | MSG91 verify endpoint URL | `MSG91_VERIFY_URL` | same | **[CONFIRM]** exact path from the MSG91 panel — do NOT guess (msg91-api-client.ts:12,19) | **[CONFIRM]** |
| V3 | Live success-response shape | — | — | confirm MSG91 returns `{type, message}` with the verified mobile in `message` (the parser tolerates this; msg91-api-client.ts:20-21). If the panel uses a different shape, the parser needs a tweak BEFORE go-live | **[CONFIRM]** |
| V4 | TRAI **DLT** template + sender | `MSG91_TEMPLATE_ID` / `MSG91_SENDER` **[CONFIRM names]** | MSG91 panel + DLT portal | DLT approval can take days — **critical path**. Needed for SMS deliverability (and later Guardian) | **[CONFIRM]** |
| V5 | MSG91 IP allowlist | — | MSG91 panel | allowlist `49.204.31.37`, `72.60.101.58` **if** enforced | **[CONFIRM]** |
| V6 | Bridge HMAC shared secret | `WG_INTERNAL_HMAC_SECRET` | **same value** on admin-proxy-bono (.23 + VPS) AND admin-proxy-james (.23), via rc-installer/secret file | `GET /internal/health` envelope round-trip returns 200 from bono→james | **[CONFIRM]** |
| V7 | PWA widget id / tokenAuth | `NEXT_PUBLIC_*` (per MSG91 widget) | `apps/pwa/.env.local` | James already dropped these | done per James |

> **Stop-gate:** if V1–V3 or V6 are not confirmed, the customer verify or the bridge will fail. Do not flip flags until they pass an isolated probe.

---

## 2. Flag matrix (verified — know exactly what each does)

| Flag | Default | Side | Controls | Source |
|---|---|---|---|---|
| `RP_OTP_PROVIDER` | `evolution` | admin-proxy-bono | which provider `requestOtp` mints (`msg91` ⇒ SMS widget handle, no code) | forwarders.ts:89-90 |
| `RP_OTP_BRIDGE_ENABLED` | `off` | admin-proxy-bono (**sender**) | whether bono PUSHes the issued token to James (`POST /internal/auth/otp-issued`) | forwarders.ts:52,379 |
| `RP_OTP_BRIDGE_COMMIT` | prod `off` (stages), sandbox on | admin-proxy-james (**receiver**) | whether James PERSISTs the pushed row (vs verify-and-stage) | server.ts:1814-1817 |
| `NEXT_PUBLIC_OTP_PROVIDER` | (build-time) | PWA | which client widget the browser renders | PWA build |

**Load-bearing coupling (forwarders.ts:431-433, verified):** `createHousehold` is `OtpVerifyToken`-bound and James validates it via `consumeOtpToken`. If bono doesn't push (`RP_OTP_BRIDGE_ENABLED=off`) **or** James doesn't persist (`RP_OTP_BRIDGE_COMMIT=off`), James never has the token ⇒ **`createHousehold` 401s**. A live household-creating login therefore needs **BOTH** `RP_OTP_BRIDGE_ENABLED=on` (bono) **AND** `RP_OTP_BRIDGE_COMMIT=on` (james).

---

## 3. Flag-flip ORDER (sequence matters — bridge before provider)

Flip in this order so there is never a window where a customer verifies but `createHousehold` 401s:

1. **James first** — `RP_OTP_BRIDGE_COMMIT=on` on admin-proxy-james (Server .23). Reload service. (James now persists any pushed token.)
2. **Bono next** — `RP_OTP_BRIDGE_ENABLED=on` on admin-proxy-bono (**both** .23 + Bono VPS). Reload. (Bono now pushes; James already persists ⇒ `createHousehold` will succeed.)
3. **Verify the bridge live** (§5.A) BEFORE switching the provider — prove the token round-trips while still on Evolution.
4. **Provider flip** — `RP_OTP_PROVIDER=msg91` on admin-proxy-bono (both). Reload. (New requests now mint SMS widget handles.)
5. **PWA build flag** — rebuild + deploy PWA with `NEXT_PUBLIC_OTP_PROVIDER=msg91` (both environments). (Browser now renders the MSG91 widget.)

> Steps 1–2 are reversible no-ops on the customer path (Evolution still serving). Step 4 is the live switch. Keep a hand on the rollback (§6) through step 5.

**[CONFIRM] env-set mechanism:** how these env vars are set + reloaded at the venue (ecosystem.config.cjs + pm2 reload / systemd / deploy-agent env / `.env.production.local`) — confirm the per-service mechanism on .23 and Bono VPS. Per the "restart is also a deploy" rule, a reload re-reads env.

---

## 4. Deploy parity (UNIVERSAL — both environments, no exceptions)

Every flag/build above lands on **Server .23 (venue)** AND **Bono VPS (cloud)** — admin-proxy-bono, admin-proxy-james (.23 only), and the PWA build. Mismatch = customers on one host get a broken/half-switched flow.

---

## 5. Live E2E verification (name the EXACT behavior — not health checks)

Run from a **real browser/SMS**, not a curl from the server (per H3: server-curl ≠ customer browser). Targets: the venue PWA on .23 + the cloud PWA on Bono VPS.

**A. Bridge round-trip (after §3 step 2, before provider flip)**
- Trigger an Evolution OTP verify (existing path) with the bridge on. Confirm James's store received the token: a subsequent `createHousehold` for that phone returns **200** (was 401 with bridge off). This proves push+persist works end-to-end on the real WG tunnel (the in-process test already passed 12/12; this is the live-network confirmation NOT covered by that test).

**B. Customer SMS OTP (after §3 step 4-5)**
- On a real phone: open the PWA → enter phone → **receive an SMS code via MSG91** (not WhatsApp). Confirm the SMS arrives (DLT-dependent).
- Complete the widget → PWA posts `{otp_request_id, access_token, request_id}` → verify returns **200** with an `otp_verify_token` (identical-200 shape).
- `createHousehold` → **200** (household created; phone bound). Not 401.

**C. Full money path (the first-INR bar)**
- register(OTP via SMS) → topup → launch on a real pod → tick-debit → end → bill. Confirm ₹ reconciles, no double-spend, one session.

**D. Anti-regression spot checks**
- A wrong/expired code → 401/expired (not a silent pass).
- Two rapid double-submits of the same verify → exactly one succeeds (G-CUT-2, PR #28).
- No phone in any caller-facing error or log (PII discipline).

---

## 6. Rollback (flag-only, ≤1 min, no code)

Reverse order: `RP_OTP_PROVIDER=evolution` (bono, both) → rebuild PWA with `NEXT_PUBLIC_OTP_PROVIDER=evolution` → (optionally) `RP_OTP_BRIDGE_ENABLED=off` (bono) + `RP_OTP_BRIDGE_COMMIT=off` (james). Evolution path is byte-identical and was never modified → customers fall straight back. Reload services to re-read env.

> Note: if WhatsApp/Evolution is already ToS-dead at the venue, rollback returns you to a *non-working* OTP, not a working one — so do NOT cut over until §1 provisioning (esp. DLT) is confirmed. Rollback protects against a *broken MSG91 config*, not against the absence of any working channel.

---

## 7. After go-live (stability window) → V1 retire

Once MSG91 is stable and Evolution traffic has drained, execute the companion **V1-retire** plan `racecontrol/.planning/specs/v2/CUTOVER/otp-msg91-v1-retire-20260602.md` (C1 delete V1 Rust verifier/columns/Evolution sender; C2 guardian) under its own per-PR Captain auth. The 5-condition retire-trigger in that file gates it.

---

## Open / unverified (honest)
- All **[CONFIRM]** rows in §1 (MSG91 panel facts: authkey, verify URL, response shape, DLT names, IP allowlist, HMAC secret install).
- §3 env-set mechanism on .23 + VPS.
- §5 steps B/C have never run live (no MSG91 creds yet) — this runbook is the first live exercise.
- Durable-PG `otp_state` path: the in-memory store serves until the durable cutover (separate slice); the verify atomicity fix (PR #28) is the in-memory analogue of the SQL transactional consume.
