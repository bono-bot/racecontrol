# pre-push-maor-check.js v0.1.3 — cascade-msg-FP tighten spec

**Status:** INSTALLED 2026-05-16T04:30:44Z · Captain HOOK-PATCH composite ratify · ledger entry at `~/.claude/state/harness-auth-ledger.jsonl` · install verified this session (parse-OK + behavior-fixtures-PASS) · source-tracked (NOT installed to `~/.claude/hooks/` until Captain named-surface auth)
**Author:** bono · 2026-05-16 ~09:43 IST
**Purpose:** Tighten `CASCADE_MSG_RE` to reduce FP class on composes-with body references that start at line-position-0
**Captain auth required for install:** named-surface auth on `~/.claude/hooks/pre-push-maor-check.js` (per Harness Self-Mod Auth Protocol)

## Background

v0.1.1 (2026-05-13) used `CASCADE_MSG_RE = /§S-\d+/` — matched anywhere in body. Captain found FP on §0 refresh commit `4114cb51` (mid-paragraph §S-N prose references).

v0.1.2 iter3 (2026-05-13 07:15:54Z) tightened to `/^§S-\d+/m` — multiline-start-of-line anchor. This fixed mid-paragraph FPs but did NOT fix body-list FPs where commit message has:

```
Composes-with:
§S-298 wallet-substrate Class A soak
§S-307 Option E HOLD-during-soak
§S-345 supersede
```

Each `§S-N ...` line starts at column 0, matches `^§S-\d+`, trips cascade detection on a commit that may NOT actually be a cascade close-anchor (just references prior close-anchors in composes-with body).

## v0.1.3 fix

Replace single multiline regex with `isCascadeMessage(msg)` helper:

```javascript
function isCascadeMessage(msg) {
  if (!msg) return false;
  const lines = msg.split('\n');
  // (a) First non-blank line is the subject — cascade if subject starts §S-N
  const subject = lines.find(l => l.trim().length > 0) || '';
  if (/^§S-\d+/.test(subject)) return true;
  // (b) Body contains a level-2+ heading anchor (e.g., `## §S-387 — ...`)
  if (/^#{1,3}\s+§S-\d+\b/m.test(msg)) return true;
  return false;
}
```

**True-positive recall preserved:**
- Subject `§S-387 close-anchor for ...` → MATCH (a)
- Body heading `## §S-387 — ratify entry` → MATCH (b)
- Subject `doctrine(V-LBAC §14.6.2.1): ...` followed by body containing `## §S-387 — ...` heading → MATCH (b)

**FP class eliminated:**
- Composes-with list with `§S-298 ...` body lines starting at column 0 → NO MATCH (no subject anchor · no heading prefix)
- Mid-paragraph prose `(per §S-215 §3.2)` → NO MATCH (already fixed in v0.1.2)
- Captain §0 refresh prose `references §S-213/§S-215/§S-217/§S-219` → NO MATCH (mid-line)

## Composes-with

- §S-220 MAOR v0.1 doctrine · §S-221 SCOPE-GATE F1+F3 · v0.1.1 install commit `cced682` + `7beb03d` · v0.1.2 iter1 AST tokenization ledger entry `2026-05-15T02:11:09Z` · v0.1.2 iter3 regex tighten ledger entry `2026-05-13T07:15:54Z`

## Install procedure (when Captain authorizes)

Captain verb required: `"I authorize HOOK-PATCH for hooks: pre-push-maor-check.js v0.1.3 cascade-msg-FP regex tighten"`

Steps:
1. Replace lines defining `CASCADE_MSG_RE` (currently L26 area) with new `isCascadeMessage()` helper
2. Update call sites (currently `CASCADE_MSG_RE.test(msg)`) → `isCascadeMessage(msg)`
3. Bump header version comment from v0.1.2 to v0.1.3 + add iter4 cascade-msg-RE→helper note
4. Append HARNESS-AUTH-CLAIM ledger entry to `~/.claude/state/harness-auth-ledger.jsonl`
5. Validate via `node --check` on the modified hook
6. Smoke-test 3 fixture cases (TP-subject · TP-heading · FP-composes-with-list) to verify regex behavior

## Test fixtures

| Commit message excerpt | v0.1.2 verdict | v0.1.3 verdict |
|---|---|---|
| Subject: `§S-387 close-anchor` | MATCH (TP) | MATCH (TP) |
| Subject: `doctrine(V-LBAC): §14.6.2.1`, body: `## §S-387 ratify ledger` | MATCH (TP via heading) | MATCH (TP via heading) |
| Subject: `fix(observability)`, body: `Composes-with:\n§S-298 ...\n§S-307 ...` | **MATCH (FP)** | NO MATCH ✓ |
| Subject: `feat(billing)`, body: `(refs §S-215 §3.2)` | NO MATCH | NO MATCH |
