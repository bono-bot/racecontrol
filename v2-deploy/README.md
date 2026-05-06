# v2-deploy — F9 Atomic-Deploy v1 substrate (Phase 0.3)

**Status:** SUBSTRATE-LANDED — Phase 0.3 substrate-class child-PACT under composite-ratify-event #4 OPTION-A §2.1 pre-ratify cascade
**PACT:** PACT-20260503-004
**Spec source:** `.planning/specs/v2-step-2.5-f9-atomic-deploy.md` (on amend1-section1-heart-admin-substrate; commit `73dcf5f0`)
**Binding contract:** CONSTRAINT-019 — *"V2.0 deploy of any surface MUST execute through F9 deploy.sh OR deploy.yml; manual surface-only deploys are forbidden post-F9 ratify."*

---

## What this directory contains

```
v2-deploy/
├── README.md                              ← this file (substrate doc + invocation guide)
├── deploy.sh                              ← F9 v1 entry-point router
├── SWAPLOG.md                             ← append-only deploy ledger (audit trail)
├── MANIFEST.schema.json                   ← deploy-manifest.json structure (per-deploy artifact)
└── halo-probe-deploy-source-attribution.sh ← HALO probe (verifies deploys trace to F9 invocation)
```

---

## Invocation

### Manual (dev / staging / canary)

```bash
# Surface deploys
bash v2-deploy/deploy.sh racecontrol           # racecontrol binary (.23 + cloud)
bash v2-deploy/deploy.sh rc-agent --pod=N      # single-pod rc-agent
bash v2-deploy/deploy.sh rc-agent --fleet      # all 8 pods rc-agent
bash v2-deploy/deploy.sh web                   # Web/POS app (.23:3200 + cloud)
bash v2-deploy/deploy.sh kiosk                 # Kiosk app (.23:3300 + cloud)
bash v2-deploy/deploy.sh admin                 # Admin app (.23:3201 + cloud)
bash v2-deploy/deploy.sh comms-link            # comms-link daemon (Bono VPS)
bash v2-deploy/deploy.sh pwa                   # PWA (cloud)

# Flags
--canary=podN          # canary deploy (rc-agent only; goes to single pod)
--dry-run              # print plan without executing
--no-swaplog           # skip SWAPLOG append (debug / E2E only)
--manifest=PATH        # write manifest JSON to PATH (default: v2-deploy/manifests/<surface>-<ts>.json)
```

### CI (scheduled / tag / manual-dispatch)

`.github/workflows/deploy.yml` (CI mirror — separate file; this README documents the contract).

---

## What F9 v1 does

For each deploy invocation, F9 v1:

1. **Build manifest** — record git SHA + author + surface + timestamp + UTC IST + binary hash + caller info
2. **Verify pre-conditions** — git working tree clean (or `--allow-dirty`), CI green on HEAD (or `--allow-red-ci`), CONSTRAINT-019 binding-text present
3. **Delegate to existing scripts** — wraps the existing per-surface deploy path:
   - `racecontrol` → `scripts/deploy/deploy.sh racecontrol` → `deploy-server.sh`
   - `rc-agent` → `scripts/deploy/deploy-all-pods.sh` (or `deploy-pod.sh` for `--canary`)
   - `web` / `kiosk` / `admin` → `scripts/deploy/deploy-nextjs.sh <surface>`
   - `comms-link` → relay `git_pull` + `pm2_restart` to Bono VPS
   - `pwa` → cloud Bono `git_pull` + `npm run build` + pm2 restart
4. **Append SWAPLOG.md** — one row per surface deploy (idempotent / never-rewrite)
5. **Sign manifest** — write `v2-deploy/manifests/<surface>-<ts>.json` (HMAC signing deferred to F9 v2)
6. **Post-deploy probe** — `halo-probe-deploy-source-attribution.sh` confirms deployed binary's commit-SHA appears in SWAPLOG with F9 invocation source-tag

## What F9 v1 does NOT do

Per the spec **Out of scope** section:

- Cross-surface transactional rollback (per-surface atomic only; cross-surface = F9 v2)
- Auto-canary (manual `--canary=podN` flag only)
- Blue-green deployment (out of scope at venue scale)

Phase-class deferrals:
- HMAC manifest signing (F9 v1 writes manifest; signing = follow-on)
- CONSTRAINT-019 fail-CLOSED hook (Phase 3 per spec — gates on F9 v1 7-day soak PASS)
- Razorpay PR-merge gate mechanical hook (composes with CONSTRAINT-020 F12)

---

## SWAPLOG.md format

Append-only ledger; one row per surface deploy. Format:

```
| ts (UTC) | ts (IST) | surface | git_sha | author | manifest_path | exit_code | duration_s | notes |
```

Schema invariants:
- `ts (UTC)` is RFC3339 (`2026-05-03T05:30:00Z`)
- `ts (IST)` is human-readable (`2026-05-03 11:00 IST`)
- `surface` is one of the 7 enumerated in the F9 spec
- `git_sha` is the full 40-char SHA of the deploy-source commit
- `manifest_path` is relative to `racecontrol/`
- `exit_code` is 0 on success; non-zero is documented in `notes`
- Never delete rows; append-only; rotate annually via separate `SWAPLOG-YYYY.md` archive

---

## MANIFEST.schema.json

Per-deploy artifact written to `v2-deploy/manifests/<surface>-<ts>.json`. Captures:

- `pact`: "PACT-20260503-004" (or amendment as it evolves)
- `f9_version`: "v1"
- `surface`: enum
- `git_sha`: 40-char
- `git_branch`: branch at deploy
- `git_author`: commit author
- `git_committer`: actual deployer (may differ from author for backports)
- `ts_utc`: RFC3339
- `ts_ist`: human readable
- `binary_hash`: SHA256 of deployed artifact (or "n/a" for source-deployed surfaces)
- `caller`: `f9-deploy-sh` | `f9-deploy-yml` | other (for CONSTRAINT-019 violation detection)
- `pre_conditions`: `{ git_clean: bool, ci_green: bool, constraint_019_binding: bool }`
- `delegate`: which sub-script was invoked + its exit code
- `swaplog_row`: line index in SWAPLOG.md (for cross-reference)

---

## HALO probe — `deploy-source-attribution`

Bash script at `v2-deploy/halo-probe-deploy-source-attribution.sh`. Logic:

1. For each running surface (read from manifest at `~/.racing-point/deploy-state.json` if present, else probe pm2 list / netstat):
   - Extract deployed binary's commit-SHA (via embedded version, build_id env var, or pm2 metadata)
   - Grep SWAPLOG.md for that SHA + surface combo
   - If SWAPLOG row exists with `caller=f9-*` → PASS for that surface
   - If SWAPLOG row exists with `caller=other` → CONSTRAINT-019 VIOLATION (manual deploy detected)
   - If no SWAPLOG row → ATTRIBUTION-GAP (deploy predates F9 v1 ship; whitelist via `v2-deploy/.attribution-whitelist`)
2. Exit codes: 0 = all PASS (or all whitelisted); 1 = ATTRIBUTION-GAP outside whitelist; 2 = CONSTRAINT-019 VIOLATION

Probe is advisory in F9 v1 (Phase 2 of spec). Becomes fail-CLOSED in Phase 3 (gates on 7-day soak PASS per spec).

---

## Composes-with

- **F9 spec** (`.planning/specs/v2-step-2.5-f9-atomic-deploy.md` on amend1-section1-heart-admin-substrate)
- **CONSTRAINT-019** binding-text in `comms-link/PACT-CHARTER.md` §V2.0
- **DMP** (Deploy Manifest Protocol) — `docs/ARCHITECTURE.md` §22 (extends with F9-attribution requirement post-F9-ship)
- **CGP Standing Rule #16** — DEPLOYED-PARTIAL → DEPLOYED → VERIFIED chain runs through F9
- **Cloud parity rule** — F9 deploy.sh executes on both venue (.23) and Bono VPS in single invocation
- **P5 One-supervisor** — F9 deploy unit definitions feed P5 supervisor (V2 architectural milestone)
- **Pre-existing infra:** `scripts/deploy/deploy.sh` (V1 router; F9 v1 wraps without replacing) + `deploy-server.sh` v3.0 + `deploy-pod.sh` + `stage-release.sh` + `gate-check.sh`

## Substrate ratify chain

- **PACT-20260502-001** — F7+F8+F9+F12 quartet substrate FILE event (composite-ratify-event #2)
- **PACT-20260503-004** — Phase 0.3 F9 v1 substrate authoring (this PACT)
- **Composite-ratify-event #4 OPTION-A §2.1** — pre-ratify Level B for substrate-class Phase 0.x cascade
- **Auto-ratify gate (this PACT):** MMA VERIFY ≥4.0 + 3 vendor-disjoint + CGP H3 evidence + V2-MASTER-STATE.md §S-N append + bono AMPLIFIER ABSORB

## NOT TESTED (post-substrate-shape this PACT)

- F9 deploy.sh actual surface-deploy invocation against live racecontrol/admin/web/kiosk/PWA targets (gates on production-deploy authorization Captain explicit OR Phase G ramp verb)
- HALO probe `deploy-source-attribution` against live deployed surfaces (advisory in F9 v1; fail-CLOSED in Phase 3)
- Cross-machine F9 invocation from Bono VPS (currently James-side primary; Bono mirror is operational follow-up)
- CI mirror `.github/workflows/deploy.yml` rewrite (current is V1; F9 v1 update is operational follow-up sub-PACT)
- Manifest signing (HMAC; deferred to F9 v2)
- 7-day soak window for Phase 3 fail-CLOSED enforcement promotion

## Stale-at

Durable until F9 v2 sibling-PACT files (cross-surface transactional rollback) OR Razorpay F12 ACTIVE PACT lands (mechanical PR-merge gate composes-with this F9 v1).
