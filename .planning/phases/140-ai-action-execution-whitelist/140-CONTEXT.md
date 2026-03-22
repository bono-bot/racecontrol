# Phase 140: AI Action Execution Whitelist - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Parse AI debugger Tier 3/4 responses for structured safe actions from a whitelist. Execute whitelisted actions automatically. Log all executed actions to activity_log. Block process-killing actions during anti-cheat safe mode.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Whitelist actions: kill_edge, relaunch_lock_screen, restart_rcagent, kill_game, clear_temp
- AI responses from Ollama/Anthropic need structured action format — define a JSON action schema
- rc-agent ai_debugger.rs try_auto_fix() is Tier 1 (deterministic). This phase adds execution for Tier 3/4 (LLM-suggested)
- Actions are executed in rc-agent (pod-side), not on the server
- Server pod_healer escalate_to_ai() sends AI suggestions to dashboard — now also parse for actions
- safe_mode_active gate on process-killing actions (AIACT-04)
- Standing rule #74: #[cfg(test)] guards on all system commands

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- ai_debugger.rs try_auto_fix() (line 491-567) — Tier 1 fix execution pattern
- ai_debugger.rs fix_kill_error_dialogs(), fix_frozen_game() — existing process kill patterns
- ai.rs query_ai() (line 157-241) — returns (response, model_used)
- pod_healer.rs escalate_to_ai() (line 607-708) — AI escalation + suggestion storage
- lock_screen.rs close_browser(), launch_browser() — already pub from Phase 137

### Integration Points
- rc-agent/src/ai_debugger.rs — add action parsing after Tier 3/4 response
- racecontrol/src/ai.rs — structured action format in AI prompt
- racecontrol/src/pod_healer.rs — parse AI response for actions

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
