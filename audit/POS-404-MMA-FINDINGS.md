# POS Machine 404 — MMA Audit Findings (2 iterations)
**Date:** 2026-03-29 IST
**Iteration 1 Models:** GPT-4.1, Gemini 2.5 Pro, DeepSeek R1, Qwen 3 235B, MiMo V2 Pro
**Iteration 2 Models:** GPT-4.1 (gap pass), MiMo V2 Pro (gap pass), Gemini 2.5 Pro (partial), DeepSeek R1 (pending)

## Executive Summary
The POS Edge kiosk shows a Next.js "404: This page could not be found" page. The root URL `/billing` returns HTTP 200 via curl from POS. All sidebar links return 200. Static assets return 200. **The 404 is a client-side issue occurring after initial page load.**

Most likely cause: **Stale JS chunks after server rebuild** (7/7 model consensus across both iterations).

---

## Iteration 1 — Consensus Findings (5 models)

### F1 — P0: Stale Chunks After Server Rebuild (5/5 consensus)
**Root cause:** When the Next.js web app is rebuilt/redeployed on the server, ALL `_next/static/chunks/` URLs change (new content hashes). If Edge on POS still has the old page loaded in memory, any client-side navigation triggers a fetch for old chunk URLs that no longer exist → 404.

**Reproduce:** Load `/billing` → rebuild web app → click any sidebar link → old chunks 404.

**Fix:**
- Restart Edge kiosk after every web app deploy
- Add version-check WS event: server broadcasts build hash, client auto-reloads on mismatch
- Set `Cache-Control: no-cache, no-store, must-revalidate` for HTML responses

**Verify:** After deploy, check DevTools Network tab for 404s on `.js` chunk files.

---

### F2 — P0: WebSocket URL Defaults to localhost (5/5 consensus)
**Root cause:** `useWebSocket.ts` defaults to `ws://localhost:8080/ws/dashboard` if `NEXT_PUBLIC_WS_URL` not set. POS browser connects to POS's localhost, not server.

**Impact:** Billing page loads but shows "No pods connected". Does NOT directly cause 404, but degrades entire billing experience.

**Fix:** Set `NEXT_PUBLIC_WS_URL=ws://192.168.31.23:8080/ws/dashboard` in `.env.production.local`, rebuild.

**Verify:** From POS, check DevTools Console for "Connected to server" log.

---

### F3 — P1: AuthGate JWT Expiry (5/5 consensus)
**Root cause:** After 15 min idle, `useIdleTimeout` clears JWT → redirect to `/login`. The `/login` route exists (200), so this is a valid redirect. However AuthGate returns `null` (blank screen) while redirecting.

**Real risk:** If localStorage is cleared by Edge kiosk policies, POS stuck on `/login` permanently.

**Fix:** Set a long-lived JWT for POS or make `/billing` a public route.

---

### F4 — P1: Edge Kiosk Session Restore (4/5 consensus)
**Root cause:** Edge restores last session after crash/restart. If Edge was on a 404 page when it crashed, it restores that 404 instead of the `--kiosk` URL.

**Fix:** Add `--disable-session-crashed-bubble` to Edge launch. Clear session data before launch.

---

### F5 — P2: RSC _rsc Prefetch for Non-Existent Route (5/5 consensus)
**Root cause:** Client-side navigation to a non-existent route → RSC fetch returns 404 → Next.js 404 page.

**Fix:** Add custom `not-found.tsx` that auto-redirects to `/billing`.

---

### F6 — P2: Service Worker Caching (3/5 consensus)
Unlikely — no service worker explicitly registered.

### F7 — P3: Sidebar Link Prefetch Caching (2/5 consensus)
Low risk — all sidebar links point to valid routes.

---

## Iteration 2 — Gap Analysis (New Findings)

### F8 — P0: Turbopack Chunk Invalidation During Rebuild (3/3 consensus)
**Root cause:** Turbopack may serve old manifest during rebuild on Windows. POS fetches manifest with old hashes → chunks 404. This is F1 but at the SERVER level during deploy, not just client cache.

**Fix:** Atomic deployment: build to shadow dir, swap symlinks. Never serve old and new assets simultaneously.

---

### F9 — P0: Deployment Race Condition (2/3 consensus)
**Root cause:** During deploy, old chunk files deleted before new ones available. POS fetches between file removal and new write → 404.

**Fix:** Use atomic deploy (shadow dir + symlink swap). Only reload server after all assets present.

---

### F10 — P1: Missing Error Boundaries → 404 Cascade (3/3 consensus)
**Root cause:** No `error.tsx` at root or `/billing` segment. If a child component throws during render, Next.js falls through to root 404 instead of showing error UI. POS shows "404" when it should show "Something went wrong."

**Fix:** Add `error.tsx` to root and billing segments:
```tsx
// app/error.tsx
'use client'
export default function RootError({ error, reset }) {
  return <div><h2>Something went wrong</h2><button onClick={reset}>Retry</button></div>
}
```

---

### F11 — P1: Concurrent Auth Redirect Race (2/3 consensus)
**Root cause:** Both AuthGate and useIdleTimeout can call `router.push("/login")` simultaneously → router enters inconsistent state → 404 on next navigation.

**Fix:** Debounce redirects with a "logout in progress" flag.

---

### F12 — P1: Edge Kiosk localStorage Volatility (2/3 consensus)
**Root cause:** Edge kiosk may clear localStorage on crash/restart. JWT wiped → POS stuck on `/login` permanently until manual re-auth.

**Fix:** Fallback to sessionStorage or IndexedDB. Or: auto-login for POS IP.

---

### F13 — P2: localStorage Quota Exceeded (2/3 consensus)
**Root cause:** If localStorage fills up (~5MB), `setItem()` throws QuotaExceededError. Auth code doesn't catch → component crashes → blank screen.

**Fix:** Wrap all localStorage access in try/catch.

---

### F14 — P0/P1: Windows Loopback Network Isolation (2/3 consensus)
**Root cause:** Edge kiosk may have loopback restrictions blocking `localhost` WebSocket connections. Windows Firewall per-path rules may reset after rebuild.

**Fix:** Always use explicit IP (192.168.31.23), never `localhost`. Create persistent firewall rules.

---

### F15 — P2: AuthGate Returns null (Blank Screen) (2/3 consensus)
**Root cause:** AuthGate returns `null` while checking auth or redirecting. No loading UI → blank screen perceived as 404.

**Fix:** Add skeleton loading state instead of returning null.

---

## Quick Debug Checklist (for future 404s on POS)

1. **Was the web app recently rebuilt/deployed?** → Restart Edge on POS (stale chunks)
2. **Is JWT present?** → `localStorage.getItem('rp_staff_jwt')` in DevTools
3. **Is WebSocket connected?** → Check DevTools Console for "Connected to server"
4. **What URL is Edge showing?** → Enable remote debugging: `edge://inspect` from James PC
5. **Did Edge crash and restore?** → Check if URL is stale/non-existent
6. **Are static assets loading?** → `curl -s -o /dev/null -w '%{http_code}' http://192.168.31.23:3200/_next/static/chunks/<any-hash>.js`
7. **Is the server running?** → `curl -s http://192.168.31.23:3200/billing`
8. **Is there an error boundary?** → Check `error.tsx` exists in app root and billing segment

### F16 — P1: Turbopack Dev Artifacts in Production (DeepSeek R1 iter2)
**Root cause:** Turbopack leaves `.turbopack` dirs and HMR artifacts in `.next/static` during build. On restart, POS loads stale paths → 404.

**Fix:** Add cleanup to build script: `find .next/static -name ".turbopack" -type d -exec rm -rf {} +`

---

### F17 — P0: Windows Intranet Zone Auto-Proxy (WPAD) (DeepSeek R1 iter2)
**Root cause:** Edge on Windows detects LAN as "intranet" and enables WPAD proxy detection. During the 5s proxy lookup timeout, WebSocket connections stall. AuthGate may redirect before WS connects.

**Fix:** Disable auto-proxy on POS: Group Policy `ProxySettings->ProxyMode = "direct"` or Edge flag.

**Verify:** Check `edge://settings/?search=proxy` — "Automatically detect settings" should be OFF.

---

## Immediate Actions (Priority Order)
1. **Restart Edge on POS** — resolves stale chunk 404 immediately
2. **Set NEXT_PUBLIC_WS_URL** — fix localhost WS default
3. **Add error.tsx** to root + billing — prevent error → 404 cascade
4. **Add Edge restart to deploy script** — prevent future stale chunks
5. **Add version-check WS event** — auto-reload on deploy mismatch
6. **Check/set JWT on POS** — ensure AuthGate isn't blocking
7. **Add `--disable-session-crashed-bubble`** to Edge kiosk launch
