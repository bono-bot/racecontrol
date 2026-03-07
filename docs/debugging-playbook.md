# RaceControl Debugging Playbook

Lessons learned from live production debugging. Reference this before troubleshooting.

---

## 1. Blank Screen Stays Active During Session

**Symptom:** Customer starts a billing session but the pod screen stays black (blank screen) instead of showing the countdown timer.

**Root Causes (in order of likelihood):**

### A. rc-agent restarted during active session
- When rc-agent restarts (deploy, crash, watchdog restart), it connects fresh to rc-core
- **Old behavior (pre-273db1c):** rc-core never re-sent billing state on reconnect
- **Fix (273db1c):** rc-core now sends `BillingStarted` + `BillingTick` on agent `Register` if an active timer exists for that pod
- **File:** `crates/rc-core/src/ws/mod.rs` — in the `AgentMessage::Register` handler, after storing the agent sender

### B. Stale old rc-agent holding port 18923
- Old rc-agent process (pre-mutex binary) still running, holding the lock screen HTTP port
- New rc-agent can't bind port 18923, lock screen server doesn't start
- **Diagnosis:** `netstat -ano | findstr 18923` — check PID vs `tasklist | findstr rc-agent`
- **Fix:** Kill ALL rc-agent processes (`taskkill /F /IM rc-agent.exe`), wait 3s, start fresh
- **Prevention (305638b):** Named mutex `Global\RacingPoint_RCAgent_SingleInstance` prevents duplicates

### C. Browser not relaunching on state change
- **Old behavior (pre-05ef1d6):** `show_active_session()` only relaunched Edge if previous state was `ScreenBlanked`
- **Fix (05ef1d6):** Always relaunch browser on `BillingStarted`, regardless of prior state
- **Fallback:** Blank page auto-reloads every 3s (was 30s) to pick up state changes
- **File:** `crates/rc-agent/src/lock_screen.rs`

### D. WebSocket not connected
- rc-agent started but hasn't connected to rc-core yet (network issue, rc-core down)
- `BillingStarted` message can't reach the agent
- **Diagnosis:** Check rc-core logs for "Pod X registered" or "Resynced billing session"
- **Quick fix:** `POST /api/v1/pods/pod_X/screen` with `{"blank": false}` to clear manually

**Debug Checklist:**
```
1. curl http://localhost:8080/api/v1/billing/active     → is session actually active?
2. exec on pod: tasklist | findstr rc-agent             → how many instances? which PID?
3. exec on pod: netstat -ano | findstr 18923            → who holds the lock screen port?
4. exec on pod: curl.exe -s http://127.0.0.1:18923/     → what page is being served?
   - "timer-display" = active session (correct)
   - "blank-pin" / "numpad" = blank screen (wrong if session active)
   - empty = lock screen server crashed
5. POST /api/v1/pods/pod_X/screen {"blank":false}       → manual override to clear
```

---

## 2. Zombie rc-agent Processes

**Symptom:** Multiple rc-agent instances running on a pod (seen up to 15 instances).

**Root Causes:**
- Deploy script firing multiple `start` commands
- Pod watchdog.bat restarting rc-agent while it's already running
- Manual restarts without killing first

**Prevention (305638b):**
- Windows named mutex `Global\RacingPoint_RCAgent_SingleInstance` in `main()`
- Second instance detects mutex exists → prints warning → `std::process::exit(0)`
- **File:** `crates/rc-agent/src/main.rs` — first thing in `main()`, before tracing init

**Cleanup Procedure:**
```
1. exec on pod: taskkill /F /IM rc-agent.exe        → kill ALL instances
2. sleep 3                                           → wait for ports to free
3. exec on pod: start "rc-agent" /D C:/RacingPoint C:/RacingPoint/rc-agent.exe
4. sleep 5
5. exec on pod: tasklist | findstr rc-agent          → verify exactly 1 instance
```

**Deploy script (deploy-rc-agent.py):**
- Uses single `start` command (not two like before)
- Sequence: ping → kill → delete old → curl download → verify size → start → verify

---

## 3. rc-agent Crash on Startup (AddrInUse)

**Symptom:** rc-agent panics with `Os { code: 10048, kind: AddrInUse }` on port 18923.

**Root Cause:** Previous rc-agent process still holding port (zombie, or slow shutdown).

**Diagnosis:**
```
exec on pod: netstat -ano | findstr 18923
```
If LISTENING on a different PID than the current rc-agent → stale process.

**Fix:**
```
exec on pod: taskkill /F /PID <stale_pid>
# wait 3 seconds for port to free
exec on pod: start "rc-agent" /D C:/RacingPoint C:/RacingPoint/rc-agent.exe
```

**Long-term:** The mutex guard prevents new zombies, but can't fix processes started with old binary. After deploying mutex-guarded binary to all pods, this should stop.

---

## 4. Kiosk Terminal Auto-Locking (Staff Login Lost)

**Symptom:** Staff terminal (kiosk Next.js app on port 3300) shows login screen unexpectedly.

**Root Cause (pre-417dd06):** Staff login state stored only in React `useState` — lost on page refresh, WebSocket reconnect, or Next.js hot reload.

**Fix (417dd06):** Login persisted in `sessionStorage`:
- `kiosk_staff_id` and `kiosk_staff_name` saved on login
- Restored from sessionStorage on component mount
- Cleared on explicit "Sign Out" or when browser tab closes
- **File:** `kiosk/src/app/page.tsx`

---

## 5. Deploying rc-agent to Pods

**Script:** `C:\Users\bono\racingpoint\deploy-rc-agent.py`

**Pre-deploy checklist:**
1. Build: `cargo build -p rc-agent --release`
2. Copy: `cp target/release/rc-agent.exe ../rc-agent.exe`
3. Update `EXPECTED_SIZE` in deploy script: `wc -c rc-agent.exe`
4. Start HTTP server: `python3 -m http.server 8888 --bind 0.0.0.0` (in racingpoint/ dir)
5. Run: `python3 deploy-rc-agent.py`

**Common failures:**
- **Pod unreachable:** Powered off or DHCP IP changed — scan subnet
- **curl exit=23:** Write error, disk full or permissions — check pod disk space
- **SIZE MISMATCH:** certutil was used (adds ~20KB metadata) — always use curl.exe
- **NOT RUNNING:** Start command timeout is expected (rc-agent is long-lived). Check tasklist after.
- **File locked:** Old rc-agent still running — kill first, wait, then download

**Never use certutil for binary downloads** — Windows Defender flags it as `Trojan:Win32/Ceprolad.A` and it corrupts binaries by adding metadata.

---

## 6. rc-core Rebuild (Binary Locked)

**Symptom:** `cargo build -p rc-core --release` fails with "Access is denied" on `racecontrol.exe`.

**Cause:** rc-core is still running.

**Fix:**
```bash
powershell -Command "Stop-Process -Name racecontrol -Force"
sleep 3
cargo build -p rc-core --release
# restart after build
./target/release/racecontrol.exe &
```

**Warning:** Stopping rc-core drops all WebSocket connections. All rc-agents will disconnect and reconnect (showing "Disconnected" on lock screens briefly). Active billing sessions survive — they're recovered from DB on restart.

---

## 7. Lock Screen Architecture Quick Reference

| Component | Location | Port |
|-----------|----------|------|
| Lock screen HTTP server | rc-agent (localhost only) | 18923 |
| Lock screen browser | Edge kiosk mode | — |
| Billing commands | rc-core → rc-agent via WebSocket | 8080 |
| Manual screen control | REST API | 8080 |
| UDP heartbeat | rc-agent ↔ rc-core | 9999 |

**Lock Screen States:**
- `Hidden` — no browser, desktop visible
- `ScreenBlanked` — black screen with PIN numpad, 3s auto-reload
- `PinEntry` — customer entering PIN
- `QrDisplay` — QR code for mobile auth, 5s auto-reload
- `ActiveSession` — countdown timer, updated every 1s via BillingTick
- `SessionSummary` — post-session stats, auto-returns after 15s
- `BetweenSessions` — wallet balance shown, waiting for next race
- `AwaitingAssistance` — staff help needed
- `Disconnected` — WebSocket lost, 3s auto-reload

**API for manual control:**
```
POST /api/v1/pods/pod_X/screen
  {"blank": true}   → show blank screen
  {"blank": false}  → clear lock screen (Hidden state)
```

---

## 8. Windows Defender False Positives

**Trigger:** `certutil -urlcache` commands flagged as `Trojan:Win32/Ceprolad.A`

**Impact:** File quarantined, Defender notification popup.

**Prevention:** Never use certutil for file downloads. Use `curl.exe` (built into Windows 11) instead.

**If triggered:** Alerts are false positives. Check with:
```powershell
Get-MpThreatDetection | Select DetectionID, ThreatID, ProcessName, InitialDetectionTime
Get-MpThreat | Select ThreatName, IsActive, DidThreatExecute
```

---

## 9. Pod-Agent Exec Escaping Issues

**Problem:** Backslashes in Windows paths get mangled through bash → Python → JSON → pod-agent chain.

**Solutions:**
- Use forward slashes: `C:/RacingPoint/rc-agent.exe` (works in most Windows commands)
- Use pod-agent `/file` endpoint for reading files: `GET /file?path=C:/RacingPoint/rc-agent.toml`
- Use pod-agent `/files` endpoint for directory listing: `GET /files?path=C:/RacingPoint`
- For complex commands, write a .bat or .ps1 file to the pod first, then exec it

**Known:** `type C:\\path` fails through JSON exec because `\\R` is interpreted. Use `more` or `/file` endpoint instead.

---

## 10. Commit Reference (Fixes Applied 2026-03-08)

| Commit | Fix |
|--------|-----|
| `305638b` | Zombie prevention: Windows named mutex in rc-agent |
| `417dd06` | Kiosk login persistence: sessionStorage |
| `05ef1d6` | Blank screen clear: always relaunch browser on BillingStarted, 3s auto-reload |
| `273db1c` | Billing resync: rc-core re-sends active session on agent reconnect |
