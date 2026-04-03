# POS Dashboard Fix — 2026-04-04

## Goal
Fix POS billing dashboard display and diagnose why Mesh Intelligence couldn't self-heal it.

## Steps

### Phase 1: Diagnosis
- [x] **1.1** SSH into POS, check Edge state and rc-agent health
- [x] **1.2** Identify what URL Edge was pointing to
- [x] **1.3** Check Mesh Intelligence POS kiosk logic in diagnostic engine + pod healer
- [x] **1.4** Find root cause of boot-time failure (Edge auto-launch race condition)
- [x] **1.5** Find root cause of Mesh Intelligence inability to fix (kiosk.enabled=false + no POS healer action)

### Phase 2: Fixes Applied
- [x] **2.1** Fix `restart-kiosk.ps1` URL: `:8080/billing` → `:3200/billing` — PERMANENT
- [x] **2.2** Enable `kiosk.enabled = true` in `rc-agent.toml` — PERMANENT
- [x] **2.3** Remove Edge `MicrosoftEdgeAutoLaunch` HKCU registry key — TEMPORARY (Edge recreates it)
- [x] **2.4** Add Edge policy `StartupBoostEnabled=0`, `BackgroundModeEnabled=0` — PERMANENT
- [x] **2.5** Add `pos1` SSH alias (Tailscale IP, correct user) — PERMANENT
- [x] **2.6** Fix kiosk middleware: remove `/staff` from STAFF_ROUTES — PERMANENT
- [x] **2.7** Rebuild and deploy kiosk to server — DONE
- [x] **2.8** Bring Edge to foreground over rc-agent console — TEMPORARY

### Phase 3: Verification
- [x] **3.1** Screenshot via rc-agent exec (interactive session) — confirmed billing PIN screen visible
- [x] **3.2** Verify `/kiosk/staff` returns 200 (was 307) — confirmed
- [ ] **3.3** User physical confirmation of POS screen — WAITING
- [ ] **3.4** Reboot POS and verify billing appears on fresh boot — TODO

### Phase 4: Permanent Fixes Needed
- [ ] **4.1** Hide rc-agent console window on POS boot (covers kiosk) — TODO
  - Options: `start /MIN` in bat, or `WindowStyle Hidden` in schtask, or VBS wrapper
  - Dependency: needs bat file update on POS
- [ ] **4.2** Add POS kiosk healer action to pod_healer.rs — TODO
  - When `PosKioskEscaped` detected → minimize non-Edge windows + bring Edge to foreground
  - When `PosKioskDown` detected → relaunch Edge with billing URL
- [ ] **4.3** Commit middleware fix + CLAUDE.md rule — TODO
- [ ] **4.4** Test reboot persistence (Edge policy prevents auto-launch race) — TODO

## Root Causes Found
1. **Edge `MicrosoftEdgeAutoLaunch` HKCU registry** — races with kiosk launch, starts Edge in session-restore mode before `--kiosk` flag can take effect
2. **`kiosk.enabled = false`** in rc-agent.toml — disabled entire kiosk enforcement subsystem
3. **No POS healer action** — diagnostic engine detects `PosKioskDown`/`PosKioskEscaped` but pod healer has zero recovery actions for POS nodes
4. **Middleware chicken-and-egg** — `/staff` in STAFF_ROUTES blocked the login form that creates the JWT
5. **`start-rcagent.bat` console window** — visible cmd.exe covers kiosk on every boot

## G9 Triggers (1 this session)
1. Staff kiosk same as customer kiosk → middleware blocking login page → CLAUDE.md rule added

## Status
**Next action:** 3.3 — User physical confirmation, then 4.1 — hide console window permanently
