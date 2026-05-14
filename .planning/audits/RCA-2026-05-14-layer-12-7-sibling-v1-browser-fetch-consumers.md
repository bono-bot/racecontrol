# §S-146 RCA — 7 sibling V1-pattern browser-fetch consumers in `web/src/`

**Status:** AUTHORED-AWAITING-CAPTAIN-DISPOSITION
**Authored:** 2026-05-14 ~17:55 IST · james
**Class:** Layer 12 ops-hygiene refactor · V2-era files post-2026-05-09 → FULL 5-section §S-146 RCA per [[v1-dependent-v2-rca-before-proceeding]] (NOT §S-186 fast-lane eligible per [[v1-antipattern-fix-eligibility-check]] G9 #4 — deployment-topology axis check fires YES on URL construction + port exposure + same-origin transition + protocol handling)
**Composes-with:** PR #80 v2 commit `edca0333` (admin login same-origin + Next.js server-side rewrite · this RCA extends to 7 sibling consumers · scope-superset of bono PR #80 v2 RCA §2 follow-up call-out)
**Captain decision queue:**
- **D-7sib-1** Substrate PR scope (single PR for all 7 OR per-file PRs) — bilateral coordination ask
- **D-7sib-2** Sequence relative to PR #80 v2 merge (after-v2-merge OR include-rewrite-block-and-supersede)
- **D-7sib-3** Per-PR Captain merge auth (foundational-boundary class per G9 #4)

---

## §1 Boundary map

**7 browser-side V1-pattern consumers in `web/src/`** (independent enumeration via `grep -rnE "NEXT_PUBLIC_API_URL" web/src` from James .27 · supersedes bono PR #80 v2 RCA §2 4-sibling count · enumeration-correction class):

| # | File:Line | Current pattern | Class |
|---|---|---|---|
| 1 | `web/src/lib/api.ts:3` | `const API_BASE = process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"` | **SHARED HELPER** `fetchApi<T>()` · highest-leverage fix (~15 downstream consumers per grep `fetchApi\|from.*lib/api`) |
| 2 | `web/src/lib/api/metrics.ts:9` | `const API_BASE = process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"` | Typed metrics client (sibling helper) |
| 3 | `web/src/components/Sidebar.tsx:104` | inline `${process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"}/api/v1/health` | Server health poll (15s cadence · GET only) |
| 4 | `web/src/components/LeaderboardTable.tsx:107` | inline `const apiBase = process.env.NEXT_PUBLIC_API_URL \|\| "http://192.168.31.23:8080"` | **Hardcoded LAN IP fallback** (worse class than localhost · already-leaked-to-bundle attacker fingerprint) |
| 5 | `web/src/app/book/page.tsx:5` | `const API_BASE = process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"` | Booking page |
| 6 | `web/src/app/leaderboard-display/page.tsx:341` | inline `const API_BASE = process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"` | Leaderboard display (inline · function-scope const) |
| 7 | `web/src/app/spectator/circuit/page.tsx:7` | `const API_BASE = process.env.NEXT_PUBLIC_API_URL \|\| "http://localhost:8080"` | Spectator circuit page |

**Out-of-scope sites referenced for context:**

- `web/src/app/login/page.tsx:8` — PR #80 (v1 `e0c996af` / v2 `edca0333` · superseded by relative URL · NOT in scope of this RCA)
- `web/src/app/api/health/deep/route.ts:13` — Next.js SERVER-SIDE route handler (runs in Node.js context · not browser-side · uses NEXT_PUBLIC_API_URL as pattern-misuse but doesn't trigger deployment-topology browser-fetch class · separate RCA scope if pursued)

**Deployment topology:** `web/` Next.js app serves on Server .23 :3200 (customer POS) + :3201 (admin dashboard) per CLAUDE.md Server Services table — one binary, two ports. Cloud Bono VPS runs same `web/` build via pm2 `racecontrol-web` (cloud admin access path). Multi-topology surface: venue LAN HTTP + cloud HTTPS + Tailscale.

**Foundational boundary classification:** Per [[v1-antipattern-fix-eligibility-check]] G9 #4 rule — these fixes change browser-visible URL construction + remove port exposure + transition cross-origin → same-origin + are deployment-topology choices. Foundational-boundary class. §S-186 fast-lane INVALID. Full §S-146 RCA required (this file) + per-PR Captain merge auth per [[v1-dependent-v2-rca-before-proceeding]].

---

## §2 Inherited-issue catalogue

V1 failure modes touching this boundary (per PR #80 v2 RCA §2 + bono MMA-VERIFY adversarial findings):

| I# | Failure mode | Surface(s) affected |
|---|---|---|
| I-1 | V1-era `NEXT_PUBLIC_API_URL` polyfill resolves to `undefined` in browser when env var not baked at Turbopack build time | All 7 sites |
| I-2 | Cross-origin browser fetch from web origin (`:3200`/`:3201`) to backend `:8080` introduces CORS preflight burden | All 7 sites |
| I-3 | HTTPS protocol mismatch when admin/web served via HTTPS (cloud Bono VPS · future TLS-terminated venue) — `https://host:8080` mixed-content blocked | All 7 sites · same MMA-VERIFY finding #1 as PR #80 v1 |
| I-4 | Cloud Bono VPS reverse proxy at web port doesn't expose `:8080` externally → cloud regression | All 7 sites · same MMA-VERIFY finding #2 as PR #80 v1 |
| I-5 | IPv6 literal hosts produce invalid URLs when constructed via `${hostname}:8080` patterns (missing brackets) | Mitigated for 7 sites (they use env-var or localhost fallback, not `${window.location.hostname}` construction) — same class as PR #80 v1 MMA-VERIFY finding #3 |
| I-6 | Phishing surface — attacker-clone page in compromised network could POST to attacker-controlled `:8080` if URL is constructed from env | Mitigated for GET-only (sites 3 health-poll · 4 leaderboards · 6 display · 7 spectator) · MORE-LOAD-BEARING for state-changing endpoints (1 shared helper · 2 metrics · 5 booking) |
| I-7 | Hardcoded LAN IP `192.168.31.23` in client bundle (site #4 LeaderboardTable.tsx:107) leaks venue infrastructure fingerprint to attacker reconnaissance | Site 4 ONLY (worse class than localhost fallback) |
| I-8 | "Mirror kiosk" antipattern carry-forward — kiosk pattern (`window.location.hostname` + :8080) is V1 LAN-only-single-host-backend pattern; carries forward improperly to multi-topology web/ surfaces | Latent risk for future browser-fetch additions to web/ — class precedent set by PR #80 v1 mine; G9 #4 anchor closes the door |

**Sibling-but-distinct class (out-of-RCA-scope · separate Layer 12 RCA candidate):**

- I-WS — 6 hardcoded WS URL fallbacks in web/ (+1 in kiosk/) per bono §S-324 §7 PWA WS inventory. WebSocket protocol requires `ws://${window.location.hostname}` construction (NOT same-origin proxy via Next.js rewrite — WS not supported by Next.js rewrites). Different fix-pattern; sibling Layer 12 RCA when surfaced. Cross-referenced as forward-channel item.

---

## §3 Past-bug disposition

| I# | Disposition | Citation |
|---|---|---|
| I-1 | PATCHED-ONLY (kiosk uses `window.location.hostname` fallback at `kiosk/src/hooks/useKioskSocket.ts:22-26`; admin login PR #80 v1 mirrored this pattern · v2 supersedes with same-origin) — for web/ 7 sites: UNRESOLVED pre-this-PR | PR #80 v1 `e0c996af` · PR #80 v2 `edca0333` |
| I-2 | UNRESOLVED for 7 sites (CORS preflight burden still active) · ROOT-CAUSED for login by PR #80 v2 same-origin pattern | bono PR #80 v2 RCA §4 |
| I-3 | ROOT-CAUSED for login by PR #80 v2 (same-origin inherits page's HTTPS context) · UNRESOLVED for 7 sites pre-this-PR | bono MMA-VERIFY finding #1 |
| I-4 | ROOT-CAUSED for login by PR #80 v2 (Next.js server-side rewrites to `${API_PROXY_TARGET \|\| "http://localhost:8080"}` · backend internal-only) · UNRESOLVED for 7 sites pre-this-PR | bono MMA-VERIFY finding #2 + bono RCA §4 |
| I-5 | NOT-APPLICABLE for 7 sites (no hostname-construction patterns) · ROOT-CAUSED-BY-DESIGN for login via PR #80 v2 | bono MMA-VERIFY finding #3 |
| I-6 | UNRESOLVED for state-changing sites (1 shared helper · 2 metrics · 5 booking) · NOT-APPLICABLE for GET-only sites · ROOT-CAUSED for login by PR #80 v2 | bono MMA-VERIFY finding #4 |
| I-7 | UNRESOLVED — site 4 LeaderboardTable.tsx:107 leaks `192.168.31.23` to bundle | NEW — not surfaced by bono PR #80 v2 RCA |
| I-8 | ROOT-CAUSED structurally by [[v1-antipattern-fix-eligibility-check]] memory anchor (G9 #4 · CANDIDATE-N1 · 2026-05-14 · this session) — future "mirror kiosk" attempts gate on deployment-topology axis check | §S-326 §9 G9 anchor + memory file `feedback_v1_antipattern_fix_eligibility_check_20260514.md` |

**Critical past-bug disposition note (PR #80 v2 RCA §2 enumeration correction):**

bono PR #80 v2 RCA §2 named 4 sibling consumers. Independent enumeration from James .27 found 7. **Sites bono missed:**
- Site 5 `web/src/app/book/page.tsx:5`
- Site 6 `web/src/app/leaderboard-display/page.tsx:341`
- Site 7 `web/src/app/spectator/circuit/page.tsx:7`

bono enumeration also off-by-3 on metrics.ts (cited `:6` · actual `:9` — non-load-bearing typo). Bilateral receipt-verify caught the undercount; AMPLIFIER discipline rubric N=3 application this session (bono caught my v1 V1-pattern via MMA-VERIFY · this catches bono's enumeration undercount via Grep). Rubric works bidirectionally.

---

## §4 V2-alignment delta

**V2 doctrine (per PR #80 v2 RCA §4 + [[v1-antipattern-fix-eligibility-check]] · ratified):**

> Same-origin browser fetch with server-side rewrite proxy. Browser fetches `/api/:path*` (relative URL). Next.js server (`:3200`/`:3201` process) intercepts via `rewrites()` and proxies to `${API_PROXY_TARGET || "http://localhost:8080"}/api/:path*`. Works uniformly across venue LAN HTTP + cloud HTTPS + Tailscale + IPv6 + HTTPS-future TLS-termination because the browser never sees the backend port.

**Current state (post-PR #80 v2 if merged · pre-this-PR for 7 siblings):**
- 7 sites use cross-origin `${API_BASE}/api/...` with `localhost` fallback (6 sites) or hardcoded LAN IP `192.168.31.23` fallback (site 4)
- Each site is independently susceptible to all 6 unresolved inherited issues (I-2 / I-3 / I-4 / I-6 / I-7 / I-8 latent)

**V2-alignment delta:**
- 7 sites replace `const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://..."` → `const API_BASE = ""` (relative URL · empty-string concat preserves existing `${API_BASE}/api/...` template-literal call-sites unchanged)
- Site 3 Sidebar.tsx:104 inline construction simplifies to `/api/v1/health` directly (no template substitution needed)
- Site 4 LeaderboardTable.tsx:107 inline `apiBase` const same treatment as Site 3 (special-case · no template literal change needed if direct `/api/v1/leaderboards${qs}`)
- Next.js `web/next.config.ts` gains `async rewrites()` block proxying `/api/:path*` to `${API_PROXY_TARGET || "http://localhost:8080"}/api/:path*` (same shape as PR #80 v2 already added · MERGES IF PR #80 v2 merged FIRST · CONFLICTS IF this PR adds same block)
- Env-var rename: `NEXT_PUBLIC_API_URL` (build-time browser-bundle baked) → `API_PROXY_TARGET` (runtime server-side env · NOT browser-exposed) — same as PR #80 v2

**Sequence option matrix (D-7sib-2):**

| Path | Order | Pros | Cons |
|---|---|---|---|
| Path-A | PR #80 v2 merges → 7-sibling PR rebases on main with rewrites() already present → 7-sibling fix only touches 7 source files | Clean Git history · no rewrite-block duplication risk · matches "small PR principle" | Sequential dependency · 7-sibling PR can't ship until PR #80 v2 lands |
| Path-B | 7-sibling PR includes 7 source fixes + adds rewrites() block (would conflict with PR #80 v2) | Decoupled from PR #80 v2 timing · could land first if Captain prefers | Merge conflict with PR #80 v2 · would supersede PR #80 v2 if landed first (force PR #80 v2 to rebase on its own auth) |
| Path-C | Merge PR #80 v2 + 7-sibling PR into combined "admin-login + 7-sibling browser-fetch V2-alignment" PR | Single Captain merge auth · single deploy event · atomic V2-alignment for all 8 sites (login + 7 sibling) | Larger blast radius if regression · harder rollback |

**Recommendation:** Path-A (sequential · simplest). Path-C if Captain prefers atomic auth + atomic deploy.

---

## §5 V2-framed proposal

### §5.1 Substrate change (browser-fetch surfaces)

7 source-file edits (~12 LOC total):

```typescript
// BEFORE (all 7 sites · same pattern with localhost or LAN-IP fallback):
const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";  // or "http://192.168.31.23:8080"

// AFTER (V2-aligned same-origin):
const API_BASE = "";  // relative URL · next.config.ts rewrite handles routing
```

Site-specific notes:
- **Site 3 Sidebar.tsx:104** — inline construction: change `${process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080"}/api/v1/health` → `/api/v1/health` (no const needed)
- **Site 4 LeaderboardTable.tsx:107** — inline `const apiBase`: change fallback to `""` (preserves call-site template literal)
- **Site 6 leaderboard-display/page.tsx:341** — inline function-scope const: same pattern as Site 4

### §5.2 Next.js config change

If Path-A (PR #80 v2 merges first): no `next.config.ts` change in this PR.
If Path-B or Path-C: include `rewrites()` block from PR #80 v2 commit `edca0333`:

```typescript
// web/next.config.ts (Path-B/C only · skip in Path-A)
async rewrites() {
  return [
    {
      source: "/api/:path*",
      destination: `${process.env.API_PROXY_TARGET || "http://localhost:8080"}/api/:path*`,
    },
  ];
}
```

### §5.3 Deploy parity (per DEPLOY PARITY universal rule)

Both targets must redeploy:
- Server .23 `web` build (serves :3200 + :3201 from same binary)
- Bono VPS pm2 `racecontrol-web` (cloud admin/POS path)

Existing kiosk separate-deploy path unchanged (kiosk doesn't share `web/` build).

### §5.4 5Q mechanism-trust check (per [[mechanism-trust-check-upstream-of-fix-rca]])

- **Q1 Behavioral test exists?** Manual: `curl :3200/api/v1/health` returns JSON (not HTML 404) before + after fix. Automated: would need Playwright per existing screenshot enforcement hook.
- **Q2 Behavioral test runs in CI?** Existing Playwright config at `web/playwright.config.ts` — verify these 7 sites covered.
- **Q3 Test gate against silent regression?** Yes — Playwright would catch any of 7 sites breaking.
- **Q4 Production-evidence parity?** Bono VPS web access path via cloud admin DNS — must verify cloud admin can read all 7 surfaces (Sidebar health-poll · book · leaderboard-display · spectator/circuit · metrics · LeaderboardTable · shared fetchApi consumers via downstream).
- **Q5 Rollback path?** Single PR revert · API_BASE constants restorable from git. **PASS** all 5.

### §5.5 Anti-pattern test assertions

Added to existing test suite (or new spec file):

```typescript
// test: no V1 cross-origin API_BASE pattern in web/src
test("no NEXT_PUBLIC_API_URL fallback to absolute URL in web/src/**", () => {
  const matches = grepWebSrc("NEXT_PUBLIC_API_URL.*http://");
  expect(matches).toHaveLength(0);
});

// test: no hardcoded LAN IP in web/src bundle
test("no '192.168.31.' literal in web/src/ source", () => {
  const matches = grepWebSrc("192\\.168\\.31\\.");
  expect(matches).toHaveLength(0);
});
```

Composes-with [[v1-antipattern-fix-eligibility-check]] V-1 verify-by ("Zero §S-186 fast-lane claims in next 30d on fixes that change browser-visible URL construction").

---

## §6 Captain decision queue

- **D-7sib-1** Substrate PR scope: single PR for all 7 sites OR per-file PRs OR Path-C combined-with-PR-#80-v2?
- **D-7sib-2** Sequence path: A (after PR #80 v2 merge) OR B (independent · merge conflict risk) OR C (combined atomic)?
- **D-7sib-3** Per-PR Captain merge auth: foundational-boundary class per G9 #4 → Captain merge gate retained (1 auth per PR)
- **D-7sib-4** James-LEAD vs bono-LEAD authoring: PR #80 v2 was bono-LEAD; 7-sibling is sibling scope; either could lead with bilateral AMPLIFIER on the other

---

## §7 NOT TESTED / forward channel

- Playwright screenshot coverage on all 7 surfaces post-fix (existing screenshot enforcement hook flags `web/src/app/login/page.tsx` already · 7-sibling adds another 6 surfaces · same hook should cover or pattern needs broadening)
- Cloud-side admin path verify post-fix (manual: `curl https://racingpoint.cloud/api/v1/health` from external browser via cloud admin DNS)
- WS sibling-class RCA (6 web/ + 1 kiosk WebSocket URLs per bono §S-324 §7 — different protocol fix-pattern · separate Layer 12 RCA when surfaced)
- Server-side route handler `app/api/health/deep/route.ts:13` (different class · server-side fetch · separate RCA scope if pursued)
- Anti-pattern test assertions (§5.5) — currently authored in RCA but not in test suite (gates on substrate PR)
- §S-N close-anchor in V2-MASTER-STATE.md citing this RCA (next §S-N when authoring this PR's substrate or when AMPLIFIER lands)

---

## §8 Composes-with

- **PR #80 v2 commit `edca0333`** — sibling fix · same V2-pattern · this RCA extends to 7-sibling scope · enumeration-correction from 4→7
- **PR #80 v2 RCA §2** (in `edca0333` commit body) — original 4-sibling call-out · this RCA supersedes with corrected enumeration
- [[v1-antipattern-fix-eligibility-check]] G9 #4 memory anchor (CANDIDATE-N1 · 2026-05-14) — this RCA IS the application of the new rule to 7 sibling sites
- [[v1-dependent-v2-rca-before-proceeding]] (parent doctrine — 5-section template + foundational-boundary classification)
- [[v1-v2-gap-flow-plan-doctrine]] B3 LIVE-BLOCKERS dual-section authoring (Forward audit class — these 7 sites are V2-doctrine-required surfaces · Backward audit class — bono MMA-VERIFY found the pattern on login)
- [[amplifier-discipline-rubric]] N=3 application this session (bidirectional · my Grep caught bono's 4-vs-7 undercount + bono's MMA-VERIFY caught my v1 antipattern)
- [[mechanism-trust-check-upstream-of-fix-rca]] (5Q applied in §5.4 · PASS all 5)
- [[grep-all-behavior-paths-before-planning]] (Grep enumeration of NEXT_PUBLIC_API_URL across web/src/ closes scope before authoring)
- `racecontrol/.planning/audits/RCA-2026-05-13-row-1.13-billing-finalize-idempotency.md` (sibling §S-146 RCA from §S-248 batch · convention precedent for canonical RCA path + 5-section template)
- §S-326 §10 #7 (this RCA was queued as forward-channel item · now landed · ready for §S-N anchor)
- racecontrol PR #82 + comms-link PR #15 (doctrine PRs · OPEN MERGEABLE · gates on Captain merge · this RCA is a CONSEQUENCE of doctrine PRs landing)

---

## §9 Push class

This RCA file is doctrine-class authoring (`.planning/audits/` per §S-256 + §S-248 convention). **AUTONOMOUS-PUSH ELIGIBLE** per [[sn-close-anchor-push-standing-rule]] §S-186 fast-lane analog for RCA-authoring class (no schema change · no source code change · pure analysis artifact). Substrate PR (the actual 7-file edit) is CAPTAIN-STAKE per §6 D-7sib-3 above.

---

End of RCA-2026-05-14-layer-12-7-sibling-v1-browser-fetch-consumers.
