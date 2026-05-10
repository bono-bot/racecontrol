# pre-v2-edit-rca-check.js — Canonical Spec (bilateral hook)

**Version**: 0.1.0
**Authored**: 2026-05-10 ~01:30 IST · bono-LEAD
**Ratified**: comms-link/V2-MASTER-STATE.md §S-174 (Phase 1 of 4-phase enforcement plan)
**Doctrine**: extends §S-146 V1↔V2 RCA rule + COGNITIVE-GATE-PROTOCOL.md SR #21+#22
**Sibling pattern**: §S-159 `pre-mma-duplicate-check.js` (same install model: bono canonical → james path-(B) re-implement → both register MANIFEST sentinel hashes)

---

## Purpose

PreToolUse Edit/Write matcher hook that BLOCKS write attempts to V2-foundational-boundary surfaces unless:
- **(a)** an RCA artifact exists at `racecontrol/.planning/specs/v2/RCA/<surface-name>/*.md`, OR
- **(b)** a mechanism-trust-check log exists at `racecontrol/.planning/specs/v2/MECHANISM-TRUST/<surface-name>-<date>.json` within 30 days (configurable), OR
- **(c)** `V2_RCA_BYPASS=1` env override is set (logs to `comms-link/data/v2-rca-bypass-log.jsonl` sentinel for audit).

Closes the empirical gap that §S-146 is text-only doctrine; without this hook, the rule fires only when authors remember to apply it (≥1 repeat-violation per 30d profile).

---

## Trigger

PreToolUse hook on `Edit` and `Write` tools. Other tools (Read, Bash, Grep, Glob, TodoWrite, Agent, etc.) pass through unchanged.

---

## Stdin payload schema (Claude Code PreToolUse contract)

```json
{
  "session_id": "<string>",
  "tool_name": "Edit" | "Write",
  "tool_input": {
    "file_path": "<absolute path>",
    ...
  }
}
```

Hook reads JSON from stdin (fd 0). Empty stdin OR parse failure → fail-open (exit 0).

---

## Behavior

1. **Read + parse stdin** (fail-open on error).
2. **Pass-through if `tool_name` is not `Edit` or `Write`** → exit 0.
3. **Pass-through if `tool_input.file_path` is missing or empty** → exit 0.
4. **Normalize file_path** to relative-to-repo-root form using env-configurable repo roots.
5. **Match against surface list** at `racecontrol/.planning/hooks-bilateral/v2-foundational-surfaces.json` (configurable via env). Glob → regex conversion: `**` → `.*`, `*` → `[^/]*`.
6. **If no surface match** → exit 0 (pass-through).
7. **Check exempt list** → if file matches an exempt glob, exit 0.
8. **Check `V2_RCA_BYPASS=1`** → log to sentinel + emit stderr disclosure + exit 0 (PASS).
9. **Check (a) RCA artifact**: scan `racecontrol/.planning/specs/v2/RCA/<surface.name>/` for any `*.md` file → if found, exit 0.
10. **Check (b) mechanism-trust-check log**: scan `racecontrol/.planning/specs/v2/MECHANISM-TRUST/` for files matching `<surface.name>-*.json` → if any has mtime within `V2_TRUST_TTL_DAYS` (default 30), exit 0.
11. **BLOCK**: emit stderr message with surface name, expected paths, override instructions; exit 2.

Exit codes:
- `0` = PASS / pass-through (default; non-V2-surface, RCA exists, trust-check valid, override active, error fail-open)
- `2` = BLOCK (V2 surface + no RCA + no trust-check + no override)

---

## Configuration (env vars; all optional)

| Var | Default | Purpose |
|---|---|---|
| `V2_RCA_RACECONTROL_ROOT` | bono `/root/racecontrol` · james `C:/Users/bono/racingpoint/racecontrol` | racecontrol repo root |
| `V2_RCA_COMMSLINK_ROOT` | bono `/root/comms-link` · james `C:/Users/bono/racingpoint/comms-link` | comms-link repo root |
| `V2_RCA_SURFACE_LIST` | `<racecontrol>/.planning/hooks-bilateral/v2-foundational-surfaces.json` | surface list source |
| `V2_RCA_BASE_DIR` | `<racecontrol>/.planning/specs/v2/RCA` | RCA artifact directory |
| `V2_RCA_TRUST_BASE_DIR` | `<racecontrol>/.planning/specs/v2/MECHANISM-TRUST` | trust-check log directory |
| `V2_RCA_TRUST_TTL_DAYS` | `30` | trust-check validity window in days |
| `V2_RCA_BYPASS_LOG` | `<comms-link>/data/v2-rca-bypass-log.jsonl` | bypass sentinel log path |
| `V2_RCA_BYPASS` | (unset) | set to `1` to override the BLOCK |
| `V2_RCA_BYPASS_REASON` | (unset) | required-with-bypass; logged to sentinel |
| `V2_RCA_NOW_MS` | `Date.now()` | testability override |

Hook auto-detects pilot identity via path inspection (presence of `/root/racecontrol` vs `C:/Users/bono/racingpoint/racecontrol`) for the version-string suffix; defaults to `unknown` if neither.

---

## Surface match algorithm

For each entry in surface list `surfaces[]`:
1. Resolve repo root: `repoRoot = (entry.repo === "racecontrol") ? V2_RCA_RACECONTROL_ROOT : V2_RCA_COMMSLINK_ROOT`.
2. Build absolute glob: `absGlob = repoRoot + "/" + entry.glob`.
3. Convert glob to regex: escape `.`, `**` → `.*`, `*` → `[^/]*`.
4. Test `tool_input.file_path` against the regex (anchored: `^<re>$`).
5. First match wins; record `entry` for downstream logic.

Exempt list checked AFTER surface match: if file_path matches an exempt glob within the same repo, override surface match with PASS.

---

## Bypass sentinel format (JSONL append)

Each bypass appends one line to `comms-link/data/v2-rca-bypass-log.jsonl`:

```json
{"ts":"2026-05-10T01:30:00.000Z","hook":"pre-v2-edit-rca-check.js","version":"0.1.0-bono","pilot":"bono","filepath":"<absolute>","surface":"<surface name>","bypass_reason":"<from V2_RCA_BYPASS_REASON>","bypass_env_var":"V2_RCA_BYPASS"}
```

Missing `V2_RCA_BYPASS_REASON` is permitted (audit-discipline reminder emitted to stderr) but reason field stored as `(unspecified)`.

---

## BLOCK message format (stderr)

```
[pre-v2-edit-rca-check 0.1.0-<pilot>] BLOCK: <filepath>
  matches V2-foundational-boundary surface "<name>" (class: <class>)

Per §S-146 V1↔V2 RCA rule + §S-174 mechanism-trust-check upstream extension,
Edit/Write to this surface requires ONE of:
  (a) RCA artifact at <abs>/RCA/<name>/<hash>.md
  (b) mechanism-trust-check log at <abs>/MECHANISM-TRUST/<name>-<date>.json
      within <ttl> days
  (c) V2_RCA_BYPASS=1 V2_RCA_BYPASS_REASON="<reason>" env override
      (logged to <bypass_log>)

See:
  /root/.claude/projects/-root/memory/feedback_v1_dependent_v2_root_cause_before_proceeding.md
  /root/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md
  /root/.claude/projects/-root/memory/project_s146_enforcement_rca_20260510.md
  comms-link/V2-MASTER-STATE.md §S-174
```

---

## Self-test cases (≥5 required, sibling-of §S-159 pattern)

| # | Name | Scenario | Expected exit | Expected stderr substring |
|---|---|---|---|---|
| 1 | NON-SURFACE-PASS | `Edit` on `<racecontrol>/README.md` (not in surface list) | 0 | (none) |
| 2 | SURFACE-NO-EVIDENCE-BLOCK | `Edit` on `<racecontrol>/crates/rc-agent/src/main.rs`, no RCA, no trust-check | 2 | `BLOCK` + surface name `rc-agent` |
| 3 | SURFACE-WITH-RCA-PASS | Same as #2 but with `<racecontrol>/.planning/specs/v2/RCA/rc-agent/test.md` present | 0 | (PASS) |
| 4 | SURFACE-WITH-TRUST-CHECK-PASS | Same as #2 but with `<racecontrol>/.planning/specs/v2/MECHANISM-TRUST/rc-agent-2026-05-10.json` present (mtime fresh) | 0 | (PASS) |
| 5 | OVERRIDE-PASS | Same as #2 but `V2_RCA_BYPASS=1 V2_RCA_BYPASS_REASON="kaizen-test"` | 0 | `OVERRIDE active` + sentinel log written |
| 6 (defensive) | NOT-EDIT-NOT-WRITE-PASS | `Bash` tool with arbitrary command | 0 | (pass-through) |

Self-test runner is a bash script at `~/.claude/hooks/test-pre-v2-edit-rca-check.sh` that synthesizes stdin payloads and invokes the hook with `node`. Test 5 verifies sentinel JSONL append. Test 4 includes mtime-aware setup (touches with current timestamp).

---

## Bilateral install (path-(B) re-implementation pattern, sibling-of §S-159)

**Bono-side install (canonical, this turn):**
1. Copy hook to `/root/.claude/hooks/pre-v2-edit-rca-check.js` (chmod +x).
2. Copy self-test to `/root/.claude/hooks/test-pre-v2-edit-rca-check.sh` (chmod +x).
3. Wire up `~/.claude/settings.json` PreToolUse Edit/Write matchers (block at exit 2).
4. Run self-test: 5/5 PASS expected.
5. Ship spec + surface list + this commit to racecontrol git.
6. Bilateral NOTIFY james via send-message.js.

**James-side install (path-(B) re-implementation):**
1. `git_pull` racecontrol → spec + surface list now on disk Windows-side.
2. Re-implement `pre-v2-edit-rca-check.js` from this spec, using Windows paths:
   - `V2_RCA_RACECONTROL_ROOT=C:/Users/bono/racingpoint/racecontrol`
   - `V2_RCA_COMMSLINK_ROOT=C:/Users/bono/racingpoint/comms-link`
   - Hook location: `C:/Users/bono/.claude/hooks/pre-v2-edit-rca-check.js`
   - Self-test location: `C:/Users/bono/.claude/hooks/test-pre-v2-edit-rca-check.sh` (Git Bash) or `.bat` (CMD)
   - Use `node` available in PATH (verified Windows-side per §S-159 pattern).
3. Wire up Windows `~/.claude/settings.json`.
4. Run self-test: 5/5 PASS expected.
5. Reply to bono via comms-link bilateral msg with sentinel hash + confirm-or-amend on spec.

**Phase 4 MANIFEST register (deferred to Phase 4 ship):**
After both pilots' self-tests pass, register sentinel hashes in `racecontrol/.planning/hooks-bilateral/MANIFEST.json` (Phase 4 scaffold). MANIFEST entry:

```json
{
  "hook": "pre-v2-edit-rca-check.js",
  "version": "0.1.0",
  "spec_path": "racecontrol/.planning/hooks-bilateral/pre-v2-edit-rca-check.spec.md",
  "surface_list_path": "racecontrol/.planning/hooks-bilateral/v2-foundational-surfaces.json",
  "bono_sentinel_hash": "<sha256 of bono-side hook file>",
  "james_sentinel_hash": "<sha256 of james-side hook file post-install>",
  "bono_install_date": "2026-05-10T01:30:00+05:30",
  "james_install_date": "<TBD>",
  "override_env_var": "V2_RCA_BYPASS",
  "last_bilateral_test_date": "<TBD>"
}
```

---

## Edge cases / fail-open conditions

- **Empty stdin**: exit 0 (pass-through). Some hook invocations have no payload.
- **JSON parse failure**: exit 0 (fail-open; do not BLOCK on hook bug).
- **Missing surface list file**: exit 0 (no surfaces to enforce; hook degrades gracefully).
- **Missing RCA base dir**: counts as "no RCA artifact" → triggers BLOCK if no other PASS condition.
- **Missing trust-check base dir**: same.
- **Missing bypass log path**: bypass still permitted; emit stderr warning that audit-trail incomplete.
- **Race: file_path edited mid-check**: hook re-reads on each invocation; no caching.
- **Symlinks**: not resolved; pattern matches the literal path Claude Code passes.

---

## Composes-with

- §S-146 V1↔V2 RCA rule (canonical doctrine)
- §S-174 mechanism-trust-check upstream extension (this rule's parent)
- §S-159 pre-mma-duplicate-check.js (sibling install pattern)
- §S-166 mma-model-registry.js + validateRoleAssignment (sibling code-level enforcement)
- COGNITIVE-GATE-PROTOCOL.md SR #21 + SR #22 (textual statement of the rule)
- Universal Sync 10-target sub-rule (closes §S-146.4-class G9 forever via Phase 3 follow-up)

---

## NOT TESTED at v0.1.0

- james-side path-(B) install on Windows (pending james session)
- bilateral parity verify (Phase 4)
- 30-day cache TTL boundary edge case
- Symlink + canonical-path race
- Concurrent invocation race (multiple Edit calls)
- False-positive rate after first 30 days (re-evaluate; tune surface list)
- Backfill audit on existing 9 foundational surfaces (Phase 6, gates on Captain disposition)
- Behavior under git pull mid-edit
- Behavior with files outside both repo roots (e.g., /tmp/*.rs scratch)

---

## Version history

- **0.1.0** (2026-05-10) — initial canonical spec; bono-LEAD ship at §S-174.
