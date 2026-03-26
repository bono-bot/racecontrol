# Autonomous Bug Detection & Self-Healing System

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    JAMES (On-Site, Primary)                       │
│  auto-detect.sh → Audit → QualityGate → E2E → Cascade → Fix    │
│  Scheduled: Daily 2:30 AM IST (Task Scheduler)                  │
│  On-demand: AUDIT_PIN=261121 bash scripts/auto-detect.sh        │
└────────────────────┬────────────────────────────────────────────┘
                     │ WS + INBOX.md (results)
                     │ git push (code fixes)
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                    BONO (VPS, Failover)                           │
│  bono-auto-detect.sh → checks James alive → if DOWN:            │
│    → Server health → Fleet health → Build consistency → Notify  │
│  Cron: Daily 2:30 AM IST (0 21 * * * UTC)                      │
│  Chain: auto-detect-bono template via relay                     │
└─────────────────────────────────────────────────────────────────┘
```

## James-Side: auto-detect.sh

### What it does (6 steps):
1. **Audit Protocol** — runs `audit.sh --mode <quick|standard|full> --auto-fix`
2. **Quality Gate** — runs comms-link test suite (contract + integration + syntax + security)
3. **E2E Health** — verifies server, Bono VPS, relay, exec round-trip, chain round-trip, Next.js apps
4. **Cascade Check** — build drift (server vs HEAD), pod consistency, cloud-venue match, comms-link sync
5. **Standing Rules** — unpushed commits, relay health
6. **Report & Notify** — sends Bono WS message + summary JSON

### Usage:
```bash
# Quick mode (Tiers 1-2, ~5 min)
AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode quick

# Standard mode (Tiers 1-9, ~15 min)
AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode standard

# Full mode (all 60 phases, ~8 min with parallel engine)
AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode full

# Dry run (parse + init, no checks)
AUDIT_PIN=261121 bash scripts/auto-detect.sh --dry-run

# No auto-fix (detect only)
AUDIT_PIN=261121 bash scripts/auto-detect.sh --no-fix

# No notifications
AUDIT_PIN=261121 bash scripts/auto-detect.sh --no-notify
```

### Exit codes:
- 0 = all clear (or all bugs auto-fixed)
- 1 = unfixed bugs remain
- 2 = fatal prerequisite error

## Bono-Side: bono-auto-detect.sh

### Failover Logic:
1. Check if James relay is alive
2. If alive → delegates to James `auto-detect.sh` via relay exec
3. If James DOWN → runs independent checks:
   - Venue server health (Tailscale SSH restart if down)
   - Cloud racecontrol status
   - Fleet health (via server API)
   - Next.js apps health
   - Git sync state (auto-pull if behind)
4. Notifies Uday via WhatsApp if critical issues found
5. Activates cloud failover if venue server unreachable

### Cron:
```
0 21 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh >> /root/auto-detect-logs/cron.log 2>&1
```
(21:00 UTC = 02:30 IST)

## Chain Templates

### auto-detect-bono
Triggers Bono-side detection via comms-link relay:
```bash
curl -s -X POST http://localhost:8766/relay/chain/run \
  -H "Content-Type: application/json" \
  -d '{"template":"auto-detect-bono"}'
```

### sync-and-verify
Pull + health check all services on Bono:
```bash
curl -s -X POST http://localhost:8766/relay/chain/run \
  -H "Content-Type: application/json" \
  -d '{"template":"sync-and-verify"}'
```

## Standing Rules for Autonomous Detection

1. Both scripts MUST notify the other AI on completion
2. Auto-fixes are whitelist-only (no risky operations without confirmation)
3. Billing sessions are NEVER interrupted (idle gate check)
4. All fixes are verified after application
5. When James is down, Bono activates cloud failover automatically
6. Daily cron ensures at least one detection cycle per 24 hours
7. On-demand runs should use `--mode quick` for fast checks, `--mode full` for deep analysis
