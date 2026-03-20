---
phase: 51
slug: claude-md-custom-skills
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 51 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification (markdown files, no compiled code) |
| **Config file** | none — Phase 51 creates markdown files only |
| **Quick run command** | `ls CLAUDE.md .claude/skills/*/SKILL.md` |
| **Full suite command** | `test -f CLAUDE.md && ls .claude/skills/*/SKILL.md && echo PASS` |
| **Estimated runtime** | ~1 second |

---

## Sampling Rate

- **After every task commit:** Run `ls CLAUDE.md .claude/skills/*/SKILL.md`
- **After every plan wave:** Verify file exists + content grep for key sections
- **Before `/gsd:verify-work`:** Full content check — CLAUDE.md has pod IPs, skills have correct frontmatter
- **Max feedback latency:** 1 second

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 51-01-01 | 01 | 1 | SKILL-01 | file+content | `grep "192.168.31" CLAUDE.md` | ❌ W0 | ⬜ pending |
| 51-02-01 | 02 | 1 | SKILL-02 | file+content | `grep "disable-model-invocation: true" .claude/skills/rp-deploy/SKILL.md` | ❌ W0 | ⬜ pending |
| 51-02-02 | 02 | 1 | SKILL-03 | file+content | `grep "disable-model-invocation: true" .claude/skills/rp-deploy-server/SKILL.md` | ❌ W0 | ⬜ pending |
| 51-02-03 | 02 | 1 | SKILL-04 | file+content | `test -f .claude/skills/rp-pod-status/SKILL.md` | ❌ W0 | ⬜ pending |
| 51-02-04 | 02 | 1 | SKILL-05 | file+content | `test -f .claude/skills/rp-incident/SKILL.md` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. No test framework needed — Phase 51 produces only markdown files verified by file existence and content grep.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Claude loads CLAUDE.md context on session start | SKILL-01 | Requires opening a new Claude Code session | Open new session in racecontrol repo, ask "What are the pod IPs?" — Claude should answer without prompting |
| /rp:deploy builds and stages | SKILL-02 | Requires running the skill in Claude Code | Type `/rp:deploy` in a Claude Code session, verify it runs cargo build |
| /rp:deploy-server swaps binary | SKILL-03 | Requires live server | Type `/rp:deploy-server`, verify racecontrol restarts on :8080 |
| /rp:pod-status queries fleet | SKILL-04 | Requires live racecontrol server | Type `/rp:pod-status pod-8`, verify pod data returned |
| /rp:incident runs 4-tier debug | SKILL-05 | Requires live pods | Type `/rp:incident "Pod 3 lock screen blank"`, verify structured response |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 1s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
