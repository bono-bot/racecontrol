---
created: 2026-04-27T14:41:33.635Z
title: Make SEC-GATE-02 pre-commit hook diff-aware (not filename-blocking)
area: tooling
files:
  - .git/hooks/pre-commit
  - scripts/security-check.js
  - scripts/security-check.cjs
---

## Problem

SEC-GATE-02 pre-commit hook blocks commits to any "sensitive file"
(currently `racecontrol.toml`, `.env.local`, etc.) by **filename match**,
without inspecting the diff. This is too coarse:

- `racecontrol.toml` legitimately carries credentials (`terminal_secret`,
  `terminal_pin`, `sentry_service_key`) — so the file is correctly tagged
  sensitive.
- BUT non-secret edits to that same file (config flags, comments, sync
  flags like the V.11 `origin_id` fix) get blocked too, with no escape
  hatch other than `--no-verify`.

Concrete incident: 2026-04-27 PACT-V11 fix added 4 lines to
`racecontrol.toml` (one config key + 2 comment lines + 1 blank). The diff
introduced ZERO secrets. Hook blocked it. Required `--no-verify` with
explicit Uday authorization (commit `d5816c5e`). Bypassing hooks is a
banned practice per CGP — normalizing it for legitimate edits creates a
larger long-term security risk than the hook prevents.

## Solution

Replace filename-based blocking with diff-content scanning:

1. Parse `git diff --cached` for the file in question.
2. Extract only `+` lines (additions/modifications, not removals).
3. Run secret regex against ONLY those `+` lines. Patterns:
   - `terminal_secret\s*=\s*["'][^"']+["']`
   - `sentry_service_key\s*=\s*["'][^"']+["']`
   - `terminal_pin\s*=\s*["'][^"']+["']`
   - `sk-[a-zA-Z0-9]{20,}` (OpenAI/Anthropic-style keys)
   - `Bearer\s+[A-Za-z0-9_-]{20,}`
   - `password\s*=\s*["'][^"']+["']` (excluding empty/placeholder)
   - AWS access keys (AKIA*), GitHub tokens (ghp_*, gho_*), etc.
4. If any `+` line matches → BLOCK with the matched line redacted.
5. If no `+` line matches → ALLOW even if file is on the legacy sensitive
   list (the file already had secrets; this commit isn't adding new ones).
6. Edge case: if a `+` line MOVES an existing secret (delete from old
   location, add to new), it'll match the regex and block — that's fine,
   moves should still be reviewed.

Implementation lives in `racecontrol/.git/hooks/pre-commit` which sources
`scripts/security-check.{js,cjs}`. Mirror the same fix in comms-link's
hook (per CLAUDE.md "Pre-commit hooks block credential leaks" rule —
both repos have parallel hooks).

Test plan:
- Commit non-secret edit to `racecontrol.toml` → should pass without
  --no-verify.
- Commit a real secret addition (`new_password = "xyz"`) to
  `racecontrol.toml` → should still block.
- Commit edit to a non-sensitive file with an embedded `sk-` string →
  should block (catches accidental leaks anywhere).

Risk: regex false-negatives miss novel secret patterns. Mitigation:
keep the filename blocklist as a SECONDARY gate that requires explicit
`SEC_OK_NON_SECRET=1` env var to bypass — gives a record of intent
without normalizing `--no-verify`.

Refs:
- Incident: PACT-V11 commit `d5816c5e` 2026-04-27
- Authorization: Uday explicit `--no-verify` for that single commit
- Standing rule: CLAUDE.md > Security > "Pre-commit hooks block credential leaks"
- Standing rule: CGP > "NEVER skip hooks unless user explicitly requests it"
