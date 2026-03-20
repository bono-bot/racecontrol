---
name: rp-deploy-server
description: Build racecontrol, swap server binary, verify :8080, commit, notify Bono
disable-model-invocation: true
---

# /rp:deploy-server — Full Server Deploy Pipeline

## When to Use

James explicitly runs `/rp:deploy-server` to update the racecontrol server binary. Never auto-triggered. This is a destructive operation — kills the running server process.

## Steps

### Step 1: Export PATH

```bash
export PATH="$PATH:/c/Users/bono/.cargo/bin"
```

### Step 2: Build racecontrol release binary

```bash
cd /c/Users/bono/racingpoint/racecontrol && cargo build --release --bin racecontrol 2>&1
```

STOP if exit code != 0. Show full error output.

### Step 3: Verify binary size

```bash
ls -la /c/Users/bono/racingpoint/racecontrol/target/release/racecontrol.exe
```

Expected: > 10,000,000 bytes. STOP if smaller.

### Step 4: Stage the binary

```bash
cp /c/Users/bono/racingpoint/racecontrol/target/release/racecontrol.exe /c/Users/bono/racingpoint/deploy-staging/racecontrol.exe
```

### Step 5: Kill old racecontrol on server via webterm (no confirmation needed — James invoked deliberately)

```bash
curl -sf "http://192.168.31.27:9999/exec" -d 'taskkill /F /IM racecontrol.exe' 2>&1 || echo "webterm unavailable — provide manual kill instructions"
```

If webterm unavailable, tell James: "Run `taskkill /F /IM racecontrol.exe` on the server (.23) manually."

### Step 6: Wait for port release (3 seconds)

```bash
sleep 3
```

### Step 7: Copy binary to server and start (via webterm)

```bash
curl -sf "http://192.168.31.27:9999/exec" -d 'copy \\192.168.31.27\deploy-staging\racecontrol.exe C:\RacingPoint\racecontrol.exe /Y' 2>&1
curl -sf "http://192.168.31.27:9999/exec" -d 'cd C:\RacingPoint && start "" start-racecontrol.bat' 2>&1
```

If webterm unavailable, tell James the manual commands to run on server.

### Step 8: Verify :8080 comes back (poll up to 30 seconds, 5s intervals)

```bash
for i in 1 2 3 4 5 6; do
  if curl -sf http://192.168.31.23:8080/api/v1/health > /dev/null 2>&1; then
    echo "Server :8080 is UP"
    break
  fi
  echo "Waiting for :8080... (attempt $i/6)"
  sleep 5
done
curl -sf http://192.168.31.23:8080/api/v1/health || echo "FAIL: :8080 not responding after 30s"
```

### Step 9: Git commit the changes

```bash
cd /c/Users/bono/racingpoint/racecontrol && git add -A && git commit -m "deploy: racecontrol server update $(date +%Y-%m-%d)" && git push
```

### Step 10: Notify Bono via comms-link

```bash
INBOX="/c/Users/bono/racingpoint/comms-link/INBOX.md"
TIMESTAMP=$(python3 -c "from datetime import datetime, timezone, timedelta; ist=timezone(timedelta(hours=5,minutes=30)); print(datetime.now(ist).strftime('%Y-%m-%d %H:%M IST'))")
COMMIT=$(cd /c/Users/bono/racingpoint/racecontrol && git rev-parse --short HEAD)
echo "" >> "$INBOX"
echo "## $TIMESTAMP — from james" >> "$INBOX"
echo "" >> "$INBOX"
echo "Deployed racecontrol to server .23. Commit: $COMMIT. Verified :8080 OK." >> "$INBOX"
cd /c/Users/bono/racingpoint/comms-link && git add INBOX.md && git commit -m "james: deploy notification $(date +%Y-%m-%d)" && git push
```

## Output

After success, report: binary size, commit hash, :8080 status, Bono notification status.

## Errors

- Build failure: show cargo error, STOP
- Port still occupied after kill: tell James to check for zombie process on server
- :8080 not responding: tell James to check server logs at `C:\RacingPoint\logs\`
- Bono notification fail: warn but do not block — deploy itself succeeded
