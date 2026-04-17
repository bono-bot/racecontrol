# Racing Point — Manual Provisioning Audit (Phase 413)

**Generated:** 2026-04-18 04:57 IST by executor (Phase 413, Plan 08)
**Repo commit at sweep:** `0fc38726`
**Scope:** Every manual-state provisioning path on the venue fleet — pods (1-8), POS (.130), server (.23). Cloud (Bono VPS) and comms-link (James .27) are noted for boundary purposes only; a separate audit is suggested for those environments.
**Purpose:** Enumerate known manual-state paths so that a Gap-4-class drift bug (pod HKLM value ≠ server TOML value, silent Tier-0 dead fleet-wide) doesn't resurface unnoticed. Every future discovery must either already be in this document or amend it with the commit hash.

---

## Sweep Methodology

The following grep commands were executed against this repository (`C:\Users\bono\racingpoint\racecontrol`) from the repo root:

```bash
# 1. setx /M occurrences in scripts and docs
grep -rn "setx" scripts/ docs/ 2>/dev/null

# 2. reg add HKLM occurrences in scripts and docs
grep -rn "reg add HKLM" scripts/ docs/ 2>/dev/null

# 3. env var reads in pod-side Rust code (rc-agent + rc-sentry)
grep -rn "std::env::var" crates/rc-agent/ crates/rc-sentry/ 2>/dev/null

# 4. HKLM / HKCU references in scripts and docs
grep -rn "HKLM\\\\|HKCU\\\\" scripts/ docs/ 2>/dev/null

# 5. RCAGENT_SERVICE_KEY / SENTRY_KEY / RCSENTRY_SERVICE_KEY in scripts
grep -rn "RCAGENT_SERVICE_KEY\|SENTRY_KEY\|RCSENTRY_SERVICE_KEY" scripts/ docs/ 2>/dev/null

# 6. process.env.* in scripts/
grep -rn "process.env" scripts/ 2>/dev/null | grep -v node_modules

# 7. JWT_SECRET / OPENROUTER_KEY / COMMS_PSK / RC_JWT_SECRET in scripts
grep -rn "RACECONTROL_JWT_SECRET\|RC_JWT_SECRET\|OPENROUTER_KEY\|OPENROUTER_MGMT_KEY\|COMMS_PSK" scripts/ 2>/dev/null
```

**Findings below are organized by status** — MIGRATED this phase, KEPT as-is, TO-MIGRATE in a future phase, and a negative-space claim for the sweep's completeness.

---

## 1. MIGRATED this phase

Paths that Phase 413 replaces with a server-fetched, periodically-refreshed cache.

| Var / Config | Previous Mechanism | New Mechanism | Plans |
|---|---|---|---|
| `RCAGENT_SERVICE_KEY` (pod-side env var, read in 3 consumers: `ai_debugger.rs:779`, `remote_ops.rs:165`, `ws_handler.rs:431`) | Manual HKLM `setx /M RCAGENT_SERVICE_KEY <value>` set once at pendrive install (`D:\pod-deploy\install.bat`). Per-pod, per-machine, persists until wiped/reimaged. Drifted out of sync with `pods.sentry_service_key` in server's `racecontrol.toml` → Gap 4 (MI Tier 0 dead fleet-wide since MMA-v29 because pods sent a stale key and server's `/audit-check` returned 401 silently). | Server route `GET /api/v1/pods/mesh-service-key` gated by `network_middleware` (pod-IP + LAN CIDR allowlist). rc-agent holds `Arc<RwLock<Option<String>>>` mesh key cache, populated at boot and refreshed every 300s via `rc_common::boot_resilience::spawn_periodic_refetch` (same template as feature-flag re-fetch at `main.rs:1604`). Env var kept as fallback for unit tests only; production cache wins. | 413-01 (server route), 413-02 (rc-agent cache module), 413-03 (rewire 3 consumers), 413-04 (integration tests), 413-05 (deploy), 413-09 (MMA audit) |

**Cross-ref to evidence:** `~/.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_verification_and_gap4.md` documents the Gap 4 root cause and the decision to choose Option Z over X (manual setx script) or Y (separate MESH_SERVICE_KEY field). CONTEXT.md for Phase 413 captures the full decision trail.

---

## 2. KEPT as-is (intentional)

Paths that remain manually provisioned by deliberate decision. Each has a stated rationale for why fetch-at-boot from the server is NOT appropriate.

| Var / Config | Mechanism | Where It Lives | Why Kept |
|---|---|---|---|
| `SENTRY_KEY` / `RCSENTRY_SERVICE_KEY` (rc-sentry service key, separate from rc-agent's mesh key) | Manually set per machine before deploy. Referenced in `scripts/deploy-pod.sh:32`, `scripts/deploy-server.sh:38`, `scripts/deploy-preflight.sh:36`. Operator exports `SENTRY_KEY=<value>` in their shell before invoking any deploy script. | rc-sentry reads it at startup via `std::env::var("RCSENTRY_SERVICE_KEY")` at `crates/rc-sentry/src/main.rs:65`. Deploy scripts send it as `X-Service-Key` header to rc-sentry's `/exec`. | rc-sentry is the **deploy-tool-facing** watchdog; its key is used ONLY by deploy scripts run by a human operator who already has access to the server TOML. A fetch-at-boot pattern would create a chicken-and-egg problem (rc-sentry would need to authenticate itself to fetch its own key). Explicitly out of scope per Phase 413 CONTEXT.md `deferred` section. |
| `OPENROUTER_KEY` | Manual — loaded from `C:\RacingPoint\data\openrouter-mma-key.txt` at rc-agent start (`scripts/deploy/start-rcagent.bat:9`) OR passed as env var by the script. Auto-recovered via OpenRouter management key if rotated (`scripts/lib/openrouter-key-recovery.js`). | Read in `ai_debugger.rs:565` (`OPENROUTER_API_KEY`), `openrouter.rs:279` (`OPENROUTER_KEY`), `rc-sentry/src/mma_engine.rs:22`. Also used in 6+ Node scripts (`multi-model-audit.js`, `kiosk-audit.js`, `gemini-audit.js`, etc.). | MMA-only. Not venue-critical for billing or game launch. James's .27 manages the canonical key. Auto-rotation via `OPENROUTER_MGMT_KEY` is already the recovery path. A server-fetch mesh pattern would leak the MMA key across more services than necessary and add a new attack surface for a non-safety system. |
| `COMMS_PSK` (comms-link pre-shared key) | Set as env var in shell profile or in `~/.claude/comms-link.env`. Hardcoded fallback in 3 scripts: `boot-time-fix.sh:142`, `ecosystem.auto-detect.config.cjs:19`, `server-health-monitor.ps1:82`, `infra-health-monitor.sh:151` (same literal value `85d1d06c...`). | comms-link WebSocket server (James .27 and Bono VPS). Used by relay exec + send-message. | Pre-shared key between James (.27) and Bono (VPS) — a 2-node mesh, not a fleet concern. The venue pods and POS do NOT participate in comms-link. Separate system, separate threat model. Hardcoded fallbacks are a known cleanup item but outside Phase 413 scope. |
| `RC_JWT_SECRET` / `RACECONTROL_JWT_SECRET` (admin web app JWT signing key) | Set in `scripts/deploy/start-admin.bat:11` (hardcoded in bat) and required as env var for `scripts/deploy/deploy-nextjs.sh:25`. Also referenced in `.planning/phases/75-security-audit-foundations/75-02-SUMMARY.md:117` as `setx /M RACECONTROL_JWT_SECRET`. | Admin Next.js app on server .23:3201. Must match `auth.jwt_secret` in `C:\RacingPoint\racecontrol.toml`. | Server-local only — the admin app runs next to racecontrol. No pod consumes this value. Fetching it from the server would be circular (the secret IS the server's secret). Current manual flow is reasonable for a single-node consumer. Future work could have admin read directly from the TOML to eliminate the parallel env var, but that's an admin-app refactor, not a fleet-provisioning concern. |
| `RC_ALLOW_SESSION0` (rc-agent debug env var) | Optional env var; unset in production. If set to `"1"`, bypasses the Session 0 refusal guard at `rc-agent/src/main.rs:629`. | rc-agent (all pods + POS) — dev/test only. | Developer-only escape hatch. Never set in production. Not a secret. No provisioning needed — absence IS the correct state. |
| `RC_DB_ENCRYPTION_KEY` (SQLite encryption) | Documented in `scripts/enable-sqlite-encryption.md:59` but **not currently deployed** (the referenced script is an enablement guide, not active in deploy-server.sh). | Would be server-local if enabled. | Aspirational feature, not in service. If/when enabled, it joins the "server-local TOML-managed" class with `RC_JWT_SECRET`. No provisioning action required today. |
| HKLM `SOFTWARE\Microsoft\Windows\CurrentVersion\Run\RCAgent` and `Run\RCSentry` | Set by `install.bat` on pendrive deploy (`D:\pod-deploy\install.bat`) and by `self_heal.rs:100-105` (repair_registry_key). Points to `C:\RacingPoint\start-rcagent.bat` and `start-rcsentry.bat`. | Pod boot. | Standard Windows software-persistence mechanism. Already **self-healed** by rc-agent's `self_heal::repair_registry_key()` if missing — so while it's "manual" at initial setup, it auto-recovers. Replacing this with a server-fetch pattern would require bootstrapping rc-agent before rc-agent runs — circular. Keep as-is. Documented at `docs/anticheat/risk-inventory.md:53` with LOW risk rating. |
| HKLM `SYSTEM\CurrentControlSet\Services\RaceControl` (service failure actions) | Set by `scripts/deploy/install-server-services.bat:29`. One-time at server setup. | Server .23 only. | Service Control Manager settings. Not a secret, not a fleet concern. Set once per server machine lifetime. |
| HKLM `SYSTEM\CurrentControlSet\Services\RCSentryServer` (service failure actions) | Set by `scripts/deploy/install-server-services.bat:55`. One-time at server setup. | Server .23 only. | Same rationale as above. |
| HKLM `SOFTWARE\OpenSSH\DefaultShell` | Set by `scripts/deploy/setup-ssh.bat:134`. One-time at pod setup to fix SSH shell. | Per-pod, one-time. | OS-level SSH configuration. Not a secret, not churn-prone. Part of first-time pod provisioning. Out of scope. |
| HKLM `SOFTWARE\Policies\Microsoft\Edge\*` (StartupBoostEnabled, BackgroundModeEnabled, HideRestoreDialogEnabled, HideFirstRunExperience) | Set on every rc-agent start by `scripts/deploy/start-rcagent.bat:20-26`. Applied imperatively on each boot — not manual drift-prone because the bat re-applies it every time. | Every pod. | Self-healing by design — the bat file is the source of truth and re-applies on every boot. Already encoded, no drift possible unless the bat itself is stale (covered by "Deploy must include bat file sync" standing rule). |
| `RACECONTROL_SERVER_IP` (optional rc-agent env var) | Optional env var. Falls back to hardcoded `"192.168.31.23"` in `rc-agent/src/diagnostic_engine.rs:589,644` and `tier_engine.rs:445,2522`. | Pod env (rarely set). | Venue topology is stable. Hardcoded default works. Setting it would be for off-venue test harnesses only. Not provisioning-critical. |
| `RACECONTROL_BUILD_ID` (optional rc-agent env var) | Set by build system; read at `mesh_gossip.rs:233`. | Pod env. | Build-time injection, not manual provisioning. |
| `COMPUTERNAME`, `USERNAME`, `APPDATA`, `LOCALAPPDATA`, `TEMP` | Windows-standard env vars, always set by the OS. Read in `rc-agent/src/main.rs:645`, `self_heal.rs:216`, `tier_engine.rs:588,2180,2316`, `process_guard.rs:628`, `startup_cleanup.rs:435,525`. | Every Windows host. | OS-managed. Not provisioning. |

---

## 3. TO-MIGRATE in future phases (candidates)

Paths that LOOK like manual drift today but are NOT tackled by Phase 413. Each has a brief argument for why a boot-fetch pattern (or other systematization) COULD eventually replace them.

| Var / Config | Current Mechanism | Why Migrate | Suggested Phase / Trigger |
|---|---|---|---|
| Cloud-side `RCAGENT_SERVICE_KEY` parity (Bono VPS) | Separate `racecontrol.toml` with separate `pods.sentry_service_key`. Any rc-agent consumers on cloud (if any) are not audited by Phase 413. | If cloud ever grows its own rc-agent-like workers, the venue Option Z cache pattern applies identically. Today cloud runs only racecontrol server + pm2 processes — no rc-agent. But deploy-parity drift has bitten us before. | Trigger: first day cloud adds an agent-class process. Until then, NOT a phase — just a documentation note. |
| `RC_JWT_SECRET` hardcoded in `start-admin.bat:11` | Literal `20141d12282c490a...` in the bat file. Mirrors server TOML but manually duplicated. | Either (a) change admin to read directly from `racecontrol.toml`, or (b) have admin fetch from a trusted local endpoint served by racecontrol. Eliminates manual duplication. | Admin-app refactor phase (lower priority — single-node drift, not fleet-wide). |
| Hardcoded `COMMS_PSK` literal in 4 scripts | Literal `85d1d06c...` in `boot-time-fix.sh:142`, `ecosystem.auto-detect.config.cjs:19`, `server-health-monitor.ps1:82`, `infra-health-monitor.sh:151`. | Secret in git. Should read from env or a local file (like `OPENROUTER_KEY` does via `openrouter-mma-key.txt`). | Cleanup phase — low risk (venue-local) but non-zero exposure. |
| POS (192.168.31.130) network-middleware LAN-IP classification | Plan 413-01 reclassifies POS as a pod-eligible IP for the new mesh-service-key route. But `network_middleware.rs` / `pod_ip_classifier.rs` grep returned NO hits — implying the classifier is either keyed off a different symbol name (possibly embedded in `public_routes` or an IP-CIDR check in middleware) OR the implementation is TBD and lives inside Phase 413 Plan 01. | Verify POST-Plan-01 that POS (.130) is programmatically on the pod allowlist, not just documented to be. If not, file a follow-up. | Verify during Plan 413-05 integration test; amend this doc if POS is accidentally excluded. |
| Pod-specific TOML fragments (`C:\RacingPoint\rc-agent.toml`) | Edited via PowerShell over SSH (see `scripts/rotate-credentials.sh:104`). Each pod has its own copy. Drift-prone class. | Could be fetched from server at boot (similar Option Z pattern) for values that don't vary per pod. Per-pod fields (like pod number) stay local. | Opportunistic — each drift incident is a candidate phase. |
| `STAFF_TOKEN` / admin ops tokens (used by `scripts/deploy/deploy-mi-seed.sh:153`, `bug-tracker-mi-seed.js:353`) | Operator-supplied env var for staff-scoped ops. | Deploy-tool facing, similar to `SENTRY_KEY`. Probably stays manual for the same reason. Flagged here only for completeness. | Not planned. |
| rc-agent `/exec` endpoint auth (per `docs/API-BOUNDARIES.md:527`) | Previously protected by `RCAGENT_SERVICE_KEY` middleware, **middleware was removed** — pods rely on LAN firewall + pod-IP allowlist. | If the threat model ever requires defense-in-depth on the LAN, re-introducing service-key middleware on rc-agent's /exec would use the Option Z cache. Today considered acceptable. | Trigger: any incident indicating intra-LAN compromise. |

---

## 4. Not discovered (negative-space claim)

A sweep of `scripts/` and `docs/` for the 7 grep patterns above (`setx /M`, `reg add HKLM`, `setx`, `HKLM\\`, `RCAGENT_SERVICE_KEY`, `SENTRY_KEY`, `process.env`) found the exhaustive list documented in sections 1, 2, and 3. Grep evidence:

- **`setx /M` calls in venue deploy scripts:** ZERO direct calls. The only `setx /M` references are in planning docs (`.planning/phases/111-.../111-01-SUMMARY.md:37`, `.planning/phases/75-.../75-02-SUMMARY.md:117`, Phase 413 CONTEXT.md) and a Rust comment (`crates/racecontrol/src/ai/providers.rs:98`) noting Windows semantics. **No active bat/ps1 in `scripts/` uses `setx /M`.** Provisioning is done instead via the pendrive `install.bat` (outside this repo — on `D:\pod-deploy\`).
- **`reg add HKLM` in active scripts:** 15 occurrences, all categorized above (Edge policy re-apply, service install, SSH default shell, pendrive installer register-self). None are secret-provisioning.
- **`std::env::var` in rc-agent + rc-sentry:** 21 occurrences, all categorized above. 3 are `RCAGENT_SERVICE_KEY` (the MIGRATED path). The rest are OS env vars (`COMPUTERNAME`, `APPDATA`, etc.), explicit test/debug flags (`RC_ALLOW_SESSION0`), or separately-categorized secrets (`OPENROUTER_KEY`, `RCSENTRY_SERVICE_KEY`, `RACECONTROL_SERVER_IP`, `RACECONTROL_BUILD_ID`).
- **Pendrive kit `D:\pod-deploy\install.bat`**: Not present in this git repo by design (lives on the physical pendrive — "install.bat v5" per CLAUDE.md). It is believed to be the ONE script that originally did the HKLM write for `RCAGENT_SERVICE_KEY`. Post-Phase-413, that write is no longer needed (cache self-heals within 5 min). Recommend amending the pendrive `install.bat` to remove the `RCAGENT_SERVICE_KEY` write in Phase 413 Plan 05 (deploy/verify) — or, at minimum, flag this audit doc at next pendrive regen.

**If a provisioning path surfaces that is NOT in this document, amend this file and reference the commit hash. Do NOT silently add new manual-state paths.**

---

## Cross-References

- **Phase 413 CONTEXT.md** — `decisions` section documents Option Z rationale and the 3 consumer rewires.
- **Plans 413-01 through 413-04** — implement the MIGRATED path (row 1.1).
- **`rc_common::boot_resilience::spawn_periodic_refetch`** — the pattern template. First deployed for the process-guard allowlist in commit `821c3031` (see CLAUDE.md "Boot Resilience" standing rule).
- **Gap 4 evidence** — `~/.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_verification_and_gap4.md`.
- **Standing rule** — CLAUDE.md "Boot Resilience: No single-fetch-at-boot without retry" is the rule this phase upholds for mesh keys.

---

**Last full sweep:** 2026-04-18 04:57 IST, commit `0fc38726`.
**Next sweep trigger:** any new service-key, env-var secret, or HKLM registry write introduced in scripts/ or docs/. Amend this doc in the same commit.
