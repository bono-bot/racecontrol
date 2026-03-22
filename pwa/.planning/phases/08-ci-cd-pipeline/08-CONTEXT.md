# Phase 8: CI/CD Pipeline - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Source:** Smart Discuss (infrastructure phase)

<domain>
## Phase Boundary

Create a GitHub Actions workflow that automatically builds and deploys all Docker services to Bono's VPS when code is pushed to main. Failed builds must not deploy. This is a new workflow file — no existing CI/CD exists.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key considerations:
- GitHub Actions workflow in `.github/workflows/deploy.yml`
- Trigger on push to main branch
- SSH into VPS (root@72.60.101.58) to pull and rebuild
- Build Docker images on VPS (not in GitHub — VPS has the build context)
- Deploy steps: git pull racecontrol + racingpoint-admin → docker compose build → docker compose up -d
- GitHub Secrets needed: SSH private key, VPS host IP
- Failed builds (non-zero exit) must prevent deploy step from running
- Existing comms-link relay commands available: `deploy_pull` does git pull on VPS

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cloud/compose.yml` — Docker Compose config with all 4 services
- `cloud/Caddyfile` — Caddy reverse proxy config
- VPS already has Docker, repos cloned, services running
- comms-link relay `deploy_pull` command — could be used instead of SSH

### Established Patterns
- Deploy on VPS: `cd /opt/racingpoint && git -C racecontrol pull && git -C racingpoint-admin pull && cp racecontrol/cloud/* . && docker compose up -d --build`
- All services use Docker Compose (no k8s, no Swarm)

### Integration Points
- GitHub → VPS SSH (or relay)
- VPS Docker Compose → all services

</code_context>

<specifics>
## Specific Ideas

- Keep it simple — single workflow, SSH deploy, no Docker registry
- Consider using comms-link relay instead of SSH for the deploy step (standing infra)

</specifics>

<deferred>
## Deferred Ideas

None

</deferred>

---

*Phase: 08-ci-cd-pipeline*
*Context gathered: 2026-03-22 via Smart Discuss (infrastructure phase)*
