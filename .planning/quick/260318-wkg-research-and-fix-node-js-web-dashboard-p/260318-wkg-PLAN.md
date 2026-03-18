---
phase: 260318-wkg
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - deploy-staging/start-web-watchdog.ps1
  - deploy-staging/start-web.bat
  - deploy-staging/install-web-watchdog.json
autonomous: false
requirements: [WEB-WATCHDOG-01]
must_haves:
  truths:
    - "Web dashboard (port 3200) auto-starts after server reboot without manual intervention"
    - "Web dashboard auto-restarts within 10 seconds if node.exe crashes or cmd window is closed"
    - "Watchdog runs hidden (no visible cmd window for user ADMIN to accidentally close)"
  artifacts:
    - path: "deploy-staging/start-web-watchdog.ps1"
      provides: "PowerShell watchdog loop that starts and monitors node.exe for the web dashboard"
    - path: "deploy-staging/start-web.bat"
      provides: "Launcher bat that invokes the PS1 watchdog hidden via -WindowStyle Hidden"
    - path: "deploy-staging/install-web-watchdog.json"
      provides: "rc-agent remote_ops command payload to install the watchdog on .23"
  key_links:
    - from: "start-web.bat"
      to: "start-web-watchdog.ps1"
      via: "powershell -WindowStyle Hidden -File invocation"
      pattern: "powershell.*-WindowStyle Hidden.*start-web-watchdog"
    - from: "HKLM Run key (WebDashboard)"
      to: "start-web.bat"
      via: "Registry auto-start at login"
      pattern: "REG ADD.*WebDashboard.*start-web.bat"
---

<objective>
Make the web dashboard (Next.js on port 3200) resilient on server .23 by creating a PowerShell watchdog wrapper that auto-starts at login and auto-restarts node.exe if it dies.

Purpose: The dashboard died after a server reboot because the scheduled task (ONSTART) didn't reliably start node.exe. The kiosk (port 3300) survived because it uses an HKLM Run key. Apply the same proven pattern to the web dashboard, plus add a watchdog loop for crash recovery.

Output: Three files in deploy-staging ready to deploy to server .23 via rc-agent remote_ops or pendrive.
</objective>

<execution_context>
@C:/Users/bono/.claude/get-shit-done/workflows/execute-plan.md
@C:/Users/bono/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@deploy-staging/kiosk-stage/start-kiosk.bat (existing kiosk launcher pattern)
@deploy-staging/watchdog-rcagent.bat (existing watchdog pattern for pods)
@deploy-staging/start-watchdog.bat (existing watchdog launcher pattern)

Server details:
- SSH: `ssh ADMIN@192.168.31.23`
- Web dashboard path: `C:\RacingPoint\web\server.js` on port 3200
- Node.js: `C:\Program Files\nodejs\node.exe`
- The kiosk at `C:\RacingPoint\kiosk\server.js` port 3300 already works via HKLM Run key
- Existing scheduled task "StartWebDashboard" should be REMOVED (replaced by Run key)

CRITICAL (.bat file creation): .bat files MUST be clean ASCII with CRLF. Write via bash heredoc + `sed 's/$/\r/'` to produce clean output. The Write tool adds UTF-8 BOM which breaks cmd.exe. Never use Write tool directly for .bat files.
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create PowerShell watchdog and launcher scripts</name>
  <files>deploy-staging/start-web-watchdog.ps1, deploy-staging/start-web.bat, deploy-staging/install-web-watchdog.json</files>
  <action>
Create three files in `C:\Users\bono\racingpoint\deploy-staging\`:

**start-web-watchdog.ps1** — PowerShell watchdog loop:
- Sets `$webDir = "C:\RacingPoint\web"`, `$logFile = "C:\RacingPoint\web-dashboard.log"`
- Sets `$env:PORT = "3200"` and `$env:HOSTNAME = "127.0.0.1"`
- Infinite loop:
  1. Log timestamp + "Starting web dashboard on port 3200"
  2. Start-Process `node.exe` with args `server.js` in `$webDir`, capture process object (`-PassThru -NoNewWindow`)
  3. `$proc.WaitForExit()` — blocks until node dies
  4. Log exit code and timestamp
  5. `Start-Sleep -Seconds 5` (backoff before restart)
  6. Loop back to step 1
- Wrap the loop body in try/catch — log any PowerShell errors, sleep 10s, continue loop
- Redirect node stdout/stderr to the log file using `-RedirectStandardOutput` and `-RedirectStandardError` on Start-Process (use separate files: `web-stdout.log` and `web-stderr.log` to avoid locking issues)
- Use `Write-Output` to `$logFile` via `Add-Content` for watchdog's own messages

**start-web.bat** — Launcher for HKLM Run key:
- MUST be created via bash heredoc + `sed 's/$/\r/'` (NOT the Write tool — BOM breaks cmd.exe)
- Content:
  ```
  @echo off
  REM Racing Point Web Dashboard Watchdog Launcher
  REM Runs at login via HKLM Run key
  start "" /B powershell.exe -ExecutionPolicy Bypass -WindowStyle Hidden -File "C:\RacingPoint\start-web-watchdog.ps1"
  ```
- The `/B` + `-WindowStyle Hidden` ensures no visible window

**install-web-watchdog.json** — Deployment reference for server .23:
```json
{
  "description": "Install web dashboard watchdog on server .23",
  "steps": [
    "1. Copy start-web-watchdog.ps1 to C:\\RacingPoint\\ on .23",
    "2. Copy start-web.bat to C:\\RacingPoint\\ on .23",
    "3. Remove old scheduled task: schtasks /Delete /TN StartWebDashboard /F",
    "4. Add HKLM Run key: REG ADD \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\" /v WebDashboard /t REG_SZ /d \"C:\\RacingPoint\\start-web.bat\" /f",
    "5. Kill any existing node.exe serving port 3200 (check with netstat first)",
    "6. Run start-web.bat to start immediately"
  ],
  "ssh_commands": [
    "schtasks /Delete /TN StartWebDashboard /F 2>nul",
    "REG ADD \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\" /v WebDashboard /t REG_SZ /d \"C:\\RacingPoint\\start-web.bat\" /f",
    "start \"\" /B C:\\RacingPoint\\start-web.bat"
  ]
}
```

This JSON is a reference for the human deployer — not an executable. The actual deployment happens via SSH or pendrive.
  </action>
  <verify>
    <automated>ls -la C:/Users/bono/racingpoint/deploy-staging/start-web-watchdog.ps1 C:/Users/bono/racingpoint/deploy-staging/start-web.bat C:/Users/bono/racingpoint/deploy-staging/install-web-watchdog.json && echo "All 3 files exist" && powershell -Command "Get-Content 'C:/Users/bono/racingpoint/deploy-staging/start-web-watchdog.ps1' | Select-Object -First 3" && file C:/Users/bono/racingpoint/deploy-staging/start-web.bat</automated>
  </verify>
  <done>Three files exist in deploy-staging: start-web-watchdog.ps1 (valid PowerShell with watchdog loop), start-web.bat (CRLF ASCII, no BOM, launches PS1 hidden), install-web-watchdog.json (deployment reference with SSH commands)</done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <name>Task 2: Deploy watchdog to server .23 and verify</name>
  <files>none (deployment to remote server)</files>
  <action>
Deploy the watchdog scripts to server .23 and configure auto-start. Claude will attempt deployment via SSH. If SSH fails, present manual deployment steps to the user.

Via SSH (`ssh ADMIN@192.168.31.23`):
1. SCP the two scripts to `C:\RacingPoint\` on .23
2. Remove old scheduled task: `schtasks /Delete /TN StartWebDashboard /F`
3. Add HKLM Run key: `REG ADD "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v WebDashboard /t REG_SZ /d "C:\RacingPoint\start-web.bat" /f`
4. Kill existing web dashboard node process (find PID via `netstat -ano | findstr :3200`)
5. Start the watchdog: `C:\RacingPoint\start-web.bat`
6. Verify: `curl -s http://127.0.0.1:3200` returns HTML
  </action>
  <verify>curl -s http://192.168.31.23:3200 from James's machine (if proxied) or SSH into .23 and run curl -s http://127.0.0.1:3200</verify>
  <done>Web dashboard running on .23 port 3200, HKLM Run key set, old scheduled task removed, watchdog active</done>
  <what-built>PowerShell watchdog wrapper for the web dashboard (port 3200) with auto-restart on crash and HKLM Run key for auto-start at login. Three files are ready in deploy-staging/.</what-built>
  <how-to-verify>
    1. SSH into server: `ssh ADMIN@192.168.31.23`
    2. Copy the two scripts to server:
       - From James (.27), use SCP:
         `scp C:\Users\bono\racingpoint\deploy-staging\start-web-watchdog.ps1 ADMIN@192.168.31.23:C:\RacingPoint\`
         `scp C:\Users\bono\racingpoint\deploy-staging\start-web.bat ADMIN@192.168.31.23:C:\RacingPoint\`
    3. On the server, remove old scheduled task:
       `schtasks /Delete /TN StartWebDashboard /F`
    4. Add HKLM Run key:
       `REG ADD "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v WebDashboard /t REG_SZ /d "C:\RacingPoint\start-web.bat" /f`
    5. Kill existing web dashboard node process if running:
       Find PID via `netstat -ano | findstr :3200` then `taskkill /F /PID <pid>`
    6. Start the watchdog: `C:\RacingPoint\start-web.bat`
    7. Verify dashboard is running: `curl -s http://127.0.0.1:3200` should return HTML
    8. Test crash recovery: Find the node PID for port 3200, kill it. Within 5-10 seconds, `curl http://127.0.0.1:3200` should work again.
    9. Check log: `type C:\RacingPoint\web-dashboard.log` should show restart entries
  </how-to-verify>
  <resume-signal>Type "approved" if dashboard survived restart test, or describe issues</resume-signal>
</task>

</tasks>

<verification>
- Web dashboard responds on port 3200 after fresh start
- Watchdog restarts node.exe within 10 seconds of process death
- No visible cmd/PowerShell window on server desktop
- HKLM Run key "WebDashboard" exists pointing to start-web.bat
- Old scheduled task "StartWebDashboard" removed
- Log file at C:\RacingPoint\web-dashboard.log shows watchdog activity
</verification>

<success_criteria>
- Web dashboard auto-starts at server login (HKLM Run key)
- Web dashboard auto-restarts on crash (PowerShell watchdog loop)
- No visible window that ADMIN could accidentally close
- Pattern matches existing kiosk approach (consistency)
</success_criteria>

<output>
After completion, create `.planning/quick/260318-wkg-research-and-fix-node-js-web-dashboard-p/260318-wkg-SUMMARY.md`
</output>
