# Phase 111: Code Signing + Per-Game Canary Validation - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase validates the entire v15.0 anti-cheat stack on Pod 8 with real games: deploy the new rc-agent binary, launch each protected game (F1 25, iRacing, LMU), verify safe mode activates, confirm no anti-cheat warnings, and verify billing continuity. Code signing is deferred until Uday procures the Sectigo OV certificate — canary validation proceeds with unsigned binaries first.

</domain>

<decisions>
## Implementation Decisions

### Code Signing (HARD-02) — DEFERRED
- Uday has not yet purchased the Sectigo OV certificate
- Code signing will be done as a follow-up task when the cert arrives
- Canary validation proceeds NOW with unsigned binaries to validate safe mode behavior
- When cert arrives: integrate signtool into deploy-staging, sign both binaries, re-deploy to Pod 8

### Canary Validation Approach (VALID-01)
- Deploy new rc-agent.exe to Pod 8 ONLY (canary discipline)
- Run staff test sessions for each available protected game: F1 25, iRacing, LMU
- For each game: verify safe mode enters, verify no anti-cheat kicks/warnings, verify billing ticks
- Document results in a validation report

### Billing Continuity (VALID-02)
- Start a billing session before launching game
- Verify billing ticks continue during safe mode
- Verify billing amount is correct at session end
- No gaps or incorrect amounts

### Claude's Discretion
- Validation report format and location
- How to deploy to Pod 8 (fleet exec vs manual copy)
- Test session duration per game (suggested: 5 minutes each)
- Which specific tracks/configs to use for each game

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `C:/Users/bono/racingpoint/deploy-staging/` — staging area with HTTP server
- Fleet exec endpoint: `POST http://192.168.31.23:8080/api/v1/fleet/exec`
- Pod 8 direct: `http://192.168.31.91:8090/`
- Build: `cargo build --release --bin rc-agent`

### Integration Points
- Phase 108 keyboard hook replacement — GPO lockdown must work on Pod 8
- Phase 109 safe mode — must enter on game launch, exit after 30s cooldown
- Phase 110 telemetry gating — deferred shm connect, UDP socket lifecycle

</code_context>

<specifics>
## Specific Ideas

- Pod 8 is already running build f3905b3 (97 commits behind HEAD)
- New binary has safe_mode.rs, GPO lockdown, telemetry gating — major changes
- Deploy sequence: kill → delete → download → size check → start → verify (CLAUDE.md rule)
- Check safe mode logs: "Safe mode: entering", "Safe mode: starting 30s cooldown", "Safe mode: exiting"

</specifics>

<deferred>
## Deferred Ideas

- Code signing with Sectigo OV cert — waiting on Uday procurement
- Fleet-wide deployment (all 8 pods) — only after Pod 8 canary passes
- EA WRC and AC EVO validation — games may not be installed on Pod 8

</deferred>
