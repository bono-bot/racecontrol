# v15.0 AntiCheat Compatibility — Canary Validation Log

**Pod:** 8 (192.168.31.91)
**Build:** 243f03d (v15.0 — safe mode, GPO lockdown, telemetry gating)
**Deploy verified:** Yes — ws_connected=true, uptime=326s, no crash

## Test 1: F1 25 (EA Javelin)

| Check | Result | Notes |
|-------|--------|-------|
| Game launched from kiosk | [ ] Pass / [ ] Fail | |
| Safe mode entered (log: "Safe mode: entering") | [ ] Pass / [ ] Fail | |
| No anti-cheat warnings during 5min session | [ ] Pass / [ ] Fail | |
| Win key blocked during session (GPO lockdown) | [ ] Pass / [ ] Fail | |
| Game closed cleanly | [ ] Pass / [ ] Fail | |
| Safe mode cooldown started (log: "starting 30s cooldown") | [ ] Pass / [ ] Fail | |
| Safe mode exited after 30s (log: "Safe mode: exiting") | [ ] Pass / [ ] Fail | |
| Billing amount correct | [ ] Pass / [ ] Fail | Amount: ___ |

## Test 2: iRacing (EOS)

| Check | Result | Notes |
|-------|--------|-------|
| Game launched from kiosk | [ ] Pass / [ ] Fail | |
| Safe mode entered | [ ] Pass / [ ] Fail | |
| No anti-cheat warnings during 5min session | [ ] Pass / [ ] Fail | |
| Win key blocked during session | [ ] Pass / [ ] Fail | |
| Game closed cleanly | [ ] Pass / [ ] Fail | |
| Safe mode cooldown started | [ ] Pass / [ ] Fail | |
| Safe mode exited after 30s | [ ] Pass / [ ] Fail | |
| Billing amount correct | [ ] Pass / [ ] Fail | Amount: ___ |

## Test 3: LMU / Le Mans Ultimate (EAC)

| Check | Result | Notes |
|-------|--------|-------|
| Game launched from kiosk | [ ] Pass / [ ] Fail | |
| Safe mode entered | [ ] Pass / [ ] Fail | |
| No anti-cheat warnings during 5min session | [ ] Pass / [ ] Fail | |
| Win key blocked during session | [ ] Pass / [ ] Fail | |
| Game closed cleanly | [ ] Pass / [ ] Fail | |
| Safe mode cooldown started | [ ] Pass / [ ] Fail | |
| Safe mode exited after 30s | [ ] Pass / [ ] Fail | |
| Billing amount correct | [ ] Pass / [ ] Fail | Amount: ___ |

## Code Signing Status

**Status:** DEFERRED — waiting on Uday to procure Sectigo OV certificate
**Next steps when cert arrives:**
1. Install USB token on James (.27)
2. `signtool sign /f cert.pfx /t http://timestamp.sectigo.com rc-agent.exe`
3. `signtool verify /pa rc-agent.exe` — must say "Successfully verified"
4. Sign rc-sentry.exe with same cert
5. Re-deploy signed binaries to Pod 8 and re-run all 3 game tests

## Summary

| Game | Anti-Cheat | Safe Mode | No Warnings | Billing | Overall |
|------|-----------|-----------|-------------|---------|---------|
| F1 25 | EA Javelin | PENDING | PENDING | PENDING | PENDING |
| iRacing | EOS | PENDING | PENDING | PENDING | PENDING |
| LMU | EAC | PENDING | PENDING | PENDING | PENDING |

**Date tested:** _______________
**Tester:** _______________
**Overall verdict:** PENDING — awaiting game test sessions
