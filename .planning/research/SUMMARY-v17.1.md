# Project Research Summary

**Project:** v17.1 Watchdog-to-AI Migration
**Domain:** AI-driven process recovery on Windows 11, 8-pod fleet (Rust/Axum monorepo)
**Researched:** 2026-03-25
**Confidence:** HIGH

## Executive Summary

v17.1 is not a greenfield AI system — it is a wiring and coordination milestone on top of a nearly-complete foundation. The core machinery (4-tier graduated recovery, pattern memory, Ollama integration, recovery authority registry, sentinel coordination files, JSONL audit log) already exists across rc-sentry, rc-common, rc-agent, and rc-watchdog. The milestone closes five concrete gaps: Tier 2 pattern memory is not wired into the crash handler before Ollama; the recovery authority registry exists but is not enforced at call sites; pod_monitor does not check sentinels before sending Wake-on-LAN; self_monitor restarts rc-agent independently without checking whether rc-sentry is already handling it; and MAINTENANCE_MODE has no auto-clear path after WoL.

The recommended approach is strict phase ordering driven by a single dependency: the recovery events API (racecontrol server) must land before rc-sentry starts reporting to it, and the recovery authority coordination must be established before any individual watchdog is upgraded. The production-proven incident record (standing rules + 2026-03-24 audit) makes the pitfall set unusually concrete: spawn verification failure, MAINTENANCE_MODE silent blocking, Session 0 isolation, and recovery authority conflicts have all caused real outages. The architectural answer to all of them is already built — it just needs to be enforced at the right call sites.

The key risk is not technical complexity but sequencing: upgrading rc-sentry's crash handler (Phase 2) before the recovery events API exists on the server means rc-sentry will emit reports to a missing endpoint. Upgrading self_monitor coordination (Phase 5) before rc-watchdog gets its grace window (Phase 6) leaves a window where rc-watchdog fights the newly-coordinated rc-sentry. The phase order in the architecture research must be followed exactly. Total new crates required: 0. Total new files: 1 (racecontrol/api/recovery.rs).

## Key Findings

### Recommended Stack

No new crates are required for v17.1. Every capability gap closes with existing workspace dependencies: serde_json and std::net::TcpStream for Ollama (already in rc-common), windows-service 0.8 and winapi 0.3 for Windows service lifecycle (already in rc-watchdog), reqwest 0.12 blocking for recovery event reporting (already in rc-watchdog). The one structural change needed is moving rc-sentry/src/ollama.rs into rc-common so both rc-sentry and rc-watchdog share the Ollama query path without duplication.

**Core technologies (existing — no new additions):**
- `rc-common::recovery` (ProcessOwnership, RecoveryAuthority, RecoveryLogger JSONL): coordination scaffolding — needs call-site enforcement, not new code
- `rc-sentry::debug_memory` (DebugMemory, derive_pattern_key, instant_fix): pattern memory — complete, needs wiring into handle_crash() before Ollama
- `rc-sentry::ollama` (query_crash, TcpStream HTTP): Tier 3 AI diagnosis — move to rc-common for sharing with rc-watchdog
- `rc-watchdog::session` (spawn_in_session1, WTSQueryUserToken): Session 1 process launch — the only proven path for GUI process restart from SYSTEM service context
- Sentinel files (GRACEFUL_RELAUNCH, MAINTENANCE_MODE, RCAGENT_SELF_RESTART, sentry-restart-breadcrumb.txt): cross-process coordination — already in use, needs enforcement at pod_monitor and rc-watchdog

### Expected Features

The feature research identifies a confirmed root cause for the entire v17.1 motivation: `.spawn().is_ok()` returning true while the child silently fails to start. This is not a hypothesis — it was proven across four test iterations (cmd, PowerShell, START /B, schtasks). Every feature in this milestone depends on spawn verification being solved first.

**Must have (table stakes — AI recovery is meaningless without these):**
- Spawn verification (Strategy B: health endpoint poll after every restart trigger) — eliminates the silent false-restart that corrupts all downstream pattern memory
- Recovery intent registry (sentinel file written before acting, read before acting, 2-min TTL) — prevents authority conflicts that caused the 45-minute incident
- Crash pattern memory persistence (debug-memory.json with fingerprinting + normalization) — foundation for Tier 2 instant replay; without it every crash is "new"
- Tier 0 false-positive suppression (3 consecutive health misses required before recovery) — prevents the tasklist-empty false-positive class (v17.0 30s flicker incident)
- Graduated response Tiers 1–4 wired together — replaces infinite blind restart loop
- MAINTENANCE_MODE with diagnostic reason payload — staff visibility without SSH; current empty-file format is a silent dead end

**Should have (differentiators — make recovery genuinely intelligent):**
- Crash fingerprinting with normalization (strip timestamps, PIDs, hex addresses) — without normalization, two identical crashes produce different hashes and pattern memory never fires
- Context-aware pod_monitor (reads recovery intent before WoL) — prevents WoL-into-MAINTENANCE_MODE infinite loop confirmed in 2026-03-24 audit
- Recovery telemetry to server (RecoveryAttempted WS events in fleet dashboard) — Uday-visible recovery status without SSH
- james_watchdog.ps1 replacement with same Tier 1–4 pipeline — James-side parity

**Defer to v17.2+:**
- Pattern memory confidence scoring — useful only after 2+ weeks of production data
- Silent failure detection (functional health beyond HTTP 200) — high complexity; v17.0 idle-state checks partially address this
- WMI event-based monitoring for James — reduces polling overhead but adds complexity

**Anti-features (confirmed harmful):**
- Blind restart as Tier 3 fallback — this is the dumb watchdog behavior being replaced; after Tier 3 failure, write MAINTENANCE_MODE with reason and stop
- LLM as first responder — qwen2.5:3b adds 2–10s latency; deterministic Tier 1 fixes handle 80% of real crashes in <100ms
- Process inspection (tasklist, OpenProcess) on gaming pods — EAC and iRacing anti-cheat ban; health endpoint polling is the only safe signal
- Unified super-watchdog binary — single point of failure; rc-sentry's value is surviving rc-agent death

### Architecture Approach

The target architecture enforces single recovery authority per machine: rc-sentry owns rc-agent recovery on each pod; self_monitor yields to rc-sentry when sentry is reachable; rc-watchdog pod service becomes last-resort fallback with a 30s grace window; pod_healer reads recovery events before WoL; james_monitor owns James-local services exclusively. The coordination mechanism is two-layer: sentinel files for coarse cross-process signaling (crash-safe, no IPC required), plus a new in-memory recovery events API endpoint on racecontrol for server-side pod_healer queries.

**Major components and their v17.1 changes:**
1. `racecontrol/api/recovery.rs` (NEW): POST /api/v1/recovery/events (rc-sentry reports) + GET (pod_healer queries before WoL) — only new file in the milestone
2. `rc-sentry/tier1_fixes.rs` (MODIFY): wire Tier 2 pattern memory before Ollama; add MAINTENANCE_MODE auto-clear (30min age + WOL_SENT sentinel)
3. `rc-agent/self_monitor.rs` (MODIFY): gate relaunch_self() behind rc-sentry availability check — if sentry is up, write GRACEFUL_RELAUNCH and exit; do not spawn PowerShell
4. `rc-watchdog/service.rs` (MODIFY): add 30s grace window on sentry-restart-breadcrumb.txt; add health poll after spawn_in_session1()
5. `racecontrol/pod_healer.rs` (MODIFY): query recovery events before WoL; write WOL_SENT sentinel via rc-sentry /exec before sending WoL

### Critical Pitfalls

All 8 pitfalls in the research are sourced from confirmed production incidents, not speculation. The top 5 that must be addressed in Phase 1:

1. **spawn().is_ok() does not confirm the child started** — use health endpoint poll (Strategy B: 500ms interval, 10s window) after every restart trigger; never advance to "resolved" state without HTTP 200 from the target; the proven working path is run_cmd_sync() (cmd.exe /C) not direct Command::new()
2. **MAINTENANCE_MODE silently blocks all recovery forever** — before every restart attempt, check sentinel age and emit AlertStaff if blocking; add 30-min auto-clear triggered by WOL_SENT; Phase 1 must not ship without this wired into the AI decision path
3. **Recovery authority conflicts create infinite restart loops** — wire ProcessOwnership registry enforcement at all four call sites (rc-sentry, self_monitor, rc-watchdog, pod_healer) before upgrading any individual watchdog; the 45-minute incident is confirmed history
4. **Pattern memory learns wrong fixes from server-down restarts** — tag every recorded fix with server_reachable: bool at recording time; server-down scenarios must not count toward MAINTENANCE_MODE threshold; if server is still unreachable after restart, tag fix as "inconclusive" not "resolved"
5. **AI diagnosis without sufficient log context produces harmful actions** — minimum context requirement before Ollama query: at least one non-None CrashContext field (panic message, non-zero exit code, or log tail with ERROR/WARN); never call Ollama on an empty CrashContext; deterministic Tier 1 fallback must fire when Ollama is unavailable

Additional confirmed pitfalls:
6. **Non-interactive Session 0 cannot launch Session 1 GUI processes** — all restart paths must route through run_cmd_sync() or rc-watchdog's session::spawn_in_session1(); tag pattern memory fixes with execution_context to prevent replaying interactive-context fixes from service context
7. **tasklist /FI returns empty intermittently** — 3-poll hysteresis (Tier 0) prevents false-positive crash detection; AI consultation must only fire after WatchdogState reaches Crashed, not at Suspect
8. **Bat file deployment failures (BOM, parentheses, /dev/null)** — explicit standing rule; test every new bat file with cmd /c before fleet deploy; Pod 8 canary first

## Implications for Roadmap

The architecture research defines a 6-phase build order driven by two hard dependencies: the server recovery events API must precede rc-sentry reporting, and authority coordination must be wired before individual watchdog upgrades. These phases map directly to the suggested implementation sequence.

### Phase 1: Recovery Events API (racecontrol server)

**Rationale:** Every subsequent phase requires this endpoint to exist. rc-sentry (Phase 2) reports to it. pod_healer (Phase 3) queries it before WoL. It must be deployed first.
**Delivers:** POST /api/v1/recovery/events + GET with since_secs filter; in-memory VecDeque<RecoveryEvent> capped at 200 items in AppState; server rebuild + deploy
**Addresses:** Pod_healer/rc-sentry authority conflict (pitfall 3); foundation for all coordination
**Avoids:** Deploying rc-sentry reporting to a missing endpoint
**Research flag:** Standard patterns, skip research-phase — new Axum route following existing patterns in racecontrol/api/

### Phase 2: rc-sentry Crash Handler Upgrade

**Rationale:** With the recovery events API live on the server, rc-sentry can now report. This phase wires Tier 2 pattern memory into handle_crash() and adds recovery event reporting. Start on Pod 8 canary.
**Delivers:** Tier 2 instant fix lookup before Ollama; POST to /api/v1/recovery/events after every restart attempt (success or failure); 3-miss Tier 0 false-positive suppression; non-blocking report (log warn and continue if server unreachable)
**Addresses:** Pattern memory not wired (STACK gap 1); spawn verification (pitfall 1); AI diagnosis on empty context (pitfall 7); false-positive suppression (pitfall 5)
**Avoids:** Parallel false-restart logging that corrupts pattern confidence scores
**Research flag:** Standard patterns, skip research-phase — changes are in-crate with well-understood data flows

### Phase 3: pod_healer WoL Coordination

**Rationale:** With recovery events API live and rc-sentry reporting, pod_healer can now make informed WoL decisions. This closes the WoL-into-MAINTENANCE_MODE infinite loop.
**Delivers:** pod_healer queries recovery events before escalating to WoL; WOL_SENT sentinel written via rc-sentry /exec before WoL send; skip WoL if rc-sentry restarted with verified=true within 60s
**Addresses:** WoL-into-MAINTENANCE_MODE loop (pitfall 3, confirmed 2026-03-24 audit); context-aware pod_monitor (FEATURES differentiator)
**Avoids:** pod_healer and rc-sentry fighting on the same pod within a 60s window
**Research flag:** Standard patterns, skip research-phase — pod_healer modification follows existing graduated recovery pattern

### Phase 4: MAINTENANCE_MODE Auto-Clear

**Rationale:** Can be batched with Phase 2 (same crate: rc-sentry) if timing allows, but logically depends on WOL_SENT sentinel being written by pod_healer (Phase 3). This ends permanent pod death after crash storms.
**Delivers:** MAINTENANCE_MODE age check in tier1_fixes::handle_crash(); auto-clear if age > 30min AND WOL_SENT exists; MAINTENANCE_MODE extended to JSON format with reason + diagnostic context
**Addresses:** MAINTENANCE_MODE silent blocking (pitfall 2); staff visibility without SSH; diagnosed 45-min incident root cause
**Avoids:** Permanent pod death after 3 crashes in 10min without manual sentinel clearing
**Research flag:** Standard patterns, skip research-phase

### Phase 5: self_monitor Coordination

**Rationale:** rc-sentry must be deployed and stable (Phase 2) before self_monitor is modified to yield to it. Changing self_monitor first would leave it yielding to an rc-sentry that hasn't yet been upgraded to handle graceful restarts correctly.
**Delivers:** self_monitor.rs relaunch_self() gated behind TCP connect to :8091 (2s timeout); if rc-sentry responds: write GRACEFUL_RELAUNCH + exit cleanly; only fall back to PowerShell+DETACHED_PROCESS if rc-sentry is unreachable; eliminates the port :8090 double-bind race condition
**Addresses:** self_monitor vs rc-sentry authority conflict (architecture decision 1); PowerShell orphan leak (90MB/restart) becoming rare-fallback instead of primary path
**Avoids:** Both rc-sentry and self_monitor independently deciding to restart rc-agent at different offsets
**Research flag:** Standard patterns, skip research-phase

### Phase 6: rc-watchdog Grace Window

**Rationale:** Last step because rc-watchdog is the last-resort fallback. It should defer to rc-sentry (now upgraded in Phase 2) and self_monitor (now coordinated in Phase 5). The 30s grace window needs the sentry-restart-breadcrumb.txt that rc-sentry writes — already present.
**Delivers:** rc-watchdog service.rs reads sentry-restart-breadcrumb.txt modified time before acting; skips restart if breadcrumb is < 30s old; adds health poll after spawn_in_session1() for spawn verification
**Addresses:** rc-watchdog fighting rc-sentry (architecture decision 2); spawn verification at rc-watchdog call site (pitfall 1)
**Avoids:** Three recovery systems all firing within 5 seconds on the same port
**Research flag:** Standard patterns, skip research-phase

### Phase Ordering Rationale

The ordering is driven by three hard dependencies identified in the architecture research:
- Server API before pod reporters: Phase 1 before Phase 2; reporting to a missing endpoint is a no-op that would mask the feature as "working"
- Authority coordination before individual upgrades: Phases 1–3 establish the coordination protocol; Phases 4–6 modify individual actors to use it; upgrading actors before the protocol exists makes conflicts worse (smarter fighters, same conflict)
- rc-sentry stability before self_monitor coordination: Phase 2 before Phase 5; self_monitor yielding to an unupgraded rc-sentry would yield to a system still doing blind restarts

The james_watchdog.ps1 replacement (FEATURES P2 item) is not in the 6-phase sequence above — it is a parallel stream that shares the same Tier 1–4 pipeline but does not have hard dependencies on the pod-side phases. It can be a Phase 3b or post-Phase 6 work item depending on priority.

### Research Flags

Phases with standard patterns (all 6 phases — skip research-phase):
- All 6 phases operate within well-understood existing patterns; no third-party integrations, no new crates, no external APIs; the research base is the codebase itself (read directly, HIGH confidence)

Phases that need implementation care (not research, but careful execution):
- **Phase 2 (rc-sentry):** Deploy canary to Pod 8 first; manually kill rc-agent on Pod 8 and verify recovery event appears at GET /api/v1/recovery/events?pod_id=pod-8 before fleet deploy
- **Phase 3 (pod_healer):** Verify on Pod 8: kill rc-agent, rc-sentry restarts within 20s, pod_healer skips WoL; check recovery-log.jsonl on both pod and server confirms single authority
- **Phase 5 (self_monitor):** Three-state verification required (sentry up + kill agent; sentry down + kill agent; sentry down + kill agent + restart sentry) — see architecture research build order for exact steps

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crates read directly from Cargo.toml files; all integration points traced through source; zero new dependencies needed — confirmed from workspace |
| Features | HIGH | Feature set derived from production incident log (standing rules + 2026-03-24 audit); every table-stakes feature maps to a confirmed outage; Windows Session 0 constraints confirmed from 4-iteration test record |
| Architecture | HIGH | All modified files listed by path; all data flows traced through existing code; conflict map built from actual concurrent actor behavior, not theoretical analysis |
| Pitfalls | HIGH | All 8 pitfalls sourced from confirmed production incidents with dates; no hypothetical pitfalls included; root causes traced to specific code paths |

**Overall confidence:** HIGH

### Gaps to Address

The research is unusually complete given that it is based on direct codebase analysis rather than external documentation. Two minor gaps remain:

- **Fleet alert endpoint existence:** STACK.md notes that rc-sentry Tier 4 WhatsApp escalation routes through `POST /api/v1/fleet/alert` on racecontrol, but this endpoint needs to be verified against existing routes before Phase 2. If it does not exist, it must be added alongside the recovery events API in Phase 1.
- **james_watchdog.ps1 scope:** The feature research includes james_watchdog.ps1 replacement as a P2 item, but the architecture research does not assign it a phase number. It can share james_monitor.rs infrastructure already present in rc-watchdog but needs an explicit phase slot in the roadmap, or a decision to defer to v17.2.
- **Ollama module move timing:** Moving ollama.rs from rc-sentry to rc-common is required before rc-watchdog can share it. This is a prerequisite for the james_watchdog replacement, not for the 6 pod-side phases. The roadmap should flag this as a dependency if james_watchdog replacement is in-scope for v17.1.

## Sources

### Primary (HIGH confidence — direct codebase reads)
- `crates/rc-sentry/src/{watchdog.rs, tier1_fixes.rs, debug_memory.rs, ollama.rs, main.rs}` — FSM, crash handler, pattern memory, Ollama integration
- `crates/rc-agent/src/self_monitor.rs` — GRACEFUL_RELAUNCH sentinel, relaunch_self() PowerShell path, CLOSE_WAIT logic
- `crates/racecontrol/src/{pod_healer.rs, pod_monitor.rs, cascade_guard.rs}` — graduated recovery, authority conflict detection
- `crates/rc-watchdog/src/{main.rs, service.rs, james_monitor.rs}` — service lifecycle, session 1 spawn, james 4-step recovery
- `crates/rc-common/src/recovery.rs` — ProcessOwnership registry, RecoveryAuthority enum, RecoveryLogger JSONL
- `.planning/PROJECT.md` — v17.1 milestone goals and incident history
- `CLAUDE.md` standing rules — spawn verification, MAINTENANCE_MODE, non-interactive context, recovery conflicts (confirmed from 10+ production incidents)

### Secondary (HIGH confidence — production incident logs)
- Memory file (MEMORY.md) — 2026-03-24 audit: Pods 5/6/7 MAINTENANCE_MODE simultaneous incident; v17.0 browser watchdog 30s flicker; PowerShell orphan leak; 4 bat deploy attempts needed
- Standing rules incident annotations — exact failure modes with dates, symptoms, and confirmed root causes

### Tertiary (MEDIUM confidence — external references)
- Erlang OTP Supervisor Behaviour docs — graduated restart strategy (intensity/period model)
- Windows Session 0 Isolation (MSDN) — WTSQueryUserToken pattern for Session 1 launch from services
- AI-Driven Failure Detection paper (IRJMETS 2025) — 50%+ recovery time reduction claim (single source, treat as directional)

---
*Research completed: 2026-03-25*
*Ready for roadmap: yes*
