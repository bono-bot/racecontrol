# Phase 215: Self-Improving Intelligence - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Self-improving loop for the auto-detect pipeline. Every run contributes to a pattern database. The system proposes and autonomously applies improvements to its own detection and fix coverage. Includes self-patch capability (modifying its own scripts), toggle control, and methodology adherence.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase.

Key constraints from REQUIREMENTS.md:
- LEARN-01: Pattern tracking -- every finding logged with category, frequency, resolution
- LEARN-02: Trend detection -- repeated findings across runs trigger improvement proposals
- LEARN-03: Improvement proposals -- structured format with rationale and risk assessment
- LEARN-04: Proposal review queue -- proposals stored for James/Bono review
- LEARN-05: Auto-apply safe proposals -- low-risk proposals applied automatically
- LEARN-06: Pattern database persistence -- survives restarts, git-tracked
- LEARN-07: Self-patch loop -- system can modify its own detection/fix scripts
- LEARN-08: Self-patch methodology -- follows Cause Elimination, tests before applying
- LEARN-09: self_patch_enabled toggle (default=false, requires explicit enable)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- audit/results/findings.json -- per-run findings output from cascade.sh
- audit/results/auto-detect-cooldown.json -- per-pod+issue cooldown tracking
- audit/results/auto-detect-config.json -- runtime toggle config (auto_fix_enabled pattern)
- scripts/cascade.sh -- _emit_finding() writes structured findings
- LOGBOOK.md -- incident + commit log pattern

### Established Patterns
- JSON file persistence in audit/results/
- jq for JSON manipulation in bash
- Toggle pattern: read config JSON at call time (no restart needed)
- Git commit from scripts: git add + git commit + git push

### Integration Points
- Pattern database populated after each auto-detect run (post-notification step)
- Trend detection runs after pattern update
- Self-patch proposals written to audit/results/proposals/
- Self-patch applied via git commit from the pipeline itself

</code_context>

<specifics>
## Specific Ideas

- Pattern database: audit/results/pattern-db.json (array of findings with frequency counts)
- Proposals: audit/results/proposals/*.json (one per proposal, with status field)
- Self-patch: only modifies files in scripts/detectors/ and scripts/healing/ (scoped)
- Safe auto-apply criteria: threshold adjustment only (no new code generation)

</specifics>

<deferred>
## Deferred Ideas

- AI-generated detection scripts (requires LLM integration) -- future
- Cross-venue pattern sharing -- future

</deferred>
