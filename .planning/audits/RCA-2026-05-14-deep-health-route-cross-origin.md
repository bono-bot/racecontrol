# RCA: web/src/app/api/health/deep/route.ts — cross-origin LAN-IP server-side fetch

**Date:** 2026-05-14
**Author:** james
**Trigger:** MAOR Tier-1 review (V2-LBAC §14.1) of PR #80 v2 + PR #84 found this site flagged IMPORTANT-90% as a real defect, despite original RCA-2026-05-14-layer-12-7-sibling-v1-browser-fetch-consumers §1 declaring it "out-of-scope".
**Class:** §S-146 V1↔V2 RCA (V1-antipattern: cross-origin LAN-IP env-var-fallback HTTP fetch)
**Captain-stake test:** foundational-boundary (V2 same-origin doctrine + DEPLOY PARITY) — per-PR Captain merge auth required.
**Sibling RCAs:** [RCA-2026-05-14-layer-12-7-sibling-v1-browser-fetch-consumers.md](RCA-2026-05-14-layer-12-7-sibling-v1-browser-fetch-consumers.md) (parent, scope-deferred this site) · [RCA-2026-05-14-cameras-cross-origin-sentry-fetch.md] (planned, D-MAOR-1).
**Composes-with:** [[v1-antipattern-fix-eligibility-check]] G9 #4 (this fix is URL-construction class → §S-186 fast-lane INVALID, full §S-146 required) · [[enumerate-by-class-not-substrate-instance]] G9 #2 (this site was missed by the original H4 enumeration; the present RCA closes the gap).

---

## §1 — Boundary map

**File:** `web/src/app/api/health/deep/route.ts:13`

```typescript
const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://192.168.31.23:8080';
```

**Surface class:** Next.js App Router route handler — runs in the **server-side Node.js process** of the web Next.js app (NOT in the browser bundle). Invoked when a client (browser, monitoring, or operator curl) GETs `/api/health/deep` on whichever host serves the web app.

**Deployment topologies the surface runs in:**

| Topology | Web Next.js process location | `API_BASE` resolution under current code | Backend racecontrol reachable? |
|---|---|---|---|
| Venue Server .23 | Server .23 pm2 web :3200 | `NEXT_PUBLIC_API_URL` set to `http://192.168.31.23:8080` OR fallback fires → same IP | ✓ co-located |
| Cloud Bono VPS | VPS pm2 racecontrol-web :3500 / 3200 | `NEXT_PUBLIC_API_URL` likely unset OR set to venue IP → fallback fires → `http://192.168.31.23:8080` | ✗ venue-LAN IP unreachable from VPS |
| James .27 dev | James workstation `next dev` | Local env, varies | Varies (Tailscale or absent) |

**Backend racecontrol :8080 source-of-truth:** runs on Server .23 (venue) AND cloud Bono VPS pm2 (DEPLOY PARITY rule). On the cloud topology, web Next.js process + cloud racecontrol are co-located (localhost reachable). On the venue topology, web Next.js + venue racecontrol are co-located (localhost reachable).

**Companion contract:** `web/next.config.ts:17` rewrites `/api/:path*` → `${process.env.API_PROXY_TARGET || "http://localhost:8080"}/api/:path*`. This is the V2-aligned same-origin rewrite proxy landed by PR #80 v2 (`edca0333`) + PR #84 (`56f67a8f`).

**Discrepancy:** `next.config.ts:17` uses env var `API_PROXY_TARGET` with `localhost:8080` fallback (works on both topologies because backend is co-located in both). `route.ts:13` uses a DIFFERENT env var `NEXT_PUBLIC_API_URL` with `192.168.31.23:8080` fallback (works only on venue, breaks on cloud). The two server-side surfaces of the web app reference different env vars and different fallback hosts.

---

## §2 — Inherited-issue catalogue

V1 failure modes touching this boundary (sourced from `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` categories A-J + §S-61 PART 41 V1 failure-mode investigation):

| ID | V1 failure mode | Touches this boundary? | Citation |
|---|---|---|---|
| **I-1** | env-var-fallback to LAN-IP literal in browser/server code (V1-antipattern: cross-origin LAN-IP env-var-fallback) | YES — direct hit | Parent RCA §2 I-1; original PR #80 v1 = same antipattern at admin login |
| **I-2** | HTTPS protocol mismatch when surface deployed to cloud (`http://` LAN IP unreachable + non-TLS in HTTPS context) | YES — cloud serves over HTTPS → mixed-content + unreachable host | Parent RCA §2 I-3 (sibling) |
| **I-3** | NEXT_PUBLIC_ prefix used for what is actually a server-side route handler (no need to bake into browser bundle) | YES — semantic misuse; env var name doesn't match the layer | Parent RCA §2 I-2 (env-var-prefix-leakage class) |
| **I-4** | Cloud-deploy path not exposed (`:8080` not externalized from VPS to public internet by design — backend is co-located + same-origin) | YES — fallback to `:8080` LAN IP from cloud Next.js process fails the moment it tries to leave the host | Parent RCA §2 I-4 (cloud-path class) |
| **I-5** | Health endpoint silently returns 503 from cloud — no operator notification, no alert | YES — fetch timeout (8s, per `TIMEOUT_MS`) returns error to caller; no fleet-health alert chain wired | §S-61 PART 41 category I (audit-blind proxy checking class) |
| **I-6** | Hardcoded LAN IP in source code creates source-state drift (canonical Network Identity table maintains `.23` but env-var-fallback ages independently) | YES — same as parent RCA I-7 finding (LeaderboardTable hardcoded IP, now closed) | Parent RCA §2 I-7 + post-merge audit; [[network_map_before_ip_probe_20260512]] |
| **I-7** | Server-side route handler tested via manual `curl :3200/api/health/deep` only — never proved on the cloud topology | YES — no Playwright/contract test covers cross-topology behavior | Parent RCA §5.4 mechanism-trust check Q2 |
| **I-8** | "Out-of-scope" scope-collapse class — original RCA §1 explicitly declared this file "separate RCA scope if pursued" without authoring the follow-up RCA, leaving the defect alive post-merge | YES — this very RCA exists to close that scope-gap | MAOR Tier-1 finding 2026-05-14; [[enumerate-by-class-not-substrate-instance]] G9 #2 |

---

## §3 — Past-bug disposition

| Issue | Disposition | Citation |
|---|---|---|
| I-1 env-var-fallback to LAN-IP in browser/server | **ROOT-CAUSED-AND-FIXED on browser surfaces** (PR #80 v2 + PR #84) · **OPEN on this server-side route** (this RCA closes it) | edca0333 + 56f67a8f |
| I-2 HTTPS protocol mismatch | **STRUCTURALLY-CLOSED by same-origin pattern** (same-origin → same scheme → no mismatch) | next.config.ts:17 rewrite contract |
| I-3 NEXT_PUBLIC_ semantic misuse | **OPEN at this boundary** — fix migrates env var to `API_PROXY_TARGET` (matches next.config.ts:17), removing the NEXT_PUBLIC_ semantic leak | This RCA §5 |
| I-4 Cloud-deploy path not exposed | **STRUCTURALLY-CLOSED by co-location** — both topologies have backend racecontrol on localhost; localhost:8080 fallback is correct in both | DEPLOY PARITY rule + Server Services table |
| I-5 Silent 503 from cloud | **PARTIALLY-OPEN** — fix resolves the 503 by making the fetch succeed; alert-chain wiring for /api/health/deep failures is a separate scope (Phase 2 D-329-4 observability per §S-332) | Defer to D-329-4 |
| I-6 Hardcoded LAN IP source drift | **CLOSED-BY-FIX-LANDED** — fallback target changes from LAN IP to localhost | This RCA §5 |
| I-7 Server-side route untested on cloud topology | **PARTIALLY-OPEN** — fix makes the route reachable from cloud; behavioral test for `/api/health/deep` from cloud topology is D-MAOR-4 scope (Phase 2 anti-pattern grep test) | Defer to D-MAOR-4 |
| I-8 Out-of-scope scope-collapse class | **CLOSED-BY-AUTHORING** — this RCA closes the scope-gap; [[enumerate-by-class-not-substrate-instance]] memory anchor prevents recurrence on future H4 enumerations | Memory file `feedback_enumerate_by_class_not_substrate_instance_20260514.md` |

---

## §4 — V2-alignment delta

**V2 doctrine surface this change moves toward:**

- **V2-MASTER-STATE §S-330 PR #80 v2 MERGED ledger** — V2-aligned same-origin pattern established at admin login; this fix extends the pattern to the deep health route (sibling surface)
- **V2-PROGRESS-MAP Layer 12 (frontend / browser-deployed code)** — closes one of the deferred sub-items; LeaderboardTable WS hardcoding (D-MAOR-3) + cameras (D-MAOR-1) remain
- **DEPLOY PARITY (racecontrol/CLAUDE.md UNIVERSAL — NO EXCEPTIONS)** — both venue web :3200 and cloud Bono VPS pm2 racecontrol-web need to call this route handler; current code fails cloud silently
- **`feedback_v2_only_forward_path.md`** — V2 closes V1-antipattern class (cross-origin LAN-IP env-var-fallback); not V1 components categorically. This site IS the V1-antipattern class.
- **`feedback_v1_dependent_v2_root_cause_before_proceeding.md`** — V1↔V2 boundary touched (V1-era server-side health route now living in V2 web Next.js process); 5-section RCA gates the fix

**Gap named explicitly:** the server-side surface of the web Next.js app was carrying V1-era env-var-fallback semantics (`NEXT_PUBLIC_API_URL` + LAN IP literal) while the companion server-side surface (`next.config.ts:17`) had already moved to V2 semantics (`API_PROXY_TARGET` + localhost). Two server-side surfaces of the same app referenced different env vars and different fallback hosts. Fix unifies on the V2 semantics.

---

## §5 — Proposed change (V2-framed)

**Diff (single line, web/src/app/api/health/deep/route.ts:13):**

```typescript
// BEFORE (V1 antipattern — bakes LAN IP into server-side default + uses NEXT_PUBLIC_ for server-only env):
const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://192.168.31.23:8080';

// AFTER (V2 aligned — matches next.config.ts:17 same-origin rewrite contract, co-located localhost fallback):
const API_BASE = process.env.API_PROXY_TARGET || 'http://localhost:8080';
```

**Why this is the smallest reversible change:**

- Single line · single file · no schema change · no protocol change · no migration · no contract change at the consumer interface (still `GET /api/health/deep`)
- Reverts cleanly with `git revert` if topology assumptions break
- Env var `API_PROXY_TARGET` already exists in the deployed env (used by `next.config.ts:17`); no new env var needs to be set on either venue or cloud
- `localhost:8080` fallback works on both topologies because racecontrol backend is co-located on both Server .23 + Bono VPS (Server Services table + DEPLOY PARITY rule)

**Why NOT §S-186 fast-lane (3-section short-RCA):** per [[v1-antipattern-fix-eligibility-check]] G9 #4, fast-lane is INVALID for this class. The fix changes URL construction + env-var-binding-class + same-origin topology assumption — all axes the fast-lane rejects. Full §S-146 5-section required (this document).

**Mechanism-trust 5Q check (per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`):**

| # | Question | Answer | Evidence |
|---|---|---|---|
| 1 | Atomic primitives? | YES | Single Edit on a single file; commit + push is atomic; no multi-step pipelined ops |
| 2 | TTL-bounded sentinels integrated with atomic primitive? | N/A | No sentinel needed for static config-default change; redeploy is the only state mutation |
| 3 | Behavioral-verify success (not echo-string)? | YES — but verify-phase next turn | Verify behavior = `curl <web>/api/health/deep` from cloud topology returns `healthy: true` instead of `healthy: false / fleet_health: error` |
| 4 | Single-target dry-run path? | YES | Deploy to venue first, verify, then cloud — DEPLOY PARITY sequence |
| 5 | Guards have written contracts with delivery script? | YES | DEPLOY PARITY rule + `deploy-server.sh` v3.0 + frontend-staleness-check.sh + manifest protocol |

5/5 PASS → V2-aligned, proceed to fix without an upstream mechanism RCA.

---

## §6 — Verify-by (post-merge behavior tests)

1. **Venue:** `curl -s http://192.168.31.23:3200/api/health/deep` from James .27 (LAN) → `healthy: true` + all 3 checks pass (`fleet_health`, `metrics_api`, `config_api`)
2. **Cloud:** `curl -s https://v2.racingpoint.cloud/api/health/deep` (or pm2 web port) from James .27 (public internet) → `healthy: true` + all 3 checks pass (was: 503 with timeout errors on each check)
3. **Web dashboard browser session** on both venue + cloud topologies → no 503 in browser console for /api/health/deep AJAX calls (if any)

**NOT TESTED (H3 anti-theater compliance):**
- Timing characteristics under load (this is a low-volume health endpoint; not load-tested)
- TLS termination at any cloud reverse-proxy (Nginx / Caddy / direct pm2) — relies on co-located localhost being TLS-free, which is the standard pattern
- Behavior when `API_PROXY_TARGET` env var IS set but points to a wrong host (config-error class; out-of-scope for this fix; sibling concern in `next.config.ts` env var validation gap)
- DPDP / GDPR consent path (unrelated surface)
- Whether kiosk/admin apps have analogous server-side route handlers with the same defect (sibling enumeration — D-MAOR-1 + adjacent-apps audit per MAOR §Q6)

---

## §7 — Sync targets

- `web/src/app/api/health/deep/route.ts` — single-line edit (this RCA's deliverable)
- This RCA file — `.planning/audits/RCA-2026-05-14-deep-health-route-cross-origin.md` (committed)
- LOGBOOK.md — entry
- comms-link/V2-MASTER-STATE.md §S-N close-anchor — DEFERRED to verify-phase next turn (H2 separation; substrate-PR ships first, close-anchor follows verify)
- V2-PROGRESS-MAP — Layer 12 row update if cataloged
- MEMORY.md index — N/A (no doctrine change; G9 #2 sibling memory already landed earlier this turn)

---

End of RCA-2026-05-14-deep-health-route-cross-origin.md.
