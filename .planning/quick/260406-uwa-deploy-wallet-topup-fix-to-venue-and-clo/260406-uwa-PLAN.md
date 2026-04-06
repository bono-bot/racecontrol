---
phase: 260406-uwa
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - "server .23: C:\\RacingPoint\\web\\.next\\*"
  - "cloud (Bono VPS): web dashboard rebuild"
autonomous: false
requirements: [DEPLOY-01, DEPLOY-02, VERIFY-01]

must_haves:
  truths:
    - "Wallet top-up with CASH method succeeds (HTTP 200) on venue server .23:3200"
    - "Wallet top-up with CASH method succeeds (HTTP 200) on cloud racingpoint.cloud"
    - "Playwright screenshot of billing page shows successful top-up flow (no 422 error)"
  artifacts:
    - path: "server .23: C:\\RacingPoint\\web\\.next\\standalone\\server.js"
      provides: "Rebuilt web dashboard with fixed WalletTopupModal"
    - path: "cloud: web dashboard"
      provides: "Deploy-parity rebuilt web dashboard"
    - path: "tests/screenshots/ (Playwright capture)"
      provides: "Visual proof of working top-up flow"
  key_links:
    - from: "WalletTopupModal.tsx"
      to: "POST /api/v1/billing/wallet/topup"
      via: "fetch body with field name 'method'"
      pattern: "method.*CASH|UPI|CARD"
    - from: "venue .23:3200"
      to: "venue .23:8080"
      via: "Next.js API proxy to racecontrol backend"
    - from: "cloud web"
      to: "cloud :8080"
      via: "Same proxy path on cloud"
---

<objective>
Deploy the wallet top-up fix (commit f01bd396: `payment_method` -> `method` in WalletTopupModal.tsx) to venue server .23 and cloud (Bono VPS). Verify with Playwright screenshot that the 422 error is gone.

Purpose: Customers and staff can top up wallets again without "Network error" on both venue and cloud.
Output: Working wallet top-up on both environments, Playwright screenshot evidence.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@web/src/components/WalletTopupModal.tsx
@web/next.config.ts

Standing rules (from CLAUDE.md):
- DEPLOY PARITY: every local deploy MUST also deploy to cloud
- Frontend standalone deploy requires `.next/static` copied into `.next/standalone/`
- `outputFileTracingRoot: path.join(__dirname)` in next.config.ts prevents stale appDir paths
- After deploy, verify from a machine that is NOT the server
- After every dashboard rebuild/deploy, curl one `_next/static/` URL to confirm static serving
</context>

<tasks>

<task type="auto">
  <name>Task 1: Deploy rebuilt web dashboard to venue server .23</name>
  <files>server .23: C:\RacingPoint\web\.next\*</files>
  <action>
  The web dashboard has already been built locally (`npm run build` completed in `web/`).

  1. **Package the build for transfer:**
     ```
     cd C:/Users/bono/racingpoint/racecontrol/web
     tar -czf /c/Users/bono/racingpoint/deploy-staging/web-next.tar.gz .next
     ```

  2. **Copy to venue server .23 via SCP:**
     ```
     scp /c/Users/bono/racingpoint/deploy-staging/web-next.tar.gz ADMIN@100.125.108.37:C:/RacingPoint/web-next.tar.gz
     ```

  3. **SSH to server, stop the web app, extract, fix standalone static, restart:**
     ```
     ssh ADMIN@100.125.108.37
     ```
     On server:
     - Kill the running Next.js web process: `taskkill /F /IM node.exe /FI "WINDOWTITLE eq web*"` or find PID via `netstat -ano | findstr :3200`
     - Backup existing: `cd /d C:\RacingPoint\web && ren .next .next-prev`
     - Extract: `tar -xzf C:\RacingPoint\web-next.tar.gz -C C:\RacingPoint\web\`
     - Copy static into standalone: `xcopy /E /I /Y C:\RacingPoint\web\.next\static C:\RacingPoint\web\.next\standalone\.next\static`
     - Restart via schtask: `schtasks /Run /TN StartWebDashboard` (or whatever the task name is — check with `schtasks /Query /FO LIST | findstr -i web`)
     - If no schtask exists, start manually: `cd /d C:\RacingPoint\web\.next\standalone && start "" node server.js`

  4. **Verify static serving works:**
     ```
     curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200/_next/static/ 
     ```
     Any `_next/static/` URL returning 200 (not 404) confirms static files are served correctly.

  5. **Verify the fix — test the actual top-up endpoint:**
     ```
     curl -s -X POST http://192.168.31.23:8080/api/v1/billing/wallet/topup \
       -H "Content-Type: application/json" \
       -d '{"customer_id":"test","amount_paise":50000,"method":"CASH"}' 
     ```
     Should NOT return 422 "missing field method". (May return 401/404 for test customer — that is fine, proves field parsing works.)

  6. **Cleanup:** `del C:\RacingPoint\web-next.tar.gz` on server. Keep `.next-prev` for 72hr rollback.
  </action>
  <verify>
    <automated>curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200 | grep -q "200"</automated>
  </verify>
  <done>Web dashboard at .23:3200 is running with the fixed WalletTopupModal. Static files serve (not 404). Top-up POST with "method" field does not return 422.</done>
</task>

<task type="auto">
  <name>Task 2: Deploy web dashboard to cloud (Bono VPS) — deploy parity</name>
  <files>cloud: web dashboard rebuild</files>
  <action>
  Use comms-link relay to execute on Bono VPS. The cloud needs git pull + rebuild + restart.

  1. **Git pull on cloud:**
     ```
     curl -s -X POST http://localhost:8766/relay/exec/run \
       -H "Content-Type: application/json" \
       -d '{"command":"git_pull","reason":"deploy wallet topup fix f01bd396"}'
     ```

  2. **Rebuild web dashboard on cloud:**
     Write a JSON file for the chain command, then execute:
     ```json
     {
       "steps": [
         {"command": "run_command", "args": {"command": "cd /root/racingpoint/racecontrol/web && npm run build", "timeout": 120}},
         {"command": "run_command", "args": {"command": "cp -r /root/racingpoint/racecontrol/web/.next/static /root/racingpoint/racecontrol/web/.next/standalone/.next/static", "timeout": 30}},
         {"command": "run_command", "args": {"command": "pm2 restart racingpoint-web || pm2 restart racingpoint-dashboard", "timeout": 30}}
       ]
     }
     ```
     Execute via: `curl -s -X POST http://localhost:8766/relay/chain/run -H "Content-Type: application/json" -d @chain-cloud-web.json`

  3. **Verify cloud web dashboard is running:**
     ```
     curl -s -o /dev/null -w "%{http_code}" https://racingpoint.cloud:3200/
     ```
     Or check via relay: `curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"run_command","args":{"command":"curl -s -o /dev/null -w \"%{http_code}\" http://localhost:3200/"}}'`

  Per CLAUDE.md DEPLOY PARITY standing rule: every local deploy MUST also deploy to cloud. This task ensures parity.
  </action>
  <verify>
    <automated>curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"run_command","args":{"command":"curl -s -o /dev/null -w %{http_code} http://localhost:3200/"}}' | grep -q "200"</automated>
  </verify>
  <done>Cloud web dashboard rebuilt from commit f01bd396, pm2 restarted, serving on cloud :3200. Deploy parity achieved.</done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>Wallet top-up fix deployed to both venue (.23:3200) and cloud. The WalletTopupModal now sends "method" instead of "payment_method", matching the server's TopupRequest struct.</what-built>
  <how-to-verify>
    1. Open billing page: `http://192.168.31.23:3200/billing` (from James .27 or POS, NOT from server itself)
    2. Select any customer with a wallet
    3. Click "Top Up" / "Add Credits"
    4. Enter 500 credits, select CASH as method
    5. Click "Add" / submit
    6. Expected: Success (balance increases), NO "Network error" or 422
    7. Playwright screenshot captures the flow as evidence:
       ```
       cd C:/Users/bono/racingpoint/racecontrol
       npx playwright test --config tests/page-crawler/playwright.config.ts
       ```
  </how-to-verify>
  <resume-signal>Type "approved" if top-up works, or describe what failed</resume-signal>
</task>

</tasks>

<verification>
1. Venue .23:3200 — web dashboard running, `curl` returns 200, `_next/static/` not 404
2. Cloud :3200 — web dashboard running via pm2, `curl` returns 200
3. POST to `/api/v1/billing/wallet/topup` with `"method":"CASH"` does NOT return 422
4. Playwright screenshot in `tests/screenshots/` showing successful top-up flow
</verification>

<success_criteria>
- Wallet top-up works end-to-end on venue (verified from non-server machine)
- Wallet top-up works end-to-end on cloud (deploy parity)
- Playwright screenshot evidence of working flow captured
- No 422 "missing field method" errors
</success_criteria>

<output>
After completion, create `.planning/quick/260406-uwa-deploy-wallet-topup-fix-to-venue-and-clo/260406-uwa-SUMMARY.md`
</output>
