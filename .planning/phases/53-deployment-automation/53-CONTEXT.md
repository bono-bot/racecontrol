# Phase 53: Deployment Automation - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Staging HTTP server and webterm auto-start on boot. Post-deploy verify script wired into deploy workflow. Canary-first (Pod 8) gate enforced with explicit approval before fleet rollout.

Deliverables: Task Scheduler entries for auto-start, verify script integration, /rp:deploy-fleet skill with canary gate.

</domain>

<decisions>
## Implementation Decisions

### Auto-start Method
- **Task Scheduler** — two scheduled tasks triggered at boot (`/sc onstart /ru SYSTEM`)
- Task 1: staging HTTP server (serves deploy-staging/ directory on James's machine)
- Task 2: `webterm.py` (port 9999 — Uday's phone terminal access)
- Runs at boot even before login — survives reboots reliably without James logging in

### Verify Script
- **Reuse existing** `tests/e2e/deploy/verify.sh` from v7.0 Phase 44
- Already checks: binary swap, port conflicts, fleet reconnect, build_id consistency, AI debugger routing
- Wire it into the deploy workflow — no new verification script needed

### Canary Gate Design
- **Skill-integrated** — create `/rp:deploy-fleet` skill
- Workflow: `/rp:deploy` stages binary → `/rp:deploy-fleet` deploys to Pod 8 canary → runs verify.sh → prompts James "Deploy to remaining pods? [y/N]" → deploys to pods 1-7
- Single workflow controlled by James through skills
- `disable-model-invocation: true` on `/rp:deploy-fleet` — never auto-triggered

### Claude's Discretion
- Task Scheduler task names and descriptions
- HTTP server command (python -m http.server or custom script)
- Whether deploy-fleet pushes sequentially (safe) or parallel (faster)
- Error handling when individual pods fail during fleet rollout

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Deploy Infrastructure
- `C:\Users\bono\racingpoint\deploy-staging\webterm.py` — Web terminal script to auto-start
- `tests/e2e/deploy/verify.sh` — Existing deploy verification script (from v7.0 Phase 44)
- `.claude/skills/rp-deploy/SKILL.md` — Existing /rp:deploy skill (stages binary)
- `.claude/skills/rp-deploy-server/SKILL.md` — Existing /rp:deploy-server skill (server deploy)

### Research
- `.planning/research/FEATURES.md` — Deploy automation feature landscape

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/e2e/deploy/verify.sh` — comprehensive post-deploy verification
- `deploy-staging/deploy_pod.py` — Python deploy script for individual pods
- `deploy-staging/deploy-all-pods.py` — Python fleet deploy script
- `.claude/skills/rp-deploy/SKILL.md` — build + stage skill (Phase 51)

### Established Patterns
- Task Scheduler used for kiosk on server (.23) — same pattern for James's machine
- rc-agent uses HKLM Run keys on pods — Task Scheduler is the next step up
- Pod 8 is always canary — documented in CLAUDE.md and all deploy scripts

### Integration Points
- `/rp:deploy` stages binary → `/rp:deploy-fleet` picks up from deploy-staging/
- `verify.sh` uses `RC_BASE_URL` and `TEST_POD_ID` env vars
- rc-agent :8090 exec endpoint on each pod for remote deployment

</code_context>

<specifics>
## Specific Ideas

- After auto-start, James should be able to cold-boot his machine and have deploy infrastructure ready within 60 seconds — no manual terminal opening
- /rp:deploy-fleet should show per-pod status as it deploys (Pod 8: ✓, Pod 1: deploying..., Pod 2: queued)

</specifics>

<deferred>
## Deferred Ideas

- Ansible fleet management (DEPLOY-04) — gated on WinRM/SSH validation, v9.x future
- CI/CD pipeline triggered on git push — requires Tailscale tunnel to venue LAN, future consideration
- rc-agent self-update endpoint — complex binary self-replace on Windows, future consideration

</deferred>

---

*Phase: 53-deployment-automation*
*Context gathered: 2026-03-20*
