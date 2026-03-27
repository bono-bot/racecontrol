# Racing Point Operations Quick Reference v2.1

**Use this at 3am.** Full details in UNIFIED-PROTOCOL.md.

---

## EMERGENCY (customers affected NOW)

```
1. TRIAGE (2 min)     How many pods? Customers waiting? Server up?
2. STABILIZE (3 min)  Reboot pod / kill orphans / clear MAINTENANCE_MODE / paper billing
3. COMMUNICATE         "5 minutes" or move customer to working pod
4. LOG AFTER           LOGBOOK entry once service restored
```

| Symptom | Fix |
|---|---|
| Pod frozen | `shutdown /r /t 5 /f` (SSH or physical) |
| Game won't launch | Kill rc-agent → RCWatchdog auto-restarts |
| Blanking stuck | `del C:\RacingPoint\MAINTENANCE_MODE` → restart |
| Server down | `ssh ADMIN@100.125.108.37 "schtasks /Run /TN StartRCTemp"` |
| Billing broken | Paper: pod#, name, start time |
| Multiple pods | Mark bad ones out of rotation, serve on rest |

**Max 15 min in emergency mode.** Minute 16 → auto-alert Uday, mark pods out of rotation, paper billing, start Phase D.

---

## OPERATING MODES

| Mode | Trigger | Checklist |
|---|---|---|
| **GREEN** | Normal ops | ~40 mandatory items, all gates |
| **AMBER** | 1-2 pods down, no customer impact | ~10 items (triage + stabilize) |
| **RED** | 3+ pods, customers affected, server down | ~7 items (stabilize + communicate + log) |

---

## BREAK-GLASS (Uday unreachable >30 min)

**AI agents CAN:** restart services, reboot pods, rollback to prev binary, clear sentinels, disable sync, kill processes, run diagnostics, commit code.

**AI agents CANNOT:** deploy NEW binary, change pricing, modify customer data, change network infra, spend >$10, promise refunds.

---

## DAILY FLOW

```
SESSION START → check fleet health, MAINTENANCE_MODE, build_id vs HEAD
PLAN          → prompt quality check, past fix lookup, risk tag sensitive areas
CREATE        → cargo test, no unwrap/any, cascade update, security gate
VERIFY        → exact behavior path, domain-matched, Pod 8 canary, multi-machine
DEPLOY        → Pod 8 first → fleet, server 7-step, cloud sync
SHIP          → 4 gates: Quality + E2E + Standing Rules + Multi-Model Audit
```

---

## 4 SHIPPING GATES (Ultimate Rule)

| # | Gate | Tool | Must Pass? |
|---|---|---|---|
| 1 | Quality Gate | `bash test/run-all.sh` | YES |
| 2 | E2E Round-Trip | exec + chain + health curl | YES |
| 3 | Standing Rules | auto-push, Bono synced, watchdog | YES |
| 4 | Multi-Model Audit | OpenRouter 5 models | YES (milestones) |

---

## DEBUGGING (when something breaks)

### 5-Tier Debug Order
1. **Deterministic** — clear sentinels, kill orphans, restart (no AI needed)
2. **Memory** — check LOGBOOK + git history + knowledge base
3. **Local Ollama** — qwen2.5:3b at .27:11434
4. **Multi-Model** — 4 OpenRouter models diagnose in parallel (~$3)
5. **Cloud Claude** — full Opus escalation (last resort)

### 5-Step Cause Elimination
1. Reproduce + document symptom
2. List ALL hypotheses (minimum 3, specific + falsifiable)
3. Test + eliminate one at a time (evidence, not assumptions)
4. Fix confirmed cause
5. Log in LOGBOOK

### When to Escalate to Multi-Model (Tier 4)
- 2+ hours, no progress
- 3+ failed fixes
- Can't explain the behavior
- Fix works but don't know why

---

## FLEET-SPECIFIC GOTCHAS

| Rule | Why |
|---|---|
| Clear MAINTENANCE_MODE first | Silent pod killer — blocks ALL restarts |
| rc-agent must be Session 1 | Session 0 = no GUI, no games, no Edge |
| Never restart explorer on pods | Breaks NVIDIA Surround triple monitors |
| `ok:true` on game launch = queued, not delivered | Check WS, not API response |
| cmd.exe mangles quotes | Use PID targeting or batch files |
| `.spawn().is_ok()` != child alive | Verify process after spawn |
| UTC logs, IST operations | Convert before counting events |

---

## DEPLOY CHEAT SHEET

### Pod (rc-agent)
```
cargo build --release --bin rc-agent
touch crates/rc-agent/build.rs
cp target/release/rc-agent.exe deploy-staging/
# Pod 8 canary first:
curl.exe -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe
# Write RCAGENT_SELF_RESTART sentinel
# Verify: curl -s http://<pod>:8090/health | jq .build_id
```

### Server (racecontrol)
```
1. Record build_id: git rev-parse --short HEAD
2. Download to server while old process runs
3. SSH: ren racecontrol.exe racecontrol-old.exe → ren new → kill → schtasks
4. Verify build_id matches
5. Verify the EXACT fix (not just health)
```

---

## COMMS

```bash
# Push + notify (EVERY commit)
git push
cd comms-link && COMMS_PSK="..." COMMS_URL="ws://srv1422716.hstgr.cloud:8765" node send-message.js "msg"
# Append to INBOX.md + git push
```

---

## COST CONTROLS

| Scope | Cost | Ceiling |
|---|---|---|
| Per change (Tier A) | ~$0.05 | No limit |
| Risk-triggered (Tier B) | ~$1.50 | - |
| Milestone (Tier C) | ~$3-5 | - |
| Diagnostic escalation | ~$3 | - |
| Per session | - | $10 without Uday |
| Monthly | ~$10-15 | $50 hard stop |

---

## PHYSICAL VENUE

**Before opening:** steering responsive, pedals firm, monitors on, HVAC running, temp <28C
**Between sessions:** wipe wheel + pedals + headphones, check for spills
**Weekly:** cables, brake springs, FFB motor temp, ConspitLink logs, UPS
**Customer safety:** brief new customers, motion sickness watch, no drinks near electronics

---

*Full protocol: UNIFIED-PROTOCOL.md | Audit: `bash audit/verify-unified-protocol.sh`*
