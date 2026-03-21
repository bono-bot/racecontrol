# Phase 107: Behavior Audit + Certificate Procurement - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers an exhaustive risk inventory of every pod-side behavior that could trigger anti-cheat detection, a ConspitLink driver audit, Windows edition verification for all 8 pods, a per-game compatibility matrix, and the initiation of OV code signing certificate procurement. No code changes — pure audit, documentation, and procurement.

</domain>

<decisions>
## Implementation Decisions

### Audit Document Format & Location
- Risk inventory stored at `docs/anticheat/risk-inventory.md` in repo — version-controlled and accessible
- Compatibility matrix stored as `racecontrol.toml` `[safe_mode]` section config + `docs/anticheat/compatibility-matrix.md` reference doc
- ConspitLink audit report as markdown with Process Monitor screenshots at `docs/anticheat/conspit-link-audit.md`
- Pod edition verification as a single table in risk-inventory.md with `winver` output per pod

### Certificate Procurement
- Certificate Authority: Sectigo OV (~$220/yr) — widely recognized, fast issuance
- Key storage: Physical USB token (single build machine = James .27)
- Procurement owner: Uday (business owner) — OV requires org verification with real company docs
- Timeline: Start immediately in Phase 107 — lead time 1-5 business days, must be ready for Phase 111

### ConspitLink Audit Methodology
- Audit on Pod 8 (canary pod) — standard for all testing
- Tool: Sysinternals Process Monitor (ProcMon) — industry standard, captures everything
- Look for: kernel drivers loaded, DLL injection into game processes, process handles opened, registry writes during game session
- If ConspitLink IS risky: document risk, add to safe mode gating list (suspend/restart around game sessions)

### Claude's Discretion
- Risk severity classification thresholds (mapping specific behaviors to CRITICAL/HIGH/MEDIUM/LOW)
- Compatibility matrix table structure and grouping
- Order of audit activities (can be parallelized where possible)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/kiosk.rs:956` — `install_keyboard_hook()` using SetWindowsHookExW (WH_KEYBOARD_LL) — CRITICAL risk behavior to document
- `crates/rc-agent/src/process_guard.rs` — continuous process monitoring and auto-kill — HIGH risk behavior
- `crates/rc-agent/src/sims/iracing.rs` — iRacing shared memory adapter (MapViewOfFile)
- `crates/rc-agent/src/sims/lmu.rs` — LMU shared memory adapter (rF2 memory mapping)
- `crates/rc-agent/src/sims/assetto_corsa_evo.rs` — AC EVO shared memory adapter
- `crates/rc-agent/src/sims/f1_25.rs` — F1 25 UDP telemetry adapter

### Established Patterns
- Config in `racecontrol.toml` with TOML section structure
- Documentation in `docs/` directory at repo root
- Pod 8 as canary for all testing and validation

### Integration Points
- `racecontrol.toml` will gain a `[safe_mode]` section defining per-game subsystem gating
- Risk inventory feeds directly into Phase 108 (keyboard hook replacement) and Phase 109 (safe mode design)
- Windows edition verification determines Phase 108 approach (Keyboard Filter vs GPO)

</code_context>

<specifics>
## Specific Ideas

- Research found F1 25 uses EA Javelin (NOT EAC) — must be reflected in all audit docs
- iRacing migrated from EAC to Epic EOS in May 2024 — telemetry via named shared memory is officially safe
- AC EVO has no confirmed anti-cheat as of early 2026 (Early Access) — treat as unknown/protected
- Ollama running on pods is a potential risk (GPU/memory contention visible to anti-cheat heuristics)

</specifics>

<deferred>
## Deferred Ideas

- Microsoft Trusted Signing (Azure-based, newer alternative to traditional OV certs) — evaluate if Sectigo doesn't work out
- Automated anti-cheat compatibility CI test (run game, check for warnings) — too complex for v15.0

</deferred>
