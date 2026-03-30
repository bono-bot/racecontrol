# Phase 266: Quality Gate & Audit - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning

<domain>
## Phase Boundary
Final quality gate: MMA design audit (minimum 3-model consensus), touch verification on actual pod hardware, cross-app consistency check between web and kiosk.
Requirements: DQ-03, DQ-04
</domain>

<decisions>
## Implementation Decisions
### MMA Audit
- Use Perplexity MCP council (3+ models) for design audit
- Focus on: visual consistency, accessibility, touch targets, color contrast, animation performance
- All P1 findings must be fixed before milestone ships
- P2 findings triaged (fix or defer with justification)

### Touch Verification
- Physical touchscreen test on at least 2 pods
- Verify: pod selection, game launch, billing, staff tools, leaderboard
- Zero hover-only interactions in kiosk
- All touch targets ≥ 44x44px

### Claude's Discretion
Audit prompt design, which models to use, fix prioritization.
</decisions>

<code_context>
## Existing Code Insights
- MMA audit protocol documented in CLAUDE.md and memory
- Perplexity MCP council tool available (gpt54, claude_opus, gemini_pro, claude_sonnet, nemotron)
- rc-doctor for fleet health verification
</code_context>

<specifics>
- Cross-app consistency: same tokens, same status colors, same Racing Red accent
- Verify deprecated orange #FF4400 not used anywhere
- Verify all NEXT_PUBLIC_ vars correct before final deploy
</specifics>

<deferred>
None — this is the final quality gate.
</deferred>
