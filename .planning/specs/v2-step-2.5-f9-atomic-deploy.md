# F9 — Atomic Deploy (Step 2.5 Resilience Foundation)

**Status:** SPEC-SKELETON — implementation gated on Captain explicit Step 2.5 implementation-execute verb
**Created:** 2026-05-02 (composite-ratify-event #2 substrate landing)
**Owner:** james-LEAD (per PACT-070 first-mover; bono AMPLIFIER eligible)
**Sub-sequence position:** **1st** (F9 → F8 → F7 → F12) — F9 ships first because deploy infrastructure must precede surface-coupling
**Ratifies:** PACT-20260502-001 quartet F7+F8+F9+F12 + CONSTRAINT-019 ACTIVE
**Substrate-anchor:** comms-link `7d86032` (composite-ratify-event #2 minimal substrate)

---

## Goal

Single-source atomic deploy across all V2.0 surfaces (Admin / POS / Kiosk / PWA / racecontrol / comms-link). Eliminates V1 fleet-drift plague where surfaces deploy independently and diverge silently.

---

## Contract

**Binding:** CONSTRAINT-019 — *"V2.0 deploy of any surface MUST execute through F9 deploy.sh OR deploy.yml; manual surface-only deploys are forbidden post-F9 ratify."*

**Single source of truth:**
- `racecontrol/scripts/deploy/deploy.sh` (manual invocation, dev / staging / canary)
- `racecontrol/.github/workflows/deploy.yml` (CI invocation, scheduled / tag / manual-dispatch)

**Forbidden post-F9-ship:**
- `pm2 restart` directly on a surface (must go through deploy.sh)
- Direct binary copy (scp / ren / move) without deploy.sh wrapper
- Surface-only `npm run build` followed by manual restart
- Any deploy that skips SWAPLOG.md append + manifest signing

---

## Composes-with

- **CONSTRAINT-019** (this contract; binding-text in PACT-CHARTER §V2.0)
- **DMP (Deploy Manifest Protocol)** — every PLAN.md `deploy:` section MUST cite F9 path post-F9-ship; existing DMP doc (`docs/ARCHITECTURE.md` §22) extends with F9-attribution requirement
- **CGP Standing Rule #16 — Definition of Shipped** (DEPLOYED-PARTIAL → DEPLOYED → VERIFIED chain runs through F9)
- **Cloud parity rule** — F9 deploy.sh MUST execute on both venue (.23) and Bono VPS in single invocation
- **P5 One-supervisor** (V2 architectural milestone; F9 deploy unit definitions feed P5 supervisor)
- **Pre-existing infra:** `deploy-server.sh` (v3.0 MMA-hardened) + `deploy-pod.sh` + `stage-release.sh` + `gate-check.sh` — F9 wraps these with single-entry semantic

---

## Surface enumeration (all targets)

| Surface | Current deploy path | F9 entry |
|---------|---------------------|----------|
| racecontrol binary (.23 + cloud) | deploy-server.sh | `deploy.sh racecontrol` |
| rc-agent (Pods 1-8) | deploy-pod.sh + rc-sentry/exec | `deploy.sh rc-agent --pod=N` or `deploy.sh rc-agent --fleet` |
| Web/POS app (.23:3200 + cloud) | npm build + scp + schtasks | `deploy.sh web` |
| Kiosk app (.23:3300 + cloud) | npm build + scp + schtasks | `deploy.sh kiosk` |
| Admin app (.23:3201 + cloud) | npm build + scp + schtasks | `deploy.sh admin` |
| comms-link daemon (Bono VPS) | git_pull + pm2 restart | `deploy.sh comms-link` |
| PWA (cloud) | next build + cloud rebuild | `deploy.sh pwa` |

**Atomicity contract:** within a surface, deploy is all-or-nothing (binary swap + config + dependent services together). Across surfaces, deploys are sequenced and SWAPLOG-appended individually (not transactional cross-surface — out of scope for F9 v1).

---

## Out of scope (F9 v1)

- Cross-surface transactional rollback (F9 v1 is per-surface atomic; cross-surface transactionality deferred to F9 v2)
- Auto-canary (F9 v1 uses `--canary=podN` flag on caller; auto-canary is operational follow-up)
- Blue-green deployment (out of scope; current ren-based swap is sufficient at venue scale)

---

## Implementation gating

**Phase 1 (this commit):** spec-shape only. CONSTRAINT-019 binding-text ACTIVE in PACT-CHARTER. No code change in `scripts/deploy/`.

**Phase 2 (gated on Captain Step-2.5-implement verb):**
- Author `racecontrol/scripts/deploy/deploy.sh` (entry-point router)
- Wire to existing deploy-server.sh / deploy-pod.sh / stage-release.sh
- Author `racecontrol/.github/workflows/deploy.yml` (CI mirror)
- HALO probe `deploy-source-attribution` (verifies all surface-version commits trace to F9 invocation)

**Phase 3 (gated on F9 v1 7-day soak PASS):**
- CONSTRAINT-019 enforcement flips from honor-system → fail-CLOSED hook (PreToolUse on Bash matching `pm2 restart` outside F9 context)
- Razorpay PR-merge gate becomes mechanical (composes with CONSTRAINT-020 F12 ACTIVE)

---

## NOT TESTED (post-spec-shape)

- F9 deploy.sh entry-point router invocation (Phase 2 implementation)
- Cross-surface sequencing across all 7 targets enumerated above
- Failure-mode behavior when deploy fails mid-fleet (rollback semantics)
- Cloud parity automatic on F9 invocation (currently manual sequence)
- HALO probe `deploy-source-attribution` (Phase 2 substrate)
- CONSTRAINT-019 fail-CLOSED hook (Phase 3 substrate)

---

## Stale-at

Durable until F9 Phase 2 implementation lands OR scope re-shape via sibling-PACT.
