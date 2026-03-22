# Phase 141: WARN Log Scanner - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add WARN log scanning to the pod healer cycle. Count WARN lines in last 5 minutes from the racecontrol log. When threshold exceeded (>50/5min), trigger AI escalation with grouped/deduplicated log context.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Racecontrol uses tracing with tracing-appender — logs go to logs/racecontrol-{date}.jsonl
- Each JSONL line has timestamp, level, fields.message, target
- Scanner runs inside the healer cycle (every 2 minutes by default)
- Deduplication: group identical WARN messages, show once with count annotation
- Escalation fires exactly once per threshold breach — use a cooldown or "last escalated" timestamp
- AI escalation reuses existing query_ai() from ai.rs

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- pod_healer.rs heal_all_pods() (line 111-377) — main healer cycle
- pod_healer.rs escalate_to_ai() (line 607-708) — AI escalation pattern
- ai.rs query_ai() — Ollama/Claude fallback chain (now fixed to point at James .27)
- logs/racecontrol-{date}.jsonl — structured JSON log format

### Integration Points
- racecontrol/src/pod_healer.rs — add scan_warn_logs() call in healer cycle
- New function: scan_warn_logs() reads JSONL, counts WARNs, groups, returns summary
- Reuse escalate_to_ai() with WARN context as the diagnostic

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
