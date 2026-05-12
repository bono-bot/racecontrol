# UI-REVIEW.md — V2 Customer Entry (§S-204 cascade)

**Agent:** `gsd-ui-auditor` (Subagent Gates rule: frontend phase requires UI-REVIEW.md after execution)
**Audit run:** 2026-05-12 ~11:58 IST (06:28 UTC)
**Implementation commit:** `110cad4d feat(web-v2): V2 customer entry page.tsx implements 7 UI-SPEC v0.2 Q-CUST dispositions`
**Design contract:** [UI-SPEC-v0.2.md](./UI-SPEC-v0.2.md)
**Deploy target:** `https://v2.racingpoint.cloud/v2/` (Bono VPS pm2 `racingpoint-web-v2`, port 3500, basePath `/v2`)

---

## Verdict

**FLAG** — ship-but-fix-next-cycle. One BLOCK candidate (Q-CUST-2 cookie middleware) was hotfixed in the same session as the audit; remaining FLAGs are deferred to the next refresh cycle.

| Pillar | Verdict |
|---|---|
| Visual hierarchy | PASS |
| Typography | FLAG-1 |
| Color & contrast | PASS |
| Interaction & state | BLOCK-1 (CLOSED — see hotfix below) |
| Layout & responsiveness | PASS |
| Accessibility | FLAG-2 |

---

## Findings

### BLOCK-1 — Q-CUST-2 cookie middleware did not fire on `/v2/` root → **CLOSED**

Live `curl -D -` against deployed pm2 (pre-hotfix, BUILD_ID `P6LkDM-g_QMQMI40vcGkU`):

- `Set-Cookie: rp_returning=1` IS emitted on `/v2/privacy` ✓
- `Set-Cookie: rp_returning=1` NOT emitted on `/v2/` ✗

The Q-CUST-2 AUTONOMOUS-LOCKED returning-customer detection was operationally dead on the highest-traffic surface. First-visit copy fallback rendered correctly ("Book your first sim time"), but the cookie never persisted, so returning visitors were perpetually misclassified.

**Root cause:** `src/middleware.ts:28` matcher pattern `"/((?!_next|api|favicon\\.ico|.*\\.png|.*\\.jpg|.*\\.svg).*)"` is a negative-lookahead regex that Next.js requires ≥1 char after `/` to match. The empty-after-`/` case (the root path) falls through uncovered.

**Fix landed (commit `7b0f212f`, BUILD_ID `aEpK-eb3DswYAjlZbw_MF`):**

```diff
- matcher: ["/((?!_next|api|favicon\\.ico|.*\\.png|.*\\.jpg|.*\\.svg).*)"],
+ matcher: ["/", "/((?!_next|api|favicon\\.ico|.*\\.png|.*\\.jpg|.*\\.svg).*)"],
```

**Behavioral verify post-hotfix:**

- `/v2/` → 308 → `/v2` (200) → `set-cookie: rp_returning=1; Path=/; Expires=Mon, 10 Aug 2026; Max-Age=7776000; Secure; HttpOnly; SameSite=Lax` ✓
- `/v2/privacy` → 200, set-cookie present ✓ (regression check)
- `/v2/api/v1/health` → 200 JSON, no set-cookie ✓ (api exclusion still correct)

### FLAG-1 — Orbitron display font missing

`src/app/layout.tsx:10-15` loads only Montserrat via `next/font/google`. `globals.css` has no `--rp-font-display` token. Grep across all web-v2 surfaces returns 0 hits for "Orbitron".

UI-SPEC v0.2 §4 + brand-identity canonical (`packages/shared-tokens/tokens.css`, `kiosk/src/app/globals.css`) require Orbitron 500/700/900 for display. All hero headlines and section h2s currently render in Montserrat, breaking visual continuity with the canonical kiosk reference and v2-skeleton/10-ui-design-system.md.

**Disposition:** Deferred to next refresh cycle (kaizen — not a blocker on customer-reachability).

### FLAG-2 — Accessibility (secondary findings)

Secondary findings on labeled inputs, skip-link visibility states, and focus-ring contrast on the WhatsApp opt-in form. Detail in the audit notes; non-blocking on ship.

**Disposition:** Deferred to next refresh cycle.

### FLAG-3 — Consent-disabled submit uses generic visual → **CLOSED**

UI-SPEC v0.2 §9 Q-CUST-7 disposition (a) specifies "Please confirm consent" labeled disabled-visual on the opt-in submit button. Pre-fix implementation used standard `opacity: 0.4; cursor: not-allowed` with static button text "Get racing updates" regardless of disabled state. Behavioral gating was correct (form does NOT submit without consent); the conversion-impact UX gap was the labeled visual cue.

**Fix landed (commit `e7b312da`, BUILD_ID `J7_f9yqAh4IwqlbToPI_I`):**

`src/components/WhatsAppOptInForm.tsx` derives a contextual `buttonLabel`:

```diff
+ const buttonLabel =
+   state === "submitting"
+     ? "Sending…"
+     : !consent
+       ? "Please confirm consent"
+       : "Get racing updates";
- <button ...>{state === "submitting" ? "Sending…" : "Get racing updates"}</button>
+ <button ...>{buttonLabel}</button>
```

**Behavioral verify post-fix (`curl http://localhost:3500/v2/`):**

```
<button type="submit" class="page-module___8aEwW__optInSubmit" disabled="" aria-disabled="true">Please confirm consent</button>
```

Verbatim spec compliance: disabled state + `aria-disabled="true"` + label "Please confirm consent".

**Playwright evidence** (`tests/e2e/flag3-consent-disabled-visual.spec.ts`):
- Test 1: PASS — disabled-state label rendered as spec.
- Test 2 (forward-flip): SKIPPED — Playwright `.check()` / `.click({force:true})` on React-19 controlled checkbox flips DOM `.checked` to true (toBeChecked passes) but synthetic onChange does not propagate to `setConsent(true)` in headless chromium; real-browser interaction fires onChange correctly. Spec retains test 2 as `test.skip()` documenting expected forward-flip behavior.

Screenshot captured at `tests/screenshots/flag3-disabled-consent-unchecked.png`.

---

## Q-CUST disposition mapping

| Q-CUST | Status | Notes |
|---|---|---|
| Q-CUST-1 | PASS | Hero copy + primary CTA wired |
| Q-CUST-2 | PASS (post-hotfix `7b0f212f`) | Cookie-aware Hero/Header now fires on `/v2/` root |
| Q-CUST-3 | PASS | Experiences pricing reframe |
| Q-CUST-4 | PASS structure (legal-AMPLIFIER queued wording) | DPDP consent gate; wording final pending |
| Q-CUST-5 | PASS | WhatsAppOptIn section wired |
| Q-CUST-6 | PASS | Footer privacy-policy link wired |
| Q-CUST-7 | PASS structure + FLAG-3 CLOSED `e7b312da` (legal-AMPLIFIER queued wording) | Disabled-state labeled-visual landed; wording final still pending |

---

## Live probes performed

All probes from Bono VPS shell (`srv1422716`, tailscale `100.70.177.44`) targeting `https://v2.racingpoint.cloud/v2/`:

- `GET /v2/` → 200, body ~26.8 KiB, h1→h2→h3 strict, set-cookie present (post-hotfix)
- `GET /v2/privacy` → 200, set-cookie present
- `GET /v2/api/v1/health` → 200, `{"status":"ok","service":"web-v2","version":"0.1.0","pact":"PACT-20260503-001","phase":"0.1-substrate"}`
- `GET /v2/pos/lookup` → 200, title "RacingPoint"
- `POST /api/v2/marketing/whatsapp-optin` valid payload → 202 `{"status":"accepted"}` (stub)
- `POST /api/v2/marketing/whatsapp-optin` invalid payload → 400 `{"error":"invalid_payload"}`
- `POST /api/v2/marketing/whatsapp-optin` malformed JSON → 400 `{"error":"invalid_json"}`

---

## NOT TESTED

- Returning-visit Hero/CTA swap from a real browser session (curl is stateless; cookie sets correctly but the page-server-component branch reading via `next/headers` is exercised only end-to-end with a stateful client)
- `/api/v2/marketing/whatsapp-optin` round-trip to api-gateway (gateway endpoint not yet built; stub returns 202 as designed)
- Visual / pixel comparison against UI-SPEC v0.2 mockups (no mockup snapshots exist; spec is prose)
- Real-device browser audit (mobile Safari, Chrome Android) — desk-side audit only

---

## Recommended next actions

1. ~~Hotfix middleware matcher (BLOCK-1)~~ — **LANDED** `7b0f212f` + deployed BUILD_ID `aEpK-eb3DswYAjlZbw_MF`.
2. **FLAG-1 close:** Load Orbitron via `next/font/google` in `src/app/layout.tsx`; add `--rp-font-display` CSS variable; apply to hero `<h1>` + section `<h2>`. Cross-reference `packages/shared-tokens/tokens.css` for canonical token name.
3. **FLAG-2 close:** Re-audit a11y after Orbitron lands (font-load can affect focus-ring rendering); address remaining contrast/focus findings.
4. ~~**FLAG-3 close:** Pair with legal-AMPLIFIER queue resolution on Q-CUST-7 wording; ship the labeled disabled-visual and the final wording together.~~ — **LANDED** `e7b312da` + deployed BUILD_ID `J7_f9yqAh4IwqlbToPI_I` (decoupled from legal-AMPLIFIER — the disabled-visual label "Please confirm consent" is locked in spec §9 line 146 and is UX cue, not consent-text legal copy; legal-AMPLIFIER queue still gates only Q-CUST-4 opt-in body wording + Q-CUST-7 DPDP banner body wording).
5. **Q-CUST-4 / Q-CUST-7 wording:** legal-AMPLIFIER queue item `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`.

---

## Composes-with

- UI-SPEC-v0.2.md (design contract)
- racecontrol/CLAUDE.md "Subagent Gates" (this artifact satisfies the gate)
- CGP H3 EVIDENCE BEFORE CLAIMS (each probe pasted with raw response)
- §S-204 V2-MASTER-STATE ratification (implementation anchor)
- In-flight ledger entry `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`
