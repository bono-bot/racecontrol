# Requirements: Racing Point Operations — v17.0 AI Debugger Autonomy & Self-Healing

**Defined:** 2026-03-22
**Core Value:** Pods self-heal browser/display issues autonomously — no human intervention needed for Edge crashes, stacking, or lock screen failures.

## v17.0 Requirements

### Browser Watchdog

- [ ] **BWDOG-01**: rc-agent polls browser_process liveness every 30s and relaunches Edge if dead
- [ ] **BWDOG-02**: rc-agent detects Edge stacking (>5 msedge.exe processes) and kills all before relaunching
- [ ] **BWDOG-03**: close_browser() kills ALL msedge.exe and msedgewebview2.exe, not just the spawned child
- [ ] **BWDOG-04**: Browser watchdog is suppressed during safe mode (anti-cheat active) — no taskkill while protected game is running

### Idle Health Monitor

- [ ] **IDLE-01**: rc-agent runs check_window_rect + check_lock_screen_http every 60s when no billing session is active
- [ ] **IDLE-02**: Idle health failure triggers close_browser + launch_browser (self-heal before alerting)
- [ ] **IDLE-03**: Idle health sends IdleHealthFailed message to server after 3 consecutive failures (hysteresis)
- [ ] **IDLE-04**: Idle health checks are skipped during active billing sessions — no interference with running games

### AI Action Execution

- [ ] **AIACT-01**: AI debugger Tier 3/4 responses are parsed for structured safe actions from a whitelist
- [ ] **AIACT-02**: Safe action whitelist includes: kill_edge, relaunch_lock_screen, restart_rcagent, kill_game, clear_temp
- [ ] **AIACT-03**: Executed AI actions are logged to activity_log with action, source model, and outcome
- [ ] **AIACT-04**: AI actions that kill processes are gated by safe mode check — blocked when anti-cheat active

### Healer Edge Recovery

- [ ] **HEAL-01**: Pod healer adds HealAction::RelaunchLockScreen when lock screen HTTP check fails
- [ ] **HEAL-02**: RelaunchLockScreen sends ForceRelaunchBrowser WS message to pod (rc-agent handles relaunch)
- [ ] **HEAL-03**: rc-agent handles ForceRelaunchBrowser message by calling close_browser + launch_browser

### WARN Log Scanner

- [ ] **WARN-01**: Pod healer scans racecontrol log for WARN count in last 5 minutes each cycle
- [ ] **WARN-02**: WARN threshold exceeded (>50/5min) triggers AI escalation with log context
- [ ] **WARN-03**: Recurring identical WARNs are grouped and deduplicated before AI escalation

## Future Requirements

### Browser Watchdog Enhancements

- **BWDOG-05**: Browser watchdog reports Edge crash count to fleet health dashboard
- **BWDOG-06**: Edge auto-update suppression verified on every watchdog cycle

### AI Debugger Enhancements

- **AIACT-05**: AI learns from executed action outcomes (success/failure) to improve future suggestions
- **AIACT-06**: Dashboard shows pending AI suggestions with approve/reject for non-whitelisted actions

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full AI shell access | Security risk — whitelist-only safe actions |
| Auto-restart rc-agent from healer | rc-sentry already handles this — avoid recovery system fights (standing rule #10) |
| Browser watchdog as separate binary | Belongs in rc-agent, not a new process to deploy |
| Real-time log streaming to dashboard | Nice-to-have, not self-healing — defer to future milestone |
| Edge browser replacement | Edge kiosk mode works — the issue was lack of monitoring, not the browser |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BWDOG-01 | — | Pending |
| BWDOG-02 | — | Pending |
| BWDOG-03 | — | Pending |
| BWDOG-04 | — | Pending |
| IDLE-01 | — | Pending |
| IDLE-02 | — | Pending |
| IDLE-03 | — | Pending |
| IDLE-04 | — | Pending |
| AIACT-01 | — | Pending |
| AIACT-02 | — | Pending |
| AIACT-03 | — | Pending |
| AIACT-04 | — | Pending |
| HEAL-01 | — | Pending |
| HEAL-02 | — | Pending |
| HEAL-03 | — | Pending |
| WARN-01 | — | Pending |
| WARN-02 | — | Pending |
| WARN-03 | — | Pending |

**Coverage:**
- v17.0 requirements: 18 total
- Mapped to phases: 0
- Unmapped: 18 ⚠️

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after initial definition*
