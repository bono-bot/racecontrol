# Phase 392-1 — P0 Zero Laps 3-Layer Fix — VERIFICATION

## Status: PARTIAL — Layer 1 deployed to server only

## What Was Built
- Per-minute billing minimum duration floor (180s) — prevents sub-lap sessions
- None duration handling for open-ended per-minute billing
- Apps preset fix adding `[RACECONTROL]` plugin to AC `apps-default.ini`

## Evidence
- Server .23 build `3c854979` = git HEAD (verified via `/api/v1/health` on 2026-04-16)
- Commits: `33717300`, `3c854979`, `d4b6247d`
- Layer 3 research: `RESEARCH-layer3-grace-window.md`

## Verification Method
Partial closure — Layer 1 server deploy verified via build_id match.
Apps preset fix committed but NOT deployed (pods need rc-agent rebuild).
No live E2E lap verification performed yet.

## Outstanding Items
- rc-agent rebuild + deploy to all 8 pods + POS with apps preset fix
- Live lap E2E test (staff session on Pod 8 → laps table row)
- Cloud (Bono VPS) binary parity
- FK pragma second-connection probe on both environments
- Layers 2 + 3 deferred to future phases

## NOT TESTED
- Actual lap appearing in `laps` table from a real session
- Per-minute billing behavior with new 180s floor from customer perspective
- Apps preset fix runtime behavior (plugin loading in AC)
- Cloud deploy parity

---

*Verification written 2026-04-16 to close phase gap artifact.*
