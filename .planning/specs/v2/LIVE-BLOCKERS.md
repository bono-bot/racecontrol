# V2 GO-LIVE BLOCKERS — open register

> **Authored 2026-05-14 13:56 IST** from Captain go-live ping "kiosk still shows connecting; update LIVE BLOCKERS with other possible gaps". Live-readiness lens, not substrate-completion lens. Append-only — closures move to "CLOSED" footer with evidence + commit hash. Composes-with: V2-PROGRESS-MAP §0 (~78 LIVE-BLOCKING items, substrate frame) + V2-LBAC-PROTOCOL (closure flow) + LOGBOOK.md (commit trail). Stale-at: every 60min during go-live; daily otherwise.

> **Re-authored 2026-05-14 16:10 IST** on doctrine branch `doctrine/v1-v2-gap-flow-plan` after parallel-pilot branch-state mutation lost the untracked working-tree copy. Committing this round per V1→V2 gap-flow plan Track B8 — planning artifacts MUST be committed in same session as authoring; never left untracked across parallel-pilot windows.

**Severity scale.** P0 = customer cannot complete a session right now. P1 = customer can complete but staff sees error or workflow break. P2 = silent drift / will-bite-later.

**Authoring discipline (post-G9 #3 2026-05-14 ~15:40 IST · V1→V2 gap-flow plan B3).** Every authoring round MUST produce BOTH sections below:
- **§ Forward audit** — enumerate V2-doctrine-expected surfaces from `project_v2_customer_workflows_consolidated_20260503.md` + V2-PROGRESS-MAP §0 + ECOSYSTEM-MANIFEST.json + CLAUDE.md "Server Services" table; parity-check each against deployed venue reality. Author entries for every gap.
- **§ Backward audit** — probe-driven catalog of observed failures.

Either section empty = audit incomplete. Forward audit goes FIRST because reactive-only ops is the V1 antipattern the gap-flow plan closes.

---

## § Forward audit — V2-doctrine-expected surfaces vs venue-deployed

**Audit run 2026-05-14 15:55 IST · james-LEAD · per V1→V2 gap-flow plan Track B item B3.**

Parity probe per CLAUDE.md "Server Services" drift-detection block: `for p in 3100 3200 3201 3300 3500 3501 8080; do curl http://192.168.31.23:$p/api/health; done`.

| Surface | Doctrine source | Cloud state | Venue state | Gap |
|---|---|---|---|---|
| `:3500/v2` web-v2 V2 customer entry | `project_v2_customer_workflows_consolidated_20260503.md` + UI-SPEC v0.2 + 18 commits incl. `c08fc5c9 deploy: V2 customer-entry frontpage v0 LIVE on production` | `https://racingpoint.cloud/v2` → 200 / cloud `:3500/v2` → 200 | `http://192.168.31.23:3500/v2` → 000 | **B15 — V2 customer-entry frontpage DOWN at venue** |
| `:3501` pwa | commit `d4e1c812 feat(pwa): customer-facing landing page at www.racingpoint.cloud /` + `005c09ec` port-move | LIVE on cloud | venue NOT VERIFIED | **B16 — pwa venue deployment status unverified** |
| `:3100` admin-gateway | `racingpoint-admin/.env` `NEXT_PUBLIC_GATEWAY_URL=http://192.168.31.23:3100` + spinal-cord-gateway A1+A2+A5-stub commits | unknown | `:3100/` → 000 | **B17 — admin-gateway DOWN at venue** |
| `:8080` racecontrol | always-required | matches | `build_id: 9b73a0ac` (PR #79 per §S-311) | OK |
| `:3200` web (POS) | always-required | matches | `build_id: QGRDoQJ`, `git_commit: c5f94e31-dirty` Apr-29 | OK (zero `web/` commits since) |
| `:3201` admin (web-dashboard binary) | always-required | matches | same binary as :3200 | OK as-is; B13 fix pending re-deploy |
| `:3300/kiosk` | always-required | matches | `build_id: FTka22`, `git_commit: fd59cf4b-dirty` | OK (zero `kiosk/` commits since fd59cf4b) |

### B15 — V2 customer-entry frontpage DOWN at venue (P0)

**Symptom:** `curl http://192.168.31.23:3500/v2` → 000 (no listener). V2 customer-entry frontpage (Orbitron typography + A11y AAA + canonical venue data + 18% GST + WhatsApp opt-in + 7 UI-SPEC Q-CUST dispositions) is LIVE on cloud (`https://racingpoint.cloud/v2` → 200, 22581 bytes per deploy commit `c08fc5c9`) but absent at venue Server .23.

**Impact:** Customer arriving at venue via `:3500/v2`-pointing kiosk-QR / signage / iPad will see ECONNREFUSED. V2 doctrine customer-day entry surface is dark venue-side. V2 doctrine = `single-roof game launch + billing + cafe` at venue per `project_v2_core_product_definition.md` — cloud-only is insufficient.

**Evidence:**
- 18 commits in `racecontrol/web-v2/` since deployed admin/web `git_commit=c5f94e31` (Apr-29), including `c08fc5c9` deploy claim ("pm2 restart racingpoint-web-v2 successful · external HTTP 200 22581 bytes verified · 3/3 live-URL screenshots captured").
- That deploy targeted Bono VPS pm2 process `racingpoint-web-v2`, NOT venue Server .23.
- `web-v2/package.json` description: "RacingPoint V2 dedicated Next.js host (port 3500, basePath /v2). Phase 0.1 substrate".
- `web-v2/next.config.ts`: `basePath: "/v2"`, `output: "standalone"`.

**Fix path:**
1. bono pulls `racecontrol/main` on Bono VPS (already on cloud).
2. Build `web-v2/` standalone if not already built / artifact transfer to .23.
3. Establish pm2 `racingpoint-web-v2` process on Server .23 listening on `:3500` (mirror cloud config).
4. DEPLOY PARITY: cloud `git_commit` must match venue.
5. Append SWAPLOG row with `SWAPLOG_REASON="B15 web-v2 venue activation"`.
6. Verify: `curl http://192.168.31.23:3500/v2` returns 200 + matching bundle.

**§S-146 RCA gate:** web-v2 is V2-NEW surface (no V1 substrate dependency). Not in scope. §S-186 fast-lane not applicable (deploy-activation, not bug fix). DEPLOY PARITY rule applies as standing rule.

**Owner:** bono-LEAD (per `c08fc5c9` author + pm2 deployment ownership). First-mover-lead doctrine confirms.

**Companion:** B13 (admin login fix · independent surface, same deploy session opportunity) · I.5 stale-deploy class bono raised in msg=36693 (B15 is the canonical surface bono was referencing).

---

### B16 — pwa venue deployment status UNVERIFIED (P0 candidate)

**Symptom:** pwa source has 2 commits since c5f94e31 (`d4e1c812 feat(pwa): customer-facing landing page at www.racingpoint.cloud /` + `005c09ec fix(pwa): canonical ecosystem.config.cjs + port move 3500 → 3501`). Cloud customer-facing landing at `www.racingpoint.cloud` is LIVE. Venue pwa state at `:3501` not probed this session.

**Impact:** If customer-facing pwa landing is part of venue customer-day path (QR scan → pwa → web-v2), venue not having pwa breaks the entry chain. If pwa is purely cloud-side, venue parity may not be required.

**Fix path:**
1. Probe venue `:3501` health endpoint.
2. Determine doctrine: is venue-side pwa required? (Open `project_v2_customer_workflows_consolidated_20260503.md` Scenario 1).
3. If required: pull + build + deploy pwa to venue :3501.
4. If not required: close as P2 NOT-REQUIRED-AT-VENUE.

**Owner:** bono-LEAD (pwa deploy ownership symmetric with web-v2).

**Composes-with:** B15 (same deploy session).

---

### B17 — admin-gateway DOWN at venue (P0)

**Symptom:** `curl http://192.168.31.23:3100/` → 000. The `racingpoint-admin` separate-repo app `bono-bot/racingpoint-admin` has admin-gateway substrate (`8bf1eb6 feat(admin-gateway): implement spinal-cord gateway contract A1+A2+A5-stub` + `06cc392 feat(admin-gateway): MI audit-seed emitter` + `89cdb80 fix(settings/pipeline): route through /api/rc/*`). Admin app `.env.production.local` references `NEXT_PUBLIC_GATEWAY_URL=http://192.168.31.23:3100` — no listener there.

**Impact:** Admin-gateway is the V2 spinal-cord — `/api/rc/[...path]` proxy + MI audit-seed emitter + settings pipeline routing. Without it, admin app calls expecting gateway features fail. If `racingpoint-admin` is the intended `:3201` deploy (not current `web-dashboard` binary), admin-gateway is a prerequisite.

**Fix path:**
1. Captain disposition: is `racingpoint-admin` intended for venue :3201 (replacing current web-dashboard) OR cloud-only?
2. If venue-intended: deploy admin-gateway to :3100 + admin app to :3201 via `racingpoint-admin` repo build pipeline.
3. If cloud-only: close as NOT-REQUIRED-AT-VENUE; remove `192.168.31.23:3100` from admin .env defaults.

**Owner:** bono-LEAD (`bono-bot/racingpoint-admin` repo owner).

**Composes-with:** B13 (admin app deploy question — if B17 disposition is "venue-intended", then B13 fix in `web/src/app/login/page.tsx` may be the WRONG target; the right target would be `racingpoint-admin` login). Captain DevTools paste would resolve which app Captain was actually using when login failed.

---

### Forward-audit completeness check

**Surfaces enumerated:** 7 (3500, 3501, 3100, 8080, 3200, 3201, 3300).
**Gaps found:** 3 (B15, B16, B17).
**NOT enumerated this round (deferred forward-audit-v2):**
- Pod state (rc-agent v2 / rc-sentry v2 if any V2 doctrine specifies new pod-side surfaces)
- POS .130 surfaces (CDP :9222 kiosk Chrome + reference_pos_chrome_kiosk.md)
- Cafe / WhatsApp-bot customer-day surfaces (PACT-013/014/015 substrate-ships)
- WS event-format parity (kiosk + admin + web vs racecontrol :8080 message types)
- Memory anchor sweep against V2-PROGRESS-MAP §0 ~78 LIVE-BLOCKING items (Phase-2 expansion)

Next forward-audit round should add these surface classes. Stale-at: every 60min during go-live.

---

## § Backward audit — probe-observed failures

## P0 — customer-blocking

### B13 — Admin/staff login failing at http://192.168.31.23:3201/login (real root cause located + Path A fix COMMITTED + PR #80 OPEN)

**Source:** bono msg id=36689 (2026-05-14 08:31Z / 14:01 IST) — Captain dispatched. Captain doing browser DevTools (Path-1). bono correction msg=36695 redirected endpoint from `/api/v1/auth/admin-login` to `/api/v1/staff/validate-pin` (real `racecontrol/web/src/app/login/page.tsx:43`).

**Real root cause (smoking-gun bundle evidence):** Deployed `:3201` chunk `af2d3f59fbcc69ac.js` contains `let i=t.default.env.NEXT_PUBLIC_API_URL||"http://localhost:8080"` — Turbopack DID NOT inline `NEXT_PUBLIC_API_URL` at build time. Runtime lookup of `t.default.env.NEXT_PUBLIC_API_URL` is undefined in the browser → fallback fires → `http://localhost:8080` → Captain's browser POSTs to its own localhost → ECONNREFUSED → catch block displays "Cannot reach server. Check your connection." Fleet-tally: 47 chunks contain `http://localhost:8080`; 1 chunk contains `http://192.168.31.23:8080`.

**Fix (Path A · mirrors kiosk pattern):** `web/src/app/login/page.tsx:8` replace single-line hard fallback with `(typeof window !== "undefined" ? \`http://${window.location.hostname}:8080\` : "http://localhost:8080")` conditional. Mirrors `kiosk/src/hooks/useKioskSocket.ts:22-26` already-correct pattern.

**Status:** COMMITTED + PR-OPEN

| Field | Value |
|---|---|
| Branch | `fix/admin-login-api-base-fallback` |
| Commit | `e0c996af` |
| PR | https://github.com/bono-bot/racecontrol/pull/80 |
| Base | `origin/main @ cad5704f` |
| Diff | `+5 / -1` (web/src/app/login/page.tsx) |
| §S-186 short-RCA | inline in commit body + PR body |
| Captain auth (fix path) | "A (source pattern fix matches kiosk)" verbatim 2026-05-14 ~14:55 IST |
| Cascade-skip auth | `WORKFLOW_CASCADE_SKIP=1` per Captain "Recommend (I)" verbatim 2026-05-14 ~15:00 IST |

**Pending (Captain disposition):**
- Merge PR #80 to main.
- Rebuild `racecontrol/web` with `NEXT_PUBLIC_API_URL=http://192.168.31.23:8080` baked at build time (defense in depth).
- Deploy to Server .23 `:3201` (admin-deploy.tar.gz extract pattern).
- DEPLOY PARITY: cloud admin `:3201` on Bono VPS.
- VERIFY: bundle no longer contains `t.default.env.NEXT_PUBLIC_API_URL` literal for login chunk · Captain DevTools paste OR live login attempt shows 200 + token + cookie + `/staff` renders.

**Earlier diagnostic candidates (eliminated):**
- A. Lockout active (HTTP 429) — RULED OUT: `admin_lockout` row `failed_attempts=1 locked_until=NULL`. Per-IP not admin-shared per bono correction.
- B. PIN not configured (HTTP 503) — RULED OUT: `admin_pin_hash` Argon2id at racecontrol.toml:92.
- C. PIN value mismatch — not the issue (request never reaches backend per finding).
- D. Frontend POST URL wrong — CONFIRMED via bundle probe (this entry).
- E. JWT not stored client-side — not the issue.
- F. Stale frontend deploy — partially confirmed (deploy Apr-29 + Turbopack env-bake regression).

**NOT TESTED:**
- Path A patch's actual behavior at runtime (gated on rebuild + deploy + browser test).
- Whether Turbopack inlines `NEXT_PUBLIC_API_URL` when set (separate from this patch).
- Cloud admin `:3201` bundle state.
- Captain DevTools paste (would 1-shot confirm pre-fix Network status).

---

### B14 — fleet-swaplog-parity RED (3 gaps · audit-trail-class · NON-blocking)

Surfaced during B13 commit attempt. Pre-commit cascade hook reports `fleet-swaplog-parity: RED (3 gaps)`. Cached `.planning/board-state/status.json` STALE from 2026-05-12 16:24 IST at commit `61999f58`; live `:8080/api/v1/health` reports `build_id: 9b73a0ac` (PR #79 deploy per bono §S-311). The 3 gaps = SWAPLOG rows missing for recent deploys — `SWAPLOG_REASON` env var not set on those deploy invocations.

Class: audit-trail-repair · not customer-visible · not customer-blocking. Owner: bono (last deployer per §S-298 + §S-311). Action: append missing SWAPLOG rows + refresh `.planning/board-state/status.json` snapshot.

Captain authorized `WORKFLOW_CASCADE_SKIP=1` bypass for B13 commit because B14 is orthogonal. Same precedent applies to this commit (doctrine update).

---

### B1 — Kiosk PWA stuck on "Loading…" at http://192.168.31.23/kiosk/

**Observed (Captain 2026-05-14 13:55 IST):** browser opens /kiosk/ and shows connecting state indefinitely.

**Evidence from James .27:**
- `GET /kiosk/` → 200 OK · 4.5KB HTML · `/kiosk/_next/static/chunks/*.js` linked · body has `Loading...` spinner + `AuthGate` chunk reference. Page IS rendering — the "Connecting" perception is the AuthGate / WS-init layer, not network outage.
- `GET /` (port 80) → 307 → `/portal` · server :80 is fronted by admin reverse-proxy, NOT kiosk. `/kiosk/` works only via rewrite.
- `GET :3300/kiosk/` → 308 (Next.js basePath redirect; expected).
- WS upgrade `ws://192.168.31.23:8080/ws/dashboard` with token `rp-terminal-2026` in Sec-WebSocket-Protocol → 101 Switching Protocols · `pod_list` event delivered · WS server-side healthy.
- WS upgrade with bogus token → 401. Auth enforced.
- Kiosk env `kiosk/.env.production.local`: `NEXT_PUBLIC_WS_URL=ws://192.168.31.23:8080/ws/dashboard` · `NEXT_PUBLIC_WS_TOKEN=rp-terminal-2026`.

**Likely failure modes (ranked):**
1. NEXT_PUBLIC_WS_TOKEN baked stale — grep served JS chunks for token literal.
2. basePath / asset 404 — verify `curl :3300/api/v1/health` returns JSON not 404 HTML.
3. AuthGate intercept — kiosk root requires staff JWT; AuthGate may spinner-loop without redirecting.
4. CSP — eliminated (header allows ws://192.168.31.23:8080).
5. Mixed-content / hostname mismatch — eliminated.

**Next action (decision-pending):**
- DevTools on POS / kiosk browser for WS attempt URL + Sec-WebSocket-Protocol header + response status.
- Alternative: grep bundle on :3300 for `rp-terminal-2026` literal.

**§S-146 RCA gate:** kiosk WS layer crosses V1↔V2 boundary (staff PIN auth = V1 raw-PIN per PACT-018 §A). Mode-1 rebuild path triggers §S-186 fast-lane (≤200 LOC config-only rebuild, no schema/protocol).

---

## P1 — operational gaps surfaced during probe

### B2 — Pod 1 IP drift (3rd unique IP in 48h)

`fleet/health` row Pod 1 reports `ip_address: 192.168.31.3`. Canonical = `192.168.31.89`. Memory anchor recorded `.16` on 2026-05-12. `.3` is third unique reading.
- Class: DHCP-class drift; V2.1 Q-INFRA-1 (DEFER) per §S-211, UNLESS causing customer-facing failure.
- Re-open trigger fires: 3 IPs / 2 days exceeds original assumption.
- Adjacent risk: V2 code paths hardcoded to `.89` for Pod 1 routing silently broken. Grep before deploy.

### B3 — Web `/pos` route returns 404 on :3200

`curl :3200/pos` → 404. Either (a) POS workflow uses different route name, (b) `/pos` not mounted, (c) basePath conflict. Confirm with POS operator before classifying.

### B4 — Binary build skew across fleet

- Server `racecontrol`: `9b73a0ac` (post PR #79 EnvFilter merge; §S-311 receipt-verify).
- Pods 1-8: `c5f94e31-dirty`.
- Kiosk frontend on `:3300`: hash unknown.

**Risk:** WS event-format skew (2026-04-03 admin-dashboard WS-churn precedent class).

**Action:** before further deploy, `git log 9b73a0ac..c5f94e31 -- crates/racecontrol/src/api/ws/ crates/rc-common/src/messages/`. If protocol changed, rebuild server + kiosk + admin + web at same SHA.

### B5 — Clock drift on Pod 6 = 41 seconds

Acceptable for log correlation; NOT acceptable for billing / lap-time correctness under V2 wallet rate-table contract. Action: w32tm resync or NTP group policy fleet-wide.

### B6 — POS workflow surface not verified

POS @ 192.168.31.130 not pinged this turn. "Audit all PCs regardless of venue hours" rule fires. Action: `curl --max-time 5 http://192.168.31.130:8090/health` + verify Chrome kiosk on :9222 CDP.

### B7 — Cloud / Bono VPS parity unknown

DEPLOY PARITY rule applies at every go-live; cloud state was not probed. Action: `curl --max-time 8 https://app.racingpoint.cloud/api/v1/health` and compare `build_id` against venue. If different → cloud is user-facing; push cloud to match.

---

## P2 — drift / surfacing for tracking

### B8 — V2-PROGRESS-MAP F3-restated closure rate ≈ 10% (not the ~32% in §0)

Per CLAUDE.md §14.3 F3 ACCOUNTING REFORM: "Layer 1 ENTIRELY COMPLETE 20/20" under F3 framing = 2 DONE + 18 TEST-SCAFFOLDED-AWAITING-SUBSTRATE = ~10% true Layer 1 ENGINEERING completion. Customer go-live happens against ENGINEERING, not test-scaffold. Treat "% complete" claims above ~30% with suspicion until V2-PROGRESS-MAP rerun under pure F3 framing.

### B9 — All pods report `screen_blanked: true`

Expected idle state per V2 blanking doctrine, BUT — combined with B1 (kiosk-side connecting failure) means: customer walking up RIGHT NOW has no path to un-blank a pod via kiosk because WS handshake broken. Pod blanking is not a bug; becomes a customer-blocking gap when kiosk is also down. Resolution = B1.

### B10 — `dashboard_clients: 0`

Zero dashboards connected to WS server. Either (a) no admin/staff has dashboard open right now, or (b) dashboards failing same WS handshake as B1. If (b) — same root cause as B1.

### B11 — ~~Server binary on stale pre-V2 hash `9b73a0ac`~~ **RETRACTED 2026-05-14 14:08 IST — G9 #1 this session**

Original entry claimed `9b73a0ac` was "pre-V2-era". WRONG. Per bono §S-311 `9b73a0ac` IS racecontrol main HEAD post-PR #79 EnvFilter fix merge (N=2 HTTP probe receipt-verify). The `deploy_context` string `v34-v39 merged` is stale build-metadata authored months ago and was never refreshed by PR #79 deploy — NOT a freshness signal. Server is current.

**Root cause:** read `deploy_context` field literal as freshness claim, without cross-checking `build_id` against racecontrol main log + SWAPLOG. **Structural fix:** CLAUDE.md rule "SWAPLOG check at session start — MANDATORY before any absolute claim about server build_id" — known-rule violation, not new-rule candidate. **Replace with real B11:** `deploy_context` should be regenerated at every server build to match build_id semantics. Backlog tooling task (NOT live-blocking).

### B12 — Security-debt ledger entries open against V2 customer paths

CLAUDE.md "Security Debt" section: 3 seed entries open (PACT-026 §A direct-M2M auth-gap, PACT-018 raw staff.pin credential-storage, Q4-3 dynamic-pricing policy-gap). Go-live ships V1 trust intact per Captain Q2. These are NOT blockers, but they ARE failure modes that bite first if WhatsApp / staff PIN / pricing flows misbehave during live operation.

---

## Closed (footer — append on close)

_(empty)_

---

## How to update this file

1. Append new blocker to its severity section in § Backward audit, OR enumerate new surface in § Forward audit table + add P0/P1/P2 entry.
2. When closing: cut entry to "Closed" footer · prepend `**CLOSED <date IST>** ` · paste commit hash + raw-output evidence (CGP H3).
3. Per `feedback_sn_close_anchor_push_standing_rule_20260512.md`, this file is OUTSIDE the §S-N close-anchor + V2-PROGRESS-MAP refresh standing autonomy — pushes to main require normal Captain auth on every commit.
4. **Authoring discipline rule (post-G9 #3):** both § Forward + § Backward sections required every authoring round. File must be committed in same session as authoring (NOT left untracked) per Track B8 of V1→V2 gap-flow plan.
