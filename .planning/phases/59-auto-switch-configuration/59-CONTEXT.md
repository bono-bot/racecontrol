# Phase 59: Auto-Switch Configuration - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix broken ConspitLink auto game detection (AresAutoChangeConfig) by placing Global.json at the correct runtime path (`C:\RacingPoint\Global.json`) on every pod, and updating GameToBaseConfig.json mappings to point to correct presets for all 4 venue games (AC, F1 25, ACC/AC EVO, AC Rally). After this phase, launching any venue game causes ConspitLink to automatically load the matching FFB preset without staff intervention.

</domain>

<decisions>
## Implementation Decisions

### Config Placement Strategy
- rc-agent ensures `Global.json` exists at `C:\RacingPoint\Global.json` at startup (self-healing)
- Copy from install dir (`C:\Program Files (x86)\Conspit Link 2.0\Global.json`) to `C:\RacingPoint\Global.json`
- Ensure `AresAutoChangeConfig` is set to `"open"` in the placed file
- If `C:\RacingPoint\` directory doesn't exist, create it
- This runs as part of rc-agent startup, before ConspitLink watchdog kicks in

### GameToBaseConfig.json Mappings
- Use ConspitLink's shipped default .Base presets for all 4 games (Phase 61 handles tuning)
- Verify existing GameToBaseConfig.json has entries for all 4 venue games:
  - Assetto Corsa → default AC .Base preset
  - F1 25 → default F1 .Base preset
  - ACC / AC EVO → default ACC .Base preset
  - AC Rally → default AC Rally .Base preset (or AC preset if no specific one exists)
- If mappings are missing or point to non-existent files, fix them

### ConspitLink Restart After Config
- After writing/updating config files, restart ConspitLink using `restart_conspit_link_hardened(false)` (not crash recovery)
- ConspitLink caches config at startup — file writes without restart are ineffective (ARCHITECTURE.md Anti-Pattern 4)
- Only restart if config actually changed (compare content before/after)

### Verification Approach
- Manual game launch test on canary pod (Pod 8)
- Launch each of the 4 venue games, verify ConspitLink auto-loads the matching preset
- This is a human-verify checkpoint — Claude builds and deploys, human tests on hardware

### Claude's Discretion
- Exact startup timing (when in rc-agent init sequence to place config)
- Whether to use file copy or atomic write (temp + rename) for Global.json placement
- JSON manipulation approach (serde_json parse + modify + write vs string replace)
- Error handling for edge cases (locked files, permission denied)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### ConspitLink Architecture & Config
- `.planning/research/conspit-link/ARCHITECTURE.md` — Runtime path discovery (`C:\RacingPoint\Global.json`), config file roles, auto-detection flow, anti-patterns
- `.planning/research/conspit-link/STACK.md` — OpenFFBoard HID commands, ConspitLink config structure
- `.planning/research/conspit-link/PITFALLS.md` — P-20 contention, config placement gotchas

### Existing Code
- `crates/rc-agent/src/ffb_controller.rs` — CONSPIT_CONFIG_FILES array (line 24-27), RUNTIME_GLOBAL_JSON const (line 30), backup/verify functions, restart_conspit_link_hardened()
- `crates/rc-agent/src/ac_launcher.rs` — ensure_conspit_link_running() watchdog, enforce_safe_state()

### Prior Phase Context
- `.planning/phases/57-session-end-safety/57-CONTEXT.md` — Hardware fleet details, ConspitLink process management decisions
- `.planning/phases/58-conspitlink-process-hardening/58-01-SUMMARY.md` — Hardened restart API, config backup/verify implementation

### Requirements
- `.planning/ROADMAP.md` Phase 59 section — PROF-01, PROF-02, PROF-04 success criteria

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RUNTIME_GLOBAL_JSON` const in ffb_controller.rs — already defines `C:\RacingPoint\Global.json` path
- `CONSPIT_CONFIG_FILES` array — lists install-dir paths for Settings.json, Global.json, GameToBaseConfig.json
- `backup_conspit_configs()` / `verify_conspit_configs()` — JSON validation + backup pattern, reusable for config placement
- `restart_conspit_link_hardened(is_crash_recovery: bool)` — ready to use after config writes

### Established Patterns
- Config file management follows backup-before-write + JSON-verify-after-write pattern (Phase 58)
- ConspitLink process lifecycle: close (WM_CLOSE) → write config → restart → minimize (Phase 57/58)
- Testable `_impl(Option<&Path>)` pattern for filesystem-dependent functions (Phase 58)

### Integration Points
- rc-agent startup sequence — new config-ensure step goes here (before ConspitLink watchdog)
- `enforce_safe_state()` in ac_launcher.rs — already calls ensure_conspit_link_running(), config placement should happen before this
- Heartbeat to racecontrol — could report config state (but that's Phase 63, not this phase)

</code_context>

<specifics>
## Specific Ideas

- The root cause is well-understood: ConspitLink reads Global.json from `C:\RacingPoint\` at runtime but the file only exists in the install directory on pods
- This is a config placement fix, not a code-heavy feature — the main work is ensuring the right files are in the right places with the right content
- Pod 8 is the canary for hardware testing

</specifics>

<deferred>
## Deferred Ideas

- Custom venue-tuned .Base presets — Phase 61 (FFB Preset Tuning)
- rc-agent pre-loading presets before game launch — Phase 60 (Pre-Launch Profile Loading)
- Fleet-wide config push from racecontrol — Phase 62 (superseded by v22.0)
- Config hash reporting in heartbeats — Phase 63 (Fleet Monitoring)

</deferred>

---

*Phase: 59-auto-switch-configuration*
*Context gathered: 2026-03-24*
