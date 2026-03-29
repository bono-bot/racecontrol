# POS 404 Fix — Meta-Audit of Verification Process
**Date:** 2026-03-29 IST
**Self-audit + MMA meta-audit:** GPT-4.1, Gemini 2.5 Pro, DeepSeek R1, MiMo V2 Pro

## Self-Audit Summary (3 Layers)

### Layer 1: Quality Gate — 6 Gaps
| # | Check Missed | Impact |
|---|-------------|--------|
| 1 | `cargo test` not run | Could miss Rust regressions |
| 2 | TypeScript type-check not run post-deploy | Type errors possible |
| 3 | `comms-link/test/run-all.sh` not run | Security gate incomplete |
| 4 | Frontend lint not run | Code quality unchecked |
| 5 | Error boundary chunk load not verified | Error.tsx might not execute |
| 6 | All 30 routes not verified (spot-checked 3) | Missing routes undetected |

### Layer 2: E2E — 10 Gaps
| # | Check Missed | Impact |
|---|-------------|--------|
| 1 | WebSocket from POS not verified | WS might not connect |
| 2 | Pod data population not tested | Billing might show empty state |
| 3 | 404 auto-redirect timer not verified | 3s redirect might not fire |
| 4 | Error boundary not tested with error injection | Unknown behavior on real errors |
| 5 | ChunkErrorRecovery not tested with stale chunks | Core fix untested |
| 6 | Auth idle timeout not tested | 15min timeout behavior unknown |
| 7 | POS actual screen not captured | Playwright != POS Edge |
| 8 | Edge kiosk mode not tested | Different browser behavior |
| 9 | localStorage persistence untested | JWT might not survive restart |
| 10 | Multiple sidebar navigations untested | Navigation errors possible |

### Layer 3: Standing Rules — 4 Violations
| # | Rule | Severity | Status |
|---|------|----------|--------|
| 1 | Verify EXACT behavior, not proxies (Playwright != Edge kiosk) | HIGH | OPEN |
| 2 | Verify from user browser, not server (POS screenshot failed) | HIGH | OPEN |
| 3 | Auto-push + notify Bono | MEDIUM | OPEN |
| 4 | LOGBOOK entry after commit | MEDIUM | OPEN |

---

## MMA Meta-Audit Consensus (4 models)

### P0 — Critical Gaps (all 4 models flagged)

**G1: Not Testing on Actual Edge Kiosk Mode**
Playwright uses headless Chromium. Edge kiosk mode has different: session restore, localStorage persistence, API restrictions, error dialog suppression, navigation controls. The entire E2E verification was done on the WRONG browser.

**How to fix:**
- Enable Edge remote debugging: add `--remote-debugging-port=9222` to kiosk launch
- Use Playwright with `channel: 'msedge'` to connect to actual Edge
- Or: add `powershell` to rc-pos-agent exec allowlist for CopyFromScreen

**Automation:**
```bash
# Edge remote debug kiosk launch
msedge.exe --kiosk http://192.168.31.23:3200/billing --edge-kiosk-type=fullscreen --remote-debugging-port=9222
# Connect from James:
node -e "const {chromium}=require('playwright'); chromium.connectOverCDP('http://192.168.31.20:9222').then(b=>b.contexts()[0].pages()[0].screenshot({path:'pos.png'}))"
```

---

### P1 — High-Risk Gaps (3+ models flagged)

**G2: No Client-Side Hydration Verification**
`curl` returns 200 but doesn't test JavaScript execution. The billing page could return HTML but fail to hydrate (broken chunks, JS errors). Need to verify INTERACTIVE elements work.

**How to test:** Playwright click a "Start Session" button, verify modal opens.
**Automate:** Add assertion: `await page.locator('button:has-text("Start Session")').first().click(); await expect(page.locator('[role="dialog"]')).toBeVisible();`

**G3: Network Partition / Offline Recovery Not Tested**
What happens when POS loses WiFi mid-session? Could ChunkErrorRecovery enter infinite reload loop? Does the app recover when network returns?

**How to test:** Playwright network throttle → offline → reconnect → verify recovery.
**Automate:** `await page.route('**/*', route => route.abort()); await page.waitForTimeout(5000); await page.unroute('**/*');`

**G4: ChunkErrorRecovery Not Tested With Real Stale Chunks**
The core fix (F1) was never tested end-to-end: deploy new build → old page tries navigation → chunk 404 → auto-reload. This is the #1 cause and it was never verified.

**How to test:** Build v1 → load in browser → build v2 → deploy → navigate sidebar → verify auto-reload.
**Automate:** Playwright with two deploys in sequence.

**G5: Deep-Link / Subroute 404 Recovery Not Tested**
Only tested `/billing/nonexistent`. Not tested: `/billing/pricing/nonexistent`, `/settings/nonexistent`, direct URL entry of mistyped routes.

**How to test:** Loop through 10+ invalid URLs, verify all show custom 404 with redirect.

---

### P2 — Medium Gaps (2+ models flagged)

**G6: localStorage Quota / Corruption Scenarios**
try/catch added but never tested with actual quota exhaustion or corrupted JSON.

**G7: Memory Leak from Error Boundary / Timer Accumulation**
POS runs 24/7. Timer accumulation in not-found.tsx or error boundary re-renders could leak memory over days.

**G8: Browser Console Errors Not Captured**
No test captures `console.error` or `console.warn` from the POS browser. Silent JS errors could indicate degradation.

**G9: POS Screen Resolution / CSS Rendering**
Playwright viewport (1920x1080) may not match POS actual resolution. CSS could break on POS hardware.

**G10: No Monitoring / Alerting on Error Boundary Activation**
Error boundaries catch errors silently. No telemetry reports when they fire. Regressions go unnoticed.

**G11: Session/Token Expiry During Recovery**
If JWT expires WHILE the app is in 404/recovery/error state, could create a deadlock.

**G12: Service Worker Cache Invalidation**
No explicit service worker, but Edge may register one internally. Stale cache could persist across deploys.

---

## Recommended Verification Improvements

### Immediate (do now)
1. **Add `powershell.exe` to rc-pos-agent exec allowlist** — enables POS screenshots
2. **Add `--remote-debugging-port=9222` to POS Edge kiosk launch** — enables remote Playwright
3. **LOGBOOK entry for da68eb51** — standing rule compliance
4. **Bono notification via comms-link** — standing rule compliance

### Short-term (this week)
5. **Create `verify-pos.js` Playwright script** that:
   - Connects to Edge on POS via CDP (port 9222)
   - Screenshots billing page
   - Clicks "Start Session" → verifies modal
   - Navigates to invalid URL → verifies custom 404
   - Waits 4s → verifies redirect to /billing
   - Captures console errors
6. **Add to deploy-server.sh:** restart Edge on POS after web app deploy
7. **Route smoke test:** curl all 30 routes, assert 200

### Medium-term (this milestone)
8. **Stale chunk E2E test:** build v1 → load → build v2 → deploy → navigate → verify reload
9. **Network partition test:** disconnect/reconnect POS network
10. **Error boundary telemetry:** log activations to server via POST
11. **localStorage stress test:** fill to quota, verify graceful handling
12. **Add 404/error monitoring:** count error boundary activations in health endpoint

---

## Verification Process Score

| Dimension | Score | Notes |
|-----------|-------|-------|
| Code correctness | 8/10 | 6 fixes, MMA-verified, 2 fixes from feedback |
| Build verification | 6/10 | Build passed but no cargo test, no lint |
| E2E coverage | 4/10 | Playwright works but wrong browser, no interaction tests |
| Standing rules | 5/10 | 2 HIGH violations (proxy verification, wrong browser) |
| Visual verification | 3/10 | Screenshots from wrong browser, no POS screen |
| Monitoring/observability | 2/10 | No telemetry on error boundaries, no alerting |
| **Overall** | **4.7/10** | Code is good but verification was shallow |

The code fixes are sound (MMA-verified). The verification process had significant gaps — primarily testing on the wrong browser (Playwright/Chromium vs Edge kiosk) and no interaction-level testing.
