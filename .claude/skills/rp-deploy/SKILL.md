---
name: rp-deploy
description: Build rc-agent release binary and stage for pod deployment
disable-model-invocation: true
---

# /rp:deploy — RC-Agent Build + Stage

## When to Use

James explicitly runs `/rp:deploy` to prepare a new rc-agent binary for pod deployment. Never auto-triggered.

## Steps

### Step 1: Export PATH

```bash
export PATH="$PATH:/c/Users/bono/.cargo/bin"
```

### Step 2: Build rc-agent release binary

```bash
cd /c/Users/bono/racingpoint/racecontrol && cargo build --release --bin rc-agent 2>&1
```

STOP if exit code != 0. Show full error output.

### Step 3: Verify binary exists and size

```bash
ls -la /c/Users/bono/racingpoint/racecontrol/target/release/rc-agent.exe
```

Expected: > 8,000,000 bytes. STOP if smaller (truncated build).

### Step 4: Copy to deploy-staging

```bash
cp /c/Users/bono/racingpoint/racecontrol/target/release/rc-agent.exe /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

### Step 5: Verify staged binary

```bash
ls -la /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

## Output

After success, report:
- Binary size in bytes
- The pendrive deploy command: `D:\pod-deploy\install.bat <pod_number>`
- Reminder: deploy to Pod 8 first (canary), verify, then remaining pods

## Errors

- Build failure: show full cargo error output, do NOT proceed
- Binary < 8MB: warn "truncated build — do NOT deploy" and STOP
- Copy failure: check disk space in deploy-staging
