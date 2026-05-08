---
phase: V2-Layer-1
slug: v2-design-layer-1-pos-foundation
checker: gsd-ui-checker (running on james)
checked: 2026-05-09 IST
ui_spec_commit: ed6d65dc (amends-applied state on feat/v2-design-layer-1-handoff)
ui_spec_predecessor: dd018474 (v0.1 baseline)
ui_spec_path: .planning/handoffs/UI-SPEC-V2-DESIGN-LAYER-1-POS-FOUNDATION.md
aggregate_verdict: BLOCKED
status_recommendation: do NOT promote draft to approved (Dimension 4 BLOCK on type scale + weight count)
composes_with:
  - UI-SPEC-V2-DESIGN-LAYER-1-POS-FOUNDATION.md (subject of review; on feat/v2-design-layer-1-handoff)
  - V2-DESIGN-LAYER-1-POS-FOUNDATION.md (source handoff at b5774aac on same branch)
  - Captain LOCK 2026-05-09 user prompt (tech stack)
  - V2-MASTER-STATE sections S-114 / S-122 / S-126 / S-128 (composition pointers verified)
---

# UI-SPEC Review V2 Design Layer 1 (POS .130 Foundation)

## Per-Dimension Verdict

| Dimension | Verdict | Notes |
|-----------|---------|-------|
| 1 Copywriting | PASS | All CTAs verb+noun; 5 empty states; error states with solutions; destructive confirms limited to 3 irreversible per anti-pattern #41; toast usage explicit (background events only). |
| 2 Visuals | PASS | Focal hierarchy implicit via 60/40 split on /v2/pos, xl primary CTA convention, hero amount type tier. Icon-only buttons require aria-label. POS-fixed-1920x1080 + xl primacy convention sufficient for executor. |
| 3 Color | PASS | Accent reserved-for list explicit (6 specific items: brand mark, primary CTA, destructive, fault state, focus ring, keypad active). 60/30/10 declared. Color-only signals BANNED (icon-paired mandate). DEPRECATED palette listed. |
| 4 Typography | BLOCK | 10 distinct font sizes declared (64/48/32/28/24/20/14/13/12/11) limit is 4. 4 distinct weights (400/500/600/700) limit is 2. POS-fixed-1920x1080 rationale is design-intent, not rule exception. |
| 5 Spacing | PASS | Scale 4/8/16/24/32 + POS-ergonomic 48/64/72 all multiples of 4. Exceptions justified. |
| 6 Registry Safety | PASS | No shadcn (banned by lock); no third-party registries; Captain claude.ai/design bundle is local-disk pre-vetted. Banned-deps list LOCKED. Vetting gate not applicable. |

## Aggregate Verdict

BLOCKED. Dimension 4 fails both size-count and weight-count caps.

## Top-3 Issues by Severity

### 1. BLOCK Typography size-count cap

10 distinct sizes declared (64, 48, 32, 28, 24, 20, 14, 13, 12, 11). Checker rule limit is 4.

Fix candidates:
  - Option A (kaizen-tightest): collapse to 4 sizes, e.g. 14 / 20 / 28 / 48.
  - Option B (preserve hero tier): 4 sizes = 14 / 20 / 28 / 48; mono inherits from context.
  - Option C (rule-exception): Researcher escalates to Captain that S-114 bundle ratify supersedes the 4-size rule for POS context. Requires explicit Captain disposition.

### 2. BLOCK Typography weight-count cap

4 weights declared (400, 500, 600, 700). Checker rule limit is 2.

Fix candidates:
  - Option A: 2 weights = 400 + 700. Mono medium (500) collapses to 400; body-bold (600) collapses to 700.
  - Option B: 2 weights = 400 + 600. Drop 500 mono medium and 700 display bold.
  - Option C: Captain rule-exception path.

### 3. FLAG (non-blocking) Per-surface focal element not explicitly named

Dashboard /v2/pos states 60/40 split but does not name focal element. Lookup surface does not say phone input is focal. Risk: low POS-fixed + xl-primary convention sufficient. Recommendation: researcher MAY add Focal element on entry row per surface next revision.

## Status Recommendation

Do NOT promote status: draft to approved.

Two paths forward, researcher choice:

1. Researcher revises section Typography to 4-size / 2-weight scale. Re-run checker; if Dimension 4 flips to PASS the aggregate flips to APPROVED.

2. Researcher escalates to Captain for explicit rule-exception on Dimension 4 grounded in S-114 bundle ratify + POS-fixed-1920x1080 hardware context. Captain G33-CONFIRM-FILE on rule-exception is required before checker can flip BLOCK to PASS without scale revision.

Captain LOCK on tech stack (no shadcn / no Lucide / no animation libs) honored cleanly throughout the UI-SPEC; only checker-gate failure is the type scale size-count and weight-count caps.

## Cross-References Verified

- S-114 supersession note (AMPLIFIER section 4-A) correctly captures Captain LOCK 2026-05-09 supersedes shadcn/ui dimension; kiosk @theme + Enthocentric DEPRECATED stand. Reasoning sound.
- S-122 cross-link in UNRESOLVED-1 (section 4-B): staff PIN telemetry composition with S-82 PIN-LOCKOUT coherent; Layer 1 design impact ZERO (UI policy-agnostic).
- S-126 cross-link in UNRESOLVED-2 (section 4-C): Wave 4 schema substrate for discount retrofit coherent; Layer 1 design impact NONE in V2.0.
- S-128 (section 4-D): frontmatter composes_with; awareness-only at Layer 1.
- Brand-voice substrate (section 4-D): frontmatter cross-link consistent with Copywriting Contract proactive voice rules.

## Scope Discipline Check (no surface-creep)

Verified:
- Layout & Viewport scope-pinned to POS .130 1920x1080 fixed; no PWA / Kiosk / Pod resolutions enumerated.
- Component Primitive Inventory composites (10/10) all serve Layer 1 surfaces.
- Foundation reservations (telemetry, driver-class colors) explicitly marked NOT used in Layer 1.
- Section Translation Coverage confirms source section 6 reuse matrix + section 7 subsequent layer skeletons NOT translated per scope discipline.

No surface-creep detected.

---

gsd-ui-checker 2026-05-09 IST 6-dimension review BLOCKED on Dimension 4 (typography count caps) 5/6 dimensions PASS
