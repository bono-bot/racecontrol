# Domain Pitfalls: Racing Dashboard UI Redesign

**Domain:** Multi-app Next.js dashboard system — UI overhaul on a live venue operations stack
**Researched:** 2026-03-30
**Confidence:** HIGH — pitfalls drawn from documented incidents in this exact codebase (CLAUDE.md standing rules, MEMORY.md, PROJECT.md, git history) plus verified Next.js/Tailwind v4 behaviour. No hypothetical pitfalls.

---

## Critical Pitfalls

### Pitfall 1: NEXT_PUBLIC_ Env Vars Baked at Build Time — Wrong Value Silently Deployed

**What goes wrong:**
`NEXT_PUBLIC_API_URL` and `NEXT_PUBLIC_WS_URL` are embedded into the JS bundle at `next build` time. If the build runs with `http://localhost:8080` defaults (because `.env.production.local` was missing or wrong), the deployed app makes all API calls to `localhost` on the CLIENT's machine. On the server itself this works fine. From every other machine — POS, kiosk, staff phone, leaderboard display — the app loads but shows no data. The silence is total: no error in the server log, HTTP 200 from the Next.js server, just empty dashboards.

**Why it happens:**
`NEXT_PUBLIC_*` values are NOT runtime environment variables. They are compile-time string replacements. Changing `.env.production.local` after a build has zero effect — the app must be rebuilt. This is counterintuitive for developers used to server-side env vars.

This codebase hit this exact bug: "NEXT_PUBLIC_WS_URL was never set — NEXT_PUBLIC_API_URL was correct so REST worked, but WebSocket defaulted to `ws://localhost:8080` causing 'page loads but no data' on the POS machine for every session until caught." (CLAUDE.md standing rules)

**Consequences:**
- WebSocket telemetry feed shows nothing on any machine except the server
- Leaderboard display (separate TV machine) shows no data
- POS billing dashboard appears to work but has no live pod status
- No errors visible — the fetch silently hits localhost and gets connection refused

**Prevention:**
- Before every build: run `grep -rn NEXT_PUBLIC_ web/src/ kiosk/src/` and verify EVERY var has a value in `.env.production.local` with the LAN IP (192.168.31.23), not localhost
- After any redesign that adds new `NEXT_PUBLIC_` vars: grep the entire new component directory immediately
- Verify from a machine that is NOT the server — SSH to POS or open from James's browser at `.23:3200`. `curl` from the server proves HTML loads, not that WebSocket works

**Detection:**
- App shows data on server but not on POS/kiosk/leaderboard display
- Browser devtools on POS shows WebSocket connection to `ws://localhost:8080` (the give-away)
- Zero errors in server logs despite the client-side failure

**Phase to address:** Phase 1 (component scaffold). Before writing any new component, establish env var audit as a mandatory pre-build step.

---

### Pitfall 2: Standalone Next.js Deploy — Static Files Return 404 After Deploy

**What goes wrong:**
`next build` with `output: "standalone"` creates a `.next/standalone/` directory. The JS and CSS bundles live in `.next/static/`. These are NOT copied into standalone automatically. If the deploy script copies only `.next/standalone/` to the server, every CSS file and JS chunk returns 404. The page loads as unstyled HTML with no interactivity. Health checks still pass — the Next.js server returns 200 for the HTML page. The bug is invisible to monitoring.

This has already happened in this codebase: "kiosk and web dashboard had all static files returning 404 for an unknown duration. Health endpoint showed 'healthy'." (CLAUDE.md)

**Why it happens:**
Next.js standalone intentionally excludes static files from the server bundle — they are meant to be served by a CDN or a static file server alongside the Node process. In this deployment (pm2 + nginx on the same server), the static directory must be manually copied. Two things must happen:
1. `.next/static/` must be copied to `.next/standalone/.next/static/`
2. `public/` must be copied to `.next/standalone/public/`

For a UI redesign with new assets, fonts, or images, failing to re-copy `public/` leaves the new assets absent on the deployed server.

**Consequences:**
- New Tailwind classes compile into CSS that never loads — the page looks broken
- New fonts (Enthocentric, Montserrat) return 404 if added to `public/fonts/`
- New leaderboard images or sponsor logos invisible
- All pages look like unstyled HTML

**Prevention:**
- Deploy scripts MUST include the copy steps: `cp -r .next/static .next/standalone/.next/static` and `cp -r public .next/standalone/public`
- After every deploy, verify with: `curl -I http://192.168.31.23:3200/_next/static/css/app.css` — must return 200, not 404
- Add this curl to the smoke test in the deploy script (currently the smoke test checks 4 endpoints — add a static file check as the 5th)
- Both `web/` and `kiosk/` (basePath: `/kiosk`) must be deployed this way. Kiosk static path is `http://192.168.31.23:3300/kiosk/_next/static/...`

**Detection:**
- Page renders with no CSS (white background, unstyled text)
- Browser devtools shows `net::ERR_FAILED` or 404 for `/_next/static/css/...`
- App "works" on James's dev machine but looks broken on server

**Phase to address:** Phase 1 (deploy pipeline setup). The static file copy step must be in the deployment script before any redesign work ships.

---

### Pitfall 3: outputFileTracingRoot Misconfiguration — Absolute Build Paths Embedded

**What goes wrong:**
Without `outputFileTracingRoot: path.join(__dirname)` in `next.config.ts`, Next.js auto-detects the monorepo root (walks up to find `package.json`, finds `C:\Users\bono\racingpoint\racecontrol\`) and embeds this absolute path in `required-server-files.json` and `server.js`. When the standalone bundle is deployed to `C:\RacingPoint\web\` on the server, the `appDir` field still points to `C:\Users\bono\racingpoint\racecontrol\web\` — a path that does not exist on the server. SSR works (served from memory) but ALL static files serve as 404 because the static file handler looks in the wrong root.

**Why it happens:**
This exact fix is already implemented in both `web/next.config.ts` and `kiosk/next.config.ts`. The risk during a redesign is: adding a new Next.js app (e.g., a dedicated leaderboard display app or a presenter mode app), or if someone inadvertently removes the config setting while refactoring `next.config.ts` to add new options (rewrites, redirects, image domains).

**Consequences:**
Same as Pitfall 2 — all static files 404. But harder to diagnose because the config looks correct to a casual reader.

**Prevention:**
- Never remove `outputFileTracingRoot: path.join(__dirname)` from any `next.config.ts`
- Any NEW Next.js app created during the redesign MUST include this line — it is not optional in this monorepo
- After adding any `next.config.ts` option, verify the line is still present: `grep outputFileTracingRoot */next.config.ts`

**Detection:**
- Check `required-server-files.json` after build: `cat .next/required-server-files.json | grep appDir` — must show the app's own directory, not the repo root or James's machine path

**Phase to address:** Phase 1 (config/scaffold). Verify in the first PR review.

---

### Pitfall 4: Kiosk is Touch-Only — Mouse-Centric UI Patterns Break Silently

**What goes wrong:**
The kiosk (`/kiosk` basePath, port 3300) runs on pods where the only input is a touchscreen. Mouse-specific interactions — hover states, right-click menus, `mouseenter`/`mouseleave` events, tooltips that appear on hover, drag handles without touch equivalents — all fail silently on touch. The designer builds and tests on a laptop trackpad; everything looks fine. The UI ships; customers at pods cannot interact with the redesigned booking wizard or game selector.

**Why it happens:**
Tailwind v4's hover utilities (`hover:`) are mouse-centric. On iOS/Android touch browsers, hover state is emulated on tap-and-hold, but this is unreliable and not the intended UX. For an eSports kiosk in a dark venue, touch targets must be at minimum 44x44px and must respond to tap, not hover.

**Consequences:**
- Redesigned game selection cards show extra info only on hover — invisible on kiosk
- Small filter buttons (track/car selectors) unreachable with fingers
- Dropdown menus that open on hover fail completely

**Prevention:**
- All interactive elements in `kiosk/` must use `onClick` (not `onMouseEnter`) as the primary interaction
- Touch target minimum: 44x44px. Use `min-h-11 min-w-11` (44px = 2.75rem = h-11 in Tailwind)
- Avoid `hover:` for revealing content; use toggled state (`useState`) instead
- Test on an actual touchscreen before marking any kiosk phase complete — NOT in browser devtools touch simulation (it emulates touch events but has different behavior for hover state)
- Any component used in BOTH `web/` (mouse) and `kiosk/` (touch) must handle both — use `@media (hover: hover)` for hover-only styles

**Detection:**
- Kiosk page requires hovering to see any action options
- Touch on the kiosk selects text instead of triggering the button (too small)
- UI works in browser, fails when Uday or staff try on the actual pod touchscreen

**Phase to address:** Phase 2 (kiosk component redesign). Also applies to Phase 1 if shared components are being built.

---

### Pitfall 5: Hydration Mismatch — localStorage/sessionStorage in useState Initializer

**What goes wrong:**
SSR renders the page on the server without browser APIs. If a redesigned component reads from `localStorage` (e.g., to restore a leaderboard filter preference, or persist a selected car) directly in a `useState` initializer, the server renders with `undefined` and the client hydrates with the stored value. React throws a hydration mismatch error. In Next.js 15+ (App Router), this can cause the entire page to re-render or show a blank screen.

**Why it happens:**
This is a standing rule in this codebase ("Next.js hydration: never read sessionStorage/localStorage in useState initializer — use useEffect + hydrated flag"). A UI redesign touching many components is exactly when this rule gets accidentally violated as new components are added.

**Consequences:**
- Leaderboard filter position resets on every page load (if the fix is to just not persist it)
- Or: entire leaderboard page shows hydration error and blank-screens for 1-2 seconds on load

**Prevention:**
- Pattern for persisted state:
  ```tsx
  const [hydrated, setHydrated] = useState(false);
  const [filter, setFilter] = useState("all");
  useEffect(() => {
    setFilter(localStorage.getItem("lb-filter") ?? "all");
    setHydrated(true);
  }, []);
  if (!hydrated) return null; // or skeleton
  ```
- Add ESLint rule or grep to catch `localStorage` in component bodies outside `useEffect`
- Verify: `next build` must complete with 0 hydration errors in the build output

**Phase to address:** Phase 2 (component implementation). Add to code review checklist.

---

### Pitfall 6: Tailwind v4 CSS-First Config — Class Names Work in Dev, Fail in Build

**What goes wrong:**
Tailwind v4 uses a CSS-first configuration (`@theme` in `globals.css`) instead of `tailwind.config.js`. Custom classes like `bg-rp-card`, `text-rp-red`, `border-rp-border` are defined there. If a redesigned component uses a custom color that exists in the old `tailwind.config.js` (from a copy-paste from another project or old docs) but NOT in `globals.css @theme`, the class purges away in production build. The color appears in dev (JIT generates it) but is absent in production.

This codebase already uses v4 CSS-first config in both `web/src/app/globals.css` and `kiosk/src/app/globals.css`. The deprecated orange `#FF4400` class must not be referenced in new components.

**Why it happens:**
Tailwind v4 changed from JS config to CSS `@theme`. References to Tailwind's old color names (e.g. `text-gray-700`, `bg-zinc-800`) still work because v4 ships a compatibility preset. But custom theme tokens MUST be in `globals.css @theme inline` — not in a `tailwind.config.js`.

**Consequences:**
- New leaderboard cards built with a custom `rp-gold` color for first place look correct in `next dev` but have no color in production
- Cards using the deprecated `#FF4400` orange appear in storybook/dev but wrong in production

**Prevention:**
- Add any new design tokens to `globals.css` under `@theme inline` BEFORE writing components that use them
- Never create `tailwind.config.js` for new apps — use `globals.css @theme`
- Deprecated `#FF4400` orange: grep for it before marking any phase complete: `grep -rn "FF4400\|orange" web/src/ kiosk/src/`
- After build: spot-check computed styles on the production URL for new color classes

**Phase to address:** Phase 1 (design token setup). Establish the full token set in `globals.css` before any component work.

---

## Moderate Pitfalls

### Pitfall 7: API Contract Breakage — Renamed Fields in Redesign Break Existing TypeScript Types

**What goes wrong:**
The redesign refactors a component that calls `/api/v1/leaderboards/records` and the developer renames a field in the local TypeScript interface (e.g., `best_lap_ms` → `lapTimeMs`) to match a new naming convention. The type change is local — the Rust backend still returns `best_lap_ms`. TypeScript catches this at compile time only if the type is imported from `packages/shared-types/` (v21.0 shared types). If the developer copies the type locally and renames it, the mismatch is invisible until runtime: the field is `undefined`, lap times show as `NaN` or `0:00.000`.

**Why it happens:**
UI redesigns often "clean up" types to match a design system naming convention. The backend cannot be changed simultaneously (would require Rust rebuild + fleet deploy). Serde silently drops unknown fields and returns `undefined` for missing fields — no runtime error.

**Consequences:**
- Lap times show as `0:00.000` or `NaN` on the leaderboard
- Driver scores show as `0`
- No error in console — the fetch succeeds, the parse succeeds, the field is just `undefined`

**Prevention:**
- All API response types MUST be imported from `packages/shared-types/` (v21.0). Never duplicate them locally
- If a type rename is needed for the new design system, add a UI-layer adapter that maps the shared type to the local display type — never rename the shared type itself
- After any leaderboard or telemetry component change: verify the rendered values match the raw API response (`curl http://192.168.31.23:8080/api/v1/leaderboards/records | jq .` and compare fields)

**Phase to address:** Phase 2 (component refactor). Enforce in code review: no local copies of shared types.

---

### Pitfall 8: WebSocket Reconnect Logic Lost During Redesign

**What goes wrong:**
The existing leaderboard and fleet dashboard components have WebSocket reconnect logic (exponential backoff, cleanup on unmount). A redesign that rewrites a component from scratch may not replicate this logic. The new component opens a WS connection, never cleans it up on unmount, and opens a new connection on every re-render — resulting in multiple concurrent connections, duplicate event handlers, and memory leaks. Or: the component reconnects too aggressively and floods the server's WS handler.

The server's WS handler has a `ws_connect_timeout >= 600ms` requirement (CLAUDE.md audit standing rule). A naive reconnect loop that retries every 100ms can overwhelm the server.

**Consequences:**
- Leaderboard display shows duplicate record-broken events (each WS listener fires)
- Server logs show 50+ simultaneous WS connections from the leaderboard TV
- Component unmounts (navigation), WS stays open, server eventually kills the connection with no client cleanup

**Prevention:**
- WS connection logic must live in a `useRef` + `useEffect` with cleanup: `return () => ws.close()`
- Reconnect delay must be at minimum 1000ms for the first retry, backing off to 30s
- Extract WebSocket into a shared hook (`useWebSocket.ts`) to ensure consistency across redesigned components
- Test: navigate away from the leaderboard page and back 5 times; check server WS connection count stays at 1, not 5

**Phase to address:** Phase 2 (leaderboard component). Extract the WS hook in Phase 1 if possible.

---

### Pitfall 9: Recharts / Dynamic Import Breaks SSR — Blank Chart on First Load

**What goes wrong:**
The existing `TelemetryChart.tsx` component uses `dynamic(() => import("@/components/TelemetryChart"), { ssr: false })` to prevent Recharts from trying to render on the server (Recharts uses `window` and `document`). If a redesign moves or renames this component, or if a new chart component is added without the `ssr: false` flag, the server-side render fails with a `window is not defined` error and the page may crash entirely (with error boundary fallback) or show a blank placeholder permanently.

**Why it happens:**
Recharts (currently `^3.8.1`) is a client-only library. Any file it imports that touches browser globals fails on the server. The `dynamic` + `ssr: false` pattern is the correct workaround but must be applied to every chart component, not just the outer one.

**Consequences:**
- Telemetry chart area shows the spinning loader indefinitely
- In development, this may not appear because `next dev` has different SSR behavior
- Production build (with full SSR) breaks the chart

**Prevention:**
- Any new component importing Recharts or charting libraries MUST use `dynamic` + `ssr: false`
- Add to code review checklist: search for `recharts` in new files that don't have `dynamic` import
- The loading placeholder must always be provided: `loading: () => <ChartSkeleton />`

**Phase to address:** Phase 2 (telemetry/analytics components). Add to PR template.

---

### Pitfall 10: Kiosk basePath Breaks Absolute URLs and Redirects

**What goes wrong:**
The kiosk app has `basePath: "/kiosk"` in `next.config.ts`. This means all client-side routes are prefixed: `/book` becomes `/kiosk/book`, `/settings` becomes `/kiosk/settings`. Internal Next.js `Link` and `router.push()` components handle this automatically. Problems arise when:
1. A redesigned component uses a hardcoded absolute URL string: `href="/book"` — this bypasses the basePath prefix and navigates to the web dashboard route instead
2. A redirect uses `basePath: false` incorrectly (the root redirect in `next.config.ts` already handles `/` → `/kiosk` with `basePath: false`, which is correct and must not be changed)
3. API calls in the kiosk use relative URLs like `/api/something` — these are proxied to the Next.js API route at `/kiosk/api/something`, not the backend server

**Consequences:**
- Tapping "Book Session" navigates to web dashboard (staff view) instead of kiosk booking flow
- Customer is looking at a billing admin panel, not the self-service booking UI
- API calls from the kiosk 404 if using relative paths instead of the explicit `NEXT_PUBLIC_API_URL`

**Prevention:**
- All `Link` `href` props and `router.push()` calls in `kiosk/` must use relative paths (e.g. `/book`), not absolute (Next.js applies basePath automatically to relative paths)
- Never hardcode `/kiosk/book` — basePath is applied automatically
- Never use relative API paths like `/api/v1/...` in kiosk — always use `NEXT_PUBLIC_API_URL + "/api/v1/..."` (the backend is at port 8080, not the kiosk's 3300)
- After any navigation change: verify by clicking through on an actual device at `192.168.31.23:3300/kiosk`

**Phase to address:** Phase 2 (kiosk booking redesign). Verify in the navigation smoke test.

---

### Pitfall 11: Leaderboard Display on TV — Touch Events and Auth Assumptions Wrong

**What goes wrong:**
The `/leaderboard-display` route runs on a dedicated TV (leaderboard display PC, Tailscale: `desktop-*` nodes). It auto-rotates through records. A redesign that adds auth guards, JWT checks, or staff-only gates to this route will break it — the TV has no user logged in and cannot complete an auth flow.

Separately, if the redesign adds `onClick` handlers expecting user interaction, the TV cannot interact (it has no keyboard or mouse — it is a pure display). Adding a "click to see more" interaction to a rotating display panel is a dead end.

**Why it happens:**
The leaderboard display page is a public/unauthenticated display. The web dashboard `AuthGate` component wraps most pages. A redesign that adds `<AuthGate>` to the leaderboard-display route will silently redirect the TV to `/login` on next deploy.

**Consequences:**
- TV at the venue shows the login page instead of leaderboards
- Staff don't notice immediately — the TV is decorative, not operationally critical
- It may stay showing login for hours or days

**Prevention:**
- `/leaderboard-display` must NEVER be wrapped in `AuthGate`
- The page must use `fetchPublic()` (not `fetchApi()` which adds Authorization headers) — this is already the pattern in the existing code
- No interactive elements (click handlers, modals) on the leaderboard display — it is display-only
- After any auth middleware change: verify `curl -s http://192.168.31.23:3200/leaderboard-display` returns HTML, not a redirect to `/login`

**Phase to address:** Phase 1 or whichever phase introduces shared layout wrappers. Auth boundary must be explicit.

---

### Pitfall 12: Standalone Deploy on Windows — PM2 / Scheduled Task Path Issues

**What goes wrong:**
The web dashboard and kiosk both run as scheduled tasks on the server (`192.168.31.23`). The `node .next/standalone/server.js` command is called from the scheduled task. After a redesign and rebuild, the new `server.js` references a different set of chunks. If the deploy copies the new `server.js` but doesn't restart the scheduled task, the old process continues serving the old bundle. The deployment appears to have worked (files are updated) but users see the old UI.

Additionally, on Windows, pm2 (used on Bono VPS for cloud deployments) does not pick up file changes automatically. `pm2 restart web` must be explicitly called.

**Consequences:**
- Deployed new leaderboard design but server still shows old layout for hours
- Health check returns 200 (process is alive), but build date or version shows the old deploy

**Prevention:**
- Deploy script must: (1) copy new files, (2) stop the old process/task, (3) start new process/task, (4) verify the new build is serving by checking a cache-busting URL or the bundle hash in the HTML
- Add a version endpoint: inject `GIT_HASH` or build timestamp into the standalone `server.js` environment at build time, expose via `/api/version` route, and verify it post-deploy
- Bono VPS: `pm2 restart web && pm2 restart admin` (not just one app)

**Phase to address:** Phase 1 (deploy pipeline). The version-verification step must be in the smoke test.

---

## Minor Pitfalls

### Pitfall 13: Enthocentric Font Missing in New Components

**What goes wrong:**
The brand uses Enthocentric for headers. This is a custom font loaded via `public/fonts/` (not Google Fonts). New components in the redesign that use generic Tailwind heading classes without specifying `font-enthocentric` (or whatever the CSS class is) will render in Montserrat instead. The difference is subtle but visible — the header font is part of the Racing Point identity.

**Prevention:**
- Verify the Enthocentric CSS class name (`grep -n "Enthocentric\|enthocentric" web/src/app/globals.css`) and document it as a standing rule for the redesign
- Any `<h1>`, `<h2>`, race name, or position number displaying "racing style" text must use Enthocentric
- After any new page/component: visual check in a browser, not just a code review

**Phase to address:** Phase 1 (design system setup).

---

### Pitfall 14: SIM_TYPES Array Diverges From Backend Enum

**What goes wrong:**
The leaderboard page has a `SIM_TYPES` array (comment: "must match SimType enum in rc-common/types.rs"). If the redesign adds a new game (e.g. "Dirt Rally 2.0") to the frontend `SIM_TYPES` without adding it to the Rust enum, the filter sends an unknown value to the backend. The API returns an error or empty results. Alternatively, if a new game is added to the backend but not the frontend filter, it is invisible on the leaderboard.

**Prevention:**
- `SIM_TYPES` must always be imported from `packages/shared-types/` or generated from the OpenAPI spec (v21.0 OpenAPI covers 66 endpoints including sim types)
- Never duplicate the enum as a local constant in a component file
- After any game is added: update rc-common first, rebuild, then update shared-types, then update frontend

**Phase to address:** Phase 2 (leaderboard filter redesign).

---

### Pitfall 15: Recharts Responsive Container Height Zero on Initial Render

**What goes wrong:**
`ResponsiveContainer` from Recharts requires a parent element with an explicit height. In a redesigned card-based layout, if the parent uses `h-auto` or `flex-1` without a bounded height, `ResponsiveContainer` receives height=0 on initial render and renders an invisible chart. The chart only appears after a window resize triggers a reflow. This is a known Recharts issue.

**Prevention:**
- Always wrap `ResponsiveContainer` in a parent with explicit height: `<div className="h-64">` or `h-[16rem]`, not `h-auto`
- Test the chart with a hard page refresh, not a client-side navigation (client nav may preserve the height from a previous render)

**Phase to address:** Phase 2 (telemetry chart component).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Design system / token setup | Tailwind v4 config confusion (P6), Enthocentric missing (P13) | Define all tokens in `globals.css @theme` before any component work |
| Leaderboard display redesign | WebSocket reconnect lost (P8), leaderboard TV auth guard (P11), SIM_TYPES drift (P14) | Extract WS hook first, keep auth boundary explicit, import shared types |
| Kiosk booking wizard redesign | Touch targets too small (P4), basePath absolute URLs (P10), NEXT_PUBLIC_ vars (P1) | Verify on actual touchscreen, use relative hrefs, pre-build env check |
| Deploy pipeline | Static files not copied (P2), wrong outputFileTracingRoot (P3), no process restart (P12) | Add static copy + version verify to smoke test |
| Telemetry/chart components | Recharts SSR crash (P9), responsive container height zero (P15), missing ssr:false (P9) | Always dynamic import with ssr:false, explicit parent height |
| API type refactor | Shared type rename breaks backend contract (P7), SIM_TYPES local copy (P14) | Import from shared-types, never duplicate |
| Any new NEXT_PUBLIC_ var | Value baked at wrong time (P1) | Audit env vars in .env.production.local before build |

---

## Deployment Checklist (specific to this redesign)

Run before marking any redesign phase as shipped:

```bash
# 1. Env var audit — must have LAN IP, not localhost
grep -n "localhost" web/.env.production.local kiosk/.env.production.local
# Must return 0 matches for NEXT_PUBLIC_ vars

# 2. Build succeeds with 0 type errors
cd web && npm run build 2>&1 | tail -20
cd kiosk && npm run build 2>&1 | tail -20

# 3. Static files present after build
ls web/.next/static/css/ | head -5
ls kiosk/.next/static/css/ | head -5

# 4. Static files copied to standalone
ls web/.next/standalone/.next/static/css/ | head -5
# If missing: cp -r web/.next/static web/.next/standalone/.next/static

# 5. Verify static serving from a non-server machine
curl -I http://192.168.31.23:3200/_next/static/css/app.css
# Must return HTTP 200

# 6. Verify leaderboard display is unauthenticated
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200/leaderboard-display
# Must return 200, not 302

# 7. Verify kiosk basePath redirect
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3300/kiosk/book
# Must return 200

# 8. Deprecated orange check
grep -rn "FF4400\|#ff4400" web/src/ kiosk/src/
# Must return 0 matches

# 9. Verify outputFileTracingRoot in all next.config.ts files
grep outputFileTracingRoot web/next.config.ts kiosk/next.config.ts
# Must appear in both

# 10. Touch target size (spot check in devtools — set device to iPhone 12 Pro)
# All interactive kiosk elements must be >= 44x44px

# 11. WebSocket env var correct in deployed bundle (check HTML source)
curl -s http://192.168.31.23:3200/ | grep -o "NEXT_PUBLIC_WS_URL[^\"]*"
# Should NOT contain "localhost"
```

---

## "Looks Done But Isn't" Checklist

- [ ] **Tested from POS machine or James's browser** — not just from the server itself (NEXT_PUBLIC_ vars only detectable from remote)
- [ ] **Kiosk tested on actual touchscreen** — devtools touch simulation does not accurately test hover state
- [ ] **Static files verified with curl** — not just "build succeeded" (standalone deploy step is separate)
- [ ] **Leaderboard TV page loads without login** — curl the URL without cookies, verify 200 not 302
- [ ] **WebSocket reconnects on disconnect** — disconnect the server briefly and verify the leaderboard reconnects within 30s
- [ ] **New components use shared types from packages/shared-types/** — no local type duplicates
- [ ] **Recharts components wrapped in dynamic import with ssr:false** — check every new chart file
- [ ] **Enthocentric font used on headers** — visual check, not code review
- [ ] **No new tailwind.config.js created** — v4 uses globals.css @theme only
- [ ] **All NEXT_PUBLIC_ vars in .env.production.local** — grep before every build

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| NEXT_PUBLIC_ deployed with localhost | MEDIUM — rebuild required | Set correct LAN IP in .env.production.local, `npm run build`, redeploy static + standalone |
| Static files 404 after deploy | LOW — no rebuild | Copy `.next/static` to `.next/standalone/.next/static`, restart process |
| outputFileTracingRoot wrong | LOW — no rebuild | Edit `required-server-files.json` appDir field manually OR rebuild with correct config |
| Leaderboard TV showing login page | LOW — deploy fix | Remove AuthGate from that route, rebuild, redeploy |
| WS reconnect leak — multiple connections | MEDIUM — code fix required | Extract WS into shared hook, rebuild, redeploy |
| Touch targets too small — kiosk unusable | HIGH — design + code fix | Must enlarge all targets, rebuild kiosk, redeploy, verify on touchscreen |

---

## Sources

- `CLAUDE.md` (racecontrol repo) — NEXT_PUBLIC_ env var bake-time trap, standalone static file 404 incident (2026-03-25), outputFileTracingRoot fix history, Tailwind v4 CSS-first config, hydration localStorage rule, kiosk basePath, Recharts dynamic import pattern, brand identity (#E10600, Enthocentric, Montserrat, deprecated orange)
- `MEMORY.md` — v16.1 cameras dashboard hardcoded array pitfall, kiosk 14-day stale deploy incident (2026-03-28), frontend staleness check in quality gate, NEXT_PUBLIC_WS_URL "page loads but no data" on POS machine
- `web/next.config.ts` + `kiosk/next.config.ts` — outputFileTracingRoot rationale comments, basePath configuration
- `web/package.json` + `kiosk/package.json` — Next.js 16.1.6, React 19.2.3, Tailwind v4, Recharts 3.8.1, socket.io-client 4.8.3
- `web/src/app/globals.css` — Tailwind v4 @theme inline config, CSS custom properties, current color tokens
- `web/src/app/leaderboards/page.tsx` — WS_BASE NEXT_PUBLIC_ pattern, SIM_TYPES array with "must match SimType enum" comment, dynamic Recharts import
- `web/src/lib/api.ts` — fetchPublic vs fetchApi distinction, 30s timeout, 401 redirect behaviour

---
*Pitfalls research for: Racing Dashboard UI Redesign (subsequent milestone)*
*Researched: 2026-03-30 IST*
