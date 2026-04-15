@echo off
REM Canonical source for C:\RacingPoint\start-admin.bat on server .23.
REM RC_JWT_SECRET MUST match auth.jwt_secret in C:\RacingPoint\racecontrol.toml.
REM If you rotate jwt_secret, update BOTH files in the same change and restart StartAdmin schtask.
REM Drift here causes silent admin login loop: /api/auth/login returns 200+cookie, then middleware
REM rejects the cookie on /, redirects to /login, user sees "PIN not working."
REM Incident: 2026-04-14 — venue admin rejected PIN 261121 due to stale secret.
set "PORT=3201"
set "HOSTNAME=0.0.0.0"
set "RC_URL=http://localhost:8080"
set "RC_JWT_SECRET=20141d12282c490a216a537160758b8fa3d981c4bb517d5b99f2ffc2feaea535"
cd /d C:\RacingPoint\admin
"C:\Program Files\nodejs\node.exe" server.js
